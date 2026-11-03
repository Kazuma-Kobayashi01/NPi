//! The Research Core's index (`docs/PLAN_3.0.md` sections 4, 7 and 8): the
//! papers of a project cut into blocks with page-level anchors, a full-text
//! index over them and over the library's references, and a deterministic
//! quote check. It lives in the project's `research.db` beside the library
//! (schema v5). The files it reads stay as they are; everything it writes is
//! inside `.npi-academic/research.db`.
//!
//! Only the search index (`search_units`, `search_fts`) is rebuildable. Blocks
//! and anchors are derived from the files too, but a citation points at an
//! anchor id, so a changed file marks its old anchors `stale` instead of
//! deleting them (see `docs/architecture/research-core.md`).

use super::cjk;
use super::csl;
use super::store::{now, sql, Library};
use super::Entry;
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// Papers, their sections, blocks and anchors, and the search index.
pub(super) const SCHEMA_V5: &str = "
CREATE TABLE papers (
    id           INTEGER PRIMARY KEY,
    path         TEXT NOT NULL UNIQUE,
    kind         TEXT NOT NULL,
    file_hash    TEXT NOT NULL,
    file_size    INTEGER NOT NULL,
    file_mtime   INTEGER NOT NULL,
    title        TEXT NOT NULL DEFAULT '',
    authors      TEXT NOT NULL DEFAULT '',
    year         INTEGER,
    doi          TEXT,
    parse_status TEXT NOT NULL,
    parse_note   TEXT NOT NULL DEFAULT '',
    page_count   INTEGER NOT NULL DEFAULT 0,
    ref_id       INTEGER,
    indexed_at   INTEGER NOT NULL
);
CREATE INDEX papers_hash ON papers(file_hash);
CREATE TABLE sections (
    id       INTEGER PRIMARY KEY,
    paper_id INTEGER NOT NULL,
    ordinal  INTEGER NOT NULL,
    heading  TEXT NOT NULL,
    level    INTEGER NOT NULL
);
CREATE INDEX sections_paper ON sections(paper_id, ordinal);
CREATE TABLE blocks (
    id         INTEGER PRIMARY KEY,
    paper_id   INTEGER NOT NULL,
    section_id INTEGER,
    ordinal    INTEGER NOT NULL,
    kind       TEXT NOT NULL,
    text       TEXT NOT NULL,
    page       INTEGER
);
CREATE INDEX blocks_paper ON blocks(paper_id, page, ordinal);
CREATE TABLE anchors (
    id              INTEGER PRIMARY KEY,
    paper_id        INTEGER NOT NULL,
    block_id        INTEGER,
    file_hash       TEXT NOT NULL,
    page            INTEGER,
    exact_quote     TEXT NOT NULL,
    quote_hash      TEXT NOT NULL,
    locator_quality TEXT NOT NULL,
    stale           INTEGER NOT NULL DEFAULT 0,
    created_at      INTEGER NOT NULL
);
CREATE INDEX anchors_block ON anchors(block_id);
CREATE INDEX anchors_paper ON anchors(paper_id, stale);
CREATE TABLE research_meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
CREATE TABLE search_units (
    id         INTEGER PRIMARY KEY,
    kind       TEXT NOT NULL,
    block_id   INTEGER,
    ref_id     INTEGER,
    norm_title TEXT NOT NULL,
    norm_text  TEXT NOT NULL
);
CREATE INDEX search_units_block ON search_units(block_id);
CREATE INDEX search_units_ref ON search_units(ref_id);
CREATE VIRTUAL TABLE search_fts USING fts5(
    norm_title, norm_text,
    content='search_units', content_rowid='id',
    tokenize='unicode61 remove_diacritics 2'
);
CREATE TRIGGER search_units_ai AFTER INSERT ON search_units BEGIN
    INSERT INTO search_fts(rowid, norm_title, norm_text) VALUES (new.id, new.norm_title, new.norm_text);
END;
CREATE TRIGGER search_units_ad AFTER DELETE ON search_units BEGIN
    INSERT INTO search_fts(search_fts, rowid, norm_title, norm_text) VALUES ('delete', old.id, old.norm_title, old.norm_text);
END;
CREATE TRIGGER search_units_au AFTER UPDATE ON search_units BEGIN
    INSERT INTO search_fts(search_fts, rowid, norm_title, norm_text) VALUES ('delete', old.id, old.norm_title, old.norm_text);
    INSERT INTO search_fts(rowid, norm_title, norm_text) VALUES (new.id, new.norm_title, new.norm_text);
END;
";

const PDF_DIR: &str = "papers";
/// Text files are indexed from these folders too (`IndexOptions::text`).
const TEXT_DIRS: &[&str] = &["notes", "digests", "drafts"];
const TEXT_EXTS: &[&str] = &["md", "txt", "tex"];
const MAX_TEXT_BYTES: u64 = 2 * 1024 * 1024;
const MAX_FILES: usize = 2000;
/// A longer paragraph is cut into several blocks.
const MAX_BLOCK_CHARS: usize = 2000;
const SNIPPET_CHARS: usize = 240;

#[derive(Debug, Clone, Copy)]
pub struct IndexOptions {
    /// Also index the text files of `notes/`, `digests/` and `drafts/`.
    pub text: bool,
    /// Most PDFs parsed in this call (a parse takes seconds); the rest wait for the next call.
    pub max_new_pdfs: Option<usize>,
}

impl Default for IndexOptions {
    fn default() -> Self {
        Self { text: true, max_new_pdfs: None }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexReport {
    pub added: usize,
    pub updated: usize,
    pub unchanged: usize,
    /// Indexed files that are no longer on disk.
    pub removed: usize,
    /// PDFs not parsed yet because of `max_new_pdfs`.
    pub waiting: usize,
    /// PDFs without a text layer (they need OCR; nothing was guessed).
    pub scanned: Vec<String>,
    /// Files that could not be read, with the reason.
    pub failed: Vec<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexStatus {
    /// An index has been built for this project.
    pub indexed: bool,
    pub papers: usize,
    pub blocks: usize,
    pub anchors: usize,
    pub stale_anchors: usize,
    pub scanned: Vec<String>,
    pub partial: Vec<String>,
    pub failed: Vec<String>,
    /// Indexed files that changed or vanished since (re-index to refresh them).
    pub stale_files: Vec<String>,
    /// Files in the project folders that are not indexed yet.
    pub unindexed: usize,
    /// Empty when the search index agrees with the blocks and references.
    pub problems: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Hit {
    pub paper_id: i64,
    pub path: String,
    pub page: Option<u32>,
    pub section: String,
    pub block_id: i64,
    pub anchor_id: Option<i64>,
    /// `original` (papers), `digest`, `note` or `draft`.
    pub source_level: &'static str,
    pub score: f64,
    pub snippet: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "verdict")]
pub enum QuoteCheck {
    /// The quote is on that page of the indexed text.
    Matched { block_id: i64, anchor_id: Option<i64>, page: Option<u32> },
    NotFound,
    /// The file changed (or is gone) since it was indexed: the indexed text no longer vouches for it.
    Stale,
    /// Indexed, but without searchable text (`scanned`, `failed`).
    Unreadable { status: String },
}

// ---------------------------------------------------------------- files

struct Found {
    rel: String,
    path: PathBuf,
    pdf: bool,
}

fn is_ext(path: &Path, exts: &[&str]) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| exts.contains(&e.to_ascii_lowercase().as_str()))
}

/// The indexable files of `project`: PDFs under `papers/`, text under the other folders.
/// A symlink is never followed.
fn discover(project: &Path, text: bool) -> Vec<Found> {
    let mut out = Vec::new();
    let mut dirs: Vec<(PathBuf, String, bool)> = vec![(project.join(PDF_DIR), PDF_DIR.into(), true)];
    if text {
        dirs.extend(TEXT_DIRS.iter().map(|d| (project.join(d), (*d).to_string(), false)));
    }
    for (root, root_rel, pdf) in dirs {
        if std::fs::symlink_metadata(&root).map_or(true, |m| !m.is_dir()) {
            continue;
        }
        let mut stack = vec![(root, root_rel)];
        while let Some((dir, rel)) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else { continue };
            for entry in entries.flatten() {
                let Ok(kind) = entry.file_type() else { continue };
                let name = entry.file_name().to_string_lossy().into_owned();
                if kind.is_symlink() || name.starts_with('.') {
                    continue;
                }
                let child = format!("{rel}/{name}");
                if kind.is_dir() {
                    stack.push((entry.path(), child));
                } else if out.len() < MAX_FILES
                    && is_ext(&entry.path(), if pdf { &["pdf"] } else { TEXT_EXTS })
                {
                    out.push(Found { rel: child, path: entry.path(), pdf });
                }
            }
        }
    }
    out.sort_by(|a, b| a.rel.cmp(&b.rel));
    out
}

/// `rel` as an indexed file of `project`: plain components, under an indexed folder, no symlink on
/// the way, and a file.
fn safe_file(project: &Path, rel: &str) -> Result<PathBuf, String> {
    let rel_path = Path::new(rel);
    let parts: Vec<&str> = rel_path
        .components()
        .map(|c| match c {
            std::path::Component::Normal(s) => s.to_str().ok_or(()),
            _ => Err(()),
        })
        .collect::<Result<_, _>>()
        .map_err(|()| "use a plain project-relative path".to_string())?;
    if parts.len() < 2 || !(parts[0] == PDF_DIR || TEXT_DIRS.contains(&parts[0])) {
        return Err(format!("only files under {PDF_DIR}, {} are indexed", TEXT_DIRS.join(", ")));
    }
    let mut path = project.to_path_buf();
    for part in &parts {
        path.push(part);
        if std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err("a link is not followed".into());
        }
    }
    Ok(path)
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

fn mtime(meta: &std::fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0))
}

// ---------------------------------------------------------------- blocks

struct Raw {
    page: Option<u32>,
    kind: &'static str,
    text: String,
    /// Heading level; 0 for anything else.
    level: usize,
}

struct Parsed {
    status: &'static str,
    note: String,
    page_count: u32,
    title: String,
    authors: String,
    year: Option<i32>,
    doi: Option<String>,
    blocks: Vec<Raw>,
}

impl Parsed {
    fn empty(status: &'static str, note: String) -> Self {
        Self { status, note, page_count: 0, title: String::new(), authors: String::new(), year: None, doi: None, blocks: Vec::new() }
    }
}

/// `text` cut into pieces of at most [`MAX_BLOCK_CHARS`], at a sentence end or a space when it can.
fn split_long(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut from = 0;
    while from < chars.len() {
        let mut to = (from + MAX_BLOCK_CHARS).min(chars.len());
        if to < chars.len() {
            let window = &chars[from + MAX_BLOCK_CHARS / 2..to];
            let cut = window
                .iter()
                .rposition(|c| matches!(c, '.' | '。' | '!' | '?' | '\n'))
                .or_else(|| window.iter().rposition(|c| c.is_whitespace()));
            if let Some(i) = cut {
                to = from + MAX_BLOCK_CHARS / 2 + i + 1;
            }
        }
        let piece: String = chars[from..to].iter().collect();
        if !piece.trim().is_empty() {
            out.push(piece.trim().to_string());
        }
        from = to;
    }
    out
}

fn caption_like(text: &str) -> bool {
    let lower = text.to_lowercase();
    for prefix in ["figure", "fig.", "fig ", "table", "图", "表"] {
        if let Some(rest) = lower.strip_prefix(prefix) {
            let rest = rest.trim_start();
            if rest.chars().next().is_some_and(|c| c.is_ascii_digit() || "一二三四五六七八九十".contains(c)) {
                return true;
            }
        }
    }
    false
}

fn flush(lines: &mut Vec<&str>, table: &mut bool, page: Option<u32>, out: &mut Vec<Raw>) {
    let text = lines.join("\n").trim().to_string();
    let kind = if *table {
        "table"
    } else if text.starts_with("$$") || text.starts_with("\\[") {
        "formula"
    } else if caption_like(&text) {
        "caption"
    } else {
        "paragraph"
    };
    lines.clear();
    *table = false;
    if !text.is_empty() {
        out.extend(split_long(&text).into_iter().map(|text| Raw { page, kind, text, level: 0 }));
    }
}

/// Markdown (a PDF page's, or a note's) cut into headings, paragraphs, tables, formulas and captions.
fn blocks_of(md: &str, page: Option<u32>, out: &mut Vec<Raw>) {
    let mut lines: Vec<&str> = Vec::new();
    let mut table = false;
    for line in md.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            flush(&mut lines, &mut table, page, out);
        } else if trimmed.starts_with('#') {
            flush(&mut lines, &mut table, page, out);
            let level = trimmed.chars().take_while(|c| *c == '#').count();
            let heading = trimmed.trim_start_matches('#').trim();
            if !heading.is_empty() {
                out.push(Raw { page, kind: "heading", text: heading.to_string(), level });
            }
        } else {
            let row = trimmed.starts_with('|');
            if !lines.is_empty() && row != table {
                flush(&mut lines, &mut table, page, out);
            }
            table = row;
            lines.push(trimmed);
        }
    }
    flush(&mut lines, &mut table, page, out);
}

fn parse_pdf(bytes: &[u8]) -> Parsed {
    match crate::pdf_text::pages(bytes) {
        Err(e) => Parsed::empty("failed", e),
        Ok(crate::pdf_text::Paged::Scanned) => {
            Parsed::empty("scanned", "no text layer; OCR is not available".into())
        }
        Ok(crate::pdf_text::Paged::Pages { pages, ocr_pages, page_count }) => {
            let mut blocks = Vec::new();
            for (page, md) in &pages {
                blocks_of(md, Some(*page), &mut blocks);
            }
            let guess = super::pdf_meta::guess(&pages[0].1);
            let f = csl::fields(&guess.csl);
            let authors = guess
                .csl
                .get("author")
                .and_then(|a| a.as_array())
                .map(|a| a.iter().map(csl::person_text).filter(|n| !n.is_empty()).collect::<Vec<_>>().join("; "))
                .unwrap_or_default();
            let (status, note) = if ocr_pages.is_empty() {
                ("ok", String::new())
            } else {
                let list: Vec<String> = ocr_pages.iter().map(u32::to_string).collect();
                ("partial", format!("pages without a reliable text layer, not indexed: {}", list.join(", ")))
            };
            Parsed { status, note, page_count, title: f.title, authors, year: f.year, doi: f.doi, blocks }
        }
    }
}

fn parse_text(bytes: &[u8], rel: &str) -> Parsed {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return Parsed::empty("failed", "not UTF-8 text".into());
    };
    let mut blocks = Vec::new();
    blocks_of(text, None, &mut blocks);
    let title = blocks
        .iter()
        .find(|b| b.kind == "heading")
        .map_or_else(
            || Path::new(rel).file_stem().and_then(|s| s.to_str()).unwrap_or(rel).to_string(),
            |b| b.text.clone(),
        );
    Parsed { title, blocks, ..Parsed::empty("ok", String::new()) }
}

// ---------------------------------------------------------------- the index

struct Known {
    id: i64,
    hash: String,
    size: i64,
    mtime: i64,
    missing: bool,
}

impl Library {
    /// Brings the index of `project`'s `papers/` (and, by `opts`, its text folders) up to date:
    /// new and changed files are parsed again, files that vanished are dropped from the blocks.
    pub fn index_project(&self, project: &Path, opts: IndexOptions) -> Result<IndexReport, String> {
        self.ensure_norm()?;
        let mut report = IndexReport::default();
        let mut known: HashMap<String, Known> = HashMap::new();
        {
            let mut stmt = self
                .conn
                .prepare("SELECT path, id, file_hash, file_size, file_mtime, parse_status = 'missing' FROM papers")
                .map_err(sql)?;
            let rows = stmt
                .query_map([], |r| Ok((r.get::<_, String>(0)?, Known { id: r.get(1)?, hash: r.get(2)?, size: r.get(3)?, mtime: r.get(4)?, missing: r.get(5)? })))
                .map_err(sql)?;
            for row in rows {
                let (path, k) = row.map_err(sql)?;
                known.insert(path, k);
            }
        }
        let mut seen: HashSet<String> = HashSet::new();
        let mut fresh_pdfs = 0usize;
        for file in discover(project, opts.text) {
            seen.insert(file.rel.clone());
            let Ok(meta) = std::fs::symlink_metadata(&file.path) else { continue };
            let (size, modified) = (i64::try_from(meta.len()).unwrap_or(i64::MAX), mtime(&meta));
            let old = known.get(&file.rel);
            if old.is_some_and(|k| k.size == size && k.mtime == modified && !k.missing) {
                report.unchanged += 1;
                continue;
            }
            let limit = if file.pdf { crate::pdf_text::MAX_PDF_BYTES } else { MAX_TEXT_BYTES };
            let (hash, parsed) = if meta.len() > limit {
                (String::new(), Parsed::empty("failed", "the file is too large".into()))
            } else {
                let Ok(bytes) = std::fs::read(&file.path) else { continue };
                let hash = hex(&Sha256::digest(&bytes));
                if old.is_some_and(|k| k.hash == hash && !k.missing) {
                    self.conn
                        .execute("UPDATE papers SET file_mtime = ?2 WHERE path = ?1", params![file.rel, modified])
                        .map_err(sql)?;
                    report.unchanged += 1;
                    continue;
                }
                if file.pdf && opts.max_new_pdfs.is_some_and(|max| fresh_pdfs >= max) {
                    report.waiting += 1;
                    continue;
                }
                if file.pdf {
                    fresh_pdfs += 1;
                }
                let parsed = if file.pdf { parse_pdf(&bytes) } else { parse_text(&bytes, &file.rel) };
                (hash, parsed)
            };
            match parsed.status {
                "scanned" => report.scanned.push(file.rel.clone()),
                "failed" => report.failed.push(format!("{}: {}", file.rel, parsed.note)),
                _ => {}
            }
            self.store_paper(&file, old.map(|k| k.id), &hash, size, modified, &parsed)?;
            if old.is_some() {
                report.updated += 1;
            } else {
                report.added += 1;
            }
        }
        for (path, k) in &known {
            // A run over PDFs only does not judge the text folders.
            if !seen.contains(path) && !k.missing && (opts.text || path.starts_with("papers/")) {
                self.drop_blocks(k.id)?;
                self.conn
                    .execute("UPDATE papers SET parse_status = 'missing', parse_note = 'the file is gone', indexed_at = ?2 WHERE id = ?1", params![k.id, now()])
                    .map_err(sql)?;
                report.removed += 1;
            }
        }
        self.conn
            .execute(
                "UPDATE papers SET ref_id = (SELECT a.ref_id FROM attachments a JOIN refs r ON r.id = a.ref_id AND r.deleted_at IS NULL
                 WHERE a.hash = papers.file_hash ORDER BY a.id LIMIT 1)",
                [],
            )
            .map_err(sql)?;
        self.sync_refs(&self.list()?)?;
        self.set_meta_value("indexed_at", &now().to_string())?;
        self.set_meta_value("norm_version", cjk::VERSION)?;
        Ok(report)
    }

    /// Drops a paper's blocks and search units and marks its anchors stale (the anchors stay,
    /// so a citation that used one can say the text changed).
    fn drop_blocks(&self, paper_id: i64) -> Result<(), String> {
        let steps = [
            "UPDATE anchors SET stale = 1, block_id = NULL WHERE paper_id = ?1",
            "DELETE FROM search_units WHERE block_id IN (SELECT id FROM blocks WHERE paper_id = ?1)",
            "DELETE FROM blocks WHERE paper_id = ?1",
            "DELETE FROM sections WHERE paper_id = ?1",
        ];
        for step in steps {
            self.conn.execute(step, [paper_id]).map_err(sql)?;
        }
        Ok(())
    }

    fn store_paper(&self, file: &Found, old: Option<i64>, hash: &str, size: i64, modified: i64, p: &Parsed) -> Result<(), String> {
        let tx = self.conn.unchecked_transaction().map_err(sql)?;
        let at = now();
        let kind = if file.pdf { "pdf" } else { "text" };
        let paper_id = if let Some(id) = old {
            self.drop_blocks(id)?;
            tx.execute(
                "UPDATE papers SET kind = ?2, file_hash = ?3, file_size = ?4, file_mtime = ?5, title = ?6, authors = ?7, year = ?8, doi = ?9,
                 parse_status = ?10, parse_note = ?11, page_count = ?12, indexed_at = ?13 WHERE id = ?1",
                params![id, kind, hash, size, modified, p.title, p.authors, p.year, p.doi, p.status, p.note, p.page_count, at],
            )
            .map_err(sql)?;
            id
        } else {
            tx.execute(
                "INSERT INTO papers (path, kind, file_hash, file_size, file_mtime, title, authors, year, doi, parse_status, parse_note, page_count, indexed_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                params![file.rel, kind, hash, size, modified, p.title, p.authors, p.year, p.doi, p.status, p.note, p.page_count, at],
            )
            .map_err(sql)?;
            tx.last_insert_rowid()
        };
        let quality = if file.pdf { "page" } else { "block" };
        let (mut section, mut heading, mut ordinal, mut sections) = (None::<i64>, String::new(), 0i64, 0i64);
        for b in &p.blocks {
            if b.kind == "heading" {
                sections += 1;
                tx.execute(
                    "INSERT INTO sections (paper_id, ordinal, heading, level) VALUES (?1, ?2, ?3, ?4)",
                    params![paper_id, sections, b.text, i64::try_from(b.level).unwrap_or(6)],
                )
                .map_err(sql)?;
                section = Some(tx.last_insert_rowid());
                heading = b.text.clone();
            }
            ordinal += 1;
            tx.execute(
                "INSERT INTO blocks (paper_id, section_id, ordinal, kind, text, page) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![paper_id, section, ordinal, b.kind, b.text, b.page],
            )
            .map_err(sql)?;
            let block_id = tx.last_insert_rowid();
            tx.execute(
                "INSERT INTO anchors (paper_id, block_id, file_hash, page, exact_quote, quote_hash, locator_quality, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![paper_id, block_id, hash, b.page, b.text, hex(&Sha256::digest(cjk::quote_form(&b.text).as_bytes())), quality, at],
            )
            .map_err(sql)?;
            tx.execute(
                "INSERT INTO search_units (kind, block_id, norm_title, norm_text) VALUES ('block', ?1, ?2, ?3)",
                params![block_id, cjk::index_text(&heading), cjk::index_text(&b.text)],
            )
            .map_err(sql)?;
        }
        tx.commit().map_err(sql)
    }

    fn meta_value(&self, key: &str) -> Result<Option<String>, String> {
        self.conn
            .query_row("SELECT value FROM research_meta WHERE key = ?1", [key], |r| r.get(0))
            .optional()
            .map_err(sql)
    }

    fn set_meta_value(&self, key: &str, value: &str) -> Result<(), String> {
        self.conn
            .execute("INSERT OR REPLACE INTO research_meta (key, value) VALUES (?1, ?2)", params![key, value])
            .map_err(sql)?;
        Ok(())
    }

    /// An index has been built (the project's files were read at least once).
    pub fn has_index(&self) -> Result<bool, String> {
        Ok(self.meta_value("indexed_at")?.is_some())
    }

    /// Rebuilds the search index when it was made with another version of the split in `cjk.rs`.
    fn ensure_norm(&self) -> Result<(), String> {
        if self.meta_value("norm_version")?.is_some_and(|v| v != cjk::VERSION) {
            self.rebuild_search()?;
        }
        Ok(())
    }

    /// Writes the search units of the references (title; authors, venue, year, DOI, abstract, citekey,
    /// tags and notes) that changed, and drops those of removed references.
    fn sync_refs(&self, entries: &[Entry]) -> Result<(), String> {
        let mut have: HashMap<i64, (i64, String, String)> = HashMap::new();
        {
            let mut stmt = self
                .conn
                .prepare("SELECT ref_id, id, norm_title, norm_text FROM search_units WHERE kind = 'ref'")
                .map_err(sql)?;
            let rows = stmt
                .query_map([], |r| Ok((r.get::<_, i64>(0)?, (r.get::<_, i64>(1)?, r.get::<_, String>(2)?, r.get::<_, String>(3)?))))
                .map_err(sql)?;
            for row in rows {
                let (ref_id, unit) = row.map_err(sql)?;
                have.insert(ref_id, unit);
            }
        }
        let tx = self.conn.unchecked_transaction().map_err(sql)?;
        for e in entries {
            let f = csl::fields(&e.csl);
            let text = |key: &str| e.csl.get(key).and_then(|v| v.as_str()).unwrap_or("").to_string();
            let authors: Vec<String> = e
                .csl
                .get("author")
                .and_then(|a| a.as_array())
                .map(|a| a.iter().map(csl::person_text).collect())
                .unwrap_or_default();
            let body = [
                authors.join(" "),
                f.year.map(|y| y.to_string()).unwrap_or_default(),
                text("container-title"),
                text("publisher"),
                text("DOI"),
                text("abstract"),
                e.citekey.clone(),
                e.tags.join(" "),
                e.notes.clone(),
            ]
            .join("\n");
            let (title, body) = (cjk::index_text(&f.title), cjk::index_text(&body));
            match have.remove(&e.id) {
                Some((_, t, b)) if t == title && b == body => {}
                Some((unit, _, _)) => {
                    tx.execute("UPDATE search_units SET norm_title = ?2, norm_text = ?3 WHERE id = ?1", params![unit, title, body])
                        .map_err(sql)?;
                }
                None => {
                    tx.execute(
                        "INSERT INTO search_units (kind, ref_id, norm_title, norm_text) VALUES ('ref', ?1, ?2, ?3)",
                        params![e.id, title, body],
                    )
                    .map_err(sql)?;
                }
            }
        }
        for (unit, _, _) in have.values() {
            tx.execute("DELETE FROM search_units WHERE id = ?1", [unit]).map_err(sql)?;
        }
        tx.commit().map_err(sql)
    }

    /// Rebuilds every search unit from the blocks and the library and then the FTS index from the
    /// units. Nothing else is touched: this is the one rebuildable part of the Research Core.
    pub fn rebuild_search(&self) -> Result<(), String> {
        let tx = self.conn.unchecked_transaction().map_err(sql)?;
        tx.execute("DELETE FROM search_units", []).map_err(sql)?;
        {
            let mut stmt = tx
                .prepare(
                    "SELECT b.id, b.text, COALESCE(s.heading, '') FROM blocks b LEFT JOIN sections s ON s.id = b.section_id ORDER BY b.id",
                )
                .map_err(sql)?;
            let rows: Vec<(i64, String, String)> = stmt
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
                .map_err(sql)?
                .collect::<Result<_, _>>()
                .map_err(sql)?;
            for (id, text, heading) in rows {
                tx.execute(
                    "INSERT INTO search_units (kind, block_id, norm_title, norm_text) VALUES ('block', ?1, ?2, ?3)",
                    params![id, cjk::index_text(&heading), cjk::index_text(&text)],
                )
                .map_err(sql)?;
            }
        }
        tx.execute("INSERT INTO search_fts(search_fts) VALUES ('rebuild')", []).map_err(sql)?;
        tx.commit().map_err(sql)?;
        self.sync_refs(&self.list()?)?;
        self.set_meta_value("norm_version", cjk::VERSION)
    }

    /// What disagrees between the blocks, the references, the search units and the FTS index.
    pub fn check_search(&self) -> Result<Vec<String>, String> {
        let mut problems = Vec::new();
        if self
            .conn
            .execute("INSERT INTO search_fts(search_fts, rank) VALUES ('integrity-check', 1)", [])
            .is_err()
        {
            problems.push("the full-text index does not match its units".to_string());
        }
        let count = |q: &str| -> Result<i64, String> { self.conn.query_row(q, [], |r| r.get(0)).map_err(sql) };
        if count("SELECT count(*) FROM blocks")? != count("SELECT count(*) FROM search_units WHERE kind = 'block'")? {
            problems.push("some blocks are not in the search index".to_string());
        }
        if count("SELECT count(*) FROM search_units WHERE kind = 'block' AND block_id NOT IN (SELECT id FROM blocks)")? > 0 {
            problems.push("the search index holds blocks that no longer exist".to_string());
        }
        let live = count("SELECT count(*) FROM refs WHERE deleted_at IS NULL")?;
        if self.has_index()? && live != count("SELECT count(*) FROM search_units WHERE kind = 'ref'")? {
            problems.push("some references are not in the search index (search or re-index to refresh)".to_string());
        }
        Ok(problems)
    }

    pub fn index_status(&self, project: &Path) -> Result<IndexStatus, String> {
        let count = |q: &str| -> Result<usize, String> {
            self.conn
                .query_row(q, [], |r| r.get::<_, i64>(0))
                .map(|n| usize::try_from(n).unwrap_or(0))
                .map_err(sql)
        };
        let mut status = IndexStatus {
            indexed: self.has_index()?,
            papers: count("SELECT count(*) FROM papers WHERE parse_status != 'missing'")?,
            blocks: count("SELECT count(*) FROM blocks")?,
            anchors: count("SELECT count(*) FROM anchors WHERE stale = 0")?,
            stale_anchors: count("SELECT count(*) FROM anchors WHERE stale = 1")?,
            ..IndexStatus::default()
        };
        let mut stmt = self
            .conn
            .prepare("SELECT path, parse_status, file_size, file_mtime, file_hash FROM papers ORDER BY path")
            .map_err(sql)?;
        let rows: Vec<(String, String, i64, i64, String)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))
            .map_err(sql)?
            .collect::<Result<_, _>>()
            .map_err(sql)?;
        let mut indexed: HashSet<String> = HashSet::new();
        for (path, parse, size, modified, hash) in rows {
            indexed.insert(path.clone());
            match parse.as_str() {
                "scanned" => status.scanned.push(path.clone()),
                "partial" => status.partial.push(path.clone()),
                "failed" => status.failed.push(path.clone()),
                _ => {}
            }
            let changed = match safe_file(project, &path).ok().and_then(|p| std::fs::metadata(&p).ok().map(|m| (p, m))) {
                None => parse != "missing",
                Some((p, m)) => {
                    parse == "missing"
                        || (i64::try_from(m.len()).unwrap_or(-1) != size
                            || mtime(&m) != modified)
                            && std::fs::read(&p).map_or(true, |b| hex(&Sha256::digest(&b)) != hash)
                }
            };
            if changed {
                status.stale_files.push(path);
            }
        }
        status.unindexed = discover(project, true).iter().filter(|f| !indexed.contains(&f.rel)).count();
        status.problems = self.check_search()?;
        Ok(status)
    }

    // ------------------------------------------------------------ search

    /// The blocks matching `query`, best first. `folder` (for example `papers`) limits the hits to
    /// files under it. The query is split into quoted tokens (`cjk::match_expression`); it is never
    /// passed to FTS as syntax. The database is this project's, and every hit is joined to a paper
    /// of it, so nothing from another project can appear.
    pub fn search_blocks(&self, query: &str, folder: Option<&str>, limit: usize) -> Result<Vec<Hit>, String> {
        let Some(expr) = cjk::match_expression(query) else { return Ok(Vec::new()) };
        self.ensure_norm()?;
        let prefix = folder.map(|f| format!("{}/", f.trim_end_matches('/')));
        let mut stmt = self
            .conn
            .prepare(
                "SELECT p.id, p.path, b.page, COALESCE(s.heading, ''), b.id, a.id, b.text, bm25(search_fts, 4.0, 1.0)
                 FROM search_fts
                 JOIN search_units u ON u.id = search_fts.rowid AND u.kind = 'block'
                 JOIN blocks b ON b.id = u.block_id
                 JOIN papers p ON p.id = b.paper_id
                 LEFT JOIN sections s ON s.id = b.section_id
                 LEFT JOIN anchors a ON a.block_id = b.id AND a.stale = 0
                 WHERE search_fts MATCH ?1 AND (?2 IS NULL OR substr(p.path, 1, length(?2)) = ?2)
                 ORDER BY bm25(search_fts, 4.0, 1.0), p.path, b.id
                 LIMIT ?3",
            )
            .map_err(sql)?;
        let terms = cjk::highlight_terms(query);
        let rows = stmt
            .query_map(params![expr, prefix, i64::try_from(limit).unwrap_or(i64::MAX)], |r| {
                let path: String = r.get(1)?;
                let text: String = r.get(6)?;
                Ok(Hit {
                    paper_id: r.get(0)?,
                    source_level: source_level(&path),
                    path,
                    page: r.get(2)?,
                    section: r.get(3)?,
                    block_id: r.get(4)?,
                    anchor_id: r.get(5)?,
                    score: -r.get::<_, f64>(7)?,
                    snippet: excerpt(&text, &terms),
                })
            })
            .map_err(sql)?;
        rows.collect::<Result<_, _>>().map_err(sql)
    }

    /// Ids of the references of `entries` matching `query`, best first; `None` when the project
    /// has no index yet (the caller scans instead). The references are brought into the index first.
    pub fn search_library(&self, entries: &[Entry], query: &str, limit: usize) -> Result<Option<Vec<i64>>, String> {
        if !self.has_index()? {
            return Ok(None);
        }
        self.ensure_norm()?;
        self.sync_refs(entries)?;
        let Some(expr) = cjk::match_expression(query) else { return Ok(Some(Vec::new())) };
        let mut stmt = self
            .conn
            .prepare(
                "SELECT u.ref_id FROM search_fts JOIN search_units u ON u.id = search_fts.rowid AND u.kind = 'ref'
                 JOIN refs r ON r.id = u.ref_id AND r.deleted_at IS NULL
                 WHERE search_fts MATCH ?1 ORDER BY bm25(search_fts, 4.0, 1.0), r.citekey LIMIT ?2",
            )
            .map_err(sql)?;
        let rows = stmt
            .query_map(params![expr, i64::try_from(limit).unwrap_or(i64::MAX)], |r| r.get(0))
            .map_err(sql)?;
        rows.collect::<Result<_, _>>().map(Some).map_err(sql)
    }

    // ------------------------------------------------------------ quotes

    /// Whether `quote` is on `page` of the indexed text of `rel` (layout, hyphenation and case are
    /// ignored). `page` is required for a PDF and ignored for a text file. A file never indexed is
    /// indexed first; one that changed since is reported `Stale`, not silently re-read.
    pub fn check_quote(&self, project: &Path, rel: &str, page: Option<u32>, quote: &str) -> Result<QuoteCheck, String> {
        let wanted = cjk::quote_form(quote);
        if wanted.is_empty() {
            return Err("the quote is empty".into());
        }
        let file = safe_file(project, rel)?;
        let row = |this: &Self| {
            this.conn
                .query_row(
                    "SELECT id, kind, file_hash, file_size, file_mtime, parse_status FROM papers WHERE path = ?1",
                    [rel],
                    |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, i64>(3)?, r.get::<_, i64>(4)?, r.get::<_, String>(5)?)),
                )
                .optional()
                .map_err(sql)
        };
        let mut found = row(self)?;
        if found.is_none() {
            let pdf = is_ext(&file, &["pdf"]);
            if !file.is_file() || !(pdf && rel.starts_with("papers/") || !pdf && is_ext(&file, TEXT_EXTS)) {
                return Err(format!("{rel}: not an indexable file"));
            }
            self.index_project_file(project, rel)?;
            found = row(self)?;
        }
        let Some((id, kind, hash, size, modified, status)) = found else {
            return Err(format!("{rel}: could not be indexed"));
        };
        let Ok(meta) = std::fs::metadata(&file) else { return Ok(QuoteCheck::Stale) };
        if (i64::try_from(meta.len()).unwrap_or(-1) != size || mtime(&meta) != modified)
            && std::fs::read(&file).map_or(true, |b| hex(&Sha256::digest(&b)) != hash)
        {
            return Ok(QuoteCheck::Stale);
        }
        if status != "ok" && status != "partial" {
            return Ok(QuoteCheck::Unreadable { status });
        }
        let page = if kind == "pdf" {
            Some(page.ok_or("a PDF needs the page number")?)
        } else {
            None
        };
        let mut stmt = self
            .conn
            .prepare(
                "SELECT b.id, b.text, (SELECT a.id FROM anchors a WHERE a.block_id = b.id AND a.stale = 0)
                 FROM blocks b WHERE b.paper_id = ?1 AND b.page IS ?2 ORDER BY b.ordinal",
            )
            .map_err(sql)?;
        let blocks: Vec<(i64, String, Option<i64>)> = stmt
            .query_map(params![id, page], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .map_err(sql)?
            .collect::<Result<_, _>>()
            .map_err(sql)?;
        // The blocks of the page in reading order, joined as `quote_form` joins words.
        let mut joined = String::new();
        let mut starts: Vec<(usize, i64, Option<i64>)> = Vec::new();
        for (block_id, text, anchor) in &blocks {
            let form = cjk::quote_form(text);
            if !joined.is_empty() && !joined.chars().last().is_some_and(cjk::is_cjk) && !form.chars().next().is_some_and(cjk::is_cjk) {
                joined.push(' ');
            }
            starts.push((joined.len(), *block_id, *anchor));
            joined.push_str(&form);
        }
        Ok(match joined.find(&wanted) {
            Some(at) => {
                let (_, block_id, anchor_id) = starts.iter().rev().find(|(s, _, _)| *s <= at).copied().unwrap_or((0, 0, None));
                QuoteCheck::Matched { block_id, anchor_id, page }
            }
            None => QuoteCheck::NotFound,
        })
    }

    /// Indexes the single file `rel` (no refresh of the rest).
    fn index_project_file(&self, project: &Path, rel: &str) -> Result<(), String> {
        let file = safe_file(project, rel)?;
        let meta = std::fs::metadata(&file).map_err(|e| format!("{rel}: {e}"))?;
        let pdf = is_ext(&file, &["pdf"]);
        let limit = if pdf { crate::pdf_text::MAX_PDF_BYTES } else { MAX_TEXT_BYTES };
        let found = Found { rel: rel.to_string(), path: file.clone(), pdf };
        let (hash, parsed) = if meta.len() > limit {
            (String::new(), Parsed::empty("failed", "the file is too large".into()))
        } else {
            let bytes = std::fs::read(&file).map_err(|e| format!("{rel}: {e}"))?;
            (hex(&Sha256::digest(&bytes)), if pdf { parse_pdf(&bytes) } else { parse_text(&bytes, rel) })
        };
        let old: Option<i64> = self
            .conn
            .query_row("SELECT id FROM papers WHERE path = ?1", [rel], |r| r.get(0))
            .optional()
            .map_err(sql)?;
        self.store_paper(&found, old, &hash, i64::try_from(meta.len()).unwrap_or(i64::MAX), mtime(&meta), &parsed)?;
        self.set_meta_value("indexed_at", &now().to_string())?;
        self.set_meta_value("norm_version", cjk::VERSION)
    }
}

fn source_level(path: &str) -> &'static str {
    match path.split('/').next().unwrap_or("") {
        "papers" => "original",
        "digests" => "digest",
        "drafts" => "draft",
        _ => "note",
    }
}

/// About [`SNIPPET_CHARS`] characters of `text` around the first of `terms` it contains, on one line.
fn excerpt(text: &str, terms: &[Vec<char>]) -> String {
    let flat: Vec<char> = text.split_whitespace().collect::<Vec<_>>().join(" ").chars().collect();
    let lower: Vec<char> = flat.iter().map(|c| c.to_lowercase().next().unwrap_or(*c)).collect();
    let at = terms
        .iter()
        .filter(|t| !t.is_empty())
        .find_map(|t| lower.windows(t.len()).position(|w| w == t.as_slice()))
        .unwrap_or(0);
    let from = at.saturating_sub(SNIPPET_CHARS / 4);
    let to = (from + SNIPPET_CHARS).min(flat.len());
    let body: String = flat[from..to].iter().collect();
    format!("{}{body}{}", if from > 0 { "…" } else { "" }, if to < flat.len() { "…" } else { "" })
}

#[cfg(test)]
#[path = "research_tests.rs"]
mod tests;
