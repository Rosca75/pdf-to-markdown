//! Throughput and peak-memory benchmark.
//!
//! ```text
//! cargo run --release -p pdf2md-engine --example bench -- [--threads N] [--runs N] [FILE.pdf ...]
//! ```
//!
//! Without files it benchmarks the synthetic documents from `pdf-fixtures`
//! (a 500-page text report and a 100-page mixed report). Peak memory is the
//! high-water mark of live heap bytes, tracked by a counting allocator.

use std::alloc::{GlobalAlloc, Layout};

use mimalloc::MiMalloc as System;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Instant;

use pdf2md_engine::{convert_bytes, CancelToken, Options};

struct Counting;

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
/// Counting costs two contended atomics per allocation, which distorts
/// multi-threaded timings; `--no-mem` turns it off for speed runs.
static COUNT: AtomicBool = AtomicBool::new(true);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(layout) };
        if !p.is_null() && COUNT.load(Ordering::Relaxed) {
            let now = LIVE.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
            PEAK.fetch_max(now, Ordering::Relaxed);
        }
        p
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) };
        if COUNT.load(Ordering::Relaxed) {
            LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let p = unsafe { System.realloc(ptr, layout, new_size) };
        if !p.is_null() && COUNT.load(Ordering::Relaxed) {
            if new_size > layout.size() {
                let now = LIVE.fetch_add(new_size - layout.size(), Ordering::Relaxed) + new_size
                    - layout.size();
                PEAK.fetch_max(now, Ordering::Relaxed);
            } else {
                LIVE.fetch_sub(layout.size() - new_size, Ordering::Relaxed);
            }
        }
        p
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

fn main() {
    let mut threads = None;
    let mut runs = 3usize;
    let mut files = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--threads" => threads = args.next().and_then(|v| v.parse().ok()),
            "--runs" => runs = args.next().and_then(|v| v.parse().ok()).unwrap_or(3),
            // Only switch before anything is allocated-and-counted matters:
            // live-byte accounting is meaningless once disabled.
            "--no-mem" => COUNT.store(false, Ordering::Relaxed),
            f => files.push(f.to_owned()),
        }
    }

    let docs: Vec<(String, Vec<u8>)> = if files.is_empty() {
        vec![
            (
                "report_500 (synthetic text)".into(),
                pdf_fixtures::large::text_report(500),
            ),
            (
                "mixed_100 (synthetic mixed)".into(),
                pdf_fixtures::large::mixed_report(100),
            ),
        ]
    } else {
        files
            .iter()
            .map(|f| (f.clone(), std::fs::read(f).expect("read input")))
            .collect()
    };

    let opts = Options {
        password: None,
        threads,
    };
    println!(
        "{:<30} {:>6} {:>9} {:>10} {:>11} {:>10}",
        "document", "pages", "MB", "best ms", "pages/s", "peak MB"
    );
    for (name, bytes) in docs {
        let mb = bytes.len() as f64 / 1e6;
        let mut best = f64::MAX;
        let mut pages = 0;
        let mut peak = 0usize;
        for _ in 0..runs {
            let input = bytes.clone();
            let base = LIVE.load(Ordering::Relaxed);
            PEAK.store(base, Ordering::Relaxed);
            let t = Instant::now();
            let conv =
                convert_bytes(input, &opts, &|_| {}, &CancelToken::new()).expect("conversion");
            let ms = t.elapsed().as_secs_f64() * 1e3;
            peak = peak.max(PEAK.load(Ordering::Relaxed) - base);
            pages = conv.pages;
            best = best.min(ms);
            drop(conv);
        }
        println!(
            "{:<30} {:>6} {:>9.1} {:>10.1} {:>11.1} {:>10.1}",
            name,
            pages,
            mb,
            best,
            pages as f64 / (best / 1e3),
            peak as f64 / 1e6
        );
    }
}
