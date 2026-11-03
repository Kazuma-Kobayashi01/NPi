//! What the Copilot may propose for a reference — a tag, a group, a note —
//! and what the user answers. A proposal changes nothing: the row waits as
//! `pending` until the user accepts it in the Librarian window, and only then
//! does Rust apply it through the same calls the form uses. A resolved row stays
//! as the log of what was applied or dismissed.

use super::store::{now, sql, Library};
use rusqlite::params;
use serde::Serialize;
use std::collections::HashMap;

const PENDING_MAX_PER_REF: i64 = 12;
const VALUE_MAX_CHARS: usize = 2000;
const REASON_MAX_CHARS: usize = 300;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Suggestion {
    pub id: i64,
    /// `tag`, `group` or `note`.
    pub kind: String,
    pub value: String,
    pub reason: String,
}

impl Library {
    pub(super) fn pending_suggestions_by_ref(&self) -> Result<HashMap<i64, Vec<Suggestion>>, String> {
        let mut out: HashMap<i64, Vec<Suggestion>> = HashMap::new();
        let mut stmt = self
            .conn
            .prepare("SELECT ref_id, id, kind, value, reason FROM suggestions WHERE status = 'pending' ORDER BY id")
            .map_err(sql)?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    Suggestion { id: r.get(1)?, kind: r.get(2)?, value: r.get(3)?, reason: r.get(4)? },
                ))
            })
            .map_err(sql)?;
        for row in rows {
            let (ref_id, s) = row.map_err(sql)?;
            out.entry(ref_id).or_default().push(s);
        }
        Ok(out)
    }

    /// Records a proposal for the live reference `ref_id`. Returns false when the same one is already waiting.
    pub fn suggest(&self, ref_id: i64, kind: &str, value: &str, reason: &str) -> Result<bool, String> {
        if !["tag", "group", "note"].contains(&kind) {
            return Err("a suggestion is a tag, a group or a note".into());
        }
        let value = value.trim();
        if value.is_empty() {
            return Err("the suggestion is empty".into());
        }
        if value.chars().count() > VALUE_MAX_CHARS {
            return Err("the suggestion is too long".into());
        }
        if self.get(ref_id)?.is_none() {
            return Err("no such reference".into());
        }
        let reason: String = reason.trim().chars().take(REASON_MAX_CHARS).collect();
        let waiting: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM suggestions WHERE ref_id = ?1 AND status = 'pending'", [ref_id], |r| r.get(0))
            .map_err(sql)?;
        if waiting >= PENDING_MAX_PER_REF {
            return Err("this reference already has many suggestions waiting".into());
        }
        let repeat: bool = self
            .conn
            .query_row(
                "SELECT 1 FROM suggestions WHERE ref_id = ?1 AND kind = ?2 AND value = ?3 COLLATE NOCASE AND status = 'pending'",
                params![ref_id, kind, value],
                |_| Ok(()),
            )
            .is_ok();
        if repeat {
            return Ok(false);
        }
        self.conn
            .execute(
                "INSERT INTO suggestions (ref_id, kind, value, reason, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![ref_id, kind, value, reason, now()],
            )
            .map_err(sql)?;
        Ok(true)
    }

    /// The user's answer. Accepting applies the change; either way the row is closed.
    pub fn resolve_suggestion(&self, id: i64, accept: bool) -> Result<super::Entry, String> {
        let (ref_id, kind, value): (i64, String, String) = self
            .conn
            .query_row(
                "SELECT ref_id, kind, value FROM suggestions WHERE id = ?1 AND status = 'pending'",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .map_err(|_| "that suggestion is already answered".to_string())?;
        if accept {
            let entry = self.get(ref_id)?.ok_or("no such reference")?;
            match kind.as_str() {
                "tag" => {
                    let mut tags = entry.tags.clone();
                    tags.push(value);
                    self.set_meta(ref_id, &tags, &entry.notes)?;
                }
                "note" => {
                    let notes = if entry.notes.trim().is_empty() {
                        value
                    } else {
                        format!("{}\n\n{value}", entry.notes.trim_end())
                    };
                    self.set_meta(ref_id, &entry.tags, &notes)?;
                }
                _ => {
                    let group = match self.groups()?.into_iter().find(|g| g.name.to_lowercase() == value.to_lowercase()) {
                        Some(g) => g.id,
                        None => self.add_group(&value)?.id,
                    };
                    self.set_member(ref_id, group, true)?;
                }
            }
        }
        self.conn
            .execute(
                "UPDATE suggestions SET status = ?2, resolved_at = ?3 WHERE id = ?1",
                params![id, if accept { "accepted" } else { "dismissed" }, now()],
            )
            .map_err(sql)?;
        self.get(ref_id)?.ok_or_else(|| "library: row vanished".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;
    use serde_json::json;

    #[test]
    fn a_suggestion_waits_changes_nothing_and_applies_only_when_accepted() {
        let dir = TempDir::new("librarian", "suggest");
        let lib = Library::open(dir.path()).unwrap();
        let e = lib.add(&json!({"title": "T"}), "manual").unwrap();
        assert!(lib.suggest(e.id, "tag", "nlp", "about language").unwrap());
        assert!(!lib.suggest(e.id, "tag", "NLP", "again").unwrap(), "the same one is not queued twice");
        assert!(lib.suggest(e.id, "group", "Methods", "").unwrap());
        assert!(lib.suggest(e.id, "note", "Read section 3 first.", "").unwrap());
        let waiting = lib.get(e.id).unwrap().unwrap();
        assert_eq!(waiting.suggestions.len(), 3);
        assert!(waiting.tags.is_empty() && waiting.groups.is_empty() && waiting.notes.is_empty(), "nothing applied yet");

        let ids: Vec<i64> = waiting.suggestions.iter().map(|s| s.id).collect();
        let after_tag = lib.resolve_suggestion(ids[0], true).unwrap();
        assert_eq!(after_tag.tags, ["nlp"]);
        let after_group = lib.resolve_suggestion(ids[1], true).unwrap();
        assert_eq!(after_group.groups.len(), 1);
        let after_dismiss = lib.resolve_suggestion(ids[2], false).unwrap();
        assert!(after_dismiss.notes.is_empty());
        assert!(after_dismiss.suggestions.is_empty());
        assert!(lib.resolve_suggestion(ids[2], true).is_err(), "an answered suggestion cannot be answered again");
    }

    #[test]
    fn a_note_is_appended_and_bad_suggestions_are_refused() {
        let dir = TempDir::new("librarian", "suggest-note");
        let lib = Library::open(dir.path()).unwrap();
        let e = lib.add(&json!({"title": "T"}), "manual").unwrap();
        lib.set_meta(e.id, &[], "mine").unwrap();
        lib.suggest(e.id, "note", "theirs", "").unwrap();
        let id = lib.get(e.id).unwrap().unwrap().suggestions[0].id;
        assert_eq!(lib.resolve_suggestion(id, true).unwrap().notes, "mine\n\ntheirs");
        assert!(lib.suggest(e.id, "delete", "x", "").is_err());
        assert!(lib.suggest(e.id, "tag", "  ", "").is_err());
        assert!(lib.suggest(999, "tag", "x", "").is_err());
        assert!(lib.suggest(e.id, "note", &"x".repeat(2001), "").is_err());
    }
}
