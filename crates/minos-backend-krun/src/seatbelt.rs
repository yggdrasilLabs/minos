//! The Seatbelt profile that confines every `minos-vmm` helper, and the `sandbox-exec` argv that applies it.
//!
//! Seatbelt is mandatory in v1 (05-CONTEXT D-03): macOS has no seccomp and no cgroups, so the deny-default
//! profile in `minos-vmm.sb` is the layer between a VMM escape and the user's files and network. The profile is
//! part of the trusted binary. It is embedded with `include_str!`, passed to `sandbox-exec` as text with `-p`,
//! and its SHA-256 is pinned by a test, so there is no profile file on disk for anything to edit between the
//! build and the launch (05-RESEARCH Don't Hand-Roll; threat T-05-09-01).
//!
//! Every job path reaches the profile as a `-D NAME=value` parameter. [`ProfileParams`] holds exactly the
//! parameters the profile names, each an absolute path, and a test proves the two sets are equal, so no rule can
//! refer to a parameter that the argv leaves unset (T-05-09-02). Callers pass canonical paths (`/private/tmp`,
//! never `/tmp`), because Seatbelt compares a rule's literal with the resolved path.

use std::ffi::{OsStr, OsString};
use std::path::{Component, Path, PathBuf};

use sha2::{Digest, Sha256};

/// The helper's deny-default Seatbelt profile, as compiled into this binary.
pub const PROFILE: &str = include_str!("minos-vmm.sb");

/// The only `sandbox-exec` the backend runs, by absolute path.
pub const SANDBOX_EXEC: &str = "/usr/bin/sandbox-exec";

/// SHA-256 of [`PROFILE`], as 64 lowercase hex characters.
#[must_use]
pub fn profile_sha256() -> String {
    String::new()
}

/// Why a set of profile parameters was refused.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SeatbeltError {
    /// A parameter is a relative path, which no Seatbelt rule can match the way the backend means.
    #[error("Seatbelt parameter {name} is not an absolute path")]
    RelativePath {
        /// The parameter name, as the profile spells it.
        name: &'static str,
    },
    /// A parameter has a `..` component, so it is not the canonical path the profile rule compares against.
    #[error("Seatbelt parameter {name} has a parent-directory component")]
    ParentComponent {
        /// The parameter name, as the profile spells it.
        name: &'static str,
    },
}

/// The values of every parameter [`PROFILE`] names, each an absolute path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileParams {
    vmm: PathBuf,
    config: PathBuf,
    krunfw: PathBuf,
    krunfw_link: PathBuf,
    root_img: PathBuf,
    input_img: PathBuf,
    sock: PathBuf,
}

impl ProfileParams {
    /// Checks and holds the profile parameters: the helper binary (`VMM`), its launch config (`CONFIG`), the
    /// libkrunfw dylib it loads (`KRUNFW`) and the link it is named by (`KRUNFW_LINK`), the root and input disk
    /// images (`ROOT_IMG`, `INPUT_IMG`), and the vsock route socket (`SOCK`).
    ///
    /// # Errors
    ///
    /// [`SeatbeltError::RelativePath`] when a path is not absolute, and [`SeatbeltError::ParentComponent`] when
    /// one has a `..` component, naming the first such parameter in the argument order.
    pub fn new(
        vmm: &Path,
        config: &Path,
        krunfw: &Path,
        krunfw_link: &Path,
        root_img: &Path,
        input_img: &Path,
        sock: &Path,
    ) -> Result<Self, SeatbeltError> {
        Ok(Self {
            vmm: checked("VMM", vmm)?,
            config: checked("CONFIG", config)?,
            krunfw: checked("KRUNFW", krunfw)?,
            krunfw_link: checked("KRUNFW_LINK", krunfw_link)?,
            root_img: checked("ROOT_IMG", root_img)?,
            input_img: checked("INPUT_IMG", input_img)?,
            sock: checked("SOCK", sock)?,
        })
    }

    /// Every parameter as `(name, value)`, in ascending name order.
    fn pairs(&self) -> [(&'static str, &Path); 7] {
        [
            ("CONFIG", &self.config),
            ("INPUT_IMG", &self.input_img),
            ("KRUNFW", &self.krunfw),
            ("KRUNFW_LINK", &self.krunfw_link),
            ("ROOT_IMG", &self.root_img),
            ("SOCK", &self.sock),
            ("VMM", &self.vmm),
        ]
    }
}

/// Refuses a relative path or one with a `..` component.
fn checked(name: &'static str, path: &Path) -> Result<PathBuf, SeatbeltError> {
    if !path.is_absolute() {
        return Err(SeatbeltError::RelativePath { name });
    }
    if path.components().any(|part| part == Component::ParentDir) {
        return Err(SeatbeltError::ParentComponent { name });
    }
    Ok(path.to_path_buf())
}

/// The argv that runs `program` with `args` under [`PROFILE`]: [`SANDBOX_EXEC`], `-p`, the profile text, one
/// `-D NAME=value` pair per parameter in ascending name order, then `program` and `args`.
#[must_use]
pub fn argv(params: &ProfileParams, program: &Path, args: &[&OsStr]) -> Vec<OsString> {
    let _ = (params, program, args);
    Vec::new()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::error::Error;
    use std::ffi::{OsStr, OsString};
    use std::path::Path;

    use super::{PROFILE, ProfileParams, SANDBOX_EXEC, SeatbeltError, argv, profile_sha256};

    type TestResult = Result<(), Box<dyn Error>>;

    /// Recompute after any profile edit with `shasum -a 256 crates/minos-backend-krun/src/minos-vmm.sb`.
    const PROFILE_SHA256: &str = "0000000000000000000000000000000000000000000000000000000000000000";

    fn sample() -> Result<ProfileParams, SeatbeltError> {
        ProfileParams::new(
            Path::new("/opt/minos/bin/minos-vmm"),
            Path::new("/private/tmp/job/vmm.json"),
            Path::new("/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib"),
            Path::new("/opt/homebrew/lib/libkrunfw.5.dylib"),
            Path::new("/Users/example/project/.minos/blobs/sha256/ab/cdef"),
            Path::new("/private/tmp/job/input.img"),
            Path::new("/private/tmp/job/o.sock"),
        )
    }

    /// Every name in a `(param "NAME")` form of the profile, comments included.
    fn profile_param_names() -> BTreeSet<String> {
        const OPEN: &str = "(param \"";
        let mut names = BTreeSet::new();
        let mut rest = PROFILE;
        while let Some((_, after)) = rest.split_once(OPEN) {
            match after.split_once('"') {
                Some((name, tail)) => {
                    names.insert(name.to_owned());
                    rest = tail;
                }
                None => break,
            }
        }
        names
    }

    /// The `NAME` of every `-D NAME=value` pair in `argv`.
    fn argv_param_names(argv: &[OsString]) -> Result<Vec<String>, Box<dyn Error>> {
        let mut names = Vec::new();
        let mut items = argv.iter();
        while let Some(item) = items.next() {
            if item == "-D" {
                let pair = items.next().ok_or("-D without a value")?;
                let pair = pair.to_str().ok_or("-D value is not UTF-8")?;
                let (name, _) = pair.split_once('=').ok_or("-D value has no =")?;
                names.push(name.to_owned());
            }
        }
        Ok(names)
    }

    #[test]
    fn profile_params_match_argv() -> TestResult {
        let line = argv(&sample()?, Path::new("/opt/minos/bin/minos-vmm"), &[]);
        let from_argv: BTreeSet<String> = argv_param_names(&line)?.into_iter().collect();
        let from_profile = profile_param_names();
        assert!(!from_profile.is_empty());
        assert_eq!(from_argv, from_profile);
        assert!(!from_profile.contains("SCRATCH_IMG"));
        assert!(!from_profile.contains("CONSOLE_LOG"));
        Ok(())
    }

    #[test]
    fn profile_is_pinned() {
        assert_eq!(profile_sha256(), PROFILE_SHA256);
        assert_eq!(profile_sha256().len(), 64);
    }

    #[test]
    fn profile_network_is_socket_only() {
        let rules: Vec<&str> = PROFILE
            .lines()
            .map(str::trim)
            .filter(|line| !line.starts_with(';') && line.contains("network"))
            .collect();
        assert_eq!(
            rules,
            ["(allow network-outbound (literal (param \"SOCK\")))"]
        );
        assert_eq!(PROFILE.matches("network-outbound").count(), 1);
        assert!(PROFILE.lines().any(|line| line == "(deny default)"));
        assert!(!PROFILE.contains("(allow default"));
    }

    #[test]
    fn argv_shape() -> TestResult {
        let program = Path::new("/opt/minos/bin/minos-vmm");
        let config = OsStr::new("/private/tmp/job/vmm.json");
        let line = argv(&sample()?, program, &[config]);
        let mut items = line.iter();
        assert_eq!(items.next().map(OsString::as_os_str), Some(OsStr::new(SANDBOX_EXEC)));
        assert_eq!(items.next().map(OsString::as_os_str), Some(OsStr::new("-p")));
        assert_eq!(items.next().map(OsString::as_os_str), Some(OsStr::new(PROFILE)));
        let expected = [
            "CONFIG=/private/tmp/job/vmm.json",
            "INPUT_IMG=/private/tmp/job/input.img",
            "KRUNFW=/opt/homebrew/Cellar/libkrunfw/5.6.2/lib/libkrunfw.5.dylib",
            "KRUNFW_LINK=/opt/homebrew/lib/libkrunfw.5.dylib",
            "ROOT_IMG=/Users/example/project/.minos/blobs/sha256/ab/cdef",
            "SOCK=/private/tmp/job/o.sock",
            "VMM=/opt/minos/bin/minos-vmm",
        ];
        for pair in expected {
            assert_eq!(items.next().map(OsString::as_os_str), Some(OsStr::new("-D")));
            assert_eq!(items.next().map(OsString::as_os_str), Some(OsStr::new(pair)));
        }
        assert_eq!(items.next().map(OsString::as_os_str), Some(program.as_os_str()));
        assert_eq!(items.next().map(OsString::as_os_str), Some(config));
        assert_eq!(items.next(), None);
        let names = argv_param_names(&line)?;
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
        Ok(())
    }

    #[test]
    fn params_must_be_absolute() {
        let abs = Path::new("/private/tmp/job/x");
        let rel = Path::new("job/x");
        let cases: [(&str, [&Path; 7]); 7] = [
            ("VMM", [rel, abs, abs, abs, abs, abs, abs]),
            ("CONFIG", [abs, rel, abs, abs, abs, abs, abs]),
            ("KRUNFW", [abs, abs, rel, abs, abs, abs, abs]),
            ("KRUNFW_LINK", [abs, abs, abs, rel, abs, abs, abs]),
            ("ROOT_IMG", [abs, abs, abs, abs, rel, abs, abs]),
            ("INPUT_IMG", [abs, abs, abs, abs, abs, rel, abs]),
            ("SOCK", [abs, abs, abs, abs, abs, abs, rel]),
        ];
        for (expected, [a, b, c, d, e, f, g]) in cases {
            let refused = ProfileParams::new(a, b, c, d, e, f, g);
            assert!(
                matches!(refused, Err(SeatbeltError::RelativePath { name }) if name == expected),
                "{expected}"
            );
        }
        let empty = ProfileParams::new(Path::new(""), abs, abs, abs, abs, abs, abs);
        assert!(matches!(empty, Err(SeatbeltError::RelativePath { name: "VMM" })));
        let dotted = Path::new("/private/tmp/job/../other/o.sock");
        let parent = ProfileParams::new(abs, abs, abs, abs, abs, abs, dotted);
        assert!(matches!(parent, Err(SeatbeltError::ParentComponent { name: "SOCK" })));
        assert!(ProfileParams::new(abs, abs, abs, abs, abs, abs, abs).is_ok());
    }
}
