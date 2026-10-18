//! `BlobHasher` names a streamed blob exactly as `BlobId::of` names the whole slice, and `ImageDigest` converts to
//! and from `BlobId` (05-CONTEXT D-09).
//!
//! The golden literal is the published SHA-256 of `abc` (FIPS 180-2, appendix B.1), recomputable with
//! `printf 'abc' | shasum -a 256`. It is a plain `assert_eq!` on a hex literal, never a snapshot.

use minos_core::id::{BlobHasher, BlobId, ImageDigest};
use proptest::prelude::*;

// printf 'abc' | shasum -a 256
const ABC_SHA256: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

/// Feeds `bytes` to a `BlobHasher` in the pieces that `cuts` marks, in order, and finishes.
fn hashed_in_pieces(bytes: &[u8], cuts: &[usize]) -> BlobId {
    let mut points: Vec<usize> = cuts.iter().map(|cut| cut % (bytes.len() + 1)).collect();
    points.sort_unstable();
    let mut hasher = BlobHasher::default();
    let mut rest = bytes;
    let mut taken = 0;
    for point in points {
        let (piece, tail) = rest.split_at(point - taken);
        hasher.update(piece);
        rest = tail;
        taken = point;
    }
    hasher.update(rest);
    hasher.finish()
}

proptest! {
    #[test]
    fn blob_hasher_matches_blob_id_on_any_split(
        bytes in prop::collection::vec(any::<u8>(), 0..=65_536),
        cuts in prop::collection::vec(any::<usize>(), 0..16),
    ) {
        prop_assert_eq!(hashed_in_pieces(&bytes, &cuts), BlobId::of(&bytes));
    }
}

#[test]
fn blob_hasher_empty_input() {
    assert_eq!(BlobHasher::default().finish(), BlobId::of(b""));
}

#[test]
fn blob_hasher_golden_abc() {
    let mut hasher = BlobHasher::default();
    hasher.update(b"a");
    hasher.update(b"");
    hasher.update(b"bc");
    assert_eq!(hasher.finish().to_hex(), ABC_SHA256);
}

#[test]
fn image_digest_round_trips_blob_id() {
    for content in [&b""[..], &b"abc"[..], &b"a root image"[..]] {
        let blob = BlobId::of(content);
        assert_eq!(ImageDigest::from_blob_id(&blob), ImageDigest::of(content));
        assert_eq!(ImageDigest::of(content).to_blob_id(), blob);
    }
}
