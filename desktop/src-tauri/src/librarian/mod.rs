//! Librarian: a reference library per Academic project (`docs/PLAN_LIBRARIAN.md`).
//! This is the data layer (L0): CSL-JSON records in the project's
//! `research.db`, RIS / BibTeX / CSL-JSON in and out, duplicate detection and
//! citation keys. The webview names a project and a reference id, never a
//! path or SQL.

mod attach;
mod bibtex;
mod citekey;
pub mod commands;
mod csl;
mod format;
mod lookup;
mod organize;
mod pdf_meta;
mod ris;
mod store;
mod transfer;

pub use format::{add_user_style, cite_draft, format, styles, Cited, Formatted, StyleInfo};

pub use organize::Group;
pub use store::{Entry, Library};
pub use transfer::{export, import, Format, ImportReport};
