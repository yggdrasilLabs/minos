//! Root image writer: an untrusted rootfs tar plus the trusted guest as /init, packed into a raw read-only `SquashFS` image.
//!
//! The tar comes from a user's Dockerfile, so every name, type, size, and link in it is hostile
//! (02-CONTEXT.md D-01, D-17). It is parsed in memory and never unpacked onto the host. Caps apply
//! before anything is read: total tar bytes, entry count, per-entry size (taken from the header),
//! and path length. The tar crate buffers GNU long-name, long-link, and pax extended headers whole
//! before the path cap can see them, so a [`HeaderGuard`] watches each 512-byte header as it streams
//! past and refuses one of those that declares more than the cap, before the crate allocates it.
//! Only regular files, directories, symlinks, and hardlinks are accepted; a hardlink becomes a copy
//! of an earlier regular file. Every mtime in the image is 0, nodes are ordered by path, and the
//! image is gzip-compressed, so the same tar always gives the same bytes.
//!
//! Peak memory is the file content kept in the tree, bounded by the tar cap (hardlink copies share
//! bytes), plus the compressed image, plus the tar crate's buffer for the entry being read. Phase 5
//! should stream file data to a temporary file instead.
//!
//! The image gets the mount points the guest needs on a read-only root (`/dev`, `/proc`, `/sys`,
//! `/tmp`, `/mnt`), and `/init` is written last from the trusted guest bytes, replacing anything the
//! tar put there.

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Cursor, Read};
use std::rc::Rc;

use backhand::compression::Compressor;
use backhand::{BackhandError, FilesystemCompressor, FilesystemWriter, NodeHeader};
use serde::Serialize;
use sha2::{Digest, Sha256};
use tar::EntryType;

/// Caps applied to a rootfs tar before and while it is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackLimits {
    tar_bytes: u64,
    entries: u32,
    entry_bytes: u64,
    path_bytes: usize,
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
            tar_bytes: max_tar_bytes,
            entries: max_entries,
            entry_bytes: max_entry_bytes,
            path_bytes: max_path_bytes,
        }
    }
}

impl Default for PackLimits {
    /// 1 GiB of tar, 200,000 entries, 256 MiB per entry, 4,096 bytes per path. The tar cap bounds the
    /// file content held in memory while the image is built, so it is sized for a v1 environment,
    /// not for the largest image a host could hold.
    fn default() -> Self {
        Self::new(1_073_741_824, 200_000, 268_435_456, 4096)
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
    /// A GNU long-name, GNU long-link, or pax extended header declares more bytes than its cap: the
    /// path cap plus one (the trailing NUL) for the GNU forms, [`PAX_HEADER_BYTES`] for pax. Refused
    /// from the header alone, before the tar crate buffers the record.
    #[error("a {what} header declares {size} bytes, over its {limit}-byte cap")]
    ExtensionHeaderTooLarge {
        /// Which kind of header.
        what: &'static str,
        /// The size its header declares.
        size: u64,
        /// The cap applied.
        limit: u64,
    },
    /// The tar is not well formed: a bad checksum, an unparsable numeric field, a block or entry that ended
    /// early, or another defect the tar parser reported. The input's fault, never retried.
    #[error("rootfs tar is malformed: {what:?}")]
    MalformedTar {
        /// The parser's description, lossy and cut to 256 characters; it can quote hostile bytes, so it is
        /// shown escaped.
        what: String,
    },
    /// Reading the tar from the host or writing the image failed at the operating-system level: the error
    /// carries an errno. Infrastructure's fault; a retry can succeed.
    #[error("rootfs tar or image I/O")]
    Io(#[source] std::io::Error),
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
    let consumed = Rc::new(Cell::new(0_u64));
    let reader = Counted {
        inner: HeaderGuard::new(rootfs_tar.take(limits.tar_bytes.saturating_add(1)), limits),
        consumed: Rc::clone(&consumed),
    };
    let mut tree = Tree::default();
    let parsed = read_tar(reader, limits, &mut tree);
    // Checked whatever the parse returned: input cut at the cap can surface as a short read, or,
    // at a block boundary, as a clean end of archive.
    if consumed.get() > limits.tar_bytes {
        return Err(PackError::TarTooLarge {
            limit: limits.tar_bytes,
        });
    }
    parsed?;
    tree.add_mount_points()?;
    tree.inject_init(init);
    let (bytes, counts) = tree.write_image()?;
    let report = PackReport {
        image_sha256: sha256_hex(&bytes),
        init_sha256: sha256_hex(init),
        files: counts.files,
        dirs: counts.dirs,
        symlinks: counts.symlinks,
        hardlinks_copied: tree.hardlinks_copied,
        setid_files: counts.setid_files,
        skipped_global_headers: tree.skipped_global_headers,
        replaced_duplicates: tree.replaced_duplicates,
        init_overridden: tree.init_overridden,
    };
    Ok(PackedImage { bytes, report })
}

/// Packs an empty rootfs: the mount points and `/init` only, with default limits.
///
/// # Errors
///
/// A [`PackError`] from the `SquashFS` writer.
pub fn minimal_root_image(init: &[u8]) -> Result<PackedImage, PackError> {
    pack_root_image(io::empty(), init, &PackLimits::default())
}

/// The name `/init` has in the tree (paths are kept without a leading slash).
const INIT_NAME: &str = "init";

/// Mount points the guest needs on a read-only root, with the mode each gets when the tar lacks it.
const MOUNT_POINTS: [(&str, u16); 5] = [
    ("dev", 0o755),
    ("mnt", 0o755),
    ("proc", 0o755),
    ("sys", 0o755),
    ("tmp", 0o1777),
];

/// Mode of the root, of `/init`, and of any parent directory the tar left out.
const DIR_MODE: u16 = 0o755;
const INIT_MODE: u16 = 0o755;

/// Setuid and setgid bits.
const SETID_BITS: u16 = 0o6000;

/// `SquashFS` limits a name to 256 bytes; 255 keeps clear of it.
const MAX_NAME_BYTES: usize = 255;

/// Hostile paths in error messages are cut to this many characters.
const SHOWN_PATH_CHARS: usize = 256;

/// The most file data allocated up front for one entry; the rest grows as it is read.
const PREALLOC_BYTES: u64 = 1 << 20;

/// A tar block: every header is one, and entry data is padded to a whole number of them.
const BLOCK_BYTES: usize = 512;

/// Largest pax extended header (`x` or `g`) accepted. Its records are short `key=value` lines, and a
/// path record can be at most the path cap, so this is generous.
pub const PAX_HEADER_BYTES: u64 = 65_536;

/// Watches the tar's blocks as they stream past the tar crate and refuses a GNU long-name (`L`),
/// GNU long-link (`K`), or pax extended (`x`, `g`) header whose declared size exceeds its cap.
///
/// tar 0.4.46 reads those records whole with `read_all` before the caller sees the entry, so the
/// path cap in [`normalize`] comes too late to bound that allocation. The guard tracks where each
/// header falls the way the crate does: a non-zero 512-byte block is a header, its size field
/// (octal, or base 256 when the first byte's top bit is set) says how many data bytes follow, padded
/// to whole blocks, and an all-zero block ends the archive. The one thing it does not mirror is a
/// pax `size` record overriding the header size, which only a file over 8 GiB needs, far beyond
/// every cap, and GNU sparse entries, which [`read_tar`] refuses anyway; after either, the guard
/// can only be stricter than the crate, never looser.
struct HeaderGuard<R> {
    inner: R,
    /// Bytes of the current header block seen so far.
    header: Vec<u8>,
    /// Data bytes, padded to whole blocks, still to pass before the next header.
    data_remaining: u64,
    /// Set by the end-of-archive marker; nothing after it is inspected.
    done: bool,
    /// Cap for a GNU long name or link: the path cap plus the trailing NUL the crate strips.
    long_name_cap: u64,
}

/// The payload of the `io::Error` a [`HeaderGuard`] raises, so [`tar_io`] can name the cause.
#[derive(Debug)]
struct ExtensionHeaderHit {
    what: &'static str,
    size: u64,
    limit: u64,
}

impl std::fmt::Display for ExtensionHeaderHit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "a {} header declares {} bytes, over its {}-byte cap",
            self.what, self.size, self.limit
        )
    }
}

impl std::error::Error for ExtensionHeaderHit {}

impl<R> HeaderGuard<R> {
    fn new(inner: R, limits: &PackLimits) -> Self {
        Self {
            inner,
            header: Vec::with_capacity(BLOCK_BYTES),
            data_remaining: 0,
            done: false,
            long_name_cap: u64::try_from(limits.path_bytes)
                .unwrap_or(u64::MAX)
                .saturating_add(1),
        }
    }

    /// Accounts for `bytes` just handed to the tar crate.
    fn scan(&mut self, mut bytes: &[u8]) -> io::Result<()> {
        while !bytes.is_empty() && !self.done {
            if self.data_remaining > 0 {
                let skip = usize::try_from(self.data_remaining)
                    .map_or(bytes.len(), |remaining| remaining.min(bytes.len()));
                self.data_remaining = self
                    .data_remaining
                    .saturating_sub(u64::try_from(skip).unwrap_or(u64::MAX));
                bytes = bytes.get(skip..).unwrap_or_default();
                continue;
            }
            let need = BLOCK_BYTES.saturating_sub(self.header.len());
            let (head, rest) = bytes
                .split_at_checked(need.min(bytes.len()))
                .unwrap_or((bytes, &[]));
            self.header.extend_from_slice(head);
            bytes = rest;
            if self.header.len() >= BLOCK_BYTES {
                self.finish_header()?;
                self.header.clear();
            }
        }
        Ok(())
    }

    /// Inspects a complete header block and sets up the data span that follows it.
    fn finish_header(&mut self) -> io::Result<()> {
        if self.header.iter().all(|byte| *byte == 0) {
            self.done = true;
            return Ok(());
        }
        let size = size_field(self.header.get(124..136).unwrap_or_default())?;
        let cap = match self.header.get(156).copied().unwrap_or(0) {
            b'L' => Some(("GNU long name", self.long_name_cap)),
            b'K' => Some(("GNU long link", self.long_name_cap)),
            b'x' | b'g' => Some(("pax extended", PAX_HEADER_BYTES)),
            _ => None,
        };
        if let Some((what, limit)) = cap
            && size > limit
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                ExtensionHeaderHit { what, size, limit },
            ));
        }
        let padded = u64::try_from(BLOCK_BYTES).unwrap_or(u64::MAX);
        self.data_remaining = size
            .checked_add(padded.saturating_sub(1))
            .map(|rounded| rounded & !padded.saturating_sub(1))
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "size overflow"))?;
        Ok(())
    }
}

impl<R: Read> Read for HeaderGuard<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let read = self.inner.read(buf)?;
        if let Some(bytes) = buf.get(..read) {
            self.scan(bytes)?;
        }
        Ok(read)
    }
}

/// The size field as tar 0.4.46 reads it: base 256 over the last eight bytes when the first byte's top
/// bit is set, otherwise octal text up to the first NUL, trimmed.
fn size_field(field: &[u8]) -> io::Result<u64> {
    match field.split_first() {
        Some((first, _)) if first & 0x80 != 0 => Ok(field
            .iter()
            .rev()
            .take(8)
            .rev()
            .fold(0_u64, |acc, byte| (acc << 8) | u64::from(*byte))),
        _ => {
            let digits = field.split(|byte| *byte == 0).next().unwrap_or_default();
            let text = std::str::from_utf8(digits).map_err(|_| {
                io::Error::new(io::ErrorKind::InvalidData, "size field is not UTF-8")
            })?;
            u64::from_str_radix(text.trim(), 8)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "size field is not octal"))
        }
    }
}

/// Counts every byte read from the tar, so input beyond the cap is an error rather than a silent
/// truncation.
struct Counted<R> {
    inner: R,
    consumed: Rc<Cell<u64>>,
}

impl<R: Read> Read for Counted<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let read = self.inner.read(buf)?;
        let total = self
            .consumed
            .get()
            .saturating_add(u64::try_from(read).unwrap_or(u64::MAX));
        self.consumed.set(total);
        Ok(read)
    }
}

/// Permission bits and ownership of one node. Every mtime in the image is 0.
#[derive(Debug, Clone, Copy)]
struct Meta {
    mode: u16,
    uid: u32,
    gid: u32,
}

impl Meta {
    const ROOT_DIR: Self = Self {
        mode: DIR_MODE,
        uid: 0,
        gid: 0,
    };

    fn node_header(self) -> NodeHeader {
        NodeHeader::new(self.mode, self.uid, self.gid, 0)
    }
}

/// One node of the image. File bytes are shared, so a hardlink copy costs no memory.
#[derive(Debug, Clone)]
enum Node {
    File { data: Rc<[u8]>, meta: Meta },
    Dir { meta: Meta },
    Symlink { target: String, meta: Meta },
}

/// The validated tree, keyed by normalized path (no leading slash), so iteration order is stable.
#[derive(Default)]
struct Tree {
    nodes: BTreeMap<String, Node>,
    content_bytes: u64,
    hardlinks_copied: u64,
    skipped_global_headers: u64,
    replaced_duplicates: u64,
    init_overridden: bool,
}

/// Node counts taken while the image is written.
#[derive(Default)]
struct Counts {
    files: u64,
    dirs: u64,
    symlinks: u64,
    setid_files: u64,
}

/// Parses every tar entry into `tree`, applying each cap before the data it guards is read.
fn read_tar(reader: impl Read, limits: &PackLimits, tree: &mut Tree) -> Result<(), PackError> {
    let mut archive = tar::Archive::new(reader);
    let mut seen: u32 = 0;
    for entry in archive.entries().map_err(tar_io)? {
        let mut entry = entry.map_err(tar_io)?;
        seen = seen.saturating_add(1);
        if seen > limits.entries {
            return Err(PackError::TooManyEntries {
                limit: limits.entries,
            });
        }
        let kind = entry.header().entry_type();
        if kind == EntryType::XGlobalHeader {
            tree.skipped_global_headers += 1;
            continue;
        }
        let path = normalize(&entry.path_bytes(), limits.path_bytes)?;
        if path.is_empty() {
            if kind == EntryType::Directory {
                // The archive root ("./"): the image root is fixed at mode 0755, uid 0, gid 0.
                continue;
            }
            return Err(PackError::BadPath {
                path: String::new(),
                reason: "is empty",
            });
        }
        let node = match kind {
            EntryType::Regular | EntryType::Continuous => {
                let declared = entry.header().size().map_err(tar_io)?.max(entry.size());
                if declared > limits.entry_bytes {
                    return Err(PackError::EntryTooLarge {
                        path: shown(path.as_bytes()),
                        size: declared,
                        limit: limits.entry_bytes,
                    });
                }
                let meta = meta_of(entry.header(), &path)?;
                let size = entry.size();
                let data = read_data(&mut entry, size)?;
                tree.add_content(data.len(), limits)?;
                Node::File { data, meta }
            }
            EntryType::Directory => Node::Dir {
                meta: meta_of(entry.header(), &path)?,
            },
            EntryType::Symlink => Node::Symlink {
                target: symlink_target(
                    &path,
                    entry.link_name_bytes().as_deref(),
                    limits.path_bytes,
                )?,
                meta: meta_of(entry.header(), &path)?,
            },
            EntryType::Link => {
                let node = tree.hardlink(&path, entry.link_name_bytes().as_deref(), limits)?;
                tree.hardlinks_copied += 1;
                node
            }
            other => {
                return Err(PackError::UnsupportedEntry {
                    path: shown(path.as_bytes()),
                    kind: format!("{other:?}"),
                });
            }
        };
        if tree.nodes.insert(path, node).is_some() {
            tree.replaced_duplicates += 1;
        }
    }
    Ok(())
}

impl Tree {
    /// Adds `len` bytes of file content, failing once the total passes `max_tar_bytes`.
    fn add_content(&mut self, len: usize, limits: &PackLimits) -> Result<(), PackError> {
        self.content_bytes = self
            .content_bytes
            .saturating_add(u64::try_from(len).unwrap_or(u64::MAX));
        if self.content_bytes > limits.tar_bytes {
            return Err(PackError::ContentTooLarge {
                limit: limits.tar_bytes,
            });
        }
        Ok(())
    }

    /// A copy of the earlier regular file a hardlink names. Content counts against the cap again.
    fn hardlink(
        &mut self,
        path: &str,
        target: Option<&[u8]>,
        limits: &PackLimits,
    ) -> Result<Node, PackError> {
        let missing = |target: &[u8]| PackError::HardlinkTargetMissing {
            path: shown(path.as_bytes()),
            target: shown(target),
        };
        let raw = target.ok_or_else(|| missing(b""))?;
        let normalized = normalize(raw, limits.path_bytes).map_err(|_| PackError::BadPath {
            path: shown(path.as_bytes()),
            reason: "is a hardlink whose target is not a valid archive path",
        })?;
        let (data, meta) = match self.nodes.get(&normalized) {
            Some(Node::File { data, meta }) => (Rc::clone(data), *meta),
            _ => return Err(missing(raw)),
        };
        self.add_content(data.len(), limits)?;
        Ok(Node::File { data, meta })
    }

    /// Makes sure each mount point exists as a directory.
    fn add_mount_points(&mut self) -> Result<(), PackError> {
        for (name, mode) in MOUNT_POINTS {
            match self.nodes.get(name) {
                Some(Node::Dir { .. }) => {}
                Some(_) => return Err(PackError::MountPoint { path: name }),
                None => {
                    self.nodes.insert(
                        name.to_owned(),
                        Node::Dir {
                            meta: Meta {
                                mode,
                                uid: 0,
                                gid: 0,
                            },
                        },
                    );
                }
            }
        }
        Ok(())
    }

    /// Removes anything the tar put at `init` (and under it), then writes the trusted bytes there.
    fn inject_init(&mut self, init: &[u8]) {
        let before = self.nodes.len();
        self.nodes
            .retain(|path, _| path != INIT_NAME && !path.starts_with("init/"));
        self.init_overridden = self.nodes.len() != before;
        self.nodes.insert(
            INIT_NAME.to_owned(),
            Node::File {
                data: Rc::from(init),
                meta: Meta {
                    mode: INIT_MODE,
                    uid: 0,
                    gid: 0,
                },
            },
        );
    }

    /// Writes the tree as a gzip `SquashFS` image with every mtime 0.
    fn write_image(&self) -> Result<(Vec<u8>, Counts), PackError> {
        let mut fs = FilesystemWriter::default();
        // The default compressor is Xz, which this build does not compile in.
        fs.set_compressor(
            FilesystemCompressor::new(Compressor::Gzip, None).map_err(squashfs_error)?,
        );
        fs.set_root_mode(DIR_MODE);
        let mut counts = Counts::default();
        let mut implicit: BTreeSet<&str> = BTreeSet::new();
        for (path, node) in &self.nodes {
            for (end, _) in path.match_indices('/') {
                let Some(parent) = path.get(..end) else {
                    continue;
                };
                match self.nodes.get(parent) {
                    // Pushed already: a parent sorts before every path under it.
                    Some(Node::Dir { .. }) => {}
                    Some(_) => {
                        return Err(PackError::BadPath {
                            path: shown(path.as_bytes()),
                            reason: "has a parent that is not a directory",
                        });
                    }
                    None => {
                        if implicit.insert(parent) {
                            fs.push_dir(parent, Meta::ROOT_DIR.node_header())
                                .map_err(squashfs_error)?;
                            counts.dirs += 1;
                        }
                    }
                }
            }
            match node {
                Node::File { data, meta } => {
                    fs.push_file(Cursor::new(Rc::clone(data)), path, meta.node_header())
                        .map_err(squashfs_error)?;
                    counts.files += 1;
                    if meta.mode & SETID_BITS != 0 {
                        counts.setid_files += 1;
                    }
                }
                Node::Dir { meta } => {
                    fs.push_dir(path, meta.node_header())
                        .map_err(squashfs_error)?;
                    counts.dirs += 1;
                }
                Node::Symlink { target, meta } => {
                    fs.push_symlink(target.as_str(), path, meta.node_header())
                        .map_err(squashfs_error)?;
                    counts.symlinks += 1;
                }
            }
        }
        let mut out = Cursor::new(Vec::new());
        fs.write(&mut out).map_err(squashfs_error)?;
        Ok((out.into_inner(), counts))
    }
}

/// Normalizes an archive path to `a/b/c`: one leading `.` component and one trailing slash are
/// dropped; anything else that is not a plain UTF-8 name is refused.
fn normalize(raw: &[u8], max_path_bytes: usize) -> Result<String, PackError> {
    let bad = |reason: &'static str| PackError::BadPath {
        path: shown(raw),
        reason,
    };
    if raw.len() > max_path_bytes {
        return Err(bad("is longer than the path cap"));
    }
    if raw.contains(&0) {
        return Err(bad("contains a NUL byte"));
    }
    let text = std::str::from_utf8(raw).map_err(|_| bad("is not UTF-8"))?;
    if text.starts_with('/') {
        return Err(bad("is absolute"));
    }
    let mut names: Vec<&str> = Vec::new();
    let mut pieces = text.split('/').enumerate().peekable();
    while let Some((index, piece)) = pieces.next() {
        let last = pieces.peek().is_none();
        match piece {
            "." if index == 0 => {}
            "" if last => {}
            "" => return Err(bad("has an empty component")),
            "." => return Err(bad("has a current-dir component after the start")),
            ".." => return Err(bad("has a parent-dir component")),
            name if name.len() > MAX_NAME_BYTES => {
                return Err(bad("has a component longer than 255 bytes"));
            }
            name => names.push(name),
        }
    }
    Ok(names.join("/"))
}

/// A symlink target: kept as written (it resolves inside the guest, never on the host), but it
/// must be non-empty UTF-8 without NUL and within the path cap.
fn symlink_target(
    path: &str,
    raw: Option<&[u8]>,
    max_path_bytes: usize,
) -> Result<String, PackError> {
    let bad = |reason: &'static str| PackError::BadPath {
        path: shown(path.as_bytes()),
        reason,
    };
    let raw = raw.ok_or_else(|| bad("is a symlink with no target"))?;
    if raw.is_empty() {
        return Err(bad("is a symlink with an empty target"));
    }
    if raw.len() > max_path_bytes {
        return Err(bad("is a symlink whose target is longer than the path cap"));
    }
    if raw.contains(&0) {
        return Err(bad("is a symlink whose target contains a NUL byte"));
    }
    std::str::from_utf8(raw)
        .map(str::to_owned)
        .map_err(|_| bad("is a symlink whose target is not UTF-8"))
}

/// Permission bits, uid, and gid from a header. Type bits are dropped; setid bits are kept.
fn meta_of(header: &tar::Header, path: &str) -> Result<Meta, PackError> {
    let mode = header.mode().map_err(tar_io)? & 0o7777;
    let id = |which: &'static str, id: u64| {
        u32::try_from(id).map_err(|_| PackError::OwnershipOutOfRange {
            path: shown(path.as_bytes()),
            which,
            id,
        })
    };
    Ok(Meta {
        // The mask leaves 12 bits, so the conversion cannot fail.
        mode: u16::try_from(mode).unwrap_or(0),
        uid: id("uid", header.uid().map_err(tar_io)?)?,
        gid: id("gid", header.gid().map_err(tar_io)?)?,
    })
}

/// Reads exactly `size` bytes of entry data. A short read means the tar is malformed.
fn read_data(entry: &mut impl Read, size: u64) -> Result<Rc<[u8]>, PackError> {
    let capacity = usize::try_from(size.min(PREALLOC_BYTES)).unwrap_or(0);
    let mut data = Vec::with_capacity(capacity);
    entry.take(size).read_to_end(&mut data).map_err(tar_io)?;
    if u64::try_from(data.len()).ok() != Some(size) {
        return Err(PackError::MalformedTar {
            what: "entry data ended early".to_owned(),
        });
    }
    Ok(Rc::from(data))
}

/// Sorts an error from the tar reader into the input's fault or the host's.
///
/// The tar crate reports a bad checksum, a numeric field that is not a number, a block that ended early, and
/// its other parse failures as `io::Error`s it made itself (mostly `ErrorKind::Other`), which carry no errno.
/// A read that failed at the operating system does carry one. That distinction, not the kind, separates
/// [`PackError::MalformedTar`] from [`PackError::Io`].
fn tar_io(error: io::Error) -> PackError {
    if error.raw_os_error().is_some() {
        return PackError::Io(error);
    }
    // The guard's refusal travels inside the io::Error (possibly wrapped by the crate); find it.
    let mut cause: Option<&(dyn std::error::Error + 'static)> = error
        .get_ref()
        .map(|inner| inner as &(dyn std::error::Error + 'static));
    while let Some(current) = cause {
        if let Some(hit) = current.downcast_ref::<ExtensionHeaderHit>() {
            return PackError::ExtensionHeaderTooLarge {
                what: hit.what,
                size: hit.size,
                limit: hit.limit,
            };
        }
        cause = current.source();
    }
    PackError::MalformedTar {
        what: shown(error.to_string().as_bytes()),
    }
}

/// A hostile path for an error message: lossy UTF-8, cut to [`SHOWN_PATH_CHARS`] characters.
/// Messages print it with `{:?}`, which escapes control characters.
fn shown(raw: &[u8]) -> String {
    String::from_utf8_lossy(raw)
        .chars()
        .take(SHOWN_PATH_CHARS)
        .collect()
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest: [u8; 32] = Sha256::digest(bytes).into();
    base16ct::lower::encode_string(&digest)
}

/// Maps a writer error to [`PackError`]. Without backhand's `error-strings` feature its `Display`
/// prints nothing, so the variant is named here.
fn squashfs_error(error: BackhandError) -> PackError {
    let what = match error {
        BackhandError::StdIo(io) => return PackError::Io(io),
        BackhandError::Deku(_) => "encoding failed",
        BackhandError::StringUtf8(_) | BackhandError::StrUtf8(_) => "invalid UTF-8",
        BackhandError::UnsupportedCompression(_) => "unsupported compression",
        BackhandError::MissingCompressor => "missing compressor",
        BackhandError::FileNotFound => "file not found",
        BackhandError::Unreachable => "unreachable branch",
        BackhandError::UnexpectedInode => "unexpected inode",
        BackhandError::UnsupportedInode => "unsupported inode",
        BackhandError::CorruptedOrInvalidSquashfs => "corrupted or invalid image",
        BackhandError::InvalidCompressionOption => "invalid compression option",
        BackhandError::InvalidFilePath => "invalid file path",
        BackhandError::UndefineFileName => "undefined file name",
        BackhandError::DuplicatedFileName => "duplicated file name",
        BackhandError::TryReserveError(_) => "allocation failed",
        BackhandError::InvalidIdTable => "invalid id table",
        BackhandError::UnsupportedSquashfsVersion(..) => "unsupported version",
        BackhandError::NumericConversion(_) => "numeric conversion failed",
        BackhandError::SystemTime(_) => "system time",
        BackhandError::MutexPoisoned => "mutex poisoned",
        BackhandError::IdNotFoundInTable => "id not in table",
        BackhandError::InternalState(_) => "internal state",
        BackhandError::CompressionInit(_) => "compressor initialization failed",
    };
    PackError::Squashfs { what }
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
        extension_tar(EntryType::GNULongName, name, None)
    }

    /// A tar whose first entry is an extension header of `kind` holding `record`, declaring
    /// `declared` bytes if given (the data is still `record`), then one regular file "placeholder".
    fn extension_tar(
        kind: EntryType,
        record: &[u8],
        declared: Option<u64>,
    ) -> Result<Vec<u8>, Box<dyn Error>> {
        let mut builder = tar::Builder::new(Vec::new());
        let mut long = Header::new_gnu();
        for (dst, src) in long.as_old_mut().name.iter_mut().zip(b"././@LongLink") {
            *dst = *src;
        }
        long.set_entry_type(kind);
        long.set_mode(0o644);
        long.set_size(match declared {
            Some(size) => size,
            None => u64::try_from(record.len())?,
        });
        long.set_cksum();
        builder.append(&long, record)?;
        let mut header = Header::new_gnu();
        for (dst, src) in header.as_old_mut().name.iter_mut().zip(b"placeholder") {
            *dst = *src;
        }
        header.set_entry_type(EntryType::Regular);
        header.set_mode(0o644);
        header.set_uid(0);
        header.set_gid(0);
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
        assert!(matches!(
            pack(&tar),
            Err(PackError::BadPath {
                reason: "has a parent-dir component",
                ..
            })
        ));
        Ok(())
    }

    #[test]
    fn rootfs_absolute_path_is_rejected() -> TestResult {
        let tar = tar_of(&[file(b"/etc/passwd", b"x")], 0)?;
        assert!(matches!(
            pack(&tar),
            Err(PackError::BadPath {
                reason: "is absolute",
                ..
            })
        ));
        Ok(())
    }

    #[test]
    fn rootfs_nul_in_name_is_rejected() -> TestResult {
        let tar = gnu_long_name_tar(b"etc/a\0b\0")?;
        assert!(matches!(
            pack(&tar),
            Err(PackError::BadPath {
                reason: "contains a NUL byte",
                ..
            })
        ));
        Ok(())
    }

    #[test]
    fn rootfs_oversized_long_name_is_refused_from_its_header() -> TestResult {
        // The record declares a megabyte that is not there. Without the guard the crate would read
        // to the end of input and fail with a truncation; the guard refuses at the header instead.
        let tar = extension_tar(EntryType::GNULongName, b"", Some(1_048_576))?;
        assert!(matches!(
            pack(&tar),
            Err(PackError::ExtensionHeaderTooLarge {
                what: "GNU long name",
                size: 1_048_576,
                limit: 4097
            })
        ));
        let tar = extension_tar(EntryType::GNULongLink, b"", Some(4098))?;
        assert!(matches!(
            pack(&tar),
            Err(PackError::ExtensionHeaderTooLarge {
                what: "GNU long link",
                size: 4098,
                limit: 4097
            })
        ));
        let tar = extension_tar(EntryType::XHeader, b"", Some(65_537))?;
        assert!(matches!(
            pack(&tar),
            Err(PackError::ExtensionHeaderTooLarge {
                what: "pax extended",
                size: 65_537,
                limit: 65_536
            })
        ));
        Ok(())
    }

    #[test]
    fn rootfs_long_name_within_the_cap_is_accepted() -> TestResult {
        // 16 components of 255 bytes and 15 slashes: 4095 bytes, within the 4096-byte path cap.
        let name = vec!["a".repeat(255); 16].join("/");
        let tar = gnu_long_name_tar(name.as_bytes())?;
        let image = pack(&tar)?;
        let fs = read_back(&image)?;
        assert_eq!(file_bytes(&fs, &format!("/{name}"))?, b"x");
        Ok(())
    }

    #[test]
    fn rootfs_pax_path_within_the_cap_is_accepted() -> TestResult {
        let tar = extension_tar(EntryType::XHeader, b"18 path=etc/named\n", None)?;
        let image = pack(&tar)?;
        let fs = read_back(&image)?;
        assert_eq!(file_bytes(&fs, "/etc/named")?, b"x");
        Ok(())
    }

    #[test]
    fn rootfs_base256_size_is_read_like_the_crate() -> TestResult {
        // A regular file with its size in base 256 (the crate's own writer emits it for sizes over
        // the octal range; here it is forced for a small one). The guard must follow the data span.
        let mut big = file(b"a", &[9; 600]);
        big.declared_size = Some(600);
        let mut tar = tar_of(&[big, file(b"b", b"y")], 0)?;
        // 600 = 2 * 256 + 88, big-endian in the last eight bytes, top bit set on the first.
        let field: [u8; 12] = [0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 88];
        for (dst, src) in tar.iter_mut().skip(124).zip(field) {
            *dst = src;
        }
        let header = tar.get_mut(..512).ok_or("short tar")?;
        let mut rebuilt = Header::new_gnu();
        rebuilt.as_mut_bytes().copy_from_slice(header);
        rebuilt.set_cksum();
        header.copy_from_slice(rebuilt.as_bytes());
        let image = pack(&tar)?;
        let fs = read_back(&image)?;
        assert_eq!(file_bytes(&fs, "/a")?, vec![9; 600]);
        assert_eq!(file_bytes(&fs, "/b")?, b"y");
        Ok(())
    }

    #[test]
    fn rootfs_non_utf8_name_is_rejected() -> TestResult {
        let tar = tar_of(&[file(b"etc/\xffname", b"x")], 0)?;
        assert!(matches!(
            pack(&tar),
            Err(PackError::BadPath {
                reason: "is not UTF-8",
                ..
            })
        ));
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
    fn rootfs_truncated_tar_is_malformed_not_io() -> TestResult {
        let tar = tar_of(&[file(b"a", &[1; 4096]), file(b"b", b"x")], 0)?;
        // Cut inside the first entry's data: the entry ends early.
        let mid_data = tar.get(..512 + 100).ok_or("short tar")?;
        assert!(matches!(
            pack(mid_data),
            Err(PackError::MalformedTar { what }) if what == "entry data ended early"
        ));
        // Cut inside the second entry's header: the block ends early.
        let mid_header = tar.get(..512 + 4096 + 100).ok_or("short tar")?;
        assert!(matches!(
            pack(mid_header),
            Err(PackError::MalformedTar { what }) if what.contains("entire block")
        ));
        Ok(())
    }

    #[test]
    fn rootfs_bad_checksum_is_malformed() -> TestResult {
        let mut tar = tar_of(&[file(b"a", b"x")], 0)?;
        // The name field is covered by the checksum; changing a byte of it breaks the sum.
        if let Some(byte) = tar.first_mut() {
            *byte = b'z';
        }
        assert!(matches!(
            pack(&tar),
            Err(PackError::MalformedTar { what }) if what.contains("checksum")
        ));
        Ok(())
    }

    #[test]
    fn rootfs_host_read_failure_is_io() {
        /// A reader whose first read fails the way a bad disk does, with an errno.
        struct Failing;
        impl Read for Failing {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::from_raw_os_error(5))
            }
        }
        assert!(matches!(
            pack_root_image(Failing, INIT, &PackLimits::default()),
            Err(PackError::Io(error)) if error.raw_os_error() == Some(5)
        ));
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
            "8e4f12d45a06ef4bf20fe0f37e74ca06e2b7b108a8c4325ed2c7f6a6600e4e98"
        );
        Ok(())
    }
}
