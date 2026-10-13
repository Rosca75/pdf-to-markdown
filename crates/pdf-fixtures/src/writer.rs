//! Deterministic raw PDF writer.
//!
//! Object ids are allocated in order and written in id order, so the layout of
//! the output file is fully controlled by the order of calls: `1` is the
//! catalog, `2` the page tree, `3` the shared resource dictionary; everything
//! else is allocated by the builder methods.

use std::io::Write;

use crate::content::Content;
use crate::encryption::{Crypt, Encryption};
use crate::fonts::{Font, Std14, UFont};
use crate::obj::{deflate, name, nums, r, string, write_dict, write_obj, Body, Dict, Obj, Stream};

/// Page geometry and extras.
#[derive(Clone, Debug)]
pub struct PageSpec {
    pub media_box: [f64; 4],
    pub crop_box: Option<[f64; 4]>,
    pub rotate: Option<i32>,
    /// Annotation object ids (see [`Pdf::add_link`]).
    pub annots: Vec<u32>,
}

impl PageSpec {
    pub fn new(width: f64, height: f64) -> Self {
        PageSpec {
            media_box: [0.0, 0.0, width, height],
            crop_box: None,
            rotate: None,
            annots: Vec::new(),
        }
    }
    pub fn letter() -> Self {
        Self::new(612.0, 792.0)
    }
    pub fn a4() -> Self {
        Self::new(595.0, 842.0)
    }
    pub fn rotate(mut self, deg: i32) -> Self {
        self.rotate = Some(deg);
        self
    }
    pub fn media_box(mut self, b: [f64; 4]) -> Self {
        self.media_box = b;
        self
    }
    pub fn with_annots(mut self, a: Vec<u32>) -> Self {
        self.annots = a;
        self
    }
}

/// Object ids of a page that was added.
#[derive(Clone, Copy, Debug)]
pub struct PageIds {
    pub page: u32,
    pub contents: u32,
}

/// How to serialise the document.
#[derive(Clone, Debug)]
pub struct Output {
    /// PDF header version, e.g. `"1.4"`.
    pub version: &'static str,
    /// Use a cross-reference stream instead of a table (PDF 1.5).
    pub xref_stream: bool,
    /// Pack non-stream objects into object streams (implies `xref_stream`).
    pub object_streams: bool,
    pub encryption: Option<Encryption>,
    /// Shift every classic xref offset so that they all point to the wrong place.
    pub bad_xref: bool,
    /// Write the trailing `%%EOF`.
    pub eof: bool,
}

impl Default for Output {
    fn default() -> Self {
        Output {
            version: "1.4",
            xref_stream: false,
            object_streams: false,
            encryption: None,
            bad_xref: false,
            eof: true,
        }
    }
}

impl Output {
    /// PDF 1.5 with a cross-reference stream and object streams.
    pub fn compact() -> Self {
        Output {
            version: "1.5",
            xref_stream: true,
            object_streams: true,
            ..Default::default()
        }
    }
}

/// A serialised document.
#[derive(Clone, Debug)]
pub struct Built {
    pub bytes: Vec<u8>,
    pub startxref: usize,
    /// Trailer `/Size`.
    pub size: u32,
}

/// The document under construction.
pub struct Pdf {
    objs: Vec<Body>,
    pages: Vec<u32>,
    fonts: Vec<(String, u32)>,
    xobjs: Vec<(String, u32)>,
    gstates: Vec<(String, u32)>,
    ufonts: Vec<UFont>,
    /// Flate-compress streams created through the builder helpers.
    pub compress: bool,
    /// Fixed document id.
    pub id: [u8; 16],
}

const CATALOG: u32 = 1;
const PAGES: u32 = 2;
const RESOURCES: u32 = 3;

impl Default for Pdf {
    fn default() -> Self {
        Self::new()
    }
}

impl Pdf {
    pub fn new() -> Pdf {
        Pdf {
            objs: vec![Body::Obj(Obj::Null); 3],
            pages: Vec::new(),
            fonts: Vec::new(),
            xobjs: Vec::new(),
            gstates: Vec::new(),
            ufonts: Vec::new(),
            compress: true,
            id: *b"PDF2MD-fixture01",
        }
    }

    /// Reserve an object id.
    pub fn alloc(&mut self) -> u32 {
        self.objs.push(Body::Obj(Obj::Null));
        self.objs.len() as u32
    }

    /// Set the body of an allocated object.
    pub fn set(&mut self, id: u32, body: impl Into<Body>) {
        self.objs[(id - 1) as usize] = body.into();
    }

    /// Allocate and set in one step.
    pub fn add(&mut self, body: impl Into<Body>) -> u32 {
        let id = self.alloc();
        self.set(id, body);
        id
    }

    /// Current body of an object.
    pub fn body(&self, id: u32) -> &Body {
        &self.objs[(id - 1) as usize]
    }

    /// Build a stream honouring [`Pdf::compress`].
    pub fn make_stream(&self, data: Vec<u8>, dict: Dict) -> Stream {
        Stream {
            dict,
            data,
            flate: self.compress,
        }
    }

    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    // ---- fonts and resources ------------------------------------------------

    /// Add a standard-14 font with WinAnsiEncoding, optionally with `/Differences`
    /// (code, glyph name) layered on top of WinAnsi.
    pub fn add_std_font(&mut self, std: Std14, differences: &[(u8, &str)]) -> Font {
        let id = self.alloc();
        let res = format!("F{}", self.fonts.len() + 1);
        let differences: Vec<(u8, String)> = differences
            .iter()
            .map(|&(c, n)| (c, n.to_string()))
            .collect();
        let font = Font::new(res.clone(), id, std, differences.clone());
        let mut d = Dict::new()
            .with("Type", name("Font"))
            .with("Subtype", name("Type1"))
            .with("BaseFont", name(std.base_font()));
        if differences.is_empty() {
            d.set("Encoding", name("WinAnsiEncoding"));
        } else {
            let mut diff = Vec::new();
            let mut last: Option<u8> = None;
            let mut sorted = differences.clone();
            sorted.sort();
            for (code, glyph) in &sorted {
                if last.map(|l| l + 1) != Some(*code) {
                    diff.push(Obj::Int(*code as i64));
                }
                diff.push(name(glyph));
                last = Some(*code);
            }
            d.set(
                "Encoding",
                Dict::new()
                    .with("Type", name("Encoding"))
                    .with("BaseEncoding", name("WinAnsiEncoding"))
                    .with("Differences", Obj::Array(diff)),
            );
            let first = sorted.iter().map(|x| x.0).min().unwrap_or(32).min(32);
            d.set("FirstChar", first as i64);
            d.set("LastChar", 255);
            let w: Vec<Obj> = (first..=255u8)
                .map(|c| Obj::Int(font_width(&font, c) as i64))
                .collect();
            d.set("Widths", Obj::Array(w));
        }
        self.set(id, d);
        self.fonts.push((res, id));
        font
    }

    /// Add a Type0 Identity-H font that can show arbitrary Unicode text.
    pub fn add_unicode_font(&mut self, base_font: &str) -> UFont {
        let ids = [self.alloc(), self.alloc(), self.alloc(), self.alloc()];
        let res = format!("F{}", self.fonts.len() + 1);
        let f = UFont::new(res.clone(), base_font, ids);
        self.fonts.push((res, ids[0]));
        self.ufonts.push(f.clone());
        f
    }

    /// Add a raw image XObject; returns its resource name.
    pub fn add_image(&mut self, width: u32, height: u32, rgb: bool, data: Vec<u8>) -> String {
        let d = Dict::new()
            .with("Type", name("XObject"))
            .with("Subtype", name("Image"))
            .with("Width", width)
            .with("Height", height)
            .with(
                "ColorSpace",
                name(if rgb { "DeviceRGB" } else { "DeviceGray" }),
            )
            .with("BitsPerComponent", 8);
        let s = self.make_stream(data, d);
        let id = self.add(s);
        let res = format!("Im{}", self.xobjs.len() + 1);
        self.xobjs.push((res.clone(), id));
        res
    }

    /// Add a form XObject drawing `content` inside `bbox`; returns its resource name.
    pub fn add_form(&mut self, bbox: [f64; 4], content: &Content) -> String {
        let d = Dict::new()
            .with("Type", name("XObject"))
            .with("Subtype", name("Form"))
            .with("BBox", nums(&bbox))
            .with("Resources", r(RESOURCES));
        let s = self.make_stream(content.buf.clone(), d);
        let id = self.add(s);
        let res = format!("Fm{}", self.xobjs.len() + 1);
        self.xobjs.push((res.clone(), id));
        res
    }

    /// Add an ExtGState; returns its resource name.
    pub fn add_extgstate(&mut self, d: Dict) -> String {
        let id = self.add(d.with("Type", name("ExtGState")));
        let res = format!("GS{}", self.gstates.len() + 1);
        self.gstates.push((res.clone(), id));
        res
    }

    /// Add a URI link annotation; returns its object id.
    pub fn add_link(&mut self, rect: [f64; 4], uri: &str) -> u32 {
        let d = Dict::new()
            .with("Type", name("Annot"))
            .with("Subtype", name("Link"))
            .with("Rect", nums(&rect))
            .with("Border", Obj::Array(vec![0.into(), 0.into(), 0.into()]))
            .with(
                "A",
                Dict::new().with("S", name("URI")).with("URI", string(uri)),
            );
        self.add(d)
    }

    // ---- pages ----------------------------------------------------------------

    /// Add a page with a content stream built from `content`.
    pub fn add_page(&mut self, spec: PageSpec, content: &Content) -> PageIds {
        let s = self.make_stream(content.buf.clone(), Dict::new());
        self.add_page_stream(spec, s)
    }

    /// Add a page with an arbitrary content stream object.
    pub fn add_page_stream(&mut self, spec: PageSpec, stream: Stream) -> PageIds {
        let page = self.alloc();
        let contents = self.add(stream);
        self.set_page(page, contents, &spec);
        self.pages.push(page);
        PageIds { page, contents }
    }

    /// Add a page with no `/Contents` at all.
    pub fn add_page_without_contents(&mut self, spec: PageSpec) -> u32 {
        let page = self.alloc();
        self.set_page(page, 0, &spec);
        self.pages.push(page);
        page
    }

    fn set_page(&mut self, page: u32, contents: u32, spec: &PageSpec) {
        let mut d = Dict::new()
            .with("Type", name("Page"))
            .with("Parent", r(PAGES))
            .with("MediaBox", nums(&spec.media_box))
            .with("Resources", r(RESOURCES));
        if let Some(c) = spec.crop_box {
            d.set("CropBox", nums(&c));
        }
        if contents != 0 {
            d.set("Contents", r(contents));
        }
        if let Some(rot) = spec.rotate {
            d.set("Rotate", rot);
        }
        if !spec.annots.is_empty() {
            d.set(
                "Annots",
                Obj::Array(spec.annots.iter().map(|&a| r(a)).collect()),
            );
        }
        self.set(page, d);
    }

    /// Replace the content stream of an existing page (used for incremental updates).
    pub fn replace_content(&mut self, contents: u32, content: &Content) {
        let s = self.make_stream(content.buf.clone(), Dict::new());
        self.set(contents, s);
    }

    // ---- serialisation -------------------------------------------------------

    fn materialize(&mut self) {
        self.set(
            CATALOG,
            Dict::new()
                .with("Type", name("Catalog"))
                .with("Pages", r(PAGES)),
        );
        let kids: Vec<Obj> = self.pages.iter().map(|&p| r(p)).collect();
        self.set(
            PAGES,
            Dict::new()
                .with("Type", name("Pages"))
                .with("Count", kids.len())
                .with("Kids", Obj::Array(kids)),
        );
        let res_dict = |v: &Vec<(String, u32)>| {
            let mut d = Dict::new();
            for (n, id) in v {
                d.set(n, r(*id));
            }
            d
        };
        let mut res = Dict::new().with(
            "ProcSet",
            Obj::Array(vec![
                name("PDF"),
                name("Text"),
                name("ImageB"),
                name("ImageC"),
            ]),
        );
        if !self.fonts.is_empty() {
            res.set("Font", res_dict(&self.fonts));
        }
        if !self.xobjs.is_empty() {
            res.set("XObject", res_dict(&self.xobjs));
        }
        if !self.gstates.is_empty() {
            res.set("ExtGState", res_dict(&self.gstates));
        }
        self.set(RESOURCES, res);

        for uf in self.ufonts.clone() {
            let st = uf.state.borrow();
            let [t0, cid, desc, tou] = st.ids;
            let bf = st.base_font.clone();
            let mut w = Vec::new();
            for (i, c) in st.chars.iter().enumerate() {
                w.push(Obj::Int(i as i64 + 1));
                w.push(Obj::Array(vec![Obj::Int(UFont::char_width(*c) as i64)]));
            }
            let chars = st.chars.clone();
            drop(st);
            self.set(
                t0,
                Dict::new()
                    .with("Type", name("Font"))
                    .with("Subtype", name("Type0"))
                    .with("BaseFont", name(&bf))
                    .with("Encoding", name("Identity-H"))
                    .with("DescendantFonts", Obj::Array(vec![r(cid)]))
                    .with("ToUnicode", r(tou)),
            );
            self.set(
                cid,
                Dict::new()
                    .with("Type", name("Font"))
                    .with("Subtype", name("CIDFontType2"))
                    .with("BaseFont", name(&bf))
                    .with(
                        "CIDSystemInfo",
                        Dict::new()
                            .with("Registry", string("Adobe"))
                            .with("Ordering", string("Identity"))
                            .with("Supplement", 0),
                    )
                    .with("FontDescriptor", r(desc))
                    .with("DW", 1000)
                    .with("W", Obj::Array(w))
                    .with("CIDToGIDMap", name("Identity")),
            );
            self.set(
                desc,
                Dict::new()
                    .with("Type", name("FontDescriptor"))
                    .with("FontName", name(&bf))
                    .with("Flags", 32)
                    .with("FontBBox", nums(&[-200.0, -300.0, 1200.0, 1000.0]))
                    .with("ItalicAngle", 0)
                    .with("Ascent", 900)
                    .with("Descent", -200)
                    .with("CapHeight", 700)
                    .with("StemV", 80),
            );
            let cmap = to_unicode_cmap(&chars);
            let s = self.make_stream(cmap.into_bytes(), Dict::new());
            self.set(tou, s);
        }
    }

    /// Serialise the document.
    pub fn build(&mut self, out: &Output) -> Built {
        self.materialize();
        let n = self.objs.len() as u32;
        let crypt = out.encryption.as_ref().map(|e| Crypt::new(e, &self.id));
        let use_stm = out.object_streams;
        let xref_stream = out.xref_stream || use_stm;

        let mut b: Vec<u8> = Vec::new();
        let _ = write!(b, "%PDF-{}\n%", out.version);
        b.extend_from_slice(&[0xE2, 0xE3, 0xCF, 0xD3, b'\n']);

        // Entry per object id (index = id).
        #[derive(Clone, Copy)]
        enum Entry {
            Free,
            Offset(usize),
            InStm(u32, u32),
        }
        let mut entries: Vec<Entry> = vec![Entry::Free; n as usize + 1];
        let mut next_id = n + 1;

        // Which objects go into object streams.
        let in_stm: Vec<u32> = if use_stm {
            (1..=n)
                .filter(|&i| matches!(self.objs[(i - 1) as usize], Body::Obj(_)))
                .collect()
        } else {
            Vec::new()
        };
        let stm_set: std::collections::HashSet<u32> = in_stm.iter().copied().collect();

        for id in 1..=n {
            if stm_set.contains(&id) {
                continue;
            }
            entries[id as usize] = Entry::Offset(b.len());
            self.write_indirect(&mut b, id, crypt.as_ref());
        }

        let mut encrypt_ref = None;
        if let Some(c) = &crypt {
            let eid = next_id;
            next_id += 1;
            entries.push(Entry::Offset(b.len()));
            let _ = writeln!(b, "{eid} 0 obj");
            write_dict(&mut b, &c.dict(), None);
            b.extend_from_slice(b"\nendobj\n");
            encrypt_ref = Some(eid);
        }

        for chunk in in_stm.chunks(50) {
            let sid = next_id;
            next_id += 1;
            let mut header = String::new();
            let mut body: Vec<u8> = Vec::new();
            for (idx, &id) in chunk.iter().enumerate() {
                header.push_str(&format!("{} {} ", id, body.len()));
                if let Body::Obj(o) = &self.objs[(id - 1) as usize] {
                    write_obj(&mut body, o, None);
                }
                body.push(b'\n');
                entries[id as usize] = Entry::InStm(sid, idx as u32);
            }
            let mut data = header.clone().into_bytes();
            data.extend_from_slice(&body);
            let dict = Dict::new()
                .with("Type", name("ObjStm"))
                .with("N", chunk.len())
                .with("First", header.len());
            let st = Stream {
                dict,
                data,
                flate: true,
            };
            entries.push(Entry::Offset(b.len()));
            self.write_stream_obj(&mut b, sid, &st, crypt.as_ref());
        }

        let mut size = next_id;
        let startxref;
        let trailer_common = |d: &mut Dict| {
            d.set("Root", r(CATALOG));
            if let Some(e) = encrypt_ref {
                d.set("Encrypt", r(e));
            }
            d.set(
                "ID",
                Obj::Array(vec![Obj::Str(self.id.to_vec()), Obj::Str(self.id.to_vec())]),
            );
        };

        if xref_stream {
            let xid = next_id;
            size = xid + 1;
            entries.push(Entry::Offset(b.len()));
            startxref = b.len();
            let mut data = Vec::new();
            for e in &entries {
                match *e {
                    Entry::Free => data.extend_from_slice(&[0, 0, 0, 0, 0, 0xFF, 0xFF][..]),
                    Entry::Offset(o) => {
                        data.push(1);
                        data.extend_from_slice(&(o as u32).to_be_bytes());
                        data.extend_from_slice(&0u16.to_be_bytes());
                    }
                    Entry::InStm(s, i) => {
                        data.push(2);
                        data.extend_from_slice(&s.to_be_bytes());
                        data.extend_from_slice(&(i as u16).to_be_bytes());
                    }
                }
            }
            let mut dict = Dict::new()
                .with("Type", name("XRef"))
                .with("Size", size)
                .with("W", Obj::Array(vec![1.into(), 4.into(), 2.into()]));
            trailer_common(&mut dict);
            let st = Stream {
                dict,
                data,
                flate: true,
            };
            self.write_stream_obj(&mut b, xid, &st, None);
        } else {
            startxref = b.len();
            let _ = writeln!(b, "xref\n0 {size}");
            b.extend_from_slice(b"0000000000 65535 f \n");
            for e in entries.iter().skip(1) {
                let off = match e {
                    Entry::Offset(o) => *o + if out.bad_xref { 17 } else { 0 },
                    _ => 0,
                };
                let _ = writeln!(b, "{off:010} 00000 n ");
            }
            let mut t = Dict::new().with("Size", size);
            trailer_common(&mut t);
            b.extend_from_slice(b"trailer\n");
            write_dict(&mut b, &t, None);
            b.push(b'\n');
        }
        let _ = writeln!(b, "startxref\n{startxref}");
        if out.eof {
            b.extend_from_slice(b"%%EOF\n");
        }
        Built {
            bytes: b,
            startxref,
            size,
        }
    }

    /// Append an incremental update (classic xref) replacing/adding objects `ids`
    /// with their current bodies. `base` must come from a non-encrypted classic build.
    pub fn append_update(&mut self, base: &Built, ids: &[u32]) -> Vec<u8> {
        let mut b = base.bytes.clone();
        let mut ids = ids.to_vec();
        ids.sort_unstable();
        let mut offsets = Vec::new();
        for &id in &ids {
            offsets.push((id, b.len()));
            self.write_indirect(&mut b, id, None);
        }
        let size = base.size.max(self.objs.len() as u32 + 1);
        let startxref = b.len();
        b.extend_from_slice(b"xref\n");
        let mut i = 0;
        while i < offsets.len() {
            let mut j = i;
            while j + 1 < offsets.len() && offsets[j + 1].0 == offsets[j].0 + 1 {
                j += 1;
            }
            let _ = writeln!(b, "{} {}", offsets[i].0, j - i + 1);
            for (_, off) in &offsets[i..=j] {
                let _ = writeln!(b, "{off:010} 00000 n ");
            }
            i = j + 1;
        }
        let t = Dict::new()
            .with("Size", size)
            .with("Root", r(CATALOG))
            .with("Prev", base.startxref)
            .with(
                "ID",
                Obj::Array(vec![Obj::Str(self.id.to_vec()), Obj::Str(self.id.to_vec())]),
            );
        b.extend_from_slice(b"trailer\n");
        write_dict(&mut b, &t, None);
        let _ = writeln!(b, "\nstartxref\n{startxref}\n%%EOF");
        b
    }

    fn write_indirect(&self, b: &mut Vec<u8>, id: u32, crypt: Option<&Crypt>) {
        match &self.objs[(id - 1) as usize] {
            Body::Obj(o) => {
                let _ = writeln!(b, "{id} 0 obj");
                match crypt {
                    Some(c) => {
                        let f = |s: &[u8]| c.encrypt(id, s);
                        write_obj(b, o, Some(&f));
                    }
                    None => write_obj(b, o, None),
                }
                b.extend_from_slice(b"\nendobj\n");
            }
            Body::Stream(s) => self.write_stream_obj(b, id, s, crypt),
        }
    }

    fn write_stream_obj(&self, b: &mut Vec<u8>, id: u32, s: &Stream, crypt: Option<&Crypt>) {
        let mut data = if s.flate {
            deflate(&s.data)
        } else {
            s.data.clone()
        };
        let mut dict = s.dict.clone();
        if s.flate {
            dict.set("Filter", name("FlateDecode"));
        }
        if let Some(c) = crypt {
            data = c.encrypt(id, &data);
        }
        dict.set("Length", data.len());
        let _ = writeln!(b, "{id} 0 obj");
        match crypt {
            Some(c) => {
                let f = |x: &[u8]| c.encrypt(id, x);
                write_dict_enc(b, &dict, &f);
            }
            None => write_dict(b, &dict, None),
        }
        b.extend_from_slice(b"\nstream\n");
        b.extend_from_slice(&data);
        b.extend_from_slice(b"\nendstream\nendobj\n");
    }
}

fn write_dict_enc(b: &mut Vec<u8>, d: &Dict, f: &dyn Fn(&[u8]) -> Vec<u8>) {
    write_obj(b, &Obj::Dict(d.clone()), Some(f));
}

fn font_width(f: &Font, code: u8) -> u16 {
    f.widths[code as usize]
}

fn to_unicode_cmap(chars: &[char]) -> String {
    let mut s = String::from(
        "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n\
         /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n\
         /CMapName /Adobe-Identity-UCS def\n/CMapType 2 def\n\
         1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n",
    );
    for (chunk_idx, chunk) in chars.chunks(100).enumerate() {
        s.push_str(&format!("{} beginbfchar\n", chunk.len()));
        for (i, c) in chunk.iter().enumerate() {
            let cid = chunk_idx * 100 + i + 1;
            let mut buf = [0u16; 2];
            let units = c.encode_utf16(&mut buf);
            let hex: String = units.iter().map(|u| format!("{u:04X}")).collect();
            s.push_str(&format!("<{cid:04X}> <{hex}>\n"));
        }
        s.push_str("endbfchar\n");
    }
    s.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
    s
}
