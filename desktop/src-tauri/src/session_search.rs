//! Search over the user's past conversations: omp's session files (`<agent dir>/sessions/*/*.jsonl`)
//! indexed into `~/.cache/npi/desktop/session-index.db`, a cache that rebuilds from those files.
//! Only what the user and the assistant said is indexed, never tool output. Each search first
//! brings the index up to date with the files (by size and modification time), so it needs no
//! watcher. CJK text is indexed as unigrams and bigrams (`librarian::cjk`), as the library is.

use crate::librarian::cjk;
use rusqlite::{params, Connection};
use std::collections::HashSet;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

const SCHEMA_VERSION: i64 = 1;
/// What of one message is kept: enough to find it and quote around the match.
const MAX_MESSAGE_CHARS: usize = 4000;
/// Characters of context on each side of the match in an excerpt.
const EXCERPT_SIDE: usize = 120;
/// Hits from one conversation in one answer, so one long chat does not fill it.
const PER_SESSION: usize = 2;

/// Which conversations a search may see.
pub struct Scope {
    /// The profile's sessions folder.
    pub root: PathBuf,
    /// Working folders (normalised, `saved_sessions::normalize_cwd`) a conversation must have run
    /// in; `None` for all of them.
    pub cwds: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    pub title: String,
    pub when: String,
    pub cwd: String,
    pub role: String,
    pub excerpt: String,
}

pub struct Index {
    conn: Connection,
}

fn sql(e: rusqlite::Error) -> String {
    format!("session index: {e}")
}

impl Index {
    pub fn open(path: &Path) -> Result<Self, String> {
        if let Some(dir) = path.parent() {
            crate::private_fs::private_dirs(true)
                .create(dir)
                .map_err(|e| format!("session index folder: {e}"))?;
        }
        let conn = Connection::open(path).map_err(sql)?;
        conn.busy_timeout(std::time::Duration::from_secs(3))
            .map_err(sql)?;
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .map_err(sql)?;
        if version != SCHEMA_VERSION {
            // A cache: another layout is dropped and rebuilt from the session files.
            conn.execute_batch("DROP TABLE IF EXISTS msgs_fts; DROP TABLE IF EXISTS msgs; DROP TABLE IF EXISTS files;").map_err(sql)?;
        }
        conn.execute_batch(&format!(
            "CREATE TABLE IF NOT EXISTS files (path TEXT PRIMARY KEY, mtime INTEGER NOT NULL, size INTEGER NOT NULL,
                 cwd TEXT NOT NULL, title TEXT NOT NULL, updated TEXT NOT NULL);
             CREATE TABLE IF NOT EXISTS msgs (id INTEGER PRIMARY KEY, path TEXT NOT NULL, role TEXT NOT NULL, ts TEXT NOT NULL, text TEXT NOT NULL);
             CREATE INDEX IF NOT EXISTS msgs_path ON msgs (path);
             CREATE VIRTUAL TABLE IF NOT EXISTS msgs_fts USING fts5 (norm);
             PRAGMA user_version = {SCHEMA_VERSION};"
        ))
        .map_err(sql)?;
        Ok(Self { conn })
    }

    /// Brings the index in line with the session files under `root`: new and changed files are
    /// read again, files that are gone are dropped.
    pub fn refresh(&mut self, root: &Path) -> Result<(), String> {
        let mut seen = HashSet::new();
        let tx = self.conn.transaction().map_err(sql)?;
        for path in session_files(root) {
            let Some(key) = path.to_str().map(str::to_string) else {
                continue;
            };
            let Ok(meta) = std::fs::metadata(&path) else {
                continue;
            };
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX));
            let size = i64::try_from(meta.len()).unwrap_or(i64::MAX);
            seen.insert(key.clone());
            let known: Option<(i64, i64)> = tx
                .query_row(
                    "SELECT mtime, size FROM files WHERE path = ?1",
                    [&key],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .ok();
            if known == Some((mtime, size)) {
                continue;
            }
            forget(&tx, &key)?;
            let Some(session) = crate::saved_sessions::parse_session_file(&path) else {
                continue;
            };
            let updated = session.updated_at.unwrap_or(session.timestamp);
            tx.execute(
                "INSERT INTO files (path, mtime, size, cwd, title, updated) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![key, mtime, size, session.cwd, session.title, updated],
            )
            .map_err(sql)?;
            for (role, ts, text) in messages(&path) {
                tx.execute(
                    "INSERT INTO msgs (path, role, ts, text) VALUES (?1, ?2, ?3, ?4)",
                    params![key, role, ts, text],
                )
                .map_err(sql)?;
                tx.execute(
                    "INSERT INTO msgs_fts (rowid, norm) VALUES (last_insert_rowid(), ?1)",
                    [cjk::index_text(&text)],
                )
                .map_err(sql)?;
            }
        }
        let prefix = format!("{}%", root.to_string_lossy());
        let stale: Vec<String> = {
            let mut stmt = tx
                .prepare("SELECT path FROM files WHERE path LIKE ?1")
                .map_err(sql)?;
            let rows = stmt.query_map([&prefix], |r| r.get(0)).map_err(sql)?;
            rows.filter_map(Result::ok)
                .filter(|p: &String| !seen.contains(p))
                .collect()
        };
        for path in &stale {
            forget(&tx, path)?;
        }
        tx.commit().map_err(sql)
    }

    /// The best matches for `query` in `scope`, at most `limit`, at most two per conversation.
    pub fn search(&mut self, query: &str, scope: &Scope, limit: usize) -> Result<Vec<Hit>, String> {
        self.refresh(&scope.root)?;
        let Some(expr) = cjk::match_expression(query) else {
            return Ok(Vec::new());
        };
        let prefix = format!("{}%", scope.root.to_string_lossy());
        let mut stmt = self
            .conn
            .prepare(
                "SELECT m.path, m.role, m.ts, m.text, f.title, f.cwd, f.updated FROM msgs_fts
                 JOIN msgs m ON m.id = msgs_fts.rowid JOIN files f ON f.path = m.path
                 WHERE msgs_fts MATCH ?1 AND f.path LIKE ?2 ORDER BY rank LIMIT 300",
            )
            .map_err(sql)?;
        let rows = stmt
            .query_map(params![expr, prefix], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, String>(6)?,
                ))
            })
            .map_err(sql)?;
        let terms = cjk::highlight_terms(query);
        let mut per_session: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        let mut hits = Vec::new();
        for (path, role, ts, text, title, cwd, updated) in rows.filter_map(Result::ok) {
            if let Some(cwds) = &scope.cwds {
                if !cwds.contains(&crate::saved_sessions::normalize_cwd(&cwd)) {
                    continue;
                }
            }
            let count = per_session.entry(path).or_default();
            if *count >= PER_SESSION {
                continue;
            }
            *count += 1;
            let when = if ts.is_empty() { updated } else { ts };
            hits.push(Hit {
                title,
                when: when.chars().take(10).collect(),
                cwd,
                role,
                excerpt: excerpt(&text, &terms),
            });
            if hits.len() >= limit {
                break;
            }
        }
        Ok(hits)
    }
}

fn forget(tx: &rusqlite::Transaction<'_>, path: &str) -> Result<(), String> {
    tx.execute(
        "DELETE FROM msgs_fts WHERE rowid IN (SELECT id FROM msgs WHERE path = ?1)",
        [path],
    )
    .map_err(sql)?;
    tx.execute("DELETE FROM msgs WHERE path = ?1", [path])
        .map_err(sql)?;
    tx.execute("DELETE FROM files WHERE path = ?1", [path])
        .map_err(sql)?;
    Ok(())
}

/// `<root>/<folder>/*.jsonl`, the layout omp writes.
fn session_files(root: &Path) -> Vec<PathBuf> {
    let Ok(folders) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    folders
        .flatten()
        .filter_map(|f| std::fs::read_dir(f.path()).ok())
        .flat_map(|files| files.flatten().map(|f| f.path()))
        .filter(|p| {
            p.is_file()
                && p.extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("jsonl"))
        })
        .collect()
}

/// What the user and the assistant said in a session file: `(role, timestamp, text)`.
fn messages(path: &Path) -> Vec<(String, String, String)> {
    let Ok(file) = std::fs::File::open(path) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for line in BufReader::new(file).lines().map_while(Result::ok) {
        let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        if val.get("type").and_then(|t| t.as_str()) != Some("message") {
            continue;
        }
        let Some(msg) = val.get("message") else {
            continue;
        };
        let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("");
        if role != "user" && role != "assistant" {
            continue;
        }
        let Some(text) = msg
            .get("content")
            .and_then(crate::saved_sessions::extract_text_from_content)
        else {
            continue;
        };
        let ts = val
            .get("timestamp")
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .to_string();
        out.push((
            role.to_string(),
            ts,
            text.chars().take(MAX_MESSAGE_CHARS).collect(),
        ));
    }
    out
}

/// One line of `text` around the first of `terms` it holds (the start when none is found).
fn excerpt(text: &str, terms: &[Vec<char>]) -> String {
    let chars: Vec<char> = text
        .chars()
        .map(|c| if c.is_whitespace() { ' ' } else { c })
        .collect();
    let lower: Vec<char> = chars.iter().flat_map(|c| c.to_lowercase()).collect();
    // Lower-casing can change the length (rare outside a few scripts); then cut from the start.
    let at = if lower.len() == chars.len() {
        terms
            .iter()
            .find_map(|t| {
                lower
                    .windows(t.len().max(1))
                    .position(|w| w == t.as_slice())
            })
            .unwrap_or(0)
    } else {
        0
    };
    let start = at.saturating_sub(EXCERPT_SIDE);
    let end = (at + EXCERPT_SIDE).min(chars.len());
    let mut out: String = chars[start..end].iter().collect();
    out = out.split_whitespace().collect::<Vec<_>>().join(" ");
    if start > 0 {
        out.insert(0, '…');
    }
    if end < chars.len() {
        out.push('…');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    fn session(root: &Path, folder: &str, name: &str, cwd: &str, said: &[(&str, &str)]) -> PathBuf {
        let dir = root.join(folder);
        std::fs::create_dir_all(&dir).unwrap();
        let mut lines = vec![format!(
            r#"{{"type":"session","id":"{name}","timestamp":"2026-11-01T10:00:00Z","cwd":"{cwd}"}}"#
        )];
        for (role, text) in said {
            lines.push(format!(r#"{{"type":"message","timestamp":"2026-11-02T10:00:00Z","message":{{"role":"{role}","content":[{{"type":"text","text":"{text}"}}]}}}}"#));
        }
        lines.push(r#"{"type":"message","message":{"role":"toolResult","content":[{"type":"text","text":"secret tool output about osaka"}]}}"#.to_string());
        let path = dir.join(format!("2026-11-01T10-00-00Z_{name}.jsonl"));
        std::fs::write(&path, lines.join("\n")).unwrap();
        path
    }

    #[test]
    fn finds_what_was_said_in_chinese_and_english_but_not_tool_output() {
        let d = TempDir::new("session-search", "find");
        let root = d.path().join("sessions");
        session(
            &root,
            "chat",
            "a",
            "/u/NPi/Chat",
            &[
                ("user", "下周去大阪看烟花大会"),
                ("assistant", "Take the JR line to Osaka."),
            ],
        );
        let mut index = Index::open(&d.path().join("index.db")).unwrap();
        let all = Scope { root, cwds: None };
        let hits = index.search("烟花", &all, 10).unwrap();
        assert_eq!(hits.len(), 1);
        assert!(
            hits[0].excerpt.contains("烟花大会")
                && hits[0].role == "user"
                && hits[0].when == "2026-11-02"
        );
        assert_eq!(
            index.search("osaka", &all, 10).unwrap().len(),
            1,
            "the tool result is not indexed"
        );
        assert_eq!(index.search("  ", &all, 10).unwrap(), []);
    }

    #[test]
    fn a_scope_keeps_to_its_folders_and_the_index_follows_the_files() {
        let d = TempDir::new("session-search", "scope");
        let root = d.path().join("sessions");
        session(
            &root,
            "p1",
            "a",
            "/u/Code/One",
            &[("user", "rust borrow checker")],
        );
        let gone = session(
            &root,
            "p2",
            "b",
            "/u/Code/Two",
            &[("user", "rust lifetimes")],
        );
        let mut index = Index::open(&d.path().join("index.db")).unwrap();
        let one = Scope {
            root: root.clone(),
            cwds: Some(vec![crate::saved_sessions::normalize_cwd("/u/Code/One")]),
        };
        assert_eq!(index.search("rust", &one, 10).unwrap().len(), 1);
        let all = Scope {
            root: root.clone(),
            cwds: None,
        };
        assert_eq!(index.search("rust", &all, 10).unwrap().len(), 2);
        std::fs::remove_file(gone).unwrap();
        session(
            &root,
            "p1",
            "a",
            "/u/Code/One",
            &[
                ("user", "rust borrow checker"),
                ("assistant", "rust traits too"),
            ],
        );
        let hits = index.search("rust", &all, 10).unwrap();
        assert_eq!(
            hits.len(),
            2,
            "the removed file is dropped, the changed one read again"
        );
        assert!(hits.iter().all(|h| h.cwd == "/u/Code/One"));
    }

    #[test]
    fn one_conversation_gives_at_most_two_hits() {
        let d = TempDir::new("session-search", "cap");
        let root = d.path().join("sessions");
        let said: Vec<(&str, &str)> = (0..5).map(|_| ("user", "the same idea again")).collect();
        session(&root, "c", "a", "/u/c", &said);
        let mut index = Index::open(&d.path().join("index.db")).unwrap();
        assert_eq!(
            index
                .search("idea", &Scope { root, cwds: None }, 10)
                .unwrap()
                .len(),
            PER_SESSION
        );
    }

    #[test]
    fn an_excerpt_is_cut_around_the_match() {
        let text = format!("{}needle{}", "a ".repeat(200), " b".repeat(200));
        let cut = excerpt(&text, &cjk::highlight_terms("needle"));
        assert!(cut.starts_with('…') && cut.ends_with('…') && cut.contains("needle"));
        assert!(cut.chars().count() <= EXCERPT_SIDE * 2 + 2);
    }
}
