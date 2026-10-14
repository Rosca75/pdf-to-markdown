//! Engine-internal document model shared by the pipeline stages.
//!
//! Coordinates are in PDF points in a *top-down* page frame: the origin is
//! the top-left corner of the page's visible box, `y` grows downwards. The
//! extraction stage converts pdf_oxide's coordinates into this frame so that
//! no later stage deals with box origins, rotation or y-flipping.

use std::sync::Arc;

/// Axis-aligned rectangle, top-down coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct BBox {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

impl BBox {
    pub fn new(x0: f32, y0: f32, x1: f32, y1: f32) -> Self {
        Self {
            x0: x0.min(x1),
            y0: y0.min(y1),
            x1: x0.max(x1),
            y1: y0.max(y1),
        }
    }
    pub fn width(&self) -> f32 {
        self.x1 - self.x0
    }
    pub fn height(&self) -> f32 {
        self.y1 - self.y0
    }
    pub fn union(&self, o: &BBox) -> BBox {
        BBox {
            x0: self.x0.min(o.x0),
            y0: self.y0.min(o.y0),
            x1: self.x1.max(o.x1),
            y1: self.y1.max(o.y1),
        }
    }
    /// Length of the vertical overlap with `o` (0 when disjoint).
    pub fn v_overlap(&self, o: &BBox) -> f32 {
        (self.y1.min(o.y1) - self.y0.max(o.y0)).max(0.0)
    }
    /// Length of the horizontal overlap with `o` (0 when disjoint).
    pub fn h_overlap(&self, o: &BBox) -> f32 {
        (self.x1.min(o.x1) - self.x0.max(o.x0)).max(0.0)
    }
    pub fn intersects(&self, o: &BBox) -> bool {
        self.h_overlap(o) > 0.0 && self.v_overlap(o) > 0.0
    }
}

/// Character-level typographic style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Style {
    pub bold: bool,
    pub italic: bool,
    pub mono: bool,
}

/// A run of text with uniform style, as produced by extraction.
#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    pub text: String,
    pub bbox: BBox,
    pub font_size: f32,
    pub style: Style,
    /// Target URI when the span lies under a link annotation.
    pub link: Option<Arc<str>>,
}

/// A horizontal ruling (table border, underline, separator) from vector paths.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rule {
    pub bbox: BBox,
}

/// URI link annotation area.
#[derive(Debug, Clone, PartialEq)]
pub struct Link {
    pub bbox: BBox,
    pub uri: Arc<str>,
}

/// Everything the layout stages need from one page.
#[derive(Debug, Clone, Default)]
pub struct PageContent {
    pub index: usize,
    pub width: f32,
    pub height: f32,
    pub spans: Vec<Span>,
}

/// Styled inline text inside a block.
#[derive(Debug, Clone, PartialEq)]
pub struct Inline {
    pub text: String,
    pub style: Style,
    pub link: Option<Arc<str>>,
}

impl Inline {
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            style: Style::default(),
            link: None,
        }
    }
}

/// List marker kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Marker {
    Bullet,
    /// Ordered item `3.`; the literal number is kept.
    Ordered(u32),
    /// Ordered item `3)` or `(3)`. Kept distinct because CommonMark starts a
    /// new list when the delimiter changes, which is how consecutive lists
    /// with different numbering styles stay separate.
    OrderedParen(u32),
}

/// A semantic block, the unit Markdown is rendered from.
#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    Heading {
        level: u8,
        inlines: Vec<Inline>,
    },
    Paragraph {
        inlines: Vec<Inline>,
    },
    ListItem {
        depth: u8,
        marker: Marker,
        inlines: Vec<Inline>,
    },
    Code {
        lines: Vec<String>,
    },
    /// Rows of cells; the first row is rendered as the header.
    Table {
        rows: Vec<Vec<Vec<Inline>>>,
    },
}
