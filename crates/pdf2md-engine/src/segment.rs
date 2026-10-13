//! Ordered line fragments → raw blocks (paragraph segmentation).
//!
//! A raw block is a run of lines that belong together typographically. It is
//! not classified yet: headings, list items and code are decided later with
//! document-wide statistics, after blocks split by page and column breaks
//! have been re-joined by [`crate::flow`].

use crate::lines::Line;
use crate::markers;
use crate::stats::DocStats;
use crate::tables::Table;

/// Extra vertical space (in em) beyond normal leading that marks a
/// paragraph break. Typical paragraph spacing is 0.3-1 em; leading jitter
/// from mixed fonts stays well below this.
const PARA_GAP_EM: f32 = 0.25;

/// Font-size difference (pt) between consecutive lines that marks a change
/// of text kind (caption, footnote, heading). Line sizes are character
/// weighted modes, so inline size variations do not reach this.
pub const SIZE_STEP: f32 = 0.75;
/// Share of the column width a line must leave empty to count as a
/// paragraph's last line without terminal punctuation.
const SHORT_LINE_SHARE: f32 = 0.25;

/// A run of lines or a detected table.
#[derive(Debug, Clone, PartialEq)]
pub enum RawKind {
    Text(Vec<Line>),
    Table(Table),
}

#[derive(Debug, Clone, PartialEq)]
pub struct RawBlock {
    pub kind: RawKind,
    /// The block follows a page or column break rather than a paragraph
    /// break, so it may continue the previous block.
    pub soft_start: bool,
    pub page: usize,
}

impl RawBlock {
    pub fn lines(&self) -> &[Line] {
        match &self.kind {
            RawKind::Text(l) => l,
            RawKind::Table(_) => &[],
        }
    }
}

/// One item in reading order.
pub enum Item {
    Line(Line),
    Table(Table),
}

/// Segment a page's items (already in reading order, each with the right
/// edge of its text column) into raw blocks.
pub fn segment(items: Vec<(Item, f32)>, page: usize, stats: &DocStats) -> Vec<RawBlock> {
    let mut out: Vec<RawBlock> = Vec::new();
    let mut cur: Vec<Line> = Vec::new();
    let mut cur_soft = true; // first block of a page follows a page break
    for (item, col_right) in items {
        match item {
            Item::Table(t) => {
                flush(&mut out, &mut cur, cur_soft, page);
                out.push(RawBlock { kind: RawKind::Table(t), soft_start: false, page });
                cur_soft = false;
            }
            Item::Line(line) => {
                if let Some(prev) = cur.last() {
                    match boundary(&cur, prev, &line, col_right, stats) {
                        Boundary::Same => {}
                        Boundary::Paragraph => {
                            flush(&mut out, &mut cur, cur_soft, page);
                            cur_soft = false;
                        }
                        Boundary::Column => {
                            flush(&mut out, &mut cur, cur_soft, page);
                            cur_soft = true;
                        }
                    }
                }
                cur.push(line);
            }
        }
    }
    flush(&mut out, &mut cur, cur_soft, page);
    out
}

fn flush(out: &mut Vec<RawBlock>, cur: &mut Vec<Line>, soft: bool, page: usize) {
    if !cur.is_empty() {
        out.push(RawBlock { kind: RawKind::Text(std::mem::take(cur)), soft_start: soft, page });
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Boundary {
    Same,
    Paragraph,
    Column,
}

/// Decide how line `c` relates to the block `block` whose last line is `p`.
fn boundary(block: &[Line], p: &Line, c: &Line, col_right: f32, stats: &DocStats) -> Boundary {
    let em = p.size.max(c.size);
    // Reading order moved back up the page: a column or region change.
    // (Horizontal disjointness alone is not one: short code lines and
    // centred lines need not overlap the line above.)
    if c.bbox.y0 < p.bbox.y0 - 0.5 * em {
        return Boundary::Column;
    }
    if (p.size - c.size).abs() >= SIZE_STEP {
        return Boundary::Paragraph;
    }
    let (p_mono, c_mono) = (p.all_mono(), c.all_mono());
    if p_mono != c_mono {
        return Boundary::Paragraph;
    }
    let c_text = c.text();
    if !c_mono && markers::parse(&c_text).is_some() {
        return Boundary::Paragraph;
    }
    let pitch = c.bbox.y1 - p.bbox.y1;
    if pitch > stats.pitch_for(p.size) + PARA_GAP_EM * em {
        return Boundary::Paragraph;
    }
    if c_mono {
        return Boundary::Same; // code keeps its lines together
    }
    // A bold-only block (heading-like) ends when regular text starts.
    let block_bold = block.iter().all(Line::all_bold);
    if block_bold != c.all_bold() && (block_bold || ends_sentence(&p.text())) {
        return Boundary::Paragraph;
    }
    let first = &block[0];
    let first_text = first.text();
    if let Some(m) = markers::parse(&first_text) {
        // List item: continuation lines hang to the right of the marker.
        let _ = m;
        return if c.bbox.x0 > first.bbox.x0 + 0.3 * em { Boundary::Same } else { Boundary::Paragraph };
    }
    let left = block.iter().map(|l| l.bbox.x0).fold(f32::MAX, f32::min);
    if block.len() >= 1 && c.bbox.x0 > left + 1.0 * em && c.bbox.x0 > p.bbox.x0 + 1.0 * em {
        return Boundary::Paragraph; // first-line indent of a new paragraph
    }
    let right = block.iter().map(|l| l.bbox.x1).fold(c.bbox.x1.max(col_right), f32::max);
    let p_text = p.text();
    if p.bbox.x1 < right - 3.0 * em && ends_sentence(&p_text) {
        return Boundary::Paragraph; // short last line of the previous paragraph
    }
    // A line stopping far short of the column edge ends its paragraph even
    // without punctuation (labels, address lines, captions) when the next
    // line starts like a new sentence.
    let width = (right - left).max(1.0);
    let starts_new = c_text.chars().next().is_some_and(|ch| ch.is_uppercase() || ch.is_ascii_digit());
    if starts_new && p.bbox.x1 < right - SHORT_LINE_SHARE * width && !p_text.ends_with(['-', ',']) {
        return Boundary::Paragraph;
    }
    Boundary::Same
}

/// Text ends a sentence (terminal punctuation, optionally followed by a
/// closing quote or bracket).
pub fn ends_sentence(text: &str) -> bool {
    let t = text.trim_end().trim_end_matches(['"', '\'', '”', '’', ')', ']', '»']);
    t.ends_with(['.', '!', '?', ':', '…', ';'])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lines::{build, tests::sp};

    fn texts(blocks: &[RawBlock]) -> Vec<String> {
        blocks
            .iter()
            .map(|b| b.lines().iter().map(Line::text).collect::<Vec<_>>().join(" | "))
            .collect()
    }

    fn run(spans: Vec<crate::model::Span>) -> Vec<RawBlock> {
        let lines = build(spans);
        let stats = DocStats::from_lines(lines.iter());
        segment(lines.into_iter().map(|l| (Item::Line(l), 540.0)).collect(), 0, &stats)
    }

    #[test]
    fn paragraph_gap_splits_blocks() {
        let b = run(vec![
            sp("First paragraph line one that is long", 72.0, 100.0, 10.0),
            sp("first paragraph line two.", 72.0, 112.0, 10.0),
            sp("Second paragraph starts here", 72.0, 140.0, 10.0),
            sp("and continues.", 72.0, 152.0, 10.0),
        ]);
        assert_eq!(texts(&b).len(), 2);
    }

    #[test]
    fn list_items_split_and_continuations_hang() {
        let b = run(vec![
            sp("• First item that wraps onto", 72.0, 100.0, 10.0),
            sp("a second line", 84.0, 112.0, 10.0),
            sp("• Second item", 72.0, 124.0, 10.0),
        ]);
        assert_eq!(texts(&b), vec!["• First item that wraps onto | a second line", "• Second item"]);
    }

    #[test]
    fn first_line_indent_starts_paragraph() {
        let b = run(vec![
            sp("Some paragraph text that runs on", 72.0, 100.0, 10.0),
            sp("to the end of the line here", 72.0, 112.0, 10.0),
            sp("Indented new paragraph", 92.0, 124.0, 10.0),
        ]);
        assert_eq!(b.len(), 2);
    }

    #[test]
    fn short_unpunctuated_lines_with_capital_starts_are_separate() {
        let b = run(vec![
            sp("The final flow is efficient", 72.0, 100.0, 12.0),
            sp("The office opens at nine", 72.0, 120.0, 12.0),
        ]);
        assert_eq!(b.len(), 2);
    }

    #[test]
    fn wrapped_full_lines_stay_together() {
        let b = run(vec![
            sp("This is a long line of a paragraph that fills the column to", 72.0, 100.0, 12.0),
            sp("Europe and beyond, continuing the same sentence.", 72.0, 114.0, 12.0),
        ]);
        assert_eq!(b.len(), 1);
    }

    #[test]
    fn smaller_caption_splits_from_paragraph() {
        let b = run(vec![
            sp("The paragraph above the figure keeps its sentence together", 72.0, 100.0, 11.0),
            sp("Figure 1 caption", 72.0, 114.0, 10.0),
        ]);
        assert_eq!(b.len(), 2);
    }

    #[test]
    fn sentence_end_detection() {
        assert!(ends_sentence("The end."));
        assert!(ends_sentence("He said \"stop.\""));
        assert!(!ends_sentence("continues on the"));
        assert!(!ends_sentence("inter-"));
    }
}
