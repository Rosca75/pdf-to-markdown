//! Writes every corpus case to `tests/corpus/` at the repository root.
//!
//! `gen-corpus --large` additionally writes the benchmark documents
//! (`report_500.pdf`, `mixed_100.pdf`) to the git-ignored `bench-data/` directory
//! and prints how long generating them took.

use std::path::PathBuf;
use std::time::Instant;

fn main() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = root.join("tests/corpus");
    std::fs::create_dir_all(&dir).expect("create tests/corpus");
    let cases = pdf_fixtures::corpus::cases();
    for (name, bytes) in &cases {
        std::fs::write(dir.join(name), bytes).unwrap_or_else(|e| panic!("write {name}: {e}"));
    }
    println!("wrote {} files to {}", cases.len(), dir.display());

    if std::env::args().any(|a| a == "--large") {
        let bench = root.join("bench-data");
        std::fs::create_dir_all(&bench).expect("create bench-data");
        let jobs: [(&str, &dyn Fn() -> Vec<u8>); 2] = [
            ("report_500.pdf", &|| pdf_fixtures::large::text_report(500)),
            ("mixed_100.pdf", &|| pdf_fixtures::large::mixed_report(100)),
        ];
        for (name, job) in jobs {
            let t = Instant::now();
            let bytes = job();
            println!(
                "{name}: {} bytes generated in {:?}",
                bytes.len(),
                t.elapsed()
            );
            std::fs::write(bench.join(name), bytes).unwrap_or_else(|e| panic!("write {name}: {e}"));
        }
    }
}
