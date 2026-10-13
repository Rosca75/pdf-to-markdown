//! Verifies that the generated corpus parses with pdf_oxide and that the text we
//! embedded is what pdf_oxide extracts (proves the fixtures encode what we intend).

use pdf_fixtures::corpus::cases;
use pdf_oxide::PdfDocument;
use std::collections::HashMap;
use std::panic::{catch_unwind, AssertUnwindSafe};

fn norm(s: &str) -> String {
    let s = s
        .replace('\u{FB00}', "ff")
        .replace('\u{FB01}', "fi")
        .replace('\u{FB02}', "fl")
        .replace('\u{FB03}', "ffi")
        .replace('\u{FB04}', "ffl")
        .replace('\u{A0}', " ");
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn corpus() -> HashMap<&'static str, Vec<u8>> {
    cases().into_iter().collect()
}

/// Page text rebuilt from spans: lines are clustered by y, ordered by x; only spans
/// whose left edge lies in `xr` are used.
fn page_text(doc: &PdfDocument, page: usize, xr: (f32, f32)) -> String {
    let mut spans = doc.extract_spans(page).unwrap_or_default();
    spans.retain(|s| s.bbox.x >= xr.0 && s.bbox.x < xr.1);
    spans.sort_by(|a, b| b.bbox.y.partial_cmp(&a.bbox.y).unwrap());
    type Piece = (f32, f32, String);
    let mut lines: Vec<(f32, Vec<Piece>)> = Vec::new();
    for s in spans {
        let item = (s.bbox.x, s.bbox.x + s.bbox.width, s.text.clone());
        match lines.last_mut() {
            Some((y, v)) if (*y - s.bbox.y).abs() < 4.0 => v.push(item),
            _ => lines.push((s.bbox.y, vec![item])),
        }
    }
    let mut out = String::new();
    for (_, mut v) in lines {
        v.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        let mut right = f32::MIN;
        for (l, r, t) in v {
            if right != f32::MIN && l - right > 1.5 {
                out.push(' ');
            }
            out.push_str(&t);
            right = r;
        }
        out.push('\n');
    }
    out
}

fn full_text_in(doc: &PdfDocument, xr: (f32, f32)) -> String {
    let n = doc.page_count().expect("page count");
    let mut out = String::new();
    for i in 0..n {
        out.push_str(&page_text(doc, i, xr));
        out.push('\n');
    }
    norm(&out)
}

fn full_text(doc: &PdfDocument) -> String {
    full_text_in(doc, (f32::MIN, f32::MAX))
}

fn check(file: &str, password: Option<&[u8]>, keys: &[&str]) {
    let bytes = corpus()
        .remove(file)
        .unwrap_or_else(|| panic!("no case {file}"));
    let doc = PdfDocument::from_bytes(bytes).unwrap_or_else(|e| panic!("{file}: open failed: {e}"));
    if let Some(pw) = password {
        assert!(
            doc.authenticate(pw).unwrap_or(false),
            "{file}: authenticate"
        );
    }
    let text = full_text(&doc);
    let missing: Vec<&&str> = keys.iter().filter(|k| !text.contains(&norm(k))).collect();
    assert!(
        missing.is_empty(),
        "{file}: missing {missing:?}\n--- extracted ---\n{text}"
    );
}

#[test]
fn key_strings_are_extracted() {
    let table: &[(&str, &[&str])] = &[
        (
            "basic_structure.pdf",
            &[
                "Annual Report",
                "Market overview",
                "Regional results",
                "This report summarises the financial performance of the company during the last fiscal year and outlines the priorities for the coming twelve months.",
                "Europe delivered the highest revenue growth of all regions, while North America contributed the largest absolute profit.",
            ],
        ),
        (
            "lists.pdf",
            &[
                "Prepare the budget forecast for the next quarter, including all regional adjustments and the revised headcount plan.",
                "Check projector availability and bring the spare cable for the presentation.",
                "dash item two with a long continuation that definitely needs to wrap onto a second line of the page",
                "Open the settings dialog and choose the advanced tab where the export options are located.",
                "Second lettered sub-step",
            ],
        ),
        (
            "emphasis.pdf",
            &[
                "This sentence has bold, italic and bold italic words inside it.",
                "Important: read the terms before continuing.",
                "A very important notice, not so much.",
            ],
        ),
        (
            "table_ruled.pdf",
            &["Gadget | Pro", "Ships separately", "Out of stock", "Prices exclude VAT"],
        ),
        ("table_unruled.pdf", &["Region", "North", "143", "All figures are in thousands of euros."]),
        (
            "links.pdf",
            &[
                "Read the documentation for all the details.",
                "Visit our website today.",
                "consult the setup guide on the support portal before you start",
                "https://example.net/page",
            ],
        ),
        (
            "headers_footers.pdf",
            &[
                "ACME Corp Quarterly Briefing",
                "Confidential",
                "Page 3 of 6",
                "Page content marker alpha is unique to this page.",
                "Page content marker foxtrot is unique to this page.",
            ],
        ),
        (
            "margin_jumps.pdf",
            &[
                "Margin marker one opens the first paragraph of this page",
                "Margin marker three opens",
                "Margin marker four opens",
                "Margin marker five opens",
                "Margin marker six opens",
                "A second paragraph on page six confirms that the reading order is preserved.",
            ],
        ),
        (
            "split_across_pages.pdf",
            &[
                "therefore decided to",
                "extend the review period by two quarters before signing anything.",
                "its packaging meets the inter-",
                "national standards required by the customer.",
                "improves the manage- ment of the portfolio",
                "was completed on schedule.",
                "Training for the warehouse staff will begin",
            ],
        ),
        (
            "mixed_graphics.pdf",
            &[
                "Quarterly diagram",
                "The second paragraph sits between the shapes and the image and keeps its sentence together.",
                "Figure 1 form caption",
                "Input",
                "Process",
                "Output",
                "The closing paragraph after the diagram ends the page.",
            ],
        ),
        (
            "encodings.pdf",
            &[
                "Café résumé naïve Zoë Müller Ångström costs €25 — “quoted” and ‘single’ with en–dash and ellipsis…",
                "Differences: αβγ Жж € and A—B ok",
                "Ελληνικά",
                "Привет мир",
                "日本語のテキスト",
                "中文测试",
                "Mixed Latin and ΑΒΓ in one font.",
            ],
        ),
        (
            "ligatures.pdf",
            &[
                "The final flow is efficient",
                "The office opens at nine",
                "Second: final flow of efficient staff",
            ],
        ),
        (
            "markdown_specials.pdf",
            &[
                "Use * and _ carefully: 2 * 3 = 6 and snake_case_name should survive.",
                "# Not a heading",
                "1984. That year was long ago",
            ],
        ),
        ("blank_pages.pdf", &["First page text.", "Fourth page text after blank pages."]),
        (
            "ocr_invisible_text.pdf",
            &[
                "Invoice number 20481 issued to Contoso Ltd.",
                "Payment is due within thirty days of receipt.",
            ],
        ),
        (
            "tj_kerning.pdf",
            &["Hello world from kerning arrays.", "WARNING do not touch", "Glyph by glyph text"],
        ),
        (
            "object_streams.pdf",
            &[
                "Compact structure",
                "This document uses a cross-reference stream and object streams, as produced by modern writers.",
                "Second compact page",
            ],
        ),
        (
            "incremental_update.pdf",
            &["Revised first page text that replaces the original wording.", "The second page stays untouched by the update."],
        ),
        (
            "code_block.pdf",
            &["fn main() {", "let x = 42;", "println!(\"{}\", x);", "After the listing the normal text continues here."],
        ),
        ("fake_bold_overprint.pdf", &["Fake Bold Heading", "This sentence is printed normally, once."]),
        (
            "title_page.pdf",
            &["Market Study 2026", "Prepared by Jane Doe", "Executive summary", "The mid-size segment is expected"],
        ),
        (
            "malformed_bad_xref.pdf",
            &["Recovery marker one stays readable", "Recovery marker three stays readable"],
        ),
        (
            "malformed_garbage_prefix.pdf",
            &["Recovery marker one stays readable", "Recovery marker three stays readable"],
        ),
        (
            "malformed_no_eof.pdf",
            &["Recovery marker one stays readable", "Recovery marker three stays readable"],
        ),
    ];
    for (file, keys) in table {
        check(file, None, keys);
    }
}

#[test]
fn encrypted_files_need_authentication() {
    let msg = "This sentence is only readable after the document has been decrypted.";
    for file in ["encrypted_user_password.pdf", "encrypted_aes128.pdf"] {
        let bytes = corpus().remove(file).unwrap();
        let doc = PdfDocument::from_bytes(bytes).unwrap();
        assert!(doc.is_encrypted(), "{file} should be encrypted");
        assert!(
            !doc.authenticate(b"wrong").unwrap_or(false),
            "{file}: wrong password accepted"
        );
        check(file, Some(b"secret"), &[msg, "Protected Document"]);
    }
    // Empty user password: opens without authenticating.
    let doc =
        PdfDocument::from_bytes(corpus().remove("encrypted_owner_only.pdf").unwrap()).unwrap();
    assert!(doc.is_encrypted());
    assert!(doc.authenticate(b"").unwrap_or(false));
    assert!(full_text(&doc).contains(msg));
}

#[test]
fn scanned_page_has_no_text() {
    let doc = PdfDocument::from_bytes(corpus().remove("scanned_image_only.pdf").unwrap()).unwrap();
    assert_eq!(doc.page_count().unwrap(), 1);
    assert!(doc.extract_text(0).unwrap_or_default().trim().is_empty());
}

#[test]
fn page_counts() {
    let expect: &[(&str, usize)] = &[
        ("headers_footers.pdf", 6),
        ("margin_jumps.pdf", 6),
        ("split_across_pages.pdf", 4),
        ("blank_pages.pdf", 5),
        ("corrupt_stream.pdf", 3),
        ("malformed_bad_xref.pdf", 3),
        ("object_streams.pdf", 2),
        ("incremental_update.pdf", 2),
        ("title_page.pdf", 2),
    ];
    for (file, n) in expect {
        let doc = PdfDocument::from_bytes(corpus().remove(file).unwrap()).unwrap();
        assert_eq!(doc.page_count().unwrap(), *n, "{file}");
    }
}

#[test]
fn corrupt_stream_keeps_good_pages() {
    let doc = PdfDocument::from_bytes(corpus().remove("corrupt_stream.pdf").unwrap()).unwrap();
    let p1 = norm(&doc.extract_text(0).unwrap());
    let p3 = norm(&doc.extract_text(2).unwrap());
    assert!(p1.contains("Recovery marker one"));
    assert!(p3.contains("Recovery marker three"));
    // Page two must not panic; an error or empty text are both acceptable.
    let r = catch_unwind(AssertUnwindSafe(|| doc.extract_text(1)));
    assert!(r.is_ok(), "extract_text panicked on the corrupt page");
}

#[test]
fn incremental_update_wins() {
    let doc = PdfDocument::from_bytes(corpus().remove("incremental_update.pdf").unwrap()).unwrap();
    let t = full_text(&doc);
    assert!(!t.contains("Original first page text"), "{t}");
}

#[test]
fn malformed_files_do_not_panic() {
    for file in [
        "malformed_truncated.pdf",
        "malformed_bad_xref.pdf",
        "malformed_garbage_prefix.pdf",
        "malformed_no_eof.pdf",
        "not_a_pdf.pdf",
        "zero_bytes.pdf",
    ] {
        let bytes = corpus().remove(file).unwrap();
        let r = catch_unwind(AssertUnwindSafe(|| {
            if let Ok(doc) = PdfDocument::from_bytes(bytes) {
                let n = doc.page_count().unwrap_or(0);
                let mut text = String::new();
                for i in 0..n {
                    text.push_str(&doc.extract_text(i).unwrap_or_default());
                }
                println!("{file}: opened, {n} pages, {} chars", text.len());
            } else {
                println!("{file}: open failed");
            }
        }));
        assert!(r.is_ok(), "{file}: panicked");
    }
}

#[test]
fn truncated_file_recovers_earlier_pages() {
    let bytes = corpus().remove("malformed_truncated.pdf").unwrap();
    // Document what pdf_oxide achieves; the engine's expectation lives in the manifest.
    let Ok(doc) = PdfDocument::from_bytes(bytes) else {
        println!("truncated: pdf_oxide cannot open the file");
        return;
    };
    let text = full_text(&doc);
    println!("truncated: {:?} pages, text: {text}", doc.page_count());
    assert!(text.contains("Recovery marker one"), "{text}");
}

#[test]
fn corpus_files_match_manifest() {
    let manifest = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/corpus/manifest.toml"
    ))
    .expect("manifest.toml");
    let listed: std::collections::BTreeSet<&str> = manifest
        .lines()
        .filter_map(|l| l.strip_prefix("file = \""))
        .filter_map(|l| l.strip_suffix('"'))
        .collect();
    let produced = corpus();
    for f in &listed {
        assert!(produced.contains_key(f), "manifest lists unknown file {f}");
    }
    for f in produced.keys() {
        assert!(listed.contains(f), "corpus file {f} missing from manifest");
    }
}

#[test]
fn corpus_is_deterministic() {
    let a = cases();
    let b = cases();
    assert_eq!(a, b);
}

#[test]
fn two_columns_split_by_x() {
    let doc = PdfDocument::from_bytes(corpus().remove("two_columns.pdf").unwrap()).unwrap();
    let left = full_text_in(&doc, (0.0, 300.0));
    let right = full_text_in(&doc, (300.0, 700.0));
    for k in [
        "The left column opens with news about the new office in Lisbon, which will host forty employees from March.",
        "A second item on the left reports that the canteen now serves vegetarian meals every day.",
    ] {
        assert!(left.contains(&norm(k)), "left column missing {k}
{left}");
    }
    for k in [
        "The right column starts with the annual picnic, scheduled for the first Saturday of June.",
        "Finally the right column thanks all volunteers who helped with the charity run.",
    ] {
        assert!(
            right.contains(&norm(k)),
            "right column missing {k}
{right}"
        );
    }
    assert!(full_text(&doc).contains("Quarterly Newsletter"));
}

#[test]
fn rotated_stamp_is_separate_from_body() {
    let doc = PdfDocument::from_bytes(corpus().remove("rotated_margin_text.pdf").unwrap()).unwrap();
    let body = full_text_in(&doc, (90.0, 700.0));
    for k in [
        "The first body paragraph explains the purpose of the document in plain words and stays intact despite the stamp in the margin next to it.",
        "The second body paragraph describes the current state of the project: all milestones were reached and the remaining budget is sufficient.",
        "The third body paragraph lists the next steps and thanks everyone who contributed to the memo.",
    ] {
        assert!(body.contains(&norm(k)), "body missing {k}
{body}");
    }
    let stamp = full_text_in(&doc, (0.0, 90.0));
    println!("rotated stamp as seen by span extraction: {stamp:?}");
    println!("plain extract_text: {:?}", doc.extract_text(0));
}
