//! Reading order by recursive XY-cut.
//!
//! A region is split along the widest whitespace channel. Vertical channels
//! (column gutters) are preferred over horizontal ones, because cutting a
//! multi-column region horizontally interleaves its columns. A vertical
//! channel only counts when both sides are substantial text — a column of
//! list bullets or a narrow number column is not a text column.

use crate::model::BBox;

/// Minimum gutter width, in multiples of the body font size.
const MIN_GUTTER_EM: f32 = 0.9;
/// Minimum width of each side of a vertical cut, in multiples of body size.
const MIN_COLUMN_EM: f32 = 6.0;
/// Horizontal gaps at least this fraction of the widest one are cut together.
const H_CUT_SHARE: f32 = 0.75;
/// Recursion limit. Real layouts nest a handful of levels; the limit only
/// guards the stack against pathological inputs (a stack overflow aborts
/// the process and cannot be caught).
const MAX_DEPTH: usize = 64;

/// One item in reading order with the horizontal extent of the text
/// column it belongs to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placed {
    pub index: usize,
    pub col_x0: f32,
    pub col_x1: f32,
}

/// Return the indices of `boxes` in reading order. `frame` is the
/// horizontal extent of the page's text area, used as the column extent
/// until a vertical cut narrows it.
pub fn reading_order(boxes: &[BBox], body_size: f32, frame: (f32, f32)) -> Vec<Placed> {
    let mut out = Vec::with_capacity(boxes.len());
    let idx: Vec<usize> = (0..boxes.len()).collect();
    cut(boxes, idx, body_size.max(1.0), frame, 0, &mut out);
    out
}

fn cut(
    boxes: &[BBox],
    mut idx: Vec<usize>,
    em: f32,
    col: (f32, f32),
    depth: usize,
    out: &mut Vec<Placed>,
) {
    let place = |i: usize| Placed {
        index: i,
        col_x0: col.0,
        col_x1: col.1,
    };
    if idx.len() <= 1 {
        out.extend(idx.into_iter().map(place));
        return;
    }
    if depth >= MAX_DEPTH {
        positional_order(boxes, &mut idx);
        out.extend(idx.into_iter().map(place));
        return;
    }
    if let Some(x) = best_vertical(boxes, &mut idx, em) {
        let (left, right): (Vec<usize>, Vec<usize>) =
            idx.into_iter().partition(|&i| boxes[i].x1 <= x);
        let right_x0 = right.iter().map(|&i| boxes[i].x0).fold(f32::MAX, f32::min);
        cut(boxes, left, em, (col.0, x), depth + 1, out);
        cut(boxes, right, em, (right_x0, col.1), depth + 1, out);
        return;
    }
    // Horizontal: sweep in y order and cut at every wide-enough gap.
    idx.sort_by(|&a, &b| {
        boxes[a]
            .y0
            .total_cmp(&boxes[b].y0)
            .then(boxes[a].x0.total_cmp(&boxes[b].x0))
    });
    let mut gaps: Vec<(usize, f32)> = Vec::new(); // (split position in idx, gap width)
    let mut reach = boxes[idx[0]].y1;
    for (k, &i) in idx.iter().enumerate().skip(1) {
        let b = &boxes[i];
        // Boxes that touch (within a hair) still separate cleanly; their gap
        // counts as zero width. Clamping keeps every gap >= 0 so the widest
        // gap always meets the cut threshold and each level makes progress.
        if b.y0 + 0.01 > reach {
            gaps.push((k, (b.y0 - reach).max(0.0)));
        }
        reach = reach.max(b.y1);
    }
    if gaps.is_empty() {
        // Overlapping band with no clean channel.
        positional_order(boxes, &mut idx);
        out.extend(idx.into_iter().map(place));
        return;
    }
    let widest = gaps.iter().map(|g| g.1).fold(0.0f32, f32::max);
    let mut start = 0;
    for &(k, w) in &gaps {
        if w >= widest * H_CUT_SHARE {
            cut(boxes, idx[start..k].to_vec(), em, col, depth + 1, out);
            start = k;
        }
    }
    debug_assert!(start > 0, "a horizontal cut must split the region");
    cut(boxes, idx[start..].to_vec(), em, col, depth + 1, out);
}

/// Left-to-right within a shared baseline band, otherwise top-to-bottom.
fn positional_order(boxes: &[BBox], idx: &mut [usize]) {
    idx.sort_by(|&a, &b| {
        let (ba, bb) = (&boxes[a], &boxes[b]);
        if ba.v_overlap(bb) > 0.5 * ba.height().min(bb.height()) {
            ba.x0.total_cmp(&bb.x0)
        } else {
            ba.y0.total_cmp(&bb.y0)
        }
    });
}

/// Widest valid vertical channel, as the x coordinate of its left edge.
fn best_vertical(boxes: &[BBox], idx: &mut [usize], em: f32) -> Option<f32> {
    idx.sort_by(|&a, &b| boxes[a].x0.total_cmp(&boxes[b].x0));
    let region_x0 = boxes[idx[0]].x0;
    let region_x1 = idx.iter().map(|&i| boxes[i].x1).fold(f32::MIN, f32::max);
    let mut best: Option<(f32, f32)> = None; // (gap left edge, width)
    let mut reach = boxes[idx[0]].x1;
    for &i in idx.iter().skip(1) {
        let b = &boxes[i];
        let gap = b.x0 - reach;
        if gap >= MIN_GUTTER_EM * em
            && reach - region_x0 >= MIN_COLUMN_EM * em
            && region_x1 - b.x0 >= MIN_COLUMN_EM * em
            && best.is_none_or(|(_, w)| gap > w)
        {
            best = Some((reach, gap));
        }
        reach = reach.max(b.x1);
    }
    best.map(|(x, _)| x)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b(x0: f32, y0: f32, x1: f32, y1: f32) -> BBox {
        BBox::new(x0, y0, x1, y1)
    }

    fn reading_order(boxes: &[BBox], em: f32) -> Vec<usize> {
        super::reading_order(boxes, em, (72.0, 540.0))
            .into_iter()
            .map(|p| p.index)
            .collect()
    }

    #[test]
    fn touching_and_slightly_overlapping_lines_terminate() {
        // Gaps of exactly 0 and of -0.005 (within the touch tolerance) used
        // to produce no cut and recurse forever on the same region.
        let boxes = vec![
            b(72.0, 100.0, 300.0, 110.0),
            b(72.0, 109.995, 300.0, 120.0),
            b(72.0, 120.0, 300.0, 130.0),
        ];
        assert_eq!(reading_order(&boxes, 10.0), vec![0, 1, 2]);
    }

    #[test]
    fn deep_inputs_do_not_exhaust_the_stack() {
        // Staircase of shrinking gaps: each level can cut only one gap.
        let mut boxes = Vec::new();
        let mut y = 0.0;
        for i in 0..5000 {
            boxes.push(b(72.0, y, 300.0, y + 1.0));
            y += 1.0 + 1000.0 / (i as f32 + 1.0);
        }
        let order = reading_order(&boxes, 10.0);
        assert_eq!(order.len(), 5000);
        assert!(order.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn columns_get_their_own_extent() {
        let boxes = vec![b(72.0, 100.0, 290.0, 110.0), b(320.0, 100.0, 540.0, 110.0)];
        let placed = super::reading_order(&boxes, 10.0, (72.0, 540.0));
        assert_eq!((placed[0].col_x0, placed[0].col_x1), (72.0, 290.0));
        assert_eq!((placed[1].col_x0, placed[1].col_x1), (320.0, 540.0));
    }

    #[test]
    fn single_column_top_to_bottom() {
        let boxes = vec![
            b(72.0, 130.0, 500.0, 140.0),
            b(72.0, 100.0, 500.0, 110.0),
            b(72.0, 115.0, 480.0, 125.0),
        ];
        assert_eq!(reading_order(&boxes, 10.0), vec![1, 2, 0]);
    }

    #[test]
    fn two_columns_under_full_width_title() {
        let boxes = vec![
            b(72.0, 60.0, 540.0, 80.0),    // 0 title
            b(72.0, 100.0, 290.0, 110.0),  // 1 L1
            b(320.0, 100.0, 540.0, 110.0), // 2 R1
            b(72.0, 114.0, 290.0, 124.0),  // 3 L2
            b(320.0, 114.0, 540.0, 124.0), // 4 R2
        ];
        assert_eq!(reading_order(&boxes, 10.0), vec![0, 1, 3, 2, 4]);
    }

    #[test]
    fn bullets_column_does_not_split() {
        let boxes = vec![
            b(72.0, 100.0, 78.0, 110.0),
            b(90.0, 100.0, 400.0, 110.0),
            b(72.0, 114.0, 78.0, 124.0),
            b(90.0, 114.0, 400.0, 124.0),
        ];
        assert_eq!(reading_order(&boxes, 10.0), vec![0, 1, 2, 3]);
    }

    #[test]
    fn full_width_footer_after_columns() {
        let boxes = vec![
            b(72.0, 100.0, 290.0, 110.0),  // 0 L1
            b(320.0, 100.0, 540.0, 110.0), // 1 R1
            b(72.0, 114.0, 290.0, 124.0),  // 2 L2
            b(320.0, 114.0, 540.0, 124.0), // 3 R2
            b(72.0, 160.0, 540.0, 170.0),  // 4 footnote
        ];
        assert_eq!(reading_order(&boxes, 10.0), vec![0, 2, 1, 3, 4]);
    }
}
