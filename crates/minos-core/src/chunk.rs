//! The authenticated chunk codec that carries guest output to the host over short vsock connections (GUEST-04).
//!
//! **Why chunks.** On the pinned `msb_krun_devices` 0.1.41 one vsock connection carries at most 8192 bytes guest to
//! host; above that, data is lost or the guest stalls, for three defects in that crate (02-SPIKE-EVIDENCE.md
//! DIAG-01). By the user's decision the output is a sequence of short connections instead (S03-ANSWER). Each
//! connection carries exactly one chunk of at most [`MAX_CHUNK_BYTES`] (4144) bytes, so each stays under the 8 KiB
//! send buffer, under the 32 KiB packet limit, and under the 256 KiB credit window, and all three defects stay
//! dormant. The guest sends with [`ChunkStream`] and the host receives with [`Reassembler`]; both sides use this
//! one module, so the codec cannot drift between them.
//!
//! **Why a token.** Any guest process, the candidate included, can connect to the host's output port, so a
//! sequence number alone would let the candidate forge or interleave chunks (05-RESEARCH Pitfall 1). Every chunk
//! therefore carries a 32-byte per-launch [`ChunkToken`] that only PID 1 holds: the backend writes it into the
//! input disk, which only root can read, and the guest never stores or logs it. The receiver compares it in
//! constant time over all 32 bytes.
//!
//! **Layout.** One connection is a 48-byte header and its payload, all integers big-endian:
//!
//! ```text
//!  0   8  magic     b"MINOSCK1"
//!  8  32  token     the per-launch ChunkToken
//! 40   4  seq       u32, 0-based; DATA and END share one counter
//! 44   1  kind      0 = DATA, 1 = END
//! 45   1  reserved  0
//! 46   2  len       u16, 1 to 4096 for DATA, exactly 44 for END
//! 48 len  payload
//! ```
//!
//! DATA chunks carry the stream in order, every one full (4096 bytes) but the last. The END chunk is sent last,
//! with a seq equal to the DATA count, and its payload is the total length (`u64`), the SHA-256 of the
//! concatenated DATA payloads (32 bytes), and the DATA count (`u32`). An empty stream is a single END.
//!
//! **Classification.** The receiver buffers by seq, so the order in which connections arrive never matters: a
//! reorder caused by the VMM is not blamed on the candidate. Every [`ChunkFault`] maps to
//! [`Reason::ChunkProtocol`], a candidate fault that is never retried, except one distinction the host needs:
//! [`ChunkFault::NoEnd`] is [`ChunkFault::is_incomplete`], a stream that stopped before its END, which the host
//! classifies from what it observed itself (a guest that died mid-send is `Crashed`, not a protocol violation).

use std::collections::BTreeMap;
use std::fmt;
use std::iter::FusedIterator;

use sha2::{Digest, Sha256};

use crate::outcome::Reason;

/// The first 8 bytes of every chunk.
pub const CHUNK_MAGIC: [u8; 8] = *b"MINOSCK1";

/// Bytes in a [`ChunkToken`].
pub const TOKEN_BYTES: usize = 32;

/// Bytes in a chunk header: magic, token, seq, kind, reserved, and len.
pub const CHUNK_HEADER_BYTES: usize = 48;

/// Most payload bytes in one chunk.
pub const MAX_CHUNK_PAYLOAD: usize = 4096;

/// Most bytes on one connection: a header and a full payload.
pub const MAX_CHUNK_BYTES: usize = 4144;

/// Bytes in an END chunk's payload: total length, digest, and DATA count.
pub const END_PAYLOAD_BYTES: usize = 44;

/// The `kind` byte of a DATA chunk.
pub const KIND_DATA: u8 = 0;

/// The `kind` byte of the END chunk.
pub const KIND_END: u8 = 1;

/// One vsock connection carries at most 8192 bytes guest to host on the pinned VMM (S03-ANSWER).
const VSOCK_CONNECTION_LIMIT: usize = 8192;

const _: () = assert!(MAX_CHUNK_BYTES == CHUNK_HEADER_BYTES + MAX_CHUNK_PAYLOAD);
const _: () = assert!(MAX_CHUNK_BYTES <= VSOCK_CONNECTION_LIMIT);
const _: () = assert!(CHUNK_HEADER_BYTES == CHUNK_MAGIC.len() + TOKEN_BYTES + 4 + 1 + 1 + 2);
const _: () = assert!(END_PAYLOAD_BYTES == 8 + 32 + 4);
const _: () = assert!(MAX_CHUNK_PAYLOAD <= u16::MAX as usize);

/// [`MAX_CHUNK_PAYLOAD`] as a `u64`, for stream-length arithmetic.
const MAX_CHUNK_PAYLOAD_U64: u64 = 4096;
const _: () = assert!(MAX_CHUNK_PAYLOAD as u64 == MAX_CHUNK_PAYLOAD_U64);

// ---------------------------------------------------------------------------------------------------------------
// ChunkToken
// ---------------------------------------------------------------------------------------------------------------

/// The 32-byte per-launch secret that authenticates every chunk of one VM's output.
///
/// It is `Clone` but not `Copy`, so it is never duplicated silently; it has no `PartialEq` (compare with
/// [`ChunkToken::ct_eq`]), no `Display`, and no serde, and its `Debug` prints `ChunkToken(..)`, never the bytes.
#[derive(Clone)]
pub struct ChunkToken([u8; TOKEN_BYTES]);

impl ChunkToken {
    /// A token from its bytes. The backend draws them from the OS random source for each launch.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; TOKEN_BYTES]) -> Self {
        Self(bytes)
    }

    /// Whether `other` is exactly this token, compared in constant time over all 32 bytes.
    ///
    /// A slice of any other length is never equal. The comparison folds every byte's difference before it decides,
    /// so its timing does not reveal where the first mismatch is.
    #[must_use]
    pub fn ct_eq(&self, other: &[u8]) -> bool {
        let Ok(other) = <&[u8; TOKEN_BYTES]>::try_from(other) else {
            return false;
        };
        let difference = self
            .0
            .iter()
            .zip(other)
            .fold(0_u8, |acc, (mine, theirs)| acc | (mine ^ theirs));
        std::hint::black_box(difference) == 0
    }

    /// The token bytes, for the encoder in this module.
    pub(crate) const fn as_bytes(&self) -> &[u8; TOKEN_BYTES] {
        &self.0
    }
}

impl fmt::Debug for ChunkToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ChunkToken(..)")
    }
}

/// Encodes one chunk, or `None` if the payload does not fit a `u16` length.
fn encode(token: &ChunkToken, seq: u32, kind: u8, payload: &[u8]) -> Option<Vec<u8>> {
    let len = u16::try_from(payload.len()).ok()?;
    let mut out = Vec::with_capacity(CHUNK_HEADER_BYTES + payload.len());
    out.extend_from_slice(&CHUNK_MAGIC);
    out.extend_from_slice(token.as_bytes());
    out.extend_from_slice(&seq.to_be_bytes());
    out.push(kind);
    out.push(0);
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(payload);
    Some(out)
}

// ---------------------------------------------------------------------------------------------------------------
// ChunkStream
// ---------------------------------------------------------------------------------------------------------------

/// The chunks of one payload, in send order: DATA chunks of at most [`MAX_CHUNK_PAYLOAD`] bytes with seq `0..`,
/// then one END chunk whose seq is the DATA count. Each item is one connection's bytes.
///
/// A payload of more DATA chunks than a `u32` seq can number ends without an END, which the receiver reports as
/// [`ChunkFault::NoEnd`]; callers cap the payload first (see [`ChunkStream::chunk_count`]).
pub struct ChunkStream<'a> {
    token: &'a ChunkToken,
    data: std::slice::Chunks<'a, u8>,
    total: Option<u64>,
    seq: u32,
    hasher: Sha256,
    done: bool,
}

impl<'a> ChunkStream<'a> {
    /// The chunks of `payload`, authenticated with `token`.
    #[must_use]
    pub fn new(token: &'a ChunkToken, payload: &'a [u8]) -> Self {
        Self {
            token,
            data: payload.chunks(MAX_CHUNK_PAYLOAD),
            total: u64::try_from(payload.len()).ok(),
            seq: 0,
            hasher: Sha256::new(),
            done: false,
        }
    }

    /// The number of chunks, and so of connections, a payload of `payload_len` bytes takes: its DATA count plus
    /// one END. `None` when that count does not fit a `u32`.
    #[must_use]
    pub fn chunk_count(payload_len: u64) -> Option<u32> {
        u32::try_from(payload_len.div_ceil(MAX_CHUNK_PAYLOAD_U64))
            .ok()?
            .checked_add(1)
    }

    /// Ends the stream: no more items.
    fn stop(&mut self) -> Option<Vec<u8>> {
        self.done = true;
        None
    }
}

impl Iterator for ChunkStream<'_> {
    type Item = Vec<u8>;

    fn next(&mut self) -> Option<Vec<u8>> {
        if self.done {
            return None;
        }
        if let Some(piece) = self.data.next() {
            let seq = self.seq;
            // The END needs a seq after the last DATA chunk, so the last DATA seq is u32::MAX - 1.
            let Some(next) = seq.checked_add(1) else {
                return self.stop();
            };
            self.seq = next;
            self.hasher.update(piece);
            return encode(self.token, seq, KIND_DATA, piece).or_else(|| self.stop());
        }
        self.done = true;
        let total = self.total?;
        let digest: [u8; 32] = std::mem::take(&mut self.hasher).finalize().into();
        let mut trailer = [0_u8; END_PAYLOAD_BYTES];
        let (total_bytes, rest) = trailer.split_at_mut(8);
        let (digest_bytes, count_bytes) = rest.split_at_mut(32);
        total_bytes.copy_from_slice(&total.to_be_bytes());
        digest_bytes.copy_from_slice(&digest);
        count_bytes.copy_from_slice(&self.seq.to_be_bytes());
        encode(self.token, self.seq, KIND_END, &trailer)
    }
}

impl FusedIterator for ChunkStream<'_> {}

// ---------------------------------------------------------------------------------------------------------------
// ChunkFault
// ---------------------------------------------------------------------------------------------------------------

/// Why an output stream broke the chunk protocol.
///
/// Every variant is a candidate fault, [`Reason::ChunkProtocol`], never retried, except that
/// [`ChunkFault::NoEnd`] is [`ChunkFault::is_incomplete`]: the stream stopped before its END, which the host
/// classifies from its own observation of the VM. Messages carry numbers only, never stream bytes.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ChunkFault {
    /// A connection is shorter than the 48-byte header.
    #[error("an output chunk is shorter than its {CHUNK_HEADER_BYTES}-byte header")]
    Short,
    /// A chunk does not start with [`CHUNK_MAGIC`].
    #[error("an output chunk has the wrong magic")]
    Magic,
    /// A chunk carries the wrong token: it was not sent by the guest's PID 1.
    #[error("an output chunk carries the wrong token")]
    Token,
    /// A chunk's kind is neither DATA nor END.
    #[error("an output chunk has an unknown kind")]
    Kind,
    /// A chunk's reserved byte is not 0.
    #[error("an output chunk's reserved byte is not 0")]
    Reserved,
    /// A chunk's length field exceeds [`MAX_CHUNK_PAYLOAD`].
    #[error("an output chunk is over {MAX_CHUNK_PAYLOAD} payload bytes")]
    Oversize,
    /// A chunk is mis-sized: its length field disagrees with the bytes received, an END payload is not
    /// [`END_PAYLOAD_BYTES`], a DATA chunk is empty, or a DATA chunk before the last is not full.
    #[error("an output chunk is mis-sized")]
    Length,
    /// Two DATA chunks share a seq.
    #[error("output chunk {seq} arrived twice")]
    Duplicate {
        /// The repeated seq.
        seq: u32,
    },
    /// A DATA chunk's seq is at or past the END's DATA count.
    #[error("output chunk {seq} lies past the end of the stream")]
    AfterEnd {
        /// The DATA chunk's seq.
        seq: u32,
    },
    /// A second END chunk arrived.
    #[error("the output stream has a second end chunk")]
    SecondEnd,
    /// A DATA chunk below the END's count never arrived.
    #[error("output chunk {seq} is missing")]
    Gap {
        /// The first missing seq.
        seq: u32,
    },
    /// The END's seq, total length, or DATA count disagrees with the DATA received.
    #[error("the output stream's end chunk disagrees with its data")]
    Totals,
    /// The END's SHA-256 does not match the DATA received.
    #[error("the output stream's digest does not match its data")]
    Digest,
    /// The stream holds or claims more bytes than the receiver's cap.
    #[error("the output stream is over its byte cap")]
    OverCap,
    /// More connections arrived than a stream under the cap can need.
    #[error("the output stream used too many connections")]
    TooManyConnections,
    /// The stream stopped before its END chunk.
    #[error("the output stream ended without an end chunk")]
    NoEnd,
}

impl ChunkFault {
    /// Whether the stream is merely incomplete ([`ChunkFault::NoEnd`]) rather than malformed. The host classifies
    /// an incomplete stream from what it observed of the VM, not as a protocol violation.
    #[must_use]
    pub const fn is_incomplete(&self) -> bool {
        matches!(self, Self::NoEnd)
    }

    /// The outcome reason for this fault: always [`Reason::ChunkProtocol`]. The host decides separately, with
    /// [`ChunkFault::is_incomplete`], whether to use it.
    #[must_use]
    pub const fn reason(&self) -> Reason {
        match self {
            Self::Short
            | Self::Magic
            | Self::Token
            | Self::Kind
            | Self::Reserved
            | Self::Oversize
            | Self::Length
            | Self::Duplicate { .. }
            | Self::AfterEnd { .. }
            | Self::SecondEnd
            | Self::Gap { .. }
            | Self::Totals
            | Self::Digest
            | Self::OverCap
            | Self::TooManyConnections
            | Self::NoEnd => Reason::ChunkProtocol,
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Reassembler
// ---------------------------------------------------------------------------------------------------------------

/// The END chunk's claims.
#[derive(Clone, Copy)]
struct EndTrailer {
    total: u64,
    digest: [u8; 32],
    count: u32,
}

/// One parsed connection.
enum Chunk<'a> {
    Data { seq: u32, payload: &'a [u8] },
    End(EndTrailer),
}

/// Takes the first `N` bytes of `bytes`, or [`ChunkFault::Short`].
fn take<const N: usize>(bytes: &[u8]) -> Result<(&[u8; N], &[u8]), ChunkFault> {
    bytes.split_first_chunk::<N>().ok_or(ChunkFault::Short)
}

/// Parses and authenticates one connection's bytes.
fn parse<'a>(token: &ChunkToken, connection: &'a [u8]) -> Result<Chunk<'a>, ChunkFault> {
    let (header, payload) = connection
        .split_at_checked(CHUNK_HEADER_BYTES)
        .ok_or(ChunkFault::Short)?;
    let (magic, rest) = take::<8>(header)?;
    let (sent_token, rest) = take::<TOKEN_BYTES>(rest)?;
    let (seq, rest) = take::<4>(rest)?;
    let &[kind, reserved, len_hi, len_lo] = rest else {
        return Err(ChunkFault::Short);
    };
    if *magic != CHUNK_MAGIC {
        return Err(ChunkFault::Magic);
    }
    if !token.ct_eq(sent_token) {
        return Err(ChunkFault::Token);
    }
    if kind != KIND_DATA && kind != KIND_END {
        return Err(ChunkFault::Kind);
    }
    if reserved != 0 {
        return Err(ChunkFault::Reserved);
    }
    let len = usize::from(u16::from_be_bytes([len_hi, len_lo]));
    if len > MAX_CHUNK_PAYLOAD {
        return Err(ChunkFault::Oversize);
    }
    if payload.len() != len {
        return Err(ChunkFault::Length);
    }
    let seq = u32::from_be_bytes(*seq);
    if kind == KIND_DATA {
        if payload.is_empty() {
            return Err(ChunkFault::Length);
        }
        return Ok(Chunk::Data { seq, payload });
    }
    let trailer: &[u8; END_PAYLOAD_BYTES] = payload.try_into().map_err(|_| ChunkFault::Length)?;
    let (total, rest) = take::<8>(trailer)?;
    let (digest, rest) = take::<32>(rest)?;
    let count: &[u8; 4] = rest.try_into().map_err(|_| ChunkFault::Length)?;
    let end = EndTrailer {
        total: u64::from_be_bytes(*total),
        digest: *digest,
        count: u32::from_be_bytes(*count),
    };
    if seq != end.count {
        return Err(ChunkFault::Totals);
    }
    Ok(Chunk::End(end))
}

/// Reassembles one authenticated output stream from its connections, in any arrival order.
///
/// It buffers DATA by seq under a byte cap and accepts at most `ceil(max_total / 4096) + 1` connections, both
/// checked before anything is buffered. After the first fault it is poisoned: every later
/// [`Reassembler::accept`] and [`Reassembler::finish`] returns that same fault.
pub struct Reassembler {
    token: ChunkToken,
    max_total: u64,
    max_connections: u32,
    connections: u32,
    data: BTreeMap<u32, Vec<u8>>,
    buffered: u64,
    end: Option<EndTrailer>,
    fault: Option<ChunkFault>,
}

impl Reassembler {
    /// A receiver for one launch's stream of at most `max_total` bytes, authenticated by `token`.
    #[must_use]
    pub fn new(token: ChunkToken, max_total: u64) -> Self {
        let max_connections = max_total
            .div_ceil(MAX_CHUNK_PAYLOAD_U64)
            .checked_add(1)
            .and_then(|n| u32::try_from(n).ok())
            .unwrap_or(u32::MAX);
        Self {
            token,
            max_total,
            max_connections,
            connections: 0,
            data: BTreeMap::new(),
            buffered: 0,
            end: None,
            fault: None,
        }
    }

    /// Accepts the bytes of one connection.
    ///
    /// # Errors
    ///
    /// The [`ChunkFault`] this connection raises, or the earlier fault that poisoned the receiver.
    pub fn accept(&mut self, connection: &[u8]) -> Result<(), ChunkFault> {
        if let Some(fault) = self.fault {
            return Err(fault);
        }
        let result = self.accept_one(connection);
        if let Err(fault) = result {
            self.fault = Some(fault);
        }
        result
    }

    fn accept_one(&mut self, connection: &[u8]) -> Result<(), ChunkFault> {
        self.connections = self.connections.saturating_add(1);
        if self.connections > self.max_connections {
            return Err(ChunkFault::TooManyConnections);
        }
        match parse(&self.token, connection)? {
            Chunk::Data { seq, payload } => {
                if self.data.contains_key(&seq) {
                    return Err(ChunkFault::Duplicate { seq });
                }
                if self.end.is_some_and(|end| seq >= end.count) {
                    return Err(ChunkFault::AfterEnd { seq });
                }
                let buffered = u64::try_from(payload.len())
                    .ok()
                    .and_then(|len| self.buffered.checked_add(len))
                    .filter(|&total| total <= self.max_total)
                    .ok_or(ChunkFault::OverCap)?;
                self.buffered = buffered;
                self.data.insert(seq, payload.to_vec());
                Ok(())
            }
            Chunk::End(end) => {
                if self.end.is_some() {
                    return Err(ChunkFault::SecondEnd);
                }
                if end.total > self.max_total {
                    return Err(ChunkFault::OverCap);
                }
                if let Some((&seq, _)) = self.data.range(end.count..).next() {
                    return Err(ChunkFault::AfterEnd { seq });
                }
                self.end = Some(end);
                Ok(())
            }
        }
    }

    /// Whether the END and every DATA chunk it counts have arrived, with no fault.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.fault.is_none()
            && self
                .end
                .is_some_and(|end| u32::try_from(self.data.len()) == Ok(end.count))
    }

    /// The connections accepted so far, faulty ones included.
    #[must_use]
    pub const fn connections(&self) -> u32 {
        self.connections
    }

    /// The reassembled stream, after checking it against the END.
    ///
    /// # Errors
    ///
    /// [`ChunkFault::NoEnd`] when no END arrived; [`ChunkFault::Gap`] naming the first missing seq;
    /// [`ChunkFault::Length`] when a DATA chunk before the last is not full; [`ChunkFault::Totals`] when the
    /// length disagrees with the END; [`ChunkFault::Digest`] when the SHA-256 does; or the fault that poisoned the
    /// receiver.
    pub fn finish(self) -> Result<Vec<u8>, ChunkFault> {
        if let Some(fault) = self.fault {
            return Err(fault);
        }
        let end = self.end.ok_or(ChunkFault::NoEnd)?;
        let mut expected: u32 = 0;
        for &seq in self.data.keys() {
            if seq != expected {
                return Err(ChunkFault::Gap { seq: expected });
            }
            expected = expected.checked_add(1).ok_or(ChunkFault::Totals)?;
        }
        if expected != end.count {
            return Err(ChunkFault::Gap { seq: expected });
        }
        let pieces = self.data.len();
        let full_before_last = self
            .data
            .values()
            .take(pieces.saturating_sub(1))
            .all(|piece| piece.len() == MAX_CHUNK_PAYLOAD);
        if !full_before_last {
            return Err(ChunkFault::Length);
        }
        if self.buffered != end.total {
            return Err(ChunkFault::Totals);
        }
        let capacity = usize::try_from(end.total).map_err(|_| ChunkFault::Totals)?;
        let mut out = Vec::with_capacity(capacity);
        let mut hasher = Sha256::new();
        for piece in self.data.values() {
            hasher.update(piece);
            out.extend_from_slice(piece);
        }
        let digest: [u8; 32] = hasher.finalize().into();
        if digest != end.digest {
            return Err(ChunkFault::Digest);
        }
        Ok(out)
    }
}
