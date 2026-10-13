//! Table corpus cases.

use super::common::*;
use crate::layout::StdFonts;
use crate::writer::{PageSpec, Pdf};

pub fn table_ruled() -> Vec<u8> {
    let mut pdf = Pdf::new();
    let f = StdFonts::new(&mut pdf);
    let mut p = Page::new();
    p.heading(&f.helv_b, 16.0, "Stock list");
    p.body(
        &f.times,
        "The following table lists the current stock levels.",
    );

    let cols = [72.0, 200.0, 280.0, 440.0];
    let row_h = 22.0;
    let top = p.y + 6.0;
    let rows: [[&str; 3]; 4] = [
        ["Item", "Qty", "Notes"],
        ["Widget", "12", "Standard part"],
        ["Gadget | Pro", "3", "Ships separately"],
        ["Gizmo", "7", "Out of stock"],
    ];
    let bottom = top - row_h * rows.len() as f64;
    // Header background.
    p.c.save();
    p.c.gray_fill(0.9);
    p.c.fill_rect(cols[0], top - row_h, cols[3] - cols[0], row_h);
    p.c.restore();
    // Grid.
    p.c.save();
    p.c.line_width(0.75);
    for i in 0..=rows.len() {
        let y = top - row_h * i as f64;
        p.c.line(cols[0], y, cols[3], y);
    }
    for x in cols {
        p.c.line(x, top, x, bottom);
    }
    p.c.restore();
    for (ri, row) in rows.iter().enumerate() {
        let font = if ri == 0 { &f.helv_b } else { &f.helv };
        for (ci, cell) in row.iter().enumerate() {
            let y = top - row_h * ri as f64 - 15.0;
            p.c.text(font, 10.0, cols[ci] + 6.0, y, cell);
        }
    }
    p.y = bottom - 28.0;
    p.body(
        &f.times,
        "Prices exclude VAT and are valid until the end of the month.",
    );
    p.add(&mut pdf, PageSpec::letter());
    finish(&mut pdf)
}

pub fn table_unruled() -> Vec<u8> {
    let mut pdf = Pdf::new();
    let f = StdFonts::new(&mut pdf);
    let mut p = Page::new();
    p.heading(&f.helv_b, 16.0, "Sales");
    p.body(&f.times, "Quarterly sales by region are shown below.");
    p.skip(4.0);
    let cols = [72.0, 200.0, 300.0, 400.0];
    let rows: [[&str; 4]; 5] = [
        ["Region", "Q1", "Q2", "Q3"],
        ["North", "120", "135", "150"],
        ["South", "98", "101", "110"],
        ["East", "143", "150", "162"],
        ["West", "87", "90", "95"],
    ];
    for row in rows {
        for (ci, cell) in row.iter().enumerate() {
            p.c.text(&f.times, 11.0, cols[ci], p.y, cell);
        }
        p.y -= 16.0;
    }
    p.skip(10.0);
    p.body(&f.times, "All figures are in thousands of euros.");
    p.add(&mut pdf, PageSpec::letter());
    finish(&mut pdf)
}
