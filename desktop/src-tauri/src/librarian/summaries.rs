//! Summaries, comparison tables and numeric checks of the Research Core (docs/archive/PLAN_3.0.md §6, §8).
//!
//! The model writes; Rust decides what may be called verified. A summary (a section's, L1, or the
//! paper's, L2) is stored with `source_hash`, the paper's file hash when it was written: when the
//! file changes the summary shows as stale and is never silently rewritten. Every fact or table
//! cell that carries a quote is checked against the indexed page (`check_quote`): a match is
//! `matched`, a quote that is not on the page is `unverified`, no quote is `model`. A summary never
//! replaces the original: numbers, quotes and comparisons go back to the page.

use super::research::QuoteCheck;
use super::store::{now, sql, Library};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;

pub(super) const SCHEMA_V6: &str = "
CREATE TABLE summaries (
    id           INTEGER PRIMARY KEY,
    paper_id     INTEGER NOT NULL,
    section_id   INTEGER,
    heading      TEXT NOT NULL DEFAULT '',
    level        TEXT NOT NULL,
    body         TEXT NOT NULL,
    facts        TEXT NOT NULL DEFAULT '[]',
    coverage     TEXT NOT NULL DEFAULT '',
    source_hash  TEXT NOT NULL,
    created_at   INTEGER NOT NULL
);
CREATE UNIQUE INDEX summaries_target ON summaries(paper_id, level, heading);
CREATE TABLE artifacts (
    id         INTEGER PRIMARY KEY,
    kind       TEXT NOT NULL,
    title      TEXT NOT NULL,
    question   TEXT NOT NULL DEFAULT '',
    columns    TEXT NOT NULL,
    rows       TEXT NOT NULL,
    created_at INTEGER NOT NULL
);
";

const MAX_BODY: usize = 4000;
const MAX_FACTS: usize = 30;
const MAX_TEXT: usize = 600;
const MAX_COLUMNS: usize = 12;
const MAX_ROWS: usize = 60;

#[derive(Debug, Clone, Deserialize)]
pub struct FactIn {
    pub text: String,
    pub quote: Option<String>,
    pub page: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryRow {
    pub id: i64,
    pub level: String,
    pub heading: String,
    pub body: String,
    pub facts: Value,
    pub coverage: String,
    /// The paper's file changed after this was written, or its section is gone.
    pub stale: bool,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Comparison {
    pub id: i64,
    pub title: String,
    pub question: String,
    pub columns: Value,
    pub rows: Value,
    pub created_at: i64,
}

fn cut(s: &str, max: usize) -> String {
    crate::text::prefix_within(s.trim(), max).to_string()
}

/// How a quote stands against the indexed page: the status word, the page and the anchor it matched.
fn judge(lib: &Library, project: &Path, rel: &str, page: Option<u32>, quote: &str) -> (&'static str, Option<u32>, Option<i64>) {
    match lib.check_quote(project, rel, page, quote) {
        Ok(QuoteCheck::Matched { anchor_id, page, .. }) => ("matched", page, anchor_id),
        _ => ("unverified", page, None),
    }
}

impl Library {
    fn paper_hash(&self, rel: &str) -> Result<Option<(i64, String)>, String> {
        self.conn
            .query_row("SELECT id, file_hash FROM papers WHERE path = ?1", [rel], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()
            .map_err(sql)
    }

    /// Stores (replacing) the summary of a paper (`level` "paper") or of one of its sections
    /// (`level` "section", named by `heading`). Facts with a quote are checked against the page.
    pub fn save_summary(
        &self,
        project: &Path,
        rel: &str,
        level: &str,
        heading: Option<&str>,
        body: &str,
        facts: &[FactIn],
        coverage: &str,
    ) -> Result<Value, String> {
        if level != "paper" && level != "section" {
            return Err("level is \"paper\" or \"section\"".into());
        }
        let body = cut(body, MAX_BODY);
        if body.is_empty() {
            return Err("the summary is empty".into());
        }
        if facts.len() > MAX_FACTS {
            return Err(format!("at most {MAX_FACTS} facts"));
        }
        let (paper_id, hash) = self.paper_hash(rel)?.ok_or_else(|| format!("{rel}: not indexed (research_search indexes a file)"))?;
        let (section_id, heading) = if level == "section" {
            let h = heading.map(str::trim).filter(|h| !h.is_empty()).ok_or("a section summary names its section heading")?;
            let id: Option<i64> = self
                .conn
                .query_row("SELECT id FROM sections WHERE paper_id = ?1 AND heading = ?2 ORDER BY ordinal LIMIT 1", params![paper_id, h], |r| r.get(0))
                .optional()
                .map_err(sql)?;
            (Some(id.ok_or_else(|| format!("{rel} has no section \"{h}\""))?), h.to_string())
        } else {
            (None, String::new())
        };
        let mut stored = Vec::new();
        let mut counts = (0, 0, 0);
        for f in facts {
            let text = cut(&f.text, MAX_TEXT);
            if text.is_empty() {
                continue;
            }
            let quote = f.quote.as_deref().map(|q| cut(q, MAX_TEXT)).filter(|q| !q.is_empty());
            let (status, page, anchor) = match &quote {
                Some(q) => judge(self, project, rel, f.page, q),
                None => ("model", f.page, None),
            };
            match status {
                "matched" => counts.0 += 1,
                "unverified" => counts.1 += 1,
                _ => counts.2 += 1,
            }
            stored.push(json!({ "text": text, "quote": quote, "page": page, "anchorId": anchor, "status": status }));
        }
        self.conn
            .execute(
                "INSERT INTO summaries (paper_id, section_id, heading, level, body, facts, coverage, source_hash, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT(paper_id, level, heading) DO UPDATE SET
                   section_id = excluded.section_id, body = excluded.body, facts = excluded.facts,
                   coverage = excluded.coverage, source_hash = excluded.source_hash, created_at = excluded.created_at",
                params![paper_id, section_id, heading, level, body, Value::Array(stored).to_string(), cut(coverage, 600), hash, now()],
            )
            .map_err(sql)?;
        Ok(json!({ "matched": counts.0, "unverified": counts.1, "model": counts.2 }))
    }

    /// The summaries of one paper, each marked stale when the file or its section changed since.
    pub fn summaries_for(&self, paper_id: i64) -> Result<Vec<SummaryRow>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT s.id, s.level, s.heading, s.body, s.facts, s.coverage, s.created_at,
                        (s.source_hash <> p.file_hash) OR (s.level = 'section' AND NOT EXISTS (SELECT 1 FROM sections c WHERE c.id = s.section_id))
                 FROM summaries s JOIN papers p ON p.id = s.paper_id
                 WHERE s.paper_id = ?1 ORDER BY (s.level = 'paper') DESC, s.id",
            )
            .map_err(sql)?;
        let rows = stmt
            .query_map(params![paper_id], |r| {
                let facts: String = r.get(4)?;
                Ok(SummaryRow {
                    id: r.get(0)?,
                    level: r.get(1)?,
                    heading: r.get(2)?,
                    body: r.get(3)?,
                    facts: serde_json::from_str(&facts).unwrap_or_else(|_| json!([])),
                    coverage: r.get(5)?,
                    created_at: r.get(6)?,
                    stale: r.get::<_, i64>(7)? != 0,
                })
            })
            .map_err(sql)?;
        rows.collect::<Result<_, _>>().map_err(sql)
    }

    /// Stores a comparison table. A cell is `{ value, path?, page?, quote?, kind? }`: `kind` is
    /// `not_reported` or `not_comparable` (no value is invented for those); a cell with a path and a
    /// quote is checked against that paper's page; any other cell is the model's reading.
    pub fn save_comparison(&self, project: &Path, title: &str, question: &str, columns: &Value, rows: &Value) -> Result<Value, String> {
        let title = cut(title, 160);
        if title.is_empty() {
            return Err("a comparison has a title".into());
        }
        let cols = columns.as_array().filter(|c| !c.is_empty() && c.len() <= MAX_COLUMNS).ok_or("1 to 12 columns")?;
        let keys: Vec<String> = cols
            .iter()
            .map(|c| c.get("key").and_then(Value::as_str).filter(|k| !k.is_empty() && k.len() <= 40).map(str::to_string))
            .collect::<Option<_>>()
            .ok_or("every column has a key")?;
        let col_out: Vec<Value> = cols
            .iter()
            .zip(&keys)
            .map(|(c, k)| json!({ "key": k, "label": cut(c.get("label").and_then(Value::as_str).unwrap_or(k), 120) }))
            .collect();
        let rows_in = rows.as_array().filter(|r| !r.is_empty() && r.len() <= MAX_ROWS).ok_or("1 to 60 rows")?;
        let mut counts = (0, 0, 0, 0);
        let mut out = Vec::new();
        for row in rows_in {
            let label = cut(row.get("label").and_then(Value::as_str).unwrap_or(""), 160);
            let mut cells = serde_json::Map::new();
            for key in &keys {
                let Some(cell) = row.get("cells").and_then(|c| c.get(key)) else { continue };
                let kind = cell.get("kind").and_then(Value::as_str);
                let value = cut(cell.get("value").and_then(Value::as_str).unwrap_or(""), 300);
                let (status, value, page, anchor) = match kind {
                    Some("not_reported") => ("not_reported", String::new(), None, None),
                    Some("not_comparable") => ("not_comparable", String::new(), None, None),
                    _ => {
                        let path = cell.get("path").and_then(Value::as_str);
                        let quote = cell.get("quote").and_then(Value::as_str).map(str::trim).filter(|q| !q.is_empty());
                        let page = cell.get("page").and_then(Value::as_u64).and_then(|p| u32::try_from(p).ok());
                        match (path, quote) {
                            (Some(p), Some(q)) => {
                                let (s, pg, a) = judge(self, project, p, page, q);
                                (s, value, pg, a)
                            }
                            _ => ("model", value, page, None),
                        }
                    }
                };
                match status {
                    "matched" => counts.0 += 1,
                    "unverified" => counts.1 += 1,
                    "model" => counts.2 += 1,
                    _ => counts.3 += 1,
                }
                cells.insert(
                    key.clone(),
                    json!({ "value": value, "status": status, "page": page, "anchorId": anchor, "path": cell.get("path").and_then(Value::as_str) }),
                );
            }
            out.push(json!({ "label": label, "cells": cells }));
        }
        self.conn
            .execute(
                "INSERT INTO artifacts (kind, title, question, columns, rows, created_at) VALUES ('comparison', ?1, ?2, ?3, ?4, ?5)",
                params![title, cut(question, 300), Value::Array(col_out).to_string(), Value::Array(out).to_string(), now()],
            )
            .map_err(sql)?;
        Ok(json!({ "id": self.conn.last_insert_rowid(), "matched": counts.0, "unverified": counts.1, "model": counts.2, "gaps": counts.3 }))
    }

    pub fn comparisons(&self) -> Result<Vec<Comparison>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, title, question, columns, rows, created_at FROM artifacts WHERE kind = 'comparison' ORDER BY id DESC")
            .map_err(sql)?;
        let rows = stmt
            .query_map([], |r| {
                let (c, rw): (String, String) = (r.get(3)?, r.get(4)?);
                Ok(Comparison {
                    id: r.get(0)?,
                    title: r.get(1)?,
                    question: r.get(2)?,
                    columns: serde_json::from_str(&c).unwrap_or_else(|_| json!([])),
                    rows: serde_json::from_str(&rw).unwrap_or_else(|_| json!([])),
                    created_at: r.get(5)?,
                })
            })
            .map_err(sql)?;
        rows.collect::<Result<_, _>>().map_err(sql)
    }

    /// Keeps a multi-expert review (`expert_review`) as an artifact of kind `review`: `experts` (each
    /// review with its angle and model) in `columns`, the arbiter's result in `rows`. Returns its id.
    ///
    /// # Errors
    /// Returns an error when the review cannot be inserted into the project database.
    pub fn save_review(&self, title: &str, subject: &str, experts: &Value, verdict: &Value) -> Result<i64, String> {
        self.conn
            .execute(
                "INSERT INTO artifacts (kind, title, question, columns, rows, created_at) VALUES ('review', ?1, ?2, ?3, ?4, ?5)",
                params![cut(title, 200), cut(subject, 300), experts.to_string(), verdict.to_string(), now()],
            )
            .map_err(sql)?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn delete_comparison(&self, id: i64) -> Result<(), String> {
        self.conn.execute("DELETE FROM artifacts WHERE id = ?1", params![id]).map_err(sql)?;
        Ok(())
    }

    /// The text of one page of an indexed paper, block by block (for the numeric checks).
    fn page_blocks(&self, rel: &str, page: Option<u32>) -> Result<Vec<(i64, String)>, String> {
        let (paper_id, _) = self.paper_hash(rel)?.ok_or_else(|| format!("{rel}: not indexed"))?;
        let mut stmt = self
            .conn
            .prepare("SELECT id, text FROM blocks WHERE paper_id = ?1 AND (?2 IS NULL OR page = ?2) ORDER BY ordinal")
            .map_err(sql)?;
        let rows = stmt
            .query_map(params![paper_id, page], |r| Ok((r.get(0)?, r.get(1)?)))
            .map_err(sql)?;
        rows.collect::<Result<_, _>>().map_err(sql)
    }

    /// Whether `value` (with an optional `unit`) appears as a number on the page: spelling-normalised
    /// (thousand separators, full-width digits, the minus sign, a trailing percent sign), matched as a
    /// whole number, and, with `context`, only in a block that also holds that word.
    pub fn check_number(&self, rel: &str, page: Option<u32>, value: &str, unit: Option<&str>, context: Option<&str>) -> Result<Value, String> {
        let want = normal_number(value).ok_or("the value is not a number")?;
        let unit = unit.map(|u| u.trim().to_lowercase()).filter(|u| !u.is_empty());
        let context = context.map(|c| c.trim().to_lowercase()).filter(|c| !c.is_empty());
        let mut seen = Vec::new();
        for (block_id, text) in self.page_blocks(rel, page)? {
            let lower = text.to_lowercase();
            if context.as_ref().is_some_and(|c| !lower.contains(c.as_str())) {
                continue;
            }
            for (num, end) in numbers_in(&text) {
                if num != want {
                    continue;
                }
                if let Some(u) = &unit {
                    let after: String = text[end..].chars().take(12).collect::<String>().to_lowercase();
                    if !after.trim_start().starts_with(u.as_str()) {
                        continue;
                    }
                }
                let around: String = text.chars().take(160).collect();
                seen.push(json!({ "blockId": block_id, "snippet": around.split_whitespace().collect::<Vec<_>>().join(" ") }));
                break;
            }
        }
        Ok(if seen.is_empty() { json!({ "verdict": "not_found" }) } else { json!({ "verdict": "matched", "where": seen }) })
    }

    /// Finds the cell at `row` x `column` of a Markdown table on the page and compares it with `value`.
    pub fn check_table_cell(&self, rel: &str, page: Option<u32>, row: &str, column: &str, value: &str) -> Result<Value, String> {
        let want = normal_number(value);
        let (row_q, col_q) = (row.trim().to_lowercase(), column.trim().to_lowercase());
        if row_q.is_empty() || col_q.is_empty() {
            return Err("name the row and the column".into());
        }
        let mut tables = 0;
        for (block_id, text) in self.page_blocks(rel, page)? {
            let lines: Vec<Vec<String>> = text
                .lines()
                .filter(|l| l.trim_start().starts_with('|'))
                .map(|l| l.trim().trim_matches('|').split('|').map(|c| c.trim().to_string()).collect())
                .filter(|cells: &Vec<String>| !cells.iter().all(|c| c.chars().all(|ch| matches!(ch, '-' | ':' | ' '))))
                .collect();
            let Some(header) = lines.first() else { continue };
            tables += 1;
            let Some(col) = header.iter().position(|h| h.to_lowercase().contains(&col_q)) else { continue };
            for cells in lines.iter().skip(1) {
                if !cells.first().is_some_and(|c| c.to_lowercase().contains(&row_q)) {
                    continue;
                }
                let Some(actual) = cells.get(col) else { continue };
                let same = match (&want, normal_number(actual)) {
                    (Some(w), Some(a)) => *w == a,
                    _ => actual.trim().eq_ignore_ascii_case(value.trim()),
                };
                return Ok(json!({ "verdict": if same { "matched" } else { "mismatch" }, "cell": actual, "blockId": block_id }));
            }
        }
        Ok(json!({ "verdict": "not_found", "tablesOnPage": tables }))
    }
}

/// A number spelled however a paper spells it, as one comparable string: full-width digits and the
/// minus sign folded, thousand separators and a trailing percent sign dropped, trailing zeros of a fraction trimmed.
fn normal_number(s: &str) -> Option<String> {
    let mut t = String::new();
    for c in s.trim().chars() {
        match c {
            '０'..='９' => t.push(char::from(b'0' + u8::try_from(u32::from(c) - u32::from('０')).ok()?)),
            '−' | '–' | '－' => t.push('-'),
            ',' | '，' | ' ' | '\u{a0}' | '%' | '％' => {}
            c => t.push(c),
        }
    }
    let t = t.trim_start_matches('+').to_string();
    let body = t.strip_prefix('-').unwrap_or(&t);
    if body.is_empty() || !body.chars().all(|c| c.is_ascii_digit() || c == '.') || body.matches('.').count() > 1 || !body.chars().any(|c| c.is_ascii_digit()) {
        return None;
    }
    let negative = t.starts_with('-');
    let (int, frac) = body.split_once('.').unwrap_or((body, ""));
    let int = int.trim_start_matches('0');
    let frac = frac.trim_end_matches('0');
    let mut out = String::new();
    if negative && (!int.is_empty() || !frac.is_empty()) {
        out.push('-');
    }
    out.push_str(if int.is_empty() { "0" } else { int });
    if !frac.is_empty() {
        out.push('.');
        out.push_str(frac);
    }
    Some(out)
}

/// Every number token of `text` in normal form, with the byte offset where it ends.
fn numbers_in(text: &str) -> Vec<(String, usize)> {
    let mut out = Vec::new();
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut i = 0;
    let is_digit = |c: char| c.is_ascii_digit() || ('０'..='９').contains(&c);
    while i < chars.len() {
        let (start, c) = chars[i];
        let neg = matches!(c, '-' | '−' | '–') && chars.get(i + 1).is_some_and(|(_, n)| is_digit(*n)) && (i == 0 || !chars[i - 1].1.is_alphanumeric());
        if is_digit(c) || neg {
            let mut j = i + usize::from(neg);
            while j < chars.len() && (is_digit(chars[j].1) || (matches!(chars[j].1, ',' | '.') && chars.get(j + 1).is_some_and(|(_, n)| is_digit(*n)))) {
                j += 1;
            }
            let end = chars.get(j).map_or(text.len(), |(b, _)| *b);
            let token: String = text[start..end].to_string();
            if i == 0 || !chars[i - 1].1.is_alphanumeric() {
                if let Some(n) = normal_number(&token) {
                    out.push((n, end));
                }
            }
            i = j.max(i + 1);
        } else {
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdf_text::pages_pdf;
    use crate::test_support::TempDir;

    fn library(d: &TempDir, pages: &[&str]) -> (std::path::PathBuf, Library) {
        let root = d.path().join("p");
        for dir in ["papers", "notes"] {
            std::fs::create_dir_all(root.join(dir)).unwrap();
        }
        std::fs::write(root.join("papers/a.pdf"), pages_pdf(pages)).unwrap();
        let lib = Library::open(&root).unwrap();
        lib.index_project(&root, super::super::research::IndexOptions::default()).unwrap();
        (root, lib)
    }

    #[test]
    fn numbers_compare_however_they_are_spelled() {
        assert_eq!(normal_number("1,234.50"), Some("1234.5".into()));
        assert_eq!(normal_number("−3.0"), Some("-3".into()));
        assert_eq!(normal_number("12.5%"), Some("12.5".into()));
        assert_eq!(normal_number("０．５"), None);
        assert_eq!(normal_number("abc"), None);
        assert_eq!(normal_number("007"), Some("7".into()));
        let found: Vec<String> = numbers_in("accuracy 91.2% on 1,024 images, delta −0.5").into_iter().map(|(n, _)| n).collect();
        assert_eq!(found, ["91.2", "1024", "-0.5"]);
        assert!(numbers_in("v2 and abc3").is_empty());
    }

    #[test]
    fn a_summary_marks_quotes_matched_or_unverified_and_goes_stale_with_the_file() {
        let d = TempDir::new("summaries", "summary");
        let (root, lib) = library(&d, &["Transformers use attention over all tokens.", "Results: accuracy 91.2 on the test set."]);
        let facts = [
            FactIn { text: "Uses attention".into(), quote: Some("use attention over all tokens".into()), page: Some(1) },
            FactIn { text: "Invented".into(), quote: Some("uses recurrence".into()), page: Some(1) },
            FactIn { text: "My reading".into(), quote: None, page: None },
        ];
        let r = lib.save_summary(&root, "papers/a.pdf", "paper", None, "A paper about attention.", &facts, "").unwrap();
        assert_eq!((r["matched"].as_i64(), r["unverified"].as_i64(), r["model"].as_i64()), (Some(1), Some(1), Some(1)));
        let paper = lib.papers_list().unwrap().remove(0);
        let rows = lib.summaries_for(paper.id).unwrap();
        assert_eq!(rows.len(), 1);
        assert!(!rows[0].stale);
        assert_eq!(rows[0].facts[0]["status"], "matched");
        assert_eq!(rows[0].facts[1]["status"], "unverified");
        // The file changes and is indexed again: the summary is kept, marked stale.
        std::fs::write(root.join("papers/a.pdf"), pages_pdf(&["Changed.", "Also changed."])).unwrap();
        lib.index_project(&root, super::super::research::IndexOptions::default()).unwrap();
        assert!(lib.summaries_for(paper.id).unwrap()[0].stale);
        assert!(lib.save_summary(&root, "papers/a.pdf", "section", Some("Nope"), "x", &[], "").is_err());
        assert!(lib.save_summary(&root, "papers/none.pdf", "paper", None, "x", &[], "").is_err());
    }

    #[test]
    fn a_comparison_keeps_gaps_as_gaps_and_checks_quoted_cells() {
        let d = TempDir::new("summaries", "compare");
        let (root, lib) = library(&d, &["Accuracy was 91.2 on the test set."]);
        let columns = json!([{ "key": "acc", "label": "Accuracy" }, { "key": "data", "label": "Dataset" }]);
        let rows = json!([{ "label": "A", "cells": {
            "acc": { "value": "91.2", "path": "papers/a.pdf", "page": 1, "quote": "Accuracy was 91.2" },
            "data": { "kind": "not_reported", "value": "made up" } } }]);
        let r = lib.save_comparison(&root, "Models", "Which is best?", &columns, &rows).unwrap();
        assert_eq!((r["matched"].as_i64(), r["gaps"].as_i64()), (Some(1), Some(1)));
        let saved = lib.comparisons().unwrap();
        assert_eq!(saved[0].rows[0]["cells"]["data"]["value"], "");
        assert_eq!(saved[0].rows[0]["cells"]["acc"]["status"], "matched");
        assert!(lib.save_comparison(&root, "", "", &columns, &rows).is_err());
        lib.delete_comparison(saved[0].id).unwrap();
        assert!(lib.comparisons().unwrap().is_empty());
    }

    #[test]
    fn numbers_and_table_cells_are_checked_against_the_page() {
        let d = TempDir::new("summaries", "numbers");
        let (_root, lib) = library(&d, &["Accuracy was 91.2% on 1,024 images."]);
        assert_eq!(lib.check_number("papers/a.pdf", Some(1), "91.20", None, None).unwrap()["verdict"], "matched");
        assert_eq!(lib.check_number("papers/a.pdf", Some(1), "1024", None, Some("images")).unwrap()["verdict"], "matched");
        assert_eq!(lib.check_number("papers/a.pdf", Some(1), "91.2", Some("%"), None).unwrap()["verdict"], "matched");
        assert_eq!(lib.check_number("papers/a.pdf", Some(1), "91.2", Some("kg"), None).unwrap()["verdict"], "not_found");
        assert_eq!(lib.check_number("papers/a.pdf", Some(1), "92.1", None, None).unwrap()["verdict"], "not_found");
        assert!(lib.check_number("papers/a.pdf", Some(1), "many", None, None).is_err());
    }

    #[test]
    fn a_markdown_table_cell_matches_or_mismatches() {
        let d = TempDir::new("summaries", "table");
        let root = d.path().join("p");
        std::fs::create_dir_all(root.join("notes")).unwrap();
        std::fs::write(root.join("notes/t.md"), "| Model | Acc | F1 |\n|---|---|---|\n| Base | 90.1 | 88.0 |\n| Ours | 91.2 | 89.5 |\n").unwrap();
        let lib = Library::open(&root).unwrap();
        lib.index_project(&root, super::super::research::IndexOptions::default()).unwrap();
        let same = lib.check_table_cell("notes/t.md", None, "ours", "acc", "91.20").unwrap();
        assert_eq!(same["verdict"], "matched");
        let other = lib.check_table_cell("notes/t.md", None, "Ours", "F1", "90").unwrap();
        assert_eq!((other["verdict"].as_str(), other["cell"].as_str()), (Some("mismatch"), Some("89.5")));
        assert_eq!(lib.check_table_cell("notes/t.md", None, "Nobody", "Acc", "1").unwrap()["verdict"], "not_found");
    }
}
