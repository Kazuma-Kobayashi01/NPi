//! Academic Copilot's host tools.
//!
//! The Copilot is an omp worker started without any of omp's own tools
//! (`--no-tools`, no extensions, skills, rules or context files). Its only
//! tools are the ones registered here over omp's Host Tool RPC: Rust sends
//! `set_host_tools` at startup, omp emits `host_tool_call` frames, and the
//! stdout reader answers each with a `host_tool_result` from [`CopilotHost::call`].
//! The webview cannot register or call these tools, and the project folder is
//! fixed when the worker starts, so neither the model nor the page names a path
//! outside it.
//!
//! Tool names use underscores: providers restrict tool names to
//! `[A-Za-z0-9_-]`.

use serde_json::{json, Value};
use std::path::{Component, Path, PathBuf};

/// Folders the Copilot may read, relative to the project.
const READ_DIRS: &[&str] = &["papers", "notes", "digests", "artifacts", "data", "drafts", "write"];
/// Folders it may write, by the `kind` of what it saves.
const WRITE_DIRS: &[(&str, &str)] = &[
    ("note", "notes"),
    ("digest", "digests"),
    ("artifact", "artifacts"),
    ("data", "data/processed"),
];
const TEXT_EXTS: &[&str] = &[
    "md", "txt", "csv", "tsv", "json", "tex", "bib", "yaml", "yml", "py",
];
const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;
const MAX_SCANNED_FILES: usize = 2000;
const MAX_HITS: usize = 20;
const MAX_READ_CHARS: usize = 24_000;
const MAX_WRITE_BYTES: usize = 256 * 1024;
const SNIPPET_CHARS: usize = 300;
/// PDFs extracted for the first time by one search; the rest wait for the next call.
const MAX_FRESH_PDFS: usize = 4;
/// Most pages one `research_ocr` call reads (a page takes a second or two).
const MAX_OCR_CALL_PAGES: usize = 6;

/// The tools registered with omp (`set_host_tools`).
pub fn tool_definitions() -> Value {
    json!([
        {
            "name": "research_search",
            "label": "Search project",
            "description": "Search this project (notes, digests, artifacts, processed data, the text of PDFs) for the query terms, Chinese or English. Returns file, line and a snippet per hit; once a paper is indexed, also its page and an anchor id that citations and research_check_quote use. A scanned PDF has no text and is reported, not searched.",
            "parameters": {
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Words to look for; a passage matches when it contains any, and ranks higher the more it contains." },
                    "limit": { "type": "integer", "description": "Maximum hits (default 10, at most 20)." }
                },
                "required": ["query"],
                "additionalProperties": false
            },
            "loadMode": "essential"
        },
        {
            "name": "research_read",
            "label": "Read project file",
            "description": "Read a text file of this project by its project-relative path (for example notes/2021-smith-attention.md), optionally from a line. Lists the folder when the path is a folder.",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Project-relative path, never absolute." },
                    "from_line": { "type": "integer", "description": "First line to return (1-based, default 1)." }
                },
                "required": ["path"],
                "additionalProperties": false
            },
            "loadMode": "essential"
        },
        {
            "name": "research_save",
            "label": "Save result",
            "description": "Save a note, digest, artifact or processed data file into the project. It never overwrites unless overwrite is true, and cannot touch papers or raw data.",
            "parameters": {
                "type": "object",
                "properties": {
                    "kind": { "type": "string", "enum": ["note", "digest", "artifact", "data"] },
                    "name": { "type": "string", "description": "File name without folders, for example 2021-smith-attention.md" },
                    "content": { "type": "string" },
                    "overwrite": { "type": "boolean" }
                },
                "required": ["kind", "name", "content"],
                "additionalProperties": false
            },
            "loadMode": "essential"
        },
        {
            "name": "research_check_quote",
            "label": "Check quote",
            "description": "Check that a quotation is on a page of a paper, ignoring line breaks, hyphenation and case. Give the project-relative path, the 1-based page and the exact words. Answers matched (with an anchor id), not found, or stale (the file changed since indexing). It does not judge whether the quote supports a claim.",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Project-relative path, for example papers/2021-smith-attention.pdf" },
                    "page": { "type": "integer", "description": "1-based page of a PDF; omit for a text file." },
                    "quote": { "type": "string", "description": "The quoted words, copied from the paper." }
                },
                "required": ["path", "quote"],
                "additionalProperties": false
            },
            "loadMode": "essential"
        },
        {
            "name": "research_summarize",
            "label": "Save summary",
            "description": "Save your summary of a paper (level \"paper\") or one section (level \"section\", the heading exactly as research_read shows it) for the user\'s reader. Search or read the paper first so it is indexed. `facts` are short claims, each with the exact `quote` and 1-based `page`: a quote is checked and stored as matched or unverified, a fact without one as your reading. Say in `coverage` what you did not read.",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Project-relative path of the paper." },
                    "level": { "type": "string", "enum": ["paper", "section"] },
                    "section": { "type": "string", "description": "The section heading, for level section." },
                    "summary": { "type": "string", "description": "The summary text, at most 4000 characters: problem, method, results, limits." },
                    "facts": { "type": "array", "items": { "type": "object", "properties": { "text": { "type": "string" }, "quote": { "type": "string" }, "page": { "type": "integer" } }, "required": ["text"], "additionalProperties": false } },
                    "coverage": { "type": "string", "description": "What the summary does not cover." }
                },
                "required": ["path", "level", "summary"],
                "additionalProperties": false
            },
            "loadMode": "essential"
        },
        {
            "name": "research_summaries",
            "label": "Read summaries",
            "description": "The summaries already saved for a paper, with each fact's status (matched, unverified, model) and whether the summary is stale because the file changed. Look here before summarizing again.",
            "parameters": {
                "type": "object",
                "properties": { "path": { "type": "string", "description": "Project-relative path of the paper." } },
                "required": ["path"],
                "additionalProperties": false
            },
            "loadMode": "essential"
        },
        {
            "name": "research_compare",
            "label": "Save comparison table",
            "description": "Save a comparison of papers as a table for the user\'s reader. `columns` are {key, label}; each row is {label, cells}, with cells mapping a column key to {value, path, page, quote, kind}. A value from a paper needs path, page and exact quote and is checked; without them it shows as your reading. Use kind \"not_reported\" when the paper does not say and \"not_comparable\" when the papers measure different things: never a plausible value.",
            "parameters": {
                "type": "object",
                "properties": {
                    "title": { "type": "string" },
                    "question": { "type": "string", "description": "The question the table answers." },
                    "columns": { "type": "array", "items": { "type": "object", "properties": { "key": { "type": "string" }, "label": { "type": "string" } }, "required": ["key"], "additionalProperties": false } },
                    "rows": { "type": "array", "items": { "type": "object", "properties": { "label": { "type": "string" }, "cells": { "type": "object" } }, "required": ["label", "cells"], "additionalProperties": false } }
                },
                "required": ["title", "columns", "rows"],
                "additionalProperties": false
            },
            "loadMode": "essential"
        },
        {
            "name": "research_check_number",
            "label": "Check number",
            "description": "Check that a number is on a page of a paper or in a text file, as a number standing alone: thousand separators, full-width digits, the minus sign and a percent sign are ignored. Optionally require a `unit` right after it and a `context` word in the same passage. Answers matched with the passages, or not found. It does not judge what the number means.",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "page": { "type": "integer", "description": "1-based page of a PDF; omit for a text file." },
                    "value": { "type": "string", "description": "The number, for example 91.2 or 1,024." },
                    "unit": { "type": "string", "description": "A unit that must follow the number, for example % or ms." },
                    "context": { "type": "string", "description": "A word that must be in the same passage, for example accuracy." }
                },
                "required": ["path", "value"],
                "additionalProperties": false
            },
            "loadMode": "essential"
        },
        {
            "name": "research_check_table",
            "label": "Check table cell",
            "description": "Find a Markdown table on a page, locate the row (first column contains `row`) and the column (header contains `column`), and compare the cell with `value` as numbers when both are numbers. Answers matched, mismatch (with the cell's actual text) or not found.",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "page": { "type": "integer" },
                    "row": { "type": "string" },
                    "column": { "type": "string" },
                    "value": { "type": "string" }
                },
                "required": ["path", "row", "column", "value"],
                "additionalProperties": false
            },
            "loadMode": "essential"
        },
        {
            "name": "research_ocr",
            "label": "OCR pages",
            "description": "Read pages of a scanned PDF with OCR on this computer, for when research_read or research_search reports no text. At most 6 pages per call (`pages`, 1-based; default page 1). The text is a recognition and may hold mistakes, above all in numbers, formulas and non-Latin scripts: say so, and check the page before quoting it. research_search indexes scanned PDFs by itself.",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Project-relative path of the PDF, for example papers/scan.pdf" },
                    "pages": { "type": "array", "items": { "type": "integer" }, "description": "1-based pages to read, at most 6." }
                },
                "required": ["path"],
                "additionalProperties": false
            },
            "loadMode": "essential"
        },
        {
            "name": "library_search",
            "label": "Search reference library",
            "description": "Search this project's reference library (the user's catalogued papers) by title, authors, year, venue, DOI, citekey, tags, notes or abstract. Returns citekey, authors, year, title and venue per reference. Use it, not memory, to know which references exist. A reference that is not in the library is unverified: say so.",
            "parameters": {
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Words to look for; a reference matches on any, and ranks higher the more it matches." },
                    "limit": { "type": "integer", "description": "Maximum references (default 10, at most 20)." }
                },
                "required": ["query"],
                "additionalProperties": false
            },
            "loadMode": "essential"
        },
        {
            "name": "library_get",
            "label": "Get reference",
            "description": "One reference of the library by citekey: its full record, the user's notes and tags, attached files (read those with research_read) and the reference formatted in a citation style.",
            "parameters": {
                "type": "object",
                "properties": {
                    "citekey": { "type": "string" },
                    "style": { "type": "string", "description": "Citation style id, default apa. Others: ieee, chicago-author-date, modern-language-association, nature, vancouver, harvard-cite-them-right, gb-7714-2015-numeric, gb-7714-2015-author-date." }
                },
                "required": ["citekey"],
                "additionalProperties": false
            },
            "loadMode": "essential"
        },
        {
            "name": "library_check",
            "label": "Check citations",
            "description": "Check a draft\'s citations against the library: [@citekey] markers, Author (Year) and (Author, Year) mentions, and DOIs. Reports which match, which differ in year, and which are not in the library (possibly invented). Give the text, or a project path such as write/markdown/01-intro.md. Numbered citations like [1] cannot be checked.",
            "parameters": {
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "The draft text." },
                    "path": { "type": "string", "description": "Or a project-relative path to the draft." }
                },
                "additionalProperties": false
            },
            "loadMode": "essential"
        },
        {
            "name": "library_suggest",
            "label": "Suggest library change",
            "description": "Propose a tag, a group or a note for a reference. Nothing changes: the user sees the suggestion on the reference and accepts or dismisses it. Use it for classification and organisation, with a short reason.",
            "parameters": {
                "type": "object",
                "properties": {
                    "citekey": { "type": "string" },
                    "kind": { "type": "string", "enum": ["tag", "group", "note"] },
                    "value": { "type": "string", "description": "The tag, the group name, or the note text." },
                    "reason": { "type": "string", "description": "Why, in one sentence." }
                },
                "required": ["citekey", "kind", "value"],
                "additionalProperties": false
            },
            "loadMode": "essential"
        }
    ])
}

/// Panics when a text names a `library_*`, `research_*` or `academic_*` tool outside `offered`: a tool
/// the model is told about and does not have is a failed call.
#[cfg(test)]
pub fn assert_names_exist(texts: &[String], offered: &[String]) {
    for text in texts {
        for word in text.split(|c: char| !c.is_alphanumeric() && c != '_') {
            let tool = ["library_", "research_", "academic_"].iter().any(|p| word.starts_with(p) && word.len() > p.len());
            assert!(!tool || offered.iter().any(|n| n == word), "{word} is named but not offered, in: {text}");
        }
    }
}

/// What the stdout reader needs of a worker's host tools: the definitions
/// sent with `set_host_tools` and the `host_tool_result` for one call.
pub trait HostTools: Send + Sync {
    fn definitions(&self) -> Value;
    fn call(&self, id: &str, tool: &str, args: &Value) -> Value;
}

impl HostTools for CopilotHost {
    fn definitions(&self) -> Value {
        tool_definitions()
    }
    fn call(&self, id: &str, tool: &str, args: &Value) -> Value {
        Self::call(self, id, tool, args)
    }
}

/// The tools of the Copilot and of an Academic session whose folder is an Academic project: the ones
/// for reading and checking, plus the library tools that reach Crossref or organise the library
/// (`librarian/tools.rs`), under plain names and bound to the project, so the model passes no project
/// and the user answers no approval prompt. Nothing here deletes anything.
pub struct AcademicHost {
    copilot: CopilotHost,
    tools: crate::librarian::tools::LibraryTools,
}

impl AcademicHost {
    pub fn new(root: PathBuf) -> Self {
        Self { copilot: CopilotHost::new(root.clone()), tools: crate::librarian::tools::LibraryTools::new(root) }
    }
}

impl HostTools for AcademicHost {
    fn definitions(&self) -> Value {
        let mut all = tool_definitions().as_array().cloned().unwrap_or_default();
        all.extend(crate::librarian::tools::LibraryTools::definitions());
        Value::Array(all)
    }
    fn call(&self, id: &str, tool: &str, args: &Value) -> Value {
        match self.tools.call(tool, args, &crate::librarian::tools::Crossref) {
            Some(outcome) => frame(id, outcome),
            None => self.copilot.call(id, tool, args),
        }
    }
}

/// The main chat's read-only view of the Academic library: it lists the
/// projects and searches or reads one by name, with the same limits as the
/// Copilot's tools (`CopilotHost`), and can write nothing.
pub struct ResearchHub {
    space: std::sync::Arc<crate::academic_space::AcademicSpace>,
}

impl ResearchHub {
    pub const fn new(space: std::sync::Arc<crate::academic_space::AcademicSpace>) -> Self {
        Self { space }
    }

    fn with_project(
        &self,
        args: &Value,
        run: impl Fn(&CopilotHost, &Value) -> ToolResult,
    ) -> ToolResult {
        let name = args.get("project").and_then(Value::as_str).unwrap_or("");
        let root = self.space.project_named(name).ok_or_else(|| {
            format!("no Academic project named {name:?}; call academic_projects first")
        })?;
        run(&CopilotHost::new(root), args)
    }
}

impl HostTools for ResearchHub {
    fn definitions(&self) -> Value {
        json!([
            {
                "name": "academic_projects",
                "label": "List Academic projects",
                "description": "List the user's Academic research projects (their local library of papers, notes and digests). Use the academic_* tools, not memory, for questions about the user's own papers and research notes.",
                "parameters": { "type": "object", "properties": {}, "additionalProperties": false },
                "loadMode": "essential"
            },
            {
                "name": "academic_search",
                "label": "Search Academic project",
                "description": "Search one Academic project's local text files (notes, digests, artifacts, processed data, text papers and the extracted text of PDFs) for passages containing the query terms; returns file:line and a snippet. Cite what you use. A scanned PDF has no text and is reported, not searched.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "project": { "type": "string", "description": "Project name from academic_projects." },
                        "query": { "type": "string" },
                        "limit": { "type": "integer" }
                    },
                    "required": ["project", "query"],
                    "additionalProperties": false
                },
                "loadMode": "essential"
            },
            {
                "name": "academic_read",
                "label": "Read Academic file",
                "description": "Read a text file of an Academic project by project-relative path (for example notes/2021-smith.md), optionally from a line; lists the folder for a folder path.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "project": { "type": "string" },
                        "path": { "type": "string" },
                        "from_line": { "type": "integer" }
                    },
                    "required": ["project", "path"],
                    "additionalProperties": false
                },
                "loadMode": "essential"
            },
            {
                "name": "academic_ocr",
                "label": "OCR pages",
                "description": "Read pages of a scanned PDF of one Academic project with OCR on this computer and return the recognised text per page (at most 6 pages per call). Use it when academic_read or academic_search says a PDF has no text. The text may contain mistakes, especially numbers and formulas: say so.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "project": { "type": "string" },
                        "path": { "type": "string", "description": "Project-relative path of the PDF, for example papers/scan.pdf" },
                        "pages": { "type": "array", "items": { "type": "integer" } }
                    },
                    "required": ["project", "path"],
                    "additionalProperties": false
                },
                "loadMode": "essential"
            },
            {
                "name": "academic_library_search",
                "label": "Search reference library",
                "description": "Search one Academic project's reference library (the user's catalogued papers) by title, authors, year, venue, DOI, citekey, tags or notes. A reference not in the library is unverified; say so instead of guessing.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "project": { "type": "string" },
                        "query": { "type": "string" },
                        "limit": { "type": "integer" }
                    },
                    "required": ["project", "query"],
                    "additionalProperties": false
                },
                "loadMode": "essential"
            },
            {
                "name": "academic_library_get",
                "label": "Get reference",
                "description": "One reference of an Academic project's library by citekey, with its formatted reference.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "project": { "type": "string" },
                        "citekey": { "type": "string" },
                        "style": { "type": "string" }
                    },
                    "required": ["project", "citekey"],
                    "additionalProperties": false
                },
                "loadMode": "essential"
            },
            {
                "name": "academic_library_check",
                "label": "Check citations",
                "description": "Check a draft's citations ([@citekey], Author (Year), DOIs) against an Academic project's library: which match, which have another year, which are not in the library (possibly invented).",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "project": { "type": "string" },
                        "text": { "type": "string" },
                        "path": { "type": "string", "description": "Or a project-relative path to the draft." }
                    },
                    "required": ["project"],
                    "additionalProperties": false
                },
                "loadMode": "essential"
            }
        ])
    }

    fn call(&self, id: &str, tool: &str, args: &Value) -> Value {
        let outcome = match tool {
            "academic_projects" => {
                let names = self.space.project_names();
                Ok(if names.is_empty() {
                    "No Academic projects yet.".to_string()
                } else {
                    names.join("\n")
                })
            }
            "academic_search" => self.with_project(args, CopilotHost::search),
            "academic_read" => self.with_project(args, CopilotHost::read),
            "academic_ocr" => self.with_project(args, CopilotHost::ocr),
            "academic_library_search" => self.with_project(args, CopilotHost::library_search),
            "academic_library_get" => self.with_project(args, CopilotHost::library_get),
            "academic_library_check" => self.with_project(args, CopilotHost::library_check),
            other => Err(format!("unknown tool: {other}")),
        };
        frame(id, outcome)
    }
}

pub(crate) fn frame(id: &str, outcome: ToolResult) -> Value {
    let (text, is_error) = match outcome {
        Ok(t) => (t, false),
        Err(e) => (e, true),
    };
    json!({
        "type": "host_tool_result",
        "id": id,
        "result": { "content": [{ "type": "text", "text": text }] },
        "isError": is_error,
    })
}

pub struct CopilotHost {
    /// The canonical project folder this worker is bound to.
    root: PathBuf,
}

pub(crate) type ToolResult = Result<String, String>;

impl CopilotHost {
    pub const fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// Runs one tool call and returns the `host_tool_result` frame for `id`.
    pub fn call(&self, id: &str, tool: &str, args: &Value) -> Value {
        let outcome = match tool {
            "research_search" => self.search(args),
            "research_read" => self.read(args),
            "research_save" => self.save(args),
            "research_check_quote" => self.check_quote(args),
            "research_ocr" => self.ocr(args),
            "research_summarize" => self.summarize(args),
            "research_summaries" => self.summaries(args),
            "research_compare" => self.compare(args),
            "research_check_number" => self.check_number(args),
            "research_check_table" => self.check_table(args),
            "library_search" => self.library_search(args),
            "library_get" => self.library_get(args),
            "library_check" => self.library_check(args),
            "library_suggest" => self.library_suggest(args),
            other => Err(format!("unknown tool: {other}")),
        };
        frame(id, outcome)
    }

    /// Every live reference of this project's library.
    fn library(&self) -> Result<(crate::librarian::Library, Vec<crate::librarian::Entry>), String> {
        let lib = crate::librarian::Library::open(&self.root)?;
        let mut entries = lib.list()?;
        // A file that moved is said to be gone, or the model would try to read it.
        crate::librarian::mark_missing(&self.root, &mut entries);
        Ok((lib, entries))
    }

    fn library_search(&self, args: &Value) -> ToolResult {
        let query = args.get("query").and_then(Value::as_str).unwrap_or("").trim();
        if query.is_empty() {
            return Err("query is empty".into());
        }
        let limit = args.get("limit").and_then(Value::as_u64).map_or(10, |n| {
            usize::try_from(n).unwrap_or(MAX_HITS).clamp(1, MAX_HITS)
        });
        let (lib, entries) = self.library()?;
        // The full-text index when the project has one (it finds short Chinese words); the plain
        // scan otherwise, and when the index finds nothing.
        let ranked = lib.search_library(&entries, query, limit).ok().flatten().unwrap_or_default();
        let mut hits: Vec<&crate::librarian::Entry> =
            ranked.iter().filter_map(|id| entries.iter().find(|e| e.id == *id)).collect();
        if hits.is_empty() {
            hits = crate::librarian::search(&entries, query, limit);
        }
        Ok(crate::librarian::render_search(&hits, query))
    }

    /// Hits from the Research Core's index, or `None` when the project has no index or it finds
    /// nothing (the caller then scans the files). New and changed files are indexed first, a few
    /// PDFs at a time.
    fn indexed_search(&self, query: &str, limit: usize) -> Option<String> {
        use crate::librarian::{IndexOptions, Library};
        let lib = Library::open(&self.root).ok()?;
        if !lib.has_index().ok()? {
            return None;
        }
        let report = lib
            .index_project(&self.root, IndexOptions { text: true, max_new_pdfs: Some(MAX_FRESH_PDFS) })
            .ok()?;
        let hits = lib.search_blocks(query, None, limit).ok()?;
        if hits.is_empty() {
            return None;
        }
        let mut lines: Vec<String> = hits
            .iter()
            .map(|h| {
                let page = h.page.map_or(String::new(), |p| format!(" p.{p}"));
                let anchor = h.anchor_id.map_or(String::new(), |a| format!(" [anchor {a}]"));
                format!("{}{page}{anchor}: {}", h.path, h.snippet)
            })
            .collect();
        if report.waiting > 0 {
            lines.push(format!(
                "{} PDF(s) were not indexed yet: they are read a few at a time, so search again.",
                report.waiting
            ));
        }
        if !report.scanned.is_empty() {
            lines.push(format!(
                "No text layer (scanned, needs OCR), not searched: {}.",
                report.scanned.join(", ")
            ));
        }
        Some(lines.join("\n"))
    }

    fn check_quote(&self, args: &Value) -> ToolResult {
        use crate::librarian::{Library, QuoteCheck};
        let get = |k: &str| args.get(k).and_then(Value::as_str).unwrap_or("");
        let real = self.resolve(get("path"), READ_DIRS)?;
        let rel = real
            .strip_prefix(&self.root)
            .map_err(|_| "the path leaves the project".to_string())?
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/");
        let page = match args.get("page") {
            None | Some(Value::Null) => None,
            Some(v) => Some(
                v.as_u64()
                    .and_then(|n| u32::try_from(n).ok())
                    .filter(|n| *n >= 1)
                    .ok_or("page is a 1-based integer")?,
            ),
        };
        let lib = Library::open(&self.root)?;
        let verdict = lib.check_quote(&self.root, &rel, page, get("quote"))?;
        let at = page.map_or(rel.clone(), |p| format!("{rel} p.{p}"));
        Ok(match verdict {
            QuoteCheck::Matched { anchor_id, .. } => format!(
                "matched: the quote is on {at}{}.",
                anchor_id.map_or(String::new(), |a| format!(" (anchor {a})"))
            ),
            QuoteCheck::NotFound => format!("not found: the quote is not on {at} of the indexed text. Check the page number and the wording."),
            QuoteCheck::Stale => format!("stale: {rel} changed or is gone since it was indexed, so the indexed text cannot vouch for it. Search the project again to re-index it."),
            QuoteCheck::Unreadable { status } => format!("{rel} has no searchable text (parse status: {status}), so the quote cannot be checked."),
        })
    }

    /// The project-relative form of a path the tool was given (it must stay inside the project's readable folders).
    fn rel_of(&self, path: &str) -> Result<String, String> {
        let real = self.resolve(path, READ_DIRS)?;
        Ok(real
            .strip_prefix(&self.root)
            .map_err(|_| "the path leaves the project".to_string())?
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/"))
    }

    fn page_arg(args: &Value) -> Result<Option<u32>, String> {
        match args.get("page") {
            None | Some(Value::Null) => Ok(None),
            Some(v) => v
                .as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .filter(|n| *n >= 1)
                .map(Some)
                .ok_or_else(|| "page is a 1-based integer".to_string()),
        }
    }

    /// OCR of a few pages of a PDF of this project, returned as text (nothing is stored).
    fn ocr(&self, args: &Value) -> ToolResult {
        let rel = self.rel_of(args.get("path").and_then(Value::as_str).unwrap_or(""))?;
        if !is_pdf(Path::new(&rel)) {
            return Err(format!("{rel} is not a PDF"));
        }
        let pages: Vec<u32> = match args.get("pages") {
            None | Some(Value::Null) => vec![1],
            Some(v) => v
                .as_array()
                .ok_or("pages is a list of 1-based integers")?
                .iter()
                .map(|n| n.as_u64().and_then(|n| u32::try_from(n).ok()).filter(|n| *n >= 1).ok_or("pages is a list of 1-based integers"))
                .collect::<Result<_, _>>()?,
        };
        if pages.is_empty() || pages.len() > MAX_OCR_CALL_PAGES {
            return Err(format!("give 1 to {MAX_OCR_CALL_PAGES} pages per call"));
        }
        let path = self.root.join(&rel);
        let len = std::fs::metadata(&path).map_err(|e| format!("{rel}: {e}"))?.len();
        if len > crate::pdf_text::MAX_PDF_BYTES {
            return Err(format!("{rel} is too large to read"));
        }
        let bytes = std::fs::read(&path).map_err(|e| format!("{rel}: {e}"))?;
        let (read, count) = crate::ocr::recognize(&bytes, Some(&pages))?;
        if read.is_empty() {
            return Ok(format!("OCR found no text on the requested pages of {rel} ({count} pages in all)."));
        }
        let mut out = format!("OCR of {rel} ({count} pages in all; recognised text, may contain mistakes):\n");
        for (page, text) in read {
            out.push_str(&format!("\n[p.{page}]\n{text}\n"));
        }
        Ok(crate::text::prefix_within(&out, MAX_READ_CHARS).to_string())
    }

    fn summarize(&self, args: &Value) -> ToolResult {
        use crate::librarian::{FactIn, Library};
        let get = |k: &str| args.get(k).and_then(Value::as_str);
        let rel = self.rel_of(get("path").unwrap_or(""))?;
        let facts: Vec<FactIn> = match args.get("facts") {
            None | Some(Value::Null) => Vec::new(),
            Some(v) => serde_json::from_value(v.clone()).map_err(|e| format!("facts: {e}"))?,
        };
        let lib = Library::open(&self.root)?;
        let counts = lib.save_summary(&self.root, &rel, get("level").unwrap_or(""), get("section"), get("summary").unwrap_or(""), &facts, get("coverage").unwrap_or(""))?;
        Ok(format!(
            "saved. Facts: {} matched the page, {} could not be matched (shown as unverified), {} are your reading. The summary is marked stale if {rel} changes.",
            counts["matched"], counts["unverified"], counts["model"]
        ))
    }

    fn summaries(&self, args: &Value) -> ToolResult {
        use crate::librarian::Library;
        let rel = self.rel_of(args.get("path").and_then(Value::as_str).unwrap_or(""))?;
        let lib = Library::open(&self.root)?;
        let paper = lib.papers_list()?.into_iter().find(|p| p.path == rel).ok_or_else(|| format!("{rel}: not indexed"))?;
        let rows = lib.summaries_for(paper.id)?;
        if rows.is_empty() {
            return Ok(format!("no summary of {rel} yet."));
        }
        serde_json::to_string(&rows).map_err(|e| e.to_string())
    }

    fn compare(&self, args: &Value) -> ToolResult {
        use crate::librarian::Library;
        let lib = Library::open(&self.root)?;
        let get = |k: &str| args.get(k).and_then(Value::as_str).unwrap_or("");
        let r = lib.save_comparison(&self.root, get("title"), get("question"), args.get("columns").unwrap_or(&Value::Null), args.get("rows").unwrap_or(&Value::Null))?;
        Ok(format!(
            "saved comparison {}: {} cells matched the papers, {} could not be matched (unverified), {} are your reading, {} gaps (not reported / not comparable).",
            r["id"], r["matched"], r["unverified"], r["model"], r["gaps"]
        ))
    }

    fn check_number(&self, args: &Value) -> ToolResult {
        use crate::librarian::Library;
        let get = |k: &str| args.get(k).and_then(Value::as_str);
        let rel = self.rel_of(get("path").unwrap_or(""))?;
        let lib = Library::open(&self.root)?;
        let r = lib.check_number(&rel, Self::page_arg(args)?, get("value").unwrap_or(""), get("unit"), get("context"))?;
        Ok(r.to_string())
    }

    fn check_table(&self, args: &Value) -> ToolResult {
        use crate::librarian::Library;
        let get = |k: &str| args.get(k).and_then(Value::as_str).unwrap_or("");
        let rel = self.rel_of(get("path"))?;
        let lib = Library::open(&self.root)?;
        Ok(lib.check_table_cell(&rel, Self::page_arg(args)?, get("row"), get("column"), get("value"))?.to_string())
    }

    fn library_get(&self, args: &Value) -> ToolResult {
        let key = args.get("citekey").and_then(Value::as_str).unwrap_or("").trim();
        let style = args.get("style").and_then(Value::as_str).unwrap_or("apa");
        let (_, entries) = self.library()?;
        entries
            .iter()
            .find(|e| e.citekey == key)
            .map(|e| crate::librarian::render_get(e, style))
            .ok_or_else(|| crate::librarian::tools::no_such_citekey(&entries, key))
    }

    fn library_check(&self, args: &Value) -> ToolResult {
        let text = match (args.get("text").and_then(Value::as_str), args.get("path").and_then(Value::as_str)) {
            (Some(t), _) if !t.trim().is_empty() => t.to_string(),
            (_, Some(p)) => {
                let file = self.resolve(p, READ_DIRS)?;
                if file.metadata().map_err(|e| e.to_string())?.len() > MAX_FILE_BYTES {
                    return Err("the draft is too large".into());
                }
                std::fs::read_to_string(&file).map_err(|_| "the draft is not a text file".to_string())?
            }
            _ => return Err("give the draft as text or as a project-relative path".into()),
        };
        let (_, entries) = self.library()?;
        Ok(crate::librarian::render_report(&crate::librarian::check_citations(&entries, &text)))
    }

    fn library_suggest(&self, args: &Value) -> ToolResult {
        let get = |k: &str| args.get(k).and_then(Value::as_str).unwrap_or("").trim().to_string();
        let (lib, entries) = self.library()?;
        let key = get("citekey");
        let entry = entries
            .iter()
            .find(|e| e.citekey == key)
            .ok_or_else(|| format!("no reference with the citekey {key:?}; use library_search"))?;
        let added = lib.suggest(entry.id, &get("kind"), &get("value"), &get("reason"))?;
        Ok(if added {
            format!("Suggested to the user for {key}. It changes nothing until they accept it.")
        } else {
            format!("That suggestion for {key} is already waiting for the user.")
        })
    }

    /// A project-relative path resolved inside the project: no absolute path,
    /// no `..`, nothing outside `dirs` (or the project itself when `dirs` is
    /// empty), and a symlink may not lead out.
    fn resolve(&self, rel: &str, dirs: &[&str]) -> Result<PathBuf, String> {
        let rel_path = Path::new(rel);
        if rel.is_empty() || rel_path.is_absolute() {
            return Err("use a project-relative path".into());
        }
        if rel_path
            .components()
            .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
        {
            return Err("the path may not contain '..'".into());
        }
        let first = rel_path
            .components()
            .find_map(|c| match c {
                Component::Normal(s) => s.to_str(),
                _ => None,
            })
            .unwrap_or("");
        if !dirs.is_empty() && !dirs.contains(&first) {
            return Err(format!("only {} are available", dirs.join(", ")));
        }
        let full = self.root.join(rel_path);
        let real = full
            .canonicalize()
            .map_err(|_| format!("{rel}: not found"))?;
        if !real.starts_with(&self.root) {
            return Err("the path leaves the project".into());
        }
        Ok(real)
    }

    fn files(&self) -> Vec<PathBuf> {
        let mut out = Vec::new();
        let mut stack: Vec<PathBuf> = READ_DIRS.iter().map(|d| self.root.join(d)).collect();
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let Ok(kind) = entry.file_type() else {
                    continue;
                };
                // A symlink is skipped, never followed; nothing hidden is read (write/.git).
                if kind.is_symlink() || entry.file_name().to_string_lossy().starts_with('.') {
                    continue;
                }
                let path = entry.path();
                if kind.is_dir() {
                    stack.push(path);
                } else if (is_text(&path) || is_pdf(&path)) && out.len() < MAX_SCANNED_FILES {
                    out.push(path);
                }
            }
        }
        out.sort();
        out
    }

    fn search(&self, args: &Value) -> ToolResult {
        let query = args
            .get("query")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        if query.is_empty() {
            return Err("query is empty".into());
        }
        let limit = args.get("limit").and_then(Value::as_u64).map_or(10, |n| {
            usize::try_from(n).unwrap_or(MAX_HITS).clamp(1, MAX_HITS)
        });
        if let Some(found) = self.indexed_search(query, limit) {
            return Ok(found);
        }
        let terms: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
        let mut hits: Vec<(usize, String, usize, String)> = Vec::new();
        let (mut fresh, mut waiting, mut scanned) = (0usize, 0usize, Vec::new());
        for path in self.files() {
            let rel = path
                .strip_prefix(&self.root)
                .unwrap_or(&path)
                .to_string_lossy()
                .into_owned();
            let text = if is_pdf(&path) {
                let known = crate::pdf_text::cached(&self.root, &rel, &path);
                let outcome = match known {
                    Some(hit) => Ok(hit),
                    None if fresh < MAX_FRESH_PDFS => {
                        fresh += 1;
                        crate::pdf_text::extract_cached(&self.root, &rel, &path)
                    }
                    None => {
                        waiting += 1;
                        continue;
                    }
                };
                match outcome {
                    Ok(crate::pdf_text::Extracted::Text(t)) => t,
                    Ok(crate::pdf_text::Extracted::Scanned) => {
                        scanned.push(rel);
                        continue;
                    }
                    Err(_) => continue,
                }
            } else {
                let Ok(meta) = std::fs::metadata(&path) else {
                    continue;
                };
                if meta.len() > MAX_FILE_BYTES {
                    continue;
                }
                let Ok(text) = std::fs::read_to_string(&path) else {
                    continue;
                };
                text
            };
            for (score, line, snippet) in best_passages(&text, &terms) {
                hits.push((score, rel.clone(), line, snippet));
            }
        }
        hits.sort_by(|a, b| {
            b.0.cmp(&a.0)
                .then_with(|| a.1.cmp(&b.1))
                .then_with(|| a.2.cmp(&b.2))
        });
        hits.truncate(limit);
        let mut notes = Vec::new();
        if waiting > 0 {
            notes.push(format!(
                "{waiting} PDF(s) were not searched yet: their text is extracted a few at a time, so search again."
            ));
        }
        if !scanned.is_empty() {
            notes.push(format!(
                "No text layer (scanned, needs OCR), not searched: {}.",
                scanned.join(", ")
            ));
        }
        let mut lines: Vec<String> = hits
            .iter()
            .map(|(_, file, line, snippet)| format!("{file}:{line}: {snippet}"))
            .collect();
        if lines.is_empty() {
            lines.push("No passage matched.".into());
        }
        lines.extend(notes);
        Ok(lines.join("\n"))
    }

    fn read(&self, args: &Value) -> ToolResult {
        let rel = args.get("path").and_then(Value::as_str).unwrap_or("");
        let real = self.resolve(rel, READ_DIRS)?;
        if real.is_dir() {
            let mut names: Vec<String> = std::fs::read_dir(&real)
                .map_err(|e| e.to_string())?
                .flatten()
                .map(|e| {
                    let suffix = if e.path().is_dir() { "/" } else { "" };
                    format!("{}{suffix}", e.file_name().to_string_lossy())
                })
                .collect();
            names.sort();
            return Ok(names.join("\n"));
        }
        let text = if is_pdf(&real) {
            let rel = real.strip_prefix(&self.root).unwrap_or(&real).to_string_lossy().into_owned();
            match crate::pdf_text::extract_cached(&self.root, &rel, &real)? {
                crate::pdf_text::Extracted::Text(t) => t,
                crate::pdf_text::Extracted::Scanned => {
                    return Err("the PDF has no text layer (scanned); OCR is not available".into())
                }
            }
        } else if is_text(&real) {
            if std::fs::metadata(&real).map_err(|e| e.to_string())?.len() > MAX_FILE_BYTES {
                return Err("the file is too large".into());
            }
            std::fs::read_to_string(&real).map_err(|e| e.to_string())?
        } else {
            return Err("only text files and PDFs can be read".into());
        };
        let from = args
            .get("from_line")
            .and_then(Value::as_u64)
            .map_or(1, |n| usize::try_from(n).unwrap_or(1).max(1));
        let body: String = text.lines().skip(from - 1).collect::<Vec<_>>().join("\n");
        let cut = crate::text::prefix_within(&body, MAX_READ_CHARS);
        if cut.len() < body.len() {
            Ok(format!(
                "{cut}\n[cut here; continue with from_line={}]",
                from + cut.lines().count()
            ))
        } else {
            Ok(body)
        }
    }

    fn save(&self, args: &Value) -> ToolResult {
        let kind = args.get("kind").and_then(Value::as_str).unwrap_or("");
        let dir = WRITE_DIRS
            .iter()
            .find_map(|(k, d)| (*k == kind).then_some(*d))
            .ok_or("kind must be note, digest, artifact or data")?;
        let name = args.get("name").and_then(Value::as_str).unwrap_or("");
        if !valid_name(name) {
            return Err(
                "name must be a plain file name (letters, digits, ._- and spaces), no folders"
                    .into(),
            );
        }
        if !TEXT_EXTS.contains(
            &Path::new(name)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or(""),
        ) {
            return Err(format!("extension must be one of {}", TEXT_EXTS.join(", ")));
        }
        let content = args.get("content").and_then(Value::as_str).unwrap_or("");
        if content.len() > MAX_WRITE_BYTES {
            return Err("content is too large".into());
        }
        let folder = self.root.join(dir);
        std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
        // The folder is ours, but a symlink planted at it must not lead out.
        if !folder
            .canonicalize()
            .map_err(|e| e.to_string())?
            .starts_with(&self.root)
        {
            return Err("the folder leaves the project".into());
        }
        let target = folder.join(name);
        let overwrite = args
            .get("overwrite")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if target.symlink_metadata().is_ok() {
            if !overwrite {
                return Err(format!(
                    "{dir}/{name} already exists; pass overwrite=true to replace it"
                ));
            }
            if target
                .symlink_metadata()
                .is_ok_and(|m| m.file_type().is_symlink())
            {
                return Err("refusing to write through a symlink".into());
            }
        }
        std::fs::write(&target, content).map_err(|e| e.to_string())?;
        Ok(format!("saved {dir}/{name} ({} bytes)", content.len()))
    }
}

fn is_text(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| TEXT_EXTS.contains(&e.to_ascii_lowercase().as_str()))
}

fn is_pdf(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 120
        && !name.starts_with('.')
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '.' | '_' | '-' | ' '))
}

/// Blank-line separated passages of `text` that contain any of `terms`:
/// `(distinct terms found, first line, snippet)`, the best few of the file.
fn best_passages(text: &str, terms: &[String]) -> Vec<(usize, usize, String)> {
    let mut out = Vec::new();
    let mut start = 1;
    let mut block: Vec<&str> = Vec::new();
    let flush = |block: &mut Vec<&str>, start: usize, out: &mut Vec<(usize, usize, String)>| {
        if block.is_empty() {
            return;
        }
        let joined = block.join(" ");
        let lower = joined.to_lowercase();
        let score = terms.iter().filter(|t| lower.contains(t.as_str())).count();
        if score > 0 {
            let snippet: String = joined.trim().chars().take(SNIPPET_CHARS).collect();
            out.push((score, start, snippet));
        }
        block.clear();
    };
    for (i, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            flush(&mut block, start, &mut out);
            start = i + 2;
        } else {
            if block.is_empty() {
                start = i + 1;
            }
            block.push(line);
        }
    }
    flush(&mut block, start, &mut out);
    out.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    out.truncate(5);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    fn host(name: &str) -> (TempDir, CopilotHost) {
        let d = TempDir::new("copilot", name);
        std::fs::create_dir_all(d.path().join("proj")).unwrap();
        let root = d.path().join("proj").canonicalize().unwrap();
        for dir in [
            "papers",
            "notes",
            "digests",
            "artifacts",
            "data/raw",
            "data/processed",
            ".omp",
        ] {
            std::fs::create_dir_all(root.join(dir)).unwrap();
        }
        (d, CopilotHost::new(root))
    }

    fn text_of(frame: &Value) -> String {
        frame["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .to_string()
    }

    #[test]
    fn definitions_are_fixed_and_underscore_named() {
        let defs = tool_definitions();
        let names: Vec<&str> = defs
            .as_array()
            .unwrap()
            .iter()
            .map(|d| d["name"].as_str().unwrap())
            .collect();
        assert_eq!(
            names,
            [
                "research_search", "research_read", "research_save", "research_check_quote", "research_summarize",
                "research_summaries", "research_compare", "research_check_number", "research_check_table", "research_ocr", "library_search",
                "library_get", "library_check", "library_suggest"
            ]
        );
        assert!(names
            .iter()
            .all(|n| n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')));
    }

    #[test]
    fn an_academic_session_has_the_copilots_tools_and_the_library_ones_once_each() {
        let (_d, h) = host("academic-host");
        let academic = AcademicHost::new(h.root.clone());
        let names: Vec<String> = academic.definitions().as_array().unwrap().iter().map(|d| d["name"].as_str().unwrap().to_string()).collect();
        let mut sorted = names.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), names.len(), "no name twice");
        for n in ["library_search", "library_lookup", "library_align", "library_add", "library_edit", "research_save"] {
            assert!(names.iter().any(|x| x == n), "{n}");
        }
        assert!(!names.iter().any(|n| n.contains("delete") || n.contains("remove")), "nothing deletes");
        assert!(names.iter().any(|n| n == "library_group"), "the organising tools");
        // A library tool is answered here, a Copilot tool by the Copilot.
        assert!(text_of(&academic.call("1", "library_edit", &json!({ "citekey": "nope" }))).contains("library_search"));
        assert!(text_of(&academic.call("2", "research_search", &json!({ "query": "x" }))).len() > 0);
    }

    #[test]
    fn a_session_is_told_only_of_tools_it_has() {
        let (_d, h) = host("tool-names");
        let check = |defs: Value, extra: &[String]| {
            let list = defs.as_array().unwrap();
            let offered: Vec<String> = list.iter().filter_map(|t| t["name"].as_str().map(str::to_string)).collect();
            let mut texts: Vec<String> = list.iter().filter_map(|t| t["description"].as_str().map(str::to_string)).collect();
            texts.extend_from_slice(extra);
            assert_names_exist(&texts, &offered);
        };
        check(AcademicHost::new(h.root.clone()).definitions(), &[crate::copilot_worker::system_prompt(&h.root)]);
        check(crate::copilot_worker::MainHost::new(h.root.clone(), None).definitions(), &[crate::app_mode::academic_prompt(Some(&h.root))]);
    }

    #[test]
    fn the_project_templates_name_only_tools_that_exist() {
        let (_d, h) = host("template-names");
        let main = crate::copilot_worker::MainHost::new(h.root.clone(), None).definitions();
        let mut offered: Vec<String> = main.as_array().unwrap().iter().filter_map(|t| t["name"].as_str().map(str::to_string)).collect();
        offered.push("academic_library_search".to_string()); // the skill says it is for the main chat, outside a project
        let texts = [
            include_str!("../templates/academic/RULES.md").to_string(),
            include_str!("../templates/academic/AGENTS.md").to_string(),
            include_str!("../templates/academic/skills/librarian/SKILL.md").to_string(),
            include_str!("../templates/academic/skills/literature-intake/SKILL.md").to_string(),
            include_str!("../templates/academic/skills/paper-digest/SKILL.md").to_string(),
            include_str!("../templates/academic/skills/data-processing/SKILL.md").to_string(),
        ];
        assert_names_exist(&texts, &offered);
    }

    #[test]
    fn a_file_that_moved_is_reported_gone() {
        let (_d, h) = host("missing-file");
        let lib = crate::librarian::Library::open(&h.root).unwrap();
        let e = lib.add(&json!({ "type": "book", "title": "Trees and forests" }), "manual").unwrap();
        lib.attach(e.id, "papers/gone.pdf", &"a".repeat(64), "pdf").unwrap();
        let got = text_of(&h.call("1", "library_get", &json!({ "citekey": e.citekey })));
        assert!(got.contains("papers/gone.pdf (missing"), "{got}");
    }

    #[test]
    fn ocr_takes_only_a_few_pdf_pages_of_the_project() {
        let (_d, h) = host("ocr-args");
        std::fs::write(h.root.join("notes/n.md"), "x").unwrap();
        std::fs::write(h.root.join("papers/a.pdf"), crate::pdf_text::pages_pdf(&["One"])).unwrap();
        let err = |args: Value| text_of(&h.call("1", "research_ocr", &args));
        assert!(err(json!({ "path": "notes/n.md" })).contains("not a PDF"));
        assert!(err(json!({ "path": "papers/a.pdf", "pages": [1, 2, 3, 4, 5, 6, 7] })).contains("1 to 6 pages"));
        assert!(err(json!({ "path": "papers/a.pdf", "pages": [0] })).contains("1-based"));
        assert!(err(json!({ "path": "../x.pdf" })).len() > 0);
    }

    fn with_library(h: &CopilotHost) {
        use crate::librarian::{import, Format, Library};
        let lib = Library::open(&h.root).unwrap();
        import(
            &lib,
            Format::Bibtex,
            "@article{a, author = {Smith, Jo}, title = {Graph methods}, year = {2020}, doi = {10.1/x}}\n@book{b, author = {Lee, Al}, title = {Trees}, year = {2018}}\n",
        )
        .unwrap();
    }

    #[test]
    fn the_library_tools_search_get_check_and_only_suggest() {
        let (_d, h) = host("library");
        with_library(&h);
        let found = text_of(&h.call("1", "library_search", &json!({ "query": "graph" })));
        assert!(found.contains("[smith2020graph] Smith, Jo (2020). Graph methods"), "{found}");
        assert_eq!(h.call("2", "library_search", &json!({ "query": " " }))["isError"], true);
        let got = text_of(&h.call("3", "library_get", &json!({ "citekey": "smith2020graph" })));
        assert!(got.contains("reference (apa): Smith, J. (2020). Graph methods."), "{got}");
        assert_eq!(h.call("4", "library_get", &json!({ "citekey": "nobody" }))["isError"], true);

        std::fs::create_dir_all(h.root.join("drafts")).unwrap();
        std::fs::write(h.root.join("drafts/d.md"), "See [@smith2020graph], Jones (2021) and [@ghost].").unwrap();
        let checked = text_of(&h.call("5", "library_check", &json!({ "path": "drafts/d.md" })));
        assert!(checked.contains("CHECK [author-year] Jones 2021"), "{checked}");
        assert!(checked.contains("unknown-key"), "{checked}");
        assert_eq!(h.call("6", "library_check", &json!({ "path": "../x.md" }))["isError"], true);
        assert_eq!(h.call("7", "library_check", &json!({ "path": "papers/../.omp/RULES.md" }))["isError"], true);

        // A suggestion changes nothing in the library.
        let said = text_of(&h.call("8", "library_suggest", &json!({ "citekey": "smith2020graph", "kind": "tag", "value": "graphs", "reason": "topic" })));
        assert!(said.contains("changes nothing"), "{said}");
        assert!(text_of(&h.call("9", "library_suggest", &json!({ "citekey": "smith2020graph", "kind": "tag", "value": "GRAPHS" }))).contains("already waiting"));
        assert_eq!(h.call("10", "library_suggest", &json!({ "citekey": "smith2020graph", "kind": "delete", "value": "x" }))["isError"], true);
        let entries = crate::librarian::Library::open(&h.root).unwrap().list().unwrap();
        let smith = entries.iter().find(|e| e.citekey == "smith2020graph").unwrap();
        assert!(smith.tags.is_empty());
        assert_eq!(smith.suggestions.len(), 1);
    }

    #[test]
    fn the_library_tools_see_only_their_own_project_and_stay_inside_it() {
        let (_d, h) = host("library-bounds");
        with_library(&h);
        // A reference of another project's library is not reachable by citekey.
        let other = TempDir::new("copilot", "library-bounds-other");
        let other_root = other.path().join("proj");
        std::fs::create_dir_all(&other_root).unwrap();
        crate::librarian::Library::open(&other_root).unwrap().add(&json!({"title": "Secret elsewhere", "author": [{"family": "Hidden"}], "issued": {"date-parts": [[2001]]}}), "manual").unwrap();
        assert_eq!(h.call("1", "library_get", &json!({ "citekey": "hidden2001secret" }))["isError"], true);
        assert!(text_of(&h.call("2", "library_search", &json!({ "query": "secret" }))).contains("No reference"));
        // A draft path may not leave the read folders, by `..`, by an absolute path or by a link.
        std::fs::create_dir_all(h.root.join("drafts")).unwrap();
        std::fs::write(other.path().join("outside.md"), "Jones (2021)").unwrap();
        for path in ["../outside.md", "/etc/passwd", ".omp/RULES.md", "data/../../outside.md", ".npi-academic/research.db"] {
            assert_eq!(h.call("3", "library_check", &json!({ "path": path }))["isError"], true, "{path}");
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(other.path().join("outside.md"), h.root.join("drafts/leak.md")).unwrap();
            assert_eq!(h.call("4", "library_check", &json!({ "path": "drafts/leak.md" }))["isError"], true);
        }
        // Nothing but suggestions is written, and a suggestion for an unknown reference is refused.
        assert_eq!(h.call("5", "library_suggest", &json!({ "citekey": "../x", "kind": "tag", "value": "x" }))["isError"], true);
        assert!(!h.root.join("papers/x").exists());
    }

    #[test]
    fn the_chat_hub_library_tools_take_a_plain_project_name_only() {
        let d = TempDir::new("copilot", "hub-library");
        let space = std::sync::Arc::new(crate::academic_space::AcademicSpace::new(d.path().join("Academic")));
        std::fs::create_dir_all(d.path().join("Academic/Thesis")).unwrap();
        std::fs::create_dir_all(d.path().join("outside")).unwrap();
        space.prepare(&d.path().join("Academic/Thesis")).unwrap();
        let hub = ResearchHub::new(space);
        for tool in ["academic_library_search", "academic_library_get", "academic_library_check"] {
            for project in ["../outside", "Thesis/notes", ".hidden", "", "missing", "/etc"] {
                let args = json!({ "project": project, "query": "x", "citekey": "x", "text": "x" });
                assert_eq!(hub.call("1", tool, &args)["isError"], true, "{tool} {project:?}");
            }
        }
        for tool in ["library_suggest", "library_search", "research_save", "research_read", "research_check_quote"] {
            assert_eq!(hub.call("2", tool, &json!({ "project": "Thesis" }))["isError"], true, "{tool} is not the chat's");
        }
    }

    #[test]
    fn search_ranks_passages_by_terms_and_reports_file_and_line() {
        let (_d, h) = host("search");
        std::fs::write(
            h.root.join("notes/a.md"),
            "intro\n\nattention is all you need\ntransformer model\n\nother",
        )
        .unwrap();
        std::fs::write(h.root.join("notes/b.md"), "only transformer here").unwrap();
        let f = h.call(
            "1",
            "research_search",
            &json!({ "query": "attention transformer" }),
        );
        assert_eq!(f["isError"], false);
        let t = text_of(&f);
        assert_eq!(
            t.lines()
                .next()
                .unwrap()
                .split(':')
                .take(2)
                .collect::<Vec<_>>(),
            ["notes/a.md", "3"]
        );
        assert!(t.contains("notes/b.md:1"));
        assert!(
            text_of(&h.call("2", "research_search", &json!({ "query": "zzzz" })))
                .contains("No passage")
        );
    }

    #[test]
    fn search_does_not_look_in_the_rules_or_outside_the_read_folders() {
        let (_d, h) = host("scope");
        std::fs::write(h.root.join(".omp/RULES.md"), "secretword").unwrap();
        std::fs::write(h.root.join("stray.md"), "secretword").unwrap();
        assert!(
            text_of(&h.call("1", "research_search", &json!({ "query": "secretword" })))
                .contains("No passage")
        );
    }

    #[test]
    fn the_paper_is_read_but_not_its_git_history() {
        let (_d, h) = host("write-read");
        std::fs::create_dir_all(h.root.join("write/.git/refs")).unwrap();
        std::fs::create_dir_all(h.root.join("write/markdown")).unwrap();
        std::fs::write(h.root.join("write/.git/refs/notes.md"), "hidden needle").unwrap();
        std::fs::write(h.root.join("write/markdown/01-intro.md"), "visible needle").unwrap();
        let found = text_of(&h.call("1", "research_search", &json!({ "query": "needle" })));
        assert!(found.contains("01-intro.md") && !found.contains(".git"), "{found}");
    }

    #[test]
    fn read_refuses_absolute_dotdot_hidden_and_symlink_paths() {
        let (d, h) = host("read");
        std::fs::write(d.path().join("outside.md"), "x").unwrap();
        std::fs::write(h.root.join("notes/ok.md"), "one\ntwo").unwrap();
        for bad in [
            "/etc/passwd",
            "../outside.md",
            "notes/../../outside.md",
            ".omp/RULES.md",
            "stray.md",
        ] {
            assert_eq!(
                h.call("1", "research_read", &json!({ "path": bad }))["isError"],
                true,
                "{bad}"
            );
        }
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(d.path().join("outside.md"), h.root.join("notes/link.md"))
                .unwrap();
            assert_eq!(
                h.call("1", "research_read", &json!({ "path": "notes/link.md" }))["isError"],
                true
            );
        }
        assert_eq!(
            text_of(&h.call(
                "1",
                "research_read",
                &json!({ "path": "notes/ok.md", "from_line": 2 })
            )),
            "two"
        );
        assert!(
            text_of(&h.call("1", "research_read", &json!({ "path": "notes" }))).contains("ok.md")
        );
    }

    #[test]
    fn a_pdf_is_listed_read_and_searched_through_its_extracted_text() {
        let (_d, h) = host("pdf");
        std::fs::write(h.root.join("papers/p.pdf"), crate::pdf_text::tiny_pdf("Transformers replace recurrence")).unwrap();
        assert!(text_of(&h.call("1", "research_read", &json!({ "path": "papers" }))).contains("p.pdf"));
        let read = text_of(&h.call("1", "research_read", &json!({ "path": "papers/p.pdf" })));
        assert!(read.contains("Transformers replace recurrence"), "{read}");
        let found = text_of(&h.call("2", "research_search", &json!({ "query": "recurrence" })));
        assert!(found.starts_with("papers/p.pdf:"), "{found}");
        // The text is cached inside the project, never beside the paper.
        assert!(h.root.join(".npi-academic/cache/pdf").is_dir());
        assert!(!h.root.join("papers").read_dir().unwrap().flatten().any(|e| e.file_name() != "p.pdf"));
    }

    #[test]
    fn a_broken_pdf_is_an_error_and_never_hides_other_hits() {
        let (_d, h) = host("pdf-broken");
        std::fs::write(h.root.join("papers/bad.pdf"), "%PDF-1.4 junk").unwrap();
        std::fs::write(h.root.join("notes/n.md"), "recurrence notes").unwrap();
        assert_eq!(h.call("1", "research_read", &json!({ "path": "papers/bad.pdf" }))["isError"], true);
        let found = text_of(&h.call("2", "research_search", &json!({ "query": "recurrence" })));
        assert!(found.starts_with("notes/n.md:"), "{found}");
    }

    #[test]
    fn one_search_extracts_only_a_few_new_pdfs_and_says_so() {
        let (_d, h) = host("pdf-budget");
        for i in 0..=MAX_FRESH_PDFS + 1 {
            std::fs::write(h.root.join(format!("papers/p{i}.pdf")), crate::pdf_text::tiny_pdf(&format!("alpha {i}"))).unwrap();
        }
        let first = text_of(&h.call("1", "research_search", &json!({ "query": "alpha", "limit": 20 })));
        assert!(first.contains("were not searched yet"), "{first}");
        let again = text_of(&h.call("2", "research_search", &json!({ "query": "alpha", "limit": 20 })));
        let again2 = text_of(&h.call("3", "research_search", &json!({ "query": "alpha", "limit": 20 })));
        assert!(!again2.contains("were not searched yet"), "{again}\n{again2}");
        assert_eq!(again2.lines().filter(|l| l.starts_with("papers/")).count(), MAX_FRESH_PDFS + 2);
    }

    #[test]
    fn save_writes_only_derived_folders_and_never_overwrites_by_default() {
        let (_d, h) = host("save");
        let ok = h.call(
            "1",
            "research_save",
            &json!({ "kind": "digest", "name": "a.md", "content": "hi" }),
        );
        assert_eq!(ok["isError"], false);
        assert_eq!(
            std::fs::read_to_string(h.root.join("digests/a.md")).unwrap(),
            "hi"
        );
        assert_eq!(
            h.call(
                "2",
                "research_save",
                &json!({ "kind": "digest", "name": "a.md", "content": "no" })
            )["isError"],
            true
        );
        assert_eq!(
            std::fs::read_to_string(h.root.join("digests/a.md")).unwrap(),
            "hi"
        );
        assert_eq!(
            h.call(
                "3",
                "research_save",
                &json!({ "kind": "digest", "name": "a.md", "content": "yes", "overwrite": true })
            )["isError"],
            false
        );
        for (kind, name) in [
            ("papers", "a.md"),
            ("note", "../a.md"),
            ("note", "a/b.md"),
            ("note", ".hidden.md"),
            ("note", "a.exe"),
            ("note", ""),
        ] {
            assert_eq!(
                h.call(
                    "4",
                    "research_save",
                    &json!({ "kind": kind, "name": name, "content": "x" })
                )["isError"],
                true,
                "{kind}/{name}"
            );
        }
        assert!(!h.root.join("papers/a.md").exists());
    }

    #[test]
    fn the_chat_hub_lists_searches_and_reads_by_project_name_and_writes_nothing() {
        let d = TempDir::new("copilot", "hub");
        let space = std::sync::Arc::new(crate::academic_space::AcademicSpace::new(
            d.path().join("Academic"),
        ));
        std::fs::create_dir_all(d.path().join("Academic/Thesis/notes")).unwrap();
        std::fs::create_dir_all(d.path().join("Academic/.hidden")).unwrap();
        space.prepare(&d.path().join("Academic/Thesis")).unwrap();
        std::fs::write(
            d.path().join("Academic/Thesis/notes/a.md"),
            "attention is all you need",
        )
        .unwrap();
        let hub = ResearchHub::new(space);
        let names: Vec<_> = hub
            .definitions()
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(
            names,
            [
                "academic_projects", "academic_search", "academic_read", "academic_ocr", "academic_library_search",
                "academic_library_get", "academic_library_check"
            ]
        );
        with_library(&CopilotHost::new(d.path().join("Academic/Thesis").canonicalize().unwrap()));
        let lib = text_of(&hub.call("l1", "academic_library_search", &json!({ "project": "Thesis", "query": "graph" })));
        assert!(lib.contains("[smith2020graph]"), "{lib}");
        assert!(text_of(&hub.call("l2", "academic_library_get", &json!({ "project": "Thesis", "citekey": "smith2020graph" }))).contains("reference (apa)"));
        assert!(text_of(&hub.call("l3", "academic_library_check", &json!({ "project": "Thesis", "text": "Jones (2021)" }))).contains("not-in-library"));
        // The main chat cannot suggest anything: that tool is the Copilot's.
        assert_eq!(hub.call("l4", "library_suggest", &json!({ "citekey": "smith2020graph", "kind": "tag", "value": "x" }))["isError"], true);
        assert_eq!(
            text_of(&hub.call("1", "academic_projects", &json!({}))),
            "Thesis"
        );
        let hit = hub.call(
            "2",
            "academic_search",
            &json!({ "project": "Thesis", "query": "attention" }),
        );
        assert!(text_of(&hit).starts_with("notes/a.md:1"));
        assert_eq!(
            text_of(&hub.call(
                "3",
                "academic_read",
                &json!({ "project": "Thesis", "path": "notes/a.md" })
            )),
            "attention is all you need"
        );
        for project in ["../Thesis", "Thesis/notes", ".hidden", "missing", ""] {
            assert_eq!(
                hub.call(
                    "4",
                    "academic_search",
                    &json!({ "project": project, "query": "x" })
                )["isError"],
                true,
                "{project}"
            );
        }
        assert_eq!(
            hub.call(
                "5",
                "research_save",
                &json!({ "kind": "note", "name": "x.md", "content": "x" })
            )["isError"],
            true
        );
    }

    #[test]
    fn an_indexed_project_searches_the_index_and_checks_quotes() {
        let (_d, h) = host("indexed");
        std::fs::write(h.root.join("papers/p.pdf"), crate::pdf_text::pages_pdf(&["First page", "Transformers replace recurrence"])).unwrap();
        std::fs::write(h.root.join("notes/n.md"), "综述文献检索方法\n".as_bytes()).unwrap();
        // No index yet: the plain scan answers (no page, no anchor).
        let scan = text_of(&h.call("1", "research_search", &json!({ "query": "recurrence" })));
        assert!(scan.starts_with("papers/p.pdf:"), "{scan}");
        let before = text_of(&h.call("2", "research_check_quote", &json!({ "path": "papers/p.pdf", "page": 2, "quote": "replace  RECURRENCE" })));
        assert!(before.starts_with("matched: the quote is on papers/p.pdf p.2 (anchor "), "{before}");
        // Now the project has an index: hits show the page and an anchor, and short Chinese words are found.
        let found = text_of(&h.call("3", "research_search", &json!({ "query": "recurrence" })));
        assert!(found.starts_with("papers/p.pdf p.2 [anchor "), "{found}");
        assert!(text_of(&h.call("4", "research_search", &json!({ "query": "文献" }))).starts_with("notes/n.md [anchor "));
        assert!(text_of(&h.call("5", "research_check_quote", &json!({ "path": "papers/p.pdf", "page": 1, "quote": "recurrence" }))).starts_with("not found"));
        assert_eq!(h.call("6", "research_check_quote", &json!({ "path": "papers/p.pdf", "quote": "recurrence" }))["isError"], true);
        assert_eq!(h.call("7", "research_check_quote", &json!({ "path": "../p.pdf", "page": 1, "quote": "x" }))["isError"], true);
        assert_eq!(h.call("8", "research_check_quote", &json!({ "path": ".npi-academic/research.db", "quote": "x" }))["isError"], true);
        // A changed paper is reported stale until the next search re-indexes it.
        std::fs::write(h.root.join("papers/p.pdf"), crate::pdf_text::pages_pdf(&["First page changed", "Other words entirely here"])).unwrap();
        let stale = text_of(&h.call("9", "research_check_quote", &json!({ "path": "papers/p.pdf", "page": 2, "quote": "recurrence" })));
        assert!(stale.starts_with("stale:"), "{stale}");
        let fresh = text_of(&h.call("10", "research_search", &json!({ "query": "entirely" })));
        assert!(fresh.contains("p.2"), "{fresh}");
        // The library search uses the index too (a two-character Chinese title word).
        let lib = crate::librarian::Library::open(&h.root).unwrap();
        lib.add(&json!({"title": "文献检索综述", "type": "book"}), "manual").unwrap();
        let refs = text_of(&h.call("11", "library_search", &json!({ "query": "文献" })));
        assert!(refs.contains("文献检索综述"), "{refs}");
    }

    #[test]
    fn an_unknown_tool_is_an_error_frame_for_the_same_call() {
        let (_d, h) = host("unknown");
        let f = h.call("42", "bash", &json!({}));
        assert_eq!(
            (f["id"].as_str(), f["isError"].as_bool()),
            (Some("42"), Some(true))
        );
    }
}
