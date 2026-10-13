use pdf_fixtures::large;
use pdf_oxide::PdfDocument;
use std::time::Instant;

#[test]
fn text_report_500_pages_is_fast_and_opens() {
    let t = Instant::now();
    let bytes = large::text_report(500);
    let elapsed = t.elapsed();
    println!("text_report(500): {} bytes in {elapsed:?}", bytes.len());
    assert!(elapsed.as_secs_f64() < 10.0, "too slow: {elapsed:?}");
    let doc = PdfDocument::from_bytes(bytes).expect("open");
    assert_eq!(doc.page_count().unwrap(), 500);
    let p = doc.extract_text(41).unwrap();
    assert!(p.contains("Operations Report"));
    assert!(p.contains("Page 42 of 500"));
    let words = p.split_whitespace().count();
    println!("page 42 words: {words}");
    assert!((350..=700).contains(&words), "words per page {words}");
}

#[test]
fn text_report_is_deterministic() {
    assert_eq!(large::text_report(12), large::text_report(12));
}

#[test]
fn mixed_report_opens() {
    let t = Instant::now();
    let bytes = large::mixed_report(100);
    println!(
        "mixed_report(100): {} bytes in {:?}",
        bytes.len(),
        t.elapsed()
    );
    let doc = PdfDocument::from_bytes(bytes).expect("open");
    assert_eq!(doc.page_count().unwrap(), 100);
    let two_col = doc.extract_text(2).unwrap();
    assert!(two_col.contains("Page 3 of 100"));
    let all: String = (0..20).map(|i| doc.extract_text(i).unwrap()).collect();
    assert!(
        all.contains("Figure 1"),
        "no figure caption in the first 20 pages"
    );
}
