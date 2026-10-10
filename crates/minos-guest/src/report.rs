//! The guest's report and the `MINOSST1` status frame that carries it to the host over vsock.
//!
//! The frame is 20 bytes of header (magic `MINOSST1`, a `u32` version, and a `u64` payload length, both
//! big-endian) followed by exactly `length` bytes of compact JSON. Field declaration order below is the wire
//! format: serde writes struct fields in the order they are declared.

use serde::Serialize;

use crate::{BOOT_MARKER, MAX_FRAME_BYTES, PROTOCOL_VERSION};

/// Magic bytes at the start of the status-frame header.
pub const FRAME_MAGIC: [u8; 8] = *b"MINOSST1";

/// Why a report could not be framed.
#[derive(Debug, thiserror::Error)]
pub enum FrameError {
    /// The report did not serialize.
    #[error("serializing the report: {0}")]
    Json(#[source] serde_json::Error),
    /// The serialized report is longer than [`MAX_FRAME_BYTES`].
    #[error("report of {0} bytes exceeds the {MAX_FRAME_BYTES}-byte frame limit")]
    TooLarge(u64),
}

/// Everything the guest reports for one job: one result per executed step, in job order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Report {
    v: u32,
    steps: Vec<StepResult>,
    job_error: Option<String>,
}

impl Report {
    /// An empty v1 report: `{"v":1,"steps":[],"job_error":null}`.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            v: PROTOCOL_VERSION,
            steps: Vec::new(),
            job_error: None,
        }
    }

    /// Appends the result of the step that just ran.
    pub fn push(&mut self, result: StepResult) {
        self.steps.push(result);
    }

    /// Records a job-level error.
    pub fn set_job_error(&mut self, error: String) {
        self.job_error = Some(error);
    }

    /// The step results so far, in job order.
    #[must_use]
    pub fn steps(&self) -> &[StepResult] {
        &self.steps
    }
}

impl Default for Report {
    fn default() -> Self {
        Self::new()
    }
}

/// One step's result. Externally tagged by the step key: `{"connect":{...}}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StepResult {
    /// Result of `report_boot`.
    ReportBoot(BootResult),
    /// Result of `connect`.
    Connect(ConnectResult),
    /// Result of `vsock_connect`.
    VsockConnect(VsockConnectResult),
    /// Result of `vsock_stream`.
    VsockStream(VsockStreamResult),
    /// Result of `scratch_roundtrip`.
    ScratchRoundtrip(ScratchResult),
    /// Result of `run`.
    Run(RunResult),
    /// Result of `console_flood`.
    ConsoleFlood(ConsoleFloodResult),
}

/// What the guest saw at boot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BootResult {
    marker: &'static str,
    cmdline: String,
    osrelease: String,
    args: Vec<String>,
    env: Vec<(String, String)>,
    net_ifaces: Vec<String>,
    virtio_devices: Vec<String>,
}

impl BootResult {
    /// Builds the boot result. `marker` is always [`BOOT_MARKER`]. `env`, `net_ifaces`, and
    /// `virtio_devices` are sorted here, so the caller's iteration order never reaches the wire.
    #[must_use]
    pub fn new(
        cmdline: String,
        osrelease: String,
        args: Vec<String>,
        mut env: Vec<(String, String)>,
        mut net_ifaces: Vec<String>,
        mut virtio_devices: Vec<String>,
    ) -> Self {
        env.sort();
        net_ifaces.sort();
        virtio_devices.sort();
        Self {
            marker: BOOT_MARKER,
            cmdline,
            osrelease,
            args,
            env,
            net_ifaces,
            virtio_devices,
        }
    }
}

/// Result of `connect`: one attempt per target, in job order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConnectResult {
    /// The attempts.
    pub results: Vec<ConnectAttempt>,
}

/// One TCP connection attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConnectAttempt {
    /// The target address as the guest formatted it.
    pub target: String,
    /// Whether the connection was established.
    pub ok: bool,
    /// The error text; `None` when `ok`.
    pub error: Option<String>,
    /// The raw OS error number; `None` when `ok` or when the error had none (for example a timeout).
    pub raw_os_error: Option<i32>,
}

/// Result of `vsock_connect`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VsockConnectResult {
    /// The host port tried.
    pub port: u32,
    /// Whether the connection was established.
    pub ok: bool,
    /// The error text; `None` when `ok`.
    pub error: Option<String>,
    /// The raw OS error number; `None` when `ok`.
    pub raw_os_error: Option<i32>,
}

/// Result of `vsock_stream`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VsockStreamResult {
    /// Bytes the job asked for.
    pub requested: u64,
    /// Bytes written before the stream ended or failed.
    pub written: u64,
    /// The first error; `None` if every byte was written.
    pub error: Option<String>,
    /// The raw OS error number of the first error.
    pub raw_os_error: Option<i32>,
}

/// Result of `scratch_roundtrip`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ScratchResult {
    /// Bytes written and read back.
    pub bytes: u64,
    /// Whether the bytes read back equal the bytes written.
    pub ok: bool,
    /// The error, or a description of the mismatch; `None` when `ok`.
    pub error: Option<String>,
}

/// Result of `run`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RunResult {
    /// The argv that ran.
    pub argv: Vec<String>,
    /// The exit status, when the process exited normally.
    pub status: Option<i32>,
    /// The terminating signal, when a signal ended the process.
    pub signal: Option<i32>,
    /// Captured stdout, lossy UTF-8, at most the job's cap.
    pub stdout: String,
    /// Whether stdout was longer than the cap.
    pub stdout_truncated: bool,
    /// Captured stderr, lossy UTF-8, at most the job's cap.
    pub stderr: String,
    /// Whether stderr was longer than the cap.
    pub stderr_truncated: bool,
    /// The spawn or wait error; `None` if the process ran.
    pub error: Option<String>,
}

/// Result of `console_flood`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConsoleFloodResult {
    /// Bytes the job asked for.
    pub requested: u64,
    /// Bytes written before the console failed.
    pub written: u64,
    /// The first error; `None` if every byte was written.
    pub error: Option<String>,
}

/// Serializes `report` as compact JSON and prepends the `MINOSST1` header.
///
/// # Errors
///
/// Returns [`FrameError::Json`] if serialization fails, and [`FrameError::TooLarge`] if the JSON is longer
/// than [`MAX_FRAME_BYTES`].
pub fn encode_frame(report: &Report) -> Result<Vec<u8>, FrameError> {
    let _ = report;
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::{
        BootResult, ConnectAttempt, ConnectResult, FrameError, Report, RunResult, StepResult,
        encode_frame,
    };
    use crate::test_hex;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn empty_report_matches_golden_frame() -> TestResult {
        let mut golden = test_hex::decode("4d494e4f53535431000000010000000000000023")?;
        golden.extend_from_slice(br#"{"v":1,"steps":[],"job_error":null}"#);
        assert_eq!(encode_frame(&Report::new())?, golden);
        Ok(())
    }

    #[test]
    fn refuses_report_over_frame_cap() {
        let mut report = Report::new();
        report.set_job_error("x".repeat(1_048_576));
        assert!(matches!(
            encode_frame(&report),
            Err(FrameError::TooLarge(len)) if len > 1_048_576
        ));
    }

    #[test]
    fn step_results_serialize_in_declared_field_order() -> TestResult {
        let mut report = Report::new();
        report.push(StepResult::ReportBoot(BootResult::new(
            "console=hvc0".to_owned(),
            "6.12.91".to_owned(),
            vec!["/init".to_owned()],
            vec![
                ("TERM".to_owned(), "linux".to_owned()),
                ("HOME".to_owned(), "/".to_owned()),
            ],
            vec!["lo".to_owned()],
            vec!["virtio1:0x0013".to_owned(), "virtio0:0x0002".to_owned()],
        )));
        report.push(StepResult::Connect(ConnectResult {
            results: vec![ConnectAttempt {
                target: "1.1.1.1:443".to_owned(),
                ok: false,
                error: Some("Network is unreachable (os error 101)".to_owned()),
                raw_os_error: Some(101),
            }],
        }));
        report.push(StepResult::Run(RunResult {
            argv: vec!["/bin/true".to_owned()],
            status: Some(0),
            signal: None,
            stdout: String::new(),
            stdout_truncated: false,
            stderr: String::new(),
            stderr_truncated: false,
            error: None,
        }));
        report.set_job_error("late".to_owned());
        let json = serde_json::to_string(&report)?;
        let expected = concat!(
            r#"{"v":1,"steps":["#,
            r#"{"report_boot":{"marker":"MINOS-GUEST-BOOT-OK","cmdline":"console=hvc0","osrelease":"6.12.91","#,
            r#""args":["/init"],"env":[["HOME","/"],["TERM","linux"]],"net_ifaces":["lo"],"#,
            r#""virtio_devices":["virtio0:0x0002","virtio1:0x0013"]}},"#,
            r#"{"connect":{"results":[{"target":"1.1.1.1:443","ok":false,"#,
            r#""error":"Network is unreachable (os error 101)","raw_os_error":101}]}},"#,
            r#"{"run":{"argv":["/bin/true"],"status":0,"signal":null,"stdout":"","stdout_truncated":false,"#,
            r#""stderr":"","stderr_truncated":false,"error":null}}"#,
            r#"],"job_error":"late"}"#,
        );
        assert_eq!(json, expected);
        let frame = encode_frame(&report)?;
        assert_eq!(frame.get(20..), Some(json.as_bytes()));
        Ok(())
    }
}
