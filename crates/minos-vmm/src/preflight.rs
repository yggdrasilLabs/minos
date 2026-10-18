//! Preflight: checks the environment and every host file a launch config names before the VMM sees any of it.
//!
//! Each file must already be in canonical form (so `/tmp` versus `/private/tmp` and symlinks are refused, and a
//! Seatbelt profile can name exact literals), and must be the expected file type. The console is the helper's own
//! stdout, so fd 1 must be a pipe the supervisor reads. libkrunfw is hashed last and compared to the config. The
//! hash is taken from the canonical path immediately before the VM is built; the window between hash and
//! `dlopen` is accepted for now (T-02-19; CLAUDE.md Open questions).

use std::fs::{self, File};
use std::io::{self, BufRead, BufReader};
use std::os::fd::AsFd;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::FileTypeExt;
use std::path::{Path, PathBuf};

use rustix::fs::FileType;
use sha2::{Digest, Sha256};

use crate::config::{STDOUT_CONSOLE, VmmConfig};

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
    /// fd 1, the guest console, is not a pipe. The supervisor gives the helper a pipe and reads it through its cap;
    /// anything else would write the console to a terminal or a file.
    #[error(
        "stdout ({STDOUT_CONSOLE}) is a {found}, not a pipe; the console must be the supervisor's pipe"
    )]
    ConsoleNotPipe {
        /// What fd 1 is instead.
        found: &'static str,
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

/// Runs every preflight check in a fixed order: environment, console pipe, files, socket, then the hash.
///
/// # Errors
///
/// The first failed check, as a [`PreflightError`]. A digest mismatch is [`PreflightError::Hash`].
pub fn check(config: &VmmConfig) -> Result<(), PreflightError> {
    check_environment(std::env::vars_os().map(|(name, _)| name))?;
    stdout_is_pipe()?;
    check_files(config)
}

/// Every file check after the environment and the console: files, socket, then the hash.
fn check_files(config: &VmmConfig) -> Result<(), PreflightError> {
    regular_file("kernel.krunfw", config.krunfw())?;
    regular_file("disks.root", config.root())?;
    regular_file("disks.input", config.input())?;
    unix_socket(config.socket())?;
    hash_matches("kernel.krunfw", config.krunfw(), config.krunfw_sha256())
}

/// fd 1, which the VMM opens as [`STDOUT_CONSOLE`], must be a pipe (`fstat` file type FIFO).
fn stdout_is_pipe() -> Result<(), PreflightError> {
    let stat =
        rustix::fs::fstat(io::stdout().as_fd()).map_err(|errno| PreflightError::Inspect {
            field: "console",
            path: PathBuf::from(STDOUT_CONSOLE),
            source: io::Error::from(errno),
        })?;
    console_pipe_ok(FileType::from_raw_mode(stat.st_mode))
}

/// The decision behind [`stdout_is_pipe`], on the file type `fstat` reported: only a FIFO passes.
fn console_pipe_ok(file_type: FileType) -> Result<(), PreflightError> {
    let found = match file_type {
        FileType::Fifo => return Ok(()),
        FileType::RegularFile => "regular file",
        FileType::Directory => "directory",
        FileType::Symlink => "symbolic link",
        FileType::Socket => "socket",
        FileType::CharacterDevice => "character device",
        FileType::BlockDevice => "block device",
        FileType::Unknown => "file of unknown type",
    };
    Err(PreflightError::ConsoleNotPipe { found })
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

/// A canonical regular file.
fn regular_file(field: &'static str, path: &Path) -> Result<(), PreflightError> {
    canonical(field, path)?;
    let metadata = fs::symlink_metadata(path).map_err(inspect(field, path))?;
    if metadata.file_type().is_file() {
        Ok(())
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
    use std::fs::File;
    use std::path::Path;

    use rustix::fs::FileType;

    use super::{PreflightError, check_environment, check_files, console_pipe_ok, regular_file};
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

    /// Only a FIFO passes as the console; every other file type `fstat` can report is refused by name.
    #[test]
    fn preflight_requires_stdout_pipe() -> TestResult {
        console_pipe_ok(FileType::Fifo)?;
        for (file_type, expected) in [
            (FileType::RegularFile, "regular file"),
            (FileType::Directory, "directory"),
            (FileType::Symlink, "symbolic link"),
            (FileType::Socket, "socket"),
            (FileType::CharacterDevice, "character device"),
            (FileType::BlockDevice, "block device"),
            (FileType::Unknown, "file of unknown type"),
        ] {
            let refused = console_pipe_ok(file_type);
            assert!(
                matches!(&refused, Err(PreflightError::ConsoleNotPipe { found }) if *found == expected),
                "{file_type:?} was not refused as {expected}"
            );
        }
        Ok(())
    }

    /// The `fstat` mapping on this host: the write end of an anonymous pipe is a FIFO and passes, and an open
    /// regular file is refused.
    #[test]
    fn preflight_stdout_pipe_mapping_matches_fstat() -> TestResult {
        let (_reader, writer) = std::io::pipe()?;
        let stat = rustix::fs::fstat(&writer)?;
        console_pipe_ok(FileType::from_raw_mode(stat.st_mode))?;

        let file = File::open(std::env::current_exe()?)?;
        let stat = rustix::fs::fstat(&file)?;
        let refused = console_pipe_ok(FileType::from_raw_mode(stat.st_mode));
        assert!(matches!(
            refused,
            Err(PreflightError::ConsoleNotPipe {
                found: "regular file"
            })
        ));
        Ok(())
    }

    /// A v2 config whose libkrunfw path does not exist is refused at the first file check, before the disks.
    #[test]
    fn preflight_checks_the_files_of_a_v2_config() -> TestResult {
        let golden = include_str!("../golden/config-v2.json");
        let installed = "/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib";
        if !golden.contains(installed) {
            return Err("the golden config does not name the pinned libkrunfw path".into());
        }
        let text = golden.replacen(installed, "/private/tmp/mns00000000/libkrunfw.5.dylib", 1);
        let config = config::parse(text.as_bytes())?;
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
}
