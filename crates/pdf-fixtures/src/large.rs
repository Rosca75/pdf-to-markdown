//! Large synthetic reports for benchmarks.
//!
//! [`text_report`] is a text-heavy business report; [`mixed_report`] adds images,
//! vector diagrams and two-column pages. Both are fully deterministic.

use crate::content::Content;
use crate::fonts::TextFont;
use crate::layout::{paragraph_text, put_lines_ex, sentence, word, wrap, Rng, StdFonts};
use crate::writer::{Output, PageSpec, Pdf};

const LEFT: f64 = 81.0;
const FULL_W: f64 = 450.0;
const TOP: f64 = 716.0;
const BOTTOM: f64 = 90.0;
const SIZE: f64 = 11.5;
const LEAD: f64 = 15.0;
const COL_GAP: f64 = 18.0;

/// A text-heavy report of `pages` pages (running header/footer, headings, justified
/// paragraphs that continue across pages, bullet lists, ruled tables).
pub fn text_report(pages: usize) -> Vec<u8> {
    Report::new(pages, false).run()
}

/// Like [`text_report`] plus images, vector diagrams with labels and two-column pages.
pub fn mixed_report(pages: usize) -> Vec<u8> {
    Report::new(pages, true).run()
}

struct Report {
    pdf: Pdf,
    f: StdFonts,
    rng: Rng,
    total: usize,
    done: usize,
    mixed: bool,
    c: Content,
    y: f64,
    col: usize,
    columns: usize,
    images: Vec<(String, f64, f64)>,
    last_table_page: usize,
    last_figure_page: usize,
    figure_no: usize,
}

impl Report {
    fn new(total: usize, mixed: bool) -> Report {
        let mut pdf = Pdf::new();
        let f = StdFonts::new(&mut pdf);
        let mut images = Vec::new();
        if mixed {
            for (i, (w, h)) in [(64u32, 40u32), (48, 48), (80, 50)].into_iter().enumerate() {
                let data: Vec<u8> = (0..w * h * 3)
                    .map(|k| ((k as usize * (i + 3) + (k as usize / 3) % 17) % 256) as u8)
                    .collect();
                let res = pdf.add_image(w, h, true, data);
                images.push((res, w as f64 * 2.0, h as f64 * 2.0));
            }
        }
        let mut r = Report {
            pdf,
            f,
            rng: Rng(0x5EED_1234_ABCD),
            total,
            done: 0,
            mixed,
            c: Content::new(),
            y: TOP,
            col: 0,
            columns: 1,
            images,
            last_table_page: 0,
            last_figure_page: 0,
            figure_no: 0,
        };
        r.start_page();
        r
    }

    fn col_width(&self) -> f64 {
        if self.columns == 1 {
            FULL_W
        } else {
            (FULL_W - COL_GAP) / 2.0
        }
    }

    fn col_x(&self) -> f64 {
        LEFT + self.col as f64 * (self.col_width() + COL_GAP)
    }

    fn start_page(&mut self) {
        self.c = Content::new();
        self.y = TOP;
        self.col = 0;
        // Every fourth page of the mixed report is typeset in two columns.
        self.columns = if self.mixed && (self.done + 1) % 4 == 3 {
            2
        } else {
            1
        };
    }

    fn finish_page(&mut self) {
        if self.done >= self.total {
            return;
        }
        self.done += 1;
        let n = self.done;
        let f = &self.f;
        let c = &mut self.c;
        c.save();
        c.gray_fill(0.3);
        c.text(&f.helv, 9.0, LEFT, 756.0, "Operations Report");
        let right = "Fiscal Year 2026";
        c.text(
            &f.helv,
            9.0,
            LEFT + FULL_W - f.helv.width(right, 9.0),
            756.0,
            right,
        );
        let foot = format!("Page {n} of {}", self.total);
        c.text(
            &f.helv,
            9.0,
            LEFT + (FULL_W - f.helv.width(&foot, 9.0)) / 2.0,
            50.0,
            &foot,
        );
        c.text(&f.helv, 9.0, LEFT, 50.0, "Confidential");
        c.restore();
        c.save();
        c.line_width(0.5);
        c.line(LEFT, 748.0, LEFT + FULL_W, 748.0);
        c.restore();
        let content = std::mem::take(&mut self.c);
        self.pdf.add_page(PageSpec::letter(), &content);
        self.start_page();
    }

    fn next_column(&mut self) {
        if self.col + 1 < self.columns {
            self.col += 1;
            self.y = TOP;
        } else {
            self.finish_page();
        }
    }

    fn need(&mut self, height: f64) {
        if self.y - height < BOTTOM {
            self.next_column();
        }
    }

    fn lines_block(
        &mut self,
        which: u8,
        size: f64,
        lead: f64,
        indent: f64,
        lines: &[String],
        justify: bool,
    ) {
        let mut i = 0;
        while i < lines.len() && self.done < self.total {
            let avail = ((self.y - BOTTOM) / lead).floor() as i64 + 1;
            if avail <= 0 {
                self.next_column();
                continue;
            }
            let take = (avail as usize).min(lines.len() - i);
            let last = i + take == lines.len();
            let x = self.col_x() + indent;
            let width = self.col_width() - indent;
            let font = match which {
                0 => &self.f.times,
                _ => &self.f.helv_b,
            };
            let ret = put_lines_ex(
                &mut self.c,
                font,
                size,
                x,
                self.y,
                lead,
                &lines[i..i + take],
                justify.then_some(width),
                !last,
            );
            self.y = ret;
            i += take;
            if !last {
                self.next_column();
            }
        }
    }

    fn heading(&mut self, size: f64, text: &str) {
        self.need(size * 1.5 + 4.0 * LEAD);
        self.y -= size * 0.6;
        let w = self.col_width();
        let lines = wrap(&self.f.helv_b, size, text, w);
        self.lines_block(1, size, size * 1.25, 0.0, &lines, false);
        self.y -= 4.0;
    }

    fn para(&mut self) {
        let n = 4 + self.rng.below(4);
        let text = paragraph_text(&mut self.rng, n);
        let lines = wrap(&self.f.times, SIZE, &text, self.col_width());
        self.lines_block(0, SIZE, LEAD, 0.0, &lines, true);
        self.y -= 7.0;
    }

    fn bullets(&mut self) {
        let items = 3 + self.rng.below(3);
        for _ in 0..items {
            let text = sentence(&mut self.rng);
            let w = self.col_width() - 16.0;
            let lines = wrap(&self.f.times, SIZE, &text, w);
            self.need(LEAD * lines.len() as f64);
            let x = self.col_x();
            self.c
                .text(&self.f.times, SIZE, x + 4.0, self.y, "\u{2022}");
            self.lines_block(0, SIZE, LEAD, 16.0, &lines, false);
            self.y -= 2.0;
        }
        self.y -= 6.0;
    }

    fn table(&mut self) {
        let rows = 6;
        let row_h = 18.0;
        self.need(rows as f64 * row_h + 24.0);
        if self.done >= self.total {
            return;
        }
        let x0 = self.col_x();
        let w = self.col_width();
        let top = self.y;
        let cols = [0.0, 0.34, 0.56, 0.78, 1.0];
        let f = &self.f;
        let c = &mut self.c;
        c.save();
        c.gray_fill(0.9);
        c.fill_rect(x0, top - row_h, w, row_h);
        c.restore();
        c.save();
        c.line_width(0.5);
        for i in 0..=rows {
            let y = top - i as f64 * row_h;
            c.line(x0, y, x0 + w, y);
        }
        for cx in cols {
            c.line(x0 + cx * w, top, x0 + cx * w, top - rows as f64 * row_h);
        }
        c.restore();
        let header = ["Segment", "Revenue", "Margin", "Change"];
        for (ci, h) in header.iter().enumerate() {
            c.text(&f.helv_b, 9.5, x0 + cols[ci] * w + 5.0, top - 13.0, h);
        }
        for ri in 1..rows {
            let y = top - ri as f64 * row_h - 13.0;
            let cells = [
                word(&mut self.rng).to_string(),
                format!("{}.{}", 10 + self.rng.below(900), self.rng.below(10)),
                format!("{}.{}%", self.rng.below(40), self.rng.below(10)),
                format!(
                    "{}{}.{}%",
                    if self.rng.below(2) == 0 { "+" } else { "-" },
                    self.rng.below(15),
                    self.rng.below(10)
                ),
            ];
            for (ci, cell) in cells.iter().enumerate() {
                c.text(&f.helv, 9.5, x0 + cols[ci] * w + 5.0, y, cell);
            }
        }
        self.y = top - rows as f64 * row_h - 14.0;
        self.last_table_page = self.done;
    }

    fn figure_image(&mut self) {
        let (res, w, h) = self.images[self.figure_no % self.images.len()].clone();
        self.need(h + 40.0);
        if self.done >= self.total {
            return;
        }
        let x = self.col_x() + (self.col_width() - w) / 2.0;
        self.c.image(&res, x, self.y - h, w, h);
        self.y -= h + 12.0;
        self.figure_no += 1;
        let cap = format!("Figure {}: {}", self.figure_no, sentence(&mut self.rng));
        let lines = wrap(&self.f.helv_i, 9.0, &cap, self.col_width());
        let x = self.col_x();
        let ret = put_lines_ex(
            &mut self.c,
            &self.f.helv_i,
            9.0,
            x,
            self.y,
            11.0,
            &lines,
            None,
            false,
        );
        self.y = ret - 10.0;
        self.last_figure_page = self.done;
    }

    fn figure_diagram(&mut self) {
        self.need(110.0);
        if self.done >= self.total {
            return;
        }
        let x0 = self.col_x();
        let top = self.y - 10.0;
        let labels = ["Plan", "Build", "Test", "Release"];
        let bw = 70.0;
        let gap = (self.col_width() - 4.0 * bw) / 3.0;
        let f = &self.f;
        let c = &mut self.c;
        for (i, l) in labels.iter().enumerate() {
            let x = x0 + i as f64 * (bw + gap);
            c.save();
            c.line_width(1.0);
            c.rgb_fill(0.92, 0.95, 1.0);
            c.rect(x, top - 34.0, bw, 34.0);
            c.fill_stroke();
            c.restore();
            let tw = f.helv.width(l, 10.0);
            c.text(&f.helv, 10.0, x + (bw - tw) / 2.0, top - 21.0, l);
            if i + 1 < labels.len() {
                let ay = top - 17.0;
                c.line(x + bw, ay, x + bw + gap - 2.0, ay);
                c.move_to(x + bw + gap, ay);
                c.line_to(x + bw + gap - 6.0, ay + 3.0);
                c.line_to(x + bw + gap - 6.0, ay - 3.0);
                c.close();
                c.fill();
            }
        }
        self.figure_no += 1;
        let cap = format!("Figure {}: Delivery process overview", self.figure_no);
        c.text(&f.helv_i, 9.0, x0, top - 54.0, &cap);
        self.y = top - 74.0;
        self.last_figure_page = self.done;
    }

    fn run(mut self) -> Vec<u8> {
        let mut section = 0usize;
        while self.done < self.total {
            section += 1;
            if section % 4 == 1 {
                let t = format!(
                    "Chapter {}: {} {}",
                    section / 4 + 1,
                    cap(word(&mut self.rng)),
                    word(&mut self.rng)
                );
                self.heading(20.0, &t);
            }
            let t = format!(
                "{} {} {}",
                section,
                cap(word(&mut self.rng)),
                word(&mut self.rng)
            );
            self.heading(14.0, &t);
            let paras = 3 + self.rng.below(4);
            for k in 0..paras {
                if self.done >= self.total {
                    break;
                }
                self.para();
                if k % 3 == 2 {
                    self.bullets();
                }
                if self.columns == 1 && self.done >= self.last_table_page + 5 {
                    self.table();
                }
                if self.mixed && self.columns == 1 && self.done >= self.last_figure_page + 3 {
                    if self.figure_no.is_multiple_of(2) {
                        self.figure_image();
                    } else {
                        self.figure_diagram();
                    }
                }
            }
        }
        let out = if self.mixed {
            Output::compact()
        } else {
            Output::default()
        };
        self.pdf.build(&out).bytes
    }
}

fn cap(w: &str) -> String {
    let mut cs = w.chars();
    match cs.next() {
        Some(f) => f.to_uppercase().collect::<String>() + cs.as_str(),
        None => String::new(),
    }
}
