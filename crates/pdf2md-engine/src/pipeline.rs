//! Orchestration of the conversion stages.
//!
//! ```text
//! open → [parallel per page: spans → lines → rules if tabular]
//!      → furniture → stats → per page: tables → reading order → segmentation
//!      → flow (cross-page) → classify → render
//! ```
//!
//! Page extraction dominates the run time (over 90 %), so it is spread over
//! worker threads. `PdfDocument` is neither `Send` nor `Sync`, so every
//! worker opens its own handle on the same bytes; pages are handed out
//! through a shared counter so uneven pages balance across workers.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc;

use crate::extract::{self, PageExtract};
use crate::furniture::{self, LineProbe};
use crate::lines::{self, Line};
use crate::model::{BBox, Rule};
use crate::order::reading_order;
use crate::segment::{segment, Item, RawBlock};
use crate::stats::DocStats;
use crate::{classify, flow, render, tables};
use crate::{CancelToken, Conversion, ConvertError, Options, Progress, Warning};

/// Pages per worker below which another worker does not pay for re-opening
/// the document (xref parsing, font loading).
const MIN_PAGES_PER_WORKER: usize = 8;
/// Heap budget for worker document handles. Each worker holds its own
/// handle; measured peak cost is about 3.3x the file size per worker (two
/// copies of the bytes inside pdf_oxide plus its object cache), estimated
/// here as 3.5x + 8 MiB. Large files therefore get fewer workers.
const WORKER_MEMORY_BUDGET: usize = 192 * 1024 * 1024;
const WORKER_FIXED_COST: usize = 8 * 1024 * 1024;
/// Generous stacks: hostile PDFs can nest content deeply, and a stack
/// overflow aborts the whole process.
const WORKER_STACK: usize = 16 * 1024 * 1024;

/// One page after extraction, ready for layout.
#[derive(Debug, Default)]
pub struct PageData {
    pub index: usize,
    pub width: f32,
    pub height: f32,
    pub lines: Vec<Line>,
    pub rules: Vec<Rule>,
}

enum PageResult {
    Ok { data: PageData, undecodable: usize },
    Failed(String),
}

pub fn run(
    bytes: Vec<u8>,
    opts: &Options,
    progress: &(dyn Fn(Progress) + Sync),
    cancel: &CancelToken,
) -> Result<Conversion, ConvertError> {
    let timing = std::env::var_os("PDF2MD_TIMING").is_some();
    let t_start = std::time::Instant::now();
    let doc = extract::open(bytes.clone(), opts.password.as_deref())?;
    let total = extract::page_count(&doc)?;
    if total == 0 {
        return Err(ConvertError::Corrupt("the document has no pages".into()));
    }
    progress(Progress { done: 0, total });

    let workers = worker_count(opts.threads, total, bytes.len());
    let mut results: Vec<Option<PageResult>> = (0..total).map(|_| None).collect();
    let next = AtomicUsize::new(0);
    let mut done = 0usize;
    let (tx, rx) = mpsc::channel::<(usize, PageResult)>();

    std::thread::scope(|scope| {
        for _ in 1..workers {
            let tx = tx.clone();
            let bytes = bytes.clone();
            let (next, cancel) = (&next, cancel);
            let password = opts.password.clone();
            let spawned = std::thread::Builder::new()
                .stack_size(WORKER_STACK)
                .spawn_scoped(scope, move || {
                    // A worker that cannot open the document simply contributes
                    // nothing; the calling thread processes every remaining page.
                    if let Ok(doc) = extract::open(bytes, password.as_deref()) {
                        work(&doc, next, total, cancel, |i, r| tx.send((i, r)).is_ok());
                    }
                });
            if spawned.is_err() {
                break; // out of threads: continue with fewer workers
            }
        }
        drop(tx);
        drop(bytes);
        // The calling thread is a worker too and reports progress in between.
        work(&doc, &next, total, cancel, |i, r| {
            results[i] = Some(r);
            done += 1;
            while let Ok((j, r)) = rx.try_recv() {
                results[j] = Some(r);
                done += 1;
            }
            progress(Progress { done, total });
            true
        });
        for (j, r) in rx.iter() {
            results[j] = Some(r);
            done += 1;
            progress(Progress { done, total });
        }
    });
    drop(doc);
    if cancel.is_cancelled() {
        return Err(ConvertError::Cancelled);
    }

    let mut warnings = Vec::new();
    let mut pages = Vec::with_capacity(total);
    let mut failed = 0usize;
    for (index, r) in results.into_iter().enumerate() {
        match r {
            Some(PageResult::Ok { data, undecodable }) => {
                if undecodable > 0 {
                    warnings.push(Warning {
                        page: Some(index),
                        message: format!(
                            "Page {}: {} character(s) could not be decoded and were omitted.",
                            index + 1,
                            undecodable
                        ),
                    });
                }
                pages.push(data);
            }
            Some(PageResult::Failed(e)) => {
                failed += 1;
                warnings.push(Warning {
                    page: Some(index),
                    message: format!(
                        "Page {} could not be read and was skipped ({e}).",
                        index + 1
                    ),
                });
                pages.push(PageData {
                    index,
                    ..PageData::default()
                });
            }
            None => unreachable!("every page index is claimed exactly once"),
        }
    }
    if failed == total {
        return Err(ConvertError::Corrupt("no page could be read".into()));
    }

    let t_layout = std::time::Instant::now();
    let markdown = layout(pages);
    if timing {
        eprintln!(
            "timing: {} worker(s), extract {:.1} ms, layout {:.1} ms",
            workers,
            (t_layout - t_start).as_secs_f64() * 1e3,
            t_layout.elapsed().as_secs_f64() * 1e3
        );
    }
    if markdown.trim().is_empty() {
        return Err(ConvertError::NoTextLayer);
    }
    Ok(Conversion {
        markdown,
        pages: total,
        warnings,
    })
}

fn worker_count(requested: Option<usize>, pages: usize, file_len: usize) -> usize {
    let cores = requested
        .unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(1)
        })
        .max(1);
    let by_pages = pages.div_ceil(MIN_PAGES_PER_WORKER).max(1);
    let per_worker = file_len.saturating_mul(7) / 2 + WORKER_FIXED_COST;
    let by_memory = (WORKER_MEMORY_BUDGET / per_worker).max(1);
    cores.min(by_pages).min(by_memory)
}

/// Claim pages from `next` until none are left or the run is cancelled;
/// `sink` receives each result and returns `false` to stop early.
fn work(
    doc: &pdf_oxide::PdfDocument,
    next: &AtomicUsize,
    total: usize,
    cancel: &CancelToken,
    mut sink: impl FnMut(usize, PageResult) -> bool,
) {
    loop {
        if cancel.is_cancelled() {
            return;
        }
        let i = next.fetch_add(1, Ordering::Relaxed);
        if i >= total {
            return;
        }
        let r = match extract::page(doc, i) {
            Ok(page) => prepare(doc, page),
            Err(e) => PageResult::Failed(e),
        };
        if !sink(i, r) {
            return;
        }
    }
}

/// Per-page work that needs only this page: line building and, for pages
/// that look tabular, ruling lines.
fn prepare(doc: &pdf_oxide::PdfDocument, page: PageExtract) -> PageResult {
    let PageExtract {
        content,
        undecodable,
        frame,
    } = page;
    let lines = lines::build(content.spans);
    let rules = if may_hold_table(&lines) {
        extract::rules(doc, content.index, &frame)
    } else {
        Vec::new()
    };
    PageResult::Ok {
        data: PageData {
            index: content.index,
            width: content.width,
            height: content.height,
            lines,
            rules,
        },
        undecodable,
    }
}

/// A table needs at least two rows that each split into several fragments.
/// Pages without such rows never use ruling lines, so their (costly) vector
/// path extraction is skipped.
fn may_hold_table(lines: &[Line]) -> bool {
    let mut multi_rows = 0;
    let mut last_row = usize::MAX;
    for w in lines.windows(2) {
        if w[0].row == w[1].row && w[0].row != last_row {
            last_row = w[0].row;
            multi_rows += 1;
            if multi_rows >= 2 {
                return true;
            }
        }
    }
    false
}

/// Pure layout half of the pipeline: extracted pages → Markdown.
pub fn layout(mut pages: Vec<PageData>) -> String {
    // Furniture, document-wide.
    let texts: Vec<Vec<String>> = pages
        .iter()
        .map(|p| p.lines.iter().map(Line::text).collect())
        .collect();
    let probes: Vec<(f32, Vec<LineProbe<'_>>)> = pages
        .iter()
        .zip(&texts)
        .map(|(p, ts)| {
            (
                p.height,
                p.lines
                    .iter()
                    .zip(ts)
                    .map(|(l, t)| LineProbe {
                        text: t,
                        y0: l.bbox.y0,
                        y1: l.bbox.y1,
                    })
                    .collect(),
            )
        })
        .collect();
    let drop_flags = furniture::detect(&probes);
    drop(probes);
    drop(texts);
    for (p, flags) in pages.iter_mut().zip(drop_flags) {
        let mut it = flags.into_iter();
        p.lines.retain(|_| !it.next().unwrap_or(false));
    }

    let stats = DocStats::from_lines(pages.iter().flat_map(|p| p.lines.iter()));
    if std::env::var_os("PDF2MD_DEBUG_LINES").is_some() {
        eprintln!("{stats:?}");
        for p in &pages {
            eprintln!("--- page {} ({}x{})", p.index, p.width, p.height);
            for l in &p.lines {
                let b = l.bbox;
                eprintln!(
                    "{:6.1} {:6.1} {:6.1} {:6.1} sz{:4.1} row{:3} {:?}",
                    b.x0,
                    b.y0,
                    b.x1,
                    b.y1,
                    l.size,
                    l.row,
                    l.text()
                );
            }
        }
    }

    // Per page: tables, reading order, segmentation.
    let mut raw: Vec<RawBlock> = Vec::new();
    for page in pages {
        raw.extend(page_blocks(page, &stats));
    }
    let raw = flow::join(raw, &stats);
    let blocks = classify::classify(raw, &stats);
    render::render(&blocks)
}

fn page_blocks(page: PageData, stats: &DocStats) -> Vec<RawBlock> {
    let PageData {
        index,
        width,
        lines,
        rules,
        ..
    } = page;
    if lines.is_empty() {
        return Vec::new();
    }
    let tables = tables::detect(&lines, &rules);
    let mut consumed = vec![false; lines.len()];
    for t in &tables {
        for &i in &t.consumed {
            consumed[i] = true;
        }
    }
    let mut items: Vec<Option<Item>> = Vec::with_capacity(lines.len());
    let mut boxes = Vec::with_capacity(lines.len());
    for t in tables {
        boxes.push(t.bbox);
        items.push(Some(Item::Table(t)));
    }
    for (l, used) in lines.into_iter().zip(consumed) {
        if !used {
            boxes.push(l.bbox);
            items.push(Some(Item::Line(l)));
        }
    }
    let frame = text_frame(&boxes, width);
    let order = reading_order(&boxes, stats.body_size, frame);
    let ordered: Vec<(Item, f32)> = order
        .into_iter()
        .filter_map(|p| items[p.index].take().map(|it| (it, p.col_x1)))
        .collect();
    segment(ordered, index, stats)
}

/// Horizontal extent of the page's text area.
///
/// The longest line gives the right edge, unless every line on the page is
/// short (a page of short paragraphs, labels or a letter head): then the
/// left margin mirrored on the right is the better estimate, because the
/// text area is wider than any line actually set on the page.
fn text_frame(boxes: &[BBox], page_width: f32) -> (f32, f32) {
    let x0 = boxes.iter().map(|b| b.x0).fold(f32::MAX, f32::min);
    let x1 = boxes.iter().map(|b| b.x1).fold(f32::MIN, f32::max);
    let mirrored = page_width - x0;
    if mirrored > x0 && (x1 - x0) < 0.6 * (mirrored - x0) {
        (x0, mirrored)
    } else {
        (x0, x1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_count_respects_pages_memory_and_request() {
        assert_eq!(worker_count(Some(4), 3, 1_000), 1);
        assert_eq!(worker_count(Some(4), 100, 1_000), 4);
        assert_eq!(worker_count(Some(4), 17, 1_000), 3);
        // 22.5 MB file: 3.5x + 8 MiB = ~87 MB per worker -> 2 within 192 MiB.
        assert_eq!(worker_count(Some(8), 1000, 22_500_000), 2);
        assert_eq!(worker_count(Some(8), 1000, 100_000_000), 1);
        assert_eq!(worker_count(Some(1), 1000, 1_000), 1);
    }
}
