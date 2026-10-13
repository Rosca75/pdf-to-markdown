//! pdf_oxide adapter: the only module that touches pdf_oxide types.
//!
//! It opens documents (mapping failures to [`ConvertError`]) and turns each
//! page into a [`PageContent`] in the engine's top-down coordinate frame:
//! origin at the top-left of the page's visible box, `y` growing downwards.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;

use pdf_oxide::extractors::text::ArtifactType;
use pdf_oxide::PdfDocument;

use crate::model::{BBox, Link, PageContent, Rule, Span, Style};
use crate::normalize::normalize;
use crate::ConvertError;

/// How far into the file a `%PDF-` header may start (ISO 32000 tolerates
/// leading garbage; readers conventionally scan the first KiB).
const HEADER_WINDOW: usize = 1024;

/// Thickness under which a path is treated as a ruling line.
const RULE_THICKNESS: f32 = 3.0;

/// Open `bytes` as a PDF and unlock it.
pub fn open(bytes: Vec<u8>, password: Option<&str>) -> Result<PdfDocument, ConvertError> {
    let window = &bytes[..bytes.len().min(HEADER_WINDOW)];
    if !window.windows(5).any(|w| w == b"%PDF-") {
        return Err(ConvertError::NotAPdf);
    }
    let doc = guarded(|| PdfDocument::from_bytes(bytes))
        .map_err(ConvertError::Corrupt)?
        .map_err(map_open_error)?;
    if !doc.is_authenticated() {
        match password {
            None | Some("") => return Err(ConvertError::PasswordRequired),
            Some(pw) => match guarded(|| doc.authenticate(pw.as_bytes())) {
                Ok(Ok(true)) => {}
                Ok(Ok(false)) => return Err(ConvertError::WrongPassword),
                Ok(Err(e)) => return Err(map_open_error(e)),
                Err(panic) => return Err(ConvertError::Corrupt(panic)),
            },
        }
    }
    Ok(doc)
}

fn map_open_error(e: pdf_oxide::Error) -> ConvertError {
    match e {
        pdf_oxide::Error::EncryptedPdf => ConvertError::PasswordRequired,
        other => ConvertError::Corrupt(other.to_string()),
    }
}

/// Number of pages, or `Corrupt` when the page tree is unreadable.
pub fn page_count(doc: &PdfDocument) -> Result<usize, ConvertError> {
    guarded(|| doc.page_count())
        .map_err(ConvertError::Corrupt)?
        .map_err(|e| ConvertError::Corrupt(e.to_string()))
}

/// Facts about one extracted page beyond its content.
pub struct PageExtract {
    pub content: PageContent,
    /// Undecodable glyphs met on this page (U+FFFD from the font decoder).
    pub undecodable: usize,
}

/// Extract one page. Errors (including panics inside pdf_oxide) are returned
/// as a message so the caller can degrade to a per-page warning.
pub fn page(doc: &PdfDocument, index: usize) -> Result<PageExtract, String> {
    guarded(|| page_inner(doc, index))?
}

fn page_inner(doc: &PdfDocument, index: usize) -> Result<PageExtract, String> {
    let (bx0, by0, bx1, by1) = doc
        .get_page_visible_box(index)
        .or_else(|_| doc.get_page_media_box(index))
        .unwrap_or((0.0, 0.0, 612.0, 792.0));
    let (bx0, bx1) = (bx0.min(bx1), bx0.max(bx1));
    let (by0, by1) = (by0.min(by1), by0.max(by1));
    let frame = Frame { x0: bx0, top: by1 };
    let width = bx1 - bx0;
    let height = by1 - by0;

    let raw = doc.extract_spans(index).map_err(|e| e.to_string())?;
    let mut undecodable = 0;
    let mut spans = Vec::with_capacity(raw.len());
    let mut span_words: Vec<Option<Vec<(f32, f32)>>> = Vec::with_capacity(raw.len());
    for s in raw {
        if matches!(s.artifact_type, Some(ArtifactType::Pagination(_)) | Some(ArtifactType::Page)) {
            // Tagged furniture (or pdf_oxide's own page-number detection):
            // excluded here, the geometric furniture pass handles the rest.
            continue;
        }
        if s.rotation_degrees.abs() > 2.0 && (s.rotation_degrees.abs() - 360.0).abs() > 2.0 {
            // Rotated runs are margin stamps, axis labels and the like; they
            // cannot take part in left-to-right line assembly.
            continue;
        }
        let n = normalize(&s.text);
        undecodable += n.undecodable;
        if n.text.trim().is_empty() {
            continue;
        }
        let words = word_ranges(&s);
        let mut bbox = frame.rect(s.bbox.x, s.bbox.y, s.bbox.width, s.bbox.height);
        let mut text = n.text;
        // Leading spaces (code indentation drawn as space glyphs) carry
        // horizontal position: fold them into the box instead of the text.
        let lead = text.len() - text.trim_start().len();
        if lead > 0 {
            let start = words.as_ref().and_then(|w| w.first()).map(|w| w.0 - frame.x0);
            let per_char = bbox.width() / text.chars().count().max(1) as f32;
            bbox.x0 = start.unwrap_or(bbox.x0 + per_char * text[..lead].chars().count() as f32);
            text.drain(..lead);
        }
        if bbox.width() <= 0.0 || bbox.y1 < 0.0 || bbox.y0 > height || bbox.x1 < 0.0 || bbox.x0 > width {
            // Off-page text (clipped away when displayed).
            continue;
        }
        span_words.push(words.map(|w| w.into_iter().map(|(a, b)| (a - frame.x0, b - frame.x0)).collect()));
        spans.push(Span {
            text,
            bbox,
            font_size: if s.font_size > 0.0 { s.font_size } else { bbox.height().max(1.0) },
            style: Style {
                bold: (s.font_weight as u16) >= 600,
                italic: s.is_italic,
                mono: s.is_monospace,
            },
            link: None,
        });
    }

    let rules = if spans.is_empty() { Vec::new() } else { rules(doc, index, &frame) };
    let links = if spans.is_empty() { Vec::new() } else { links(doc, index, &frame) };
    crate::links::apply(&mut spans, &span_words, &links);

    Ok(PageExtract { content: PageContent { index, width, height, spans, rules }, undecodable })
}

/// X extents (PDF space) of each whitespace-separated word of a span, from
/// pdf_oxide's per-glyph advances. `None` when glyph widths are unavailable
/// or do not line up with the text.
fn word_ranges(s: &pdf_oxide::layout::TextSpan) -> Option<Vec<(f32, f32)>> {
    let chars: Vec<char> = s.text.chars().collect();
    if s.char_widths.len() != chars.len() || chars.is_empty() {
        return None;
    }
    let total: f32 = s.char_widths.iter().sum();
    if total <= 0.0 {
        return None;
    }
    // Scale advances to the span box: TJ adjustments and spacing are
    // spread proportionally, which keeps word edges within a glyph or two.
    let scale = s.bbox.width / total;
    let mut out = Vec::new();
    let mut x = s.bbox.x;
    let mut word_start: Option<f32> = None;
    for (c, w) in chars.iter().zip(&s.char_widths) {
        if c.is_whitespace() {
            if let Some(st) = word_start.take() {
                out.push((st, x));
            }
        } else if word_start.is_none() {
            word_start = Some(x);
        }
        x += w * scale;
    }
    if let Some(st) = word_start {
        out.push((st, x));
    }
    Some(out)
}

/// Maps PDF user space (y up, arbitrary origin) to the page frame.
struct Frame {
    x0: f32,
    top: f32,
}

impl Frame {
    /// `x, y` is the lower-left corner in PDF space.
    fn rect(&self, x: f32, y: f32, w: f32, h: f32) -> BBox {
        BBox::new(x - self.x0, self.top - (y + h), x + w - self.x0, self.top - y)
    }
}

fn rules(doc: &PdfDocument, index: usize, frame: &Frame) -> Vec<Rule> {
    let Ok(paths) = doc.extract_paths(index) else { return Vec::new() };
    let mut out = Vec::new();
    for p in paths {
        let r = &p.bbox;
        let (w, h) = (r.width.abs(), r.height.abs());
        if w <= RULE_THICKNESS || h <= RULE_THICKNESS {
            // Ruling line or hairline rectangle.
            if w.max(h) >= 4.0 {
                out.push(Rule { bbox: frame.rect(r.x, r.y, w.max(0.5), h.max(0.5)) });
            }
        } else if p.stroke_color.is_some() && p.is_rectangle() {
            // Stroked cell border: contributes its four edges.
            let b = frame.rect(r.x, r.y, w, h);
            out.push(Rule { bbox: BBox::new(b.x0, b.y0, b.x1, b.y0 + 0.5) });
            out.push(Rule { bbox: BBox::new(b.x0, b.y1 - 0.5, b.x1, b.y1) });
            out.push(Rule { bbox: BBox::new(b.x0, b.y0, b.x0 + 0.5, b.y1) });
            out.push(Rule { bbox: BBox::new(b.x1 - 0.5, b.y0, b.x1, b.y1) });
        }
    }
    out
}

fn links(doc: &PdfDocument, index: usize, frame: &Frame) -> Vec<Link> {
    let Ok(annots) = doc.get_annotations(index) else { return Vec::new() };
    annots
        .into_iter()
        .filter_map(|a| {
            let uri = match a.action? {
                pdf_oxide::annotations::LinkAction::Uri(u) => u,
                _ => return None,
            };
            let uri = uri.trim();
            if uri.is_empty() {
                return None;
            }
            let [x0, y0, x1, y1] = a.rect?;
            let (x0, x1) = (x0.min(x1) as f32, x0.max(x1) as f32);
            let (y0, y1) = (y0.min(y1) as f32, y0.max(y1) as f32);
            Some(Link { bbox: frame.rect(x0, y0, x1 - x0, y1 - y0), uri: Arc::from(uri) })
        })
        .collect()
}

/// Run `f`, converting a panic into an error message. pdf_oxide is robust
/// but PDF inputs are adversarial; a panic must never take the app down.
pub fn guarded<T>(f: impl FnOnce() -> T) -> Result<T, String> {
    catch_unwind(AssertUnwindSafe(f)).map_err(|p| {
        p.downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| p.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "internal parser failure".into())
    })
}
