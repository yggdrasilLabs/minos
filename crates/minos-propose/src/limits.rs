//! Every cap only `minos-propose` enforces, each with its unit and reason.
//!
//! Caps shared with other crates stay in [`minos_core::limits`] and are reused here, never redefined:
//! [`RATIONALE_EXCERPT_BYTES`](minos_core::limits::RATIONALE_EXCERPT_BYTES),
//! [`MAX_LOG_EXCERPT_BYTES`](minos_core::limits::MAX_LOG_EXCERPT_BYTES),
//! [`SINGLE_SHOT_SURFACE_MAX_BYTES`],
//! [`MAX_STEER_NOTE_BYTES`](minos_core::limits::MAX_STEER_NOTE_BYTES),
//! [`BATCH_MIN`](minos_core::limits::BATCH_MIN), and [`BATCH_MAX`](minos_core::limits::BATCH_MAX).

use std::time::Duration;

use minos_core::limits::{
    MAX_GATES, MAX_NAME_BYTES, MAX_OBJECTIVES, MAX_PATH_BYTES, MAX_READABLE, MAX_WRITABLE,
    SINGLE_SHOT_SURFACE_MAX_BYTES,
};

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
/// Most failure excerpts per prompt, the newest first. The byte cap bounds their text, not their number, and each
/// excerpt's fence and label cost up to [`EXCERPT_BLOCK_OVERHEAD_BYTES`] whatever its length, so the count is
/// capped too; an excerpt whose sanitized text is empty is skipped rather than counted (review IN-03).
pub const EXCERPTS_MAX_COUNT: usize = 32;
/// Total steer-note bytes per prompt. Older ordinals are dropped first, so the user's latest note stays (D-09).
pub const STEER_NOTES_MAX_BYTES: usize = 16_384;
/// Most steer notes per prompt, the highest ordinals first, because each note's fence costs
/// [`NOTE_BLOCK_OVERHEAD_BYTES`] whatever its length (review IN-03).
pub const STEER_NOTES_MAX_COUNT: usize = 64;
/// Budget, in bytes, for everything in a prompt that is neither content under one of the four content caps nor
/// proportional to the surface: the fixed instruction block and headings (`FIXED_TEXT_MAX_BYTES`), the objective
/// and gate lines (`JUDGE_LINES_MAX_BYTES`), the `HISTORY` fence, [`EXCERPTS_MAX_COUNT`] excerpt fences with
/// their labels at [`EXCERPT_BLOCK_OVERHEAD_BYTES`] each, and [`STEER_NOTES_MAX_COUNT`] note fences at
/// [`NOTE_BLOCK_OVERHEAD_BYTES`] each. The assertions below prove that sum fits, and that this budget plus the
/// surface-proportional fences (`SURFACE_OVERHEAD_MAX_BYTES`) and the four content caps fit [`MAX_PROMPT_BYTES`].
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

/// Bytes a fenced block costs besides its content, kind, and label: `<<<BEGIN `, the space before the token, the
/// 16-hex-character token, `>>>\n`, the newline added when the content lacks one, `<<<END `, the space, the
/// token, and `>>>\n` (`prompt::push_block`).
const FENCE_FIXED_BYTES: usize = 9 + 1 + 16 + 4 + 1 + 7 + 1 + 16 + 4;

/// The bytes a block of `kind` with a label of at most `label_max` bytes costs besides its content: the kind is on
/// both lines, and a label is followed by one space.
const fn block_overhead(kind: &str, label_max: usize) -> usize {
    FENCE_FIXED_BYTES + 2 * kind.len() + label_max + 1
}

/// Digits of the longest `u32`.
const U32_DIGITS: usize = 10;

/// The longest excerpt label: `gen <u32> exp <12 hex> ` and the longest unscored text, `rejected: ` plus the
/// longest `Reason` display, `hunk <u32> of <path> does not apply` with the path at [`MAX_PATH_BYTES`]
/// (`prompt::history::render_row`).
const EXCERPT_LABEL_MAX_BYTES: usize =
    4 + U32_DIGITS + 5 + 12 + 1 + 10 + (5 + U32_DIGITS + 4 + MAX_PATH_BYTES + 15);

/// Most bytes one `USER NOTE` block costs besides its text: the fence and the ordinal label.
pub const NOTE_BLOCK_OVERHEAD_BYTES: usize = block_overhead("USER NOTE", U32_DIGITS);

/// Most bytes one `UNTRUSTED SANDBOX OUTPUT` block costs besides its text: the fence and the longest label.
pub const EXCERPT_BLOCK_OVERHEAD_BYTES: usize =
    block_overhead("UNTRUSTED SANDBOX OUTPUT", EXCERPT_LABEL_MAX_BYTES);

/// Most bytes one `FILE` block costs besides its content: the fence, the path, and ` final-newline: yes`.
const FILE_BLOCK_OVERHEAD_BYTES: usize =
    block_overhead("FILE", MAX_PATH_BYTES + " final-newline: yes".len());

/// Prompt bytes proportional to the surface besides the file contents, which [`SINGLE_SHOT_SURFACE_MAX_BYTES`]
/// caps: the writable list (`- <path>\n` per file), the program heading (a blank line, then
/// `## Instructions from the project (<path>)\n`) and its `PROGRAM` fence, and one `FILE` fence per surface file, every path at
/// [`MAX_PATH_BYTES`].
const SURFACE_OVERHEAD_MAX_BYTES: usize = MAX_WRITABLE * (2 + MAX_PATH_BYTES + 1)
    + (37 + MAX_PATH_BYTES)
    + block_overhead("PROGRAM", MAX_PATH_BYTES)
    + (MAX_WRITABLE + MAX_READABLE) * FILE_BLOCK_OVERHEAD_BYTES;

/// The fixed instruction block, every heading, `None yet.`, and the two omitted-count lines: about 1.5 KiB as
/// written, budgeted generously so wording can grow without touching the proof.
const FIXED_TEXT_MAX_BYTES: usize = 8_192;

/// The objective lines (`- <name>: maximize\n`) and gate lines (`- <name>\n`), names at [`MAX_NAME_BYTES`].
const JUDGE_LINES_MAX_BYTES: usize =
    MAX_OBJECTIVES * (2 + MAX_NAME_BYTES + 11) + MAX_GATES * (2 + MAX_NAME_BYTES + 1);

const _: () = assert!((MAX_PROMPT_BYTES as u64) < CLI_STDIN_CAP_BYTES);
// What PROMPT_OVERHEAD_BYTES covers fits in it: the fixed text, the judge lines, the HISTORY fence, and every
// excerpt and note fence at their count caps (review IN-03).
const _: () = assert!(
    FIXED_TEXT_MAX_BYTES
        + JUDGE_LINES_MAX_BYTES
        + block_overhead("HISTORY", 0)
        + EXCERPTS_MAX_COUNT * EXCERPT_BLOCK_OVERHEAD_BYTES
        + STEER_NOTES_MAX_COUNT * NOTE_BLOCK_OVERHEAD_BYTES
        <= PROMPT_OVERHEAD_BYTES
);
// The four content caps, the surface-proportional fences, and the overhead budget fit the prompt cap.
const _: () = assert!(
    SINGLE_SHOT_SURFACE_MAX_BYTES
        + SURFACE_OVERHEAD_MAX_BYTES as u64
        + HISTORY_TABLE_MAX_BYTES as u64
        + EXCERPTS_MAX_BYTES as u64
        + STEER_NOTES_MAX_BYTES as u64
        + PROMPT_OVERHEAD_BYTES as u64
        <= MAX_PROMPT_BYTES as u64
);
const _: () = assert!(MAX_RETURNED_FILE_BYTES as u64 == SINGLE_SHOT_SURFACE_MAX_BYTES);
