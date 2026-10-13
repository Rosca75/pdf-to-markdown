//! Re-join text split by page and column breaks.
//!
//! Typesetting breaks paragraphs wherever a column or page ends. A block that
//! starts right after such a break (`soft_start`) continues the previous
//! text block when the previous block did not finish its sentence and both
//! look like the same kind of text.

use crate::classify::is_heading_candidate;
use crate::lines::Line;
use crate::markers;
use crate::segment::{ends_sentence, RawBlock, RawKind};
use crate::stats::DocStats;

pub fn join(blocks: Vec<RawBlock>, stats: &DocStats) -> Vec<RawBlock> {
    let mut out: Vec<RawBlock> = Vec::with_capacity(blocks.len());
    for block in blocks {
        if block.soft_start {
            if let Some(prev) = out.last_mut() {
                if let (RawKind::Text(pl), RawKind::Text(nl)) = (&mut prev.kind, &block.kind) {
                    if continues(pl, nl, stats) {
                        pl.extend(nl.iter().cloned());
                        continue;
                    }
                }
            }
        }
        out.push(block);
    }
    out
}

fn continues(prev: &[Line], next: &[Line], stats: &DocStats) -> bool {
    let (Some(p), Some(n)) = (prev.last(), next.first()) else { return false };
    if is_heading_candidate(prev, stats) || is_heading_candidate(next, stats) {
        return false;
    }
    if (p.size - n.size).abs() >= crate::segment::SIZE_STEP || p.all_mono() != n.all_mono() {
        return false;
    }
    let n_text = n.text();
    if markers::parse(&n_text).is_some() {
        return false;
    }
    let p_text = p.text();
    if p_text.ends_with('-') {
        return true;
    }
    !ends_sentence(&p_text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lines::{build, tests::sp};

    fn block(spans: Vec<crate::model::Span>, soft: bool, page: usize) -> RawBlock {
        RawBlock { kind: RawKind::Text(build(spans)), soft_start: soft, page }
    }

    #[test]
    fn unfinished_sentence_continues_on_next_page() {
        let stats = DocStats::default();
        let blocks = vec![
            block(vec![sp("The paragraph continues onto the", 72.0, 700.0, 10.0)], true, 0),
            block(vec![sp("next page without a break.", 72.0, 72.0, 10.0)], true, 1),
        ];
        let joined = join(blocks, &stats);
        assert_eq!(joined.len(), 1);
        assert_eq!(joined[0].lines().len(), 2);
    }

    #[test]
    fn finished_sentence_does_not_continue() {
        let stats = DocStats::default();
        let blocks = vec![
            block(vec![sp("This paragraph ends here.", 72.0, 700.0, 10.0)], true, 0),
            block(vec![sp("New paragraph on the next page.", 72.0, 72.0, 10.0)], true, 1),
        ];
        assert_eq!(join(blocks, &stats).len(), 2);
    }

    #[test]
    fn hard_break_never_joins() {
        let stats = DocStats::default();
        let blocks = vec![
            block(vec![sp("No terminal punctuation", 72.0, 100.0, 10.0)], true, 0),
            block(vec![sp("separate paragraph", 72.0, 140.0, 10.0)], false, 0),
        ];
        assert_eq!(join(blocks, &stats).len(), 2);
    }
}
