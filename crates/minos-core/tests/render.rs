//! The `SafeText` allowlist renderer against an escape corpus (CSI, OSC 52, OSC 8, carriage-return overwrite,
//! C1 controls, DEL), visible rendering of invisible format characters (Trojan Source bidi, zero-width, separators,
//! tag characters, variation selectors), and the capped `LogExcerpt`.
//!
//! Inputs are written with `\u{..}` escapes so the corpus itself is readable; expected outputs are literals.

use minos_core::limits::MAX_LOG_EXCERPT_BYTES;
use minos_core::text::{LogExcerpt, SafeText, is_invisible_format};
use proptest::prelude::*;

fn shown(input: &str) -> String {
    SafeText::new(input).as_str().to_owned()
}

// ---------------------------------------------------------------------------------------------------------------
// Controls are dropped
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn csi_color_leaves_inert_residue() {
    assert_eq!(shown("\u{1b}[31mred\u{1b}[0m"), "[31mred[0m");
}

#[test]
fn osc_52_clipboard_write_is_inert() {
    assert_eq!(shown("\u{1b}]52;c;aGVsbG8=\u{7}"), "]52;c;aGVsbG8=");
}

#[test]
fn osc_8_hyperlink_is_inert() {
    let link = "\u{1b}]8;;https://evil.example/\u{7}click\u{1b}]8;;\u{7}";
    assert_eq!(shown(link), "]8;;https://evil.example/click]8;;");
    assert_eq!(
        shown("\u{1b}]8;;https://e.example/\u{1b}\\x\u{1b}]8;;\u{1b}\\"),
        "]8;;https://e.example/\\x]8;;\\"
    );
}

#[test]
fn csi_cursor_moves_are_inert() {
    assert_eq!(shown("ok\u{1b}[2A\u{1b}[2Kfake"), "ok[2A[2Kfake");
}

#[test]
fn carriage_return_cannot_overwrite() {
    assert_eq!(shown("abc\rxyz"), "abcxyz");
}

#[test]
fn c1_csi_is_dropped() {
    assert_eq!(shown("\u{9b}2J"), "2J");
}

#[test]
fn del_is_dropped() {
    assert_eq!(shown("\u{7f}"), "");
    assert_eq!(shown("a\u{7f}b"), "ab");
}

#[test]
fn nul_and_backspace_are_dropped() {
    assert_eq!(shown("a\u{0}b\u{8}c"), "abc");
}

#[test]
fn newline_and_tab_are_kept() {
    assert_eq!(shown("a\tb\nc"), "a\tb\nc");
}

// ---------------------------------------------------------------------------------------------------------------
// Invisible format characters are shown
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn bidi_override_is_shown() {
    assert_eq!(shown("\u{202e}"), "<U+202E>");
    assert_eq!(shown("\u{61c}"), "<U+061C>");
    assert_eq!(shown("\u{2066}x\u{2069}"), "<U+2066>x<U+2069>");
}

#[test]
fn trojan_source_sample_is_shown() {
    let sample = "if access_level != \"user\u{202e} \u{2066}// Check if admin\u{2069} \u{2066}\" {";
    assert_eq!(
        shown(sample),
        "if access_level != \"user<U+202E> <U+2066>// Check if admin<U+2069> <U+2066>\" {"
    );
}

#[test]
fn zero_width_characters_are_shown() {
    assert_eq!(shown("\u{200d}"), "<U+200D>");
    assert_eq!(shown("a\u{200b}b"), "a<U+200B>b");
    assert_eq!(shown("\u{feff}"), "<U+FEFF>");
    assert_eq!(shown("\u{ad}"), "<U+00AD>");
}

#[test]
fn line_separator_is_shown() {
    assert_eq!(shown("\u{2028}"), "<U+2028>");
    assert_eq!(shown("\u{2029}"), "<U+2029>");
}

#[test]
fn tag_characters_are_shown() {
    assert_eq!(shown("\u{e0041}"), "<U+E0041>");
    assert_eq!(shown("hi\u{e0069}\u{e0067}"), "hi<U+E0069><U+E0067>");
}

#[test]
fn variation_selectors_are_shown() {
    assert_eq!(shown("\u{fe0f}"), "<U+FE0F>");
    assert_eq!(shown("\u{e0100}"), "<U+E0100>");
}

#[test]
fn ordinary_unicode_is_kept() {
    assert_eq!(shown("é漢字😀"), "é漢字😀");
}

#[test]
fn invisible_table_edges() {
    for c in [
        '\u{ad}', '\u{34f}', '\u{61c}', '\u{115f}', '\u{1160}', '\u{180e}', '\u{200b}', '\u{200f}', '\u{2028}',
        '\u{2029}', '\u{202a}', '\u{202e}', '\u{2060}', '\u{2064}', '\u{2066}', '\u{2069}', '\u{3164}', '\u{fe00}',
        '\u{fe0f}', '\u{feff}', '\u{ffa0}', '\u{e0000}', '\u{e007f}', '\u{e0100}', '\u{e01ef}',
    ] {
        assert!(is_invisible_format(c), "U+{:04X} should be invisible", u32::from(c));
    }
    for c in ['a', ' ', '\u{ac}', '\u{200a}', '\u{2030}', '\u{2065}', '\u{fdff}', '\u{e0080}', '\u{e01f0}'] {
        assert!(!is_invisible_format(c), "U+{:04X} should be visible", u32::from(c));
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Constructors
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn inline_shows_newline_and_tab() {
    assert_eq!(SafeText::inline("a\nb\tc").as_str(), "a<U+000A>b<U+0009>c");
}

#[test]
fn lossy_bytes_get_a_replacement_character() {
    assert_eq!(SafeText::from_utf8_lossy(b"ok\xff").as_str(), "ok\u{fffd}");
    assert_eq!(SafeText::from_utf8_lossy(b"\x1b[1m").as_str(), "[1m");
}

#[test]
fn display_writes_the_sanitized_text() {
    let text = SafeText::new("x\u{1b}y");
    assert_eq!(text.to_string(), "xy");
    assert_eq!(text.len(), 2);
    assert!(!text.is_empty());
    assert!(SafeText::new("\u{1b}").is_empty());
}

// ---------------------------------------------------------------------------------------------------------------
// LogExcerpt
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn long_log_keeps_its_tail() {
    let mut log = vec![b'a'; 9_990];
    log.extend_from_slice(b"0123456789");
    let excerpt = LogExcerpt::from_bytes(&log, 4096);
    assert!(excerpt.is_truncated());
    assert!(excerpt.text().len() <= 4096);
    assert!(excerpt.text().as_str().ends_with("0123456789"));
}

#[test]
fn short_log_is_whole() {
    let excerpt = LogExcerpt::from_bytes(b"built ok\n", 4096);
    assert!(!excerpt.is_truncated());
    assert_eq!(excerpt.text().as_str(), "built ok\n");
}

#[test]
fn excerpt_cap_is_clamped() {
    let log = vec![b'x'; MAX_LOG_EXCERPT_BYTES * 4];
    let excerpt = LogExcerpt::from_bytes(&log, usize::MAX);
    assert!(excerpt.is_truncated());
    assert_eq!(excerpt.text().len(), MAX_LOG_EXCERPT_BYTES);
}

#[test]
fn expansion_past_the_cap_is_trimmed() {
    // 1000 zero-width spaces are 3000 bytes of input and 8000 bytes once each is shown as <U+200B>.
    let log = "\u{200b}".repeat(1000);
    let excerpt = LogExcerpt::from_bytes(log.as_bytes(), MAX_LOG_EXCERPT_BYTES);
    assert!(excerpt.is_truncated());
    assert!(excerpt.text().len() <= MAX_LOG_EXCERPT_BYTES);
    assert!(excerpt.text().as_str().ends_with("<U+200B>"));
}

#[test]
fn excerpt_cut_inside_a_character_does_not_start_with_a_replacement() {
    // "é" is two bytes; a tail of 3 bytes starts on its continuation byte.
    let excerpt = LogExcerpt::from_bytes("éé".as_bytes(), 3);
    assert!(excerpt.is_truncated());
    assert_eq!(excerpt.text().as_str(), "é");
}

#[test]
fn excerpt_escapes_are_dropped() {
    let excerpt = LogExcerpt::from_bytes(b"\x1b]52;c;eA==\x07done", 4096);
    assert_eq!(excerpt.text().as_str(), "]52;c;eA==done");
}

// ---------------------------------------------------------------------------------------------------------------
// Properties
// ---------------------------------------------------------------------------------------------------------------

proptest! {
    #[test]
    fn output_has_no_controls_or_invisibles(input in any::<String>()) {
        let text = SafeText::new(&input);
        for c in text.as_str().chars() {
            prop_assert!(!c.is_control() || c == '\n' || c == '\t', "U+{:04X}", u32::from(c));
            prop_assert!(!is_invisible_format(c), "U+{:04X}", u32::from(c));
        }
        let inline = SafeText::inline(&input);
        prop_assert!(!inline.as_str().chars().any(char::is_control));
    }

    #[test]
    fn rendering_is_idempotent(input in any::<String>()) {
        let once = SafeText::new(&input);
        prop_assert_eq!(SafeText::new(once.as_str()), once.clone());
        let inline = SafeText::inline(&input);
        prop_assert_eq!(SafeText::inline(inline.as_str()), inline.clone());
    }

    #[test]
    fn excerpt_never_exceeds_its_cap(bytes in proptest::collection::vec(any::<u8>(), 0..10_000), max in 0_usize..8_192) {
        let excerpt = LogExcerpt::from_bytes(&bytes, max);
        prop_assert!(excerpt.text().len() <= max.min(MAX_LOG_EXCERPT_BYTES));
        for c in excerpt.text().as_str().chars() {
            prop_assert!(!c.is_control() || c == '\n' || c == '\t');
            prop_assert!(!is_invisible_format(c));
        }
    }
}
