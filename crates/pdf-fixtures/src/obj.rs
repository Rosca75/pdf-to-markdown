//! PDF object model and byte serialisation primitives.

use std::io::Write;

/// A PDF object (direct value).
#[derive(Clone, Debug)]
pub enum Obj {
    Null,
    Bool(bool),
    Int(i64),
    Real(f64),
    Name(String),
    /// A string; written as a literal string (or hex when encrypted/binary).
    Str(Vec<u8>),
    Array(Vec<Obj>),
    Dict(Dict),
    /// Indirect reference (generation is always 0).
    Ref(u32),
}

/// An ordered PDF dictionary.
#[derive(Clone, Debug, Default)]
pub struct Dict(pub Vec<(String, Obj)>);

impl Dict {
    pub fn new() -> Self {
        Dict(Vec::new())
    }

    /// Builder-style insert.
    pub fn with(mut self, key: &str, v: impl Into<Obj>) -> Self {
        self.set(key, v);
        self
    }

    /// Insert or replace a key.
    pub fn set(&mut self, key: &str, v: impl Into<Obj>) {
        let v = v.into();
        if let Some(slot) = self.0.iter_mut().find(|(k, _)| k == key) {
            slot.1 = v;
        } else {
            self.0.push((key.to_string(), v));
        }
    }

    pub fn get(&self, key: &str) -> Option<&Obj> {
        self.0.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn remove(&mut self, key: &str) {
        self.0.retain(|(k, _)| k != key);
    }
}

impl From<i32> for Obj {
    fn from(v: i32) -> Self {
        Obj::Int(v as i64)
    }
}
impl From<i64> for Obj {
    fn from(v: i64) -> Self {
        Obj::Int(v)
    }
}
impl From<u32> for Obj {
    fn from(v: u32) -> Self {
        Obj::Int(v as i64)
    }
}
impl From<usize> for Obj {
    fn from(v: usize) -> Self {
        Obj::Int(v as i64)
    }
}
impl From<f64> for Obj {
    fn from(v: f64) -> Self {
        Obj::Real(v)
    }
}
impl From<bool> for Obj {
    fn from(v: bool) -> Self {
        Obj::Bool(v)
    }
}
impl From<Dict> for Obj {
    fn from(v: Dict) -> Self {
        Obj::Dict(v)
    }
}
impl From<Vec<Obj>> for Obj {
    fn from(v: Vec<Obj>) -> Self {
        Obj::Array(v)
    }
}

/// Name object constructor.
pub fn name(s: &str) -> Obj {
    Obj::Name(s.to_string())
}

/// Literal string object constructor.
pub fn string(s: &str) -> Obj {
    Obj::Str(s.as_bytes().to_vec())
}

/// Indirect reference constructor.
pub fn r(id: u32) -> Obj {
    Obj::Ref(id)
}

/// Array of numbers.
pub fn nums(v: &[f64]) -> Obj {
    Obj::Array(v.iter().map(|&x| Obj::Real(x)).collect())
}

/// A stream object: a dictionary plus data.
#[derive(Clone, Debug)]
pub struct Stream {
    /// Extra dictionary entries (`/Length` and `/Filter` for `flate` are added on write).
    pub dict: Dict,
    pub data: Vec<u8>,
    /// Compress `data` with Flate when writing.
    pub flate: bool,
}

impl Stream {
    pub fn new(data: Vec<u8>, flate: bool) -> Self {
        Stream {
            dict: Dict::new(),
            data,
            flate,
        }
    }
}

/// The body of an indirect object.
#[derive(Clone, Debug)]
pub enum Body {
    Obj(Obj),
    Stream(Stream),
}

impl From<Obj> for Body {
    fn from(o: Obj) -> Self {
        Body::Obj(o)
    }
}
impl From<Dict> for Body {
    fn from(d: Dict) -> Self {
        Body::Obj(Obj::Dict(d))
    }
}
impl From<Stream> for Body {
    fn from(s: Stream) -> Self {
        Body::Stream(s)
    }
}

/// Format a real number without exponent, at most 4 decimals.
pub fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e12 {
        return format!("{}", v as i64);
    }
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" || s.is_empty() {
        "0".to_string()
    } else {
        s.to_string()
    }
}

/// Escape bytes as a PDF literal string, including parentheses.
pub fn literal_string(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len() + 2);
    out.push(b'(');
    for &b in bytes {
        match b {
            b'(' | b')' | b'\\' => {
                out.push(b'\\');
                out.push(b);
            }
            b'\n' => out.extend_from_slice(b"\\n"),
            b'\r' => out.extend_from_slice(b"\\r"),
            b'\t' => out.extend_from_slice(b"\\t"),
            0x20..=0x7E => out.push(b),
            _ => {
                let _ = write!(out, "\\{b:03o}");
            }
        }
    }
    out.push(b')');
    out
}

/// Hex string `<...>`.
pub fn hex_string(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len() * 2 + 2);
    out.push(b'<');
    for b in bytes {
        let _ = write!(out, "{b:02X}");
    }
    out.push(b'>');
    out
}

/// Optional string transformer (encryption) applied to every string.
pub type StrXform<'a> = Option<&'a dyn Fn(&[u8]) -> Vec<u8>>;

/// Serialise an object to `out`.
pub fn write_obj(out: &mut Vec<u8>, o: &Obj, enc: StrXform) {
    match o {
        Obj::Null => out.extend_from_slice(b"null"),
        Obj::Bool(b) => out.extend_from_slice(if *b { b"true" } else { b"false" }),
        Obj::Int(i) => {
            let _ = write!(out, "{i}");
        }
        Obj::Real(f) => out.extend_from_slice(fmt_num(*f).as_bytes()),
        Obj::Name(n) => {
            out.push(b'/');
            for &b in n.as_bytes() {
                if b.is_ascii_alphanumeric() || b"-_.+".contains(&b) {
                    out.push(b);
                } else {
                    let _ = write!(out, "#{b:02X}");
                }
            }
        }
        Obj::Str(s) => match enc {
            Some(f) => out.extend_from_slice(&hex_string(&f(s))),
            None => out.extend_from_slice(&literal_string(s)),
        },
        Obj::Array(a) => {
            out.push(b'[');
            for (i, v) in a.iter().enumerate() {
                if i > 0 {
                    out.push(b' ');
                }
                write_obj(out, v, enc);
            }
            out.push(b']');
        }
        Obj::Dict(d) => write_dict(out, d, enc),
        Obj::Ref(id) => {
            let _ = write!(out, "{id} 0 R");
        }
    }
}

/// Serialise a dictionary `<< ... >>`.
pub fn write_dict(out: &mut Vec<u8>, d: &Dict, enc: StrXform) {
    out.extend_from_slice(b"<<");
    for (k, v) in &d.0 {
        write_obj(out, &Obj::Name(k.clone()), None);
        out.push(b' ');
        write_obj(out, v, enc);
        out.push(b'\n');
    }
    out.extend_from_slice(b">>");
}

/// Deflate (zlib) compress.
pub fn deflate(data: &[u8]) -> Vec<u8> {
    use flate2::write::ZlibEncoder;
    use flate2::Compression;
    let mut e = ZlibEncoder::new(Vec::with_capacity(data.len() / 2 + 16), Compression::fast());
    let _ = e.write_all(data);
    e.finish().unwrap_or_default()
}
