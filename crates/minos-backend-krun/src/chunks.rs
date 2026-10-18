//! The host side of the guest's chunked output protocol (GUEST-04).

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
        (0..len).map(|i| u8::try_from(i % 251).unwrap_or(0)).collect()
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
        assert!(matches!(result.outcome(), ReceiveOutcome::Fault(ChunkFault::Token)));
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
        assert!(matches!(result.outcome(), ReceiveOutcome::Fault(ChunkFault::Short)));
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
        assert!(matches!(result.outcome(), ReceiveOutcome::Fault(ChunkFault::Oversize)));
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
            thread::sleep(Duration::from_secs(3));
            drop(stream);
        });
        assert!(wait_ended(&receiver));
        assert!(started.elapsed() < Duration::from_secs(3), "the deadline, not the close, ended the read");
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
