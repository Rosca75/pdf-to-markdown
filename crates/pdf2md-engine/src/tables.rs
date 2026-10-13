//! Table detection on a page's line fragments.
//!
//! Two independent detectors, ruled first:
//!
//! * **Ruled grids** — connected components of ruling lines with at least two
//!   horizontal and two vertical rules define a cell grid; fragments are
//!   assigned to cells by their centre. A component that yields a single row
//!   or column is a frame (call-out box), not a table.
//! * **Aligned columns** — a run of consecutive rows that each contain
//!   several fragments falling into consistent column bands. Prose laid out
//!   in columns is rejected by requiring short cells.
//!
//! Detection is best-effort by nature; the output is a GFM table whose first
//! row is the header.

use crate::lines::Line;
use crate::model::{BBox, Inline, Rule, Style};

/// A detected table: its area, the fragments it consumed and its cells.
#[derive(Debug, Clone, PartialEq)]
pub struct Table {
    pub bbox: BBox,
    pub consumed: Vec<usize>,
    pub rows: Vec<Vec<Vec<Inline>>>,
}

const SNAP: f32 = 2.5;

pub fn detect(lines: &[Line], rules: &[Rule]) -> Vec<Table> {
    let mut used = vec![false; lines.len()];
    let mut tables = ruled(lines, rules, &mut used);
    tables.extend(aligned(lines, &mut used));
    tables
}

// ---------------------------------------------------------------- ruled ----

fn ruled(lines: &[Line], rules: &[Rule], used: &mut [bool]) -> Vec<Table> {
    let (h, v): (Vec<&Rule>, Vec<&Rule>) = rules
        .iter()
        .filter(|r| r.bbox.width().max(r.bbox.height()) >= 5.0)
        .partition(|r| r.bbox.width() >= r.bbox.height());
    if h.len() < 2 || v.len() < 2 {
        return Vec::new();
    }
    let all: Vec<&Rule> = h.iter().chain(v.iter()).copied().collect();
    let mut tables = Vec::new();
    for comp in components(&all) {
        let hs: Vec<BBox> = comp.iter().map(|&i| all[i].bbox).filter(|b| b.width() >= b.height()).collect();
        let vs: Vec<BBox> = comp.iter().map(|&i| all[i].bbox).filter(|b| b.width() < b.height()).collect();
        if hs.len() < 2 || vs.len() < 2 {
            continue;
        }
        let ys = cluster(hs.iter().map(|b| (b.y0 + b.y1) / 2.0).collect());
        let xs = cluster(vs.iter().map(|b| (b.x0 + b.x1) / 2.0).collect());
        if ys.len() < 3 && xs.len() < 3 {
            continue; // a single framed cell: a box, not a table
        }
        let bbox = BBox::new(xs[0], ys[0], *xs.last().unwrap(), *ys.last().unwrap());
        let (nr, nc) = (ys.len() - 1, xs.len() - 1);
        let mut grid: Vec<Vec<Vec<usize>>> = vec![vec![Vec::new(); nc]; nr];
        let mut consumed = Vec::new();
        for (i, l) in lines.iter().enumerate() {
            if used[i] {
                continue;
            }
            let (cx, cy) = ((l.bbox.x0 + l.bbox.x1) / 2.0, (l.bbox.y0 + l.bbox.y1) / 2.0);
            if cx < bbox.x0 || cx > bbox.x1 || cy < bbox.y0 || cy > bbox.y1 {
                continue;
            }
            let r = ys.windows(2).position(|w| cy >= w[0] && cy <= w[1]);
            // A fragment straddling several columns (a cell whose text
            // overflows) is assigned by its left edge.
            let c = xs.windows(2).position(|w| l.bbox.x0 + 1.0 >= w[0] && l.bbox.x0 + 1.0 <= w[1]);
            if let (Some(r), Some(c)) = (r, c) {
                grid[r][c].push(i);
                consumed.push(i);
            }
        }
        let rows: Vec<Vec<Vec<Inline>>> = grid
            .into_iter()
            .filter(|row| row.iter().any(|c| !c.is_empty()))
            .map(|row| row.into_iter().map(|cell| cell_inlines(lines, &cell)).collect())
            .collect();
        if rows.len() < 2 || rows.iter().map(|r| r.iter().filter(|c| !c.is_empty()).count()).max() < Some(2) {
            continue;
        }
        for &i in &consumed {
            used[i] = true;
        }
        tables.push(Table { bbox, consumed, rows: header_plain(rows) });
    }
    tables
}

/// Union-find over rules that touch (within `SNAP`).
fn components(rules: &[&Rule]) -> Vec<Vec<usize>> {
    let n = rules.len();
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(p: &mut [usize], mut i: usize) -> usize {
        while p[i] != i {
            p[i] = p[p[i]];
            i = p[i];
        }
        i
    }
    for i in 0..n {
        for j in i + 1..n {
            let (a, b) = (rules[i].bbox, rules[j].bbox);
            let grown = BBox::new(a.x0 - SNAP, a.y0 - SNAP, a.x1 + SNAP, a.y1 + SNAP);
            if grown.intersects(&b) {
                let (ri, rj) = (find(&mut parent, i), find(&mut parent, j));
                parent[ri] = rj;
            }
        }
    }
    let mut groups: std::collections::HashMap<usize, Vec<usize>> = std::collections::HashMap::new();
    for i in 0..n {
        let r = find(&mut parent, i);
        groups.entry(r).or_default().push(i);
    }
    let mut out: Vec<Vec<usize>> = groups.into_values().collect();
    out.sort();
    out
}

/// Sort and merge coordinates closer than `SNAP`.
fn cluster(mut v: Vec<f32>) -> Vec<f32> {
    v.sort_by(f32::total_cmp);
    let mut out: Vec<f32> = Vec::new();
    for x in v {
        match out.last() {
            Some(&l) if x - l <= SNAP => {}
            _ => out.push(x),
        }
    }
    out
}

// -------------------------------------------------------------- aligned ----

/// Cells longer than this (characters, median) indicate prose, not a table.
const MAX_MEDIAN_CELL_CHARS: usize = 40;
/// Mean fragment width relative to the run width above which a 2-column run
/// is considered column-set prose.
const MAX_TWO_COL_WIDTH_RATIO: f32 = 0.38;

fn aligned(lines: &[Line], used: &mut [bool]) -> Vec<Table> {
    // Group unused fragments by row, rows in top-down order.
    let mut rows: Vec<Vec<usize>> = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        if used[i] {
            continue;
        }
        match rows.last_mut() {
            Some(r) if lines[r[0]].row == l.row => r.push(i),
            _ => rows.push(vec![i]),
        }
    }
    for r in &mut rows {
        r.sort_by(|&a, &b| lines[a].bbox.x0.total_cmp(&lines[b].bbox.x0));
    }

    let mut tables = Vec::new();
    let mut start = 0;
    while start < rows.len() {
        if rows[start].len() < 2 {
            start += 1;
            continue;
        }
        // Extend the run while rows are multi-fragment and vertically close.
        let mut end = start + 1;
        while end < rows.len() && rows[end].len() >= 2 && close(lines, &rows[end - 1], &rows[end]) {
            end += 1;
        }
        if end - start >= 3 {
            if let Some(t) = build_aligned(lines, &rows[start..end]) {
                for &i in &t.consumed {
                    used[i] = true;
                }
                tables.push(t);
            }
        }
        start = end;
    }
    tables
}

fn close(lines: &[Line], a: &[usize], b: &[usize]) -> bool {
    let ay1 = a.iter().map(|&i| lines[i].bbox.y1).fold(f32::MIN, f32::max);
    let by0 = b.iter().map(|&i| lines[i].bbox.y0).fold(f32::MAX, f32::min);
    let size = lines[a[0]].size.max(lines[b[0]].size);
    by0 - ay1 <= 1.6 * size
}

fn build_aligned(lines: &[Line], rows: &[Vec<usize>]) -> Option<Table> {
    // Column bands: merge the x-intervals of all fragments in the run.
    let mut intervals: Vec<(f32, f32)> =
        rows.iter().flatten().map(|&i| (lines[i].bbox.x0, lines[i].bbox.x1)).collect();
    intervals.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut bands: Vec<(f32, f32)> = Vec::new();
    for (x0, x1) in intervals {
        match bands.last_mut() {
            Some(b) if x0 <= b.1 => b.1 = b.1.max(x1),
            _ => bands.push((x0, x1)),
        }
    }
    let nc = bands.len();
    if nc < 2 {
        return None;
    }
    // Every row must place its fragments in distinct bands.
    let mut grid: Vec<Vec<Vec<usize>>> = Vec::with_capacity(rows.len());
    for r in rows {
        let mut cells = vec![Vec::new(); nc];
        for &i in r {
            let c = bands.iter().position(|b| lines[i].bbox.x0 >= b.0 - 0.5 && lines[i].bbox.x0 <= b.1)?;
            if !cells[c].is_empty() {
                return None;
            }
            cells[c].push(i);
        }
        grid.push(cells);
    }
    // Reject prose set in columns.
    let mut lens: Vec<usize> = rows.iter().flatten().map(|&i| lines[i].text().chars().count()).collect();
    lens.sort_unstable();
    if lens[lens.len() / 2] > MAX_MEDIAN_CELL_CHARS {
        return None;
    }
    let run = rows.iter().flatten().fold(lines[rows[0][0]].bbox, |b, &i| b.union(&lines[i].bbox));
    if nc == 2 {
        let mean_w: f32 = rows.iter().flatten().map(|&i| lines[i].bbox.width()).sum::<f32>()
            / rows.iter().map(Vec::len).sum::<usize>() as f32;
        if mean_w / run.width().max(1.0) > MAX_TWO_COL_WIDTH_RATIO {
            return None;
        }
    }
    let consumed: Vec<usize> = rows.iter().flatten().copied().collect();
    let cells = grid.into_iter().map(|row| row.into_iter().map(|c| cell_inlines(lines, &c)).collect()).collect();
    Some(Table { bbox: run, consumed, rows: header_plain(cells) })
}

// --------------------------------------------------------------- shared ----

fn cell_inlines(lines: &[Line], idx: &[usize]) -> Vec<Inline> {
    let mut sorted = idx.to_vec();
    sorted.sort_by(|&a, &b| lines[a].bbox.y0.total_cmp(&lines[b].bbox.y0).then(lines[a].bbox.x0.total_cmp(&lines[b].bbox.x0)));
    let mut out: Vec<Inline> = Vec::new();
    for (n, &i) in sorted.iter().enumerate() {
        if n > 0 {
            out.push(Inline::plain(" "));
        }
        for s in &lines[i].spans {
            out.push(Inline { text: s.text.clone(), style: s.style, link: s.link.clone() });
        }
    }
    out
}

/// Header cells are rendered plain: GFM already distinguishes the header row.
fn header_plain(mut rows: Vec<Vec<Vec<Inline>>>) -> Vec<Vec<Vec<Inline>>> {
    if let Some(head) = rows.first_mut() {
        for cell in head {
            for inl in cell {
                inl.style = Style { mono: inl.style.mono, ..Style::default() };
            }
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lines::{build, tests::sp};

    fn texts(t: &Table) -> Vec<Vec<String>> {
        t.rows.iter().map(|r| r.iter().map(|c| c.iter().map(|i| i.text.as_str()).collect()).collect()).collect()
    }

    fn rule(x0: f32, y0: f32, x1: f32, y1: f32) -> Rule {
        Rule { bbox: BBox::new(x0, y0, x1, y1) }
    }

    #[test]
    fn aligned_table_detected() {
        let mut spans = Vec::new();
        for (r, row) in [["Name", "Units", "Price"], ["Alpha", "12", "3.50"], ["Beta", "7", "12.00"]].iter().enumerate() {
            for (c, cell) in row.iter().enumerate() {
                spans.push(sp(cell, 72.0 + c as f32 * 150.0, 100.0 + r as f32 * 15.0, 11.0));
            }
        }
        let lines = build(spans);
        let t = detect(&lines, &[]);
        assert_eq!(t.len(), 1);
        assert_eq!(texts(&t[0])[1], vec!["Alpha", "12", "3.50"]);
        assert_eq!(t[0].consumed.len(), 9);
    }

    #[test]
    fn two_column_prose_is_not_a_table() {
        let mut spans = Vec::new();
        for r in 0..5 {
            spans.push(sp("Left column prose line with words", 72.0, 100.0 + r as f32 * 14.0, 11.0));
            spans.push(sp("Right column prose line with words", 320.0, 100.0 + r as f32 * 14.0, 11.0));
        }
        let lines = build(spans);
        assert!(detect(&lines, &[]).is_empty());
    }

    #[test]
    fn ruled_grid_detected_with_cells() {
        let mut spans = Vec::new();
        for (r, row) in [["Item", "Qty"], ["Bolts", "4"], ["Nuts", "8"]].iter().enumerate() {
            for (c, cell) in row.iter().enumerate() {
                spans.push(sp(cell, 105.0 + c as f32 * 100.0, 105.0 + r as f32 * 20.0, 10.0));
            }
        }
        let mut rules = Vec::new();
        for r in 0..4 {
            let y = 100.0 + r as f32 * 20.0;
            rules.push(rule(100.0, y, 300.0, y + 0.5));
        }
        for c in 0..3 {
            let x = 100.0 + c as f32 * 100.0;
            rules.push(rule(x, 100.0, x + 0.5, 160.0));
        }
        let lines = build(spans);
        let t = detect(&lines, &rules);
        assert_eq!(t.len(), 1);
        assert_eq!(texts(&t[0]), vec![vec!["Item", "Qty"], vec!["Bolts", "4"], vec!["Nuts", "8"]]);
    }

    #[test]
    fn framed_box_is_not_a_table() {
        let lines = build(vec![sp("A call-out note", 110.0, 110.0, 10.0)]);
        let rules = vec![rule(100.0, 100.0, 300.0, 100.5), rule(100.0, 140.0, 300.0, 140.5), rule(100.0, 100.0, 100.5, 140.0), rule(300.0, 100.0, 300.5, 140.0)];
        assert!(detect(&lines, &rules).is_empty());
    }
}
