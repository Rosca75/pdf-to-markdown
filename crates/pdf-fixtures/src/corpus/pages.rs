//! Page-level corpus cases: running furniture, margins, page breaks, columns,
//! blank pages and margin stamps.

use super::common::*;
use crate::content::Content;
use crate::fonts::TextFont;
use crate::layout::{paragraph_text, put_lines, text_for_lines, wrap, Rng, StdFonts};
use crate::writer::{PageSpec, Pdf};

const TOPICS: [&str; 6] = ["alpha", "bravo", "charlie", "delta", "echo", "foxtrot"];

pub fn headers_footers() -> Vec<u8> {
    let mut pdf = Pdf::new();
    let f = StdFonts::new(&mut pdf);
    for (i, topic) in TOPICS.iter().enumerate() {
        let n = i + 1;
        let mut rng = Rng(1000 + n as u64);
        let mut p = Page::new();
        p.y = 690.0;
        p.heading(&f.helv_b, 14.0, &format!("Findings on topic {topic}"));
        p.body(
            &f.times,
            &format!(
                "Page content marker {topic} is unique to this page. {}",
                paragraph_text(&mut rng, 6)
            ),
        );
        p.body(&f.times, &paragraph_text(&mut rng, 8));
        // Running header: shifts 36pt to the right on even pages.
        let hx = if n % 2 == 0 { 108.0 } else { 72.0 };
        p.c.text(&f.helv, 9.0, hx, 756.0, "ACME Corp Quarterly Briefing");
        // Footers.
        p.c.text(&f.helv, 9.0, 72.0, 40.0, "Confidential");
        if n <= 4 {
            p.c.text(&f.helv, 9.0, 270.0, 40.0, &format!("Page {n} of 6"));
        } else {
            p.c.text(&f.helv, 9.0, 300.0, 40.0, &n.to_string());
        }
        p.add(&mut pdf, PageSpec::letter());
    }
    finish(&mut pdf)
}

pub fn margin_jumps() -> Vec<u8> {
    let mut pdf = Pdf::new();
    let f = StdFonts::new(&mut pdf);
    // (spec, left, top, width, ordinal)
    let pages: Vec<(PageSpec, f64, f64, f64, &str)> = vec![
        (PageSpec::letter(), 72.0, 720.0, 468.0, "one"),
        (PageSpec::letter(), 108.0, 720.0, 432.0, "two"),
        (PageSpec::a4(), 72.0, 770.0, 451.0, "three"),
        (PageSpec::new(792.0, 612.0), 108.0, 560.0, 576.0, "four"),
        (
            PageSpec::letter().media_box([50.0, 50.0, 662.0, 842.0]),
            122.0,
            770.0,
            468.0,
            "five",
        ),
        (PageSpec::letter().rotate(90), 72.0, 720.0, 468.0, "six"),
    ];
    for (spec, left, top, width, ord) in pages {
        let mut p = Page::at(left, top, width);
        p.heading(&f.helv_b, 14.0, &format!("Margin test {ord}"));
        p.body(
            &f.times,
            &format!(
                "Margin marker {ord} opens the first paragraph of this page, which starts at a left margin of {left} points and runs across the whole text area without any surprises."
            ),
        );
        p.body(
            &f.times,
            &format!(
                "A second paragraph on page {ord} confirms that the reading order is preserved."
            ),
        );
        p.add(&mut pdf, spec);
    }
    finish(&mut pdf)
}

enum Item {
    Heading(&'static str, f64),
    Lines(Vec<String>),
}

/// A page with explicit head items, auto-generated filler that ends exactly where the
/// bottom-anchored `tail` begins.
fn controlled_page(f: &StdFonts, rng: &mut Rng, head: Vec<Item>, tail: &[String]) -> Content {
    let size = 11.0;
    let lead = 14.0;
    let mut c = Content::new();
    let mut y = TOP;
    for item in head {
        match item {
            Item::Heading(text, sz) => {
                c.text(&f.helv_b, sz, LEFT, y, text);
                y -= sz * 1.8;
            }
            Item::Lines(lines) => {
                y = put_lines(&mut c, &f.times, size, LEFT, y, lead, &lines, None) - 6.0;
            }
        }
    }
    let last_baseline = 80.0;
    let tail_first = last_baseline + lead * (tail.len().max(1) - 1) as f64;
    let limit = if tail.is_empty() {
        last_baseline
    } else {
        tail_first + lead + 6.0
    };
    let nf = ((y - limit) / lead).floor() as usize + 1;
    let filler = text_for_lines(rng, &f.times, size, WIDTH, nf);
    put_lines(&mut c, &f.times, size, LEFT, y, lead, &filler, None);
    if !tail.is_empty() {
        put_lines(&mut c, &f.times, size, LEFT, tail_first, lead, tail, None);
    }
    c
}

pub fn split_across_pages() -> Vec<u8> {
    let mut pdf = Pdf::new();
    let f = StdFonts::new(&mut pdf);
    let mut rng = Rng(77);
    let w = |t: &str| wrap(&f.times, 11.0, t, WIDTH);
    let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();

    // Page 1: in-page hyphenation, then a paragraph that continues on page 2.
    let c1 = controlled_page(
        &f,
        &mut rng,
        vec![
            Item::Heading("Logistics review", 20.0),
            Item::Lines(s(&[
                "The new warehouse system improves the manage-",
                "ment of the portfolio and reduces the picking time per order.",
            ])),
        ],
        &w("The steering committee reviewed the freight contracts at length and, after a long discussion of the risks involved in each of them, therefore decided to"),
    );
    // Page 2: continuation, then a paragraph whose last line ends with a hyphenated word.
    let c2 = controlled_page(
        &f,
        &mut rng,
        vec![Item::Lines(w(
            "extend the review period by two quarters before signing anything. The second sentence of this paragraph closes the thought.",
        ))],
        &s(&["Every supplier in the network must now document that its packaging meets the inter-"]),
    );
    // Page 3: the word continues; ends with a complete sentence exactly at the page end.
    let c3 = controlled_page(
        &f,
        &mut rng,
        vec![Item::Lines(w(
            "national standards required by the customer. This was verified during the audit of the second quarter.",
        ))],
        &s(&["The migration of all customer records to the new platform was completed on schedule."]),
    );
    // Page 4: a new capitalised paragraph.
    let c4 = controlled_page(
        &f,
        &mut rng,
        vec![Item::Lines(w(
            "Training for the warehouse staff will begin in the first week of the new year and continue for six weeks.",
        ))],
        &[],
    );
    for c in [c1, c2, c3, c4] {
        pdf.add_page(PageSpec::letter(), &c);
    }
    finish(&mut pdf)
}

pub fn two_columns() -> Vec<u8> {
    let mut pdf = Pdf::new();
    let f = StdFonts::new(&mut pdf);
    let mut rng = Rng(5150);
    let mut c = Content::new();
    let title = "Quarterly Newsletter";
    let tw = f.helv_b.width(title, 20.0);
    c.text(&f.helv_b, 20.0, (612.0 - tw) / 2.0, 720.0, title);

    let size = 10.5;
    let lead = 13.5;
    let cw = 216.0;
    let paras_l = [
        format!("The left column opens with news about the new office in Lisbon, which will host forty employees from March. {}", paragraph_text(&mut rng, 3)),
        format!("A second item on the left reports that the canteen now serves vegetarian meals every day. {}", paragraph_text(&mut rng, 3)),
    ];
    let paras_r = [
        format!("The right column starts with the annual picnic, scheduled for the first Saturday of June. {}", paragraph_text(&mut rng, 3)),
        format!("Finally the right column thanks all volunteers who helped with the charity run. {}", paragraph_text(&mut rng, 2)),
    ];
    // (baseline y, line, justify) per column.
    let layout = |paras: &[String]| {
        let mut out: Vec<(f64, String, bool)> = Vec::new();
        let mut y = 670.0;
        for p in paras {
            let lines = wrap(&f.times, size, p, cw);
            let n = lines.len();
            for (i, l) in lines.into_iter().enumerate() {
                out.push((y, l, i + 1 < n));
                y -= lead;
            }
            y -= 8.0;
        }
        out
    };
    let left = layout(&paras_l);
    let right = layout(&paras_r);
    // Interleave row by row so that stream order is not column order.
    let emit = |c: &mut Content, x: f64, (y, l, j): &(f64, String, bool)| {
        c.begin_text();
        c.set_font(&f.times, size);
        c.td(x, *y);
        if *j {
            let sp = l.matches(' ').count().max(1) as f64;
            c.word_spacing((cw - f.times.width(l, size)) / sp);
        }
        c.show(&f.times, l);
        c.end_text();
    };
    for i in 0..left.len().max(right.len()) {
        if let Some(r) = right.get(i) {
            emit(&mut c, 324.0, r);
        }
        if let Some(l) = left.get(i) {
            emit(&mut c, 72.0, l);
        }
    }
    pdf.add_page(PageSpec::letter(), &c);
    finish(&mut pdf)
}

pub fn blank_pages() -> Vec<u8> {
    let mut pdf = Pdf::new();
    let f = StdFonts::new(&mut pdf);
    let spec = PageSpec::letter;
    let mut c = Content::new();
    c.text(&f.times, 12.0, 72.0, 700.0, "First page text.");
    pdf.add_page(spec(), &c);
    // Empty content stream.
    pdf.add_page(spec(), &Content::new());
    // Whitespace-only text.
    let mut w = Content::new();
    w.text(&f.times, 12.0, 72.0, 700.0, "   ");
    pdf.add_page(spec(), &w);
    let mut c4 = Content::new();
    c4.text(
        &f.times,
        12.0,
        72.0,
        700.0,
        "Fourth page text after blank pages.",
    );
    pdf.add_page(spec(), &c4);
    // No /Contents at all.
    pdf.add_page_without_contents(spec());
    finish(&mut pdf)
}

pub fn rotated_margin_text() -> Vec<u8> {
    let mut pdf = Pdf::new();
    let f = StdFonts::new(&mut pdf);
    let mut p = Page::at(100.0, TOP, 440.0);
    p.heading(&f.helv_b, 16.0, "Status memo");
    p.body(
        &f.times,
        "The first body paragraph explains the purpose of the document in plain words and stays intact despite the stamp in the margin next to it.",
    );
    p.body(
        &f.times,
        "The second body paragraph describes the current state of the project: all milestones were reached and the remaining budget is sufficient.",
    );
    p.body(
        &f.times,
        "The third body paragraph lists the next steps and thanks everyone who contributed to the memo.",
    );
    // Vertical stamp in the left margin, rotated 90 degrees counter-clockwise.
    p.c.begin_text();
    p.c.set_font(&f.helv, 14.0);
    p.c.gray_fill(0.5);
    p.c.tm([0.0, 1.0, -1.0, 0.0, 50.0, 250.0]);
    p.c.show(&f.helv, "DRAFT - CONFIDENTIAL - DO NOT DISTRIBUTE");
    p.c.end_text();
    p.add(&mut pdf, PageSpec::letter());
    finish(&mut pdf)
}
