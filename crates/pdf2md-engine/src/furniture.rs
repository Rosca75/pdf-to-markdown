//! Page furniture: running headers, running footers and page numbers.
//!
//! Furniture is identified across the whole document, never per page: a line
//! is furniture when its *shape* (case-folded text with every digit run
//! replaced by `#`) recurs in the same margin band on enough pages, or when
//! it is a bare page number in a margin band. Position inside the band is
//! deliberately ignored so headers that jump between odd and even pages, or
//! between pages of different sizes, are still recognised.

use std::collections::HashMap;

/// Fraction of the page height treated as the top / bottom margin band.
const BAND: f32 = 0.12;

/// Which margin band a line sits in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Band {
    Top,
    Bottom,
}

/// The facts furniture detection needs about one line.
#[derive(Debug, Clone, Copy)]
pub struct LineProbe<'a> {
    pub text: &'a str,
    /// Top and bottom of the line, top-down page coordinates.
    pub y0: f32,
    pub y1: f32,
}

/// Classify the band of a line on a page of height `page_h`.
pub fn band_of(y0: f32, y1: f32, page_h: f32) -> Option<Band> {
    if page_h <= 0.0 {
        return None;
    }
    if y1 <= page_h * BAND {
        Some(Band::Top)
    } else if y0 >= page_h * (1.0 - BAND) {
        Some(Band::Bottom)
    } else {
        None
    }
}

/// `pages[p]` lists the lines of page `p` with that page's height.
/// Returns, per page, a flag per line: `true` = furniture, drop it.
pub fn detect(pages: &[(f32, Vec<LineProbe<'_>>)]) -> Vec<Vec<bool>> {
    let mut counts: HashMap<(Band, String), usize> = HashMap::new();
    let mut keys: Vec<Vec<Option<(Band, String)>>> = Vec::with_capacity(pages.len());
    let mut pages_with_text = 0usize;
    for (h, lines) in pages {
        if !lines.is_empty() {
            pages_with_text += 1;
        }
        let mut page_keys = Vec::with_capacity(lines.len());
        let mut seen_on_page: Vec<(Band, String)> = Vec::new();
        for l in lines {
            let k = band_of(l.y0, l.y1, *h).map(|b| (b, shape_key(l.text)));
            if let Some(k) = &k {
                if !k.1.is_empty() && !seen_on_page.contains(k) {
                    seen_on_page.push(k.clone());
                    *counts.entry(k.clone()).or_default() += 1;
                }
            }
            page_keys.push(k);
        }
        keys.push(page_keys);
    }
    // A shape must recur on at least half of the pages that carry text, and
    // on two pages at minimum: one occurrence is content, not furniture.
    let threshold = (pages_with_text.div_ceil(2)).max(2);
    pages
        .iter()
        .zip(keys)
        .map(|((_, lines), page_keys)| {
            lines
                .iter()
                .zip(page_keys)
                .map(|(l, k)| match k {
                    Some(k) => is_page_number(l.text) || counts.get(&k).copied().unwrap_or(0) >= threshold,
                    None => false,
                })
                .collect()
        })
        .collect()
}

/// Case-folded text with digit runs collapsed to `#` and whitespace squeezed.
pub fn shape_key(text: &str) -> String {
    let mut key = String::with_capacity(text.len());
    let mut in_digits = false;
    let mut in_space = true;
    for c in text.chars() {
        if c.is_ascii_digit() {
            if !in_digits {
                key.push('#');
            }
            in_digits = true;
            in_space = false;
        } else if c.is_whitespace() {
            if !in_space {
                key.push(' ');
            }
            in_space = true;
            in_digits = false;
        } else {
            key.extend(c.to_lowercase());
            in_digits = false;
            in_space = false;
        }
    }
    key.truncate(key.trim_end().len());
    key
}

/// Bare page-number lines: `7`, `- 7 -`, `Page 7`, `page 7 of 12`, `7 / 12`,
/// `vii`.
pub fn is_page_number(text: &str) -> bool {
    let key = shape_key(text);
    let k = key.trim_matches(|c: char| c == '-' || c == '–' || c == '—' || c == ' ' || c == '|');
    let k = k.strip_prefix("page").map(str::trim_start).unwrap_or(k);
    let k = k.strip_prefix("p.").map(str::trim_start).unwrap_or(k);
    if matches!(k, "#" | "# of #" | "# / #" | "#/#" | "# sur #" | "# von #" | "# de #") {
        return true;
    }
    is_roman(k)
}

/// Strict lower-case roman numeral up to 399 (`i` … `cccxcix`).
fn is_roman(s: &str) -> bool {
    fn strip<'a>(s: &'a str, options: &[&str]) -> &'a str {
        // Longest matching option wins; options are given longest first.
        options.iter().find_map(|o| s.strip_prefix(o)).unwrap_or(s)
    }
    if s.is_empty() || s.len() > 9 {
        return false;
    }
    let rest = strip(s, &["ccc", "cc", "c"]);
    let rest = strip(rest, &["xc", "xl", "lxxx", "lxx", "lx", "l", "xxx", "xx", "x"]);
    let rest = strip(rest, &["ix", "iv", "viii", "vii", "vi", "v", "iii", "ii", "i"]);
    rest.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn probe(text: &str, y0: f32) -> LineProbe<'_> {
        LineProbe { text, y0, y1: y0 + 10.0 }
    }

    #[test]
    fn shape_key_masks_digits_and_case() {
        assert_eq!(shape_key("  Page 12 of  30 "), "page # of #");
        assert_eq!(shape_key("ACME Report 2026"), "acme report #");
    }

    #[test]
    fn page_numbers_recognised() {
        for t in ["7", "- 7 -", "Page 7", "page 7 of 12", "7 / 12", "vii", "— 12 —"] {
            assert!(is_page_number(t), "{t}");
        }
        for t in ["Introduction", "7 apples", "Chapter 7", "civil", "vivid", "mix"] {
            assert!(!is_page_number(t), "{t}");
        }
    }

    #[test]
    fn recurring_header_and_footer_removed_even_when_shifted() {
        let pages: Vec<(f32, Vec<LineProbe>)> = (1..=4)
            .map(|p| {
                let shift = if p % 2 == 0 { 20.0 } else { 0.0 };
                (
                    792.0,
                    vec![
                        probe("ACME Annual Report", 30.0 + shift),
                        probe("Body text that is unique", 300.0),
                        probe(if p == 1 { "Page 1 of 4" } else { "Page 9 of 4" }, 760.0),
                    ],
                )
            })
            .collect();
        let flags = detect(&pages);
        for f in &flags {
            assert_eq!(f, &vec![true, false, true]);
        }
    }

    #[test]
    fn single_occurrence_in_band_is_content() {
        let pages = vec![
            (792.0, vec![probe("Big Title On First Page", 40.0), probe("body", 300.0)]),
            (792.0, vec![probe("other body", 300.0)]),
            (792.0, vec![probe("more body", 300.0)]),
        ];
        let flags = detect(&pages);
        assert_eq!(flags[0], vec![false, false]);
    }

    #[test]
    fn body_lines_never_removed_even_if_repeated() {
        let pages: Vec<(f32, Vec<LineProbe>)> =
            (0..3).map(|_| (792.0, vec![probe("Repeated body sentence", 400.0)])).collect();
        assert!(detect(&pages).iter().all(|f| f == &vec![false]));
    }

    #[test]
    fn bare_page_number_removed_even_once() {
        let pages = vec![(792.0, vec![probe("body", 300.0), probe("3", 770.0)])];
        assert_eq!(detect(&pages)[0], vec![false, true]);
    }
}
