//! Performance guards backing the targets in docs/performance.md.
//!
//! Floors are set well below the measured numbers (about 5x headroom) so the
//! tests catch regressions of the order of a lost optimisation, not machine
//! noise. They only run in optimised builds; CI runs the suite with
//! `--profile ci`.

use std::alloc::{GlobalAlloc, Layout};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use mimalloc::MiMalloc;
use pdf2md_engine::{convert_bytes, CancelToken, Options};

struct Counting;
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = unsafe { MiMalloc.alloc(l) };
        if !p.is_null() {
            let now = LIVE.fetch_add(l.size(), Ordering::Relaxed) + l.size();
            PEAK.fetch_max(now, Ordering::Relaxed);
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { MiMalloc.dealloc(p, l) };
        LIVE.fetch_sub(l.size(), Ordering::Relaxed);
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

const OPTIMISED: bool = !cfg!(debug_assertions);

#[test]
fn throughput_floor_on_text_report() {
    if !OPTIMISED {
        eprintln!("skipped: performance guards need an optimised build (--release)");
        return;
    }
    let pdf = pdf_fixtures::large::text_report(300);
    let opts = Options {
        password: None,
        threads: Some(2),
    };
    let mut best = f64::MAX;
    for _ in 0..3 {
        let t = Instant::now();
        convert_bytes(pdf.clone(), &opts, &|_| {}, &CancelToken::new()).expect("conversion");
        best = best.min(t.elapsed().as_secs_f64());
    }
    let pages_per_s = 300.0 / best;
    assert!(
        pages_per_s >= 100.0,
        "throughput regressed: {pages_per_s:.0} pages/s (floor 100)"
    );
}

#[test]
fn peak_heap_ceiling_on_text_report() {
    if !OPTIMISED {
        return;
    }
    let pdf = pdf_fixtures::large::text_report(500);
    let opts = Options {
        password: None,
        threads: Some(2),
    };
    let base = LIVE.load(Ordering::Relaxed);
    PEAK.store(base, Ordering::Relaxed);
    let conv = convert_bytes(pdf, &opts, &|_| {}, &CancelToken::new()).expect("conversion");
    let peak = PEAK.load(Ordering::Relaxed).saturating_sub(base);
    drop(conv);
    // Measured about 31 MB; the target is 64 MB.
    assert!(
        peak <= 64 * 1024 * 1024,
        "peak heap regressed: {:.1} MB (ceiling 64 MB)",
        peak as f64 / 1e6
    );
}
