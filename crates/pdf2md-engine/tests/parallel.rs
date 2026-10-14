//! Parallel extraction must not change the output.

use pdf2md_engine::{convert_bytes, CancelToken, Options, Progress};
use std::sync::Mutex;

fn convert(bytes: &[u8], threads: usize) -> String {
    let opts = Options {
        password: None,
        threads: Some(threads),
    };
    convert_bytes(bytes.to_vec(), &opts, &|_| {}, &CancelToken::new())
        .expect("conversion")
        .markdown
}

#[test]
fn multi_threaded_output_is_identical_to_single_threaded() {
    let pdf = pdf_fixtures::large::mixed_report(48);
    let single = convert(&pdf, 1);
    for threads in [2, 3, 4] {
        assert!(
            convert(&pdf, threads) == single,
            "output differs with {threads} threads"
        );
    }
}

#[test]
fn progress_reaches_total_monotonically() {
    let pdf = pdf_fixtures::large::text_report(40);
    let seen = Mutex::new(Vec::new());
    let opts = Options {
        password: None,
        threads: Some(4),
    };
    convert_bytes(
        pdf,
        &opts,
        &|p: Progress| seen.lock().unwrap().push(p),
        &CancelToken::new(),
    )
    .expect("conversion");
    let seen = seen.into_inner().unwrap();
    assert_eq!(seen.first().map(|p| p.done), Some(0));
    assert_eq!(seen.last().map(|p| (p.done, p.total)), Some((40, 40)));
    assert!(seen.windows(2).all(|w| w[0].done <= w[1].done));
}

#[test]
fn cancellation_stops_the_run() {
    let pdf = pdf_fixtures::large::text_report(200);
    let cancel = CancelToken::new();
    let opts = Options {
        password: None,
        threads: Some(2),
    };
    let c = cancel.clone();
    let r = convert_bytes(
        pdf,
        &opts,
        &move |p: Progress| {
            if p.done >= 5 {
                c.cancel()
            }
        },
        &cancel,
    );
    assert_eq!(r.err().map(|e| e.code()), Some("cancelled"));
}
