# Performance

PDF2MD targets standard business laptops, so performance was treated as a design constraint. This document records the baseline, the targets, the candidate techniques, what was implemented and the measured results.

## Method

- **Reference machine:** Intel Core i7-6600U, 2 cores / 4 threads, 15 W, 8 GB RAM, Windows 10. This is an older and slower part than current business laptops, so the numbers are conservative.
- **Harness:** `cargo run --release -p pdf2md-engine --example bench -- [--threads N] [--runs N] [--no-mem] [file.pdf ...]`. It reports the best of N runs, in pages/s, and the peak live heap, measured with a counting global allocator. `--no-mem` disables counting for timing runs: its two atomic operations per allocation contend across threads.
- **Documents:**
  - **Spec:** the ISO 32000-1 (PDF 1.7) specification as published by Adobe. 756 pages, 22.5 MB, embedded fonts, about 600 tables, figures. Freely downloadable, so it is not committed.
  - **report_500:** `pdf_fixtures::large::text_report(500)`, a synthetic text report with running headers, headings, lists and ruled tables.
  - **mixed_100:** `pdf_fixtures::large::mixed_report(100)`, synthetic, with images, vector diagrams and two-column pages.
- **Stage timing:** set `PDF2MD_TIMING=1` to get the split between extraction (pdf_oxide parsing plus line building) and layout.
- **Thermal variance:** the 15 W reference CPU throttles under sustained load. After hours of compiling, the same binary measured up to 2× slower on long runs, while short runs were unaffected. Comparisons below were taken back to back in one session; the final table was measured after a cool-down.

## Baseline (single thread, system allocator)

| Document | Pages | Time | Pages/s | Peak heap | Extraction share |
|---|---|---|---|---|---|
| Spec | 756 | 6.95 s | 109 | 69 MB | 96 % |
| report_500 | 500 | 1.67 s | 300 | 14 MB | 92 % |
| mixed_100 | 100 | 0.33 s | 303 | 8 MB | 91 % |

Extraction is pdf_oxide content-stream parsing plus line building. It dominates, so layout micro-optimisations could never have moved the totals by more than a few percent.

## Targets

| Target | Value |
|---|---|
| T1: real text-heavy documents | ≥ 250 pages/s (the spec in ≤ 3.0 s) |
| T2: synthetic report | ≥ 600 pages/s |
| T3: peak heap for a 22 MB / 756-page document | ≤ 160 MB |
| T4: peak heap for the 500-page synthetic report | ≤ 64 MB |
| T5: layout share of total time | ≤ 10 % |
| T6: first progress event | immediately after the document opens, before page 1 |

## Candidate techniques

| # | Technique | Expected effect | Decision |
|---|---|---|---|
| 1 | Parallel page extraction | Extraction is about 95 % of time and pages are independent | **Done.** Scoped worker threads, each with its own `PdfDocument` (pdf_oxide's handle is `!Send`), pages handed out by an atomic counter, results merged in page order. |
| 2 | Faster global allocator (mimalloc) | pdf_oxide is allocation-heavy; the Windows heap serialises multi-threaded churn | **Done.** 1.6× single-threaded, and it is what made parallel scaling work. |
| 3 | Memory-aware worker count | Each worker holds a full document handle (about 3.3× the file size) | **Done.** Workers capped by a 192 MiB budget at 3.5× file size + 8 MiB each, and by ≥ 8 pages per worker. |
| 4 | Lazy ruling-line extraction | `extract_paths` re-parses each content stream | **Done.** Paths are fetched only for pages with ≥ 2 multi-fragment rows. Measured gain was negligible: path extraction was cheap relative to text. |
| 5 | Line building inside the workers | Parallelises the only heavy layout step for free | **Done.** |
| 6 | Allocation reduction in layout (cached line text, no `format!` in hot paths) | Layout is ≤ 5 % of total | Partly done (marker probe without `format!`). The rest was not worth the complexity at 5 % share. |
| 7 | Parallel layout (tables, order, segmentation per page) | At most 3–5 % of total | Rejected: below measurement noise. |
| 8 | Streaming / memory-mapped input | Avoid the extra copies of the file bytes | Rejected: pdf_oxide's `from_bytes` copies its input internally, so there is no zero-copy path in this version. |
| 9 | Larger worker stacks | Robustness, not speed: hostile PDFs can nest deeply, and a stack overflow aborts the process | **Done.** The whole conversion runs on a 16 MiB-stack thread, workers get the same stack, and panics are mapped to errors. |

## Results

Same binary, back to back, after cool-down. "auto" is the shipped default: logical cores, capped by the worker policy.

| Configuration | Spec pages/s | report_500 pages/s | mixed_100 pages/s |
|---|---|---|---|
| Baseline: 1 thread, system heap | 109 | 300 | 303 |
| 1 thread, mimalloc | 150 (181 when cool) | 403 (529 when cool) | 449 |
| 2 threads, mimalloc | 201 (259 when cool) | 573 (695 when cool) | 588 |
| **auto (shipped)** | **197** (2 workers: memory cap) | **739** (4 workers) | **706** |

| Peak heap at auto | Spec | report_500 | mixed_100 |
|---|---|---|---|
| Baseline (1 thread) | 69 MB | 14 MB | 8 MB |
| auto (shipped) | **151 MB** | **54 MB** | 30 MB |

Layout share at auto: 4 % of total on the spec (154 ms of 3.8 s).

### Target status

| Target | Status |
|---|---|
| T1: ≥ 250 pages/s on real text-heavy documents | **Met when the CPU is not throttled** (259 pages/s, 2.9 s for 756 pages). Under sustained thermal throttling on the 15 W reference part, 197–201 pages/s. That is a 1.8× speed-up over baseline either way. The spec is held to 2 workers by the memory cap, and this CPU has only 2 physical cores (hyper-threads gave no gain). |
| T2: ≥ 600 pages/s on the synthetic report | **Met**: 739 pages/s, 2.5× baseline. |
| T3: ≤ 160 MB peak for the 22 MB spec | **Met**: 151 MB. |
| T4: ≤ 64 MB peak for report_500 | **Met**: 54 MB. |
| T5: layout ≤ 10 % of time | **Met**: about 4 %. |
| T6: first progress event before page 1 | **Met**: `Progress { done: 0, total }` is sent right after the document opens. |

### What did not help

- **Hyper-threads.** Going from 2 to 4 workers on the 2-core reference CPU gave no gain and added about 73 MB per extra worker on the spec. On newer laptops with 4+ physical cores, the same policy gives more workers to documents that fit the memory budget.
- **Lazy ruling lines.** No measurable change. pdf_oxide's path extraction is cheap next to its text extraction.
- **Bypassing pdf_oxide's per-page span cache.** `extract_spans` deep-copies every page's spans into a bounded 8-page cache. The public way around it (`extract_spans_with_config`) also skips pdf_oxide's span post-processing (ActualText, off-page filtering, decode fixes). An estimated 5–10 % was not worth a semantic change.

### Remaining headroom

Extraction is now about 96 % pdf_oxide parsing. Further gains would need pdf_oxide changes upstream: an uncached span API, and a shared handle across threads instead of one parse per worker.

## Guards

- `crates/pdf2md-engine/tests/performance.rs`, optimised builds only (CI runs tests with `--profile ci`):
  - a throughput floor of ≥ 100 pages/s on a 300-page report;
  - a peak-heap ceiling of ≤ 64 MB on report_500.

  The floors sit about 5× below the measured values, so they catch a lost optimisation, not noise.
- `crates/pdf2md-engine/tests/parallel.rs`: multi-threaded output is byte-identical to single-threaded output; progress is monotonic; cancellation stops a run.
- `cargo bench -p pdf2md-engine` (Criterion): conversion throughput at 1, 2 and 4 threads.
