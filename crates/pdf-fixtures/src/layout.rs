//! Text layout helpers: wrapping, justified paragraphs, a set of common fonts and
//! a deterministic pseudo-random text generator.

use crate::content::Content;
use crate::fonts::{Font, Std14, TextFont};
use crate::obj::fmt_num;
use crate::writer::Pdf;

/// The fonts most fixtures need, registered once per document.
#[derive(Clone, Debug)]
pub struct StdFonts {
    pub helv: Font,
    pub helv_b: Font,
    pub helv_i: Font,
    pub times: Font,
    pub times_b: Font,
    pub times_i: Font,
    pub times_bi: Font,
    pub cour: Font,
}

impl StdFonts {
    pub fn new(pdf: &mut Pdf) -> StdFonts {
        StdFonts {
            helv: pdf.add_std_font(Std14::Helvetica, &[]),
            helv_b: pdf.add_std_font(Std14::HelveticaBold, &[]),
            helv_i: pdf.add_std_font(Std14::HelveticaOblique, &[]),
            times: pdf.add_std_font(Std14::TimesRoman, &[]),
            times_b: pdf.add_std_font(Std14::TimesBold, &[]),
            times_i: pdf.add_std_font(Std14::TimesItalic, &[]),
            times_bi: pdf.add_std_font(Std14::TimesBoldItalic, &[]),
            cour: pdf.add_std_font(Std14::Courier, &[]),
        }
    }
}

/// Greedy word wrap to `width` points.
pub fn wrap(f: &dyn TextFont, size: f64, text: &str, width: f64) -> Vec<String> {
    let space = f.width(" ", size);
    let mut lines = Vec::new();
    let mut cur = String::new();
    let mut cur_w = 0.0;
    for word in text.split_whitespace() {
        let w = f.width(word, size);
        if cur.is_empty() {
            cur.push_str(word);
            cur_w = w;
        } else if cur_w + space + w <= width {
            cur.push(' ');
            cur.push_str(word);
            cur_w += space + w;
        } else {
            lines.push(std::mem::take(&mut cur));
            cur.push_str(word);
            cur_w = w;
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

/// Write `lines` starting with the first baseline at `y`, one `BT` block using
/// `TL`/`T*`. With `justify_width`, every line except the last is justified using `Tw`.
/// Returns the baseline y of the line after the last one.
#[allow(clippy::too_many_arguments)]
pub fn put_lines(
    c: &mut Content,
    f: &dyn TextFont,
    size: f64,
    x: f64,
    y: f64,
    leading: f64,
    lines: &[String],
    justify_width: Option<f64>,
) -> f64 {
    put_lines_ex(c, f, size, x, y, leading, lines, justify_width, false)
}

/// Like [`put_lines`]; with `justify_last` the final line is justified too (used when a
/// paragraph continues in the next column or page).
#[allow(clippy::too_many_arguments)]
pub fn put_lines_ex(
    c: &mut Content,
    f: &dyn TextFont,
    size: f64,
    x: f64,
    y: f64,
    leading: f64,
    lines: &[String],
    justify_width: Option<f64>,
    justify_last: bool,
) -> f64 {
    if lines.is_empty() {
        return y;
    }
    c.begin_text();
    c.set_font(f, size);
    c.leading(leading);
    c.td(x, y);
    let mut tw_set = false;
    for (i, l) in lines.iter().enumerate() {
        if i > 0 {
            c.next_line();
        }
        if let Some(w) = justify_width {
            let spaces = l.matches(' ').count();
            if (i + 1 < lines.len() || justify_last) && spaces > 0 {
                let natural = f.width(l, size);
                c.word_spacing((w - natural) / spaces as f64);
                tw_set = true;
            } else if tw_set {
                c.word_spacing(0.0);
                tw_set = false;
            }
        }
        c.show(f, l);
    }
    if tw_set {
        c.word_spacing(0.0);
    }
    c.end_text();
    y - leading * lines.len() as f64
}

/// Wrap and draw a paragraph; returns the baseline y after it.
#[allow(clippy::too_many_arguments)]
pub fn paragraph(
    c: &mut Content,
    f: &dyn TextFont,
    size: f64,
    x: f64,
    y: f64,
    width: f64,
    leading: f64,
    text: &str,
    justify: bool,
) -> f64 {
    let lines = wrap(f, size, text, width);
    put_lines(c, f, size, x, y, leading, &lines, justify.then_some(width))
}

/// Deterministic xorshift64* generator.
pub struct Rng(pub u64);

impl Rng {
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    /// Uniform-ish value in `0..n`.
    pub fn below(&mut self, n: usize) -> usize {
        (self.next_u64() >> 33) as usize % n
    }
}

const WORDS: &[&str] = &[
    "market",
    "revenue",
    "growth",
    "customer",
    "strategy",
    "quarter",
    "operations",
    "supply",
    "chain",
    "performance",
    "forecast",
    "investment",
    "portfolio",
    "regional",
    "segment",
    "analysis",
    "margin",
    "budget",
    "partner",
    "delivery",
    "capacity",
    "demand",
    "pricing",
    "product",
    "service",
    "team",
    "project",
    "milestone",
    "risk",
    "compliance",
    "review",
    "committee",
    "process",
    "quality",
    "supplier",
    "contract",
    "expansion",
    "efficiency",
    "innovation",
    "platform",
    "customers",
    "channel",
    "initiative",
    "outlook",
    "results",
    "volume",
    "inventory",
    "logistics",
    "network",
    "transition",
    "agreement",
    "evaluation",
    "framework",
    "objective",
    "resources",
    "workforce",
    "training",
    "technology",
    "reporting",
    "stakeholders",
    "governance",
    "sustainability",
    "acquisition",
    "benchmark",
    "cost",
    "savings",
    "target",
    "baseline",
    "trend",
    "adoption",
    "retention",
    "pipeline",
    "launch",
    "roadmap",
    "dependency",
    "assessment",
    "approval",
    "policy",
    "standard",
    "audit",
    "control",
    "planning",
    "execution",
    "delivery",
    "support",
    "maintenance",
    "upgrade",
    "migration",
    "integration",
    "reliability",
    "availability",
    "security",
    "governance",
    "insight",
];

const FUNCTION_WORDS: &[&str] = &[
    "the",
    "of",
    "and",
    "to",
    "in",
    "for",
    "with",
    "on",
    "by",
    "across",
    "during",
    "after",
    "within",
    "under",
    "through",
    "is",
    "was",
    "has",
    "will",
    "should",
    "remains",
    "improved",
    "declined",
    "continued",
    "requires",
    "supports",
    "reflects",
    "drives",
    "a",
    "an",
    "this",
    "that",
    "these",
    "its",
    "our",
    "their",
    "which",
    "while",
    "as",
    "from",
    "between",
];

/// A pseudo-random business word.
pub fn word(rng: &mut Rng) -> &'static str {
    WORDS[rng.below(WORDS.len())]
}

/// Generate one sentence of 8 to 20 words.
pub fn sentence(rng: &mut Rng) -> String {
    let n = 8 + rng.below(13);
    let mut s = String::new();
    for i in 0..n {
        let w = if i % 2 == 1 || rng.below(4) == 0 {
            FUNCTION_WORDS[rng.below(FUNCTION_WORDS.len())]
        } else {
            WORDS[rng.below(WORDS.len())]
        };
        if i == 0 {
            let mut cs = w.chars();
            if let Some(f) = cs.next() {
                s.extend(f.to_uppercase());
                s.push_str(cs.as_str());
            }
        } else {
            s.push(' ');
            s.push_str(w);
        }
    }
    s.push('.');
    s
}

/// Generate a paragraph of `sentences` sentences.
pub fn paragraph_text(rng: &mut Rng, sentences: usize) -> String {
    (0..sentences)
        .map(|_| sentence(rng))
        .collect::<Vec<_>>()
        .join(" ")
}

/// A paragraph of roughly `lines` wrapped lines, ending with a full stop.
pub fn text_for_lines(
    rng: &mut Rng,
    f: &dyn TextFont,
    size: f64,
    width: f64,
    lines: usize,
) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    while out.len() < lines {
        out = wrap(f, size, &paragraph_text(rng, lines.max(2)), width);
    }
    out.truncate(lines);
    if let Some(last) = out.last_mut() {
        if !last.ends_with('.') {
            last.push('.');
        }
    }
    out
}

/// Format helper for tests and fixtures: a number with at most 2 decimals.
pub fn n2(v: f64) -> String {
    fmt_num((v * 100.0).round() / 100.0)
}
