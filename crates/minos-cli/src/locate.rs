//! Where `minos` finds the `minos-vmm` helper, the static `minos-guest`, and the guest kernel (05-CONTEXT D-04).
//!
//! The locations are fixed, with no configuration file and no environment variable, so nothing in the user's
//! environment can redirect a launch to another binary or kernel. `MINOS_VMM`, `MINOS_GUEST`, and `MINOS_KRUNFW`
//! are read only by the ignored VM tests, never here.
//!
//! In order, [`locate`] checks:
//!
//! 1. `<dir>/minos-vmm`, where `<dir>` is the canonical directory of the running `minos`: accepted when it is a
//!    regular file.
//! 2. `<dir>/minos-guest`: accepted only when it is a static AArch64 ELF (`check_guest_elf`), as in an installed
//!    layout. In a development checkout this is the macOS build, a Mach-O file, which is rejected with its reason
//!    (05-RESEARCH Pitfall 8): packed as `/init` it would never boot, and the run would be blamed on the candidate.
//! 3. `<dir>/../aarch64-unknown-linux-musl/release/minos-guest`, the cargo musl build of a development checkout,
//!    with the same check, only when step 2 found nothing.
//!
//! libkrunfw is always [`KRUNFW_PATH`], its Homebrew Cellar path; the host probes check its pinned SHA-256.
//! Every path checked is kept, with what was found there, so `minos host doctor` can name each one.

use std::fs::{self, File};
use std::io::{ErrorKind, Read};
use std::path::{Path, PathBuf};

use minos_backend_krun::host::{HostPaths, KRUNFW_PATH};
use minos_backend_krun::rootfs::{ElfError, check_guest_elf};

/// The helper's file name.
pub const VMM_NAME: &str = "minos-vmm";

/// The guest's file name.
pub const GUEST_NAME: &str = "minos-guest";

/// The cargo target triple the guest is built for, as a directory under the cargo target directory.
pub const GUEST_TARGET: &str = "aarch64-unknown-linux-musl";

/// The largest guest binary [`locate`] reads; a larger file is rejected unread past the cap.
pub const GUEST_MAX_BYTES: u64 = 64 * 1024 * 1024;

/// One path [`locate`] checked, and what it found there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    path: PathBuf,
    accepted: bool,
    reason: &'static str,
}

impl Candidate {
    /// The path as checked, before it was resolved.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Whether the file there was accepted.
    #[must_use]
    pub const fn is_accepted(&self) -> bool {
        self.accepted
    }

    /// What was found: why it was accepted or rejected.
    #[must_use]
    pub const fn reason(&self) -> &'static str {
        self.reason
    }
}

/// What [`locate`] found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Located {
    vmm: Option<PathBuf>,
    guest: Option<PathBuf>,
    krunfw: PathBuf,
    vmm_checked: PathBuf,
    guest_checked: PathBuf,
    checked: Vec<Candidate>,
}

impl Located {
    /// The canonical path of the helper, when one was found.
    #[must_use]
    pub fn vmm(&self) -> Option<&Path> {
        self.vmm.as_deref()
    }

    /// The canonical path of the static AArch64 guest, when one was found.
    #[must_use]
    pub fn guest(&self) -> Option<&Path> {
        self.guest.as_deref()
    }

    /// The libkrunfw dylib: always [`KRUNFW_PATH`].
    #[must_use]
    pub fn krunfw(&self) -> &Path {
        &self.krunfw
    }

    /// Every path checked, in the order checked.
    #[must_use]
    pub fn checked(&self) -> &[Candidate] {
        &self.checked
    }
}

/// Finds the helper, the guest, and libkrunfw for the `minos` binary at `exe` (normally
/// [`std::env::current_exe`]), in the order the module documentation gives. Reads no environment variable and no
/// configuration file.
#[must_use]
pub fn locate(exe: &Path) -> Located {
    Located {
        vmm: None,
        guest: None,
        krunfw: PathBuf::from(KRUNFW_PATH),
        vmm_checked: exe.to_path_buf(),
        guest_checked: exe.to_path_buf(),
        checked: Vec::new(),
    }
}

/// The paths the backend launches with, when both binaries were found.
#[must_use]
pub fn host_paths(located: &Located) -> Option<HostPaths> {
    match (located.vmm(), located.guest()) {
        (Some(vmm), Some(guest)) => Some(HostPaths::new(vmm, guest, located.krunfw())),
        _ => None,
    }
}

/// The paths the host probes check: each binary found, or, for one not found, the last path [`locate`] checked
/// for it, so the probe refuses naming that path and every other probe still runs.
#[must_use]
pub fn probe_paths(located: &Located) -> HostPaths {
    HostPaths::new(
        located.vmm().unwrap_or(&located.vmm_checked),
        located.guest().unwrap_or(&located.guest_checked),
        located.krunfw(),
    )
}

/// Runs `test` on `path`, records the result, and returns the canonical path when it was accepted.
fn check(
    path: &Path,
    test: fn(&Path) -> Result<&'static str, &'static str>,
    checked: &mut Vec<Candidate>,
) -> Option<PathBuf> {
    let result = test(path).and_then(|reason| {
        fs::canonicalize(path)
            .map(|canonical| (canonical, reason))
            .map_err(|_| "its path could not be resolved")
    });
    let (found, accepted, reason) = match result {
        Ok((canonical, reason)) => (Some(canonical), true, reason),
        Err(reason) => (None, false, reason),
    };
    checked.push(Candidate {
        path: path.to_path_buf(),
        accepted,
        reason,
    });
    found
}

/// The helper must be a regular file.
fn check_vmm(path: &Path) -> Result<&'static str, &'static str> {
    match fs::metadata(path) {
        Ok(meta) if meta.is_file() => Ok("a regular file"),
        Ok(_) => Err("not a regular file"),
        Err(error) if error.kind() == ErrorKind::NotFound => Err("not found"),
        Err(_) => Err("could not be inspected"),
    }
}

/// The guest must be a regular file of at most [`GUEST_MAX_BYTES`] that is a static AArch64 ELF.
fn check_guest(path: &Path) -> Result<&'static str, &'static str> {
    // Checked before opening, so a FIFO or a device is never opened.
    match fs::metadata(path) {
        Ok(meta) if meta.is_file() => {}
        Ok(_) => return Err("not a regular file"),
        Err(error) if error.kind() == ErrorKind::NotFound => return Err("not found"),
        Err(_) => return Err("could not be inspected"),
    }
    let file = File::open(path).map_err(|_| "could not be opened")?;
    if !file.metadata().is_ok_and(|meta| meta.is_file()) {
        return Err("not a regular file");
    }
    let mut bytes = Vec::new();
    file.take(GUEST_MAX_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| "could not be read")?;
    if u64::try_from(bytes.len()).map_or(true, |len| len > GUEST_MAX_BYTES) {
        return Err("larger than 64 MiB");
    }
    check_guest_elf(&bytes)
        .map(|()| "a static AArch64 ELF")
        .map_err(elf_reason)
}

/// Why a guest candidate is not a static AArch64 ELF, as fixed text.
fn elf_reason(error: ElfError) -> &'static str {
    match error {
        ElfError::NotElf => "not an ELF file (the macOS build of minos-guest is Mach-O)",
        ElfError::Not64Bit => "not a 64-bit ELF",
        ElfError::NotLittleEndian => "not a little-endian ELF",
        ElfError::NotAarch64 => "an ELF not built for AArch64",
        ElfError::Dynamic => "dynamically linked (it has a PT_INTERP program header)",
        ElfError::Truncated => "a truncated ELF",
        _ => "not a static AArch64 ELF",
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::fs;
    use std::path::{Path, PathBuf};

    use minos_backend_krun::host::KRUNFW_PATH;

    use super::{GUEST_NAME, GUEST_TARGET, VMM_NAME, host_paths, locate, probe_paths};

    type TestResult = Result<(), Box<dyn Error>>;

    /// A minimal static AArch64 ELF header: 64-bit, little-endian, `EM_AARCH64`, no program headers.
    fn static_elf() -> Vec<u8> {
        let mut bytes = Vec::with_capacity(64);
        bytes.extend_from_slice(&[0x7f, b'E', b'L', b'F', 2, 1, 1, 0]);
        bytes.extend_from_slice(&[0; 8]);
        bytes.extend_from_slice(&2_u16.to_le_bytes()); // e_type ET_EXEC
        bytes.extend_from_slice(&183_u16.to_le_bytes()); // e_machine EM_AARCH64
        bytes.extend_from_slice(&1_u32.to_le_bytes()); // e_version
        bytes.extend_from_slice(&0x40_0000_u64.to_le_bytes()); // e_entry
        bytes.extend_from_slice(&0_u64.to_le_bytes()); // e_phoff
        bytes.extend_from_slice(&0_u64.to_le_bytes()); // e_shoff
        bytes.extend_from_slice(&0_u32.to_le_bytes()); // e_flags
        bytes.extend_from_slice(&64_u16.to_le_bytes()); // e_ehsize
        bytes.extend_from_slice(&56_u16.to_le_bytes()); // e_phentsize
        bytes.extend_from_slice(&0_u16.to_le_bytes()); // e_phnum
        bytes.extend_from_slice(&[0; 6]); // e_shentsize, e_shnum, e_shstrndx
        bytes
    }

    /// The first bytes of a 64-bit Mach-O file, as the macOS build of `minos-guest` starts.
    fn mach_o() -> Vec<u8> {
        let mut bytes = vec![0xcf, 0xfa, 0xed, 0xfe];
        bytes.resize(4096, 0);
        bytes
    }

    /// A canonical temporary directory (on macOS `/var` resolves to `/private/var`).
    fn temp() -> Result<(tempfile::TempDir, PathBuf), Box<dyn Error>> {
        let dir = tempfile::tempdir()?;
        let root = fs::canonicalize(dir.path())?;
        Ok((dir, root))
    }

    fn paths(located: &super::Located) -> Vec<&Path> {
        located.checked().iter().map(super::Candidate::path).collect()
    }

    #[test]
    fn locate_installed_layout() -> TestResult {
        let (_dir, root) = temp()?;
        let bin = root.join("bin");
        fs::create_dir(&bin)?;
        fs::write(bin.join("minos"), b"minos")?;
        fs::write(bin.join(VMM_NAME), b"vmm")?;
        fs::write(bin.join(GUEST_NAME), static_elf())?;

        let located = locate(&bin.join("minos"));

        assert_eq!(located.vmm(), Some(bin.join(VMM_NAME).as_path()));
        assert_eq!(located.guest(), Some(bin.join(GUEST_NAME).as_path()));
        assert_eq!(located.krunfw(), Path::new(KRUNFW_PATH));
        assert_eq!(
            paths(&located),
            [bin.join(VMM_NAME).as_path(), bin.join(GUEST_NAME).as_path()]
        );
        assert!(located.checked().iter().all(super::Candidate::is_accepted));
        let host = host_paths(&located).ok_or("both binaries were found")?;
        assert_eq!(host.vmm(), bin.join(VMM_NAME));
        assert_eq!(host.guest(), bin.join(GUEST_NAME));
        assert_eq!(host.krunfw(), Path::new(KRUNFW_PATH));
        Ok(())
    }

    #[test]
    fn locate_dev_checkout() -> TestResult {
        let (_dir, root) = temp()?;
        let debug = root.join("target").join("debug");
        let release = root.join("target").join(GUEST_TARGET).join("release");
        fs::create_dir_all(&debug)?;
        fs::create_dir_all(&release)?;
        fs::write(debug.join("minos"), b"minos")?;
        fs::write(debug.join(VMM_NAME), b"vmm")?;
        fs::write(debug.join(GUEST_NAME), mach_o())?;
        fs::write(release.join(GUEST_NAME), static_elf())?;

        let located = locate(&debug.join("minos"));

        assert_eq!(located.vmm(), Some(debug.join(VMM_NAME).as_path()));
        assert_eq!(located.guest(), Some(release.join(GUEST_NAME).as_path()));
        let rejected = located
            .checked()
            .iter()
            .find(|candidate| candidate.path() == debug.join(GUEST_NAME))
            .ok_or("the Mach-O guest is listed")?;
        assert!(!rejected.is_accepted());
        assert!(rejected.reason().contains("not an ELF"), "{}", rejected.reason());
        assert_eq!(
            paths(&located),
            [
                debug.join(VMM_NAME).as_path(),
                debug.join(GUEST_NAME).as_path(),
                release.join(GUEST_NAME).as_path(),
            ]
        );
        Ok(())
    }

    #[test]
    fn locate_missing() -> TestResult {
        let (_dir, root) = temp()?;
        let debug = root.join("target").join("debug");
        fs::create_dir_all(&debug)?;
        fs::write(debug.join("minos"), b"minos")?;

        let located = locate(&debug.join("minos"));

        assert_eq!(located.vmm(), None);
        assert_eq!(located.guest(), None);
        let musl = root
            .join("target")
            .join(GUEST_TARGET)
            .join("release")
            .join(GUEST_NAME);
        assert_eq!(
            paths(&located),
            [
                debug.join(VMM_NAME).as_path(),
                debug.join(GUEST_NAME).as_path(),
                musl.as_path(),
            ]
        );
        assert!(
            located
                .checked()
                .iter()
                .all(|candidate| !candidate.is_accepted() && candidate.reason() == "not found")
        );
        assert_eq!(host_paths(&located), None);
        let probe = probe_paths(&located);
        assert_eq!(probe.vmm(), debug.join(VMM_NAME));
        assert_eq!(probe.guest(), musl);
        Ok(())
    }

    #[test]
    fn locate_rejects_a_guest_that_is_not_a_regular_file() -> TestResult {
        let (_dir, root) = temp()?;
        let bin = root.join("bin");
        fs::create_dir(&bin)?;
        fs::create_dir(bin.join(GUEST_NAME))?;

        let located = locate(&bin.join("minos"));

        let guest = located
            .checked()
            .iter()
            .find(|candidate| candidate.path() == bin.join(GUEST_NAME))
            .ok_or("the guest directory is listed")?;
        assert_eq!(guest.reason(), "not a regular file");
        assert_eq!(located.guest(), None);
        Ok(())
    }
}
