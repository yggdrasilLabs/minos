//! newc initramfs writer holding only /init (02-CONTEXT.md D-17).
//!
//! The archive has exactly two records, `init` and the `TRAILER!!!` terminator, and every metadata
//! field is fixed: no clock, no host uid or gid, no inode numbers from a filesystem. The same bytes
//! in always give the same archive out.
//!
//! Stock libkrunfw 5.6.2 is built without `CONFIG_BLK_DEV_INITRD`, so it cannot unpack this archive
//! (D-18). Its one consumer is the negative-evidence boot in 02-06; the VM boots from the `SquashFS`
//! root in [`crate::rootfs`] instead.

/// Why an initramfs could not be written.
#[derive(Debug, thiserror::Error)]
pub enum InitramfsError {
    /// newc stores a file size in eight hex digits, so 0xFFFFFFFF bytes is the most it can hold.
    #[error("init is {size} bytes; a newc record holds at most 4294967295")]
    TooLarge {
        /// The size of the rejected file.
        size: usize,
    },
}

/// Writes a newc archive whose only file is `init`, mode 0100755, owned by uid 0 and gid 0.
///
/// # Errors
///
/// [`InitramfsError::TooLarge`] when `init` is longer than 0xFFFFFFFF bytes.
pub fn initramfs_image(init: &[u8]) -> Result<Vec<u8>, InitramfsError> {
    let _unused = init;
    Ok(Vec::new())
}

/// The newc file size field for `len` bytes.
fn file_size_field(len: usize) -> Result<u32, InitramfsError> {
    let _unused = len;
    Ok(0)
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use sha2::{Digest, Sha256};

    use super::{InitramfsError, file_size_field, initramfs_image};

    type TestResult = Result<(), Box<dyn Error>>;

    fn sha256_hex(bytes: &[u8]) -> String {
        let digest: [u8; 32] = Sha256::digest(bytes).into();
        base16ct::lower::encode_string(&digest)
    }

    fn contains(haystack: &[u8], needle: &[u8]) -> bool {
        haystack.windows(needle.len()).any(|window| window == needle)
    }

    #[test]
    fn initramfs_layout_for_hello() -> TestResult {
        let image = initramfs_image(b"hello")?;
        assert!(image.starts_with(b"070701"));
        // mode is the second field: 6 magic bytes, then 8 hex digits of ino
        assert_eq!(image.get(14..22), Some(&b"000081ED"[..]));
        // filesize is the seventh field
        assert_eq!(image.get(54..62), Some(&b"00000005"[..]));
        // namesize is the twelfth field
        assert_eq!(image.get(94..102), Some(&b"00000005"[..]));
        // the name and its NUL follow the 110-byte header; header plus name pads to 116
        assert_eq!(image.get(110..116), Some(&b"init\0\0"[..]));
        // the data starts at 116 and pads to a multiple of 4
        assert_eq!(image.get(116..124), Some(&b"hello\0\0\0"[..]));
        assert_eq!(image.get(124..130), Some(&b"070701"[..]));
        assert!(contains(&image, b"TRAILER!!!\0"));
        assert_eq!(image.len() % 512, 0);
        assert_eq!(image.len(), 512);
        Ok(())
    }

    #[test]
    fn initramfs_golden() -> TestResult {
        let image = initramfs_image(b"hello")?;
        assert_eq!(
            sha256_hex(&image),
            "0000000000000000000000000000000000000000000000000000000000000000"
        );
        Ok(())
    }

    #[test]
    fn initramfs_is_deterministic() -> TestResult {
        let init: Vec<u8> = (0..=255).cycle().take(10_000).collect();
        let first = initramfs_image(&init)?;
        assert!(contains(&first, &init));
        assert_eq!(first, initramfs_image(&init)?);
        Ok(())
    }

    #[test]
    fn initramfs_size_field_rejects_more_than_32_bits() -> TestResult {
        let too_big = usize::try_from(u64::from(u32::MAX) + 1)?;
        assert!(matches!(
            file_size_field(too_big),
            Err(InitramfsError::TooLarge { size }) if size == too_big
        ));
        assert_eq!(
            file_size_field(usize::try_from(u32::MAX)?)?,
            u32::MAX
        );
        Ok(())
    }

    /// Writes the initramfs for the release musl guest, for the macOS `cpio` cross-check (IR-01).
    #[test]
    #[ignore = "writes an initramfs for the cpio cross-check"]
    fn initramfs_write_for_cpio_cross_check() -> TestResult {
        let (Some(guest), Some(out)) = (
            std::env::var_os("MINOS_GUEST"),
            std::env::var_os("MINOS_SPIKE_OUT"),
        ) else {
            return Err("set MINOS_GUEST to the release musl guest binary and MINOS_SPIKE_OUT to \
                        the output file (under the session scratch directory)"
                .into());
        };
        let image = initramfs_image(&std::fs::read(guest)?)?;
        std::fs::write(out, &image)?;
        Ok(())
    }
}
