//! The tools that put a library in order: see what needs doing, then group, tag and merge. Agents
//! with the library's other editing tools have these too (`tools.rs`), the Copilot included. Every
//! change to a reference is first written, as it was, to `.npi-academic/agent-edits.jsonl`; nothing is
//! deleted (a merged reference goes to the Trash, where the user can restore it).

use super::csl::fields;
use super::tools::LibraryTools;
use super::{Entry, Library};
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// Most citekeys named per list in the overview, and per call.
const MAX_LISTED: usize = 30;
const MAX_KEYS: usize = 200;

pub(super) fn definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "library_overview",
            "label": "Library overview",
            "description": "What the library holds and what needs organising: totals, the groups and tags with their counts, and the citekeys that lack metadata, have no PDF, have no tag or no group, or look like duplicates. Start here when asked to organise the library. Changes nothing.",
            "parameters": { "type": "object", "properties": {}, "additionalProperties": false },
            "loadMode": "essential"
        }),
        json!({
            "name": "library_group",
            "label": "Group references",
            "description": "Put references into a group, made if it does not exist yet, or with `remove: true` take them out (the references stay). Example: {\"group\": \"Graph methods\", \"citekeys\": [\"smith2020graph\", \"lee2021deep\"]}. Reuse an existing group from library_overview rather than a near-duplicate name.",
            "parameters": {
                "type": "object",
                "properties": {
                    "group": { "type": "string" },
                    "citekeys": { "type": "array", "items": { "type": "string" } },
                    "remove": { "type": "boolean" }
                },
                "required": ["group", "citekeys"],
                "additionalProperties": false
            },
            "loadMode": "essential"
        }),
        json!({
            "name": "library_tag",
            "label": "Tag references",
            "description": "Add or remove tags on several references at once. Example: {\"citekeys\": [\"smith2020graph\", \"lee2021deep\"], \"add\": [\"survey\"]}. Reuse existing tags from library_overview rather than a near-duplicate spelling.",
            "parameters": {
                "type": "object",
                "properties": {
                    "citekeys": { "type": "array", "items": { "type": "string" } },
                    "add": { "type": "array", "items": { "type": "string" } },
                    "remove": { "type": "array", "items": { "type": "string" } }
                },
                "required": ["citekeys"],
                "additionalProperties": false
            },
            "loadMode": "essential"
        }),
        json!({
            "name": "library_merge",
            "label": "Merge duplicates",
            "description": "Merge the duplicate `other` into `keep`: `keep`'s values win, its blanks are filled from `other`, tags, notes, groups and files move over, and `other` goes to the Trash. Default is a preview: nothing is written. Call again with `apply: true`. Only merge two records of the same work; a preprint and its journal version are often kept apart, so read both with library_get first.",
            "parameters": {
                "type": "object",
                "properties": {
                    "keep": { "type": "string" },
                    "other": { "type": "string" },
                    "apply": { "type": "boolean" }
                },
                "required": ["keep", "other"],
                "additionalProperties": false
            },
            "loadMode": "essential"
        }),
    ]
}

fn strings(args: &Value, name: &str) -> Vec<String> {
    args.get(name)
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect())
        .unwrap_or_default()
}

fn keys_of<'a>(list: impl Iterator<Item = &'a Entry>) -> Value {
    let keys: Vec<&str> = list.map(|e| e.citekey.as_str()).collect();
    json!({ "count": keys.len(), "citekeys": keys.iter().take(MAX_LISTED).collect::<Vec<_>>() })
}

/// What makes two records the same work: the DOI, else the title's letters and digits plus the year.
fn same_work_key(entry: &Entry) -> String {
    if let Some(doi) = entry.csl.get("DOI").and_then(Value::as_str).filter(|d| !d.trim().is_empty()) {
        return doi.trim().to_lowercase();
    }
    let f = fields(&entry.csl);
    let title: String = f.title.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect();
    format!("{title}|{}", f.year.unwrap_or_default())
}

fn duplicates(entries: &[Entry]) -> Vec<Vec<&str>> {
    let mut same: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    for e in entries {
        same.entry(same_work_key(e)).or_default().push(&e.citekey);
    }
    same.into_values().filter(|keys| keys.len() > 1).take(MAX_LISTED).collect()
}

impl LibraryTools {
    fn entries_named<'a>(entries: &'a [Entry], keys: &[String]) -> Result<Vec<&'a Entry>, String> {
        if keys.is_empty() {
            return Err("`citekeys` is empty: give the citekeys from library_search or library_overview".into());
        }
        if keys.len() > MAX_KEYS {
            return Err(format!("at most {MAX_KEYS} citekeys in one call"));
        }
        keys.iter().map(|k| Self::find(entries, k)).collect()
    }

    pub(super) fn overview(&self) -> Result<String, String> {
        let lib = Library::open(&self.root)?;
        let entries = lib.list()?;
        let no_venue = |e: &&Entry| e.csl.get("container-title").and_then(Value::as_str).is_none_or(|v| v.trim().is_empty());
        let no_doi = |e: &&Entry| e.csl.get("DOI").and_then(Value::as_str).is_none_or(|v| v.trim().is_empty());
        let mut tags: BTreeMap<String, usize> = BTreeMap::new();
        for t in entries.iter().flat_map(|e| &e.tags) {
            *tags.entry(t.clone()).or_default() += 1;
        }
        let groups: Vec<Value> = lib.groups()?.iter().map(|g| json!({ "name": g.name, "count": g.count })).collect();
        let report = json!({
            "total": entries.len(),
            "groups": groups,
            "tags": tags,
            "missing_doi": keys_of(entries.iter().filter(no_doi)),
            "missing_venue": keys_of(entries.iter().filter(no_venue)),
            "no_pdf": keys_of(entries.iter().filter(|e| e.attachments.is_empty())),
            "no_tag": keys_of(entries.iter().filter(|e| e.tags.is_empty())),
            "no_group": keys_of(entries.iter().filter(|e| e.groups.is_empty())),
            "possible_duplicates": duplicates(&entries),
        });
        serde_json::to_string_pretty(&report).map_err(|e| e.to_string())
    }

    pub(super) fn group(&self, args: &Value) -> Result<String, String> {
        let name = args.get("group").and_then(Value::as_str).map_or("", str::trim);
        if name.is_empty() {
            return Err("`group` is empty: name the group".into());
        }
        let remove = args.get("remove").and_then(Value::as_bool).unwrap_or(false);
        let lib = Library::open(&self.root)?;
        let entries = lib.list()?;
        let chosen = Self::entries_named(&entries, &strings(args, "citekeys"))?;
        let found = lib.groups()?.into_iter().find(|g| g.name.eq_ignore_ascii_case(name));
        let group = match found {
            Some(group) => group,
            None if remove => return Err(format!("there is no group \"{name}\"; library_overview lists them")),
            None => lib.add_group(name)?,
        };
        for entry in &chosen {
            self.remember(entry, "library_group");
            lib.set_member(entry.id, group.id, !remove)?;
        }
        let verb = if remove { "Removed from" } else { "Added to" };
        Ok(format!("{verb} group \"{}\": {} references.", group.name, chosen.len()))
    }

    pub(super) fn tag(&self, args: &Value) -> Result<String, String> {
        let (add, remove) = (strings(args, "add"), strings(args, "remove"));
        if add.is_empty() && remove.is_empty() {
            return Err("give `add` or `remove`, a list of tags".into());
        }
        let lib = Library::open(&self.root)?;
        let entries = lib.list()?;
        let chosen = Self::entries_named(&entries, &strings(args, "citekeys"))?;
        let mut changed = 0;
        for entry in &chosen {
            let mut tags = entry.tags.clone();
            tags.retain(|t| !remove.iter().any(|r| r.eq_ignore_ascii_case(t)));
            for t in &add {
                if !tags.iter().any(|x| x.eq_ignore_ascii_case(t)) {
                    tags.push(t.clone());
                }
            }
            if tags != entry.tags {
                self.remember(entry, "library_tag");
                lib.set_meta(entry.id, &tags, &entry.notes)?;
                changed += 1;
            }
        }
        Ok(format!("Tags changed on {changed} of {} references.", chosen.len()))
    }

    pub(super) fn merge(&self, args: &Value) -> Result<String, String> {
        let name = |key: &str| args.get(key).and_then(Value::as_str).unwrap_or("").to_string();
        let lib = Library::open(&self.root)?;
        let entries = lib.list()?;
        let (keep, other) = (Self::find(&entries, &name("keep"))?, Self::find(&entries, &name("other"))?);
        if keep.id == other.id {
            return Err("`keep` and `other` are the same reference".into());
        }
        if !args.get("apply").and_then(Value::as_bool).unwrap_or(false) {
            let line = |e: &Entry| {
                let f = fields(&e.csl);
                json!({ "citekey": e.citekey, "title": f.title, "year": f.year, "doi": e.csl.get("DOI"), "tags": e.tags, "files": e.attachments.len() })
            };
            return serde_json::to_string_pretty(&json!({ "preview": true, "keep": line(keep), "other": line(other), "next": "call again with apply: true" }))
                .map_err(|e| e.to_string());
        }
        self.remember(keep, "library_merge");
        self.remember(other, "library_merge");
        let merged = lib.merge(keep.id, other.id)?;
        Ok(format!("Merged [@{}] into [@{}]; [@{}] is in the Trash.", other.citekey, merged.citekey, other.citekey))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;
    use std::path::PathBuf;

    fn project(d: &TempDir) -> (PathBuf, LibraryTools) {
        let root = d.path().join("p");
        let lib = Library::open(&root).unwrap();
        for (title, doi) in [("Deep graph learning", "10.1/a"), ("Deep Graph Learning", "10.1/A"), ("Trees", "")] {
            let mut csl = json!({ "type": "article-journal", "title": title, "issued": { "date-parts": [[2020]] } });
            if !doi.is_empty() {
                csl["DOI"] = json!(doi);
            }
            lib.add(&csl, "manual").unwrap();
        }
        (root.clone(), LibraryTools::new(root))
    }

    fn keys(root: &std::path::Path) -> Vec<String> {
        Library::open(root).unwrap().list().unwrap().into_iter().map(|e| e.citekey).collect()
    }

    #[test]
    fn the_overview_says_what_needs_doing() {
        let d = TempDir::new("organize-tools", "overview");
        let (root, tools) = project(&d);
        let report: Value = serde_json::from_str(&tools.overview().unwrap()).unwrap();
        assert_eq!(report["total"], 3);
        assert_eq!(report["missing_doi"]["count"], 1);
        assert_eq!(report["no_tag"]["count"], 3);
        let k = keys(&root);
        assert_eq!(report["possible_duplicates"], json!([[k[0], k[1]]]));
    }

    #[test]
    fn grouping_makes_the_group_reuses_it_and_takes_references_out() {
        let d = TempDir::new("organize-tools", "group");
        let (root, tools) = project(&d);
        let k = keys(&root);
        tools.group(&json!({ "group": "GNN", "citekeys": [k[0], k[1]] })).unwrap();
        tools.group(&json!({ "group": "gnn", "citekeys": [k[2]] })).unwrap();
        let lib = Library::open(&root).unwrap();
        let groups = lib.groups().unwrap();
        assert_eq!((groups.len(), groups[0].count), (1, 3), "one group, whatever the case");
        tools.group(&json!({ "group": "GNN", "citekeys": [k[2]], "remove": true })).unwrap();
        assert_eq!(lib.groups().unwrap()[0].count, 2);
        assert!(tools.group(&json!({ "group": "none", "citekeys": [k[0]], "remove": true })).is_err());
        assert!(tools.group(&json!({ "group": "x", "citekeys": ["nokey"] })).is_err(), "a mistyped key changes nothing");
        assert_eq!(lib.groups().unwrap().len(), 1);
        assert!(tools.group(&json!({ "group": "x", "citekeys": [] })).unwrap_err().contains("library_search"));
    }

    #[test]
    fn tagging_changes_only_what_differs_and_keeps_the_notes() {
        let d = TempDir::new("organize-tools", "tag");
        let (root, tools) = project(&d);
        let k = keys(&root);
        let lib = Library::open(&root).unwrap();
        let first = lib.list().unwrap()[0].clone();
        lib.set_meta(first.id, &["old".to_string()], "my note").unwrap();
        assert!(tools.tag(&json!({ "citekeys": [k[0], k[1]], "add": ["Survey"], "remove": ["OLD"] })).unwrap().contains("2 of 2"));
        assert!(tools.tag(&json!({ "citekeys": [k[0]], "add": ["survey"] })).unwrap().contains("0 of 1"));
        let e = lib.get(first.id).unwrap().unwrap();
        assert_eq!((e.tags, e.notes.as_str()), (vec!["Survey".to_string()], "my note"));
        assert!(tools.tag(&json!({ "citekeys": [k[0]] })).is_err());
        assert!(std::fs::read_to_string(root.join(".npi-academic/agent-edits.jsonl")).unwrap().contains("library_tag"));
    }

    #[test]
    fn merging_previews_first_and_the_other_goes_to_the_trash() {
        let d = TempDir::new("organize-tools", "merge");
        let (root, tools) = project(&d);
        let k = keys(&root);
        let args = json!({ "keep": k[0], "other": k[1] });
        assert!(tools.merge(&args).unwrap().contains("apply"));
        assert_eq!(keys(&root).len(), 3, "a preview writes nothing");
        assert!(tools.merge(&json!({ "keep": k[0], "other": k[0] })).is_err());
        tools.merge(&json!({ "keep": k[0], "other": k[1], "apply": true })).unwrap();
        assert_eq!(keys(&root).len(), 2);
        assert_eq!(Library::open(&root).unwrap().trashed().unwrap().len(), 1, "restorable");
    }
}
