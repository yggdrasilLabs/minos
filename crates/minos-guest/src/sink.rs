//! Where PID 1 sends its one output tar (GUEST-04; 05-CONTEXT D-07).
//!
//! PID 1 writes its output through the [`OutputSink`] trait, so the packer never names the transport. Locally the
//! sink is a [`ChunkSink`] over vsock; the AWS milestone adds a lifecycle-hook sink that implements the same
//! trait.
//!
//! **One connection per chunk.** On the pinned `msb_krun_devices` 0.1.41 one vsock connection carries at most 8192
//! bytes guest to host, so the tar travels as a sequence of short connections, each carrying one chunk of at most
//! [`MAX_CHUNK_BYTES`] bytes encoded by minos-core's [`ChunkStream`] (02-SPIKE-EVIDENCE.md S03-ANSWER). Every
//! chunk carries the per-launch token from the input disk header, so the host can tell PID 1's chunks from any
//! other guest process's (05-RESEARCH Pattern 2).
//!
//! **Retry.** The host accepts connections one at a time, so its listen backlog can fill and refuse a connect.
//! A refused or reset connect is retried for the same chunk with bounded backoff, 1 ms doubling to 64 ms, at most
//! 200 attempts per chunk; any other error fails at once (05-RESEARCH Pitfall 6). The END chunk goes last.
//!
//! [`MAX_CHUNK_BYTES`]: minos_core::chunk::MAX_CHUNK_BYTES

use std::io::{self, ErrorKind, Write};
use std::time::Duration;

use minos_core::chunk::{ChunkStream, ChunkToken};
use minos_core::limits::MAX_OUTPUT_STREAM_BYTES;

/// Where PID 1 sends its output tar.
pub trait OutputSink {
    /// Sends the whole output tar.
    ///
    /// # Errors
    ///
    /// A [`SinkError`] naming the chunk that could not be delivered.
    fn send(&mut self, tar: &[u8]) -> Result<SendReport, SinkError>;
}

/// Opens and closes the connections a [`ChunkSink`] sends chunks on.
///
/// The Linux connector (plan 05-12) opens a vsock stream to the host CID on
/// [`OUTPUT_VSOCK_PORT`](minos_core::guest::OUTPUT_VSOCK_PORT); tests use an in-memory one.
pub trait Connector {
    /// One connection's stream.
    type Stream: Write;

    /// Opens a new connection.
    ///
    /// # Errors
    ///
    /// The connect error; [`ErrorKind::ConnectionRefused`] and [`ErrorKind::ConnectionReset`] are retried.
    fn connect(&mut self) -> io::Result<Self::Stream>;

    /// Shuts down the write side of `stream` and closes it, so the host sees the end of the chunk.
    ///
    /// # Errors
    ///
    /// The shutdown or close error.
    fn close(&mut self, stream: Self::Stream) -> io::Result<()>;
}

/// How a [`ChunkSink`] retries a refused or reset connect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Backoff {
    first_ms: u64,
    max_ms: u64,
    attempts: u32,
}

impl Backoff {
    /// The standard backoff: 1 ms doubling to 64 ms, at most 200 connect attempts per chunk. 200 attempts at the
    /// 64 ms ceiling bound one chunk's wait at about 13 s, long enough for the host to drain a full backlog and
    /// short enough that a host that went away does not hold the VM to its whole deadline.
    #[must_use]
    pub const fn standard() -> Self {
        Self {
            first_ms: 1,
            max_ms: 64,
            attempts: 200,
        }
    }

    /// No delay between attempts, still at most 200 per chunk: for tests, which must not sleep.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            first_ms: 0,
            max_ms: 0,
            attempts: 200,
        }
    }

    /// The most connect attempts for one chunk.
    #[must_use]
    pub const fn attempts(&self) -> u32 {
        self.attempts
    }

    /// The delay before retry number `retry` (1 for the first retry): the first delay doubled `retry - 1` times,
    /// never above the ceiling.
    fn delay(&self, retry: u32) -> Duration {
        let doublings = retry.saturating_sub(1);
        let ms = 2_u64
            .checked_pow(doublings)
            .and_then(|factor| self.first_ms.checked_mul(factor))
            .map_or(self.max_ms, |ms| ms.min(self.max_ms));
        Duration::from_millis(ms)
    }
}

/// An [`OutputSink`] that sends the tar as authenticated chunks, one connection per chunk.
pub struct ChunkSink<C: Connector> {
    connector: C,
    token: ChunkToken,
    backoff: Backoff,
}

impl<C: Connector> ChunkSink<C> {
    /// A sink that authenticates every chunk with `token` and connects through `connector`, with the
    /// [`Backoff::standard`] retry.
    #[must_use]
    pub const fn new(connector: C, token: ChunkToken) -> Self {
        Self {
            connector,
            token,
            backoff: Backoff::standard(),
        }
    }

    /// The same sink with `backoff` in place of the standard retry.
    #[must_use]
    pub const fn with_backoff(mut self, backoff: Backoff) -> Self {
        self.backoff = backoff;
        self
    }

    /// Connects for chunk `chunk`, retrying a refused or reset connect. Adds every retry to `retries`.
    fn connect(&mut self, chunk: u32, retries: &mut u32) -> Result<C::Stream, SinkError> {
        let mut attempt: u32 = 1;
        loop {
            match self.connector.connect() {
                Ok(stream) => return Ok(stream),
                Err(error)
                    if matches!(
                        error.kind(),
                        ErrorKind::ConnectionRefused | ErrorKind::ConnectionReset
                    ) && attempt < self.backoff.attempts =>
                {
                    let delay = self.backoff.delay(attempt);
                    if !delay.is_zero() {
                        std::thread::sleep(delay);
                    }
                    *retries = retries.saturating_add(1);
                    attempt = attempt.saturating_add(1);
                }
                Err(error) => {
                    return Err(SinkError::Connect {
                        chunk,
                        attempts: attempt,
                        kind: error.kind(),
                    });
                }
            }
        }
    }
}

impl<C: Connector> OutputSink for ChunkSink<C> {
    fn send(&mut self, tar: &[u8]) -> Result<SendReport, SinkError> {
        let bytes = u64::try_from(tar.len()).unwrap_or(u64::MAX);
        if bytes > MAX_OUTPUT_STREAM_BYTES {
            return Err(SinkError::TooLarge { bytes });
        }
        let mut report = SendReport {
            connections: 0,
            retries: 0,
            bytes: 0,
        };
        let token = self.token.clone();
        for frame in ChunkStream::new(&token, tar) {
            let chunk = report.connections;
            let mut stream = self.connect(chunk, &mut report.retries)?;
            stream.write_all(&frame).map_err(|error| SinkError::Write {
                chunk,
                kind: error.kind(),
            })?;
            self.connector
                .close(stream)
                .map_err(|error| SinkError::Write {
                    chunk,
                    kind: error.kind(),
                })?;
            report.connections = report.connections.saturating_add(1);
            report.bytes = report
                .bytes
                .saturating_add(u64::try_from(frame.len()).unwrap_or(u64::MAX));
        }
        Ok(report)
    }
}

/// What one [`OutputSink::send`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SendReport {
    connections: u32,
    retries: u32,
    bytes: u64,
}

impl SendReport {
    /// The connections that carried a chunk: the DATA chunks and the END.
    #[must_use]
    pub const fn connections(&self) -> u32 {
        self.connections
    }

    /// The connect attempts that were refused or reset and retried, over all chunks.
    #[must_use]
    pub const fn retries(&self) -> u32 {
        self.retries
    }

    /// The bytes written over all connections, chunk headers included.
    #[must_use]
    pub const fn bytes(&self) -> u64 {
        self.bytes
    }
}

/// Why an [`OutputSink`] could not deliver the tar. Payloads are numbers and error kinds only.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SinkError {
    /// The tar is over [`MAX_OUTPUT_STREAM_BYTES`], which the host would refuse; nothing was sent.
    #[error("the output tar is {bytes} bytes; the cap is {MAX_OUTPUT_STREAM_BYTES}")]
    TooLarge {
        /// The tar's length.
        bytes: u64,
    },
    /// Connecting for a chunk failed: an error that is not retried, or the last of the allowed attempts.
    #[error("connecting for output chunk {chunk} failed after {attempts} attempts: {kind}")]
    Connect {
        /// The chunk's seq.
        chunk: u32,
        /// The connect attempts made for it.
        attempts: u32,
        /// The last connect error's kind.
        kind: ErrorKind,
    },
    /// Writing a chunk, or shutting down and closing its connection, failed.
    #[error("writing output chunk {chunk} failed: {kind}")]
    Write {
        /// The chunk's seq.
        chunk: u32,
        /// The error's kind.
        kind: ErrorKind,
    },
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::io::{self, ErrorKind, Write};

    use minos_core::chunk::{ChunkToken, Reassembler};
    use minos_core::limits::MAX_OUTPUT_STREAM_BYTES;

    use super::{Backoff, ChunkSink, Connector, OutputSink, SinkError};

    type TestResult = Result<(), Box<dyn Error>>;

    const TOKEN: [u8; 32] = [0x3c; 32];

    fn token() -> ChunkToken {
        ChunkToken::from_bytes(TOKEN)
    }

    /// One in-memory connection; writes fail when `fail` is set.
    struct Stream {
        bytes: Vec<u8>,
        fail: bool,
    }

    impl Write for Stream {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            if self.fail {
                return Err(io::Error::from(ErrorKind::BrokenPipe));
            }
            self.bytes.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    /// Collects each closed connection's bytes. Refuses the first `refuse_per_chunk` connects of every chunk
    /// (all of them when `None`), and fails the writes of chunk `fail_write_at`.
    struct Memory {
        closed: Vec<Vec<u8>>,
        refuse_per_chunk: Option<u32>,
        refused_this_chunk: u32,
        connects: u32,
        fail_write_at: Option<usize>,
    }

    impl Memory {
        const fn new() -> Self {
            Self {
                closed: Vec::new(),
                refuse_per_chunk: Some(0),
                refused_this_chunk: 0,
                connects: 0,
                fail_write_at: None,
            }
        }
    }

    impl Connector for Memory {
        type Stream = Stream;

        fn connect(&mut self) -> io::Result<Stream> {
            self.connects = self.connects.saturating_add(1);
            let refuse = self
                .refuse_per_chunk
                .is_none_or(|limit| self.refused_this_chunk < limit);
            if refuse {
                self.refused_this_chunk = self.refused_this_chunk.saturating_add(1);
                return Err(io::Error::from(ErrorKind::ConnectionRefused));
            }
            Ok(Stream {
                bytes: Vec::new(),
                fail: self.fail_write_at == Some(self.closed.len()),
            })
        }

        fn close(&mut self, stream: Stream) -> io::Result<()> {
            self.refused_this_chunk = 0;
            self.closed.push(stream.bytes);
            Ok(())
        }
    }

    /// A 20 KiB tar stand-in whose bytes differ by position.
    fn tar() -> Vec<u8> {
        (0..20_480_u32)
            .map(|index| u8::try_from(index % 251).unwrap_or(0))
            .collect()
    }

    fn reassemble(connections: &[Vec<u8>]) -> Result<Vec<u8>, Box<dyn Error>> {
        let mut receiver = Reassembler::new(token(), MAX_OUTPUT_STREAM_BYTES);
        for connection in connections {
            receiver.accept(connection)?;
        }
        Ok(receiver.finish()?)
    }

    #[test]
    fn chunk_sink_round_trip() -> TestResult {
        let tar = tar();
        let mut sink = ChunkSink::new(Memory::new(), token()).with_backoff(Backoff::none());

        let report = sink.send(&tar)?;

        assert_eq!(report.connections(), 6, "5 DATA chunks and the END");
        assert_eq!(report.retries(), 0);
        let closed = &sink.connector.closed;
        let written: usize = closed.iter().map(Vec::len).sum();
        assert_eq!(report.bytes(), u64::try_from(written)?);
        assert!(closed.iter().all(|connection| connection.len() <= 4144));
        assert_eq!(reassemble(closed)?, tar);
        Ok(())
    }

    #[test]
    fn chunk_sink_empty_tar_is_one_end() -> TestResult {
        let mut sink = ChunkSink::new(Memory::new(), token()).with_backoff(Backoff::none());
        let report = sink.send(&[])?;
        assert_eq!(report.connections(), 1);
        assert_eq!(reassemble(&sink.connector.closed)?, Vec::<u8>::new());
        Ok(())
    }

    #[test]
    fn chunk_sink_retries_then_succeeds() -> TestResult {
        let tar = tar();
        let mut connector = Memory::new();
        connector.refuse_per_chunk = Some(2);
        let mut sink = ChunkSink::new(connector, token()).with_backoff(Backoff::none());

        let report = sink.send(&tar)?;

        assert_eq!(report.connections(), 6);
        assert_eq!(report.retries(), 2 * 6, "two refused connects per chunk");
        assert_eq!(sink.connector.connects, 3 * 6);
        assert_eq!(reassemble(&sink.connector.closed)?, tar);
        Ok(())
    }

    #[test]
    fn chunk_sink_gives_up() {
        let mut connector = Memory::new();
        connector.refuse_per_chunk = None;
        let mut sink = ChunkSink::new(connector, token()).with_backoff(Backoff::none());

        let result = sink.send(&tar());

        assert_eq!(
            result,
            Err(SinkError::Connect {
                chunk: 0,
                attempts: 200,
                kind: ErrorKind::ConnectionRefused,
            })
        );
        assert_eq!(sink.connector.connects, 200);
        assert!(sink.connector.closed.is_empty());
    }

    #[test]
    fn chunk_sink_does_not_retry_other_errors() {
        struct Denied;
        impl Connector for Denied {
            type Stream = Vec<u8>;
            fn connect(&mut self) -> io::Result<Vec<u8>> {
                Err(io::Error::from(ErrorKind::PermissionDenied))
            }
            fn close(&mut self, _stream: Vec<u8>) -> io::Result<()> {
                Ok(())
            }
        }
        let mut sink = ChunkSink::new(Denied, token()).with_backoff(Backoff::none());
        assert_eq!(
            sink.send(b"x"),
            Err(SinkError::Connect {
                chunk: 0,
                attempts: 1,
                kind: ErrorKind::PermissionDenied,
            })
        );
    }

    #[test]
    fn chunk_sink_write_error() {
        let mut connector = Memory::new();
        connector.fail_write_at = Some(2);
        let mut sink = ChunkSink::new(connector, token()).with_backoff(Backoff::none());

        let result = sink.send(&tar());

        assert_eq!(
            result,
            Err(SinkError::Write {
                chunk: 2,
                kind: ErrorKind::BrokenPipe,
            })
        );
        assert_eq!(sink.connector.closed.len(), 2);
    }

    #[test]
    fn chunk_sink_refuses_a_tar_over_the_cap_before_connecting() -> TestResult {
        let over = usize::try_from(MAX_OUTPUT_STREAM_BYTES)?.saturating_add(1);
        let tar = vec![0_u8; over];
        let mut sink = ChunkSink::new(Memory::new(), token()).with_backoff(Backoff::none());
        assert_eq!(
            sink.send(&tar),
            Err(SinkError::TooLarge {
                bytes: MAX_OUTPUT_STREAM_BYTES + 1
            })
        );
        assert_eq!(sink.connector.connects, 0);
        Ok(())
    }

    #[test]
    fn standard_backoff_doubles_to_its_ceiling() {
        let backoff = Backoff::standard();
        let delays: Vec<u128> = [1, 2, 3, 4, 7, 8, 199]
            .into_iter()
            .map(|retry| backoff.delay(retry).as_millis())
            .collect();
        assert_eq!(delays, [1, 2, 4, 8, 64, 64, 64]);
        assert_eq!(backoff.attempts(), 200);
        assert!(Backoff::none().delay(5).is_zero());
        assert_eq!(Backoff::none().attempts(), 200);
    }
}
