//! The deterministic byte pattern used for the vsock stream and the scratch-disk round trip.
//!
//! Byte `i` of the pattern is `i % 251`. The modulus is prime, so the pattern does not line up with any
//! power-of-two block or chunk size, and a dropped, duplicated, or reordered chunk changes the bytes the
//! receiver checks.

/// The pattern period.
pub const PATTERN_MODULUS: u64 = 251;

/// Byte `index` of the pattern.
#[must_use]
pub fn pattern_byte(index: u64) -> u8 {
    // index % 251 is at most 250, so the conversion never fails; the fallback only avoids a panic path.
    u8::try_from(index % PATTERN_MODULUS).unwrap_or(0)
}

/// Fills `buf` with the pattern bytes starting at pattern position `offset`.
///
/// Positions past `u64::MAX` do not exist, so bytes beyond that point are left unchanged rather than wrapping.
pub fn fill_pattern(offset: u64, buf: &mut [u8]) {
    for (byte, index) in buf.iter_mut().zip(offset..=u64::MAX) {
        *byte = pattern_byte(index);
    }
}

#[cfg(test)]
mod tests {
    use super::{fill_pattern, pattern_byte};

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn pattern_bytes_wrap_at_251() {
        assert_eq!(pattern_byte(0), 0);
        assert_eq!(pattern_byte(1), 1);
        assert_eq!(pattern_byte(250), 250);
        assert_eq!(pattern_byte(251), 0);
        assert_eq!(pattern_byte(252), 1);
        assert_eq!(pattern_byte(u64::MAX), 68);
    }

    #[test]
    fn fill_matches_pattern_byte_at_every_offset() -> TestResult {
        for offset in [0_u64, 1, 250, 251, 65_536, 1_073_741_000] {
            let mut buf = vec![0xAA_u8; 1000];
            fill_pattern(offset, &mut buf);
            for (i, byte) in (0_u64..).zip(&buf) {
                assert_eq!(
                    *byte,
                    pattern_byte(offset.checked_add(i).ok_or("overflow")?)
                );
            }
        }
        Ok(())
    }
}
