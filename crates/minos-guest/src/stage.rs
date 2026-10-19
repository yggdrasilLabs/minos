//! Who a stage runs as, where, with what environment, and under which limits (GUEST-03; 05-CONTEXT D-05, D-08).
//!
//! **One identity.** Every stage, build and run alike, runs as one fixed unprivileged uid and gid,
//! [`GUEST_UID`] and [`GUEST_GID`] (D-08). Build and run share it because build output must be runnable. The guest
//! needs no `/etc/passwd`: PID 1 passes the numbers straight to `Command::uid` and `Command::gid`, so the user's
//! image supplies no account, and the Dockerfile `USER` is ignored. PID 1 and the output packer stay root, and the
//! candidate's processes are all killed before packing, so a candidate cannot race the packer.
//!
//! **The trampoline.** Setting rlimits between `fork` and `exec` needs `CommandExt::pre_exec`, which is `unsafe`
//! and forbidden here. Instead PID 1 spawns `/init --minos-stage <limits> -- <argv>` as the stage uid, and that
//! process, already unprivileged, lowers its own limits and then `exec`s the command; limits survive `exec`
//! (05-RESEARCH Pattern 5). [`trampoline_argv`] writes that argv and [`parse_trampoline`] reads it back strictly.
//! A candidate can run the trampoline too, but only as itself, with limits it may only lower; the Linux side
//! (plan 05-12) also refuses to run it as PID 1 or as root.
//!
//! **The limits** follow the D-05 memory budget, the VM's memory being the guest reserve plus the tmpfs scratch
//! cap plus the candidate's allowance:
//!
//! - `RLIMIT_AS` is the candidate allowance. Runtimes that reserve large virtual ranges (Go, the JVM, V8,
//!   sanitizers) can fail under it (05-RESEARCH Pitfall 10); the VM's memory is the real bound, tmpfs pages count
//!   against it, and the defaults are generous and manifest-tunable.
//! - `RLIMIT_FSIZE` is the scratch cap: no single file can be larger than the whole scratch mount.
//! - `RLIMIT_CPU` is the stage budget in whole seconds plus one, multiplied by the VM's vCPU count, so the
//!   kernel's `SIGXCPU` backs up PID 1's own wall-clock deadline without ever firing before it. `RLIMIT_CPU`
//!   counts the CPU time of every thread of a process, so a process that keeps every vCPU busy for the whole
//!   budget uses `budget * vcpus` of CPU time; an unscaled limit would kill a parallel build with signal 24
//!   before the deadline and blame the candidate (review CR-01). PID 1 reads the count from the kernel.
//! - `RLIMIT_NPROC` is [`MAX_PROCESSES`] and `RLIMIT_NOFILE` is [`MAX_OPEN_FILES`].
//! - `RLIMIT_CORE` is always 0, so a crashing candidate writes no core file into scratch; it is not a field.

use std::ffi::{OsStr, OsString};
use std::path::Path;

use minos_core::guest::{GuestJob, StageSpec};
use minos_core::limits::{MAX_MEMORY_MIB, MAX_SCRATCH_MIB, MAX_STAGE_MS, MAX_VCPUS};

/// The uid every stage runs as (D-08). Far above the ranges distributions give system and login accounts, so it
/// names no account in any image.
pub const GUEST_UID: u32 = 64_000;

/// The gid every stage runs as (D-08), equal to [`GUEST_UID`].
pub const GUEST_GID: u32 = 64_000;

/// Where PID 1 mounts the size-capped tmpfs scratch (D-05).
pub const SCRATCH_MOUNT: &str = "/scratch";

/// Where a candidate's base tree is unpacked and its diff applied: the build and run working directory.
pub const SRC_DIR: &str = "/scratch/src";

/// The stage's `HOME`.
pub const HOME_DIR: &str = "/scratch/home";

/// The scratch directory bind-mounted on [`TMP_MOUNT`].
pub const SCRATCH_TMP_DIR: &str = "/scratch/tmp";

/// The scratch directory bind-mounted on [`OUT_MOUNT`].
pub const SCRATCH_OUT_DIR: &str = "/scratch/out";

/// Where a judge's private data is unpacked.
pub const JUDGE_DIR: &str = "/scratch/judge";

/// Where a judge finds the candidate's output.
pub const CANDIDATE_DIR: &str = "/scratch/candidate";

/// The stage's temporary directory, backed by [`SCRATCH_TMP_DIR`].
pub const TMP_MOUNT: &str = "/tmp";

/// Where a stage writes its output, backed by [`SCRATCH_OUT_DIR`].
pub const OUT_MOUNT: &str = "/out";

/// The whole environment of every stage. PID 1 clears its environment before applying this, so no host value
/// ever enters a stage (GUEST-05).
pub const STAGE_ENV: [(&str, &str); 4] = [
    (
        "PATH",
        "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
    ),
    ("HOME", HOME_DIR),
    ("TMPDIR", TMP_MOUNT),
    ("LANG", "C"),
];

/// `RLIMIT_NPROC` for every stage: enough for a parallel build's compilers and linkers, and a bound on a fork
/// bomb. A recorded assumption (05-RESEARCH A6), to be revisited if real builds hit it.
pub const MAX_PROCESSES: u64 = 512;

/// `RLIMIT_NOFILE` for every stage: room for a build's open sources, objects, and pipes. A recorded assumption
/// (05-RESEARCH A6), to be revisited if real builds hit it.
pub const MAX_OPEN_FILES: u64 = 4096;

/// Bytes in a MiB.
const MIB: u64 = 1_048_576;

/// The largest address-space limit the trampoline accepts: the largest VM memory.
const MAX_ADDRESS_SPACE_BYTES: u64 = MAX_MEMORY_MIB as u64 * MIB;

/// The largest file-size limit the trampoline accepts: the largest scratch cap.
const MAX_FILE_SIZE_BYTES: u64 = MAX_SCRATCH_MIB as u64 * MIB;

/// The largest CPU limit the trampoline accepts: the longest stage budget in seconds, plus one, on every vCPU
/// a sandbox may have.
const MAX_CPU_SECONDS: u64 = (MAX_STAGE_MS.div_ceil(1000) + 1) * MAX_VCPUS as u64;

/// The rlimits of one stage, derived from the job by [`StageLimits::for_stage`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StageLimits {
    address_space_bytes: u64,
    file_size_bytes: u64,
    processes: u64,
    cpu_seconds: u64,
    open_files: u64,
}

impl StageLimits {
    /// The limits for `stage` of `job` on a VM of `vcpus` vCPUs: the candidate allowance as the address space,
    /// the scratch cap as the file size, the stage budget in whole seconds plus one, times the vCPU count, as
    /// the CPU time, and [`MAX_PROCESSES`] and [`MAX_OPEN_FILES`].
    ///
    /// `vcpus` is clamped to 1 to `MAX_VCPUS`, so the CPU limit never exceeds the trampoline's cap.
    #[must_use]
    pub fn for_stage(job: &GuestJob, stage: &StageSpec, vcpus: u8) -> Self {
        let wall_seconds = stage.budget_ms().div_ceil(1000).saturating_add(1);
        let vcpus = u64::from(vcpus.clamp(1, MAX_VCPUS));
        Self {
            address_space_bytes: u64::from(job.allowance_mib()) * MIB,
            file_size_bytes: u64::from(job.scratch_mib()) * MIB,
            processes: MAX_PROCESSES,
            // Every thread of the process may run on its own vCPU for the whole budget.
            cpu_seconds: wall_seconds.saturating_mul(vcpus),
            open_files: MAX_OPEN_FILES,
        }
    }

    /// `RLIMIT_AS`, in bytes.
    #[must_use]
    pub const fn address_space_bytes(&self) -> u64 {
        self.address_space_bytes
    }

    /// `RLIMIT_FSIZE`, in bytes.
    #[must_use]
    pub const fn file_size_bytes(&self) -> u64 {
        self.file_size_bytes
    }

    /// `RLIMIT_NPROC`.
    #[must_use]
    pub const fn processes(&self) -> u64 {
        self.processes
    }

    /// `RLIMIT_CPU`, in seconds.
    #[must_use]
    pub const fn cpu_seconds(&self) -> u64 {
        self.cpu_seconds
    }

    /// `RLIMIT_NOFILE`.
    #[must_use]
    pub const fn open_files(&self) -> u64 {
        self.open_files
    }

    /// The five limits in trampoline order, each with its cap.
    const fn with_caps(&self) -> [(u64, u64); 5] {
        [
            (self.address_space_bytes, MAX_ADDRESS_SPACE_BYTES),
            (self.file_size_bytes, MAX_FILE_SIZE_BYTES),
            (self.processes, MAX_PROCESSES),
            (self.cpu_seconds, MAX_CPU_SECONDS),
            (self.open_files, MAX_OPEN_FILES),
        ]
    }
}

/// The mount data of the scratch tmpfs for a job whose scratch cap is `scratch_mib`: its size cap and a root
/// directory of mode 0755, so only PID 1 creates entries at the top of [`SCRATCH_MOUNT`] (D-05).
#[must_use]
pub fn scratch_mount_data(scratch_mib: u16) -> String {
    format!("size={scratch_mib}m,mode=0755")
}

/// The flag that makes `/init` act as the stage trampoline instead of PID 1.
pub const TRAMPOLINE_FLAG: &str = "--minos-stage";

/// The argument that ends the limits and starts the command.
const SEPARATOR: &str = "--";

/// The argv PID 1 spawns a stage with: `[init, "--minos-stage", as, fsize, nproc, cpu, nofile, "--", argv...]`,
/// each limit in plain decimal.
#[must_use]
pub fn trampoline_argv(init: &Path, limits: &StageLimits, argv: &[String]) -> Vec<OsString> {
    let mut out = Vec::with_capacity(argv.len().saturating_add(8));
    out.push(init.as_os_str().to_owned());
    out.push(OsString::from(TRAMPOLINE_FLAG));
    out.extend(
        limits
            .with_caps()
            .into_iter()
            .map(|(value, _)| OsString::from(value.to_string())),
    );
    out.push(OsString::from(SEPARATOR));
    out.extend(argv.iter().map(OsString::from));
    out
}

/// Why a trampoline argv is not the one [`trampoline_argv`] writes.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TrampolineError {
    /// The argv is not `init`, the flag, five limits, `--`, and a command: the flag or the separator is missing
    /// or misplaced, or arguments are missing or extra before the separator.
    #[error("the trampoline argv is not the flag, five limits, the separator, and a command")]
    Layout,
    /// The limit at argv index `position` is not plain decimal ASCII digits without a leading zero, or is 0, or
    /// is above its cap.
    #[error("trampoline argument {position} is not a limit within its cap")]
    Number {
        /// The limit's index in the argv, from 2 to 6.
        position: u8,
    },
    /// No command follows the separator, or its program is empty.
    #[error("the trampoline has no command after the separator")]
    EmptyArgv,
}

/// The limit at argv index `position`: plain decimal ASCII digits, no sign, no leading zero, from 1 to `cap`.
fn parse_limit(arg: &OsStr, position: u8, cap: u64) -> Result<u64, TrampolineError> {
    let refused = TrampolineError::Number { position };
    let text = arg.to_str().ok_or(refused)?;
    let plain = !text.is_empty()
        && !text.starts_with('0')
        && text.bytes().all(|byte| byte.is_ascii_digit());
    if !plain {
        return Err(refused);
    }
    text.parse::<u64>()
        .ok()
        .filter(|value| (1..=cap).contains(value))
        .ok_or(refused)
}

/// Parses the trampoline's own argv, `args[0]` included, strictly: exactly the layout [`trampoline_argv`] writes.
///
/// # Errors
///
/// [`TrampolineError::Layout`] when the flag or the separator is not where the layout puts it,
/// [`TrampolineError::EmptyArgv`] when no command or an empty program follows the separator, and
/// [`TrampolineError::Number`] for the first limit that is not plain decimal within its cap.
pub fn parse_trampoline(
    args: &[OsString],
) -> Result<(StageLimits, Vec<OsString>), TrampolineError> {
    let [
        _init,
        flag,
        address_space,
        file_size,
        processes,
        cpu,
        open_files,
        separator,
        argv @ ..,
    ] = args
    else {
        return Err(TrampolineError::Layout);
    };
    if flag != TRAMPOLINE_FLAG || separator != SEPARATOR {
        return Err(TrampolineError::Layout);
    }
    if argv.first().is_none_or(|program| program.is_empty()) {
        return Err(TrampolineError::EmptyArgv);
    }
    let limits = StageLimits {
        address_space_bytes: parse_limit(address_space, 2, MAX_ADDRESS_SPACE_BYTES)?,
        file_size_bytes: parse_limit(file_size, 3, MAX_FILE_SIZE_BYTES)?,
        processes: parse_limit(processes, 4, MAX_PROCESSES)?,
        cpu_seconds: parse_limit(cpu, 5, MAX_CPU_SECONDS)?,
        open_files: parse_limit(open_files, 6, MAX_OPEN_FILES)?,
    };
    Ok((limits, argv.to_vec()))
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::ffi::OsString;
    use std::path::Path;

    use proptest::prelude::*;

    use super::{
        GUEST_GID, GUEST_UID, HOME_DIR, MAX_ADDRESS_SPACE_BYTES, MAX_CPU_SECONDS,
        MAX_FILE_SIZE_BYTES, MAX_OPEN_FILES, MAX_PROCESSES, MIB, STAGE_ENV, StageLimits, TMP_MOUNT,
        TRAMPOLINE_FLAG, TrampolineError, parse_trampoline, scratch_mount_data, trampoline_argv,
    };
    use crate::test_fixture::{candidate_job, judge_job};
    use minos_core::limits::{MAX_STAGE_MS, MAX_VCPUS};
    use minos_core::outcome::Stage;

    type TestResult = Result<(), Box<dyn Error>>;

    const INIT: &str = "/init";

    fn limits() -> StageLimits {
        StageLimits {
            address_space_bytes: 768 * MIB,
            file_size_bytes: 1024 * MIB,
            processes: 512,
            cpu_seconds: 121,
            open_files: 4096,
        }
    }

    fn os(args: &[&str]) -> Vec<OsString> {
        args.iter().map(OsString::from).collect()
    }

    /// A well-formed trampoline argv with `numbers` as the five limits and `tail` after them.
    fn with(numbers: [&str; 5], tail: &[&str]) -> Vec<OsString> {
        let mut out = os(&[INIT, TRAMPOLINE_FLAG]);
        out.extend(os(&numbers));
        out.extend(os(tail));
        out
    }

    const GOOD: [&str; 5] = ["805306368", "1073741824", "512", "121", "4096"];

    #[test]
    fn scratch_mount_data_names_the_cap_and_mode() -> TestResult {
        let job = candidate_job()?;

        assert_eq!(
            scratch_mount_data(job.scratch_mib()),
            "size=1024m,mode=0755"
        );
        assert_eq!(scratch_mount_data(64), "size=64m,mode=0755");
        Ok(())
    }

    #[test]
    fn guest_identity_is_fixed_and_unprivileged() {
        assert_eq!(GUEST_UID, 64_000);
        assert_eq!(GUEST_GID, 64_000);
    }

    #[test]
    fn stage_limits_from_job() -> TestResult {
        let job = candidate_job()?;
        assert_eq!((job.memory_mib(), job.scratch_mib()), (2048, 1024));
        let build = job
            .stages()
            .iter()
            .find(|stage| stage.stage() == Stage::Build)
            .ok_or("no build stage")?;
        assert_eq!(build.budget_ms(), 120_000);

        let limits = StageLimits::for_stage(&job, build, 1);

        assert_eq!(limits.address_space_bytes(), 768 * 1_048_576);
        assert_eq!(limits.file_size_bytes(), 1024 * 1_048_576);
        assert_eq!(limits.processes(), 512);
        assert_eq!(limits.cpu_seconds(), 121);
        assert_eq!(limits.open_files(), 4096);
        assert_eq!(limits, self::limits());
        Ok(())
    }

    #[test]
    fn stage_limits_round_a_partial_second_up() -> TestResult {
        let job = judge_job()?;
        let judge = job.stages().first().ok_or("no judge stage")?;
        // The default judge budget is 2 minutes: 120 s plus the one-second margin.
        assert_eq!(StageLimits::for_stage(&job, judge, 1).cpu_seconds(), 121);
        Ok(())
    }

    #[test]
    fn stage_cpu_limit_scales_with_vcpus_within_the_cap() -> TestResult {
        let job = candidate_job()?;
        let build = job
            .stages()
            .iter()
            .find(|stage| stage.stage() == Stage::Build)
            .ok_or("no build stage")?;
        // Eight vCPUs: a process that keeps all eight busy for the 120 s budget uses 968 s of CPU time.
        let eight = StageLimits::for_stage(&job, build, 8);
        assert_eq!(eight.cpu_seconds(), 121 * 8);
        assert!(eight.cpu_seconds() <= MAX_CPU_SECONDS);
        // The other limits do not scale.
        assert_eq!(eight.address_space_bytes(), limits().address_space_bytes());
        assert_eq!(eight.file_size_bytes(), limits().file_size_bytes());
        assert_eq!(eight.processes(), limits().processes());
        assert_eq!(eight.open_files(), limits().open_files());
        // Zero is treated as one vCPU; a count over MAX_VCPUS is clamped to it.
        assert_eq!(StageLimits::for_stage(&job, build, 0).cpu_seconds(), 121);
        assert_eq!(
            StageLimits::for_stage(&job, build, u8::MAX).cpu_seconds(),
            121 * u64::from(MAX_VCPUS)
        );
        // The longest budget on the most vCPUs is exactly the trampoline's cap, so the trampoline accepts
        // every limit `for_stage` can produce.
        assert_eq!(
            MAX_CPU_SECONDS,
            (MAX_STAGE_MS.div_ceil(1000) + 1) * u64::from(MAX_VCPUS)
        );
        Ok(())
    }

    #[test]
    fn trampoline_argv_layout() {
        let argv = trampoline_argv(
            Path::new(INIT),
            &limits(),
            &["cargo".to_owned(), "build".to_owned()],
        );
        assert_eq!(argv, with(GOOD, &["--", "cargo", "build"]));
    }

    #[test]
    fn trampoline_refusals() {
        let cases: [(Vec<OsString>, TrampolineError); 14] = [
            // No flag at all: the program's argv as PID 1 would see it.
            (os(&[INIT]), TrampolineError::Layout),
            // The flag misspelled, so it is not the trampoline argv.
            (
                {
                    let mut args = with(GOOD, &["--", "true"]);
                    if let Some(flag) = args.get_mut(1) {
                        *flag = OsString::from("--minos-stag");
                    }
                    args
                },
                TrampolineError::Layout,
            ),
            // No separator.
            (with(GOOD, &["true"]), TrampolineError::Layout),
            // An extra argument before the separator.
            (with(GOOD, &["7", "--", "true"]), TrampolineError::Layout),
            // Too few limits.
            (
                os(&[INIT, TRAMPOLINE_FLAG, "1", "1", "1", "1", "--", "true"]),
                TrampolineError::Layout,
            ),
            // An empty argv after the separator, and an empty program.
            (with(GOOD, &["--"]), TrampolineError::EmptyArgv),
            (with(GOOD, &["--", ""]), TrampolineError::EmptyArgv),
            // Signed, spaced, hex, leading-zero, and empty numbers.
            (
                with(["+805306368", "1", "1", "1", "1"], &["--", "true"]),
                TrampolineError::Number { position: 2 },
            ),
            (
                with(["1", "-1", "1", "1", "1"], &["--", "true"]),
                TrampolineError::Number { position: 3 },
            ),
            (
                with(["1", "1", " 5", "1", "1"], &["--", "true"]),
                TrampolineError::Number { position: 4 },
            ),
            (
                with(["1", "1", "1", "0x10", "1"], &["--", "true"]),
                TrampolineError::Number { position: 5 },
            ),
            (
                with(["1", "1", "1", "1", "0"], &["--", "true"]),
                TrampolineError::Number { position: 6 },
            ),
            (
                with(["1", "1", "1", "012", "1"], &["--", "true"]),
                TrampolineError::Number { position: 5 },
            ),
            (
                with(["", "1", "1", "1", "1"], &["--", "true"]),
                TrampolineError::Number { position: 2 },
            ),
        ];
        for (args, expected) in cases {
            assert_eq!(parse_trampoline(&args), Err(expected), "{args:?}");
        }
    }

    #[test]
    fn trampoline_refuses_a_limit_above_its_cap() {
        let caps = [
            MAX_ADDRESS_SPACE_BYTES,
            MAX_FILE_SIZE_BYTES,
            MAX_PROCESSES,
            MAX_CPU_SECONDS,
            MAX_OPEN_FILES,
        ];
        for (index, cap) in caps.into_iter().enumerate() {
            let mut numbers = GOOD.map(str::to_owned);
            let at_cap = cap.to_string();
            let over = cap.saturating_add(1).to_string();
            if let Some(slot) = numbers.get_mut(index) {
                slot.clone_from(&at_cap);
            }
            let args = with(numbers.each_ref().map(String::as_str), &["--", "true"]);
            assert!(parse_trampoline(&args).is_ok(), "the cap itself is allowed");
            if let Some(slot) = numbers.get_mut(index) {
                *slot = over;
            }
            let args = with(numbers.each_ref().map(String::as_str), &["--", "true"]);
            let position = u8::try_from(index + 2).unwrap_or(u8::MAX);
            assert_eq!(
                parse_trampoline(&args),
                Err(TrampolineError::Number { position })
            );
        }
        let huge = with(
            ["99999999999999999999999", "1", "1", "1", "1"],
            &["--", "true"],
        );
        assert_eq!(
            parse_trampoline(&huge),
            Err(TrampolineError::Number { position: 2 })
        );
    }

    #[test]
    fn trampoline_keeps_a_separator_inside_the_command() -> TestResult {
        let (_, argv) = parse_trampoline(&with(GOOD, &["--", "sh", "--", "-c"]))?;
        assert_eq!(argv, os(&["sh", "--", "-c"]));
        Ok(())
    }

    #[test]
    fn stage_env_is_fixed() {
        let keys: Vec<&str> = STAGE_ENV.iter().map(|(key, _)| *key).collect();
        assert_eq!(keys, ["PATH", "HOME", "TMPDIR", "LANG"]);
        let host_prefixes = [
            "/Users",
            "/home/",
            "/private",
            "/var/folders",
            "/opt/homebrew",
            "/root",
        ];
        for (key, value) in STAGE_ENV {
            assert!(
                host_prefixes.iter().all(|prefix| !value.contains(prefix)),
                "{key} holds a host path"
            );
        }
        assert!(STAGE_ENV.contains(&("HOME", HOME_DIR)));
        assert!(STAGE_ENV.contains(&("TMPDIR", TMP_MOUNT)));
    }

    fn any_limits() -> impl Strategy<Value = StageLimits> {
        (
            1..=MAX_ADDRESS_SPACE_BYTES,
            1..=MAX_FILE_SIZE_BYTES,
            1..=MAX_PROCESSES,
            1..=MAX_CPU_SECONDS,
            1..=MAX_OPEN_FILES,
        )
            .prop_map(
                |(address_space_bytes, file_size_bytes, processes, cpu_seconds, open_files)| {
                    StageLimits {
                        address_space_bytes,
                        file_size_bytes,
                        processes,
                        cpu_seconds,
                        open_files,
                    }
                },
            )
    }

    proptest! {
        #[test]
        fn trampoline_round_trip(
            limits in any_limits(),
            argv in prop::collection::vec("[^\u{0}]{1,24}", 1..=8),
        ) {
            let args = trampoline_argv(Path::new(INIT), &limits, &argv);
            let parsed = parse_trampoline(&args);
            let expected: Vec<OsString> = argv.iter().map(OsString::from).collect();
            prop_assert_eq!(parsed, Ok((limits, expected)));
        }
    }
}
