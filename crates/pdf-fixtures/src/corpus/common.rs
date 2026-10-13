//! Shared helpers for corpus cases: a simple top-down page cursor.

use crate::content::Content;
use crate::fonts::{Font, TextFont};
use crate::layout::{paragraph, put_lines};
use crate::writer::{Output, PageIds, PageSpec, Pdf};

pub const LEFT: f64 = 72.0;
pub const TOP: f64 = 720.0;
pub const WIDTH: f64 = 468.0;
pub const BODY: f64 = 11.0;

/// Serialise with default (classic xref) output.
pub fn finish(pdf: &mut Pdf) -> Vec<u8> {
    pdf.build(&Output::default()).bytes
}

/// A top-down text cursor over one page's content stream.
pub struct Page {
    pub c: Content,
    pub x: f64,
    pub y: f64,
    pub w: f64,
}

impl Default for Page {
    fn default() -> Self {
        Self::new()
    }
}

impl Page {
    pub fn new() -> Page {
        Page {
            c: Content::new(),
            x: LEFT,
            y: TOP,
            w: WIDTH,
        }
    }

    /// A page with custom origin and width.
    pub fn at(x: f64, y: f64, w: f64) -> Page {
        Page {
            c: Content::new(),
            x,
            y,
            w,
        }
    }

    /// Move down by `dy` points.
    pub fn skip(&mut self, dy: f64) -> &mut Self {
        self.y -= dy;
        self
    }

    /// A heading line (space before and after proportional to its size).
    pub fn heading(&mut self, f: &Font, size: f64, text: &str) -> &mut Self {
        self.y -= size * 0.5;
        self.c.text(f, size, self.x, self.y, text);
        self.y -= size * 1.3;
        self
    }

    /// A wrapped paragraph with a gap after it.
    pub fn para(&mut self, f: &dyn TextFont, size: f64, text: &str, justify: bool) -> &mut Self {
        let lead = size * 1.27;
        self.y = paragraph(
            &mut self.c,
            f,
            size,
            self.x,
            self.y,
            self.w,
            lead,
            text,
            justify,
        ) - 6.0;
        self
    }

    /// Body paragraph in `f` at the default body size.
    pub fn body(&mut self, f: &dyn TextFont, text: &str) -> &mut Self {
        self.para(f, BODY, text, false)
    }

    /// Explicit lines (no wrapping) as one paragraph.
    pub fn lines(&mut self, f: &dyn TextFont, size: f64, lines: &[&str]) -> &mut Self {
        let v: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
        self.y = put_lines(&mut self.c, f, size, self.x, self.y, size * 1.27, &v, None) - 6.0;
        self
    }

    /// Add this page to `pdf`.
    pub fn add(self, pdf: &mut Pdf, spec: PageSpec) -> PageIds {
        pdf.add_page(spec, &self.c)
    }
}

/// Draw consecutive text runs on one baseline, each in its own `BT` block with its
/// own font resource. Returns the x where the line ends.
pub fn runs(c: &mut Content, size: f64, x: f64, y: f64, parts: &[(&dyn TextFont, &str)]) -> f64 {
    let mut cx = x;
    for (f, s) in parts {
        c.text(*f, size, cx, y, s);
        cx += f.width(s, size);
    }
    cx
}
