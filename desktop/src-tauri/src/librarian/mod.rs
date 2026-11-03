//! Librarian: a reference library per Academic project (`docs/PLAN_LIBRARIAN.md`).
//! This is the data layer (L0): CSL-JSON records in the project's
//! `research.db`, RIS / BibTeX / CSL-JSON in and out, duplicate detection and
//! citation keys. The webview names a project and a reference id, never a
//! path or SQL.

mod bibtex;
mod citekey;
pub mod commands;
mod csl;
mod ris;
mod store;
mod transfer;

pub use store::{Entry, Library};
pub use transfer::{export, import, Format, ImportReport};
