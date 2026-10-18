//! Job wire format: the `MINOSJB1` header and the v1 job JSON the guest reads from `/dev/vdb`.
//!
//! The header is 20 bytes: magic `MINOSJB1`, a `u32` version, and a `u64` payload length, both big-endian.
//! Exactly `length` bytes of UTF-8 JSON follow, and the rest of the disk is zero padding the guest never reads.
//! The host is the only writer, but the guest validates as if the job were hostile: the length is capped before
//! any payload byte is read, every object rejects unknown fields, and every number is range-checked here.

use std::io::Read;
use std::net::SocketAddr;
use std::str::FromStr;
use std::time::Duration;

use serde::Deserialize;

use crate::{MAX_JOB_BYTES, PROTOCOL_VERSION};

/// Magic bytes at the start of the job header.
pub const JOB_MAGIC: [u8; 8] = *b"MINOSJB1";

/// Fewest steps a job may hold.
pub const MIN_STEPS: usize = 1;
/// Most steps a job may hold.
pub const MAX_STEPS: usize = 64;
/// Largest vsock port a job may name (`u32::MAX` is `VMADDR_PORT_ANY`).
pub const MAX_VSOCK_PORT: u32 = 4_294_967_294;
/// Most targets one `connect` step may probe.
pub const MAX_CONNECT_TARGETS: usize = 16;
/// Longest per-target timeout of a `connect` step, in milliseconds.
pub const MAX_CONNECT_TIMEOUT_MS: u64 = 30_000;
/// Most bytes one `vsock_stream` step may write.
pub const MAX_VSOCK_STREAM_BYTES: u64 = 1_073_741_824;
/// Most bytes one `scratch_roundtrip` step may write and read back.
pub const MAX_SCRATCH_BYTES: u64 = 67_108_864;
/// Most argv entries of a `run` step.
pub const MAX_ARGV_ENTRIES: usize = 64;
/// Longest single argv entry of a `run` step, in bytes.
pub const MAX_ARG_BYTES: usize = 4096;
/// Largest per-stream capture of a `run` step, in bytes.
pub const MAX_STDOUT_CAP: u64 = 1_048_576;
/// Longest timeout of a `run` step, in milliseconds: one hour, the launcher's own `max_duration_ms` ceiling.
pub const MAX_RUN_TIMEOUT_MS: u64 = 3_600_000;
/// Most bytes one `console_flood` step may write.
pub const MAX_CONSOLE_FLOOD_BYTES: u64 = 1_073_741_824;

/// Why a job could not be read.
#[derive(Debug, thiserror::Error)]
pub enum JobError {
    /// The header does not start with [`JOB_MAGIC`].
    #[error("job header magic is {found:02x?}, expected MINOSJB1")]
    BadMagic {
        /// The eight bytes found where the magic belongs.
        found: [u8; 8],
    },
    /// The header names a protocol version this guest does not speak.
    #[error("job protocol version {0} is not supported (expected {PROTOCOL_VERSION})")]
    UnsupportedVersion(u32),
    /// The header declares a payload longer than [`MAX_JOB_BYTES`].
    #[error("job payload of {0} bytes exceeds the {MAX_JOB_BYTES}-byte limit")]
    TooLarge(u64),
    /// The disk ended before the declared payload length.
    #[error("job payload truncated: header declares {declared} bytes, disk holds {read}")]
    Truncated {
        /// Payload length from the header.
        declared: u64,
        /// Bytes actually available.
        read: u64,
    },
    /// Reading the header or payload failed.
    #[error("reading the job: {0}")]
    Io(#[source] std::io::Error),
    /// The payload is not valid job JSON: bad syntax, a wrong type, a missing field, or an unknown field.
    #[error("job JSON: {0}")]
    Json(#[source] serde_json::Error),
    /// A field parsed but is outside its allowed range.
    #[error("job field {0}")]
    Invalid(#[from] InvalidField),
}

/// A job field whose value is outside its allowed range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("{field} must be {limit}")]
pub struct InvalidField {
    field: &'static str,
    limit: &'static str,
}

impl InvalidField {
    const fn new(field: &'static str, limit: &'static str) -> Self {
        Self { field, limit }
    }

    /// Dotted name of the offending field, such as `steps.connect.targets`.
    #[must_use]
    pub const fn field(&self) -> &'static str {
        self.field
    }

    /// The allowed range, in words.
    #[must_use]
    pub const fn limit(&self) -> &'static str {
        self.limit
    }
}

/// Reads one job: the header, then exactly the declared number of payload bytes, then validates the JSON.
///
/// # Errors
///
/// Returns [`JobError`] for a bad magic, an unsupported version, a declared length over [`MAX_JOB_BYTES`],
/// a payload shorter than declared, an I/O failure, malformed or unknown JSON, or a field out of range.
pub fn read_job(reader: &mut impl Read) -> Result<Job, JobError> {
    let mut magic = [0_u8; 8];
    let mut version = [0_u8; 4];
    let mut length = [0_u8; 8];
    reader.read_exact(&mut magic).map_err(JobError::Io)?;
    reader.read_exact(&mut version).map_err(JobError::Io)?;
    reader.read_exact(&mut length).map_err(JobError::Io)?;
    if magic != JOB_MAGIC {
        return Err(JobError::BadMagic { found: magic });
    }
    let version = u32::from_be_bytes(version);
    if version != PROTOCOL_VERSION {
        return Err(JobError::UnsupportedVersion(version));
    }
    let declared = u64::from_be_bytes(length);
    if declared > MAX_JOB_BYTES {
        return Err(JobError::TooLarge(declared));
    }
    // declared <= MAX_JOB_BYTES (1 MiB), so the conversion cannot fail on any supported target.
    let capacity = usize::try_from(declared).map_err(|_| JobError::TooLarge(declared))?;
    let mut payload = Vec::with_capacity(capacity);
    reader
        .take(declared)
        .read_to_end(&mut payload)
        .map_err(JobError::Io)?;
    let read = u64::try_from(payload.len()).map_err(|_| JobError::TooLarge(declared))?;
    if read != declared {
        return Err(JobError::Truncated { declared, read });
    }
    let raw: RawJob = serde_json::from_slice(&payload).map_err(JobError::Json)?;
    Ok(Job::try_from(raw)?)
}

/// What the guest does after the last step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Finish {
    /// Power the VM off (`reboot(POWER_OFF)`).
    Poweroff,
    /// Return from PID 1, which makes the kernel panic. Used to observe how an init exit surfaces.
    Return,
}

/// The job JSON as written, before range checks.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawJob {
    v: u32,
    vsock_port: u32,
    steps: Vec<RawStep>,
    finish: Finish,
}

/// A validated job. If a value of this type exists, every field is within its limits.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "RawJob")]
pub struct Job {
    vsock_port: u32,
    steps: Vec<Step>,
    finish: Finish,
}

impl Job {
    /// The host vsock port (CID 2) that receives the status frame and any `vsock_stream` bytes.
    #[must_use]
    pub const fn vsock_port(&self) -> u32 {
        self.vsock_port
    }

    /// The steps, in the order the guest runs them.
    #[must_use]
    pub fn steps(&self) -> &[Step] {
        &self.steps
    }

    /// What the guest does after the last step.
    #[must_use]
    pub const fn finish(&self) -> Finish {
        self.finish
    }
}

impl TryFrom<RawJob> for Job {
    type Error = InvalidField;

    fn try_from(raw: RawJob) -> Result<Self, Self::Error> {
        if raw.v != PROTOCOL_VERSION {
            return Err(InvalidField::new("v", "1"));
        }
        let vsock_port = check_port("vsock_port", raw.vsock_port)?;
        if !(MIN_STEPS..=MAX_STEPS).contains(&raw.steps.len()) {
            return Err(InvalidField::new("steps", "1 to 64 entries"));
        }
        let steps = raw
            .steps
            .into_iter()
            .map(Step::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            vsock_port,
            steps,
            finish: raw.finish,
        })
    }
}

/// One step as written, before range checks. Externally tagged: `{"connect":{...}}`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RawStep {
    /// See [`ReportBoot`].
    ReportBoot(RawReportBoot),
    /// See [`Connect`].
    Connect(RawConnect),
    /// See [`VsockConnect`].
    VsockConnect(RawVsockConnect),
    /// See [`VsockStream`].
    VsockStream(RawVsockStream),
    /// See [`ScratchRoundtrip`].
    ScratchRoundtrip(RawScratchRoundtrip),
    /// See [`Run`].
    Run(RawRun),
    /// See [`ConsoleFlood`].
    ConsoleFlood(RawConsoleFlood),
    /// See [`Hang`].
    Hang(RawHang),
}

/// A validated step.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "RawStep")]
pub enum Step {
    /// Report what the guest sees at boot: cmdline, kernel release, argv, env, interfaces, virtio devices.
    ReportBoot(ReportBoot),
    /// Try a TCP connection to each target and record the result.
    Connect(Connect),
    /// Try a vsock connection to the host (CID 2) on a port and record the result.
    VsockConnect(VsockConnect),
    /// Stream a byte pattern to the host on the job's vsock port.
    VsockStream(VsockStream),
    /// Write a byte pattern to the scratch disk (`/dev/vdc`) and read it back.
    ScratchRoundtrip(ScratchRoundtrip),
    /// Run a command from the root image with an empty environment and capped output capture.
    Run(Run),
    /// Write printable bytes to the console.
    ConsoleFlood(ConsoleFlood),
    /// Sleep forever; no later step runs and no status frame is sent.
    Hang(Hang),
}

impl TryFrom<RawStep> for Step {
    type Error = InvalidField;

    fn try_from(raw: RawStep) -> Result<Self, Self::Error> {
        Ok(match raw {
            RawStep::ReportBoot(RawReportBoot {}) => Self::ReportBoot(ReportBoot),
            RawStep::Connect(r) => Self::Connect(Connect::try_from(r)?),
            RawStep::VsockConnect(r) => Self::VsockConnect(VsockConnect::try_from(r)?),
            RawStep::VsockStream(r) => Self::VsockStream(VsockStream::try_from(r)?),
            RawStep::ScratchRoundtrip(r) => Self::ScratchRoundtrip(ScratchRoundtrip::try_from(r)?),
            RawStep::Run(r) => Self::Run(Run::try_from(r)?),
            RawStep::ConsoleFlood(r) => Self::ConsoleFlood(ConsoleFlood::try_from(r)?),
            RawStep::Hang(RawHang {}) => Self::Hang(Hang),
        })
    }
}

/// `report_boot` as written: `{}`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawReportBoot {}

/// The validated `report_boot` step. It has no fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReportBoot;

/// `connect` as written.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawConnect {
    targets: Vec<String>,
    timeout_ms: u64,
}

/// The validated `connect` step: 1 to 16 socket addresses and a timeout of 1 to 30000 ms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Connect {
    targets: Vec<SocketAddr>,
    timeout_ms: u64,
}

impl Connect {
    /// The addresses to probe, in job order.
    #[must_use]
    pub fn targets(&self) -> &[SocketAddr] {
        &self.targets
    }

    /// The per-target connect timeout.
    #[must_use]
    pub const fn timeout(&self) -> Duration {
        Duration::from_millis(self.timeout_ms)
    }
}

impl TryFrom<RawConnect> for Connect {
    type Error = InvalidField;

    fn try_from(raw: RawConnect) -> Result<Self, Self::Error> {
        if !(1..=MAX_CONNECT_TARGETS).contains(&raw.targets.len()) {
            return Err(InvalidField::new(
                "steps.connect.targets",
                "1 to 16 entries",
            ));
        }
        let targets = raw
            .targets
            .iter()
            .map(|text| SocketAddr::from_str(text))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| {
                InvalidField::new(
                    "steps.connect.targets",
                    "socket addresses such as 1.1.1.1:443 or [::1]:443",
                )
            })?;
        if !(1..=MAX_CONNECT_TIMEOUT_MS).contains(&raw.timeout_ms) {
            return Err(InvalidField::new("steps.connect.timeout_ms", "1 to 30000"));
        }
        Ok(Self {
            targets,
            timeout_ms: raw.timeout_ms,
        })
    }
}

/// `vsock_connect` as written.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawVsockConnect {
    port: u32,
}

/// The validated `vsock_connect` step: a port of 1 to 4294967294.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VsockConnect {
    port: u32,
}

impl VsockConnect {
    /// The host vsock port to try.
    #[must_use]
    pub const fn port(&self) -> u32 {
        self.port
    }
}

impl TryFrom<RawVsockConnect> for VsockConnect {
    type Error = InvalidField;

    fn try_from(raw: RawVsockConnect) -> Result<Self, Self::Error> {
        Ok(Self {
            port: check_port("steps.vsock_connect.port", raw.port)?,
        })
    }
}

/// `vsock_stream` as written.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawVsockStream {
    bytes: u64,
}

/// The validated `vsock_stream` step: 0 to 1073741824 bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VsockStream {
    bytes: u64,
}

impl VsockStream {
    /// How many pattern bytes to write.
    #[must_use]
    pub const fn bytes(&self) -> u64 {
        self.bytes
    }
}

impl TryFrom<RawVsockStream> for VsockStream {
    type Error = InvalidField;

    fn try_from(raw: RawVsockStream) -> Result<Self, Self::Error> {
        if raw.bytes > MAX_VSOCK_STREAM_BYTES {
            return Err(InvalidField::new(
                "steps.vsock_stream.bytes",
                "0 to 1073741824",
            ));
        }
        Ok(Self { bytes: raw.bytes })
    }
}

/// `scratch_roundtrip` as written.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawScratchRoundtrip {
    bytes: u64,
}

/// The validated `scratch_roundtrip` step: 1 to 67108864 bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScratchRoundtrip {
    bytes: u64,
}

impl ScratchRoundtrip {
    /// How many pattern bytes to write and read back.
    #[must_use]
    pub const fn bytes(&self) -> u64 {
        self.bytes
    }
}

impl TryFrom<RawScratchRoundtrip> for ScratchRoundtrip {
    type Error = InvalidField;

    fn try_from(raw: RawScratchRoundtrip) -> Result<Self, Self::Error> {
        if !(1..=MAX_SCRATCH_BYTES).contains(&raw.bytes) {
            return Err(InvalidField::new(
                "steps.scratch_roundtrip.bytes",
                "1 to 67108864",
            ));
        }
        Ok(Self { bytes: raw.bytes })
    }
}

/// `run` as written.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawRun {
    argv: Vec<String>,
    stdout_cap: u64,
    timeout_ms: u64,
}

/// The validated `run` step: an argv of 1 to 64 entries whose first entry is an absolute path, each entry at
/// most 4096 bytes with no NUL, a capture cap of 1 to 1048576 bytes per stream, and a timeout of 1 to
/// 3600000 ms after which the command's whole process group is killed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    program: String,
    args: Vec<String>,
    stdout_cap: u64,
    timeout_ms: u64,
}

impl Run {
    /// The absolute path of the program, `argv[0]`.
    #[must_use]
    pub fn program(&self) -> &str {
        &self.program
    }

    /// The arguments after the program.
    #[must_use]
    pub fn args(&self) -> &[String] {
        &self.args
    }

    /// The whole argv, program first, as the job wrote it.
    #[must_use]
    pub fn argv(&self) -> Vec<String> {
        std::iter::once(self.program.clone())
            .chain(self.args.iter().cloned())
            .collect()
    }

    /// Most bytes captured from each of stdout and stderr.
    #[must_use]
    pub const fn stdout_cap(&self) -> u64 {
        self.stdout_cap
    }

    /// How long the command may run before its process group is killed.
    #[must_use]
    pub const fn timeout(&self) -> Duration {
        Duration::from_millis(self.timeout_ms)
    }
}

impl TryFrom<RawRun> for Run {
    type Error = InvalidField;

    fn try_from(raw: RawRun) -> Result<Self, Self::Error> {
        if raw.argv.len() > MAX_ARGV_ENTRIES {
            return Err(InvalidField::new("steps.run.argv", "1 to 64 entries"));
        }
        if raw
            .argv
            .iter()
            .any(|arg| arg.len() > MAX_ARG_BYTES || arg.contains('\0'))
        {
            return Err(InvalidField::new(
                "steps.run.argv",
                "entries of at most 4096 bytes with no NUL",
            ));
        }
        let mut argv = raw.argv.into_iter();
        let program = argv
            .next()
            .ok_or(InvalidField::new("steps.run.argv", "1 to 64 entries"))?;
        if !program.starts_with('/') {
            return Err(InvalidField::new(
                "steps.run.argv",
                "led by an absolute path",
            ));
        }
        if !(1..=MAX_STDOUT_CAP).contains(&raw.stdout_cap) {
            return Err(InvalidField::new("steps.run.stdout_cap", "1 to 1048576"));
        }
        if !(1..=MAX_RUN_TIMEOUT_MS).contains(&raw.timeout_ms) {
            return Err(InvalidField::new("steps.run.timeout_ms", "1 to 3600000"));
        }
        Ok(Self {
            program,
            args: argv.collect(),
            stdout_cap: raw.stdout_cap,
            timeout_ms: raw.timeout_ms,
        })
    }
}

/// `console_flood` as written.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawConsoleFlood {
    bytes: u64,
}

/// The validated `console_flood` step: 1 to 1073741824 bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConsoleFlood {
    bytes: u64,
}

impl ConsoleFlood {
    /// How many bytes to write to the console.
    #[must_use]
    pub const fn bytes(&self) -> u64 {
        self.bytes
    }
}

impl TryFrom<RawConsoleFlood> for ConsoleFlood {
    type Error = InvalidField;

    fn try_from(raw: RawConsoleFlood) -> Result<Self, Self::Error> {
        if !(1..=MAX_CONSOLE_FLOOD_BYTES).contains(&raw.bytes) {
            return Err(InvalidField::new(
                "steps.console_flood.bytes",
                "1 to 1073741824",
            ));
        }
        Ok(Self { bytes: raw.bytes })
    }
}

/// `hang` as written: `{}`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawHang {}

/// The validated `hang` step. It has no fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hang;

fn check_port(field: &'static str, port: u32) -> Result<u32, InvalidField> {
    if (1..=MAX_VSOCK_PORT).contains(&port) {
        Ok(port)
    } else {
        Err(InvalidField::new(field, "1 to 4294967294"))
    }
}

#[cfg(test)]
mod tests {
    use super::{Finish, JobError, Step, read_job};
    use crate::test_hex;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    /// The canonical job JSON from 02-01-PLAN.md Exact texts E-9.
    const CANONICAL: &str = r#"{"v":1,"vsock_port":1024,"steps":[{"report_boot":{}},{"scratch_roundtrip":{"bytes":65536}}],"finish":"poweroff"}"#;

    fn header(magic: [u8; 8], version: u32, len: u64) -> Vec<u8> {
        let mut out = magic.to_vec();
        out.extend_from_slice(&version.to_be_bytes());
        out.extend_from_slice(&len.to_be_bytes());
        out
    }

    fn framed(json: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let mut out = header(*b"MINOSJB1", 1, u64::try_from(json.len())?);
        out.extend_from_slice(json.as_bytes());
        // Disk padding after the payload must be ignored.
        out.extend_from_slice(&[0_u8; 64]);
        Ok(out)
    }

    fn parse(json: &str) -> Result<Result<super::Job, JobError>, Box<dyn std::error::Error>> {
        let bytes = framed(json)?;
        Ok(read_job(&mut bytes.as_slice()))
    }

    fn invalid_field(result: &Result<super::Job, JobError>) -> Option<&'static str> {
        match result {
            Err(JobError::Invalid(field)) => Some(field.field()),
            _ => None,
        }
    }

    fn with_step(step: &str) -> String {
        format!(r#"{{"v":1,"vsock_port":1024,"steps":[{step}],"finish":"poweroff"}}"#)
    }

    #[test]
    fn canonical_job_decodes() -> TestResult {
        let job = parse(CANONICAL)??;
        assert_eq!(job.vsock_port(), 1024);
        assert_eq!(job.finish(), Finish::Poweroff);
        assert!(matches!(
            job.steps(),
            [Step::ReportBoot(_), Step::ScratchRoundtrip(scratch)] if scratch.bytes() == 65536
        ));
        Ok(())
    }

    #[test]
    fn golden_header_for_two_byte_payload() -> TestResult {
        let golden = test_hex::decode("4d494e4f534a42310000000100000000000000027b7d")?;
        let mut built = header(*b"MINOSJB1", 1, 2);
        built.extend_from_slice(b"{}");
        assert_eq!(built, golden);
        // The header is accepted; the empty object then fails as JSON (missing fields), not as a header.
        assert!(matches!(
            read_job(&mut golden.as_slice()),
            Err(JobError::Json(_))
        ));
        Ok(())
    }

    #[test]
    fn rejects_wrong_magic() {
        let bytes = header(*b"MINOSJB2", 1, 2);
        assert!(matches!(
            read_job(&mut bytes.as_slice()),
            Err(JobError::BadMagic { found }) if &found == b"MINOSJB2"
        ));
    }

    #[test]
    fn rejects_version_two_header() {
        let bytes = header(*b"MINOSJB1", 2, 2);
        assert!(matches!(
            read_job(&mut bytes.as_slice()),
            Err(JobError::UnsupportedVersion(2))
        ));
    }

    #[test]
    fn rejects_length_over_cap_before_reading() {
        // No payload follows: the cap must fire before any payload byte is read.
        let bytes = header(*b"MINOSJB1", 1, 1_048_577);
        assert!(matches!(
            read_job(&mut bytes.as_slice()),
            Err(JobError::TooLarge(1_048_577))
        ));
    }

    #[test]
    fn rejects_payload_shorter_than_declared() {
        let mut bytes = header(*b"MINOSJB1", 1, 100);
        bytes.extend_from_slice(b"{\"v\":1}");
        assert!(matches!(
            read_job(&mut bytes.as_slice()),
            Err(JobError::Truncated {
                declared: 100,
                read: 7
            })
        ));
    }

    #[test]
    fn rejects_unknown_fields_at_every_level() -> TestResult {
        let top =
            r#"{"v":1,"vsock_port":1024,"steps":[{"hang":{}}],"finish":"poweroff","extra":1}"#;
        assert!(matches!(parse(top)?, Err(JobError::Json(_))));
        let step = with_step(r#"{"scratch_roundtrip":{"bytes":1,"extra":1}}"#);
        assert!(matches!(parse(&step)?, Err(JobError::Json(_))));
        let empty_step = with_step(r#"{"report_boot":{"extra":1}}"#);
        assert!(matches!(parse(&empty_step)?, Err(JobError::Json(_))));
        let unknown_step = with_step(r#"{"format_disk":{}}"#);
        assert!(matches!(parse(&unknown_step)?, Err(JobError::Json(_))));
        Ok(())
    }

    #[test]
    fn rejects_json_version_two() -> TestResult {
        let job = r#"{"v":2,"vsock_port":1024,"steps":[{"hang":{}}],"finish":"poweroff"}"#;
        assert_eq!(invalid_field(&parse(job)?), Some("v"));
        Ok(())
    }

    #[test]
    fn rejects_zero_and_sixty_five_steps() -> TestResult {
        let zero = r#"{"v":1,"vsock_port":1024,"steps":[],"finish":"poweroff"}"#;
        assert_eq!(invalid_field(&parse(zero)?), Some("steps"));
        let many = vec![r#"{"report_boot":{}}"#; 65].join(",");
        assert_eq!(invalid_field(&parse(&with_step(&many))?), Some("steps"));
        let max = vec![r#"{"report_boot":{}}"#; 64].join(",");
        assert!(parse(&with_step(&max))?.is_ok());
        Ok(())
    }

    #[test]
    fn rejects_bad_connect_targets() -> TestResult {
        let seventeen = vec![r#""1.1.1.1:443""#; 17].join(",");
        let too_many = with_step(&format!(
            r#"{{"connect":{{"targets":[{seventeen}],"timeout_ms":2000}}}}"#
        ));
        assert_eq!(
            invalid_field(&parse(&too_many)?),
            Some("steps.connect.targets")
        );
        let unparsable =
            with_step(r#"{"connect":{"targets":["one.one.one.one:443"],"timeout_ms":2000}}"#);
        assert_eq!(
            invalid_field(&parse(&unparsable)?),
            Some("steps.connect.targets")
        );
        let ipv6 =
            with_step(r#"{"connect":{"targets":["[::1]:443","1.1.1.1:443"],"timeout_ms":2000}}"#);
        assert!(parse(&ipv6)?.is_ok());
        Ok(())
    }

    #[test]
    fn rejects_bad_run_argv() -> TestResult {
        let empty = with_step(r#"{"run":{"argv":[],"stdout_cap":16,"timeout_ms":1000}}"#);
        assert_eq!(invalid_field(&parse(&empty)?), Some("steps.run.argv"));
        let relative =
            with_step(r#"{"run":{"argv":["bin/true"],"stdout_cap":16,"timeout_ms":1000}}"#);
        assert_eq!(invalid_field(&parse(&relative)?), Some("steps.run.argv"));
        let nul = with_step(
            r#"{"run":{"argv":["/bin/echo","a\u0000b"],"stdout_cap":16,"timeout_ms":1000}}"#,
        );
        assert_eq!(invalid_field(&parse(&nul)?), Some("steps.run.argv"));
        let long_arg = "x".repeat(4097);
        let long = with_step(&format!(
            r#"{{"run":{{"argv":["/bin/echo","{long_arg}"],"stdout_cap":16,"timeout_ms":1000}}}}"#
        ));
        assert_eq!(invalid_field(&parse(&long)?), Some("steps.run.argv"));
        let ok =
            with_step(r#"{"run":{"argv":["/bin/echo","hi"],"stdout_cap":16,"timeout_ms":1000}}"#);
        let job = parse(&ok)??;
        assert!(matches!(
            job.steps(),
            [Step::Run(run)] if run.program() == "/bin/echo" && run.argv() == ["/bin/echo", "hi"]
                && run.timeout() == std::time::Duration::from_secs(1)
        ));
        let missing = with_step(r#"{"run":{"argv":["/bin/echo","hi"],"stdout_cap":16}}"#);
        assert!(matches!(parse(&missing)?, Err(JobError::Json(_))));
        Ok(())
    }

    #[test]
    fn rejects_each_number_outside_its_range() -> TestResult {
        let cases = [
            (
                r#"{"v":1,"vsock_port":0,"steps":[{"hang":{}}],"finish":"poweroff"}"#.to_owned(),
                "vsock_port",
            ),
            (
                r#"{"v":1,"vsock_port":4294967295,"steps":[{"hang":{}}],"finish":"poweroff"}"#
                    .to_owned(),
                "vsock_port",
            ),
            (
                with_step(r#"{"connect":{"targets":["1.1.1.1:443"],"timeout_ms":0}}"#),
                "steps.connect.timeout_ms",
            ),
            (
                with_step(r#"{"connect":{"targets":["1.1.1.1:443"],"timeout_ms":30001}}"#),
                "steps.connect.timeout_ms",
            ),
            (
                with_step(r#"{"vsock_connect":{"port":0}}"#),
                "steps.vsock_connect.port",
            ),
            (
                with_step(r#"{"vsock_connect":{"port":4294967295}}"#),
                "steps.vsock_connect.port",
            ),
            (
                with_step(r#"{"vsock_stream":{"bytes":1073741825}}"#),
                "steps.vsock_stream.bytes",
            ),
            (
                with_step(r#"{"scratch_roundtrip":{"bytes":0}}"#),
                "steps.scratch_roundtrip.bytes",
            ),
            (
                with_step(r#"{"scratch_roundtrip":{"bytes":67108865}}"#),
                "steps.scratch_roundtrip.bytes",
            ),
            (
                with_step(r#"{"run":{"argv":["/bin/true"],"stdout_cap":0,"timeout_ms":1000}}"#),
                "steps.run.stdout_cap",
            ),
            (
                with_step(
                    r#"{"run":{"argv":["/bin/true"],"stdout_cap":1048577,"timeout_ms":1000}}"#,
                ),
                "steps.run.stdout_cap",
            ),
            (
                with_step(r#"{"run":{"argv":["/bin/true"],"stdout_cap":16,"timeout_ms":0}}"#),
                "steps.run.timeout_ms",
            ),
            (
                with_step(r#"{"run":{"argv":["/bin/true"],"stdout_cap":16,"timeout_ms":3600001}}"#),
                "steps.run.timeout_ms",
            ),
            (
                with_step(r#"{"console_flood":{"bytes":0}}"#),
                "steps.console_flood.bytes",
            ),
            (
                with_step(r#"{"console_flood":{"bytes":1073741825}}"#),
                "steps.console_flood.bytes",
            ),
        ];
        for (json, field) in &cases {
            assert_eq!(invalid_field(&parse(json)?), Some(*field), "{json}");
        }
        let bounds = with_step(
            r#"{"vsock_stream":{"bytes":0}},{"vsock_stream":{"bytes":1073741824}},{"vsock_connect":{"port":4294967294}},{"console_flood":{"bytes":1073741824}},{"scratch_roundtrip":{"bytes":67108864}},{"run":{"argv":["/bin/true"],"stdout_cap":1048576,"timeout_ms":3600000}},{"connect":{"targets":["127.0.0.1:1"],"timeout_ms":30000}}"#,
        );
        assert!(parse(&bounds)?.is_ok());
        Ok(())
    }

    #[test]
    fn finish_return_decodes() -> TestResult {
        let job = r#"{"v":1,"vsock_port":7,"steps":[{"hang":{}}],"finish":"return"}"#;
        assert_eq!(parse(job)??.finish(), Finish::Return);
        Ok(())
    }

    /// The 64-character lowercase hex of 32 bytes of 0x5a, the wave0 test token.
    const TEST_TOKEN_HEX: &str = "5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a";

    fn chunk_stream_step(bytes: u64, token: &str) -> String {
        with_step(&format!(
            r#"{{"chunk_stream":{{"bytes":{bytes},"token":"{token}"}}}}"#
        ))
    }

    #[test]
    fn job_chunk_stream_step() -> TestResult {
        let job = parse(&chunk_stream_step(16_777_216, TEST_TOKEN_HEX))??;
        let [Step::ChunkStream(step)] = job.steps() else {
            return Err("expected one chunk_stream step".into());
        };
        assert_eq!(step.bytes(), 16_777_216);
        assert!(step.token().ct_eq(&[0x5a_u8; 32]));
        let max = parse(&chunk_stream_step(1_073_741_824, TEST_TOKEN_HEX))?;
        assert!(max.is_ok());
        Ok(())
    }

    #[test]
    fn job_chunk_stream_refuses_bad_bytes_and_tokens() -> TestResult {
        let short = TEST_TOKEN_HEX.get(1..).ok_or("token too short")?;
        let upper = TEST_TOKEN_HEX.replacen('a', "A", 1);
        let not_hex = TEST_TOKEN_HEX.replacen('a', "g", 1);
        let long = format!("{TEST_TOKEN_HEX}5");
        let cases = [
            (
                chunk_stream_step(0, TEST_TOKEN_HEX),
                "steps.chunk_stream.bytes",
            ),
            (
                chunk_stream_step(1_073_741_825, TEST_TOKEN_HEX),
                "steps.chunk_stream.bytes",
            ),
            (chunk_stream_step(1, short), "steps.chunk_stream.token"),
            (chunk_stream_step(1, &long), "steps.chunk_stream.token"),
            (chunk_stream_step(1, &upper), "steps.chunk_stream.token"),
            (chunk_stream_step(1, &not_hex), "steps.chunk_stream.token"),
            (chunk_stream_step(1, ""), "steps.chunk_stream.token"),
        ];
        for (json, field) in &cases {
            assert_eq!(invalid_field(&parse(json)?), Some(*field), "{json}");
        }
        let extra = with_step(&format!(
            r#"{{"chunk_stream":{{"bytes":1,"token":"{TEST_TOKEN_HEX}","extra":1}}}}"#
        ));
        assert!(matches!(parse(&extra)?, Err(JobError::Json(_))));
        Ok(())
    }

    /// The step's Debug never prints the token bytes.
    #[test]
    fn job_chunk_stream_debug_hides_the_token() -> TestResult {
        let job = parse(&chunk_stream_step(1, TEST_TOKEN_HEX))??;
        let shown = format!("{job:?}");
        assert!(!shown.contains("5a5a"), "{shown}");
        assert!(!shown.contains("90, 90"), "{shown}");
        Ok(())
    }
}
