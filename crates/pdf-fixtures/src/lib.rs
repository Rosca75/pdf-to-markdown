//! Dev-only raw PDF writer and deterministic test corpus for PDF2MD.
//!
//! * [`writer`] builds PDFs object by object (xref tables or streams, object
//!   streams, incremental updates, standard security handler).
//! * [`corpus::cases`] returns every corpus file as `(name, bytes)`.
//! * [`large`] generates big reports for benchmarks.

pub mod content;
pub mod corpus;
pub mod encryption;
pub mod fonts;
pub mod large;
pub mod layout;
pub mod obj;
pub mod writer;
