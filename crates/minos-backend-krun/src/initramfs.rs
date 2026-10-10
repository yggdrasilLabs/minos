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
    let mut out = Vec::with_capacity(init.len().saturating_add(1024));
    push_record(
        &mut out,
        Record {
            ino: 1,
            mode: INIT_MODE,
            filesize: file_size_field(init.len())?,
            name: b"init\0",
        },
    );
    out.extend_from_slice(init);
    pad_to(&mut out, 4);
    push_record(
        &mut out,
        Record {
            ino: 0,
            mode: 0,
            filesize: 0,
            name: b"TRAILER!!!\0",
        },
    );
    pad_to(&mut out, ARCHIVE_ALIGN);
    Ok(out)
}

/// The newc magic: ASCII cpio with no checksum.
const MAGIC: &[u8; 6] = b"070701";

/// A regular file (0o100000) with permissions 0o755.
const INIT_MODE: u32 = 0o100_755;

/// The archive is zero-padded to whole 512-byte blocks.
const ARCHIVE_ALIGN: usize = 512;

/// The fields that differ between the two records. Every other field is fixed: uid 0, gid 0,
/// nlink 1, mtime 0, device numbers 0, check 0.
#[derive(Clone, Copy)]
struct Record<'a> {
    ino: u32,
    mode: u32,
    filesize: u32,
    /// The name with its terminating NUL.
    name: &'a [u8],
}

/// Writes one 110-byte header and the name, padded so header plus name ends on a 4-byte boundary.
fn push_record(out: &mut Vec<u8>, record: Record<'_>) {
    // A name is one of two short literals, so its length always fits.
    let namesize = u32::try_from(record.name.len()).unwrap_or(u32::MAX);
    out.extend_from_slice(MAGIC);
    for field in [
        record.ino,
        record.mode,
        0, // uid
        0, // gid
        1, // nlink
        0, // mtime
        record.filesize,
        0, // devmajor
        0, // devminor
        0, // rdevmajor
        0, // rdevminor
        namesize,
        0, // check
    ] {
        out.extend_from_slice(format!("{field:08X}").as_bytes());
    }
    out.extend_from_slice(record.name);
    pad_to(out, 4);
}

/// Zero-pads `out` to a multiple of `align`. Every record starts on a 4-byte boundary, so absolute
/// alignment is the same as alignment within the record.
fn pad_to(out: &mut Vec<u8>, align: usize) {
    let rem = out.len() % align;
    if rem != 0 {
        out.resize(out.len() + (align - rem), 0);
    }
}

/// The newc file size field for `len` bytes.
fn file_size_field(len: usize) -> Result<u32, InitramfsError> {
    u32::try_from(len).map_err(|_| InitramfsError::TooLarge { size: len })
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
        haystack
            .windows(needle.len())
            .any(|window| window == needle)
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
            "6b2629df82fc94f9798c3853a508935c6a448a7e48fabd58e4e730a99148ad1a"
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
        assert_eq!(file_size_field(usize::try_from(u32::MAX)?)?, u32::MAX);
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
            return Err(
                "set MINOS_GUEST to the release musl guest binary and MINOS_SPIKE_OUT to \
                        the output file (under the session scratch directory)"
                    .into(),
            );
        };
        let image = initramfs_image(&std::fs::read(guest)?)?;
        std::fs::write(out, &image)?;
        Ok(())
    }
}
