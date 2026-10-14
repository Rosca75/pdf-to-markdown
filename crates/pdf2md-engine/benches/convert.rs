//! `cargo bench -p pdf2md-engine` — conversion throughput on synthetic
//! documents, single- and multi-threaded.

use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use pdf2md_engine::{convert_bytes, CancelToken, Options};

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn bench(c: &mut Criterion) {
    let docs = [
        (
            "text_report_100",
            pdf_fixtures::large::text_report(100),
            100u64,
        ),
        (
            "mixed_report_50",
            pdf_fixtures::large::mixed_report(50),
            50u64,
        ),
    ];
    let mut group = c.benchmark_group("convert");
    group.sample_size(10);
    for (name, pdf, pages) in &docs {
        group.throughput(Throughput::Elements(*pages));
        for threads in [1usize, 2, 4] {
            let opts = Options {
                password: None,
                threads: Some(threads),
            };
            group.bench_function(format!("{name}/threads_{threads}"), |b| {
                b.iter(|| {
                    convert_bytes(pdf.clone(), &opts, &|_| {}, &CancelToken::new())
                        .expect("conversion")
                })
            });
        }
    }
    group.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
