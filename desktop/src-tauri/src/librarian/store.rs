//! A project's reference library: the `references` table of
//! `<project>/.npi-academic/research.db`, one CSL-JSON record per row plus the
//! columns it is searched and compared by. The database is the user's work,
//! not a cache: rows are only soft-deleted (`deleted_at`), and the schema
//! carries a version for migrations.

use super::citekey;
use super::csl::{self, normalize_title};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::Serialize;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const SCHEMA_VERSION: i64 = 4;

const SCHEMA_V1: &str = "
CREATE TABLE refs (
    id          INTEGER PRIMARY KEY,
    citekey     TEXT NOT NULL UNIQUE,
    csl_json    TEXT NOT NULL,
    title       TEXT NOT NULL,
    title_norm  TEXT NOT NULL,
    author      TEXT NOT NULL,
    author_norm TEXT NOT NULL,
    year        INTEGER,
    doi         TEXT,
    kind        TEXT NOT NULL,
    source      TEXT NOT NULL,
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL,
    deleted_at  INTEGER
);
CREATE INDEX refs_doi ON refs(doi) WHERE doi IS NOT NULL;
CREATE INDEX refs_title ON refs(title_norm);
";

#[cfg(test)]
pub(super) const SCHEMA_V1_FOR_TESTS: &str = SCHEMA_V1;

/// Notes per reference, manual groups and tags.
const SCHEMA_V2: &str = "
ALTER TABLE refs ADD COLUMN notes TEXT NOT NULL DEFAULT '';
CREATE TABLE groups (
    id         INTEGER PRIMARY KEY,
    name       TEXT NOT NULL UNIQUE COLLATE NOCASE,
    created_at INTEGER NOT NULL
);
CREATE TABLE group_refs (
    group_id INTEGER NOT NULL,
    ref_id   INTEGER NOT NULL,
    PRIMARY KEY (group_id, ref_id)
);
CREATE TABLE ref_tags (
    ref_id INTEGER NOT NULL,
    tag    TEXT NOT NULL COLLATE NOCASE,
    PRIMARY KEY (ref_id, tag)
);
";

/// One reference as the frontend sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub id: i64,
    pub citekey: String,
    pub csl: Value,
    pub source: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub notes: String,
    pub tags: Vec<String>,
    /// Ids of the groups it is in.
    pub groups: Vec<i64>,
    pub attachments: Vec<super::attach::Attachment>,
    /// Proposals from the Copilot waiting for the user.
    pub suggestions: Vec<super::suggest::Suggestion>,
}

/// A removed reference, as the trash lists it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Trashed {
    pub id: i64,
    pub citekey: String,
    pub csl: Value,
    pub deleted_at: i64,
}

pub struct Library {
    pub(super) conn: Connection,
}

pub(super) fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0))
}

pub(super) fn sql(e: rusqlite::Error) -> String {
    format!("library: {e}")
}

pub(super) fn entry(row: &Row<'_>) -> rusqlite::Result<Entry> {
    let text: String = row.get(2)?;
    Ok(Entry {
        id: row.get(0)?,
        citekey: row.get(1)?,
        csl: serde_json::from_str(&text).unwrap_or(Value::Null),
        source: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
        notes: row.get(6)?,
        tags: Vec::new(),
        groups: Vec::new(),
        attachments: Vec::new(),
        suggestions: Vec::new(),
    })
}

pub(super) const COLUMNS: &str = "id, citekey, csl_json, source, created_at, updated_at, notes";

/// Files attached to a reference (PDFs kept under the project's `papers/`).
const SCHEMA_V3: &str = "
CREATE TABLE attachments (
    id       INTEGER PRIMARY KEY,
    ref_id   INTEGER NOT NULL,
    path     TEXT NOT NULL,
    hash     TEXT NOT NULL,
    kind     TEXT NOT NULL,
    added_at INTEGER NOT NULL
);
CREATE INDEX attachments_ref ON attachments(ref_id);
";

/// What the Copilot proposes and the user answers; a resolved row is the log of what was applied.
const SCHEMA_V4: &str = "
CREATE TABLE suggestions (
    id          INTEGER PRIMARY KEY,
    ref_id      INTEGER NOT NULL,
    kind        TEXT NOT NULL,
    value       TEXT NOT NULL,
    reason      TEXT NOT NULL,
    status      TEXT NOT NULL DEFAULT 'pending',
    created_at  INTEGER NOT NULL,
    resolved_at INTEGER
);
CREATE INDEX suggestions_ref ON suggestions(ref_id, status);
";

impl Library {
    /// The library file of `project`.
    pub fn path(project: &Path) -> PathBuf {
        project.join(".npi-academic").join("research.db")
    }

    /// Opens (creating, migrating) the library of `project`.
    pub fn open(project: &Path) -> Result<Self, String> {
        let path = Self::path(project);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("library folder: {e}"))?;
        }
        // A library from before notes, groups and tags is kept as it was before it is migrated.
        if path.is_file() {
            let old: i64 = Connection::open(&path)
                .and_then(|c| c.query_row("PRAGMA user_version", [], |r| r.get(0)))
                .map_err(sql)?;
            if (1..=3).contains(&old) {
                std::fs::copy(&path, path.with_extension(format!("db.bak-v{old}")))
                    .map_err(|e| format!("library backup: {e}"))?;
            }
        }
        Self::init(Connection::open(path).map_err(sql)?)
    }

    pub(super) fn init(conn: Connection) -> Result<Self, String> {
        // The window, the Copilot and the main chat may each open this file; wait a little for
        // a writer instead of failing with "database is locked".
        conn.busy_timeout(std::time::Duration::from_secs(5)).map_err(sql)?;
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .map_err(sql)?;
        if version > SCHEMA_VERSION {
            return Err(format!(
                "library was made by a newer version (schema {version})"
            ));
        }
        if version < 1 {
            conn.execute_batch(&format!(
                "BEGIN; {SCHEMA_V1} PRAGMA user_version = 1; COMMIT;"
            ))
            .map_err(sql)?;
        }
        if version < 2 {
            conn.execute_batch(&format!(
                "BEGIN; {SCHEMA_V2} PRAGMA user_version = 2; COMMIT;"
            ))
            .map_err(sql)?;
        }
        if version < 3 {
            conn.execute_batch(&format!(
                "BEGIN; {SCHEMA_V3} PRAGMA user_version = 3; COMMIT;"
            ))
            .map_err(sql)?;
        }
        if version < 4 {
            conn.execute_batch(&format!(
                "BEGIN; {SCHEMA_V4} PRAGMA user_version = 4; COMMIT;"
            ))
            .map_err(sql)?;
        }
        Ok(Self { conn })
    }

    fn citekey_taken(&self, key: &str) -> Result<bool, String> {
        self.conn
            .query_row("SELECT 1 FROM refs WHERE citekey = ?1", [key], |_| Ok(()))
            .optional()
            .map(|found| found.is_some())
            .map_err(sql)
    }

    /// Adds a record under a fresh citekey.
    pub fn add(&self, csl: &Value, source: &str) -> Result<Entry, String> {
        if !csl.is_object() {
            return Err("a reference is a JSON object".to_string());
        }
        let f = csl::fields(csl);
        let key = citekey::unique(&citekey::base(&f), |k| self.citekey_taken(k))?;
        let at = now();
        self.conn
            .execute(
                "INSERT INTO refs (citekey, csl_json, title, title_norm, author, author_norm, year, doi, kind, source, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11)",
                params![
                    key, csl.to_string(), f.title, normalize_title(&f.title), f.first_author,
                    normalize_title(&f.first_author), f.year, f.doi, f.kind, source, at
                ],
            )
            .map_err(sql)?;
        self.get(self.conn.last_insert_rowid())?
            .ok_or_else(|| "library: row vanished".to_string())
    }

    pub fn get(&self, id: i64) -> Result<Option<Entry>, String> {
        let found = self
            .conn
            .query_row(
                &format!("SELECT {COLUMNS} FROM refs WHERE id = ?1 AND deleted_at IS NULL"),
                [id],
                entry,
            )
            .optional()
            .map_err(sql)?;
        let mut one: Vec<Entry> = found.into_iter().collect();
        self.hydrate(&mut one)?;
        Ok(one.pop())
    }

    /// Every live reference, oldest first.
    pub fn list(&self) -> Result<Vec<Entry>, String> {
        let mut stmt = self
            .conn
            .prepare(&format!(
                "SELECT {COLUMNS} FROM refs WHERE deleted_at IS NULL ORDER BY created_at, id"
            ))
            .map_err(sql)?;
        let rows = stmt.query_map([], entry).map_err(sql)?;
        let mut entries: Vec<Entry> = rows.collect::<Result<_, _>>().map_err(sql)?;
        self.hydrate(&mut entries)?;
        Ok(entries)
    }

    /// Replaces a record's content; its citekey stays, since drafts cite it.
    pub fn update(&self, id: i64, csl: &Value) -> Result<Entry, String> {
        if !csl.is_object() {
            return Err("a reference is a JSON object".to_string());
        }
        let f = csl::fields(csl);
        let changed = self
            .conn
            .execute(
                "UPDATE refs SET csl_json = ?2, title = ?3, title_norm = ?4, author = ?5, author_norm = ?6,
                 year = ?7, doi = ?8, kind = ?9, updated_at = ?10 WHERE id = ?1 AND deleted_at IS NULL",
                params![
                    id, csl.to_string(), f.title, normalize_title(&f.title), f.first_author,
                    normalize_title(&f.first_author), f.year, f.doi, f.kind, now()
                ],
            )
            .map_err(sql)?;
        if changed == 0 {
            return Err("no such reference".to_string());
        }
        self.get(id)?
            .ok_or_else(|| "library: row vanished".to_string())
    }

    /// Marks a reference deleted (the row stays, so its citekey is never reused).
    pub fn remove(&self, id: i64) -> Result<bool, String> {
        let changed = self
            .conn
            .execute(
                "UPDATE refs SET deleted_at = ?2 WHERE id = ?1 AND deleted_at IS NULL",
                params![id, now()],
            )
            .map_err(sql)?;
        Ok(changed > 0)
    }

    /// What was removed, most recently removed first.
    pub fn trashed(&self) -> Result<Vec<Trashed>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, citekey, csl_json, deleted_at FROM refs WHERE deleted_at IS NOT NULL ORDER BY deleted_at DESC, id DESC")
            .map_err(sql)?;
        let rows = stmt
            .query_map([], |r| {
                let text: String = r.get(2)?;
                Ok(Trashed {
                    id: r.get(0)?,
                    citekey: r.get(1)?,
                    csl: serde_json::from_str(&text).unwrap_or(Value::Null),
                    deleted_at: r.get(3)?,
                })
            })
            .map_err(sql)?;
        rows.collect::<Result<_, _>>().map_err(sql)
    }

    /// Brings a removed reference back, with its groups, tags, notes and files as they were.
    pub fn restore(&self, id: i64) -> Result<Entry, String> {
        let changed = self
            .conn
            .execute(
                "UPDATE refs SET deleted_at = NULL, updated_at = ?2 WHERE id = ?1 AND deleted_at IS NOT NULL",
                params![id, now()],
            )
            .map_err(sql)?;
        if changed == 0 {
            return Err("that reference is not in the trash".to_string());
        }
        self.get(id)?
            .ok_or_else(|| "library: row vanished".to_string())
    }

    /// The live reference `csl` repeats: the same DOI, or the same title, year
    /// and first author once normalised. Only reported, never merged.
    pub fn duplicate_of(&self, csl: &Value) -> Result<Option<Entry>, String> {
        let f = csl::fields(csl);
        let title = normalize_title(&f.title);
        self.conn
            .query_row(
                &format!(
                    "SELECT {COLUMNS} FROM refs WHERE deleted_at IS NULL AND (
                        (?1 IS NOT NULL AND doi = ?1)
                        OR (?2 != '' AND title_norm = ?2 AND year IS ?3 AND author_norm = ?4)
                     ) ORDER BY id LIMIT 1"
                ),
                params![f.doi, title, f.year, normalize_title(&f.first_author)],
                entry,
            )
            .optional()
            .map_err(sql)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn lib() -> Library {
        Library::init(Connection::open_in_memory().unwrap()).unwrap()
    }

    fn paper(title: &str, year: i32, doi: Option<&str>) -> Value {
        let mut v = json!({
            "type": "article-journal", "title": title,
            "author": [{ "family": "Smith", "given": "J." }],
            "issued": { "date-parts": [[year]] },
        });
        if let Some(doi) = doi {
            v["DOI"] = json!(doi);
        }
        v
    }

    #[test]
    fn add_get_list_update_and_remove_round_trip() {
        let l = lib();
        let a = l.add(&paper("Graphs", 2020, None), "manual").unwrap();
        assert_eq!(a.citekey, "smith2020graphs");
        assert_eq!(l.get(a.id).unwrap().unwrap().csl["title"], "Graphs");
        let b = l.add(&paper("Graphs", 2020, None), "manual").unwrap();
        assert_eq!(b.citekey, "smith2020graphsa");
        let edited = l
            .update(a.id, &paper("Graphs, revised", 2020, None))
            .unwrap();
        assert_eq!(
            (edited.citekey.as_str(), edited.csl["title"].as_str()),
            ("smith2020graphs", Some("Graphs, revised"))
        );
        assert!(l.remove(a.id).unwrap());
        assert!(!l.remove(a.id).unwrap());
        assert_eq!(
            l.list().unwrap().iter().map(|e| e.id).collect::<Vec<_>>(),
            vec![b.id]
        );
        assert!(l.update(a.id, &paper("x", 1, None)).is_err());
    }

    #[test]
    fn a_deleted_reference_keeps_its_citekey_taken() {
        let l = lib();
        let a = l.add(&paper("Graphs", 2020, None), "manual").unwrap();
        l.remove(a.id).unwrap();
        assert_eq!(
            l.add(&paper("Graphs", 2020, None), "manual")
                .unwrap()
                .citekey,
            "smith2020graphsa"
        );
    }

    #[test]
    fn duplicates_match_by_doi_or_by_title_year_and_author() {
        let l = lib();
        let a = l
            .add(
                &paper("Deep Graphs: A Survey", 2020, Some("10.1/AB")),
                "ris",
            )
            .unwrap();
        let by_doi = paper("Another title", 1999, Some("https://doi.org/10.1/ab"));
        assert_eq!(l.duplicate_of(&by_doi).unwrap().map(|e| e.id), Some(a.id));
        let by_title = paper("deep graphs - a survey", 2020, None);
        assert_eq!(l.duplicate_of(&by_title).unwrap().map(|e| e.id), Some(a.id));
        assert!(l
            .duplicate_of(&paper("Deep Graphs: A Survey", 2021, None))
            .unwrap()
            .is_none());
        assert!(l
            .duplicate_of(&json!({ "type": "book" }))
            .unwrap()
            .is_none());
        l.remove(a.id).unwrap();
        assert!(l.duplicate_of(&by_doi).unwrap().is_none());
    }

    #[test]
    fn a_removed_reference_is_listed_in_the_trash_and_comes_back_whole() {
        let dir = crate::test_support::TempDir::new("librarian", "trash");
        let lib = Library::open(dir.path()).unwrap();
        let a = lib.add(&json!({"title": "Keep me", "issued": {"date-parts": [[2020]]}}), "manual").unwrap();
        let b = lib.add(&json!({"title": "Other"}), "manual").unwrap();
        lib.set_meta(a.id, &["nlp".to_string()], "my note").unwrap();
        let group = lib.add_group("Methods").unwrap();
        lib.set_member(a.id, group.id, true).unwrap();
        assert!(lib.remove(a.id).unwrap());
        assert_eq!(lib.list().unwrap().len(), 1);
        let trash = lib.trashed().unwrap();
        assert_eq!(trash.iter().map(|t| t.citekey.as_str()).collect::<Vec<_>>(), [a.citekey.as_str()]);
        assert_eq!(trash[0].csl["title"], "Keep me");
        let back = lib.restore(a.id).unwrap();
        assert_eq!((back.tags.as_slice(), back.notes.as_str(), back.groups.len()), (["nlp".to_string()].as_slice(), "my note", 1));
        assert!(lib.trashed().unwrap().is_empty());
        assert!(lib.restore(a.id).is_err(), "a live reference is not in the trash");
        assert!(lib.restore(9999).is_err());
        // A removed citekey is never given to a new reference.
        lib.remove(b.id).unwrap();
        let again = lib.add(&json!({"title": "Other"}), "manual").unwrap();
        assert_ne!(again.citekey, b.citekey);
    }

    #[test]
    fn the_file_survives_a_reopen_and_a_newer_schema_is_refused() {
        let dir = crate::test_support::TempDir::new("librarian", "reopen");
        let id = Library::open(dir.path())
            .unwrap()
            .add(&paper("Kept", 2020, None), "manual")
            .unwrap()
            .id;
        assert_eq!(
            Library::open(dir.path())
                .unwrap()
                .get(id)
                .unwrap()
                .unwrap()
                .csl["title"],
            "Kept"
        );
        let conn = Connection::open(Library::path(dir.path())).unwrap();
        conn.execute_batch("PRAGMA user_version = 99").unwrap();
        drop(conn);
        assert!(Library::open(dir.path()).err().unwrap().contains("newer"));
    }
}
