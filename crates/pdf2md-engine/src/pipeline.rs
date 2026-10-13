//! Orchestration of the conversion stages.
//!
//! ```text
//! open → extract pages → lines → furniture → stats → per page:
//!   tables → reading order → segmentation → flow (cross-page) → classify → render
//! ```

use crate::extract;
use crate::furniture::{self, LineProbe};
use crate::lines::{self, Line};
use crate::model::{BBox, PageContent};
use crate::order::reading_order;
use crate::segment::{segment, Item, RawBlock};
use crate::stats::DocStats;
use crate::{classify, flow, render, tables};
use crate::{CancelToken, Conversion, ConvertError, Options, Progress, Warning};

pub fn run(
    bytes: Vec<u8>,
    opts: &Options,
    progress: &(dyn Fn(Progress) + Sync),
    cancel: &CancelToken,
) -> Result<Conversion, ConvertError> {
    let doc = extract::open(bytes, opts.password.as_deref())?;
    let total = extract::page_count(&doc)?;
    if total == 0 {
        return Err(ConvertError::Corrupt("the document has no pages".into()));
    }
    progress(Progress { done: 0, total });

    let mut warnings = Vec::new();
    let mut pages: Vec<PageContent> = Vec::with_capacity(total);
    let mut failed = 0usize;
    for index in 0..total {
        if cancel.is_cancelled() {
            return Err(ConvertError::Cancelled);
        }
        match extract::page(&doc, index) {
            Ok(p) => {
                if p.undecodable > 0 {
                    warnings.push(Warning {
                        page: Some(index),
                        message: format!(
                            "Page {}: {} character(s) could not be decoded and were omitted.",
                            index + 1,
                            p.undecodable
                        ),
                    });
                }
                pages.push(p.content);
            }
            Err(e) => {
                failed += 1;
                warnings.push(Warning {
                    page: Some(index),
                    message: format!("Page {} could not be read and was skipped ({e}).", index + 1),
                });
                pages.push(PageContent { index, ..PageContent::default() });
            }
        }
        progress(Progress { done: index + 1, total });
    }
    drop(doc);
    if failed == total {
        return Err(ConvertError::Corrupt("no page could be read".into()));
    }

    let markdown = layout(pages);
    if markdown.trim().is_empty() {
        return Err(ConvertError::NoTextLayer);
    }
    Ok(Conversion { markdown, pages: total, warnings })
}

/// Pure layout half of the pipeline: page contents → Markdown.
pub fn layout(pages: Vec<PageContent>) -> String {
    // Lines per page.
    let mut page_lines: Vec<(PageContent, Vec<Line>)> = pages
        .into_iter()
        .map(|mut p| {
            let spans = std::mem::take(&mut p.spans);
            let l = lines::build(spans);
            (p, l)
        })
        .collect();

    // Furniture, document-wide.
    let texts: Vec<Vec<String>> = page_lines.iter().map(|(_, ls)| ls.iter().map(Line::text).collect()).collect();
    let probes: Vec<(f32, Vec<LineProbe<'_>>)> = page_lines
        .iter()
        .zip(&texts)
        .map(|((p, ls), ts)| {
            (p.height, ls.iter().zip(ts).map(|(l, t)| LineProbe { text: t, y0: l.bbox.y0, y1: l.bbox.y1 }).collect())
        })
        .collect();
    let drop_flags = furniture::detect(&probes);
    drop(probes);
    for ((_, ls), flags) in page_lines.iter_mut().zip(drop_flags) {
        let mut it = flags.into_iter();
        ls.retain(|_| !it.next().unwrap_or(false));
    }

    let stats = DocStats::from_lines(page_lines.iter().flat_map(|(_, ls)| ls.iter()));
    if std::env::var_os("PDF2MD_DEBUG_LINES").is_some() {
        eprintln!("{stats:?}");
        for (p, ls) in &page_lines {
            eprintln!("--- page {} ({}x{})", p.index, p.width, p.height);
            for l in ls {
                let b = l.bbox;
                eprintln!("{:6.1} {:6.1} {:6.1} {:6.1} sz{:4.1} row{:3} {:?}", b.x0, b.y0, b.x1, b.y1, l.size, l.row, l.text());
            }
        }
    }

    // Per page: tables, reading order, segmentation.
    let mut raw: Vec<RawBlock> = Vec::new();
    for (page, ls) in page_lines {
        raw.extend(page_blocks(page, ls, &stats));
    }
    let raw = flow::join(raw, &stats);
    let blocks = classify::classify(raw, &stats);
    render::render(&blocks)
}

fn page_blocks(page: PageContent, lines: Vec<Line>, stats: &DocStats) -> Vec<RawBlock> {
    if lines.is_empty() {
        return Vec::new();
    }
    let tables = tables::detect(&lines, &page.rules);
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
    let frame = text_frame(&boxes, page.width);
    let order = reading_order(&boxes, stats.body_size, frame);
    let ordered: Vec<(Item, f32)> =
        order.into_iter().filter_map(|p| items[p.index].take().map(|it| (it, p.col_x1))).collect();
    segment(ordered, page.index, stats)
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
