//! Every cap only `minos-propose` enforces, each with its unit and reason.
//!
//! Caps shared with other crates stay in [`minos_core::limits`] and are reused here, never redefined:
//! [`RATIONALE_EXCERPT_BYTES`](minos_core::limits::RATIONALE_EXCERPT_BYTES),
//! [`MAX_LOG_EXCERPT_BYTES`](minos_core::limits::MAX_LOG_EXCERPT_BYTES),
//! [`SINGLE_SHOT_SURFACE_MAX_BYTES`],
//! [`MAX_STEER_NOTE_BYTES`](minos_core::limits::MAX_STEER_NOTE_BYTES),
//! [`BATCH_MIN`](minos_core::limits::BATCH_MIN), and [`BATCH_MAX`](minos_core::limits::BATCH_MAX).

use std::time::Duration;

use minos_core::limits::SINGLE_SHOT_SURFACE_MAX_BYTES;

// ---------------------------------------------------------------------------------------------------------------
// Prompt (06-CONTEXT D-07, D-08, D-09)
// ---------------------------------------------------------------------------------------------------------------

/// Generations of history rows in the prompt, besides every accepted winner, which is always shown (D-07).
pub const HISTORY_GENERATIONS: u32 = 8;
/// Largest rendered history table, in bytes. Older rows are dropped first, so the newest results stay (D-07).
pub const HISTORY_TABLE_MAX_BYTES: usize = 32_768;
/// Total failure-excerpt bytes per prompt: four full log excerpts, so a flood of failures cannot push
/// `program.md`'s instructions out of the prompt (D-08).
pub const EXCERPTS_MAX_BYTES: usize = 16_384;
/// Total steer-note bytes per prompt. Older ordinals are dropped first, so the user's latest note stays (D-09).
pub const STEER_NOTES_MAX_BYTES: usize = 16_384;
/// Budget for the fixed instruction block, headings, and fences, in bytes.
pub const PROMPT_OVERHEAD_BYTES: usize = 65_536;
/// Largest whole prompt, in bytes (1 MiB). The CLI caps piped stdin at 10 MB
/// (code.claude.com/docs/en/headless), so a prompt this size always fits.
pub const MAX_PROMPT_BYTES: usize = 1_048_576;

// ---------------------------------------------------------------------------------------------------------------
// Returned proposals (D-10, D-12)
// ---------------------------------------------------------------------------------------------------------------

/// Longest rationale a model may return, in characters: the output schema's `maxLength`, which JSON Schema counts
/// in characters, not bytes, so the parser counts characters against this cap too and every schema-valid
/// rationale passes it (review IN-01).
pub const MAX_RATIONALE_CHARS: usize = 2_000;
/// Byte ceiling of a rationale, for storage: a UTF-8 character is at most 4 bytes, so every rationale within
/// [`MAX_RATIONALE_CHARS`] fits, and the parser refuses a string over this ceiling before counting its characters.
pub const MAX_RATIONALE_BYTES: usize = MAX_RATIONALE_CHARS * 4;
/// Largest content of one returned file, in bytes: equal to core's single-shot surface cap, because the whole
/// surface had to fit the prompt for the tier to be eligible.
pub const MAX_RETURNED_FILE_BYTES: usize = 262_144;

// ---------------------------------------------------------------------------------------------------------------
// Provider process (D-16)
// ---------------------------------------------------------------------------------------------------------------

/// Deadline of one model call. Above the CLI's 10-minute per-request timeout plus its retry backoff; it bounds
/// one call, not the CLI's whole retry budget of up to 10 retries. On expiry the process group is killed and the
/// call is an infrastructure failure (D-16).
pub const CALL_DEADLINE: Duration = Duration::from_mins(15);
/// Deadline of each `--version` and `auth status` preflight run.
pub const PREFLIGHT_DEADLINE: Duration = Duration::from_secs(30);
/// Most stdout bytes read from a model call (4 MiB). This bounds what the provider reads, not what the schema
/// permits: a batch of [`BATCH_MAX`](minos_core::limits::BATCH_MAX) proposals, each with every writable file at
/// [`MAX_RETURNED_FILE_BYTES`], carried twice (as the object and as the `result` string), is far larger. A reply
/// over this cap is therefore the model's choice and an empty call (`EmptyCause::OutputTooLarge`), never an
/// infrastructure failure; the per-file cap on what does arrive is enforced by `parse_output`.
pub const MAX_ENVELOPE_BYTES: u64 = 4_194_304;
/// Most stdout bytes read from `--version` and `auth status`, which print a line or a small JSON object.
pub const MAX_PREFLIGHT_STDOUT_BYTES: u64 = 16_384;
/// Most stderr bytes kept from any provider process; the rest is drained and dropped, and stderr is never shown
/// raw.
pub const MAX_STDERR_BYTES: u64 = 65_536;
/// How long after the process group is killed each pipe thread keeps waiting for its pipe to close before it
/// abandons the pipe with what it has. A descendant that left the group (`setsid`, `setpgid`) survives the kill
/// and can hold a pipe open; this grace is what bounds the call then, so a call returns within its deadline plus
/// this grace (review WR-04).
pub const DRAIN_GRACE: Duration = Duration::from_secs(5);
/// The process watcher's poll period, as `minos-cli`'s image builder uses.
pub const POLL_INTERVAL: Duration = Duration::from_millis(20);

// ---------------------------------------------------------------------------------------------------------------
// Compile-time relations between the caps
// ---------------------------------------------------------------------------------------------------------------

/// The CLI's piped-stdin cap, in bytes (code.claude.com/docs/en/headless).
const CLI_STDIN_CAP_BYTES: u64 = 10_000_000;

const _: () = assert!((MAX_PROMPT_BYTES as u64) < CLI_STDIN_CAP_BYTES);
const _: () = assert!(
    SINGLE_SHOT_SURFACE_MAX_BYTES
        + HISTORY_TABLE_MAX_BYTES as u64
        + EXCERPTS_MAX_BYTES as u64
        + STEER_NOTES_MAX_BYTES as u64
        + PROMPT_OVERHEAD_BYTES as u64
        <= MAX_PROMPT_BYTES as u64
);
const _: () = assert!(MAX_RETURNED_FILE_BYTES as u64 == SINGLE_SHOT_SURFACE_MAX_BYTES);
