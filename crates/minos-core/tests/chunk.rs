//! The authenticated chunk codec that carries guest output over short vsock connections (GUEST-04;
//! 02-SPIKE-EVIDENCE.md S03-ANSWER; 05-RESEARCH Pattern 2).
//!
//! The layout tests fix the wire bytes by hand, the property tests check that every chunk fits one connection and
//! that arrival order never matters, and the fault tests cover each `ChunkFault` variant the receiver can raise.

use minos_core::chunk::{
    CHUNK_HEADER_BYTES, CHUNK_MAGIC, ChunkFault, ChunkStream, ChunkToken, END_PAYLOAD_BYTES,
    KIND_DATA, KIND_END, MAX_CHUNK_BYTES, MAX_CHUNK_PAYLOAD, Reassembler, TOKEN_BYTES,
};
use minos_core::outcome::Reason;
use proptest::prelude::*;
use sha2::{Digest, Sha256};

const TOKEN: [u8; 32] = [0x5a; 32];
const MAX_TOTAL: u64 = 1024 * 1024;

fn token() -> ChunkToken {
    ChunkToken::from_bytes(TOKEN)
}

fn stream(payload: &[u8]) -> Vec<Vec<u8>> {
    ChunkStream::new(&token(), payload).collect()
}

/// One connection built by hand, so a test can set any header field to any value.
fn frame(seq: u32, kind: u8, reserved: u8, len: u16, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&CHUNK_MAGIC);
    out.extend_from_slice(&TOKEN);
    out.extend_from_slice(&seq.to_be_bytes());
    out.push(kind);
    out.push(reserved);
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(payload);
    out
}

fn data(seq: u32, payload: &[u8]) -> Vec<u8> {
    let len = u16::try_from(payload.len()).unwrap_or(u16::MAX);
    frame(seq, KIND_DATA, 0, len, payload)
}

fn end(seq: u32, total: u64, digest: [u8; 32], count: u32) -> Vec<u8> {
    let mut trailer = Vec::new();
    trailer.extend_from_slice(&total.to_be_bytes());
    trailer.extend_from_slice(&digest);
    trailer.extend_from_slice(&count.to_be_bytes());
    let len = u16::try_from(trailer.len()).unwrap_or(u16::MAX);
    frame(seq, KIND_END, 0, len, &trailer)
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn reassembler() -> Reassembler {
    Reassembler::new(token(), MAX_TOTAL)
}

/// Feeds every connection, stopping at the first fault.
fn feed(r: &mut Reassembler, connections: &[Vec<u8>]) -> Result<(), ChunkFault> {
    connections.iter().try_for_each(|c| r.accept(c))
}

fn reassemble(connections: &[Vec<u8>]) -> Result<Vec<u8>, ChunkFault> {
    let mut r = reassembler();
    feed(&mut r, connections)?;
    r.finish()
}

fn every_fault() -> Vec<ChunkFault> {
    vec![
        ChunkFault::Short,
        ChunkFault::Magic,
        ChunkFault::Token,
        ChunkFault::Kind,
        ChunkFault::Reserved,
        ChunkFault::Oversize,
        ChunkFault::Length,
        ChunkFault::Duplicate { seq: 3 },
        ChunkFault::AfterEnd { seq: 3 },
        ChunkFault::SecondEnd,
        ChunkFault::Gap { seq: 3 },
        ChunkFault::Totals,
        ChunkFault::Digest,
        ChunkFault::OverCap,
        ChunkFault::TooManyConnections,
        ChunkFault::NoEnd,
    ]
}

// ---------------------------------------------------------------------------------------------------------------
// Layout
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn constants_fit_one_connection() {
    assert_eq!(CHUNK_MAGIC, *b"MINOSCK1");
    assert_eq!(TOKEN_BYTES, 32);
    assert_eq!(CHUNK_HEADER_BYTES, 48);
    assert_eq!(MAX_CHUNK_PAYLOAD, 4096);
    assert_eq!(MAX_CHUNK_BYTES, CHUNK_HEADER_BYTES + MAX_CHUNK_PAYLOAD);
    assert_eq!(END_PAYLOAD_BYTES, 44);
    const { assert!(MAX_CHUNK_BYTES <= 8192) };
}

#[test]
fn chunk_header_layout() {
    let tok = ChunkToken::from_bytes([0x11; 32]);
    let mut chunks = ChunkStream::new(&tok, b"hi");
    // The encoder numbers DATA chunks from 0, so its first chunk fixes the layout at seq 0, and the hand-built
    // frame below fixes the seq field at 7.
    let first = chunks.next().unwrap_or_default();
    assert_eq!(first.len(), 50);
    assert_eq!(first.get(0..8), Some(&b"MINOSCK1"[..]));
    assert_eq!(first.get(8..40), Some(&[0x11_u8; 32][..]));
    assert_eq!(first.get(40..44), Some(&0_u32.to_be_bytes()[..]));
    assert_eq!(first.get(44), Some(&0));
    assert_eq!(first.get(45), Some(&0));
    assert_eq!(first.get(46..48), Some(&2_u16.to_be_bytes()[..]));
    assert_eq!(first.get(48..), Some(&b"hi"[..]));

    // seq 7, DATA, "hi": the hand-built frame is the same layout, and only the seq bytes differ.
    let seven = data(7, b"hi");
    assert_eq!(seven.len(), 50);
    assert_eq!(seven.get(40..44), Some(&7_u32.to_be_bytes()[..]));
    assert_eq!(seven.get(48..), Some(&b"hi"[..]));
    let mut differs = first
        .iter()
        .zip(&seven)
        .enumerate()
        .filter(|(_, (a, b))| a != b);
    // The token differs (0x11 vs 0x5a) in bytes 8..40 and the seq in byte 43.
    assert!(differs.all(|(i, _)| (8..40).contains(&i) || i == 43));
}

#[test]
fn end_trailer_layout() {
    let payload = vec![0x42_u8; 5000];
    let chunks = stream(&payload);
    assert_eq!(chunks.len(), 3);
    let end_chunk = chunks.last().cloned().unwrap_or_default();
    assert_eq!(end_chunk.len(), 48 + 44);
    assert_eq!(end_chunk.get(40..44), Some(&2_u32.to_be_bytes()[..]));
    assert_eq!(end_chunk.get(44), Some(&1));
    assert_eq!(end_chunk.get(45), Some(&0));
    assert_eq!(end_chunk.get(46..48), Some(&44_u16.to_be_bytes()[..]));
    assert_eq!(end_chunk.get(48..56), Some(&5000_u64.to_be_bytes()[..]));
    assert_eq!(end_chunk.get(56..88), Some(&sha256(&payload)[..]));
    assert_eq!(end_chunk.get(88..92), Some(&2_u32.to_be_bytes()[..]));
}

#[test]
fn chunk_count_is_data_plus_end() {
    assert_eq!(ChunkStream::chunk_count(0), Some(1));
    assert_eq!(ChunkStream::chunk_count(1), Some(2));
    assert_eq!(ChunkStream::chunk_count(4096), Some(2));
    assert_eq!(ChunkStream::chunk_count(4097), Some(3));
    assert_eq!(ChunkStream::chunk_count(u64::MAX), None);
    assert_eq!(
        u32::try_from(stream(&[7_u8; 9000]).len()).ok(),
        ChunkStream::chunk_count(9000)
    );
}

#[test]
fn empty_payload_is_one_end() -> Result<(), ChunkFault> {
    let chunks = stream(&[]);
    assert_eq!(chunks.len(), 1);
    let only = chunks.first().cloned().unwrap_or_default();
    assert_eq!(only.get(44), Some(&KIND_END));
    assert_eq!(only.get(40..44), Some(&0_u32.to_be_bytes()[..]));
    assert_eq!(reassemble(&chunks)?, Vec::<u8>::new());
    Ok(())
}

proptest! {
    #[test]
    fn every_chunk_fits_one_connection(payload in proptest::collection::vec(any::<u8>(), 0..65_536)) {
        let chunks = stream(&payload);
        prop_assert_eq!(u32::try_from(chunks.len()).ok(), ChunkStream::chunk_count(u64::try_from(payload.len()).unwrap_or(u64::MAX)));
        for chunk in &chunks {
            prop_assert!(chunk.len() <= MAX_CHUNK_BYTES);
            prop_assert!(chunk.len() >= CHUNK_HEADER_BYTES);
            prop_assert!(chunk.len() - CHUNK_HEADER_BYTES <= MAX_CHUNK_PAYLOAD);
        }
    }

    #[test]
    fn reassembly_survives_any_order(
        (payload, shuffled) in proptest::collection::vec(any::<u8>(), 0..65_536).prop_flat_map(|payload| {
            let chunks = stream(&payload);
            (Just(payload), Just(chunks).prop_shuffle())
        })
    ) {
        let mut r = reassembler();
        for chunk in &shuffled {
            prop_assert_eq!(r.accept(chunk), Ok(()));
        }
        prop_assert!(r.is_complete());
        prop_assert_eq!(u32::try_from(shuffled.len()).ok(), Some(r.connections()));
        prop_assert_eq!(r.finish(), Ok(payload));
    }
}

#[test]
fn in_order_stream_reassembles_and_completes() -> Result<(), ChunkFault> {
    let payload: Vec<u8> = (0..10_000_u32)
        .map(|i| u8::try_from(i % 251).unwrap_or(0))
        .collect();
    let chunks = stream(&payload);
    let mut r = reassembler();
    let (last, rest) = chunks.split_last().ok_or(ChunkFault::NoEnd)?;
    feed(&mut r, rest)?;
    // Every DATA chunk is in but no END yet.
    assert!(!r.is_complete());
    r.accept(last)?;
    assert!(r.is_complete());
    assert_eq!(r.finish()?, payload);
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// Faults
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn wrong_token_is_refused() {
    let other = ChunkToken::from_bytes([0x5b; 32]);
    let chunks: Vec<Vec<u8>> = ChunkStream::new(&other, b"forged").collect();
    assert_eq!(reassemble(&chunks), Err(ChunkFault::Token));
    // One differing byte anywhere in the token is enough.
    let mut one_off = TOKEN;
    if let Some(byte) = one_off.last_mut() {
        *byte ^= 1;
    }
    let chunks: Vec<Vec<u8>> = ChunkStream::new(&ChunkToken::from_bytes(one_off), b"x").collect();
    assert_eq!(reassemble(&chunks), Err(ChunkFault::Token));
}

#[test]
fn flipped_magic_is_refused() {
    let mut chunk = data(0, b"abc");
    if let Some(byte) = chunk.first_mut() {
        *byte ^= 0x20;
    }
    assert_eq!(reassembler().accept(&chunk), Err(ChunkFault::Magic));
}

#[test]
fn short_header_is_refused() {
    assert_eq!(reassembler().accept(&[]), Err(ChunkFault::Short));
    let chunk = data(0, b"");
    let short = chunk.get(..CHUNK_HEADER_BYTES - 1).unwrap_or_default();
    assert_eq!(reassembler().accept(short), Err(ChunkFault::Short));
}

#[test]
fn oversize_chunk_is_refused() {
    let payload = vec![0_u8; 4097];
    assert_eq!(
        reassembler().accept(&data(0, &payload)),
        Err(ChunkFault::Oversize)
    );
    // An END claiming a length over the cap is oversize too.
    assert_eq!(
        reassembler().accept(&frame(0, KIND_END, 0, 4097, &payload)),
        Err(ChunkFault::Oversize)
    );
}

#[test]
fn length_mismatch_is_refused() {
    // len says 2, three bytes follow.
    assert_eq!(
        reassembler().accept(&frame(0, KIND_DATA, 0, 2, b"abc")),
        Err(ChunkFault::Length)
    );
    // len says 4, two bytes follow.
    assert_eq!(
        reassembler().accept(&frame(0, KIND_DATA, 0, 4, b"ab")),
        Err(ChunkFault::Length)
    );
    // An END whose payload is not exactly 44 bytes.
    assert_eq!(
        reassembler().accept(&frame(0, KIND_END, 0, 3, b"abc")),
        Err(ChunkFault::Length)
    );
    // An empty DATA chunk: a valid stream never sends one.
    assert_eq!(reassembler().accept(&data(0, b"")), Err(ChunkFault::Length));
}

#[test]
fn short_data_before_the_last_is_refused() {
    // Two DATA chunks of 10 bytes each: a valid stream fills every DATA chunk but the last.
    let payload = [1_u8; 20];
    let chunks = vec![
        data(0, &[1_u8; 10]),
        data(1, &[1_u8; 10]),
        end(2, 20, sha256(&payload), 2),
    ];
    assert_eq!(reassemble(&chunks), Err(ChunkFault::Length));
}

#[test]
fn unknown_kind_is_refused() {
    assert_eq!(
        reassembler().accept(&frame(0, 2, 0, 1, b"a")),
        Err(ChunkFault::Kind)
    );
}

#[test]
fn nonzero_reserved_is_refused() {
    assert_eq!(
        reassembler().accept(&frame(0, KIND_DATA, 1, 1, b"a")),
        Err(ChunkFault::Reserved)
    );
}

#[test]
fn duplicate_seq_is_refused() {
    let chunk = data(0, b"abc");
    assert_eq!(
        reassemble(&[chunk.clone(), chunk]),
        Err(ChunkFault::Duplicate { seq: 0 })
    );
}

#[test]
fn second_end_is_refused() {
    let chunks = stream(b"abc");
    let end_chunk = chunks.last().cloned().unwrap_or_default();
    let mut doubled = chunks;
    doubled.push(end_chunk);
    assert_eq!(reassemble(&doubled), Err(ChunkFault::SecondEnd));
}

#[test]
fn data_after_the_end_is_refused() {
    // The END says one DATA chunk; a DATA with seq 1 arrives after it.
    let chunks = vec![data(0, b"abc"), end(1, 3, sha256(b"abc"), 1), data(1, b"x")];
    assert_eq!(reassemble(&chunks), Err(ChunkFault::AfterEnd { seq: 1 }));
    // The same DATA arriving before the END is refused when the END arrives.
    let chunks = vec![data(0, b"abc"), data(5, b"x"), end(1, 3, sha256(b"abc"), 1)];
    assert_eq!(reassemble(&chunks), Err(ChunkFault::AfterEnd { seq: 5 }));
}

#[test]
fn wrong_totals_are_refused() {
    // The total length is wrong.
    let chunks = vec![data(0, b"abc"), end(1, 4, sha256(b"abc"), 1)];
    assert_eq!(reassemble(&chunks), Err(ChunkFault::Totals));
    // The END's seq disagrees with its DATA count.
    let chunks = vec![data(0, b"abc"), end(2, 3, sha256(b"abc"), 1)];
    assert_eq!(reassemble(&chunks), Err(ChunkFault::Totals));
}

#[test]
fn wrong_digest_is_refused() {
    let chunks = vec![data(0, b"abc"), end(1, 3, sha256(b"abd"), 1)];
    assert_eq!(reassemble(&chunks), Err(ChunkFault::Digest));
}

#[test]
fn data_over_the_cap_is_refused() {
    let mut r = Reassembler::new(token(), 10);
    assert_eq!(r.accept(&data(0, &[0_u8; 11])), Err(ChunkFault::OverCap));
    // Within the cap, then past it on the second chunk.
    let mut r = Reassembler::new(token(), 4100);
    assert_eq!(r.accept(&data(0, &[0_u8; 4096])), Ok(()));
    assert_eq!(r.accept(&data(1, &[0_u8; 5])), Err(ChunkFault::OverCap));
    // An END claiming a total over the cap.
    let mut r = Reassembler::new(token(), 10);
    assert_eq!(r.accept(&end(0, 11, [0; 32], 0)), Err(ChunkFault::OverCap));
}

#[test]
fn one_connection_past_the_limit_is_refused() {
    // max_total 0 allows ceil(0 / 4096) + 1 = 1 connection: the END of an empty stream.
    let mut r = Reassembler::new(token(), 0);
    let chunks = stream(&[]);
    assert_eq!(feed(&mut r, &chunks), Ok(()));
    assert_eq!(r.connections(), 1);
    assert_eq!(r.accept(&[]), Err(ChunkFault::TooManyConnections));
    // max_total 4096 allows two connections; the third is refused whatever it holds.
    let mut r = Reassembler::new(token(), 4096);
    assert_eq!(r.accept(&data(0, &[0_u8; 4096])), Ok(()));
    assert_eq!(r.accept(&[]), Err(ChunkFault::Short));
    let mut r = Reassembler::new(token(), 4096);
    assert_eq!(r.accept(&data(0, &[0_u8; 4096])), Ok(()));
    assert_eq!(r.accept(&data(5, &[0_u8; 1])), Err(ChunkFault::OverCap));
    let mut r = Reassembler::new(token(), 4096);
    let chunks = stream(&[0_u8; 4096]);
    assert_eq!(feed(&mut r, &chunks), Ok(()));
    assert_eq!(
        r.accept(&data(9, b"x")),
        Err(ChunkFault::TooManyConnections)
    );
}

#[test]
fn missing_seq_is_a_gap() {
    let chunks = stream(&[3_u8; 10_000]);
    let without_first: Vec<Vec<u8>> = chunks.iter().skip(1).cloned().collect();
    assert_eq!(reassemble(&without_first), Err(ChunkFault::Gap { seq: 0 }));
    let without_middle: Vec<Vec<u8>> = chunks
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != 1)
        .map(|(_, c)| c.clone())
        .collect();
    assert_eq!(reassemble(&without_middle), Err(ChunkFault::Gap { seq: 1 }));
}

#[test]
fn missing_end_is_no_end() {
    let chunks = stream(&[3_u8; 10_000]);
    let without_end = chunks
        .split_last()
        .map(|(_, rest)| rest)
        .unwrap_or_default();
    assert_eq!(reassemble(without_end), Err(ChunkFault::NoEnd));
    assert_eq!(reassembler().finish(), Err(ChunkFault::NoEnd));
}

#[test]
fn reassembler_is_poisoned_after_a_fault() {
    let mut r = reassembler();
    assert_eq!(r.accept(&frame(0, 9, 0, 1, b"a")), Err(ChunkFault::Kind));
    // A valid chunk after the fault returns the same fault, and so does finish.
    assert_eq!(r.accept(&data(0, b"abc")), Err(ChunkFault::Kind));
    assert_eq!(r.finish(), Err(ChunkFault::Kind));
}

#[test]
fn fault_classes() {
    for fault in every_fault() {
        assert_eq!(fault.is_incomplete(), fault == ChunkFault::NoEnd, "{fault}");
        assert_eq!(fault.reason(), Reason::ChunkProtocol, "{fault}");
        assert!(!fault.to_string().is_empty());
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Token
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn token_debug_is_redacted() {
    let tok = ChunkToken::from_bytes([0xab; 32]);
    let shown = format!("{tok:?}");
    assert_eq!(shown, "ChunkToken(..)");
    assert!(!shown.contains("ab"));
    assert!(!shown.contains("171"));
}

#[test]
fn token_ct_eq() {
    let bytes: [u8; 32] = core::array::from_fn(|i| u8::try_from(i).unwrap_or(0));
    let tok = ChunkToken::from_bytes(bytes);
    assert!(tok.ct_eq(&bytes));
    for i in 0..32 {
        let mut other = bytes;
        if let Some(byte) = other.get_mut(i) {
            *byte ^= 0x80;
        }
        assert!(!tok.ct_eq(&other), "byte {i}");
    }
    assert!(!tok.ct_eq(bytes.get(..31).unwrap_or_default()));
    let mut longer = bytes.to_vec();
    longer.push(0);
    assert!(!tok.ct_eq(&longer));
    assert!(!tok.ct_eq(&[]));
}
