//! Safe rendering of hostile text: the `SafeText` allowlist renderer and `LogExcerpt` (CORE-15).
//!
//! Every string that originated in a sandbox or a model (logs, file names, rationales, diffs, model output)
//! passes through [`SafeText`] before it is displayed or written into a report (CLAUDE.md "Report and adopt").
//! The renderer is an allowlist, not an escape-sequence parser:
//!
//! - newline and tab are kept;
//! - every other control character is dropped: C0 (U+0000 to U+001F, including ESC and CR), DEL (U+007F), and
//!   C1 (U+0080 to U+009F, including the one-byte CSI U+009B);
//! - invisible format characters ([`is_invisible_format`]) are shown as `<U+XXXX>`;
//! - everything else is kept.
//!
//! Dropping ESC alone leaves inert visible residue, such as `[31m` from a color sequence or `]52;c;...` from a
//! clipboard write. That is intended: the user sees that something was there, and nothing acts on their
//! terminal. A carriage return cannot overwrite a line, and a bidi override cannot reorder what is shown.

use std::fmt;

use crate::limits::MAX_LOG_EXCERPT_BYTES;

/// Whether `c` is an invisible format character that [`SafeText`] shows as `<U+XXXX>`.
///
/// Std Rust has no Unicode general-category API beyond `char::is_control`, so the set is an explicit table. It
/// holds every General Category `Cf` (format) code point of Unicode 16.0 plus a few invisible marks, and a test
/// pins the exact set, so a future Unicode addition is a deliberate edit here:
///
/// - bidi controls (Trojan Source, CVE-2021-42574): U+061C, U+200E, U+200F, U+202A to U+202E, U+2066 to
///   U+2069, and the deprecated U+206A to U+206F;
/// - zero-width and other invisible characters: U+00AD soft hyphen, U+034F combining grapheme joiner, U+180E,
///   U+200B to U+200D, U+2060 to U+2064, U+FEFF byte order mark;
/// - number and mark signs that are `Cf`: U+0600 to U+0605, U+06DD, U+070F, U+0890, U+0891, U+08E2 (Arabic
///   and Syriac), U+110BD and U+110CD (Kaithi);
/// - other format controls: U+FFF9 to U+FFFB (interlinear annotation), U+13430 to U+1343F (Egyptian
///   hieroglyph), U+1BCA0 to U+1BCA3 (shorthand), U+1D173 to U+1D17A (musical symbol);
/// - Hangul fillers that render as blank space: U+115F, U+1160, U+3164, U+FFA0;
/// - line and paragraph separators U+2028 and U+2029, which some terminals and editors treat as line breaks;
/// - variation selectors U+180B to U+180F (Mongolian), U+FE00 to U+FE0F, and U+E0100 to U+E01EF;
/// - tag characters U+E0000 to U+E007F, used for "ASCII smuggling" of hidden instructions into models.
#[must_use]
pub const fn is_invisible_format(c: char) -> bool {
    matches!(
        c,
        '\u{00AD}'
            | '\u{034F}'
            | '\u{0600}'..='\u{0605}'
            | '\u{061C}'
            | '\u{06DD}'
            | '\u{070F}'
            | '\u{0890}'..='\u{0891}'
            | '\u{08E2}'
            | '\u{115F}'
            | '\u{1160}'
            | '\u{180B}'..='\u{180F}'
            | '\u{200B}'..='\u{200F}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2060}'..='\u{2064}'
            | '\u{2066}'..='\u{206F}'
            | '\u{3164}'
            | '\u{FE00}'..='\u{FE0F}'
            | '\u{FEFF}'
            | '\u{FFA0}'
            | '\u{FFF9}'..='\u{FFFB}'
            | '\u{110BD}'
            | '\u{110CD}'
            | '\u{13430}'..='\u{1343F}'
            | '\u{1BCA0}'..='\u{1BCA3}'
            | '\u{1D173}'..='\u{1D17A}'
            | '\u{E0000}'..='\u{E007F}'
            | '\u{E0100}'..='\u{E01EF}'
    )
}

/// Text that is safe to print to a terminal or put in a report.
///
/// The only constructors are [`SafeText::new`], [`SafeText::from_utf8_lossy`], and [`SafeText::inline`], and
/// each one sanitizes, so a `SafeText` that exists holds no control character other than newline and tab and no
/// invisible format character. There is no `Deref` and no `From<String>`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SafeText(String);

/// Appends `c` as `<U+XXXX>`: uppercase hex, at least four digits.
fn push_visible(out: &mut String, c: char) {
    let value = u32::from(c);
    let digits = (u32::BITS - value.leading_zeros()).div_ceil(4).max(4);
    out.push_str("<U+");
    for place in (0..digits).rev() {
        let nibble = (value >> (place * 4)) & 0xF;
        out.extend(char::from_digit(nibble, 16).map(|digit| digit.to_ascii_uppercase()));
    }
    out.push('>');
}

/// The sanitized form of `input`. When `inline` is set, newline and tab are shown as `<U+000A>` and `<U+0009>`
/// so the text stays on one line.
fn render(input: &str, inline: bool) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        if c == '\n' || c == '\t' {
            if inline {
                push_visible(&mut out, c);
            } else {
                out.push(c);
            }
        } else if c.is_control() {
            // Dropped: C0, DEL, and C1, including ESC and CR.
        } else if is_invisible_format(c) {
            push_visible(&mut out, c);
        } else {
            out.push(c);
        }
    }
    out
}

impl SafeText {
    /// Sanitizes `input`, keeping newlines and tabs.
    #[must_use]
    pub fn new(input: &str) -> Self {
        Self(render(input, false))
    }

    /// Decodes `bytes` as UTF-8, replacing invalid sequences with U+FFFD, then sanitizes, keeping newlines and
    /// tabs.
    #[must_use]
    pub fn from_utf8_lossy(bytes: &[u8]) -> Self {
        Self::new(&String::from_utf8_lossy(bytes))
    }

    /// Sanitizes `input` for a single line: newline and tab are shown as `<U+000A>` and `<U+0009>`.
    #[must_use]
    pub fn inline(input: &str) -> Self {
        Self(render(input, true))
    }

    /// The sanitized text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Length of the sanitized text, in bytes.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether the sanitized text is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for SafeText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The sanitized tail of a log, at most [`MAX_LOG_EXCERPT_BYTES`] bytes after sanitization.
///
/// Phase 4's `Untrusted::into_log_excerpt` calls [`LogExcerpt::from_bytes`]; nothing else turns raw log bytes into
/// displayable text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogExcerpt {
    text: SafeText,
    truncated: bool,
}

/// Most UTF-8 continuation bytes that can follow a cut inside one character.
const MAX_CONTINUATION_BYTES: usize = 3;

impl LogExcerpt {
    /// Keeps the last `max_bytes` bytes of `bytes` (clamped to [`MAX_LOG_EXCERPT_BYTES`]), decodes them lossily,
    /// and sanitizes them.
    ///
    /// The tail is where a build or run failure is reported, so the head is what gets dropped. A cut inside a
    /// UTF-8 character drops that character's leftover continuation bytes. If showing invisible characters as
    /// `<U+XXXX>` makes the text longer than the cap, leading characters are dropped at character boundaries
    /// until it fits.
    #[must_use]
    pub fn from_bytes(bytes: &[u8], max_bytes: usize) -> Self {
        let cap = max_bytes.min(MAX_LOG_EXCERPT_BYTES);
        let start = bytes.len().saturating_sub(cap);
        let mut truncated = start > 0;
        let mut tail = bytes.get(start..).unwrap_or_default();
        if truncated {
            let leftover = tail
                .iter()
                .take(MAX_CONTINUATION_BYTES)
                .take_while(|byte| **byte & 0xC0 == 0x80)
                .count();
            tail = tail.get(leftover..).unwrap_or_default();
        }
        let mut text = SafeText::from_utf8_lossy(tail);
        if text.len() > cap {
            let excess = text.len() - cap;
            let cut = text
                .0
                .char_indices()
                .map(|(index, _)| index)
                .find(|index| *index >= excess)
                .unwrap_or(text.0.len());
            text = SafeText(text.0.get(cut..).unwrap_or_default().to_owned());
            truncated = true;
        }
        Self { text, truncated }
    }

    /// The sanitized excerpt.
    #[must_use]
    pub const fn text(&self) -> &SafeText {
        &self.text
    }

    /// Whether anything was dropped from the head of the log.
    #[must_use]
    pub const fn is_truncated(&self) -> bool {
        self.truncated
    }
}
