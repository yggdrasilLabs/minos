//! Root image writer: an untrusted rootfs tar plus the trusted guest as /init, packed into a raw read-only `SquashFS` image.
//!
//! The tar comes from a user's Dockerfile, so every name, type, size, and link in it is hostile
//! (02-CONTEXT.md D-01, D-17). It is parsed in memory and never unpacked onto the host. Caps apply
//! before anything is read: total tar bytes, entry count, per-entry size (taken from the header),
//! and path length. Only regular files, directories, symlinks, and hardlinks are accepted; a
//! hardlink becomes a copy of an earlier regular file. Every mtime in the image is 0, nodes are
//! ordered by path, and the image is gzip-compressed, so the same tar always gives the same bytes.
//!
//! The image gets the mount points the guest needs on a read-only root (`/dev`, `/proc`, `/sys`,
//! `/tmp`, `/mnt`), and `/init` is written last from the trusted guest bytes, replacing anything the
//! tar put there.

use std::io::Read;

use serde::Serialize;

/// Caps applied to a rootfs tar before and while it is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackLimits {
    max_tar_bytes: u64,
    max_entries: u32,
    max_entry_bytes: u64,
    max_path_bytes: usize,
}

impl PackLimits {
    /// Caps on total tar bytes, entry count, bytes in one entry, and bytes in one path.
    ///
    /// `max_tar_bytes` also caps the file content in the image, counting each hardlink copy.
    #[must_use]
    pub const fn new(
        max_tar_bytes: u64,
        max_entries: u32,
        max_entry_bytes: u64,
        max_path_bytes: usize,
    ) -> Self {
        Self {
            max_tar_bytes,
            max_entries,
            max_entry_bytes,
            max_path_bytes,
        }
    }
}

impl Default for PackLimits {
    /// 4 GiB of tar, 200,000 entries, 1 GiB per entry, 4,096 bytes per path.
    fn default() -> Self {
        Self::new(4_294_967_296, 200_000, 1_073_741_824, 4096)
    }
}

/// What the packer did, for provenance and for the spike evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PackReport {
    image_sha256: String,
    init_sha256: String,
    files: u64,
    dirs: u64,
    symlinks: u64,
    hardlinks_copied: u64,
    setid_files: u64,
    skipped_global_headers: u64,
    replaced_duplicates: u64,
    init_overridden: bool,
}

impl PackReport {
    /// SHA-256 of the image bytes, lowercase hex.
    #[must_use]
    pub fn image_sha256(&self) -> &str {
        &self.image_sha256
    }

    /// SHA-256 of the trusted `/init` bytes, lowercase hex.
    #[must_use]
    pub fn init_sha256(&self) -> &str {
        &self.init_sha256
    }

    /// Regular files in the image, `/init` included.
    #[must_use]
    pub const fn files(&self) -> u64 {
        self.files
    }

    /// Directories in the image, the root excluded.
    #[must_use]
    pub const fn dirs(&self) -> u64 {
        self.dirs
    }

    /// Symlinks in the image.
    #[must_use]
    pub const fn symlinks(&self) -> u64 {
        self.symlinks
    }

    /// Hardlink entries written as copies of their target.
    #[must_use]
    pub const fn hardlinks_copied(&self) -> u64 {
        self.hardlinks_copied
    }

    /// Regular files with a setuid or setgid bit. The bits are kept.
    #[must_use]
    pub const fn setid_files(&self) -> u64 {
        self.setid_files
    }

    /// Pax global headers in the tar, which carry no file and are skipped.
    #[must_use]
    pub const fn skipped_global_headers(&self) -> u64 {
        self.skipped_global_headers
    }

    /// Tar entries that replaced an earlier entry at the same path.
    #[must_use]
    pub const fn replaced_duplicates(&self) -> u64 {
        self.replaced_duplicates
    }

    /// Whether the tar supplied its own `init`, which the trusted bytes replaced.
    #[must_use]
    pub const fn init_overridden(&self) -> bool {
        self.init_overridden
    }
}

/// A packed `SquashFS` root image and its report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackedImage {
    bytes: Vec<u8>,
    report: PackReport,
}

impl PackedImage {
    /// The image bytes.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The image bytes, consuming the image.
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    /// What the packer did.
    #[must_use]
    pub const fn report(&self) -> &PackReport {
        &self.report
    }
}

/// Why a rootfs tar could not be packed.
///
/// Paths in these errors come from the tar and are hostile: they are shown escaped (`{:?}`) and cut
/// to 256 characters.
#[derive(Debug, thiserror::Error)]
pub enum PackError {
    /// The tar is longer than `max_tar_bytes`.
    #[error("rootfs tar is larger than {limit} bytes")]
    TarTooLarge {
        /// The cap.
        limit: u64,
    },
    /// The tar has more than `max_entries` entries.
    #[error("rootfs tar has more than {limit} entries")]
    TooManyEntries {
        /// The cap.
        limit: u32,
    },
    /// One entry declares more than `max_entry_bytes` bytes.
    #[error("entry {path:?} declares {size} bytes, over the {limit}-byte entry cap")]
    EntryTooLarge {
        /// The entry path.
        path: String,
        /// The size its header declares.
        size: u64,
        /// The cap.
        limit: u64,
    },
    /// File content in the image, counting hardlink copies, exceeds `max_tar_bytes`.
    #[error("file content in the image exceeds {limit} bytes (each hardlink copy counts)")]
    ContentTooLarge {
        /// The cap.
        limit: u64,
    },
    /// An entry path or link target is not acceptable.
    #[error("entry path {path:?} {reason}")]
    BadPath {
        /// The entry path.
        path: String,
        /// What is wrong with it.
        reason: &'static str,
    },
    /// An entry type other than file, directory, symlink, or hardlink.
    #[error("entry {path:?} has unsupported type {kind}")]
    UnsupportedEntry {
        /// The entry path.
        path: String,
        /// The tar entry type.
        kind: String,
    },
    /// A hardlink whose target is not an earlier regular file in the tar.
    #[error("hardlink {path:?} names {target:?}, which is not an earlier regular file")]
    HardlinkTargetMissing {
        /// The hardlink path.
        path: String,
        /// The target it names.
        target: String,
    },
    /// A uid or gid that does not fit in 32 bits.
    #[error("entry {path:?} has {which} {id}, outside the 32-bit range")]
    OwnershipOutOfRange {
        /// The entry path.
        path: String,
        /// `uid` or `gid`.
        which: &'static str,
        /// The value in the header.
        id: u64,
    },
    /// The tar put something other than a directory at a mount point.
    #[error("/{path} must be a directory, because the guest mounts a filesystem there")]
    MountPoint {
        /// The mount point, without its leading slash.
        path: &'static str,
    },
    /// Reading the tar or writing the image failed.
    #[error("rootfs tar or image I/O")]
    Io(#[from] std::io::Error),
    /// The `SquashFS` writer refused the tree.
    #[error("SquashFS writer: {what}")]
    Squashfs {
        /// The writer's error variant, in words.
        what: &'static str,
    },
}

/// Packs `rootfs_tar` into a gzip `SquashFS` image with `init` as `/init`.
///
/// # Errors
///
/// Any [`PackError`]: a cap is exceeded, a path or entry type is refused, a hardlink has no earlier
/// target, a mount point is not a directory, or reading the tar or writing the image fails.
pub fn pack_root_image(
    rootfs_tar: impl Read,
    init: &[u8],
    limits: &PackLimits,
) -> Result<PackedImage, PackError> {
    let _unused = (rootfs_tar, init, limits);
    Err(PackError::Squashfs {
        what: "not implemented",
    })
}

/// Packs an empty rootfs: the mount points and `/init` only, with default limits.
///
/// # Errors
///
/// A [`PackError`] from the `SquashFS` writer.
pub fn minimal_root_image(init: &[u8]) -> Result<PackedImage, PackError> {
    pack_root_image(std::io::empty(), init, &PackLimits::default())
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::io::{Cursor, Read};
    use std::path::Path;

    use backhand::{FilesystemReader, InnerNode, Node, NodeHeader, SquashfsFileReader};
    use sha2::{Digest, Sha256};
    use tar::{EntryType, Header};

    use super::{PackError, PackLimits, PackedImage, minimal_root_image, pack_root_image};

    type TestResult = Result<(), Box<dyn Error>>;

    const INIT: &[u8] = b"trusted-init-bytes";

    /// One tar entry for a synthetic archive. Names and link targets go into the header bytes as
    /// given, so hostile names the tar builder would refuse can be written.
    #[derive(Clone, Copy)]
    struct Spec<'a> {
        name: &'a [u8],
        kind: EntryType,
        mode: u32,
        data: &'a [u8],
        link: &'a [u8],
        declared_size: Option<u64>,
    }

    const fn file<'a>(name: &'a [u8], data: &'a [u8]) -> Spec<'a> {
        Spec {
            name,
            kind: EntryType::Regular,
            mode: 0o644,
            data,
            link: b"",
            declared_size: None,
        }
    }

    const fn dir(name: &[u8]) -> Spec<'_> {
        Spec {
            name,
            kind: EntryType::Directory,
            mode: 0o755,
            data: b"",
            link: b"",
            declared_size: None,
        }
    }

    const fn link<'a>(name: &'a [u8], kind: EntryType, target: &'a [u8]) -> Spec<'a> {
        Spec {
            name,
            kind,
            mode: 0o777,
            data: b"",
            link: target,
            declared_size: None,
        }
    }

    const fn special(name: &[u8], kind: EntryType) -> Spec<'_> {
        Spec {
            name,
            kind,
            mode: 0o644,
            data: b"",
            link: b"",
            declared_size: None,
        }
    }

    fn tar_of(specs: &[Spec<'_>], mtime: u64) -> Result<Vec<u8>, Box<dyn Error>> {
        let mut builder = tar::Builder::new(Vec::new());
        for spec in specs {
            let mut header = Header::new_ustar();
            for (dst, src) in header.as_old_mut().name.iter_mut().zip(spec.name) {
                *dst = *src;
            }
            header.set_link_name_literal(spec.link)?;
            header.set_entry_type(spec.kind);
            header.set_mode(spec.mode);
            header.set_uid(0);
            header.set_gid(0);
            header.set_mtime(mtime);
            let size = match spec.declared_size {
                Some(size) => size,
                None => u64::try_from(spec.data.len())?,
            };
            header.set_size(size);
            header.set_cksum();
            builder.append(&header, spec.data)?;
        }
        Ok(builder.into_inner()?)
    }

    /// A tar whose only entry gets its name from a GNU long-name record, so the name can hold a NUL.
    fn gnu_long_name_tar(name: &[u8]) -> Result<Vec<u8>, Box<dyn Error>> {
        let mut builder = tar::Builder::new(Vec::new());
        let mut long = Header::new_gnu();
        for (dst, src) in long.as_old_mut().name.iter_mut().zip(b"././@LongLink") {
            *dst = *src;
        }
        long.set_entry_type(EntryType::GNULongName);
        long.set_mode(0o644);
        long.set_size(u64::try_from(name.len())?);
        long.set_cksum();
        builder.append(&long, name)?;
        let mut header = Header::new_gnu();
        for (dst, src) in header.as_old_mut().name.iter_mut().zip(b"placeholder") {
            *dst = *src;
        }
        header.set_entry_type(EntryType::Regular);
        header.set_mode(0o644);
        header.set_size(1);
        header.set_cksum();
        builder.append(&header, &b"x"[..])?;
        Ok(builder.into_inner()?)
    }

    fn sample() -> [Spec<'static>; 6] {
        [
            dir(b"./"),
            dir(b"./etc/"),
            file(b"./etc/hostname", b"minos\n"),
            dir(b"./usr/"),
            file(b"./usr/bin/tool", b"#!/bin/sh\necho tool\n"),
            link(b"./bin", EntryType::Symlink, b"usr/bin"),
        ]
    }

    fn pack(tar: &[u8]) -> Result<PackedImage, PackError> {
        pack_root_image(tar, INIT, &PackLimits::default())
    }

    fn read_back(image: &PackedImage) -> Result<FilesystemReader<'static>, Box<dyn Error>> {
        Ok(FilesystemReader::from_reader(Cursor::new(
            image.bytes().to_vec(),
        ))?)
    }

    fn node<'r>(
        fs: &'r FilesystemReader<'_>,
        path: &str,
    ) -> Result<&'r Node<SquashfsFileReader>, Box<dyn Error>> {
        fs.files()
            .find(|node| node.fullpath == Path::new(path))
            .ok_or_else(|| format!("{path} is not in the image").into())
    }

    fn file_bytes(fs: &FilesystemReader<'_>, path: &str) -> Result<Vec<u8>, Box<dyn Error>> {
        match &node(fs, path)?.inner {
            InnerNode::File(file) => {
                let mut out = Vec::new();
                fs.file(file).reader().read_to_end(&mut out)?;
                Ok(out)
            }
            _ => Err(format!("{path} is not a regular file").into()),
        }
    }

    fn sha256_hex(bytes: &[u8]) -> String {
        let digest: [u8; 32] = Sha256::digest(bytes).into();
        base16ct::lower::encode_string(&digest)
    }

    #[test]
    fn rootfs_same_tar_packs_to_identical_bytes() -> TestResult {
        let tar = tar_of(&sample(), 0)?;
        let first = pack(&tar)?;
        let second = pack(&tar)?;
        assert_eq!(first.bytes(), second.bytes());
        assert_eq!(first.report(), second.report());
        Ok(())
    }

    #[test]
    fn rootfs_tar_mtimes_do_not_change_the_image() -> TestResult {
        let epoch = pack(&tar_of(&sample(), 0)?)?;
        let later = pack(&tar_of(&sample(), 1_760_000_000)?)?;
        assert_eq!(epoch.bytes(), later.bytes());
        Ok(())
    }

    #[test]
    fn rootfs_image_has_trusted_init_and_mount_points() -> TestResult {
        let image = pack(&tar_of(&sample(), 1_760_000_000)?)?;
        let fs = read_back(&image)?;
        assert_eq!(file_bytes(&fs, "/init")?, INIT);
        assert_eq!(node(&fs, "/init")?.header, NodeHeader::new(0o755, 0, 0, 0));
        for mount_point in ["/dev", "/proc", "/sys", "/mnt"] {
            let found = node(&fs, mount_point)?;
            assert!(matches!(found.inner, InnerNode::Dir(_)), "{mount_point}");
            assert_eq!(found.header.permissions, 0o755, "{mount_point}");
        }
        let tmp = node(&fs, "/tmp")?;
        assert!(matches!(tmp.inner, InnerNode::Dir(_)));
        assert_eq!(tmp.header.permissions, 0o1777);
        assert!(fs.files().all(|node| node.header.mtime == 0));
        assert_eq!(fs.mod_time, 0);
        assert_eq!(file_bytes(&fs, "/etc/hostname")?, b"minos\n");
        let report = image.report();
        assert_eq!(report.init_sha256(), sha256_hex(INIT));
        assert_eq!(report.image_sha256(), sha256_hex(image.bytes()));
        assert!(!report.init_overridden());
        assert_eq!(report.files(), 3);
        assert_eq!(report.symlinks(), 1);
        // etc, usr, usr/bin (implicit), and the five mount points
        assert_eq!(report.dirs(), 8);
        Ok(())
    }

    #[test]
    fn rootfs_tar_supplied_init_is_replaced() -> TestResult {
        let tar = tar_of(&[file(b"init", b"#!/bin/sh\nevil\n")], 0)?;
        let image = pack(&tar)?;
        let fs = read_back(&image)?;
        assert_eq!(file_bytes(&fs, "/init")?, INIT);
        assert!(image.report().init_overridden());
        Ok(())
    }

    #[test]
    fn rootfs_tar_supplied_init_directory_is_replaced() -> TestResult {
        let tar = tar_of(&[dir(b"init/"), file(b"init/payload", b"evil")], 0)?;
        let image = pack(&tar)?;
        let fs = read_back(&image)?;
        assert_eq!(file_bytes(&fs, "/init")?, INIT);
        assert!(node(&fs, "/init/payload").is_err());
        assert!(image.report().init_overridden());
        Ok(())
    }

    #[test]
    fn rootfs_hardlink_becomes_a_copy() -> TestResult {
        let tar = tar_of(
            &[
                file(b"lib/libc.so", b"libc-bytes"),
                link(b"lib/libc.so.1", EntryType::Link, b"lib/libc.so"),
            ],
            0,
        )?;
        let image = pack(&tar)?;
        let fs = read_back(&image)?;
        assert_eq!(file_bytes(&fs, "/lib/libc.so.1")?, b"libc-bytes");
        assert_eq!(image.report().hardlinks_copied(), 1);
        Ok(())
    }

    #[test]
    fn rootfs_symlink_keeps_its_target() -> TestResult {
        let image = pack(&tar_of(&sample(), 0)?)?;
        let fs = read_back(&image)?;
        match &node(&fs, "/bin")?.inner {
            InnerNode::Symlink(symlink) => assert_eq!(symlink.link, Path::new("usr/bin")),
            _ => return Err("/bin is not a symlink".into()),
        }
        Ok(())
    }

    #[test]
    fn rootfs_setid_bits_are_counted_and_kept() -> TestResult {
        let mut su = file(b"bin/su", b"su");
        su.mode = 0o4755;
        let image = pack(&tar_of(&[su], 0)?)?;
        let fs = read_back(&image)?;
        assert_eq!(node(&fs, "/bin/su")?.header.permissions, 0o4755);
        assert_eq!(image.report().setid_files(), 1);
        Ok(())
    }

    #[test]
    fn rootfs_parent_dir_component_is_rejected() -> TestResult {
        let tar = tar_of(&[file(b"etc/../../escape", b"x")], 0)?;
        assert!(matches!(pack(&tar), Err(PackError::BadPath { .. })));
        Ok(())
    }

    #[test]
    fn rootfs_absolute_path_is_rejected() -> TestResult {
        let tar = tar_of(&[file(b"/etc/passwd", b"x")], 0)?;
        assert!(matches!(pack(&tar), Err(PackError::BadPath { .. })));
        Ok(())
    }

    #[test]
    fn rootfs_nul_in_name_is_rejected() -> TestResult {
        let tar = gnu_long_name_tar(b"etc/a\0b\0")?;
        assert!(matches!(pack(&tar), Err(PackError::BadPath { .. })));
        Ok(())
    }

    #[test]
    fn rootfs_non_utf8_name_is_rejected() -> TestResult {
        let tar = tar_of(&[file(b"etc/\xffname", b"x")], 0)?;
        assert!(matches!(pack(&tar), Err(PackError::BadPath { .. })));
        Ok(())
    }

    #[test]
    fn rootfs_character_device_is_rejected() -> TestResult {
        let tar = tar_of(&[special(b"x/console", EntryType::Char)], 0)?;
        assert!(matches!(
            pack(&tar),
            Err(PackError::UnsupportedEntry { .. })
        ));
        Ok(())
    }

    #[test]
    fn rootfs_fifo_is_rejected() -> TestResult {
        let tar = tar_of(&[special(b"x/pipe", EntryType::Fifo)], 0)?;
        assert!(matches!(
            pack(&tar),
            Err(PackError::UnsupportedEntry { .. })
        ));
        Ok(())
    }

    #[test]
    fn rootfs_oversized_entry_is_rejected_before_reading() -> TestResult {
        // The header declares a megabyte that is not there: reading it would be an I/O error, so
        // EntryTooLarge proves the cap is applied from the header alone.
        let mut big = file(b"big", b"");
        big.declared_size = Some(1_048_576);
        let tar = tar_of(&[big], 0)?;
        let limits = PackLimits::new(1 << 32, 100, 10, 4096);
        assert!(matches!(
            pack_root_image(tar.as_slice(), INIT, &limits),
            Err(PackError::EntryTooLarge {
                size: 1_048_576,
                limit: 10,
                ..
            })
        ));
        Ok(())
    }

    #[test]
    fn rootfs_oversized_tar_is_rejected() -> TestResult {
        let tar = tar_of(
            &[
                file(b"a", &[1; 512]),
                file(b"b", &[2; 512]),
                file(b"c", &[3; 512]),
            ],
            0,
        )?;
        let limits = PackLimits::new(1024, 100, 4096, 4096);
        assert!(matches!(
            pack_root_image(tar.as_slice(), INIT, &limits),
            Err(PackError::TarTooLarge { limit: 1024 })
        ));
        Ok(())
    }

    #[test]
    fn rootfs_too_many_entries_is_rejected() -> TestResult {
        let tar = tar_of(&[file(b"a", b"1"), file(b"b", b"2"), file(b"c", b"3")], 0)?;
        let limits = PackLimits::new(1 << 32, 2, 4096, 4096);
        assert!(matches!(
            pack_root_image(tar.as_slice(), INIT, &limits),
            Err(PackError::TooManyEntries { limit: 2 })
        ));
        Ok(())
    }

    #[test]
    fn rootfs_hardlink_copies_count_against_the_content_cap() -> TestResult {
        let tar = tar_of(
            &[
                file(b"a", &[7; 4000]),
                link(b"b", EntryType::Link, b"a"),
                link(b"c", EntryType::Link, b"a"),
                link(b"d", EntryType::Link, b"a"),
            ],
            0,
        )?;
        let limits = PackLimits::new(8192, 100, 4096, 4096);
        assert!(matches!(
            pack_root_image(tar.as_slice(), INIT, &limits),
            Err(PackError::ContentTooLarge { limit: 8192 })
        ));
        Ok(())
    }

    #[test]
    fn rootfs_hardlink_to_missing_target_is_rejected() -> TestResult {
        let tar = tar_of(&[link(b"b", EntryType::Link, b"nowhere")], 0)?;
        assert!(matches!(
            pack(&tar),
            Err(PackError::HardlinkTargetMissing { .. })
        ));
        Ok(())
    }

    #[test]
    fn rootfs_regular_file_at_proc_is_rejected() -> TestResult {
        let tar = tar_of(&[file(b"proc", b"not a directory")], 0)?;
        assert!(matches!(
            pack(&tar),
            Err(PackError::MountPoint { path: "proc" })
        ));
        Ok(())
    }

    #[test]
    fn rootfs_minimal_root_image_golden() -> TestResult {
        let image = minimal_root_image(b"minos-golden-init")?;
        assert_eq!(
            image.report().image_sha256(),
            "0000000000000000000000000000000000000000000000000000000000000000"
        );
        Ok(())
    }
}
