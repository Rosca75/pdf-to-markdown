//! Structural corpus cases: encryption, damaged files, object streams, updates.

use super::common::*;
use crate::content::Content;
use crate::encryption::Encryption;
use crate::layout::{Rng, StdFonts};
use crate::obj::{name, Dict, Stream};
use crate::writer::{Output, PageSpec, Pdf};

const ORDINALS: [&str; 3] = ["one", "two", "three"];

fn recovery_page(f: &StdFonts, n: usize) -> Content {
    let mut p = Page::new();
    p.heading(&f.helv_b, 18.0, &format!("Chapter {}", ORDINALS[n]));
    p.body(
        &f.times,
        &format!(
            "Recovery marker {} stays readable even though the file structure is damaged. The converter should still deliver this paragraph.",
            ORDINALS[n]
        ),
    );
    p.c
}

/// A three-page document with uncompressed streams.
fn three_pages(compress: bool) -> Pdf {
    let mut pdf = Pdf::new();
    pdf.compress = compress;
    let f = StdFonts::new(&mut pdf);
    for n in 0..3 {
        pdf.add_page(PageSpec::letter(), &recovery_page(&f, n));
    }
    pdf
}

fn protected_doc(enc: Encryption) -> Vec<u8> {
    let mut pdf = Pdf::new();
    let f = StdFonts::new(&mut pdf);
    let mut p = Page::new();
    p.heading(&f.helv_b, 18.0, "Protected Document");
    p.body(
        &f.times,
        "This sentence is only readable after the document has been decrypted.",
    );
    p.body(
        &f.times,
        "A second paragraph proves that every string and stream is handled.",
    );
    p.add(&mut pdf, PageSpec::letter());
    pdf.build(&Output {
        encryption: Some(enc),
        ..Default::default()
    })
    .bytes
}

pub fn encrypted_user_password() -> Vec<u8> {
    protected_doc(Encryption::rc4("secret", "owner-secret"))
}

pub fn encrypted_owner_only() -> Vec<u8> {
    protected_doc(Encryption::rc4("", "owner-secret"))
}

pub fn encrypted_aes128() -> Vec<u8> {
    protected_doc(Encryption::aes128("secret", "owner-secret"))
}

pub fn malformed_bad_xref() -> Vec<u8> {
    three_pages(true)
        .build(&Output {
            bad_xref: true,
            ..Default::default()
        })
        .bytes
}

pub fn malformed_truncated() -> Vec<u8> {
    let mut b = three_pages(false).build(&Output::default()).bytes;
    let needle = b"\nstream\n";
    let start = b
        .windows(needle.len())
        .rposition(|w| w == needle)
        .expect("a stream exists")
        + needle.len();
    let end = b[start..]
        .windows(9)
        .position(|w| w == b"endstream")
        .map_or(b.len(), |p| start + p);
    b.truncate(start + (end - start) / 2);
    b
}

pub fn malformed_garbage_prefix() -> Vec<u8> {
    let mut rng = Rng(0xBAD);
    let mut junk: Vec<u8> = (0..300)
        .map(|_| match rng.below(40) {
            0 => b'\n',
            1 => b' ',
            _ => b'!' + (rng.below(90) as u8),
        })
        .collect();
    junk.retain(|&c| c != b'%');
    while junk.len() < 300 {
        junk.push(b'x');
    }
    let mut out = junk;
    out.extend(three_pages(true).build(&Output::default()).bytes);
    out
}

pub fn malformed_no_eof() -> Vec<u8> {
    three_pages(true)
        .build(&Output {
            eof: false,
            ..Default::default()
        })
        .bytes
}

pub fn not_a_pdf() -> Vec<u8> {
    b"This is a plain text file that was given a .pdf extension.\nIt contains no PDF structure at all.\n".to_vec()
}

pub fn zero_bytes() -> Vec<u8> {
    Vec::new()
}

pub fn corrupt_stream() -> Vec<u8> {
    let mut pdf = Pdf::new();
    let f = StdFonts::new(&mut pdf);
    pdf.add_page(PageSpec::letter(), &recovery_page(&f, 0));
    // Page two: a Flate stream whose data is not valid deflate.
    let mut junk = vec![0x78, 0x9C];
    let mut rng = Rng(99);
    junk.extend((0..200).map(|_| rng.below(256) as u8));
    let st = Stream {
        dict: Dict::new().with("Filter", name("FlateDecode")),
        data: junk,
        flate: false,
    };
    pdf.add_page_stream(PageSpec::letter(), st);
    pdf.add_page(PageSpec::letter(), &recovery_page(&f, 2));
    finish(&mut pdf)
}

pub fn object_streams() -> Vec<u8> {
    let mut pdf = Pdf::new();
    let f = StdFonts::new(&mut pdf);
    let mut p = Page::new();
    p.heading(&f.helv_b, 18.0, "Compact structure");
    p.body(
        &f.times,
        "This document uses a cross-reference stream and object streams, as produced by modern writers.",
    );
    let link = pdf.add_link([72.0, 600.0, 200.0, 612.0], "https://example.com/compact");
    p.c.text(&f.times, 11.0, 72.0, 602.0, "Compact link target");
    p.add(&mut pdf, PageSpec::letter().with_annots(vec![link]));
    let mut p2 = Page::new();
    p2.heading(&f.helv_b, 18.0, "Second compact page");
    p2.body(
        &f.times,
        "The second page is stored in the same object stream.",
    );
    p2.add(&mut pdf, PageSpec::letter());
    pdf.build(&Output::compact()).bytes
}

pub fn incremental_update() -> Vec<u8> {
    let mut pdf = Pdf::new();
    let f = StdFonts::new(&mut pdf);
    let mut p1 = Page::new();
    p1.heading(&f.helv_b, 18.0, "Contract draft");
    p1.body(
        &f.times,
        "Original first page text that must be replaced by the later revision.",
    );
    let ids = p1.add(&mut pdf, PageSpec::letter());
    let mut p2 = Page::new();
    p2.body(&f.times, "The second page stays untouched by the update.");
    p2.add(&mut pdf, PageSpec::letter());
    let built = pdf.build(&Output::default());

    let mut p1b = Page::new();
    p1b.heading(&f.helv_b, 18.0, "Contract draft");
    p1b.body(
        &f.times,
        "Revised first page text that replaces the original wording.",
    );
    pdf.replace_content(ids.contents, &p1b.c);
    pdf.append_update(&built, &[ids.contents])
}
