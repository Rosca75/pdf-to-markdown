//! Spans → line fragments.
//!
//! A *row* is every span sharing a baseline band; a row is cut into
//! *fragments* wherever the horizontal gap is too wide to be a word space
//! (column gutters, table cell gaps). Fragments are the atoms of reading
//! order, furniture detection and table detection.

use crate::model::{BBox, Span};

/// Gap (in multiples of the font size) above which a row splits into
/// separate fragments. Word spaces, even in loosely justified text, stay
/// well under one em; gutters and cell gaps are wider.
const FRAGMENT_GAP_EM: f32 = 1.2;
/// A lone list marker is joined with the following text up to this gap.
const MARKER_GAP_EM: f32 = 4.0;
/// Gap (in em) above which a space is inserted between adjacent spans.
const SPACE_GAP_EM: f32 = 0.12;
/// Minimum vertical overlap (fraction of the smaller height) to share a row.
const ROW_OVERLAP: f32 = 0.5;

/// A horizontal run of text on one baseline.
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    /// Spans in left-to-right order; inter-span spaces are already folded
    /// into the span texts.
    pub spans: Vec<Span>,
    pub bbox: BBox,
    /// Character-weighted dominant font size.
    pub size: f32,
    /// Index of the row the fragment came from (fragments of one row share it).
    pub row: usize,
}

impl Line {
    pub fn text(&self) -> String {
        let mut s = String::with_capacity(self.spans.iter().map(|s| s.text.len()).sum());
        for sp in &self.spans {
            s.push_str(&sp.text);
        }
        s
    }

    fn visible_chars(&self, pred: impl Fn(&Span) -> bool) -> (usize, usize) {
        let mut yes = 0;
        let mut all = 0;
        for sp in &self.spans {
            let n = sp.text.chars().filter(|c| !c.is_whitespace()).count();
            all += n;
            if pred(sp) {
                yes += n;
            }
        }
        (yes, all)
    }

    pub fn all_bold(&self) -> bool {
        let (b, a) = self.visible_chars(|s| s.style.bold);
        a > 0 && b == a
    }

    pub fn all_mono(&self) -> bool {
        let (m, a) = self.visible_chars(|s| s.style.mono);
        a > 0 && m == a
    }
}

/// Build line fragments from a page's spans.
pub fn build(mut spans: Vec<Span>) -> Vec<Line> {
    spans.sort_by(|a, b| {
        a.bbox
            .y0
            .total_cmp(&b.bbox.y0)
            .then(a.bbox.x0.total_cmp(&b.bbox.x0))
    });

    // Rows: a span joins the open row it overlaps most vertically.
    let mut rows: Vec<(BBox, Vec<Span>)> = Vec::new();
    let mut open_from = 0usize;
    for span in spans {
        while open_from < rows.len() && rows[open_from].0.y1 < span.bbox.y0 {
            open_from += 1;
        }
        let mut best: Option<(usize, f32)> = None;
        for (i, (rb, _)) in rows.iter().enumerate().skip(open_from) {
            let min_h = rb.height().min(span.bbox.height()).max(0.1);
            let ov = rb.v_overlap(&span.bbox) / min_h;
            if ov >= ROW_OVERLAP && best.is_none_or(|(_, b)| ov > b) {
                best = Some((i, ov));
            }
        }
        match best {
            Some((i, _)) => {
                let row = &mut rows[i];
                // Keep the row band anchored on its dominant text: grow
                // only horizontally so a tall glyph cannot swallow the
                // next line.
                row.0.x0 = row.0.x0.min(span.bbox.x0);
                row.0.x1 = row.0.x1.max(span.bbox.x1);
                row.1.push(span);
            }
            None => rows.push((span.bbox, vec![span])),
        }
    }

    let mut lines = Vec::with_capacity(rows.len());
    for (row_idx, (_, mut row)) in rows.into_iter().enumerate() {
        row.sort_by(|a, b| a.bbox.x0.total_cmp(&b.bbox.x0));
        dedupe_overprint(&mut row);
        split_row(row, row_idx, &mut lines);
    }
    lines.sort_by(|a, b| {
        a.bbox
            .y0
            .total_cmp(&b.bbox.y0)
            .then(a.bbox.x0.total_cmp(&b.bbox.x0))
    });
    lines
}

/// Remove spans that repeat the previous span's text at (almost) the same
/// position: simulated bold, drop shadows, duplicated drawing.
fn dedupe_overprint(row: &mut Vec<Span>) {
    let mut kept: Vec<Span> = Vec::with_capacity(row.len());
    for s in row.drain(..) {
        let dup = kept.iter().rev().take(4).any(|k| {
            k.text == s.text
                && (k.bbox.x0 - s.bbox.x0).abs() < 0.2 * s.font_size.max(1.0)
                && (k.bbox.y0 - s.bbox.y0).abs() < 0.2 * s.font_size.max(1.0)
        });
        if !dup {
            kept.push(s);
        }
    }
    *row = kept;
}

fn split_row(row: Vec<Span>, row_idx: usize, out: &mut Vec<Line>) {
    let mut current: Vec<Span> = Vec::new();
    for span in row {
        if let Some(prev) = current.last() {
            let em = prev.font_size.max(span.font_size).max(1.0);
            let gap = span.bbox.x0 - prev.bbox.x1;
            let lone_marker = current.len() == 1 && is_marker_only(&current[0].text);
            let limit = if lone_marker {
                MARKER_GAP_EM
            } else {
                FRAGMENT_GAP_EM
            } * em;
            if gap > limit {
                out.push(finish(std::mem::take(&mut current), row_idx));
            } else if gap > SPACE_GAP_EM * em
                && !prev.text.ends_with(char::is_whitespace)
                && !span.text.starts_with(char::is_whitespace)
            {
                current.last_mut().expect("non-empty").text.push(' ');
            }
        }
        current.push(span);
    }
    if !current.is_empty() {
        out.push(finish(current, row_idx));
    }
}

/// A fragment that is nothing but a list marker (`•`, `1.`, `a)`, `-`) or a
/// section number (`7.6.3`, `A.2`): typeset apart from the text it labels,
/// but belonging to it.
fn is_marker_only(text: &str) -> bool {
    let t = text.trim();
    if t.is_empty() || t.chars().count() > 12 {
        return false;
    }
    if crate::markers::section_depth(t, true).is_some() {
        return true;
    }
    if t.chars().count() > 5 {
        return false;
    }
    let mut probe = String::with_capacity(t.len() + 2);
    probe.push_str(t);
    probe.push_str(" x");
    crate::markers::parse(&probe).is_some_and(|m| m.len >= t.len())
}

fn finish(mut spans: Vec<Span>, row: usize) -> Line {
    if let Some(last) = spans.last_mut() {
        let trimmed = last.text.trim_end().len();
        last.text.truncate(trimmed);
    }
    if let Some(first) = spans.first_mut() {
        let lead = first.text.len() - first.text.trim_start().len();
        first.text.drain(..lead);
    }
    let bbox = spans
        .iter()
        .skip(1)
        .fold(spans[0].bbox, |b, s| b.union(&s.bbox));
    let size = dominant_size(&spans);
    Line {
        spans,
        bbox,
        size,
        row,
    }
}

/// Character-weighted most common font size (0.5 pt buckets).
pub fn dominant_size(spans: &[Span]) -> f32 {
    let mut buckets: Vec<(i32, usize)> = Vec::new();
    for s in spans {
        let key = (s.font_size * 2.0).round() as i32;
        let n = s.text.chars().filter(|c| !c.is_whitespace()).count().max(1);
        match buckets.iter_mut().find(|(k, _)| *k == key) {
            Some((_, c)) => *c += n,
            None => buckets.push((key, n)),
        }
    }
    buckets
        .iter()
        .max_by_key(|(k, c)| (*c, *k))
        .map(|(k, _)| *k as f32 / 2.0)
        .unwrap_or(10.0)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::model::Style;

    pub fn sp(text: &str, x0: f32, y0: f32, size: f32) -> Span {
        // Rough Helvetica-ish advance: half an em per character.
        let w = text.chars().count() as f32 * size * 0.5;
        Span {
            text: text.into(),
            bbox: BBox::new(x0, y0, x0 + w, y0 + size),
            font_size: size,
            style: Style::default(),
            link: None,
        }
    }

    #[test]
    fn spans_on_one_baseline_form_one_line_with_spaces() {
        let lines = build(vec![
            sp("world", 40.0, 100.0, 10.0),
            sp("Hello", 10.0, 100.0, 10.0),
        ]);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text(), "Hello world");
    }

    #[test]
    fn adjacent_spans_without_gap_join_without_space() {
        // "Hel" ends at 25, "lo" starts at 25.
        let lines = build(vec![
            sp("Hel", 10.0, 100.0, 10.0),
            sp("lo", 25.0, 100.0, 10.0),
        ]);
        assert_eq!(lines[0].text(), "Hello");
    }

    #[test]
    fn wide_gap_splits_fragments() {
        let lines = build(vec![
            sp("Left column", 72.0, 100.0, 10.0),
            sp("Right column", 320.0, 100.0, 10.0),
        ]);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].row, lines[1].row);
    }

    #[test]
    fn lone_bullet_joins_its_text() {
        let lines = build(vec![
            sp("•", 72.0, 100.0, 10.0),
            sp("Item text", 95.0, 100.0, 10.0),
        ]);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text(), "• Item text");
    }

    #[test]
    fn section_number_joins_its_title() {
        let lines = build(vec![
            sp("7.6.3", 36.8, 100.0, 10.0),
            sp("Standard Security Handler", 79.4, 100.0, 10.0),
        ]);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].text(), "7.6.3 Standard Security Handler");
    }

    #[test]
    fn separate_baselines_separate_lines() {
        let lines = build(vec![
            sp("one", 10.0, 100.0, 10.0),
            sp("two", 10.0, 112.0, 10.0),
        ]);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].text(), "one");
    }

    #[test]
    fn overprinted_text_deduplicated() {
        let lines = build(vec![
            sp("Bold", 10.0, 100.0, 10.0),
            sp("Bold", 10.3, 100.2, 10.0),
        ]);
        assert_eq!(lines[0].text(), "Bold");
    }

    #[test]
    fn superscript_stays_on_its_line() {
        let mut sup = sp("2", 40.0, 97.0, 6.0);
        sup.bbox.y1 = 103.0;
        let lines = build(vec![sp("E=mc", 10.0, 100.0, 10.0), sup]);
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn dominant_size_weighted_by_chars() {
        let spans = vec![
            sp("a", 0.0, 0.0, 20.0),
            sp("many characters here", 0.0, 0.0, 10.0),
        ];
        assert_eq!(dominant_size(&spans), 10.0);
    }
}
