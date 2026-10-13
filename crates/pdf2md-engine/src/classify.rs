//! Raw blocks → semantic blocks (headings, lists, code, paragraphs, tables).

use crate::lines::Line;
use crate::markers;
use crate::model::{Block, Inline, Style};
use crate::segment::{RawBlock, RawKind};
use crate::stats::DocStats;

/// Font size, relative to body text, from which a short block is a heading.
const HEADING_SIZE_RATIO: f32 = 1.12;
const MAX_HEADING_LINES: usize = 3;
const MAX_HEADING_CHARS: usize = 200;
const MAX_BOLD_HEADING_CHARS: usize = 120;

/// Whether a text block looks like a heading (size- or weight-based).
pub fn is_heading_candidate(lines: &[Line], stats: &DocStats) -> bool {
    if lines.is_empty() || lines.len() > MAX_HEADING_LINES || lines.iter().any(Line::all_mono) {
        return false;
    }
    let text: String = lines.iter().map(Line::text).collect::<Vec<_>>().join(" ");
    let chars = text.chars().count();
    if chars > MAX_HEADING_CHARS || !text.chars().any(char::is_alphabetic) {
        return false;
    }
    let size = crate::lines::dominant_size(&lines.iter().flat_map(|l| l.spans.iter().cloned()).collect::<Vec<_>>());
    if size >= stats.body_size * HEADING_SIZE_RATIO {
        return true;
    }
    let trimmed = text.trim_end();
    lines.len() <= 2
        && lines.iter().all(Line::all_bold)
        && chars <= MAX_BOLD_HEADING_CHARS
        && size >= stats.body_size * 0.95
        && !trimmed.ends_with(['.', ',', ';', ':'])
        && markers::parse(trimmed).is_none()
}

/// Heading identity used for level assignment: size bucket and weight.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct HeadingKey {
    half_points: i32,
    bold: bool,
}

pub fn classify(raw: Vec<RawBlock>, stats: &DocStats) -> Vec<Block> {
    // Pass 1: decide kinds; collect heading keys for level assignment.
    enum Kind {
        Heading(HeadingKey),
        List,
        Code,
        Para,
    }
    let kinds: Vec<Option<Kind>> = raw
        .iter()
        .map(|b| match &b.kind {
            RawKind::Table(_) => None,
            RawKind::Text(lines) => Some(if lines.iter().all(Line::all_mono) {
                Kind::Code
            } else if is_heading_candidate(lines, stats) {
                let size = lines.iter().map(|l| l.size).fold(0.0f32, f32::max);
                Kind::Heading(HeadingKey { half_points: (size * 2.0).round() as i32, bold: lines.iter().all(Line::all_bold) })
            } else if markers::parse(&lines[0].text()).is_some() {
                Kind::List
            } else {
                Kind::Para
            }),
        })
        .collect();

    let mut keys: Vec<HeadingKey> =
        kinds.iter().filter_map(|k| if let Some(Kind::Heading(h)) = k { Some(*h) } else { None }).collect();
    keys.sort_by(|a, b| b.half_points.cmp(&a.half_points).then(b.bold.cmp(&a.bold)));
    keys.dedup();

    // List depth: rank of the marker's x position among the markers of the
    // current consecutive list run.
    let mut out = Vec::with_capacity(raw.len());
    let mut list_xs: Vec<f32> = Vec::new();
    for (b, k) in raw.into_iter().zip(kinds) {
        if !matches!(k, Some(Kind::List)) {
            list_xs.clear();
        }
        match (b.kind, k) {
            (RawKind::Table(t), _) => out.push(Block::Table { rows: t.rows }),
            (RawKind::Text(lines), Some(Kind::Code)) => out.push(code(&lines)),
            (RawKind::Text(lines), Some(Kind::Heading(key))) => {
                let level = keys.iter().position(|k| *k == key).unwrap_or(0) as u8 + 1;
                let mut inlines = join_lines(&lines);
                for i in &mut inlines {
                    i.style = Style { mono: i.style.mono, ..Style::default() };
                }
                out.push(Block::Heading { level: level.min(6), inlines: trim(inlines) });
            }
            (RawKind::Text(lines), Some(Kind::List)) => {
                let first = lines[0].text();
                let parsed = markers::parse(&first).expect("classified as list");
                let x = lines[0].bbox.x0;
                let depth = match list_xs.iter().position(|&lx| (lx - x).abs() <= 4.0) {
                    Some(d) => {
                        list_xs.truncate(d + 1);
                        d
                    }
                    None => {
                        // Deeper when indented further than the last marker.
                        if list_xs.last().is_some_and(|&lx| x < lx) {
                            list_xs.clear();
                        }
                        list_xs.push(x);
                        list_xs.len() - 1
                    }
                };
                let inlines = strip_prefix(join_lines(&lines), parsed.len);
                out.push(Block::ListItem { depth: depth.min(8) as u8, marker: parsed.marker, inlines: trim(inlines) });
            }
            (RawKind::Text(lines), _) => out.push(Block::Paragraph { inlines: trim(join_lines(&lines)) }),
        }
    }
    out
}

fn code(lines: &[Line]) -> Block {
    let left = lines.iter().map(|l| l.bbox.x0).fold(f32::MAX, f32::min);
    let out = lines
        .iter()
        .map(|l| {
            let char_w = (l.size * 0.6).max(1.0);
            let indent = ((l.bbox.x0 - left) / char_w).round().max(0.0) as usize;
            format!("{}{}", " ".repeat(indent.min(40)), l.text())
        })
        .collect();
    Block::Code { lines: out }
}

/// Concatenate lines into inlines, re-joining hyphenated words.
pub fn join_lines(lines: &[Line]) -> Vec<Inline> {
    let mut out: Vec<Inline> = Vec::new();
    for (n, line) in lines.iter().enumerate() {
        if n > 0 {
            let next_lower = line.text().chars().next().is_some_and(char::is_lowercase);
            let prev = out.iter_mut().rev().find(|i| !i.text.is_empty());
            match prev {
                Some(p) if ends_with_word_hyphen(&p.text) => {
                    if next_lower {
                        p.text.pop();
                    }
                }
                _ => out.push(Inline::plain(" ")),
            }
        }
        for s in &line.spans {
            out.push(Inline { text: s.text.clone(), style: s.style, link: s.link.clone() });
        }
    }
    out
}

fn ends_with_word_hyphen(t: &str) -> bool {
    let mut it = t.chars().rev();
    matches!(it.next(), Some('-') | Some('\u{2010}')) && it.next().is_some_and(char::is_alphabetic)
}

/// Remove the first `n` bytes of text (a list marker) across inlines.
fn strip_prefix(mut inlines: Vec<Inline>, mut n: usize) -> Vec<Inline> {
    for i in &mut inlines {
        if n == 0 {
            break;
        }
        let take = n.min(i.text.len());
        let take = (0..=take).rev().find(|&k| i.text.is_char_boundary(k)).unwrap_or(0);
        i.text.drain(..take);
        n -= take;
        if take == 0 {
            break;
        }
    }
    inlines
}

fn trim(mut inlines: Vec<Inline>) -> Vec<Inline> {
    inlines.retain(|i| !i.text.is_empty());
    if let Some(f) = inlines.first_mut() {
        let lead = f.text.len() - f.text.trim_start().len();
        f.text.drain(..lead);
    }
    if let Some(l) = inlines.last_mut() {
        l.text.truncate(l.text.trim_end().len());
    }
    inlines.retain(|i| !i.text.is_empty());
    inlines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lines::{build, tests::sp};
    use crate::model::Span;
    use crate::render::render;

    fn bold(mut s: Span) -> Span {
        s.style.bold = true;
        s
    }

    fn raw(spans: Vec<Span>) -> RawBlock {
        RawBlock { kind: RawKind::Text(build(spans)), soft_start: false, page: 0 }
    }

    fn stats() -> DocStats {
        DocStats { body_size: 10.0, pitch_ratio: 1.2 }
    }

    #[test]
    fn heading_levels_by_size_then_weight() {
        let blocks = classify(
            vec![
                raw(vec![sp("Title", 72.0, 50.0, 24.0)]),
                raw(vec![sp("Section", 72.0, 100.0, 16.0)]),
                raw(vec![bold(sp("Subsection", 72.0, 150.0, 10.0))]),
                raw(vec![sp("Body text here.", 72.0, 170.0, 10.0)]),
                raw(vec![sp("Another section", 72.0, 200.0, 16.0)]),
            ],
            &stats(),
        );
        assert_eq!(render(&blocks), "# Title\n\n## Section\n\n### Subsection\n\nBody text here.\n\n## Another section\n");
    }

    #[test]
    fn hyphenated_line_end_rejoined() {
        let blocks = classify(
            vec![raw(vec![
                sp("the inter-", 72.0, 100.0, 10.0),
                sp("national market and Franco-", 72.0, 112.0, 10.0),
                sp("German trade", 72.0, 124.0, 10.0),
            ])],
            &stats(),
        );
        assert_eq!(render(&blocks), "the international market and Franco-German trade\n");
    }

    #[test]
    fn nested_list_depths() {
        let blocks = classify(
            vec![
                raw(vec![sp("• One", 72.0, 100.0, 10.0)]),
                raw(vec![sp("◦ Nested", 90.0, 112.0, 10.0)]),
                raw(vec![sp("• Two", 72.0, 124.0, 10.0)]),
                raw(vec![sp("2) Second", 72.0, 136.0, 10.0)]),
            ],
            &stats(),
        );
        assert_eq!(render(&blocks), "- One\n  - Nested\n- Two\n2) Second\n");
    }

    #[test]
    fn bold_sentence_is_not_a_heading() {
        let blocks = classify(vec![raw(vec![bold(sp("Note: this is important.", 72.0, 100.0, 10.0))])], &stats());
        assert!(matches!(blocks[0], Block::Paragraph { .. }));
    }

    #[test]
    fn monospace_block_is_code_with_indent() {
        let mut a = sp("fn main() {", 72.0, 100.0, 10.0);
        let mut b = sp("println!();", 96.0, 112.0, 10.0);
        a.style.mono = true;
        b.style.mono = true;
        let blocks = classify(vec![raw(vec![a, b])], &stats());
        assert_eq!(blocks[0], Block::Code { lines: vec!["fn main() {".into(), "    println!();".into()] });
    }
}
