//! Preflight: checks the environment and every host file a launch config names before the VMM sees any of it.
//!
//! Each file must already be in canonical form (so `/tmp` versus `/private/tmp` and symlinks are refused, and a
//! Seatbelt profile can name exact literals), and must be the expected file type. libkrunfw and the initramfs
//! are hashed last and compared to the config. The hash is taken from the canonical path immediately before the
//! VM is built; the window between hash and `dlopen` is accepted for the spike (T-02-19).

use std::fs::{self, File};
use std::io::{self, BufRead, BufReader};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::FileTypeExt;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::config::{Boot, STDOUT_CONSOLE, VmmConfig};

/// Environment variable name prefixes the launcher refuses to run under. `msb_krun` appends
/// `MSB_KRUN_KERNEL_CMDLINE` to the guest kernel command line, and `DYLD_` variables change what the loader maps
/// into the one entitled process.
const FORBIDDEN_ENV_PREFIXES: [&[u8]; 2] = [b"MSB_KRUN_", b"DYLD_"];

/// Read buffer for hashing, in bytes.
const HASH_BUFFER_BYTES: usize = 1 << 16;

/// Why preflight refused to launch.
#[derive(Debug, thiserror::Error)]
pub enum PreflightError {
    /// A forbidden environment variable is set. Only its name is reported.
    #[error("environment variable {name} is set; MSB_KRUN_* and DYLD_* variables are refused")]
    ForbiddenEnv {
        /// The variable name, lossily decoded.
        name: String,
    },
    /// A named file could not be resolved, inspected, or read.
    #[error("{field} {}", .path.display())]
    Inspect {
        /// The config field that names the file.
        field: &'static str,
        /// The configured path.
        path: PathBuf,
        /// The underlying error.
        #[source]
        source: io::Error,
    },
    /// A named path is not in canonical form.
    #[error("{field} {} is not canonical (it resolves to {})", .path.display(), .canonical.display())]
    NotCanonical {
        /// The config field that names the path.
        field: &'static str,
        /// The configured path.
        path: PathBuf,
        /// What `canonicalize` returned.
        canonical: PathBuf,
    },
    /// A named path is not a regular file.
    #[error("{field} {} is not a regular file", .path.display())]
    NotRegularFile {
        /// The config field that names the path.
        field: &'static str,
        /// The configured path.
        path: PathBuf,
    },
    /// The vsock path is not a Unix socket.
    #[error("vsock.socket {} is not a Unix socket", .path.display())]
    NotSocket {
        /// The configured path.
        path: PathBuf,
    },
    /// The console path already exists; the VMM must create it.
    #[error("console.path {} already exists", .path.display())]
    ConsoleExists {
        /// The configured path.
        path: PathBuf,
    },
    /// The directory that will hold the console file is missing or not a directory.
    #[error("console.path {} has no parent directory", .path.display())]
    ConsoleParent {
        /// The configured console path.
        path: PathBuf,
    },
    /// The scratch image is longer than its cap.
    #[error("disks.scratch.path is {len} bytes, over its cap of {cap} bytes")]
    ScratchTooLarge {
        /// Length of the scratch image.
        len: u64,
        /// The configured cap.
        cap: u64,
    },
    /// A file's SHA-256 does not match the config. `main` reports this with its own exit code (67).
    #[error("{field} {}: SHA-256 is {actual}, the config expects {expected}", .path.display())]
    Hash {
        /// The config field that names the file.
        field: &'static str,
        /// The configured path.
        path: PathBuf,
        /// Expected digest, lowercase hex.
        expected: String,
        /// Measured digest, lowercase hex.
        actual: String,
    },
}

/// Runs every preflight check in a fixed order: environment, files, socket, console, then hashes.
///
/// # Errors
///
/// The first failed check, as a [`PreflightError`]. A digest mismatch is [`PreflightError::Hash`].
pub fn check(config: &VmmConfig) -> Result<(), PreflightError> {
    check_environment(std::env::vars_os().map(|(name, _)| name))?;
    check_files(config)
}

/// Every file check after the environment: files, socket, console, then hashes.
fn check_files(config: &VmmConfig) -> Result<(), PreflightError> {
    regular_file("kernel.krunfw", config.krunfw())?;
    regular_file("disks.root", config.root())?;
    regular_file("disks.input", config.input())?;
    let scratch_len = regular_file("disks.scratch.path", config.scratch())?;
    if scratch_len > config.scratch_cap_bytes() {
        return Err(PreflightError::ScratchTooLarge {
            len: scratch_len,
            cap: config.scratch_cap_bytes(),
        });
    }
    if let Boot::Initramfs { path, .. } = config.boot() {
        regular_file("boot.initramfs.path", path)?;
    }
    unix_socket(config.socket())?;
    console_ok(config.console())?;
    hash_matches("kernel.krunfw", config.krunfw(), config.krunfw_sha256())?;
    if let Boot::Initramfs { path, sha256 } = config.boot() {
        hash_matches("boot.initramfs.path", path, sha256)?;
    }
    Ok(())
}

/// Refuses the first variable name that starts with a forbidden prefix.
fn check_environment(
    names: impl IntoIterator<Item = std::ffi::OsString>,
) -> Result<(), PreflightError> {
    for name in names {
        let bytes = name.as_bytes();
        if FORBIDDEN_ENV_PREFIXES
            .iter()
            .any(|prefix| bytes.starts_with(prefix))
        {
            return Err(PreflightError::ForbiddenEnv {
                name: name.to_string_lossy().into_owned(),
            });
        }
    }
    Ok(())
}

fn inspect(field: &'static str, path: &Path) -> impl FnOnce(io::Error) -> PreflightError {
    move |source| PreflightError::Inspect {
        field,
        path: path.to_path_buf(),
        source,
    }
}

/// `canonicalize(path)` must return `path` itself.
fn canonical(field: &'static str, path: &Path) -> Result<(), PreflightError> {
    let canonical = fs::canonicalize(path).map_err(inspect(field, path))?;
    if canonical == path {
        Ok(())
    } else {
        Err(PreflightError::NotCanonical {
            field,
            path: path.to_path_buf(),
            canonical,
        })
    }
}

/// A canonical regular file. Returns its length.
fn regular_file(field: &'static str, path: &Path) -> Result<u64, PreflightError> {
    canonical(field, path)?;
    let metadata = fs::symlink_metadata(path).map_err(inspect(field, path))?;
    if metadata.file_type().is_file() {
        Ok(metadata.len())
    } else {
        Err(PreflightError::NotRegularFile {
            field,
            path: path.to_path_buf(),
        })
    }
}

/// The host end of the vsock route: an existing, canonical Unix socket that the parent bound.
fn unix_socket(path: &Path) -> Result<(), PreflightError> {
    canonical("vsock.socket", path)?;
    let metadata = fs::symlink_metadata(path).map_err(inspect("vsock.socket", path))?;
    if metadata.file_type().is_socket() {
        Ok(())
    } else {
        Err(PreflightError::NotSocket {
            path: path.to_path_buf(),
        })
    }
}

/// The console is either the helper's own stdout, named by exactly [`STDOUT_CONSOLE`], or a file that does not
/// exist yet in a canonical directory. The stdout literal is compared byte for byte, so `/dev/stdout`, another
/// descriptor, or any other existing path is still refused as an existing console.
fn console_ok(path: &Path) -> Result<(), PreflightError> {
    if path.as_os_str().as_bytes() == STDOUT_CONSOLE.as_bytes() {
        Ok(())
    } else {
        console_absent(path)
    }
}

/// The console file must not exist yet, and its parent must be a canonical directory.
fn console_absent(path: &Path) -> Result<(), PreflightError> {
    match fs::symlink_metadata(path) {
        Ok(_) => {
            return Err(PreflightError::ConsoleExists {
                path: path.to_path_buf(),
            });
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(inspect("console.path", path)(error)),
    }
    let parent = path.parent().ok_or_else(|| PreflightError::ConsoleParent {
        path: path.to_path_buf(),
    })?;
    canonical("console.path", parent)?;
    let metadata = fs::symlink_metadata(parent).map_err(inspect("console.path", parent))?;
    if metadata.is_dir() {
        Ok(())
    } else {
        Err(PreflightError::ConsoleParent {
            path: path.to_path_buf(),
        })
    }
}

/// SHA-256 of the file at `path` over a buffered read, compared to `expected`.
fn hash_matches(
    field: &'static str,
    path: &Path,
    expected: &[u8; 32],
) -> Result<(), PreflightError> {
    let file = File::open(path).map_err(inspect(field, path))?;
    let mut reader = BufReader::with_capacity(HASH_BUFFER_BYTES, file);
    let mut hasher = Sha256::new();
    loop {
        let chunk = match reader.fill_buf() {
            Ok(chunk) => chunk,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(inspect(field, path)(error)),
        };
        if chunk.is_empty() {
            break;
        }
        hasher.update(chunk);
        let used = chunk.len();
        reader.consume(used);
    }
    let actual: [u8; 32] = hasher.finalize().into();
    if &actual == expected {
        Ok(())
    } else {
        Err(PreflightError::Hash {
            field,
            path: path.to_path_buf(),
            expected: base16ct::lower::encode_string(expected),
            actual: base16ct::lower::encode_string(&actual),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::ffi::OsString;
    use std::path::Path;

    use super::{PreflightError, check_environment, check_files, console_ok, regular_file};
    use crate::config;

    type TestResult = Result<(), Box<dyn Error>>;

    #[test]
    fn preflight_refuses_msb_krun_and_dyld_variables() {
        for name in [
            "MSB_KRUN_KERNEL_CMDLINE",
            "MSB_KRUN_BOOT_TRACE",
            "DYLD_INSERT_LIBRARIES",
        ] {
            let names = [OsString::from("PATH"), OsString::from(name)];
            let refused = check_environment(names);
            assert!(
                matches!(&refused, Err(PreflightError::ForbiddenEnv { name: found }) if found == name),
                "{name} was not refused"
            );
        }
    }

    #[test]
    fn preflight_allows_ordinary_variables() -> TestResult {
        let names = ["PATH", "HOME", "MSB_KRUNX", "XDYLD_PATH"].map(OsString::from);
        check_environment(names)?;
        Ok(())
    }

    #[test]
    fn preflight_refuses_a_directory_as_a_file() {
        let refused = regular_file("disks.root", Path::new("/"));
        assert!(matches!(
            refused,
            Err(PreflightError::NotRegularFile {
                field: "disks.root",
                ..
            })
        ));
    }

    #[test]
    fn preflight_refuses_a_missing_file() {
        let refused = regular_file("disks.root", Path::new("/private/tmp/mns00000000/root.img"));
        assert!(matches!(
            refused,
            Err(PreflightError::Inspect {
                field: "disks.root",
                ..
            })
        ));
    }

    /// A v1 config with every host path under a job directory that does not exist, and console `/dev/fd/1`.
    const STDOUT_CONSOLE_CONFIG: &str = r#"{"v":1,"kernel":{"krunfw":"/private/tmp/mns00000000/libkrunfw.5.dylib","sha256":"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"},"boot":"root_disk","disks":{"root":"/private/tmp/mns00000000/root.img","input":"/private/tmp/mns00000000/input.img","scratch":{"path":"/private/tmp/mns00000000/scratch.img","cap_bytes":67108864}},"vsock":{"port":1024,"socket":"/private/tmp/mns00000000/o.sock"},"console":{"path":"/dev/fd/1","cap_bytes":1048576},"max_duration_ms":60000,"machine":{"vcpus":1,"memory_mib":256}}"#;

    /// The stdout literal skips only the console-absent and parent checks; every other file is still checked.
    #[test]
    fn preflight_allows_stdout_console() -> TestResult {
        console_ok(Path::new("/dev/fd/1"))?;
        let config = config::parse(STDOUT_CONSOLE_CONFIG.as_bytes())?;
        let refused = check_files(&config);
        assert!(
            matches!(
                refused,
                Err(PreflightError::Inspect {
                    field: "kernel.krunfw",
                    ..
                })
            ),
            "a missing libkrunfw was not refused"
        );
        Ok(())
    }

    /// Only the exact literal `/dev/fd/1` is the helper's stdout. Other descriptors and the `/dev/stdout`
    /// spelling, and any existing file, are still refused as an existing console.
    #[test]
    fn preflight_still_refuses_existing_console_file() -> TestResult {
        let this_binary = std::env::current_exe()?;
        for path in [
            Path::new("/dev/stdout"),
            Path::new("/dev/fd/2"),
            Path::new("/dev/fd/0"),
            this_binary.as_path(),
        ] {
            let refused = console_ok(path);
            assert!(
                matches!(&refused, Err(PreflightError::ConsoleExists { path: found }) if found == path),
                "{} was not refused as an existing console",
                path.display()
            );
        }
        Ok(())
    }
}
