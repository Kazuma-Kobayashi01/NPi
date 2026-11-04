//! Librarian: a reference library per Academic project (`docs/PLAN_LIBRARIAN.md`).
//! This is the data layer (L0): CSL-JSON records in the project's
//! `research.db`, RIS / BibTeX / CSL-JSON in and out, duplicate detection and
//! citation keys. The webview names a project and a reference id, never a
//! path or SQL.

mod attach;
mod bibtex;
mod citekey;
mod cjk;
pub mod commands;
mod csl;
mod format;
mod lookup;
pub mod mcp;
pub mod agent;
mod organize;
mod organize_tools;
mod pdf_meta;
mod pdf_tools;
mod reader;
mod research;
mod ris;
mod store;
mod suggest;
mod summaries;
mod tagged;
pub mod tools;
mod transfer;

pub use agent::{check_citations, render_get, render_report, render_search, search};
pub use format::{add_user_style, cite_draft, format, styles, Cited, Formatted, StyleInfo};

pub use organize::Group;
pub use reader::{PageView, PaperRow, SectionRow, Target};
pub use research::{IndexOptions, IndexReport, IndexStatus, QuoteCheck};
pub use store::{Entry, Library, Trashed};
pub use summaries::{Comparison, FactIn, SummaryRow};
pub use transfer::{export, import, import_any, Format, ImportReport};
