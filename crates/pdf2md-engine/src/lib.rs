//! PDF to Markdown conversion engine.
//!
//! The public surface is deliberately small: [`convert_path`] / [`convert_bytes`]
//! turn a PDF into a [`Conversion`] or a typed [`ConvertError`].

mod classify;
mod error;
mod extract;
mod flow;
mod furniture;
mod lines;
mod links;
mod markers;
mod model;
mod normalize;
mod order;
mod pipeline;
mod render;
mod segment;
mod stats;
mod tables;

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub use error::ConvertError;

/// Caller-tunable conversion settings.
#[derive(Debug, Clone, Default)]
pub struct Options {
    /// Password for encrypted documents. `None` tries the empty user password.
    pub password: Option<String>,
    /// Worker threads for page extraction. `None` uses all logical cores.
    pub threads: Option<usize>,
}

/// Progress report: `done` of `total` pages processed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    pub done: usize,
    pub total: usize,
}

/// Cooperative cancellation flag shared between the caller and the engine.
#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

/// A non-fatal problem met during conversion.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Warning {
    /// Zero-based page index, when the problem is tied to one page.
    pub page: Option<usize>,
    pub message: String,
}

/// A successful conversion.
#[derive(Debug, Clone)]
pub struct Conversion {
    pub markdown: String,
    pub pages: usize,
    pub warnings: Vec<Warning>,
}

/// Convert the PDF at `path`.
pub fn convert_path(
    path: &Path,
    opts: &Options,
    progress: &(dyn Fn(Progress) + Sync),
    cancel: &CancelToken,
) -> Result<Conversion, ConvertError> {
    let bytes = std::fs::read(path)?;
    convert_bytes(bytes, opts, progress, cancel)
}

/// Stack for the conversion thread and its workers. Hostile PDFs can nest
/// objects and content deeply; a stack overflow aborts the whole process
/// and cannot be caught, so every thread that parses gets a generous stack.
pub(crate) const PARSE_STACK: usize = 16 * 1024 * 1024;

/// Convert an in-memory PDF.
///
/// Runs on a dedicated thread with a large stack (see [`PARSE_STACK`]);
/// a panic anywhere in the pipeline is reported as [`ConvertError::Corrupt`]
/// instead of unwinding into the caller.
pub fn convert_bytes(
    bytes: Vec<u8>,
    opts: &Options,
    progress: &(dyn Fn(Progress) + Sync),
    cancel: &CancelToken,
) -> Result<Conversion, ConvertError> {
    std::thread::scope(|scope| {
        let handle = std::thread::Builder::new()
            .name("pdf2md-convert".into())
            .stack_size(PARSE_STACK)
            .spawn_scoped(scope, || pipeline::run(bytes, opts, progress, cancel));
        match handle {
            Ok(h) => h
                .join()
                .unwrap_or_else(|panic| Err(ConvertError::Corrupt(panic_text(&panic)))),
            Err(e) => Err(ConvertError::Io(format!(
                "cannot start conversion thread: {e}"
            ))),
        }
    })
}

fn panic_text(p: &Box<dyn std::any::Any + Send>) -> String {
    p.downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| p.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "internal converter failure".into())
}
