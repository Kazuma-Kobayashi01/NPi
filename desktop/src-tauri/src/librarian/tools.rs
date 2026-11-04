//! The Librarian tools that go beyond reading, shared by every agent that works on a project's
//! library: the Academic sessions of the app (as built-in host tools, no approval prompt) and the MCP
//! server (`mcp.rs`).
//!
//! - `library_lookup`: ask Crossref for a work's metadata (by DOI, or title and first author).
//! - `library_align`: compare a reference with Crossref; a preview unless `apply`, then blank fields
//!   are filled (`overwrite` also replaces differing ones); a record with another title is never applied.
//! - `library_add`: add a reference, from a DOI (Crossref), or from CSL-JSON the agent gives; a
//!   reference already in the library is reported, not added twice.
//! - `library_edit`: set fields, add or remove tags, append a note on one reference.
//!
//! There is no tool that deletes. Every change an agent makes to a reference is first written, as the
//! reference was, to `.npi-academic/agent-edits.jsonl`, so it can be put back by hand. Only a DOI, or a
//! title and first author, leaves the machine, to the pinned Crossref host (`lookup.rs`). Errors say what
//! to do next (the nearest citekeys, which argument is missing), because a model reads them and retries.

use super::lookup::{self, Candidate};
use super::store::{now, Entry, Library};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::io::Write as _;
use std::path::PathBuf;

/// References one `library_align` call may look up (each is a network request).
const MAX_ALIGN: usize = 20;

/// Where metadata comes from (Crossref, then OpenAlex: `lookup.rs`; a fake in tests).
pub trait Lookup {
    fn by_doi(&self, doi: &str) -> Result<Vec<Candidate>, String>;
    fn by_title(&self, title: &str, author: &str) -> Result<Vec<Candidate>, String>;
}

pub struct Crossref;

impl Lookup for Crossref {
    fn by_doi(&self, doi: &str) -> Result<Vec<Candidate>, String> {
        lookup::by_doi(doi)
    }
    fn by_title(&self, title: &str, author: &str) -> Result<Vec<Candidate>, String> {
        lookup::by_title(title, author)
    }
}

pub struct LibraryTools {
    root: PathBuf,
}

impl LibraryTools {
    pub const fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub const NAMES: [&'static str; 4] = ["library_lookup", "library_align", "library_add", "library_edit"];

    /// The tools as host tools define them (`parameters`); `mcp.rs` renames that to `inputSchema`.
    pub fn definitions() -> Vec<Value> {
        vec![
            json!({
                "name": "library_lookup",
                "label": "Look up metadata",
                "description": "Ask Crossref for a work's metadata, by DOI or by title (plus the first author's family name). Returns up to 3 candidates as CSL-JSON summaries; nothing in the library changes. Example: {\"doi\": \"10.1038/nature14539\"} or {\"title\": \"Attention is all you need\", \"author\": \"Vaswani\"}. To add the work to the library use library_add; to fix a reference already there use library_align.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "doi": { "type": "string", "description": "A DOI, with or without https://doi.org/." },
                        "title": { "type": "string", "description": "At least a few words." },
                        "author": { "type": "string", "description": "The first author's family name." }
                    },
                    "additionalProperties": false
                },
                "loadMode": "essential"
            }),
            json!({
                "name": "library_align",
                "label": "Align metadata",
                "description": "Bring references' metadata in line with Crossref. Give `citekey` (one reference), or `all: true` (the first 20 references that lack a DOI or a venue). Crossref is asked by the reference's DOI, else by its title and first author; a record is accepted only if its title is almost the same, otherwise the reference is reported as `mismatch` (a wrong DOI shows up this way) and left alone. The result lists, per reference, each field with its `from` and `to`. Default is a preview: nothing is written. Call again with `apply: true` to fill the blank fields (your own values stay); `overwrite: true` also replaces differing values. Example: {\"citekey\": \"smith2020graph\"} then {\"citekey\": \"smith2020graph\", \"apply\": true}.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "citekey": { "type": "string", "description": "Exactly as library_search shows it." },
                        "all": { "type": "boolean" },
                        "apply": { "type": "boolean", "description": "Write the changes (default false: preview only)." },
                        "overwrite": { "type": "boolean", "description": "Also replace fields that already hold a different value (default false)." }
                    },
                    "additionalProperties": false
                },
                "loadMode": "essential"
            }),
            json!({
                "name": "library_add",
                "label": "Add reference",
                "description": "Add a reference to the library. Give a `doi` (the record comes from Crossref), or `csl` (a CSL-JSON object you built, for example {\"type\": \"article-journal\", \"title\": \"...\", \"author\": [{\"family\": \"Smith\", \"given\": \"A\"}], \"issued\": {\"date-parts\": [[2020]]}}). Returns the new citekey. A reference that is already in the library (same DOI, or same title and year) is reported with its citekey and not added twice. Only add what you have seen in a source; never invent a reference.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "doi": { "type": "string" },
                        "csl": { "type": "object", "description": "CSL-JSON of the reference." }
                    },
                    "additionalProperties": false
                },
                "loadMode": "essential"
            }),
            json!({
                "name": "library_edit",
                "label": "Edit reference",
                "description": "Change one reference: `set` replaces CSL fields (title, author, year, issued, container-title, volume, issue, page, publisher, DOI, URL, abstract, type, ISSN, ISBN, language, keyword), `tags_add` / `tags_remove` change its tags, `note_append` adds a line to the user's notes. `author` is a list of {family, given} objects or of strings like \"Smith, Anna\"; `year` is a shorthand for issued. Nothing is deleted: the reference as it was is kept in .npi-academic/agent-edits.jsonl. Example: {\"citekey\": \"smith2020graph\", \"set\": {\"volume\": \"7\", \"page\": \"1-9\"}, \"tags_add\": [\"gnn\"]}.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "citekey": { "type": "string" },
                        "set": { "type": "object" },
                        "tags_add": { "type": "array", "items": { "type": "string" } },
                        "tags_remove": { "type": "array", "items": { "type": "string" } },
                        "note_append": { "type": "string" }
                    },
                    "required": ["citekey"],
                    "additionalProperties": false
                },
                "loadMode": "essential"
            }),
        ]
    }

    /// `None`: not one of these tools.
    pub fn call(&self, name: &str, args: &Value, lookup: &dyn Lookup) -> Option<Result<String, String>> {
        Some(match name {
            "library_lookup" => lookup_tool(args, lookup),
            "library_align" => self.align(args, lookup),
            "library_add" => self.add(args, lookup),
            "library_edit" => self.edit(args),
            _ => return None,
        })
    }

    /// Keeps a copy of a reference as it was before an agent changed it.
    fn remember(&self, entry: &Entry, what: &str) {
        let line = json!({ "at": now(), "what": what, "citekey": entry.citekey, "id": entry.id, "csl": entry.csl, "notes": entry.notes, "tags": entry.tags });
        let dir = self.root.join(".npi-academic");
        let _ = std::fs::create_dir_all(&dir);
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("agent-edits.jsonl")) {
            let _ = writeln!(f, "{line}");
        }
    }

    fn find<'a>(entries: &'a [Entry], key: &str) -> Result<&'a Entry, String> {
        let key = key.trim().trim_start_matches("[@").trim_start_matches('@').trim_end_matches(']');
        entries.iter().find(|e| e.citekey == key).ok_or_else(|| no_such_citekey(entries, key))
    }

    // -------------------------------------------------------------- align

    fn align(&self, args: &Value, lookup: &dyn Lookup) -> Result<String, String> {
        let apply = args.get("apply").and_then(Value::as_bool).unwrap_or(false);
        let overwrite = args.get("overwrite").and_then(Value::as_bool).unwrap_or(false);
        let all = args.get("all").and_then(Value::as_bool).unwrap_or(false);
        let citekey = args.get("citekey").and_then(Value::as_str).map(str::trim).filter(|k| !k.is_empty());
        let lib = Library::open(&self.root)?;
        let entries = lib.list()?;
        let targets: Vec<&Entry> = match (citekey, all) {
            (Some(k), _) => vec![Self::find(&entries, k)?],
            (None, true) => entries
                .iter()
                .filter(|e| blank(e.csl.get("DOI")) || blank(e.csl.get("container-title")))
                .take(MAX_ALIGN)
                .collect(),
            (None, false) => return Err("give a `citekey` (from library_search), or `all: true` for the first 20 references that lack a DOI or venue".into()),
        };
        let mut report = Vec::new();
        let mut applied = 0;
        for (i, entry) in targets.iter().enumerate() {
            if i > 0 {
                std::thread::sleep(std::time::Duration::from_millis(150));
            }
            let f = super::csl::fields(&entry.csl);
            let found = match &f.doi {
                Some(doi) => lookup.by_doi(doi),
                None if f.title.chars().count() >= 8 => lookup.by_title(&f.title, &f.first_author),
                None => Err("no DOI, and a title too short to search; add a DOI with library_edit first".to_string()),
            };
            let candidates = match found {
                Ok(c) => c,
                Err(e) => {
                    report.push(json!({ "citekey": entry.citekey, "status": "no_record", "reason": e }));
                    continue;
                }
            };
            let best = candidates
                .iter()
                .map(|c| (title_similarity(&f.title, &super::csl::fields(&c.csl).title), c))
                .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
            let Some((similarity, candidate)) = best else {
                report.push(json!({ "citekey": entry.citekey, "status": "no_record" }));
                continue;
            };
            if similarity < SAME_WORK && !f.title.is_empty() {
                report.push(json!({
                    "citekey": entry.citekey,
                    "status": "mismatch",
                    "reason": format!("Crossref's record is titled \"{}\" (title similarity {:.0}%); not applied. Check the DOI with library_lookup.", super::csl::fields(&candidate.csl).title, similarity * 100.0),
                }));
                continue;
            }
            let (changes, merged) = merge(&entry.csl, &candidate.csl, overwrite);
            if changes.is_empty() {
                report.push(json!({ "citekey": entry.citekey, "status": "up_to_date" }));
                continue;
            }
            let list: Vec<Value> = changes.iter().map(|(field, from, to)| json!({ "field": field, "from": from, "to": to })).collect();
            if apply {
                self.remember(entry, "library_align");
                lib.update(entry.id, &merged)?;
                applied += 1;
            }
            report.push(json!({ "citekey": entry.citekey, "status": if apply { "applied" } else { "would_change" }, "source": candidate.source, "changes": list }));
        }
        let hint = if apply { "" } else { "Nothing was written. To write these changes call library_align again with apply: true." };
        serde_json::to_string_pretty(&json!({ "applied": apply, "referencesChanged": applied, "results": report, "note": hint })).map_err(|e| e.to_string())
    }

    // -------------------------------------------------------------- add

    fn add(&self, args: &Value, lookup: &dyn Lookup) -> Result<String, String> {
        let lib = Library::open(&self.root)?;
        let (csl, source) = match (args.get("doi").and_then(Value::as_str).map(str::trim).filter(|d| !d.is_empty()), args.get("csl")) {
            (Some(doi), _) => {
                let found = lookup.by_doi(doi).map_err(|e| format!("{e}. Check the DOI with library_lookup, or give `csl` yourself."))?;
                (found.into_iter().next().ok_or("no record for that DOI")?.csl, "agent:crossref")
            }
            (None, Some(csl)) if csl.is_object() => (csl.clone(), "agent"),
            _ => return Err("give a `doi`, or a `csl` object with at least a title".into()),
        };
        if super::csl::fields(&csl).title.trim().is_empty() {
            return Err("the reference needs a title".into());
        }
        if let Some(existing) = lib.duplicate_of(&csl)? {
            return Ok(format!("already in the library as [@{}] (\"{}\"); not added again. Use library_edit or library_align to change it.", existing.citekey, super::csl::fields(&existing.csl).title));
        }
        let entry = lib.add(&csl, source)?;
        Ok(format!("added [@{}] (\"{}\"). Cite it as [@{}].", entry.citekey, super::csl::fields(&entry.csl).title, entry.citekey))
    }

    // -------------------------------------------------------------- edit

    fn edit(&self, args: &Value) -> Result<String, String> {
        let lib = Library::open(&self.root)?;
        let entries = lib.list()?;
        let entry = Self::find(&entries, args.get("citekey").and_then(Value::as_str).unwrap_or(""))?;
        let mut csl = entry.csl.clone();
        let mut changed = Vec::new();
        if let Some(set) = args.get("set") {
            let set = set.as_object().ok_or("`set` is an object of fields, for example {\"volume\": \"7\"}")?;
            for (key, value) in set {
                let (field, value) = normal_field(key, value)?;
                if csl.get(field) != Some(&value) {
                    csl[field] = value;
                    changed.push(field.to_string());
                }
            }
        }
        let list = |name: &str| -> Vec<String> {
            args.get(name).and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect()).unwrap_or_default()
        };
        let (add, remove) = (list("tags_add"), list("tags_remove"));
        let mut tags = entry.tags.clone();
        tags.retain(|t| !remove.iter().any(|r| r.eq_ignore_ascii_case(t)));
        for t in add {
            if !tags.iter().any(|x| x.eq_ignore_ascii_case(&t)) {
                tags.push(t);
            }
        }
        let mut notes = entry.notes.clone();
        if let Some(line) = args.get("note_append").and_then(Value::as_str).map(str::trim).filter(|l| !l.is_empty()) {
            if !notes.is_empty() && !notes.ends_with('\n') {
                notes.push('\n');
            }
            notes.push_str(line);
            changed.push("notes".into());
        }
        if tags != entry.tags {
            changed.push("tags".into());
        }
        if changed.is_empty() {
            return Ok(format!("[@{}] already has those values; nothing changed.", entry.citekey));
        }
        self.remember(entry, "library_edit");
        if csl != entry.csl {
            lib.update(entry.id, &csl)?;
        }
        if tags != entry.tags || notes != entry.notes {
            lib.set_meta(entry.id, &tags, &notes)?;
        }
        Ok(format!("[@{}] changed: {}. The earlier version is kept in .npi-academic/agent-edits.jsonl.", entry.citekey, changed.join(", ")))
    }
}

/// The up to five citekeys that look like `key`, for a model that mistyped or invented one.
pub(crate) fn no_such_citekey(entries: &[Entry], key: &str) -> String {
    let low = key.to_lowercase();
    let mut near: Vec<&str> = entries
        .iter()
        .map(|e| e.citekey.as_str())
        .filter(|k| {
            let k = k.to_lowercase();
            !low.is_empty() && (k.contains(&low) || low.contains(&k) || words(&k).intersection(&words(&low)).next().is_some())
        })
        .take(5)
        .collect();
    near.dedup();
    if near.is_empty() {
        format!("no reference with the citekey {key:?}. Find the right one with library_search; a citekey looks like smith2020graph and is never guessed")
    } else {
        format!("no reference with the citekey {key:?}. Did you mean: {}? (library_search shows the exact citekeys)", near.join(", "))
    }
}

// ------------------------------------------------------------------ lookup

fn summarize(c: &Candidate) -> Value {
    let f = super::csl::fields(&c.csl);
    json!({
        "source": c.source, "title": f.title, "firstAuthor": f.first_author, "year": f.year, "doi": f.doi, "type": f.kind,
        "container": c.csl.get("container-title"), "csl": c.csl,
    })
}

fn lookup_tool(args: &Value, lookup: &dyn Lookup) -> Result<String, String> {
    let get = |k: &str| args.get(k).and_then(Value::as_str).unwrap_or("").trim();
    let candidates = if !get("doi").is_empty() {
        lookup.by_doi(get("doi"))?
    } else if get("title").chars().count() >= 8 {
        lookup.by_title(get("title"), get("author"))?
    } else {
        return Err("give a `doi`, or a `title` of at least a few words (and the first author's family name as `author`)".into());
    };
    serde_json::to_string_pretty(&candidates.iter().map(summarize).collect::<Vec<_>>()).map_err(|e| e.to_string())
}

// ------------------------------------------------------------------ matching and merging

/// Lower-case alphanumeric words of a title.
fn words(title: &str) -> HashSet<String> {
    title
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

/// How much of the shorter title's words the other holds (0 to 1), so a subtitle does not make the
/// same work look different; a title of fewer than three words must match as a whole (Dice).
pub(super) fn title_similarity(a: &str, b: &str) -> f64 {
    let (a, b) = (words(a), words(b));
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let both = a.intersection(&b).count() as f64;
    let shorter = a.len().min(b.len()) as f64;
    if shorter >= 3.0 { both / shorter } else { both / (a.len() + b.len()) as f64 * 2.0 }
}

/// A title this close is the same work (subtitles and punctuation differ).
pub(super) const SAME_WORK: f64 = 0.85;

/// The metadata fields `library_align` may fill or replace; never the title of an entry that has one, tags or notes.
const FIELDS: &[&str] = &[
    "DOI", "container-title", "container-title-short", "volume", "issue", "page", "issued", "publisher", "publisher-place",
    "ISSN", "ISBN", "URL", "abstract", "author", "editor", "type",
];

/// What `library_edit` may set.
const EDITABLE: &[&str] = &[
    "title", "author", "editor", "issued", "container-title", "container-title-short", "volume", "issue", "page", "publisher",
    "publisher-place", "DOI", "URL", "abstract", "type", "ISSN", "ISBN", "language", "keyword",
];

fn blank(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => true,
        Some(Value::String(s)) => s.trim().is_empty(),
        Some(Value::Array(a)) => a.is_empty(),
        Some(Value::Object(o)) => o.is_empty(),
        _ => false,
    }
}

/// The differences between `have` and Crossref's `found` as `(field, from, to)`, and the merged record.
fn merge(have: &Value, found: &Value, overwrite: bool) -> (Vec<(String, Value, Value)>, Value) {
    let mut merged = have.clone();
    let mut changes = Vec::new();
    for field in FIELDS {
        let Some(new) = found.get(*field).filter(|v| !blank(Some(v))) else { continue };
        let old = have.get(*field);
        // "document" is what a missing type reads as, not a user's choice.
        let is_blank = blank(old) || (*field == "type" && old.and_then(Value::as_str) == Some("document"));
        if old == Some(new) || (!is_blank && !overwrite) {
            continue;
        }
        changes.push(((*field).to_string(), old.cloned().unwrap_or(Value::Null), new.clone()));
        merged[*field] = new.clone();
    }
    (changes, merged)
}

/// One person as a CSL name: an object with `family` / `given` / `literal`, or "Family, Given" / "Given Family".
fn person(v: &Value) -> Result<Value, String> {
    match v {
        Value::Object(_) => Ok(v.clone()),
        Value::String(line) => {
            let s = line.trim();
            if let Some((family, given)) = s.split_once(',') {
                Ok(json!({ "family": family.trim(), "given": given.trim() }))
            } else if let Some((given, family)) = s.rsplit_once(' ') {
                Ok(json!({ "family": family.trim(), "given": given.trim() }))
            } else if s.is_empty() {
                Err("an author is not empty".into())
            } else {
                Ok(json!({ "literal": s }))
            }
        }
        _ => Err("an author is an object {family, given} or a string \"Family, Given\"".into()),
    }
}

/// A `library_edit` field as the library stores it: the CSL key and value.
fn normal_field(key: &str, value: &Value) -> Result<(&'static str, Value), String> {
    if key == "year" {
        let year = value.as_i64().or_else(|| value.as_str().and_then(|s| s.trim().parse().ok())).filter(|y| (1000..=2999).contains(y));
        return year.map(|y| ("issued", json!({ "date-parts": [[y]] }))).ok_or_else(|| "`year` is a four-digit year".to_string());
    }
    let field = EDITABLE.iter().find(|f| f.eq_ignore_ascii_case(key)).copied().ok_or_else(|| format!("cannot set {key:?}; editable fields: year, {}", EDITABLE.join(", ")))?;
    let value = match field {
        "author" | "editor" => Value::Array(value.as_array().ok_or_else(|| format!("{field} is a list of names"))?.iter().map(person).collect::<Result<_, _>>()?),
        "issued" => match value {
            Value::Object(_) => value.clone(),
            Value::Number(_) | Value::String(_) => return normal_field("year", value),
            _ => return Err("`issued` is {\"date-parts\": [[2020, 5]]} or use `year`".into()),
        },
        _ => match value {
            Value::String(s) => Value::String(s.trim().to_string()),
            Value::Number(n) => Value::String(n.to_string()),
            _ => return Err(format!("{field} is text")),
        },
    };
    Ok((field, value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use crate::test_support::TempDir;

    pub struct Fake(pub Vec<Candidate>);
    impl Lookup for Fake {
        fn by_doi(&self, _: &str) -> Result<Vec<Candidate>, String> {
            Ok(self.0.clone())
        }
        fn by_title(&self, _: &str, _: &str) -> Result<Vec<Candidate>, String> {
            Ok(self.0.clone())
        }
    }

    pub fn crossref(title: &str) -> Candidate {
        Candidate {
            csl: json!({ "type": "article-journal", "title": title, "DOI": "10.1/x", "container-title": "Nature", "volume": "7", "page": "1-9", "issued": { "date-parts": [[2020]] }, "author": [{ "family": "Smith", "given": "A" }] }),
            source: "crossref",
        }
    }

    pub fn project(d: &TempDir) -> PathBuf {
        let root = d.path().join("p");
        std::fs::create_dir_all(root.join("papers")).unwrap();
        let lib = Library::open(&root).unwrap();
        lib.add(&json!({ "type": "article-journal", "title": "Deep graph learning", "author": [{ "family": "Smith", "given": "A" }], "volume": "9" }), "manual").unwrap();
        root
    }

    fn key_of(root: &Path) -> String {
        Library::open(root).unwrap().list().unwrap()[0].citekey.clone()
    }

    fn run(root: &Path, fake: &Fake, name: &str, args: Value) -> Result<Value, String> {
        LibraryTools::new(root.to_path_buf()).call(name, &args, fake).expect("a tool of this set").map(|t| serde_json::from_str(&t).unwrap_or(Value::String(t)))
    }

    #[test]
    fn align_previews_by_default_then_fills_blanks_only() {
        let d = TempDir::new("tools", "align");
        let root = project(&d);
        let fake = Fake(vec![crossref("Deep Graph Learning: a survey")]);
        let key = key_of(&root);
        let preview = run(&root, &fake, "library_align", json!({ "citekey": key })).unwrap();
        assert_eq!(preview["applied"], false);
        assert_eq!(preview["results"][0]["status"], "would_change");
        let fields: Vec<&str> = preview["results"][0]["changes"].as_array().unwrap().iter().map(|c| c["field"].as_str().unwrap()).collect();
        assert!(fields.contains(&"DOI") && fields.contains(&"container-title") && fields.contains(&"page"));
        assert!(!fields.contains(&"volume"), "a filled field is left alone without overwrite");
        assert!(preview["note"].as_str().unwrap().contains("apply: true"));
        assert!(Library::open(&root).unwrap().list().unwrap()[0].csl.get("DOI").is_none(), "a preview writes nothing");

        let done = run(&root, &fake, "library_align", json!({ "citekey": key, "apply": true })).unwrap();
        assert_eq!(done["referencesChanged"], 1);
        let csl = Library::open(&root).unwrap().list().unwrap()[0].csl.clone();
        assert_eq!(csl["DOI"], "10.1/x");
        assert_eq!(csl["volume"], "9", "the user's value kept");
        assert!(root.join(".npi-academic/agent-edits.jsonl").is_file(), "the earlier version is kept");
        assert_eq!(run(&root, &fake, "library_align", json!({ "citekey": key })).unwrap()["results"][0]["status"], "up_to_date");

        let over = run(&root, &fake, "library_align", json!({ "citekey": key, "apply": true, "overwrite": true })).unwrap();
        assert_eq!(over["results"][0]["changes"][0]["field"], "volume");
        assert_eq!(Library::open(&root).unwrap().list().unwrap()[0].csl["volume"], "7");
    }

    #[test]
    fn a_record_with_another_title_is_a_mismatch_and_a_bad_call_says_what_to_give() {
        let d = TempDir::new("tools", "mismatch");
        let root = project(&d);
        let fake = Fake(vec![crossref("Quantum chromodynamics on the lattice")]);
        let r = run(&root, &fake, "library_align", json!({ "all": true, "apply": true })).unwrap();
        assert_eq!(r["results"][0]["status"], "mismatch");
        assert!(Library::open(&root).unwrap().list().unwrap()[0].csl.get("DOI").is_none());
        assert!(run(&root, &fake, "library_align", json!({})).unwrap_err().contains("citekey"));
        let key = key_of(&root);
        let near = run(&root, &fake, "library_align", json!({ "citekey": &key[..key.len() - 2] })).unwrap_err();
        assert!(near.contains("Did you mean"), "{near}");
        let none = run(&root, &fake, "library_edit", json!({ "citekey": "zzz" })).unwrap_err();
        assert!(none.contains("library_search"), "{none}");
    }

    #[test]
    fn lookup_needs_a_doi_or_a_real_title() {
        let d = TempDir::new("tools", "lookup");
        let root = project(&d);
        let fake = Fake(vec![crossref("Deep graph learning")]);
        assert!(run(&root, &fake, "library_lookup", json!({ "title": "short" })).unwrap_err().contains("doi"));
        assert!(run(&root, &fake, "library_lookup", json!({ "doi": "10.1/x" })).unwrap().to_string().contains("Nature"));
    }

    #[test]
    fn add_takes_a_doi_or_csl_and_never_twice() {
        let d = TempDir::new("tools", "add");
        let root = project(&d);
        let fake = Fake(vec![crossref("Graph attention networks")]);
        let said = run(&root, &fake, "library_add", json!({ "doi": "10.1/x" })).unwrap();
        assert!(said.as_str().unwrap().starts_with("added [@"), "{said}");
        let again = run(&root, &fake, "library_add", json!({ "doi": "10.1/x" })).unwrap();
        assert!(again.as_str().unwrap().contains("already in the library"), "{again}");
        let own = run(&root, &fake, "library_add", json!({ "csl": { "type": "book", "title": "A book of proofs", "issued": { "date-parts": [[2019]] } } })).unwrap();
        assert!(own.as_str().unwrap().starts_with("added"));
        assert_eq!(Library::open(&root).unwrap().list().unwrap().len(), 3);
        assert!(run(&root, &fake, "library_add", json!({})).unwrap_err().contains("doi"));
        assert!(run(&root, &fake, "library_add", json!({ "csl": { "type": "book" } })).unwrap_err().contains("title"));
    }

    #[test]
    fn edit_sets_fields_tags_and_notes_and_keeps_the_old_version() {
        let d = TempDir::new("tools", "edit");
        let root = project(&d);
        let fake = Fake(vec![]);
        let key = key_of(&root);
        let said = run(
            &root,
            &fake,
            "library_edit",
            json!({ "citekey": format!("[@{key}]"), "set": { "year": 2021, "page": "1-9", "author": ["Smith, Anna", "Lee Wei"] }, "tags_add": ["gnn", "GNN"], "note_append": "read in 2021" }),
        )
        .unwrap();
        assert!(said.as_str().unwrap().contains("issued") && said.as_str().unwrap().contains("tags") && said.as_str().unwrap().contains("notes"), "{said}");
        let e = Library::open(&root).unwrap().list().unwrap().remove(0);
        assert_eq!(e.csl["issued"]["date-parts"][0][0], 2021);
        assert_eq!(e.csl["author"][1]["family"], "Wei");
        assert_eq!(e.tags, ["gnn"]);
        assert_eq!(e.notes, "read in 2021");
        let log = std::fs::read_to_string(root.join(".npi-academic/agent-edits.jsonl")).unwrap();
        assert!(log.contains("Deep graph learning") && log.contains("library_edit"));
        assert!(run(&root, &fake, "library_edit", json!({ "citekey": key, "set": { "nonsense": "x" } })).unwrap_err().contains("editable fields"));
        let same = run(&root, &fake, "library_edit", json!({ "citekey": key, "tags_add": ["gnn"] })).unwrap();
        assert!(same.as_str().unwrap().contains("nothing changed"));
        let removed = run(&root, &fake, "library_edit", json!({ "citekey": key, "tags_remove": ["GNN"] })).unwrap();
        assert!(removed.as_str().unwrap().contains("tags"));
        assert!(Library::open(&root).unwrap().list().unwrap()[0].tags.is_empty());
    }

    #[test]
    fn titles_compare_by_shared_words() {
        assert!(title_similarity("Deep graph learning", "Deep Graph Learning") > 0.99);
        assert!(title_similarity("Deep graph learning", "Deep graph learning: a survey") >= SAME_WORK);
        assert!(title_similarity("Deep graph learning", "Quantum chromodynamics") < 0.1);
    }

    #[test]
    fn the_tool_set_is_named_once_and_in_order() {
        let names: Vec<String> = LibraryTools::definitions().iter().map(|d| d["name"].as_str().unwrap().to_string()).collect();
        assert_eq!(names, LibraryTools::NAMES);
    }
}
