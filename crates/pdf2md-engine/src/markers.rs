//! List-marker recognition at the start of a line.

use crate::model::Marker;

/// A recognised list marker: its kind and the byte length of the marker
/// plus following whitespace, i.e. where the item text starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedMarker {
    pub marker: Marker,
    pub len: usize,
}

const BULLETS: &[char] = &[
    '•', '◦', '▪', '▫', '●', '○', '■', '□', '‣', '⁃', '∙', '·', '➢', '►', '▸', '▹', '❖', '✓', '✔',
    '➤', '\u{F0B7}',
];

/// Recognise a list marker at the start of `text`.
///
/// Bullets: the glyphs in [`BULLETS`] (whitespace after them optional, since
/// many producers position the glyph separately), and `-`, `–`, `*`, `+`
/// followed by whitespace. Ordered: `12.` / `12)` / `(12)`, single letters
/// `a.` / `b)` / `(c)` and lower-case roman numerals `iv.` / `(iv)`, each
/// followed by whitespace. Letters and roman numerals map to their ordinal.
pub fn parse(text: &str) -> Option<ParsedMarker> {
    let first = text.chars().next()?;
    if BULLETS.contains(&first) {
        let len = first.len_utf8();
        let rest = &text[len..];
        let ws = rest.len() - rest.trim_start().len();
        if rest.trim().is_empty() {
            return None;
        }
        return Some(ParsedMarker {
            marker: Marker::Bullet,
            len: len + ws,
        });
    }
    if matches!(first, '-' | '–' | '*' | '+') {
        let len = first.len_utf8();
        let rest = &text[len..];
        let ws = rest.len() - rest.trim_start().len();
        if ws == 0 || rest.trim().is_empty() {
            return None;
        }
        return Some(ParsedMarker {
            marker: Marker::Bullet,
            len: len + ws,
        });
    }
    parse_ordered(text)
}

fn parse_ordered(text: &str) -> Option<ParsedMarker> {
    let (open, body_start) = if text.starts_with('(') {
        (true, 1)
    } else {
        (false, 0)
    };
    let body: String = text[body_start..]
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric())
        .collect();
    if body.is_empty() {
        return None;
    }
    let after = &text[body_start + body.len()..];
    let close = after.chars().next()?;
    let ok_close = if open {
        close == ')'
    } else {
        close == '.' || close == ')'
    };
    if !ok_close {
        return None;
    }
    let rest = &after[1..];
    let ws = rest.len() - rest.trim_start().len();
    if ws == 0 || rest.trim().is_empty() {
        return None;
    }
    let n = ordinal(&body)?;
    let marker = if close == ')' {
        Marker::OrderedParen(n)
    } else {
        Marker::Ordered(n)
    };
    Some(ParsedMarker {
        marker,
        len: body_start + body.len() + 1 + ws,
    })
}

fn ordinal(body: &str) -> Option<u32> {
    if body.bytes().all(|b| b.is_ascii_digit()) {
        return if body.len() <= 3 {
            body.parse().ok()
        } else {
            None
        };
    }
    if let Some(n) = roman(body) {
        // A lone "i" or "v" or "x" could be a letter; letters win for the
        // single characters that are not i (i is far more often roman).
        if body.len() > 1 || body == "i" {
            return Some(n);
        }
    }
    let mut chars = body.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if c.is_ascii_lowercase() => Some(c as u32 - 'a' as u32 + 1),
        _ => None,
    }
}

fn roman(s: &str) -> Option<u32> {
    if s.is_empty() || s.len() > 6 {
        return None;
    }
    let val = |c| match c {
        'i' => Some(1),
        'v' => Some(5),
        'x' => Some(10),
        'l' => Some(50),
        _ => None,
    };
    let digits: Option<Vec<u32>> = s.chars().map(val).collect();
    let digits = digits?;
    let mut total = 0i64;
    for (i, d) in digits.iter().enumerate() {
        if digits.get(i + 1).is_some_and(|n| n > d) {
            total -= *d as i64;
        } else {
            total += *d as i64;
        }
    }
    (total > 0).then_some(total as u32)
}

/// Depth of a section number at the start of a heading: `7` → 1,
/// `7.6.3` → 3, `A.2` → 2, `Annex B` → 1. The number must be followed by
/// whitespace and then text (or be the whole string when `alone`).
pub fn section_depth(text: &str, alone: bool) -> Option<u8> {
    let t = text.trim_start();
    for word in ["Annex", "Appendix", "ANNEX", "APPENDIX"] {
        if let Some(rest) = t.strip_prefix(word) {
            let rest = rest.trim_start();
            let label: String = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric())
                .collect();
            let ok = !label.is_empty()
                && label.len() <= 2
                && label
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_uppercase() || c.is_ascii_digit());
            return ok.then_some(1);
        }
    }
    let end = t.find(char::is_whitespace).unwrap_or(t.len());
    let (num, rest) = t.split_at(end);
    if alone != rest.trim().is_empty() {
        return None;
    }
    let num = num.strip_suffix('.').unwrap_or(num);
    let parts: Vec<&str> = num.split('.').collect();
    if parts.is_empty() || parts.len() > 6 {
        return None;
    }
    let first_ok = parts[0].len() <= 2
        && (parts[0].bytes().all(|b| b.is_ascii_digit())
            || (parts.len() > 1
                && parts[0].len() == 1
                && parts[0].bytes().all(|b| b.is_ascii_uppercase())));
    let rest_ok = parts[1..]
        .iter()
        .all(|p| !p.is_empty() && p.len() <= 3 && p.bytes().all(|b| b.is_ascii_digit()));
    (first_ok && rest_ok && !parts[0].is_empty()).then_some(parts.len() as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn section_numbers() {
        assert_eq!(
            section_depth("7.6.3 Standard Security Handler", false),
            Some(3)
        );
        assert_eq!(section_depth("2 Conformance", false), Some(1));
        assert_eq!(section_depth("4. Results", false), Some(1));
        assert_eq!(section_depth("A.2 Details", false), Some(2));
        assert_eq!(section_depth("Annex B (normative)", false), Some(1));
        assert_eq!(section_depth("7.6.3", true), Some(3));
        assert_eq!(section_depth("2026 was a good year", false), None);
        assert_eq!(section_depth("Introduction", false), None);
        assert_eq!(section_depth("3.5 percent growth", false), Some(2));
        assert_eq!(section_depth("7.6.3", false), None);
    }

    fn m(s: &str) -> Option<(Marker, &str)> {
        parse(s).map(|p| (p.marker, &s[p.len..]))
    }

    #[test]
    fn bullets() {
        assert_eq!(m("• First"), Some((Marker::Bullet, "First")));
        assert_eq!(m("•First"), Some((Marker::Bullet, "First")));
        assert_eq!(m("- dash item"), Some((Marker::Bullet, "dash item")));
        assert_eq!(m("– en dash"), Some((Marker::Bullet, "en dash")));
        assert_eq!(m("◦ nested"), Some((Marker::Bullet, "nested")));
    }

    #[test]
    fn not_bullets() {
        assert_eq!(m("-5 degrees"), None);
        assert_eq!(m("•"), None);
        assert_eq!(m("*emphasis*"), None);
    }

    #[test]
    fn ordered() {
        assert_eq!(m("1. One"), Some((Marker::Ordered(1), "One")));
        assert_eq!(m("12) Twelve"), Some((Marker::OrderedParen(12), "Twelve")));
        assert_eq!(m("(3) Three"), Some((Marker::OrderedParen(3), "Three")));
        assert_eq!(m("b. Bee"), Some((Marker::Ordered(2), "Bee")));
        assert_eq!(m("(iv) Four"), Some((Marker::OrderedParen(4), "Four")));
        assert_eq!(m("i. One"), Some((Marker::Ordered(1), "One")));
    }

    #[test]
    fn not_ordered() {
        assert_eq!(m("1.5 million"), None);
        assert_eq!(m("2026. A year"), None);
        assert_eq!(m("Fig. 3"), None);
        assert_eq!(m("e.g. this"), None);
        assert_eq!(m("1."), None);
    }
}
