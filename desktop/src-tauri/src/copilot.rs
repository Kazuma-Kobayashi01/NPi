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
const READ_DIRS: &[&str] = &["papers", "notes", "digests", "artifacts", "data", "drafts"];
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

/// The tools registered with omp (`set_host_tools`).
pub fn tool_definitions() -> Value {
    json!([
        {
            "name": "research_search",
            "label": "Search project",
            "description": "Search the text files of this research project (notes, digests, artifacts, processed data, text papers and the extracted text of PDFs) for passages containing the query terms. Returns file, line and a snippet for each hit. A scanned PDF has no text and is reported, not searched.",
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
        }
    ])
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
            other => Err(format!("unknown tool: {other}")),
        };
        frame(id, outcome)
    }
}

fn frame(id: &str, outcome: ToolResult) -> Value {
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

type ToolResult = Result<String, String>;

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
            other => Err(format!("unknown tool: {other}")),
        };
        frame(id, outcome)
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
                // A symlink is skipped, never followed.
                if kind.is_symlink() {
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
        assert_eq!(names, ["research_search", "research_read", "research_save"]);
        assert!(names
            .iter()
            .all(|n| n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')));
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
            ["academic_projects", "academic_search", "academic_read"]
        );
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
    fn an_unknown_tool_is_an_error_frame_for_the_same_call() {
        let (_d, h) = host("unknown");
        let f = h.call("42", "bash", &json!({}));
        assert_eq!(
            (f["id"].as_str(), f["isError"].as_bool()),
            (Some("42"), Some(true))
        );
    }
}
