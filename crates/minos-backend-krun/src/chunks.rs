//! The host side of the guest's chunked output protocol (GUEST-04).
//!
//! The guest's PID 1 sends its output tar as a stream of chunks in core's codec ([`minos_core::chunk`]), one
//! short vsock connection per chunk, so every connection stays under the 8192-byte limit of the pinned
//! `msb_krun_devices` (02-SPIKE-EVIDENCE.md DIAG-01, S03-ANSWER). Each chunk carries the per-launch token, which
//! only PID 1 knows: any guest process, the candidate included, can connect to the output port, so the token is
//! what tells PID 1's stream from a forgery (05-RESEARCH Pattern 2, Pitfall 1).
//!
//! [`ChunkReceiver`] runs on its own thread. It waits on the listener with `poll` rather than sleep polling
//! (05-RESEARCH Pitfall 6), reads each connection through [`MAX_CHUNK_BYTES`] + 1 bytes against its own deadline
//! ([`CONNECTION_DEADLINE`] in production), and feeds it to core's [`Reassembler`], which absorbs out-of-order
//! arrival by buffering by seq under the stream's byte cap (05-RESEARCH A5). The receiver never writes, mounts,
//! or unpacks anything the guest sent.
//!
//! Every fault is a candidate fault on the candidate's own run, [`Reason::ChunkProtocol`], never an infrastructure
//! retry: a wrong token, a gap, a duplicate, an over-cap chunk or stream, a bad END, an empty connection, a
//! connection over [`MAX_CHUNK_BYTES`], or one still open at its deadline. The first fault raises a flag the
//! supervisor can read to stop the VM early, and the receiver stops accepting. A stream with no END is reported
//! apart from a fault, as [`ReceiveOutcome::Incomplete`], so the supervisor can classify a PID 1 that died
//! before finishing as `Crashed` from what it observed of the VM.
//!
//! [`Reason::ChunkProtocol`]: minos_core::outcome::Reason::ChunkProtocol

use std::fmt;
use std::io::{self, ErrorKind, Read};
use std::os::fd::AsFd;
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use minos_core::chunk::{ChunkFault, ChunkToken, MAX_CHUNK_BYTES, Reassembler};
use rustix::event::{PollFd, PollFlags, Timespec};
use rustix::io::Errno;

/// How long one connection may take to deliver its chunk and close: 5 s. A connection still open at its deadline
/// is a fault, so a trickling peer cannot hold the receiver.
pub const CONNECTION_DEADLINE: Duration = Duration::from_secs(5);

/// How long one wait on the listener lasts before the receiver checks whether it was told to stop.
const POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Most bytes read from one connection: one more than a chunk can hold, so an oversize connection is detected
/// rather than silently truncated.
const READ_CAP: usize = MAX_CHUNK_BYTES + 1;

/// How one output stream ended.
///
/// `Debug` shows the length of a complete stream only, never its bytes.
pub enum ReceiveOutcome {
    /// The whole authenticated stream: the guest's output tar, still hostile, for the caller to wrap as untrusted
    /// output.
    Complete(Vec<u8>),
    /// No fault, but the stream stopped before its END chunk: PID 1 did not finish sending.
    Incomplete,
    /// The first fault, a candidate fault ([`ChunkFault::reason`] is always `ChunkProtocol`).
    Fault(ChunkFault),
}

impl fmt::Debug for ReceiveOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Complete(bytes) => f.debug_tuple("Complete").field(&bytes.len()).finish(),
            Self::Incomplete => f.write_str("Incomplete"),
            Self::Fault(fault) => f.debug_tuple("Fault").field(fault).finish(),
        }
    }
}

/// What the receiver observed of one output stream.
#[derive(Debug)]
pub struct ReceiveResult {
    outcome: ReceiveOutcome,
    connections: u32,
    first_byte_to_end: Option<Duration>,
}

impl ReceiveResult {
    /// How the stream ended.
    #[must_use]
    pub const fn outcome(&self) -> &ReceiveOutcome {
        &self.outcome
    }

    /// The outcome, by value.
    #[must_use]
    pub fn into_outcome(self) -> ReceiveOutcome {
        self.outcome
    }

    /// The connections the receiver accepted and read, faulty ones included.
    #[must_use]
    pub const fn connections(&self) -> u32 {
        self.connections
    }

    /// The time from the first accepted connection to the one that completed the stream; `None` unless the stream
    /// completed.
    #[must_use]
    pub const fn first_byte_to_end(&self) -> Option<Duration> {
        self.first_byte_to_end
    }
}

/// The receiver of one launch's output stream, running on its own thread.
///
/// The thread ends by itself on the first fault or once the stream is complete, and otherwise when told to stop.
/// Dropping the receiver tells it to stop without waiting; [`ChunkReceiver::stop_and_join`] waits for its result.
pub struct ChunkReceiver {
    handle: Option<JoinHandle<io::Result<ReceiveResult>>>,
    stop: Arc<AtomicBool>,
    fault: Arc<AtomicBool>,
}

impl ChunkReceiver {
    /// Starts receiving on `listener` the stream of at most `max_total` bytes authenticated by `token`, giving
    /// each connection `deadline` to deliver its chunk ([`CONNECTION_DEADLINE`] in production).
    ///
    /// The listener is made non-blocking and moved to the receiver's thread, which closes it when it ends.
    ///
    /// # Errors
    ///
    /// An [`io::Error`] when the listener cannot be made non-blocking or the thread cannot be started.
    pub fn spawn(
        listener: UnixListener,
        token: ChunkToken,
        max_total: u64,
        deadline: Duration,
    ) -> io::Result<Self> {
        listener.set_nonblocking(true)?;
        let stop = Arc::new(AtomicBool::new(false));
        let fault = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let thread_fault = Arc::clone(&fault);
        let handle = thread::Builder::new()
            .name("minos-chunks".to_owned())
            .spawn(move || {
                let receiver = Receiver {
                    listener,
                    deadline,
                    stop: thread_stop,
                    fault_flag: thread_fault,
                    stream: Stream::new(token, max_total),
                };
                receiver.run()
            })?;
        Ok(Self {
            handle: Some(handle),
            stop,
            fault,
        })
    }

    /// Whether the stream has faulted. The supervisor reads this to stop the VM early.
    #[must_use]
    pub fn faulted(&self) -> bool {
        self.fault.load(Ordering::Acquire)
    }

    /// Whether the receiver's thread has ended: on a fault, a complete stream, or a host failure.
    #[must_use]
    pub fn ended(&self) -> bool {
        self.handle.as_ref().is_none_or(JoinHandle::is_finished)
    }

    /// Tells the receiver to stop and waits for its result.
    ///
    /// A receiver still running accepts the connections already pending once more before it stops, because a
    /// guest's last chunk can land as the helper exits.
    ///
    /// # Errors
    ///
    /// An [`io::Error`] for a host failure: the listener could not be polled or accepted on, or the receiver's
    /// thread panicked. Each is an infrastructure failure, never a candidate fault.
    pub fn stop_and_join(mut self) -> io::Result<ReceiveResult> {
        self.stop.store(true, Ordering::Release);
        let handle = self
            .handle
            .take()
            .ok_or_else(|| io::Error::other("the chunk receiver was already joined"))?;
        handle
            .join()
            .map_err(|_| io::Error::other("the chunk receiver thread panicked"))?
    }
}

impl Drop for ChunkReceiver {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}

impl fmt::Debug for ChunkReceiver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChunkReceiver")
            .field("faulted", &self.faulted())
            .field("ended", &self.ended())
            .finish_non_exhaustive()
    }
}

/// The stream state the receiver's thread keeps.
struct Stream {
    reassembler: Reassembler,
    fault: Option<ChunkFault>,
    connections: u32,
    first: Option<Instant>,
    end: Option<Instant>,
}

impl Stream {
    fn new(token: ChunkToken, max_total: u64) -> Self {
        Self {
            reassembler: Reassembler::new(token, max_total),
            fault: None,
            connections: 0,
            first: None,
            end: None,
        }
    }

    /// Whether the receiver has nothing more to accept: a fault, or a complete stream.
    fn done(&self) -> bool {
        self.fault.is_some() || self.end.is_some()
    }

    /// Records one connection: its bytes, or the fault reading it raised.
    fn record(&mut self, read: Result<Vec<u8>, ChunkFault>) {
        self.connections = self.connections.saturating_add(1);
        let now = Instant::now();
        self.first.get_or_insert(now);
        let accepted = read.and_then(|bytes| self.reassembler.accept(&bytes));
        match accepted {
            Err(fault) => self.fault = Some(fault),
            Ok(()) if self.reassembler.is_complete() => self.end = Some(now),
            Ok(()) => {}
        }
    }

    fn finish(self) -> ReceiveResult {
        let first_byte_to_end = self
            .first
            .zip(self.end)
            .map(|(first, end)| end.saturating_duration_since(first));
        let outcome = match self.fault {
            Some(fault) => ReceiveOutcome::Fault(fault),
            None => match self.reassembler.finish() {
                Ok(bytes) => ReceiveOutcome::Complete(bytes),
                Err(fault) if fault.is_incomplete() => ReceiveOutcome::Incomplete,
                Err(fault) => ReceiveOutcome::Fault(fault),
            },
        };
        ReceiveResult {
            outcome,
            connections: self.connections,
            first_byte_to_end,
        }
    }
}

/// The receiver's thread.
struct Receiver {
    listener: UnixListener,
    deadline: Duration,
    stop: Arc<AtomicBool>,
    fault_flag: Arc<AtomicBool>,
    stream: Stream,
}

impl Receiver {
    fn run(mut self) -> io::Result<ReceiveResult> {
        loop {
            let stopping = self.stop.load(Ordering::Acquire);
            // After a stop, the pending connections are accepted once more without waiting.
            if stopping || readable(&self.listener, POLL_INTERVAL)? {
                self.accept_pending()?;
            }
            if self.stream.done() || stopping {
                break;
            }
        }
        Ok(self.stream.finish())
    }

    /// Accepts and reads connections until none is pending, the stream faults, or it is complete.
    ///
    /// Bounded: every connection is counted by the reassembler or ends the stream with a fault, and the
    /// reassembler refuses more connections than a stream under its cap can need.
    fn accept_pending(&mut self) -> io::Result<()> {
        while !self.stream.done() {
            let connection = match self.listener.accept() {
                Ok((connection, _)) => connection,
                Err(error) if error.kind() == ErrorKind::WouldBlock => return Ok(()),
                // A peer that reset before the accept is not a host failure.
                Err(error)
                    if matches!(
                        error.kind(),
                        ErrorKind::Interrupted | ErrorKind::ConnectionAborted
                    ) =>
                {
                    continue;
                }
                Err(error) => return Err(error),
            };
            let read = read_connection(&connection, self.deadline)?;
            self.stream.record(read);
            if self.stream.fault.is_some() {
                self.fault_flag.store(true, Ordering::Release);
            }
        }
        Ok(())
    }
}

/// Waits up to `timeout` for `fd` to be readable, or hung up. `false` when the wait timed out.
fn readable(fd: &impl AsFd, timeout: Duration) -> io::Result<bool> {
    let timeout = Timespec::try_from(timeout).map_err(io::Error::other)?;
    let mut fds = [PollFd::new(fd, PollFlags::IN)];
    loop {
        match rustix::event::poll(&mut fds, Some(&timeout)) {
            Ok(ready) => return Ok(ready > 0),
            Err(Errno::INTR) => {}
            Err(errno) => return Err(errno.into()),
        }
    }
}

/// Reads one connection to its end, through at most [`READ_CAP`] bytes, within `deadline`.
///
/// The connection is read non-blocking and polled against one total deadline rather than with `SO_RCVTIMEO`,
/// which is a per-read timeout a trickling peer can renew (Phase 2 decision 02-05). The inner result is the
/// candidate's: the bytes, [`ChunkFault::Oversize`] for a connection over [`MAX_CHUNK_BYTES`], or
/// [`ChunkFault::Length`] for one still open at its deadline, whose chunk did not arrive whole. A peer error ends
/// the connection with the bytes already read. The outer error is the host's: the stream could not be made
/// non-blocking or polled.
fn read_connection(
    connection: &UnixStream,
    deadline: Duration,
) -> io::Result<Result<Vec<u8>, ChunkFault>> {
    connection.set_nonblocking(true)?;
    let until = Instant::now()
        .checked_add(deadline)
        .ok_or_else(|| io::Error::other("the connection deadline overflows the clock"))?;
    let mut bytes = Vec::with_capacity(READ_CAP);
    let mut buf = [0_u8; READ_CAP];
    let mut reader = connection;
    loop {
        let left = until.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Ok(Err(ChunkFault::Length));
        }
        let room = buf
            .get_mut(..READ_CAP.saturating_sub(bytes.len()))
            .ok_or_else(|| io::Error::other("the read cap was passed"))?;
        match reader.read(room) {
            Ok(0) => return Ok(Ok(bytes)),
            Ok(read) => {
                let part = room
                    .get(..read)
                    .ok_or_else(|| io::Error::other("a read returned more than its buffer"))?;
                bytes.extend_from_slice(part);
                if bytes.len() > MAX_CHUNK_BYTES {
                    return Ok(Err(ChunkFault::Oversize));
                }
            }
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                readable(connection, left)?;
            }
            Err(error) if error.kind() == ErrorKind::Interrupted => {}
            Err(_) => return Ok(Ok(bytes)),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::io::{self, ErrorKind, Write};
    use std::net::Shutdown;
    use std::os::unix::net::{UnixListener, UnixStream};
    use std::path::{Path, PathBuf};
    use std::thread::{self, JoinHandle};
    use std::time::{Duration, Instant};

    use minos_core::chunk::{
        ChunkFault, ChunkStream, ChunkToken, MAX_CHUNK_BYTES, MAX_CHUNK_PAYLOAD, TOKEN_BYTES,
    };
    use tempfile::TempDir;

    use super::{CONNECTION_DEADLINE, ChunkReceiver, ReceiveOutcome};

    type TestResult = Result<(), Box<dyn Error>>;

    const TOKEN: [u8; TOKEN_BYTES] = [0x7e; TOKEN_BYTES];
    /// The receivers' byte cap in these tests, above every payload they send.
    const MAX_TOTAL: u64 = 32 * 1024 * 1024;
    /// How long a test waits for a receiver to end by itself.
    const WAIT: Duration = Duration::from_secs(30);

    /// A listener on a socket in a fresh temporary directory, and the socket's path.
    fn listen() -> Result<(TempDir, UnixListener, PathBuf), Box<dyn Error>> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("o.sock");
        let listener = UnixListener::bind(&path)?;
        Ok((dir, listener, path))
    }

    fn spawn(listener: UnixListener, deadline: Duration) -> io::Result<ChunkReceiver> {
        ChunkReceiver::spawn(listener, ChunkToken::from_bytes(TOKEN), MAX_TOTAL, deadline)
    }

    /// Connects to `path`, retrying a refused or reset connect as the guest does when the backlog is full.
    fn connect(path: &Path) -> io::Result<UnixStream> {
        let give_up = Instant::now() + WAIT;
        loop {
            match UnixStream::connect(path) {
                Ok(stream) => return Ok(stream),
                Err(error)
                    if matches!(
                        error.kind(),
                        ErrorKind::ConnectionRefused
                            | ErrorKind::ConnectionReset
                            | ErrorKind::WouldBlock
                            | ErrorKind::Interrupted
                    ) && Instant::now() < give_up =>
                {
                    thread::sleep(Duration::from_millis(1));
                }
                Err(error) => return Err(error),
            }
        }
    }

    /// Sends `chunk` over one connection, as the guest does: connect, write it whole, shut down the write side.
    fn send(path: &Path, chunk: &[u8]) -> io::Result<()> {
        let mut stream = connect(path)?;
        stream.write_all(chunk)?;
        stream.shutdown(Shutdown::Write)
    }

    /// A client thread sending each of `chunks` over its own connection, in order.
    fn client(path: &Path, chunks: Vec<Vec<u8>>) -> JoinHandle<io::Result<()>> {
        let path = path.to_path_buf();
        thread::spawn(move || chunks.iter().try_for_each(|chunk| send(&path, chunk)))
    }

    fn join(handle: JoinHandle<io::Result<()>>) -> TestResult {
        handle.join().map_err(|_| "the client thread panicked")??;
        Ok(())
    }

    /// The chunks of `payload` under `token`, END last.
    fn chunks(token: [u8; TOKEN_BYTES], payload: &[u8]) -> Vec<Vec<u8>> {
        ChunkStream::new(&ChunkToken::from_bytes(token), payload).collect()
    }

    fn pattern(len: usize) -> Vec<u8> {
        (0..len)
            .map(|i| u8::try_from(i % 251).unwrap_or(0))
            .collect()
    }

    /// Waits up to [`WAIT`] for the receiver's thread to end by itself.
    fn wait_ended(receiver: &ChunkReceiver) -> bool {
        let give_up = Instant::now() + WAIT;
        while !receiver.ended() {
            if Instant::now() >= give_up {
                return false;
            }
            thread::sleep(Duration::from_millis(5));
        }
        true
    }

    #[test]
    fn receive_complete_stream() -> TestResult {
        let (_dir, listener, path) = listen()?;
        let receiver = spawn(listener, CONNECTION_DEADLINE)?;
        let payload = pattern(100 * 1024);
        join(client(&path, chunks(TOKEN, &payload)))?;
        // A complete stream ends the receiver early, with no stop.
        assert!(wait_ended(&receiver));
        assert!(!receiver.faulted());
        let result = receiver.stop_and_join()?;
        assert!(matches!(result.outcome(), ReceiveOutcome::Complete(bytes) if *bytes == payload));
        assert_eq!(result.connections(), 26);
        assert!(result.first_byte_to_end().is_some());
        Ok(())
    }

    #[test]
    fn receive_out_of_order() -> TestResult {
        let (_dir, listener, path) = listen()?;
        let receiver = spawn(listener, CONNECTION_DEADLINE)?;
        let payload = pattern(5 * MAX_CHUNK_PAYLOAD + 7);
        let mut sent = chunks(TOKEN, &payload);
        sent.reverse();
        join(client(&path, sent))?;
        assert!(wait_ended(&receiver));
        let result = receiver.stop_and_join()?;
        assert!(matches!(result.outcome(), ReceiveOutcome::Complete(bytes) if *bytes == payload));
        Ok(())
    }

    #[test]
    fn receive_wrong_token() -> TestResult {
        let (_dir, listener, path) = listen()?;
        let receiver = spawn(listener, CONNECTION_DEADLINE)?;
        let forged = chunks([0; TOKEN_BYTES], b"forged output");
        let first = forged.first().ok_or("no chunk")?;
        send(&path, first)?;
        assert!(wait_ended(&receiver));
        assert!(receiver.faulted());
        // The receiver stopped accepting: its listener is closed.
        assert!(UnixStream::connect(&path).is_err());
        let result = receiver.stop_and_join()?;
        assert!(matches!(
            result.outcome(),
            ReceiveOutcome::Fault(ChunkFault::Token)
        ));
        assert_eq!(result.connections(), 1);
        Ok(())
    }

    #[test]
    fn receive_empty_connection() -> TestResult {
        let (_dir, listener, path) = listen()?;
        let receiver = spawn(listener, CONNECTION_DEADLINE)?;
        drop(connect(&path)?);
        assert!(wait_ended(&receiver));
        assert!(receiver.faulted());
        let result = receiver.stop_and_join()?;
        assert!(matches!(
            result.outcome(),
            ReceiveOutcome::Fault(ChunkFault::Short)
        ));
        Ok(())
    }

    #[test]
    fn receive_oversize_connection() -> TestResult {
        let (_dir, listener, path) = listen()?;
        let receiver = spawn(listener, CONNECTION_DEADLINE)?;
        // A whole valid chunk followed by 100 more bytes: only the size can refuse it.
        let mut oversize = chunks(TOKEN, &pattern(MAX_CHUNK_PAYLOAD))
            .into_iter()
            .next()
            .ok_or("no chunk")?;
        oversize.extend(std::iter::repeat_n(0_u8, 100));
        assert_eq!(oversize.len(), MAX_CHUNK_BYTES + 100);
        let mut stream = connect(&path)?;
        // The receiver may close the connection once it has read MAX_CHUNK_BYTES + 1 bytes, so the tail of this
        // write can fail; the receiver's verdict is what the test checks.
        stream.write_all(&oversize).ok();
        assert!(wait_ended(&receiver));
        let result = receiver.stop_and_join()?;
        assert!(matches!(
            result.outcome(),
            ReceiveOutcome::Fault(ChunkFault::Oversize)
        ));
        Ok(())
    }

    #[test]
    fn receive_trickle() -> TestResult {
        let (_dir, listener, path) = listen()?;
        let receiver = spawn(listener, Duration::from_millis(200))?;
        let mut stream = connect(&path)?;
        let started = Instant::now();
        stream.write_all(&[0x4d; 10])?;
        // The client holds the connection open far past the 200 ms deadline.
        let held = thread::spawn(move || {
            thread::sleep(Duration::from_secs(1));
            drop(stream);
        });
        assert!(wait_ended(&receiver));
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "the deadline, not the close, ended the read"
        );
        assert!(receiver.faulted());
        let result = receiver.stop_and_join()?;
        assert!(matches!(
            result.outcome(),
            ReceiveOutcome::Fault(ChunkFault::Length | ChunkFault::Short)
        ));
        held.join().map_err(|_| "the holding thread panicked")?;
        Ok(())
    }

    #[test]
    fn receive_no_end() -> TestResult {
        let (_dir, listener, path) = listen()?;
        let receiver = spawn(listener, CONNECTION_DEADLINE)?;
        let mut sent = chunks(TOKEN, &pattern(3 * MAX_CHUNK_PAYLOAD));
        sent.pop();
        join(client(&path, sent))?;
        // Every chunk the client sent is pending or read; the stop drains what is pending.
        let result = receiver.stop_and_join()?;
        assert!(matches!(result.outcome(), ReceiveOutcome::Incomplete));
        assert_eq!(result.connections(), 3);
        assert!(result.first_byte_to_end().is_none());
        Ok(())
    }

    #[test]
    fn receive_nothing() -> TestResult {
        let (_dir, listener, _path) = listen()?;
        let receiver = spawn(listener, CONNECTION_DEADLINE)?;
        let result = receiver.stop_and_join()?;
        assert!(matches!(result.outcome(), ReceiveOutcome::Incomplete));
        assert_eq!(result.connections(), 0);
        Ok(())
    }

    #[test]
    fn receive_backlog() -> TestResult {
        let (_dir, listener, path) = listen()?;
        let receiver = spawn(listener, CONNECTION_DEADLINE)?;
        // 4999 DATA chunks and the END: 5000 connections, sent as fast as the client can connect.
        let payload = pattern(4999 * MAX_CHUNK_PAYLOAD);
        let sent = chunks(TOKEN, &payload);
        assert_eq!(sent.len(), 5000);
        join(client(&path, sent))?;
        assert!(wait_ended(&receiver));
        let result = receiver.stop_and_join()?;
        assert!(matches!(result.outcome(), ReceiveOutcome::Complete(bytes) if *bytes == payload));
        assert_eq!(result.connections(), 5000);
        Ok(())
    }
}
