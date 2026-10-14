// Prevents an extra console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// pdf_oxide allocates heavily; mimalloc made conversion 1.6x faster
// single-threaded and lets page extraction scale across threads, where the
// system heap serialised it (see docs/performance.md).
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() {
    pdf2md_lib::run()
}
