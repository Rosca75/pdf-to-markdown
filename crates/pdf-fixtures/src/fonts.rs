//! Font metrics (built-in AFM width tables), simple-font encodings and the
//! Unicode (Type0 / Identity-H) font used to emit arbitrary text.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// The standard-14 fonts supported by the writer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Std14 {
    Helvetica,
    HelveticaBold,
    HelveticaOblique,
    HelveticaBoldOblique,
    TimesRoman,
    TimesBold,
    TimesItalic,
    TimesBoldItalic,
    Courier,
    CourierBold,
    CourierOblique,
    CourierBoldOblique,
}

impl Std14 {
    pub fn base_font(self) -> &'static str {
        match self {
            Std14::Helvetica => "Helvetica",
            Std14::HelveticaBold => "Helvetica-Bold",
            Std14::HelveticaOblique => "Helvetica-Oblique",
            Std14::HelveticaBoldOblique => "Helvetica-BoldOblique",
            Std14::TimesRoman => "Times-Roman",
            Std14::TimesBold => "Times-Bold",
            Std14::TimesItalic => "Times-Italic",
            Std14::TimesBoldItalic => "Times-BoldItalic",
            Std14::Courier => "Courier",
            Std14::CourierBold => "Courier-Bold",
            Std14::CourierOblique => "Courier-Oblique",
            Std14::CourierBoldOblique => "Courier-BoldOblique",
        }
    }

    fn table(self) -> Option<&'static [u16; 95]> {
        match self {
            Std14::Helvetica | Std14::HelveticaOblique => Some(&HELV),
            Std14::HelveticaBold | Std14::HelveticaBoldOblique => Some(&HELV_B),
            Std14::TimesRoman => Some(&TIMES),
            // Times-BoldItalic widths are approximated by Times-Bold.
            Std14::TimesBold | Std14::TimesBoldItalic => Some(&TIMES_B),
            Std14::TimesItalic => Some(&TIMES_I),
            _ => None,
        }
    }

    fn is_times(self) -> bool {
        matches!(
            self,
            Std14::TimesRoman | Std14::TimesBold | Std14::TimesItalic | Std14::TimesBoldItalic
        )
    }

    /// Advance width of a character in 1/1000 em.
    pub fn char_width(self, c: char) -> u16 {
        let Some(t) = self.table() else {
            return 600;
        };
        let cp = c as u32;
        if (32..=126).contains(&cp) {
            return t[(cp - 32) as usize];
        }
        if let Some(b) = base_letter(c) {
            return t[(b as u32 - 32) as usize];
        }
        let times = self.is_times();
        // (Helvetica width, Times width)
        let (h, tm) = match c {
            '\u{A0}' => return t[0],
            '€' | '¢' | '£' | '¥' | '¤' | '§' | 'µ' | '«' | '»' => (556, 500),
            '‘' | '’' | '‚' => (222, 333),
            '“' | '”' | '„' => (333, 444),
            '–' => (556, 500),
            '—' | '…' | '™' | '‰' => (1000, 1000),
            '•' => (350, 350),
            '©' | '®' => (737, 760),
            '°' => (400, 400),
            '±' | '×' | '÷' | '¬' => (584, 564),
            'ß' => (611, 500),
            '¿' => (611, 444),
            '¡' | '´' | '¸' | '¨' | '¯' => (333, 333),
            '¼' | '½' | '¾' => (834, 750),
            'Æ' | 'Œ' => (1000, 889),
            'æ' | 'œ' => (889, 667),
            'Ø' => (778, 722),
            'ø' | 'ð' | 'þ' => (556, 500),
            _ => (500, 500),
        };
        if times {
            tm
        } else {
            h
        }
    }
}

fn base_letter(c: char) -> Option<char> {
    Some(match c {
        'À'..='Å' => 'A',
        'Ç' => 'C',
        'È'..='Ë' => 'E',
        'Ì'..='Ï' => 'I',
        'Ñ' => 'N',
        'Ò'..='Ö' => 'O',
        'Ù'..='Ü' => 'U',
        'Ý' | 'Ÿ' => 'Y',
        'à'..='å' => 'a',
        'ç' => 'c',
        'è'..='ë' => 'e',
        'ì'..='ï' => 'i',
        'ñ' => 'n',
        'ò'..='ö' => 'o',
        'ù'..='ü' => 'u',
        'ý' | 'ÿ' => 'y',
        'Š' => 'S',
        'š' => 's',
        'Ž' => 'Z',
        'ž' => 'z',
        _ => return None,
    })
}

#[rustfmt::skip]
static HELV: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278,
    556, 556, 556, 556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584, 556,
    1015, 667, 667, 722, 722, 667, 611, 778, 722, 278, 500, 667, 556, 833, 722, 778,
    667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 278, 278, 278, 469, 556,
    333, 556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500, 222, 833, 556, 556,
    556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334, 260, 334, 584,
];

#[rustfmt::skip]
static HELV_B: [u16; 95] = [
    278, 333, 474, 556, 556, 889, 722, 238, 333, 333, 389, 584, 278, 333, 278, 278,
    556, 556, 556, 556, 556, 556, 556, 556, 556, 556, 333, 333, 584, 584, 584, 611,
    975, 722, 722, 722, 722, 667, 611, 778, 722, 278, 556, 722, 611, 833, 722, 778,
    667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 333, 278, 333, 584, 556,
    333, 556, 611, 556, 611, 556, 333, 611, 611, 278, 278, 556, 278, 889, 611, 611,
    611, 611, 389, 556, 333, 611, 556, 778, 556, 556, 500, 389, 280, 389, 584,
];

#[rustfmt::skip]
static TIMES: [u16; 95] = [
    250, 333, 408, 500, 500, 833, 778, 180, 333, 333, 500, 564, 250, 333, 250, 278,
    500, 500, 500, 500, 500, 500, 500, 500, 500, 500, 278, 278, 564, 564, 564, 444,
    921, 722, 667, 667, 722, 611, 556, 722, 722, 333, 389, 722, 611, 889, 722, 722,
    556, 722, 667, 556, 611, 722, 722, 944, 722, 722, 611, 333, 278, 333, 469, 500,
    333, 444, 500, 444, 500, 444, 333, 500, 500, 278, 278, 500, 278, 778, 500, 500,
    500, 500, 333, 389, 278, 500, 500, 722, 500, 500, 444, 480, 200, 480, 541,
];

#[rustfmt::skip]
static TIMES_B: [u16; 95] = [
    250, 333, 555, 500, 500, 1000, 833, 278, 333, 333, 500, 570, 250, 333, 250, 278,
    500, 500, 500, 500, 500, 500, 500, 500, 500, 500, 333, 333, 570, 570, 570, 500,
    930, 722, 667, 722, 722, 667, 611, 778, 778, 389, 500, 778, 667, 944, 722, 778,
    611, 778, 722, 556, 667, 722, 722, 1000, 722, 722, 667, 333, 278, 333, 581, 500,
    333, 500, 556, 444, 556, 444, 333, 500, 556, 278, 333, 556, 278, 833, 556, 500,
    556, 556, 444, 389, 333, 556, 500, 722, 500, 500, 444, 394, 220, 394, 520,
];

#[rustfmt::skip]
static TIMES_I: [u16; 95] = [
    250, 333, 420, 500, 500, 833, 778, 214, 333, 333, 500, 675, 250, 333, 250, 278,
    500, 500, 500, 500, 500, 500, 500, 500, 500, 500, 333, 333, 675, 675, 675, 500,
    920, 611, 611, 667, 722, 611, 611, 722, 722, 333, 444, 667, 556, 833, 667, 722,
    611, 722, 611, 500, 556, 722, 611, 833, 611, 556, 556, 389, 278, 389, 422, 500,
    333, 500, 500, 444, 500, 444, 278, 500, 500, 278, 278, 444, 278, 722, 500, 500,
    500, 500, 389, 389, 278, 500, 444, 667, 444, 444, 389, 400, 275, 400, 541,
];

/// cp1252 0x80..=0x9F (NUL = undefined).
const CP1252_HIGH: [char; 32] = [
    '€', '\0', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\0', 'Ž', '\0', '\0', '‘',
    '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\0', 'ž', 'Ÿ',
];

/// Character for a WinAnsi code (None if undefined).
pub fn winansi_char(code: u8) -> Option<char> {
    match code {
        0x20..=0x7E => Some(code as char),
        0x80..=0x9F => {
            let c = CP1252_HIGH[(code - 0x80) as usize];
            (c != '\0').then_some(c)
        }
        0xA0..=0xFF => Some(code as char),
        _ => None,
    }
}

/// WinAnsi code for a character.
pub fn winansi_code(c: char) -> Option<u8> {
    let cp = c as u32;
    if (0x20..=0x7E).contains(&cp) || (0xA0..=0xFF).contains(&cp) {
        return Some(cp as u8);
    }
    CP1252_HIGH
        .iter()
        .position(|&h| h == c && h != '\0')
        .map(|i| 0x80 + i as u8)
}

/// Map an Adobe glyph name to a Unicode character (small built-in subset plus `uniXXXX`).
pub fn glyph_to_char(name: &str) -> Option<char> {
    if let Some(hex) = name.strip_prefix("uni") {
        if hex.len() == 4 {
            return u32::from_str_radix(hex, 16).ok().and_then(char::from_u32);
        }
    }
    const TABLE: &[(&str, char)] = &[
        ("alpha", 'α'),
        ("beta", 'β'),
        ("gamma", 'γ'),
        ("delta", 'δ'),
        ("epsilon", 'ε'),
        ("lambda", 'λ'),
        ("mu", 'μ'),
        ("pi", 'π'),
        ("sigma", 'σ'),
        ("omega", 'ω'),
        ("Omega", 'Ω'),
        ("Euro", '€'),
        ("fi", '\u{FB01}'),
        ("fl", '\u{FB02}'),
        ("ff", '\u{FB00}'),
        ("ffi", '\u{FB03}'),
        ("bullet", '•'),
        ("endash", '–'),
        ("emdash", '—'),
        ("ellipsis", '…'),
        ("quotedblleft", '“'),
        ("quotedblright", '”'),
        ("quoteleft", '‘'),
        ("quoteright", '’'),
        ("space", ' '),
        ("eacute", 'é'),
        ("section", '§'),
    ];
    TABLE.iter().find(|(n, _)| *n == name).map(|&(_, c)| c)
}

/// Anything that can show text in a content stream.
pub trait TextFont {
    /// Resource name (e.g. `F1`).
    fn res(&self) -> &str;
    /// The string operand (including delimiters) that shows `s`.
    fn operand(&self, s: &str) -> Vec<u8>;
    /// Width of `s` in 1/1000 em.
    fn width_units(&self, s: &str) -> f64;
    /// Width of `s` in points at `size`.
    fn width(&self, s: &str, size: f64) -> f64 {
        self.width_units(s) * size / 1000.0
    }
}

/// A simple (single-byte) font.
#[derive(Clone, Debug)]
pub struct Font {
    pub res: String,
    pub obj: u32,
    pub std: Std14,
    pub(crate) widths: Vec<u16>,
    pub(crate) to_code: HashMap<char, u8>,
    /// Fast path for ASCII: (code, width); code 0 = unmapped.
    ascii: Vec<(u8, u16)>,
    /// Glyph-name differences over WinAnsi, if any.
    pub differences: Vec<(u8, String)>,
}

impl Font {
    pub(crate) fn new(res: String, obj: u32, std: Std14, differences: Vec<(u8, String)>) -> Font {
        let mut chars: Vec<Option<char>> = (0..=255u8).map(winansi_char).collect();
        for (code, glyph) in &differences {
            chars[*code as usize] = glyph_to_char(glyph);
        }
        let mut to_code = HashMap::new();
        let mut widths = vec![0u16; 256];
        for (code, ch) in chars.iter().enumerate() {
            if let Some(c) = ch {
                to_code.entry(*c).or_insert(code as u8);
                widths[code] = std.char_width(*c);
            }
        }
        // Differences take priority for their characters.
        for (code, glyph) in &differences {
            if let Some(c) = glyph_to_char(glyph) {
                to_code.insert(c, *code);
            }
        }
        let ascii = (0..128u8)
            .map(|b| match to_code.get(&(b as char)) {
                Some(&code) => (code, widths[code as usize]),
                None => (0, 0),
            })
            .collect();
        Font {
            res,
            obj,
            std,
            widths,
            to_code,
            ascii,
            differences,
        }
    }

    /// Encode text to single-byte codes (unmappable characters become `?`).
    pub fn encode(&self, s: &str) -> Vec<u8> {
        s.chars()
            .map(|c| {
                if c.is_ascii() && self.ascii[c as usize].0 != 0 {
                    self.ascii[c as usize].0
                } else {
                    self.to_code.get(&c).copied().unwrap_or(b'?')
                }
            })
            .collect()
    }
}

impl TextFont for Font {
    fn res(&self) -> &str {
        &self.res
    }
    fn operand(&self, s: &str) -> Vec<u8> {
        crate::obj::literal_string(&self.encode(s))
    }
    fn width_units(&self, s: &str) -> f64 {
        s.chars()
            .map(|c| {
                if c.is_ascii() && self.ascii[c as usize].0 != 0 {
                    return self.ascii[c as usize].1 as f64;
                }
                self.to_code
                    .get(&c)
                    .map_or(500.0, |&code| self.widths[code as usize] as f64)
            })
            .sum()
    }
}

#[derive(Debug, Default)]
pub(crate) struct UState {
    pub chars: Vec<char>,
    pub map: HashMap<char, u16>,
    pub base_font: String,
    /// Object ids: Type0, CIDFont, FontDescriptor, ToUnicode.
    pub ids: [u32; 4],
}

/// A Type0 / Identity-H / CIDFontType2 font without an embedded font program.
///
/// CIDs are assigned sequentially on first use, so the `/ToUnicode` CMap is the
/// only way to recover the text.
#[derive(Clone, Debug)]
pub struct UFont {
    pub res: String,
    pub(crate) state: Rc<RefCell<UState>>,
}

impl UFont {
    pub(crate) fn new(res: String, base_font: &str, ids: [u32; 4]) -> UFont {
        UFont {
            res,
            state: Rc::new(RefCell::new(UState {
                base_font: base_font.to_string(),
                ids,
                ..Default::default()
            })),
        }
    }

    /// Object id of the Type0 font dictionary.
    pub fn obj(&self) -> u32 {
        self.state.borrow().ids[0]
    }

    /// Width of one character in 1/1000 em.
    pub fn char_width(c: char) -> u16 {
        let cp = c as u32;
        if winansi_code(c).is_some() {
            Std14::Helvetica.char_width(c)
        } else if cp >= 0x2E80 {
            1000
        } else if cp == 0xFB01 || cp == 0xFB02 {
            500
        } else if cp == 0xFB00 {
            600
        } else if cp == 0xFB03 || cp == 0xFB04 {
            800
        } else {
            620
        }
    }

    /// Encode text as big-endian 2-byte CIDs, assigning new CIDs as needed.
    pub fn encode(&self, s: &str) -> Vec<u8> {
        let mut st = self.state.borrow_mut();
        let mut out = Vec::with_capacity(s.len() * 2);
        for c in s.chars() {
            let cid = if let Some(&cid) = st.map.get(&c) {
                cid
            } else {
                st.chars.push(c);
                let cid = st.chars.len() as u16;
                st.map.insert(c, cid);
                cid
            };
            out.extend_from_slice(&cid.to_be_bytes());
        }
        out
    }
}

impl TextFont for UFont {
    fn res(&self) -> &str {
        &self.res
    }
    fn operand(&self, s: &str) -> Vec<u8> {
        crate::obj::hex_string(&self.encode(s))
    }
    fn width_units(&self, s: &str) -> f64 {
        s.chars().map(|c| UFont::char_width(c) as f64).sum()
    }
}
