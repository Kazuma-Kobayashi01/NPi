//! Memory: short notes about the user that the model keeps across conversations, in Chat, Code and
//! Academic alike. One SQLite file on this computer, `~/.local/share/npi/memory.db`; nothing here
//! leaves it except as part of a session's prompt. `tools.rs` is what the model sees. Settings →
//! Chat shows every note and can edit, delete or turn the whole thing off (on by default).

pub mod commands;
pub mod tools;

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const SCHEMA_VERSION: i64 = 1;
pub const KINDS: [&str; 4] = ["you", "topic", "area", "person"];
const MAX_NOTES: i64 = 300;
const MAX_TITLE_CHARS: usize = 60;
const MAX_TEXT_CHARS: usize = 300;
/// What of the notes goes into a prompt, newest first; the rest stay reachable with a search.
const MAX_PROMPT_BYTES: usize = 4096;
const MAX_FOUND: usize = 20;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub id: i64,
    pub kind: String,
    pub title: String,
    pub text: String,
    /// The mode it was last written in: chat, code, academic or user.
    pub mode: String,
    pub created_at: i64,
    pub updated_at: i64,
}

pub struct Store {
    conn: Connection,
}

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0))
}

fn sql(e: rusqlite::Error) -> String {
    format!("memory: {e}")
}

/// A long unbroken run of letters and digits, or a known key prefix: not something to keep in a note.
fn looks_secret(text: &str) -> bool {
    text.contains("-----BEGIN")
        || text.split(|c: char| c.is_whitespace() || "\"'`,;:()".contains(c)).any(|word| {
            let keyish = ["sk-", "ghp_", "gho_", "AKIA", "xoxb-", "AIza"].iter().any(|p| word.starts_with(p));
            let long = word.len() >= 32 && word.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
            (keyish && word.len() >= 16) || long
        })
}

fn clean_text(text: &str) -> Result<String, String> {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.is_empty() {
        return Err("`text` is empty: one short sentence".into());
    }
    if text.chars().count() > MAX_TEXT_CHARS {
        return Err(format!("`text` is over {MAX_TEXT_CHARS} characters: one short sentence, merged with what the note said"));
    }
    if looks_secret(&text) {
        return Err("that looks like a key or token: never keep those in memory".into());
    }
    Ok(text)
}

impl Store {
    /// `~/.local/share/npi/memory.db`.
    pub fn default_path() -> Result<PathBuf, String> {
        let home = std::env::var_os("HOME").filter(|h| !h.is_empty()).ok_or("no home folder")?;
        Ok(PathBuf::from(home).join(".local/share/npi/memory.db"))
    }

    pub fn open_default() -> Result<Self, String> {
        Self::open(&Self::default_path()?)
    }

    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(dir) = path.parent() {
            crate::private_fs::private_dirs(true).create(dir).map_err(|e| format!("memory folder: {e}"))?;
        }
        let conn = Connection::open(path).map_err(sql)?;
        conn.busy_timeout(std::time::Duration::from_secs(3)).map_err(sql)?;
        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).map_err(sql)?;
        if version > SCHEMA_VERSION {
            return Err("memory.db is from a newer version of the app".into());
        }
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS notes (
                 id INTEGER PRIMARY KEY,
                 kind TEXT NOT NULL,
                 title TEXT NOT NULL COLLATE NOCASE,
                 text TEXT NOT NULL,
                 mode TEXT NOT NULL,
                 created_at INTEGER NOT NULL,
                 updated_at INTEGER NOT NULL,
                 UNIQUE (title)
             );
             CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
             PRAGMA user_version = 1;",
        )
        .map_err(sql)?;
        Ok(Self { conn })
    }

    fn meta(&self, key: &str) -> Result<Option<String>, String> {
        self.conn.query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0)).optional().map_err(sql)
    }

    fn set_meta(&self, key: &str, value: &str) -> Result<(), String> {
        self.conn
            .execute("INSERT INTO meta (key, value) VALUES (?1, ?2) ON CONFLICT (key) DO UPDATE SET value = excluded.value", params![key, value])
            .map(|_| ())
            .map_err(sql)
    }

    /// On unless the user turned it off.
    pub fn enabled(&self) -> bool {
        self.meta("enabled").ok().flatten().is_none_or(|v| v != "0")
    }

    pub fn set_enabled(&self, on: bool) -> Result<(), String> {
        self.set_meta("enabled", if on { "1" } else { "0" })
    }

    /// Writes the note `title`, replacing what it said. Returns whether it already existed.
    pub fn save(&self, kind: &str, title: &str, text: &str, mode: &str) -> Result<bool, String> {
        if !KINDS.contains(&kind) {
            return Err(format!("`kind` is one of {}", KINDS.join(", ")));
        }
        let title = title.split_whitespace().collect::<Vec<_>>().join(" ");
        if title.is_empty() || title.chars().count() > MAX_TITLE_CHARS {
            return Err(format!("`title` is 1 to {MAX_TITLE_CHARS} characters, the note's subject"));
        }
        let text = clean_text(text)?;
        let time = now();
        let changed = self
            .conn
            .execute("UPDATE notes SET kind = ?1, text = ?2, mode = ?3, updated_at = ?4 WHERE title = ?5", params![kind, text, mode, time, title])
            .map_err(sql)?;
        if changed > 0 {
            return Ok(true);
        }
        let count: i64 = self.conn.query_row("SELECT COUNT(*) FROM notes", [], |r| r.get(0)).map_err(sql)?;
        if count >= MAX_NOTES {
            return Err("the memory is full: merge notes, or use forget_memory on one that no longer matters".into());
        }
        self.conn
            .execute("INSERT INTO notes (kind, title, text, mode, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?5)", params![kind, title, text, mode, time])
            .map_err(sql)?;
        Ok(false)
    }

    /// Every note, newest first.
    pub fn list(&self) -> Result<Vec<Note>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, kind, title, text, mode, created_at, updated_at FROM notes ORDER BY updated_at DESC, id DESC")
            .map_err(sql)?;
        let rows = stmt
            .query_map([], |r| {
                Ok(Note { id: r.get(0)?, kind: r.get(1)?, title: r.get(2)?, text: r.get(3)?, mode: r.get(4)?, created_at: r.get(5)?, updated_at: r.get(6)? })
            })
            .map_err(sql)?;
        rows.collect::<Result<_, _>>().map_err(sql)
    }

    /// The notes holding every word of `query`, ignoring case.
    pub fn search(&self, query: &str) -> Result<Vec<Note>, String> {
        let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
        let mut notes = self.list()?;
        notes.retain(|n| {
            let hay = format!("{} {}", n.title, n.text).to_lowercase();
            words.iter().all(|w| hay.contains(w))
        });
        notes.truncate(MAX_FOUND);
        Ok(notes)
    }

    /// Deletes the note `title`. Returns whether there was one.
    pub fn forget(&self, title: &str) -> Result<bool, String> {
        self.conn.execute("DELETE FROM notes WHERE title = ?1", [title.trim()]).map(|n| n > 0).map_err(sql)
    }

    pub fn delete(&self, id: i64) -> Result<(), String> {
        self.conn.execute("DELETE FROM notes WHERE id = ?1", [id]).map(|_| ()).map_err(sql)
    }

    /// The user's own edit of a note.
    pub fn update(&self, id: i64, text: &str) -> Result<(), String> {
        let text = clean_text(text)?;
        self.conn.execute("UPDATE notes SET text = ?2, mode = 'user', updated_at = ?3 WHERE id = ?1", params![id, text, now()]).map(|_| ()).map_err(sql)
    }

    pub fn clear(&self) -> Result<(), String> {
        self.conn.execute("DELETE FROM notes", []).map(|_| ()).map_err(sql)
    }

    /// Takes in the lines of the Chat folder's old `MEMORY.md`, once.
    pub fn import_chat_memory(&self, text: &str) -> Result<(), String> {
        if self.meta("chat-memory-imported")?.is_some() {
            return Ok(());
        }
        for line in text.lines() {
            let fact = line.trim().trim_start_matches(['-', '*']).trim();
            if !fact.is_empty() && !fact.starts_with('#') {
                let title: String = fact.chars().take(30).collect();
                // A line too long or that looks like a key stays in the old file, which is not deleted.
                let _ = self.save("you", &title, fact, "chat");
            }
        }
        self.set_meta("chat-memory-imported", "1")
    }

    /// The notes as a prompt shows them, newest first, within the size limit.
    fn prompt_lines(&self) -> Result<(String, usize), String> {
        let notes = self.list()?;
        let mut out = String::new();
        let mut left_out = 0;
        for n in &notes {
            let line = format!("- [{}] {}: {}\n", n.kind, n.title, n.text);
            if out.len() + line.len() > MAX_PROMPT_BYTES {
                left_out += 1;
            } else {
                out.push_str(&line);
            }
        }
        Ok((out, left_out))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    fn store(d: &TempDir) -> Store {
        Store::open(&d.path().join("share/npi/memory.db")).unwrap()
    }

    #[test]
    fn a_note_is_saved_then_replaced_under_its_title() {
        let d = TempDir::new("memory", "save");
        let s = store(&d);
        assert!(!s.save("area", "Paper Agent", "Builds an agent for papers.", "code").unwrap());
        assert!(s.save("topic", "paper agent", "Builds an agent  for\npapers, in Rust.", "chat").unwrap());
        let notes = s.list().unwrap();
        assert_eq!(notes.len(), 1, "the same title whatever the case");
        assert_eq!((notes[0].kind.as_str(), notes[0].text.as_str(), notes[0].mode.as_str()), ("topic", "Builds an agent for papers, in Rust.", "chat"));
    }

    #[test]
    fn what_is_not_a_note_is_refused_with_the_reason() {
        let d = TempDir::new("memory", "refuse");
        let s = store(&d);
        assert!(s.save("nonsense", "T", "x", "chat").unwrap_err().contains("kind"));
        assert!(s.save("you", " ", "x", "chat").unwrap_err().contains("title"));
        assert!(s.save("you", "T", "  ", "chat").unwrap_err().contains("text"));
        assert!(s.save("you", "T", &"x ".repeat(200), "chat").unwrap_err().contains("300"));
        for secret in ["my key is sk-abcdefghijklmnop1234", "token: 0123456789abcdef0123456789abcdef0123", "-----BEGIN PRIVATE KEY-----"] {
            assert!(s.save("you", "T", secret, "chat").unwrap_err().contains("key"), "{secret}");
        }
        assert!(s.list().unwrap().is_empty());
        assert!(!looks_secret("studying abroad in Osaka, works on electrical impedance tomography"));
    }

    #[test]
    fn a_search_needs_every_word_and_notes_can_be_changed_and_removed() {
        let d = TempDir::new("memory", "search");
        let s = store(&d);
        s.save("topic", "Travel", "Plans a Hokkaido trip in winter.", "chat").unwrap();
        s.save("you", "Study", "Studies in Osaka.", "chat").unwrap();
        assert_eq!(s.search("hokkaido TRIP").unwrap().len(), 1);
        assert!(s.search("hokkaido osaka").unwrap().is_empty());
        let id = s.search("osaka").unwrap()[0].id;
        s.update(id, "Studies in Kyoto.").unwrap();
        assert_eq!(s.search("kyoto").unwrap()[0].mode, "user");
        assert!(s.forget("travel").unwrap());
        assert!(!s.forget("travel").unwrap());
        s.delete(id).unwrap();
        s.save("you", "A", "b", "chat").unwrap();
        s.clear().unwrap();
        assert!(s.list().unwrap().is_empty());
    }

    #[test]
    fn it_is_on_until_the_user_turns_it_off_and_that_lasts() {
        let d = TempDir::new("memory", "enabled");
        assert!(store(&d).enabled());
        store(&d).set_enabled(false).unwrap();
        assert!(!store(&d).enabled());
    }

    #[test]
    fn the_old_memory_file_comes_in_once() {
        let d = TempDir::new("memory", "import");
        let s = store(&d);
        s.import_chat_memory("# Notes\n- Lives in Osaka\n\n* Likes tea\n").unwrap();
        assert_eq!(s.list().unwrap().len(), 2);
        s.clear().unwrap();
        s.import_chat_memory("- Lives in Osaka\n").unwrap();
        assert!(s.list().unwrap().is_empty(), "what the user deleted does not come back");
    }

    #[test]
    fn the_prompt_keeps_to_its_size_and_says_how_much_it_left_out() {
        let d = TempDir::new("memory", "prompt");
        let s = store(&d);
        for i in 0..60 {
            s.save("topic", &format!("Note {i}"), &"word ".repeat(40), "chat").unwrap();
        }
        let (text, left_out) = s.prompt_lines().unwrap();
        assert!(text.len() <= MAX_PROMPT_BYTES && left_out > 0);
    }

    #[test]
    fn a_newer_file_is_refused_not_touched() {
        let d = TempDir::new("memory", "newer");
        let path = d.path().join("memory.db");
        Connection::open(&path).unwrap().execute_batch("PRAGMA user_version = 99;").unwrap();
        assert!(Store::open(&path).is_err());
    }
}
