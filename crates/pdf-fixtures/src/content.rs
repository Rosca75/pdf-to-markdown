//! Content-stream builder: text, paths, images and colour operators.

use crate::fonts::TextFont;
use crate::obj::fmt_num;
use std::io::Write;

/// An in-progress content stream.
#[derive(Clone, Debug, Default)]
pub struct Content {
    pub buf: Vec<u8>,
}

impl Content {
    pub fn new() -> Self {
        Content::default()
    }

    /// Append raw operator text followed by a newline.
    pub fn raw(&mut self, s: &str) -> &mut Self {
        self.buf.extend_from_slice(s.as_bytes());
        self.buf.push(b'\n');
        self
    }

    fn nums(&mut self, v: &[f64], op: &str) -> &mut Self {
        for (i, x) in v.iter().enumerate() {
            if i > 0 {
                self.buf.push(b' ');
            }
            if x.fract() == 0.0 && x.abs() < 1e9 {
                let _ = write!(self.buf, "{}", *x as i64);
            } else {
                self.buf.extend_from_slice(fmt_num(*x).as_bytes());
            }
        }
        if !v.is_empty() {
            self.buf.push(b' ');
        }
        self.buf.extend_from_slice(op.as_bytes());
        self.buf.push(b'\n');
        self
    }

    // ---- graphics state -------------------------------------------------

    pub fn save(&mut self) -> &mut Self {
        self.raw("q")
    }
    pub fn restore(&mut self) -> &mut Self {
        self.raw("Q")
    }
    pub fn cm(&mut self, m: [f64; 6]) -> &mut Self {
        self.nums(&m, "cm")
    }
    pub fn line_width(&mut self, w: f64) -> &mut Self {
        self.nums(&[w], "w")
    }
    pub fn gray_fill(&mut self, g: f64) -> &mut Self {
        self.nums(&[g], "g")
    }
    pub fn gray_stroke(&mut self, g: f64) -> &mut Self {
        self.nums(&[g], "G")
    }
    pub fn rgb_fill(&mut self, r: f64, g: f64, b: f64) -> &mut Self {
        self.nums(&[r, g, b], "rg")
    }
    pub fn rgb_stroke(&mut self, r: f64, g: f64, b: f64) -> &mut Self {
        self.nums(&[r, g, b], "RG")
    }

    // ---- paths ------------------------------------------------------------

    pub fn move_to(&mut self, x: f64, y: f64) -> &mut Self {
        self.nums(&[x, y], "m")
    }
    pub fn line_to(&mut self, x: f64, y: f64) -> &mut Self {
        self.nums(&[x, y], "l")
    }
    pub fn curve_to(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, x3: f64, y3: f64) -> &mut Self {
        self.nums(&[x1, y1, x2, y2, x3, y3], "c")
    }
    pub fn rect(&mut self, x: f64, y: f64, w: f64, h: f64) -> &mut Self {
        self.nums(&[x, y, w, h], "re")
    }
    pub fn close(&mut self) -> &mut Self {
        self.raw("h")
    }
    pub fn stroke(&mut self) -> &mut Self {
        self.raw("S")
    }
    pub fn fill(&mut self) -> &mut Self {
        self.raw("f")
    }
    pub fn fill_stroke(&mut self) -> &mut Self {
        self.raw("B")
    }

    /// Stroke a straight line.
    pub fn line(&mut self, x1: f64, y1: f64, x2: f64, y2: f64) -> &mut Self {
        self.move_to(x1, y1).line_to(x2, y2).stroke()
    }

    /// Stroked rectangle.
    pub fn stroke_rect(&mut self, x: f64, y: f64, w: f64, h: f64) -> &mut Self {
        self.rect(x, y, w, h).stroke()
    }

    /// Filled rectangle in the current fill colour.
    pub fn fill_rect(&mut self, x: f64, y: f64, w: f64, h: f64) -> &mut Self {
        self.rect(x, y, w, h).fill()
    }

    /// Paint an XObject (image or form) into the box `x y w h`.
    pub fn image(&mut self, res: &str, x: f64, y: f64, w: f64, h: f64) -> &mut Self {
        self.save();
        self.cm([w, 0.0, 0.0, h, x, y]);
        self.raw(&format!("/{res} Do"));
        self.restore()
    }

    /// Paint an XObject with the current transformation.
    pub fn do_xobject(&mut self, res: &str) -> &mut Self {
        self.raw(&format!("/{res} Do"))
    }

    // ---- text ---------------------------------------------------------------

    pub fn begin_text(&mut self) -> &mut Self {
        self.raw("BT")
    }
    pub fn end_text(&mut self) -> &mut Self {
        self.raw("ET")
    }
    pub fn set_font(&mut self, f: &dyn TextFont, size: f64) -> &mut Self {
        self.raw(&format!("/{} {} Tf", f.res(), fmt_num(size)))
    }
    pub fn td(&mut self, x: f64, y: f64) -> &mut Self {
        self.nums(&[x, y], "Td")
    }
    pub fn tm(&mut self, m: [f64; 6]) -> &mut Self {
        self.nums(&m, "Tm")
    }
    pub fn leading(&mut self, l: f64) -> &mut Self {
        self.nums(&[l], "TL")
    }
    pub fn word_spacing(&mut self, w: f64) -> &mut Self {
        self.nums(&[w], "Tw")
    }
    pub fn char_spacing(&mut self, c: f64) -> &mut Self {
        self.nums(&[c], "Tc")
    }
    /// Text rendering mode (`Tr`).
    pub fn render_mode(&mut self, mode: u8) -> &mut Self {
        self.raw(&format!("{mode} Tr"))
    }
    pub fn next_line(&mut self) -> &mut Self {
        self.raw("T*")
    }

    /// `(...) Tj` in the current font `f`.
    pub fn show(&mut self, f: &dyn TextFont, s: &str) -> &mut Self {
        self.buf.extend_from_slice(&f.operand(s));
        self.buf.extend_from_slice(b" Tj\n");
        self
    }

    /// `[...] TJ` with alternating strings and adjustments (thousandths of an em).
    pub fn show_adjusted(&mut self, f: &dyn TextFont, parts: &[Part]) -> &mut Self {
        self.buf.push(b'[');
        for p in parts {
            match p {
                Part::Text(s) => self.buf.extend_from_slice(&f.operand(s)),
                Part::Adjust(a) => self.buf.extend_from_slice(fmt_num(*a).as_bytes()),
            }
            self.buf.push(b' ');
        }
        self.buf.extend_from_slice(b"] TJ\n");
        self
    }

    /// One self-contained text line: `BT /F size Tf x y Td (s) Tj ET`.
    pub fn text(&mut self, f: &dyn TextFont, size: f64, x: f64, y: f64, s: &str) -> &mut Self {
        self.begin_text();
        self.set_font(f, size);
        self.td(x, y);
        self.show(f, s);
        self.end_text()
    }

    /// Several lines with `TL`/`T*`, one `BT` block.
    pub fn lines(
        &mut self,
        f: &dyn TextFont,
        size: f64,
        x: f64,
        y: f64,
        leading: f64,
        lines: &[&str],
    ) -> &mut Self {
        self.begin_text();
        self.set_font(f, size);
        self.leading(leading);
        self.td(x, y);
        for (i, l) in lines.iter().enumerate() {
            if i > 0 {
                self.next_line();
            }
            self.show(f, l);
        }
        self.end_text()
    }
}

/// An element of a `TJ` array.
#[derive(Clone, Debug)]
pub enum Part {
    Text(String),
    Adjust(f64),
}

/// Shorthand for [`Part::Text`].
pub fn t(s: &str) -> Part {
    Part::Text(s.to_string())
}

/// Shorthand for [`Part::Adjust`].
pub fn adj(a: f64) -> Part {
    Part::Adjust(a)
}
