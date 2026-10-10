//! Job input disk: a fixed header and the job JSON, zero-padded to whole 512-byte sectors.
//!
//! The header is 20 bytes: magic [`JOB_MAGIC`], a `u32` [`PROTOCOL_VERSION`], and a `u64` payload length, both
//! big-endian. Exactly `length` bytes of job JSON follow, then zeros to the next sector boundary. The guest's
//! decoder (`minos-guest` `job::read_job`, 02-01 E-9) reads the same layout; the golden bytes in this module's
//! tests and in the guest's tests bind the two, since this crate never links the guest.

/// Magic bytes at the start of the job header.
pub const JOB_MAGIC: [u8; 8] = *b"MINOSJB1";

/// Version carried in the job header.
pub const PROTOCOL_VERSION: u32 = 1;

/// Most job JSON bytes the guest accepts.
pub const MAX_JOB_BYTES: u64 = 1_048_576;

/// The image is zero-padded to a whole number of sectors of this size.
pub const SECTOR_BYTES: usize = 512;

/// Length of the job header: magic, version, and payload length.
const HEADER_LEN: usize = 20;

/// Why a job input disk could not be written.
#[derive(Debug, thiserror::Error)]
pub enum InputDiskError {
    /// The job JSON is longer than [`MAX_JOB_BYTES`].
    #[error("job JSON is {size} bytes; the job disk holds at most {MAX_JOB_BYTES}")]
    TooLarge {
        /// Length of the rejected JSON.
        size: usize,
    },
}

/// Builds the job input disk image for `job_json`: the header, the JSON, and zero padding to whole sectors.
///
/// # Errors
///
/// [`InputDiskError::TooLarge`] when `job_json` is longer than [`MAX_JOB_BYTES`].
pub fn input_disk_image(job_json: &[u8]) -> Result<Vec<u8>, InputDiskError> {
    let _ = (job_json, HEADER_LEN);
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use super::{InputDiskError, MAX_JOB_BYTES, SECTOR_BYTES, input_disk_image};

    type TestResult = Result<(), Box<dyn Error>>;

    /// 02-05-PLAN.md Exact texts E-2: the first 22 bytes of `input_disk_image(b"{}")`, the same golden header
    /// the guest's `golden_header_for_two_byte_payload` test decodes.
    const E2_HEX: &str = "4d494e4f534a42310000000100000000000000027b7d";

    /// 02-05-PLAN.md Exact texts E-6, identical to 02-01 E-9 and the guest's `CANONICAL` job.
    const E6: &str = r#"{"v":1,"vsock_port":1024,"steps":[{"report_boot":{}},{"scratch_roundtrip":{"bytes":65536}}],"finish":"poweroff"}"#;

    fn hex(text: &str) -> Result<Vec<u8>, Box<dyn Error>> {
        text.as_bytes()
            .chunks(2)
            .map(|pair| Ok(u8::from_str_radix(std::str::from_utf8(pair)?, 16)?))
            .collect()
    }

    fn declared_length(image: &[u8]) -> Result<u64, Box<dyn Error>> {
        let field: [u8; 8] = image.get(12..20).ok_or("image shorter than the header")?.try_into()?;
        Ok(u64::from_be_bytes(field))
    }

    #[test]
    fn input_disk_empty_object_matches_e2_then_zeros() -> TestResult {
        let image = input_disk_image(b"{}")?;
        assert_eq!(image.len(), SECTOR_BYTES);
        assert_eq!(image.get(..22), Some(hex(E2_HEX)?.as_slice()));
        assert!(image.get(22..).ok_or("short image")?.iter().all(|byte| *byte == 0));
        Ok(())
    }

    #[test]
    fn input_disk_canonical_job_declares_its_length_and_fills_whole_sectors() -> TestResult {
        let image = input_disk_image(E6.as_bytes())?;
        assert_eq!(declared_length(&image)?, u64::try_from(E6.len())?);
        assert_eq!(image.get(20..20 + E6.len()), Some(E6.as_bytes()));
        assert_eq!(image.len(), (20 + E6.len()).next_multiple_of(SECTOR_BYTES));
        assert_eq!(image.len() % SECTOR_BYTES, 0);
        Ok(())
    }

    #[test]
    fn input_disk_pads_exactly_at_the_sector_boundary() -> TestResult {
        assert_eq!(input_disk_image(&[b' '; 492])?.len(), 512);
        assert_eq!(input_disk_image(&[b' '; 493])?.len(), 1024);
        Ok(())
    }

    #[test]
    fn input_disk_accepts_the_largest_job() -> TestResult {
        let payload = vec![b' '; usize::try_from(MAX_JOB_BYTES)?];
        let image = input_disk_image(&payload)?;
        assert_eq!(declared_length(&image)?, MAX_JOB_BYTES);
        assert_eq!(image.len() % SECTOR_BYTES, 0);
        Ok(())
    }

    #[test]
    fn input_disk_rejects_a_job_one_byte_over_the_limit() -> TestResult {
        let payload = vec![b' '; usize::try_from(MAX_JOB_BYTES)? + 1];
        assert!(matches!(
            input_disk_image(&payload),
            Err(InputDiskError::TooLarge { size: 1_048_577 })
        ));
        Ok(())
    }
}
