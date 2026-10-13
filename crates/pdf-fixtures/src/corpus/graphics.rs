//! Graphics-heavy corpus cases: shapes, images, forms, scans and OCR layers.

use super::common::*;
use crate::content::Content;
use crate::fonts::TextFont;
use crate::layout::StdFonts;
use crate::writer::{PageSpec, Pdf};

/// Deterministic RGB test pattern.
pub fn rgb_pattern(w: u32, h: u32) -> Vec<u8> {
    let mut v = Vec::with_capacity((w * h * 3) as usize);
    for y in 0..h {
        for x in 0..w {
            v.push(((x * 255) / w.max(1)) as u8);
            v.push(((y * 255) / h.max(1)) as u8);
            v.push(((x * 7 + y * 13) % 256) as u8);
        }
    }
    v
}

/// Deterministic grey test pattern.
pub fn gray_pattern(w: u32, h: u32) -> Vec<u8> {
    let mut v = Vec::with_capacity((w * h) as usize);
    for y in 0..h {
        for x in 0..w {
            v.push(((x * 7 + y * 13 + (x * y) % 31) % 256) as u8);
        }
    }
    v
}

pub fn mixed_graphics() -> Vec<u8> {
    let mut pdf = Pdf::new();
    let f = StdFonts::new(&mut pdf);
    let img = pdf.add_image(16, 16, true, rgb_pattern(16, 16));
    let mut form_c = Content::new();
    form_c.text(&f.helv, 10.0, 2.0, 5.0, "Figure 1 form caption");
    let form = pdf.add_form([0.0, 0.0, 200.0, 20.0], &form_c);

    let mut p = Page::new();
    p.heading(&f.helv_b, 18.0, "Quarterly diagram");
    p.body(
        &f.times,
        "The first paragraph before the graphics describes the situation in a few clear sentences.",
    );
    // Grey bar.
    p.c.save();
    p.c.gray_fill(0.8);
    p.c.fill_rect(72.0, p.y, 468.0, 10.0);
    p.c.restore();
    p.skip(24.0);
    // Bezier blob.
    p.c.save();
    p.c.rgb_fill(0.2, 0.4, 0.8);
    let by = p.y - 40.0;
    p.c.move_to(72.0, by);
    p.c.curve_to(72.0, by + 40.0, 160.0, by + 40.0, 160.0, by);
    p.c.curve_to(160.0, by - 30.0, 72.0, by - 30.0, 72.0, by);
    p.c.close();
    p.c.fill();
    p.c.restore();
    // Image next to it.
    p.c.image(&img, 200.0, p.y - 70.0, 120.0, 80.0);
    p.skip(100.0);
    p.body(
        &f.times,
        "The second paragraph sits between the shapes and the image and keeps its sentence together.",
    );
    // Form XObject.
    p.c.save();
    p.c.cm([1.0, 0.0, 0.0, 1.0, 72.0, p.y]);
    p.c.do_xobject(&form);
    p.c.restore();
    p.skip(40.0);
    // Small labelled diagram.
    let labels = ["Input", "Process", "Output"];
    for (i, l) in labels.iter().enumerate() {
        let x = 72.0 + i as f64 * 140.0;
        p.c.save();
        p.c.line_width(1.0);
        p.c.stroke_rect(x, p.y - 30.0, 80.0, 30.0);
        p.c.restore();
        let tw = f.helv.width(l, 10.0);
        p.c.text(&f.helv, 10.0, x + (80.0 - tw) / 2.0, p.y - 19.0, l);
        if i + 1 < labels.len() {
            let ay = p.y - 15.0;
            p.c.line(x + 80.0, ay, x + 138.0, ay);
            p.c.move_to(x + 140.0, ay);
            p.c.line_to(x + 132.0, ay + 3.0);
            p.c.line_to(x + 132.0, ay - 3.0);
            p.c.close();
            p.c.fill();
        }
    }
    p.skip(60.0);
    p.body(
        &f.times,
        "The closing paragraph after the diagram ends the page.",
    );
    p.add(&mut pdf, PageSpec::letter());
    finish(&mut pdf)
}

pub fn scanned_image_only() -> Vec<u8> {
    let mut pdf = Pdf::new();
    let img = pdf.add_image(80, 100, false, gray_pattern(80, 100));
    let mut c = Content::new();
    c.image(&img, 0.0, 0.0, 612.0, 792.0);
    pdf.add_page(PageSpec::letter(), &c);
    finish(&mut pdf)
}

pub fn ocr_invisible_text() -> Vec<u8> {
    let mut pdf = Pdf::new();
    let f = StdFonts::new(&mut pdf);
    let img = pdf.add_image(80, 100, false, gray_pattern(80, 100));
    let mut c = Content::new();
    c.image(&img, 0.0, 0.0, 612.0, 792.0);
    c.begin_text();
    c.render_mode(3);
    c.set_font(&f.helv, 12.0);
    c.td(72.0, 700.0);
    c.show(&f.helv, "Invoice number 20481 issued to Contoso Ltd.");
    c.td(0.0, -16.0);
    c.show(&f.helv, "Payment is due within thirty days of receipt.");
    c.end_text();
    pdf.add_page(PageSpec::letter(), &c);
    finish(&mut pdf)
}
