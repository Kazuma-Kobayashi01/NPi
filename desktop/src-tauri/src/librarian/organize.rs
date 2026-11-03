//! What the user adds to a reference by hand: groups, tags and notes.
//! (Groups and tags belong to a project's library, like the references.)

use super::store::{now, sql, Entry, Library};
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use std::collections::HashMap;

const TAG_MAX_CHARS: usize = 40;
const TAGS_MAX: usize = 30;
const NAME_MAX_CHARS: usize = 60;
const NOTES_MAX_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Group {
    pub id: i64,
    pub name: String,
    /// Live references in it.
    pub count: i64,
}

/// A group name or tag as stored: trimmed, inner whitespace collapsed.
fn clean(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

impl Library {
    /// Fills `tags` and `groups` of `entries` (live ones only are ever passed).
    pub(super) fn hydrate(&self, entries: &mut [Entry]) -> Result<(), String> {
        if entries.is_empty() {
            return Ok(());
        }
        let mut tags: HashMap<i64, Vec<String>> = HashMap::new();
        let mut stmt = self
            .conn
            .prepare("SELECT ref_id, tag FROM ref_tags ORDER BY tag")
            .map_err(sql)?;
        for row in stmt
            .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))
            .map_err(sql)?
        {
            let (id, tag) = row.map_err(sql)?;
            tags.entry(id).or_default().push(tag);
        }
        let mut groups: HashMap<i64, Vec<i64>> = HashMap::new();
        let mut stmt = self
            .conn
            .prepare("SELECT ref_id, group_id FROM group_refs ORDER BY group_id")
            .map_err(sql)?;
        for row in stmt
            .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))
            .map_err(sql)?
        {
            let (id, group) = row.map_err(sql)?;
            groups.entry(id).or_default().push(group);
        }
        let mut attachments = self.attachments_by_ref()?;
        for e in entries {
            e.tags = tags.remove(&e.id).unwrap_or_default();
            e.groups = groups.remove(&e.id).unwrap_or_default();
            e.attachments = attachments.remove(&e.id).unwrap_or_default();
        }
        Ok(())
    }

    pub fn groups(&self) -> Result<Vec<Group>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT g.id, g.name,
                        (SELECT COUNT(*) FROM group_refs gr JOIN refs r ON r.id = gr.ref_id
                          WHERE gr.group_id = g.id AND r.deleted_at IS NULL)
                 FROM groups g ORDER BY g.name COLLATE NOCASE",
            )
            .map_err(sql)?;
        let rows = stmt
            .query_map([], |r| {
                Ok(Group {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    count: r.get(2)?,
                })
            })
            .map_err(sql)?;
        rows.collect::<Result<_, _>>().map_err(sql)
    }

    fn checked_name(name: &str) -> Result<String, String> {
        let name = clean(name);
        if name.is_empty() {
            return Err("a group needs a name".to_string());
        }
        if name.chars().count() > NAME_MAX_CHARS {
            return Err(format!(
                "a group name is at most {NAME_MAX_CHARS} characters"
            ));
        }
        Ok(name)
    }

    pub fn add_group(&self, name: &str) -> Result<Group, String> {
        let name = Self::checked_name(name)?;
        self.conn
            .execute(
                "INSERT INTO groups (name, created_at) VALUES (?1, ?2)",
                params![name, now()],
            )
            .map_err(|e| {
                if e.to_string().contains("UNIQUE") {
                    "there is a group with that name".to_string()
                } else {
                    sql(e)
                }
            })?;
        Ok(Group {
            id: self.conn.last_insert_rowid(),
            name,
            count: 0,
        })
    }

    pub fn rename_group(&self, id: i64, name: &str) -> Result<(), String> {
        let name = Self::checked_name(name)?;
        let changed = self
            .conn
            .execute(
                "UPDATE groups SET name = ?2 WHERE id = ?1",
                params![id, name],
            )
            .map_err(|e| {
                if e.to_string().contains("UNIQUE") {
                    "there is a group with that name".to_string()
                } else {
                    sql(e)
                }
            })?;
        if changed == 0 {
            Err("no such group".to_string())
        } else {
            Ok(())
        }
    }

    /// Removes the group and who is in it; the references stay.
    pub fn remove_group(&self, id: i64) -> Result<bool, String> {
        self.conn
            .execute("DELETE FROM group_refs WHERE group_id = ?1", [id])
            .map_err(sql)?;
        Ok(self
            .conn
            .execute("DELETE FROM groups WHERE id = ?1", [id])
            .map_err(sql)?
            > 0)
    }

    /// Puts a reference in a group, or takes it out.
    pub fn set_member(&self, ref_id: i64, group_id: i64, member: bool) -> Result<(), String> {
        let live = self.get(ref_id)?.is_some();
        let group = self
            .conn
            .query_row("SELECT 1 FROM groups WHERE id = ?1", [group_id], |_| Ok(()))
            .optional()
            .map_err(sql)?
            .is_some();
        if !live || !group {
            return Err("no such reference or group".to_string());
        }
        let sql_text = if member {
            "INSERT OR IGNORE INTO group_refs (group_id, ref_id) VALUES (?1, ?2)"
        } else {
            "DELETE FROM group_refs WHERE group_id = ?1 AND ref_id = ?2"
        };
        self.conn
            .execute(sql_text, params![group_id, ref_id])
            .map_err(sql)?;
        Ok(())
    }

    /// Replaces a reference's tags and notes.
    pub fn set_meta(&self, ref_id: i64, tags: &[String], notes: &str) -> Result<Entry, String> {
        if notes.len() > NOTES_MAX_BYTES {
            return Err("the note is too long".to_string());
        }
        let mut kept: Vec<String> = Vec::new();
        for tag in tags.iter().map(|t| clean(t)).filter(|t| !t.is_empty()) {
            if tag.chars().count() > TAG_MAX_CHARS {
                return Err(format!("a tag is at most {TAG_MAX_CHARS} characters"));
            }
            if !kept.iter().any(|k| k.to_lowercase() == tag.to_lowercase()) {
                kept.push(tag);
            }
        }
        if kept.len() > TAGS_MAX {
            return Err(format!("at most {TAGS_MAX} tags"));
        }
        if self.get(ref_id)?.is_none() {
            return Err("no such reference".to_string());
        }
        self.conn
            .execute("DELETE FROM ref_tags WHERE ref_id = ?1", [ref_id])
            .map_err(sql)?;
        for tag in &kept {
            self.conn
                .execute(
                    "INSERT INTO ref_tags (ref_id, tag) VALUES (?1, ?2)",
                    params![ref_id, tag],
                )
                .map_err(sql)?;
        }
        self.conn
            .execute(
                "UPDATE refs SET notes = ?2, updated_at = ?3 WHERE id = ?1",
                params![ref_id, notes, now()],
            )
            .map_err(sql)?;
        self.get(ref_id)?
            .ok_or_else(|| "library: row vanished".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use serde_json::json;

    fn lib() -> Library {
        Library::init(Connection::open_in_memory().unwrap()).unwrap()
    }

    fn add(l: &Library, title: &str) -> Entry {
        l.add(&json!({ "type": "book", "title": title, "author": [{ "family": "Lee" }], "issued": { "date-parts": [[2020]] } }), "manual").unwrap()
    }

    #[test]
    fn groups_hold_references_count_live_ones_and_survive_removal_of_the_group() {
        let l = lib();
        let (a, b) = (add(&l, "One"), add(&l, "Two"));
        let g = l.add_group("  Chapter   1 ").unwrap();
        assert_eq!(g.name, "Chapter 1");
        l.set_member(a.id, g.id, true).unwrap();
        l.set_member(b.id, g.id, true).unwrap();
        l.set_member(b.id, g.id, true).unwrap();
        assert_eq!(
            l.groups().unwrap(),
            vec![Group {
                id: g.id,
                name: "Chapter 1".into(),
                count: 2
            }]
        );
        assert_eq!(l.get(a.id).unwrap().unwrap().groups, vec![g.id]);
        l.remove(b.id).unwrap();
        assert_eq!(l.groups().unwrap()[0].count, 1);
        l.set_member(a.id, g.id, false).unwrap();
        assert_eq!(l.get(a.id).unwrap().unwrap().groups.len(), 0);
        l.set_member(a.id, g.id, true).unwrap();
        assert!(l.remove_group(g.id).unwrap());
        assert!(l.get(a.id).unwrap().is_some() && l.groups().unwrap().is_empty());
        assert_eq!(l.get(a.id).unwrap().unwrap().groups.len(), 0);
    }

    #[test]
    fn group_names_are_checked_and_unique_ignoring_case() {
        let l = lib();
        l.add_group("Methods").unwrap();
        assert!(l
            .add_group("methods")
            .unwrap_err()
            .contains("with that name"));
        assert!(l.add_group("   ").is_err());
        assert!(l.add_group(&"x".repeat(61)).is_err());
        let other = l.add_group("Results").unwrap();
        assert!(l.rename_group(other.id, "METHODS").is_err());
        l.rename_group(other.id, "Findings").unwrap();
        assert!(l.rename_group(999, "Nope").is_err());
        assert!(l.set_member(999, other.id, true).is_err());
    }

    #[test]
    fn tags_and_notes_are_replaced_cleaned_and_bounded() {
        let l = lib();
        let a = add(&l, "One");
        let e = l
            .set_meta(
                a.id,
                &[
                    "  deep  learning ".into(),
                    "Deep Learning".into(),
                    String::new(),
                    "survey".into(),
                ],
                "my note",
            )
            .unwrap();
        assert_eq!(e.tags, vec!["deep learning", "survey"]);
        assert_eq!(e.notes, "my note");
        let e = l.set_meta(a.id, &[], "").unwrap();
        assert!(e.tags.is_empty() && e.notes.is_empty());
        assert!(l.set_meta(a.id, &["x".repeat(41)], "").is_err());
        assert!(l
            .set_meta(
                a.id,
                &(0..31).map(|i| format!("t{i}")).collect::<Vec<_>>(),
                ""
            )
            .is_err());
        assert!(l.set_meta(999, &[], "").is_err());
    }

    #[test]
    fn a_version_1_library_is_backed_up_and_migrated_with_its_references_intact() {
        let dir = crate::test_support::TempDir::new("librarian", "migrate");
        let path = Library::path(dir.path());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(super::super::store::SCHEMA_V1_FOR_TESTS)
            .unwrap();
        conn.execute_batch("PRAGMA user_version = 1;").unwrap();
        conn.execute("INSERT INTO refs (citekey, csl_json, title, title_norm, author, author_norm, year, doi, kind, source, created_at, updated_at) VALUES ('old2019x', '{\"title\":\"Old\"}', 'Old', 'old', '', '', 2019, NULL, 'book', 'manual', 1, 1)", []).unwrap();
        drop(conn);
        let l = Library::open(dir.path()).unwrap();
        let e = l.list().unwrap();
        assert_eq!(
            (e.len(), e[0].citekey.as_str(), e[0].notes.as_str()),
            (1, "old2019x", "")
        );
        assert!(path.with_extension("db.bak-v1").is_file());
        assert!(l.add_group("Fine").is_ok());
    }
}
