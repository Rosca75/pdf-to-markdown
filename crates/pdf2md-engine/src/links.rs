//! Attach URI link annotations to the text they cover.
//!
//! A link annotation is only a rectangle; the text under it is whatever span
//! (or part of a span) lies inside. Spans fully covered get the link; spans
//! partially covered are split at word boundaries and each word is assigned
//! by whether its centre falls inside the rectangle. Word positions come from
//! the glyph advances when extraction provides them, otherwise they are
//! estimated proportionally to character count.

use crate::model::{BBox, Link, Span};

/// Minimum share of a word's height that must overlap the link rectangle.
const MIN_V_OVERLAP: f32 = 0.5;

/// `words[i]` optionally gives the x extent of each word of `spans[i]`.
pub fn apply(spans: &mut Vec<Span>, words: &[Option<Vec<(f32, f32)>>], links: &[Link]) {
    if links.is_empty() {
        return;
    }
    let mut out = Vec::with_capacity(spans.len() + links.len() * 2);
    for (k, span) in spans.drain(..).enumerate() {
        let hits: Vec<&Link> = links.iter().filter(|l| covers_vertically(&l.bbox, &span.bbox)).collect();
        if hits.is_empty() || !hits.iter().any(|l| l.bbox.h_overlap(&span.bbox) > 0.0) {
            out.push(span);
            continue;
        }
        if let Some(l) = hits.iter().find(|l| contains_h(&l.bbox, &span.bbox)) {
            out.push(Span { link: Some(l.uri.clone()), ..span });
            continue;
        }
        split_by_words(span, words.get(k).and_then(Option::as_deref), &hits, &mut out);
    }
    *spans = out;
}

fn covers_vertically(link: &BBox, span: &BBox) -> bool {
    let h = span.height().max(0.1);
    link.v_overlap(span) / h >= MIN_V_OVERLAP
}

fn contains_h(link: &BBox, span: &BBox) -> bool {
    let tol = 1.0;
    link.x0 - tol <= span.x0 && span.x1 <= link.x1 + tol
}

/// Split `span` into word pieces (whitespace kept with the preceding word)
/// and give each piece the link whose rectangle contains its centre.
fn split_by_words(span: Span, ranges: Option<&[(f32, f32)]>, hits: &[&Link], out: &mut Vec<Span>) {
    let total = span.text.chars().count().max(1) as f32;
    let per_char = span.bbox.width() / total;
    let parts = split_keep_ws(&span.text);
    let ranges = ranges.filter(|r| r.len() == parts.len());
    let mut pieces: Vec<Span> = Vec::new();
    let mut char_pos = 0usize;
    for (wi, word) in parts.into_iter().enumerate() {
        let n = word.chars().count();
        let core = word.trim_end().chars().count().max(1);
        let x0 = span.bbox.x0 + char_pos as f32 * per_char;
        let centre = match ranges {
            Some(r) => (r[wi].0 + r[wi].1) / 2.0,
            None => x0 + core as f32 * per_char / 2.0,
        };
        let link = hits.iter().find(|l| l.bbox.x0 <= centre && centre <= l.bbox.x1).map(|l| l.uri.clone());
        let bbox = BBox { x0, x1: x0 + n as f32 * per_char, ..span.bbox };
        match pieces.last_mut() {
            Some(last) if last.link == link => {
                last.text.push_str(word);
                last.bbox.x1 = bbox.x1;
            }
            _ => pieces.push(Span { text: word.to_owned(), bbox, link, ..span.clone() }),
        }
        char_pos += n;
    }
    // Whitespace trailing a linked word belongs outside the link text.
    for i in 0..pieces.len() {
        if pieces[i].link.is_some() && i + 1 < pieces.len() {
            let trimmed = pieces[i].text.trim_end().len();
            if trimmed < pieces[i].text.len() {
                let ws = pieces[i].text.split_off(trimmed);
                pieces[i + 1].text.insert_str(0, &ws);
            }
        }
    }
    out.extend(pieces);
}

/// Split into words, each carrying its trailing whitespace.
fn split_keep_ws(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut in_ws = false;
    for (i, c) in s.char_indices() {
        if c.is_whitespace() {
            in_ws = true;
        } else if in_ws {
            parts.push(&s[start..i]);
            start = i;
            in_ws = false;
        }
    }
    if start < s.len() {
        parts.push(&s[start..]);
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Style;
    use std::sync::Arc;

    fn span(text: &str, x0: f32, x1: f32) -> Span {
        Span {
            text: text.into(),
            bbox: BBox::new(x0, 100.0, x1, 110.0),
            font_size: 10.0,
            style: Style::default(),
            link: None,
        }
    }

    fn link(x0: f32, x1: f32) -> Link {
        Link { bbox: BBox::new(x0, 98.0, x1, 112.0), uri: Arc::from("https://x.test") }
    }

    #[test]
    fn fully_covered_span_gets_link() {
        let mut spans = vec![span("example.com", 10.0, 70.0)];
        apply(&mut spans, &[], &[link(8.0, 72.0)]);
        assert_eq!(spans.len(), 1);
        assert!(spans[0].link.is_some());
    }

    #[test]
    fn partially_covered_span_split_at_words() {
        // 30 chars over 300pt: 10pt per char. "Visit " = 0..60, "example.com " = 60..180.
        let mut spans = vec![span("Visit example.com for details.", 0.0, 300.0)];
        apply(&mut spans, &[], &[link(60.0, 170.0)]);
        let texts: Vec<(&str, bool)> = spans.iter().map(|s| (s.text.as_str(), s.link.is_some())).collect();
        assert_eq!(texts, vec![("Visit ", false), ("example.com", true), (" for details.", false)]);
    }

    #[test]
    fn glyph_word_ranges_override_estimate() {
        // The proportional estimate would put "for" inside the link; the
        // real glyph ranges show it starts after the link ends.
        let mut spans = vec![span("Read documentation for details.", 0.0, 300.0)];
        let ranges = vec![Some(vec![(0.0, 40.0), (45.0, 160.0), (165.0, 185.0), (190.0, 300.0)])];
        apply(&mut spans, &ranges, &[link(44.0, 161.0)]);
        let texts: Vec<(&str, bool)> = spans.iter().map(|s| (s.text.as_str(), s.link.is_some())).collect();
        assert_eq!(texts, vec![("Read ", false), ("documentation", true), (" for details.", false)]);
    }

    #[test]
    fn link_on_other_line_ignored() {
        let mut spans = vec![span("text", 0.0, 40.0)];
        let far = Link { bbox: BBox::new(0.0, 300.0, 40.0, 310.0), uri: Arc::from("u") };
        apply(&mut spans, &[], &[far]);
        assert!(spans[0].link.is_none());
    }
}
