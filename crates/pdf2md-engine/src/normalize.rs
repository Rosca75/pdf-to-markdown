//! Unicode clean-up of extracted text.
//!
//! PDF text arrives in whatever code points the producer's font encoding and
//! ToUnicode maps yield: presentation-form ligatures, soft hyphens, exotic
//! spaces, control characters and decomposed accents. Everything downstream
//! (layout, classification, Markdown escaping) assumes plain composed text,
//! so this stage is the single place where that assumption is established.

use unicode_normalization::UnicodeNormalization;

/// Result of normalising one piece of text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Normalized {
    pub text: String,
    /// Number of U+FFFD replacement characters (undecodable glyphs) seen.
    pub undecodable: usize,
}

/// Normalise `raw` for Markdown output.
///
/// - presentation-form ligatures (U+FB00–U+FB06) are expanded;
/// - soft hyphens and zero-width characters are removed;
/// - every Unicode space separator and tab becomes a plain space;
/// - C0/C1 control characters are dropped;
/// - private-use code points (unmapped glyphs) are dropped;
/// - the result is NFC-composed.
///
/// Line breaks are not expected inside spans; any that occur become spaces.
pub fn normalize(raw: &str) -> Normalized {
    let mut undecodable = 0;
    // Fast path: pure printable ASCII needs no work beyond a copy.
    if raw.bytes().all(|b| (0x20..0x7f).contains(&b)) {
        return Normalized {
            text: raw.to_owned(),
            undecodable,
        };
    }
    let mut out = String::with_capacity(raw.len());
    for c in raw.chars() {
        match c {
            '\u{FB00}' => out.push_str("ff"),
            '\u{FB01}' => out.push_str("fi"),
            '\u{FB02}' => out.push_str("fl"),
            '\u{FB03}' => out.push_str("ffi"),
            '\u{FB04}' => out.push_str("ffl"),
            '\u{FB05}' | '\u{FB06}' => out.push_str("st"),
            '\u{00AD}' | '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{2060}' | '\u{FEFF}' => {}
            '\u{FFFD}' => undecodable += 1,
            '\t' | '\n' | '\r' => out.push(' '),
            c if c.is_control() => {}
            c if is_private_use(c) => {}
            c if c != ' ' && is_space_separator(c) => out.push(' '),
            c => out.push(c),
        }
    }
    let text = if out.is_ascii() {
        out
    } else {
        out.nfc().collect()
    };
    Normalized { text, undecodable }
}

fn is_space_separator(c: char) -> bool {
    matches!(
        c,
        '\u{00A0}' | '\u{1680}' | '\u{2000}'..='\u{200A}' | '\u{202F}' | '\u{205F}' | '\u{3000}'
    )
}

fn is_private_use(c: char) -> bool {
    matches!(c, '\u{E000}'..='\u{F8FF}' | '\u{F0000}'..='\u{FFFFD}' | '\u{100000}'..='\u{10FFFD}')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(s: &str) -> String {
        normalize(s).text
    }

    #[test]
    fn ascii_passes_through() {
        assert_eq!(n("Hello, world!"), "Hello, world!");
    }

    #[test]
    fn ligatures_expand() {
        assert_eq!(
            n("e\u{FB03}cient \u{FB02}ow \u{FB01}sh"),
            "efficient flow fish"
        );
    }

    #[test]
    fn soft_hyphen_and_zero_width_removed() {
        assert_eq!(n("inter\u{00AD}national\u{200B}"), "international");
    }

    #[test]
    fn exotic_spaces_become_plain() {
        assert_eq!(n("a\u{00A0}b\u{2009}c\u{3000}d\te"), "a b c d e");
    }

    #[test]
    fn controls_and_private_use_dropped() {
        assert_eq!(n("a\u{0001}b\u{E001}c\u{0085}"), "abc");
    }

    #[test]
    fn decomposed_accents_compose() {
        assert_eq!(n("Cafe\u{0301}"), "Caf\u{00E9}");
    }

    #[test]
    fn replacement_chars_counted_and_dropped() {
        let r = normalize("a\u{FFFD}b\u{FFFD}");
        assert_eq!(r.text, "ab");
        assert_eq!(r.undecodable, 2);
    }

    #[test]
    fn non_latin_scripts_preserved() {
        assert_eq!(n("Αθήνα Москва 東京"), "Αθήνα Москва 東京");
    }
}
