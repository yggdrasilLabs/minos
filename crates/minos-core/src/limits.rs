//! Every cap, default, and bound the core enforces, each with its unit and reason.
//!
//! A cap that only one module enforces is declared at the top of that module, with its reason, in the same style.
//! A bound shared across crates lives here, because `minos-core` is the one crate every other crate may link: the
//! VM machine bounds and memory budget (05-CONTEXT D-05, D-06) and the guest output caps.

// ---------------------------------------------------------------------------------------------------------------
// Paths
// ---------------------------------------------------------------------------------------------------------------

/// Longest relative path, in bytes. Far above any real source path, and small enough that a hostile path cannot
/// make path handling expensive.
pub const MAX_PATH_BYTES: usize = 1024;
/// Most components in a relative path. Real source trees are far shallower; the cap bounds component walks.
pub const MAX_PATH_DEPTH: usize = 32;
/// Longest single path component, in bytes: the common filesystem name limit (`NAME_MAX`).
pub const MAX_COMPONENT_BYTES: usize = 255;

// ---------------------------------------------------------------------------------------------------------------
// Names
// ---------------------------------------------------------------------------------------------------------------

/// Longest metric, param, or project name, in bytes. Names appear in reports and prompts, so they stay short.
pub const MAX_NAME_BYTES: usize = 64;
/// Longest model identifier, in bytes. Long enough for a Bedrock inference-profile ID such as
/// `us.anthropic.claude-...-v1:0`.
pub const MAX_MODEL_ID_BYTES: usize = 128;

// ---------------------------------------------------------------------------------------------------------------
// Diffs
// ---------------------------------------------------------------------------------------------------------------

/// Largest diff text, in bytes (1 MiB). A proposal is model output and hostile; the cap is checked before parsing.
pub const MAX_DIFF_BYTES: usize = 1_048_576;
/// Most lines in a diff text. Bounds the parse and apply work independently of line length.
pub const MAX_DIFF_LINES: usize = 65_536;
/// Most files one diff may touch. Also bounded by [`MAX_WRITABLE`], since every touched file must be writable.
pub const MAX_DIFF_FILES: usize = 64;
/// Most hunks in one file section of a diff. Bounds the applier's per-file work.
pub const MAX_HUNKS_PER_FILE: usize = 256;

// ---------------------------------------------------------------------------------------------------------------
// Manifest surface and commands
// ---------------------------------------------------------------------------------------------------------------

/// Most `[surface] writable` entries. Each names exactly one file (03-CONTEXT D-01).
pub const MAX_WRITABLE: usize = 64;
/// Most `[surface] readable` entries.
pub const MAX_READABLE: usize = 256;
/// Most `[source] exclude` entries.
pub const MAX_EXCLUDES: usize = 256;
/// Most argv entries in one manifest command. Matches the guest's own cap (`minos-guest` `job::MAX_ARGV_ENTRIES`).
pub const MAX_ARGV_ENTRIES: usize = 64;
/// Longest single argv entry, in bytes. Matches the guest's own cap (`minos-guest` `job::MAX_ARG_BYTES`).
pub const MAX_ARG_BYTES: usize = 4096;

// ---------------------------------------------------------------------------------------------------------------
// Stage budgets
// ---------------------------------------------------------------------------------------------------------------

/// Shortest build, run, or judge budget, in milliseconds. A budget below one second cannot boot a VM.
pub const MIN_STAGE_MS: u64 = 1_000;
/// Longest build, run, or judge budget, in milliseconds: one hour, the launcher's own `max_duration_ms` ceiling.
pub const MAX_STAGE_MS: u64 = 3_600_000;
/// Judge budget when the manifest names none, in milliseconds (two minutes).
pub const DEFAULT_JUDGE_MS: u64 = 120_000;

// ---------------------------------------------------------------------------------------------------------------
// Spend and tokens
// ---------------------------------------------------------------------------------------------------------------

/// Largest `[budget] spend_usd`, in whole US dollars. A typo of extra zeros must not lift the cap out of reach.
pub const MAX_SPEND_USD: u64 = 10_000;
/// Smallest per-run token cap. Below this no proposal call can complete.
pub const TOKEN_CAP_MIN: u64 = 1_000;
/// Largest per-run token cap.
pub const TOKEN_CAP_MAX: u64 = 1_000_000_000;
/// Per-run token cap when the manifest names none.
pub const TOKEN_CAP_DEFAULT: u64 = 5_000_000;

// ---------------------------------------------------------------------------------------------------------------
// Policy and proposers
// ---------------------------------------------------------------------------------------------------------------

/// Smallest `[policy] width`: at least one candidate per generation.
pub const WIDTH_MIN: u32 = 1;
/// Largest `[policy] width`: concurrent VMs on one machine are bounded by memory.
pub const WIDTH_MAX: u32 = 16;
/// Smallest proposer `batch`.
pub const BATCH_MIN: u32 = 1;
/// Largest proposer `batch`: proposals per model call.
pub const BATCH_MAX: u32 = 16;
/// Most `[[proposer]]` entries: one per tier (sweep, single-shot, planned, agentic).
pub const MAX_PROPOSERS: usize = 4;
/// Most `[[judge.objective]]` entries.
pub const MAX_OBJECTIVES: usize = 8;
/// Most `[[judge.gate]]` entries.
pub const MAX_GATES: usize = 16;
/// Largest `min_improvement` percentage. Anything above ten times the baseline is a typo.
pub const MAX_MIN_IMPROVEMENT_PERCENT: u32 = 1_000;
/// Largest `[judge] version` label: a local integer naming pushed judge data.
pub const MAX_JUDGE_LABEL: u32 = 1_000_000;

// ---------------------------------------------------------------------------------------------------------------
// Routing and circuit breaker (03-CONTEXT D-13, D-14). These are manifest fields with defaults; each minimum
// stops a value that would disable routing or the breaker.
// ---------------------------------------------------------------------------------------------------------------

/// Smallest `[policy] stall_step`, in generations without improvement before routing moves up one tier.
pub const STALL_STEP_MIN: u32 = 1;
/// Largest `[policy] stall_step`, in generations.
pub const STALL_STEP_MAX: u32 = 100;
/// `[policy] stall_step` when the manifest names none, in generations.
pub const STALL_STEP_DEFAULT: u32 = 3;
/// Smallest `[budget] max_empty_generations`: consecutive generations with no valid candidate before the breaker
/// stops the run. Zero would disable the breaker.
pub const EMPTY_GENERATIONS_MIN: u32 = 1;
/// Largest `[budget] max_empty_generations`.
pub const EMPTY_GENERATIONS_MAX: u32 = 20;
/// `[budget] max_empty_generations` when the manifest names none.
pub const EMPTY_GENERATIONS_DEFAULT: u32 = 3;
/// Smallest `[budget] max_infra_failures`: consecutive infrastructure failures before the breaker stops the run.
/// Zero would disable the breaker.
pub const INFRA_FAILURES_MIN: u32 = 1;
/// Largest `[budget] max_infra_failures`.
pub const INFRA_FAILURES_MAX: u32 = 20;
/// `[budget] max_infra_failures` when the manifest names none.
pub const INFRA_FAILURES_DEFAULT: u32 = 3;

// ---------------------------------------------------------------------------------------------------------------
// Sweep params
// ---------------------------------------------------------------------------------------------------------------

/// Most declared sweep params.
pub const MAX_PARAMS: usize = 32;
/// Most choices in one categorical param.
pub const MAX_CHOICES: usize = 64;
/// Longest single choice value, in bytes.
pub const MAX_CHOICE_BYTES: usize = 256;
/// Longest anchor text that locates a param in a surface file, in bytes.
pub const MAX_ANCHOR_BYTES: usize = 256;
/// Most points in one param's domain. Bounds the sampler's range mapping.
pub const MAX_PARAM_POINTS: u64 = 1_000_000;

// ---------------------------------------------------------------------------------------------------------------
// Scorecards and logs
// ---------------------------------------------------------------------------------------------------------------

/// Most metrics in one scorecard.
pub const MAX_METRICS: usize = 64;
/// Largest scorecard, in bytes: one 4 KiB vsock chunk (02-SPIKE-EVIDENCE.md S03-ANSWER).
pub const MAX_SCORECARD_BYTES: usize = 4096;
/// Largest log excerpt kept from sandbox output, in bytes, after truncation and control stripping.
pub const MAX_LOG_EXCERPT_BYTES: usize = 4096;

// ---------------------------------------------------------------------------------------------------------------
// Trials
// ---------------------------------------------------------------------------------------------------------------

/// Most judge trials of one experiment. Noisy metrics are measured repeatedly, but each trial costs a VM run.
pub const MAX_TRIALS: u32 = 32;

// ---------------------------------------------------------------------------------------------------------------
// Noise and report (03-CONTEXT D-12, D-16)
// ---------------------------------------------------------------------------------------------------------------

/// Fewest baseline trials from which a noise floor (a spread) can be measured.
pub const MIN_NOISE_TRIALS: u32 = 2;
/// The noise check refuses a run whose `min_improvement` is at most this multiple of the baseline spread: the
/// threshold sits inside the noise (D-12).
pub const NOISE_REFUSE_MULTIPLE: f64 = 1.0;
/// The noise check warns when `min_improvement` is at most this multiple of the baseline spread (D-12).
pub const NOISE_WARN_MULTIPLE: f64 = 2.0;
/// The report flags an improvement as anomalous when its delta over the parent exceeds this multiple of the
/// baseline spread, or of the largest earlier accepted delta when no floor was recorded (D-16).
pub const ANOMALY_MULTIPLE: f64 = 10.0;
/// Shortest run of base64 or hex characters in added lines that the report flags as an encoded blob, in bytes.
pub const ENCODED_RUN_MIN: usize = 64;
/// Shortest added literal that the report matches against visible surface inputs, in bytes. Shorter literals
/// match by chance.
pub const LITERAL_MIN_BYTES: usize = 6;
/// Longest excerpt of a proposal rationale shown in the report, in bytes.
pub const RATIONALE_EXCERPT_BYTES: usize = 200;

// ---------------------------------------------------------------------------------------------------------------
// Prompt and steering
// ---------------------------------------------------------------------------------------------------------------

/// Largest total surface size for which the single-shot tier is eligible, in bytes (256 KiB): the surface files
/// must fit in one prompt.
pub const SINGLE_SHOT_SURFACE_MAX_BYTES: u64 = 262_144;
/// Longest `minos steer` note, in bytes.
pub const MAX_STEER_NOTE_BYTES: usize = 4096;

// ---------------------------------------------------------------------------------------------------------------
// VM machine and memory budget (05-CONTEXT D-05, D-06)
//
// The machine bounds live here, not in `minos-sandbox`, because `Manifest::try_from` range-checks
// `[environment] memory_mib` against them and `minos-core` depends on no internal crate (05-RESEARCH Pitfall 3).
// `crates/minos-vmm/src/config.rs` restates them under the WR-02 paired tests, because minos-vmm links no
// internal crate either.
// ---------------------------------------------------------------------------------------------------------------

/// Most vCPUs a sandbox may have: the launcher refuses more.
pub const MAX_VCPUS: u8 = 8;
/// Least guest memory a sandbox may have, in MiB: the guest kernel and `/init` need at least this much.
pub const MIN_MEMORY_MIB: u16 = 128;
/// Most guest memory a sandbox may have, in MiB: the launcher refuses more.
pub const MAX_MEMORY_MIB: u16 = 16_384;
/// Memory held back for the guest kernel, PID 1, and the page cache, in MiB. The Phase 2 boots ran in 256 MiB
/// VMs.
pub const GUEST_RESERVE_MIB: u16 = 256;
/// Least address space left for the candidate, in MiB. An address-space limit below this breaks ordinary
/// toolchains (05-RESEARCH Pitfall 10).
pub const MIN_CANDIDATE_ALLOWANCE_MIB: u16 = 256;
/// Smallest `[environment] scratch_mib`: the tmpfs scratch cap, in MiB.
///
/// This floor is below what a role's largest inputs need (up to 64 MiB of base tree for a candidate; up to 96
/// MiB of judge data plus 16 MiB of candidate output for a judge), and the manifest does not refuse a scratch
/// smaller than a project's actual inputs, because it reads no file. A scratch that fills while the guest
/// unpacks is reported as `guest::SetupStep::ScratchFull`, a configuration error the host must report against
/// `scratch_mib` and never retry (review WR-02).
pub const MIN_SCRATCH_MIB: u16 = 64;
/// Largest `[environment] scratch_mib`, in MiB: the largest VM less the guest reserve and the least candidate
/// allowance (15872).
pub const MAX_SCRATCH_MIB: u16 = MAX_MEMORY_MIB - GUEST_RESERVE_MIB - MIN_CANDIDATE_ALLOWANCE_MIB;
/// `[environment] memory_mib` when the manifest names none, in MiB.
///
/// Recorded assumption (05-RESEARCH A6, Open Question 3), flagged for the user to confirm: generous enough for
/// one Rust build of the sample project; a project sets its own values.
pub const DEFAULT_MEMORY_MIB: u16 = 2048;
/// `[environment] scratch_mib` when the manifest names none, in MiB.
///
/// Recorded assumption (05-RESEARCH A6, Open Question 3), flagged for the user to confirm: generous enough for
/// one Rust build of the sample project; a project sets its own values.
pub const DEFAULT_SCRATCH_MIB: u16 = 1024;

/// The candidate's memory allowance, in MiB, or `None` when the budget does not fit.
///
/// A VM's memory is a budget (05-CONTEXT D-05): the guest reserve ([`GUEST_RESERVE_MIB`]), plus the tmpfs
/// scratch cap (`scratch_mib`), plus the candidate allowance, which is what remains and must be at least
/// [`MIN_CANDIDATE_ALLOWANCE_MIB`]. The guest sets the candidate's `RLIMIT_AS` to the allowance. The arithmetic
/// is checked, so a scratch cap larger than the memory is `None`, never a wrapped value.
#[must_use]
pub const fn candidate_allowance_mib(memory_mib: u16, scratch_mib: u16) -> Option<u16> {
    let Some(after_reserve) = memory_mib.checked_sub(GUEST_RESERVE_MIB) else {
        return None;
    };
    let Some(allowance) = after_reserve.checked_sub(scratch_mib) else {
        return None;
    };
    if allowance >= MIN_CANDIDATE_ALLOWANCE_MIB {
        Some(allowance)
    } else {
        None
    }
}

const _: () = assert!(
    candidate_allowance_mib(DEFAULT_MEMORY_MIB, DEFAULT_SCRATCH_MIB).is_some(),
    "the default memory and scratch sizes fit the D-05 budget"
);
const _: () = assert!(
    MIN_SCRATCH_MIB <= MAX_SCRATCH_MIB,
    "the scratch bounds are ordered"
);

// ---------------------------------------------------------------------------------------------------------------
// Output caps
//
// The guest's output packer and the host's reader share these; `minos-sandbox` re-exports them (plan 05-06).
// ---------------------------------------------------------------------------------------------------------------

/// Most content bytes, summed over every file of one output: 16 MiB. Logs are capped by the guest, a scorecard is
/// 4 KiB, and a diff 1 MiB, so this leaves room for artifacts. It is 4096 vsock chunks of 4 KiB, whose throughput
/// is unmeasured (02-SPIKE-EVIDENCE.md S03-ANSWER).
pub const MAX_OUTPUT_CONTENT_BYTES: u64 = 16 * 1024 * 1024;

/// Most bytes in one file: 8 MiB, so one artifact cannot take the whole budget.
pub const MAX_OUTPUT_ENTRY_BYTES: u64 = 8 * 1024 * 1024;

/// Most files in one output: bounds the per-entry work and the size of the map.
pub const MAX_OUTPUT_ENTRIES: usize = 1024;

/// Most bytes in one output stream: the content cap, plus one header block and up to 511 padding bytes per entry,
/// plus the two end blocks. A stream any longer cannot be a valid output.
pub const MAX_OUTPUT_STREAM_BYTES: u64 =
    MAX_OUTPUT_CONTENT_BYTES + (MAX_OUTPUT_ENTRIES as u64 + 2) * 1024;

/// The caps the core applies to hostile input: diffs, scorecards, and logs.
///
/// There is no setter and no constructor other than [`Limits::standard`], so a caller cannot raise a cap on
/// hostile input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    diff_bytes: usize,
    diff_lines: usize,
    diff_files: usize,
    hunks_per_file: usize,
    scorecard_bytes: usize,
    metrics: usize,
    log_excerpt_bytes: usize,
}

impl Limits {
    /// The standard caps, filled from the constants in this module.
    #[must_use]
    pub const fn standard() -> Self {
        Self {
            diff_bytes: MAX_DIFF_BYTES,
            diff_lines: MAX_DIFF_LINES,
            diff_files: MAX_DIFF_FILES,
            hunks_per_file: MAX_HUNKS_PER_FILE,
            scorecard_bytes: MAX_SCORECARD_BYTES,
            metrics: MAX_METRICS,
            log_excerpt_bytes: MAX_LOG_EXCERPT_BYTES,
        }
    }

    /// Largest diff text, in bytes.
    #[must_use]
    pub const fn max_diff_bytes(&self) -> usize {
        self.diff_bytes
    }

    /// Most lines in a diff text.
    #[must_use]
    pub const fn max_diff_lines(&self) -> usize {
        self.diff_lines
    }

    /// Most files one diff may touch.
    #[must_use]
    pub const fn max_diff_files(&self) -> usize {
        self.diff_files
    }

    /// Most hunks in one file section of a diff.
    #[must_use]
    pub const fn max_hunks_per_file(&self) -> usize {
        self.hunks_per_file
    }

    /// Largest scorecard, in bytes.
    #[must_use]
    pub const fn max_scorecard_bytes(&self) -> usize {
        self.scorecard_bytes
    }

    /// Most metrics in one scorecard.
    #[must_use]
    pub const fn max_metrics(&self) -> usize {
        self.metrics
    }

    /// Largest log excerpt, in bytes.
    #[must_use]
    pub const fn max_log_excerpt_bytes(&self) -> usize {
        self.log_excerpt_bytes
    }
}

impl Default for Limits {
    fn default() -> Self {
        Self::standard()
    }
}
