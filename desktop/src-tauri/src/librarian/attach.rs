//! Files attached to a reference. A PDF the user adds is first *staged* in
//! `<project>/.npi-academic/inbox/<hash>.pdf` (so a draft that is discarded
//! leaves nothing in `papers/`), and once the user confirms it is copied to
//! `papers/<year>-<author>-<short title>.pdf` and recorded here. The user's
//! original is never moved or changed; the library keeps a project-relative
//! path and the content hash, so a moved file shows as missing, not as gone.

use super::store::{now, sql, Library};
use rusqlite::params;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

const INBOX: &str = ".npi-academic/inbox";
const PAPERS: &str = "papers";
/// A staged PDF nobody confirmed is removed after this long.
const INBOX_KEEP: Duration = Duration::from_secs(7 * 24 * 3600);
/// Largest PDF attached.
pub const MAX_PDF_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    pub id: i64,
    /// Relative to the project folder.
    pub path: String,
    pub hash: String,
    pub kind: String,
    pub added_at: i64,
    /// The file is no longer where it was recorded.
    pub missing: bool,
}

/// A PDF waiting in the inbox for the user's confirmation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Staged {
    pub hash: String,
    /// The file's own name, for the draft's title when the PDF has none.
    pub name: String,
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

fn hash_valid(hash: &str) -> bool {
    hash.len() == 64 && hash.chars().all(|c| c.is_ascii_hexdigit())
}

/// Copies `source` (a PDF) into the project's inbox under its content hash.
pub fn stage(project: &Path, source: &Path) -> Result<Staged, String> {
    let meta = std::fs::metadata(source).map_err(|e| format!("could not read the file: {e}"))?;
    if meta.len() > MAX_PDF_BYTES {
        return Err("the PDF is too large".into());
    }
    let bytes = std::fs::read(source).map_err(|e| format!("could not read the file: {e}"))?;
    if !bytes.starts_with(b"%PDF") {
        return Err("that file is not a PDF".into());
    }
    let hash = hex(&Sha256::digest(&bytes));
    let dir = project.join(INBOX);
    std::fs::create_dir_all(&dir).map_err(|e| format!("could not create the inbox: {e}"))?;
    sweep(&dir);
    let target = dir.join(format!("{hash}.pdf"));
    if !target.exists() {
        std::fs::write(&target, &bytes).map_err(|e| format!("could not stage the PDF: {e}"))?;
    }
    let name = source
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("paper")
        .to_string();
    Ok(Staged { hash, name })
}

/// Removes staged PDFs older than [`INBOX_KEEP`].
fn sweep(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let old = e
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| SystemTime::now().duration_since(t).ok())
            .is_some_and(|age| age > INBOX_KEEP);
        if old {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

/// A file name from a record: `<year>-<first author>-<short title>`, safe on every OS.
pub fn paper_name(csl: &serde_json::Value, fallback: &str) -> String {
    let f = super::csl::fields(csl);
    let part = |text: &str, max: usize| -> String {
        let mut out = String::new();
        for c in text.chars() {
            if c.is_alphanumeric() {
                out.push(c);
            } else if !out.ends_with('-') && !out.is_empty() {
                out.push('-');
            }
            if out.chars().count() >= max {
                break;
            }
        }
        out.trim_matches('-').to_string()
    };
    let parts: Vec<String> = [
        f.year.map(|y| y.to_string()).unwrap_or_default(),
        part(&f.first_author, 24),
        part(&f.title, 48),
    ]
    .into_iter()
    .filter(|p| !p.is_empty())
    .collect();
    if parts.is_empty() {
        let name = part(fallback, 60);
        return if name.is_empty() { "paper".into() } else { name };
    }
    parts.join("-")
}

/// Moves the staged PDF `hash` to `papers/<stem>.pdf` (a number is added if another file
/// has that name); the same file already there is reused. Returns the relative path.
pub fn finalize(project: &Path, hash: &str, stem: &str) -> Result<String, String> {
    if !hash_valid(hash) {
        return Err("unknown staged file".into());
    }
    let staged = project.join(INBOX).join(format!("{hash}.pdf"));
    let papers = project.join(PAPERS);
    std::fs::create_dir_all(&papers).map_err(|e| format!("could not create papers/: {e}"))?;
    for n in 0.. {
        let name = if n == 0 { format!("{stem}.pdf") } else { format!("{stem}-{n}.pdf") };
        let target: PathBuf = papers.join(&name);
        if target.exists() {
            let same = std::fs::read(&target)
                .map(|b| hex(&Sha256::digest(&b)) == hash)
                .unwrap_or(false);
            if same {
                let _ = std::fs::remove_file(&staged);
                return Ok(format!("{PAPERS}/{name}"));
            }
            continue;
        }
        if !staged.is_file() {
            return Err("the staged PDF is gone; choose the file again".into());
        }
        std::fs::copy(&staged, &target).map_err(|e| format!("could not copy the PDF: {e}"))?;
        let _ = std::fs::remove_file(&staged);
        return Ok(format!("{PAPERS}/{name}"));
    }
    unreachable!()
}

impl Library {
    pub(super) fn attachments_by_ref(&self) -> Result<HashMap<i64, Vec<Attachment>>, String> {
        let mut out: HashMap<i64, Vec<Attachment>> = HashMap::new();
        let mut stmt = self
            .conn
            .prepare("SELECT id, ref_id, path, hash, kind, added_at FROM attachments ORDER BY id")
            .map_err(sql)?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, i64>(1)?,
                    Attachment {
                        id: r.get(0)?,
                        path: r.get(2)?,
                        hash: r.get(3)?,
                        kind: r.get(4)?,
                        added_at: r.get(5)?,
                        missing: false,
                    },
                ))
            })
            .map_err(sql)?;
        for row in rows {
            let (ref_id, a) = row.map_err(sql)?;
            out.entry(ref_id).or_default().push(a);
        }
        Ok(out)
    }

    /// Records `path` (project-relative) as a file of reference `ref_id`; the same hash is recorded once.
    pub fn attach(&self, ref_id: i64, path: &str, hash: &str, kind: &str) -> Result<(), String> {
        let exists: bool = self
            .conn
            .query_row("SELECT 1 FROM attachments WHERE ref_id = ?1 AND hash = ?2", params![ref_id, hash], |_| Ok(()))
            .is_ok();
        if exists {
            return Ok(());
        }
        self.conn
            .execute(
                "INSERT INTO attachments (ref_id, path, hash, kind, added_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![ref_id, path, hash, kind, now()],
            )
            .map_err(sql)?;
        Ok(())
    }

    /// Forgets an attachment; the file stays where it is.
    pub fn detach(&self, attachment: i64) -> Result<bool, String> {
        Ok(self
            .conn
            .execute("DELETE FROM attachments WHERE id = ?1", [attachment])
            .map_err(sql)?
            > 0)
    }

    /// The recorded path of an attachment of a live reference.
    pub fn attachment_path(&self, attachment: i64) -> Result<Option<String>, String> {
        self.conn
            .query_row(
                "SELECT a.path FROM attachments a JOIN refs r ON r.id = a.ref_id
                 WHERE a.id = ?1 AND r.deleted_at IS NULL",
                [attachment],
                |r| r.get(0),
            )
            .map(Some)
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                other => Err(sql(other)),
            })
    }
}

/// Marks attachments whose file is not at the recorded path.
pub fn mark_missing(project: &Path, entries: &mut [super::Entry]) {
    for e in entries {
        for a in &mut e.attachments {
            a.missing = !project.join(&a.path).is_file();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdf_text::tiny_pdf;
    use crate::test_support::TempDir;
    use serde_json::json;

    #[test]
    fn a_pdf_is_staged_confirmed_named_and_not_taken_twice() {
        let dir = TempDir::new("librarian", "attach");
        let project = dir.path().join("p");
        let src = dir.path().join("Some Paper.pdf");
        std::fs::write(&src, tiny_pdf("hello")).unwrap();
        let staged = stage(&project, &src).unwrap();
        assert_eq!(staged.name, "Some Paper");
        assert!(project.join(INBOX).join(format!("{}.pdf", staged.hash)).is_file());
        assert!(!project.join(PAPERS).exists(), "nothing in papers/ before the user confirms");

        let stem = paper_name(&json!({"title": "Deep Learning: a survey", "author": [{"family": "Smith"}], "issued": {"date-parts": [[2020]]}}), "x");
        assert_eq!(stem, "2020-Smith-Deep-Learning-a-survey");
        let rel = finalize(&project, &staged.hash, &stem).unwrap();
        assert_eq!(rel, "papers/2020-Smith-Deep-Learning-a-survey.pdf");
        assert!(project.join(&rel).is_file());
        assert!(src.is_file(), "the original is untouched");

        // The same file again reuses it; a different file with that name gets a number.
        let again = stage(&project, &src).unwrap();
        assert_eq!(finalize(&project, &again.hash, &stem).unwrap(), rel);
        let other = dir.path().join("other.pdf");
        std::fs::write(&other, tiny_pdf("different")).unwrap();
        let staged2 = stage(&project, &other).unwrap();
        assert_eq!(finalize(&project, &staged2.hash, &stem).unwrap(), "papers/2020-Smith-Deep-Learning-a-survey-1.pdf");
    }

    #[test]
    fn only_pdfs_and_known_hashes_get_through() {
        let dir = TempDir::new("librarian", "attach-bad");
        let project = dir.path().join("p");
        let txt = dir.path().join("a.pdf");
        std::fs::write(&txt, "not a pdf").unwrap();
        assert!(stage(&project, &txt).unwrap_err().contains("not a PDF"));
        assert!(finalize(&project, "../../etc/passwd", "x").is_err());
        assert!(finalize(&project, &"a".repeat(64), "x").unwrap_err().contains("gone"));
    }

    #[test]
    fn names_fall_back_and_stay_safe() {
        assert_eq!(paper_name(&json!({}), "My file (final).v2"), "My-file-final-v2");
        assert_eq!(paper_name(&json!({}), "///"), "paper");
        assert!(!paper_name(&json!({"title": "a/b\\c:d"}), "x").contains(['/', '\\', ':']));
    }

    #[test]
    fn attachments_are_listed_marked_missing_and_forgotten() {
        let dir = TempDir::new("librarian", "attach-db");
        let project = dir.path().join("p");
        let lib = Library::open(&project).unwrap();
        let e = lib.add(&json!({"title": "T"}), "manual").unwrap();
        lib.attach(e.id, "papers/t.pdf", &"b".repeat(64), "pdf").unwrap();
        lib.attach(e.id, "papers/t.pdf", &"b".repeat(64), "pdf").unwrap();
        let mut list = lib.list().unwrap();
        assert_eq!(list[0].attachments.len(), 1);
        mark_missing(&project, &mut list);
        assert!(list[0].attachments[0].missing);
        let id = list[0].attachments[0].id;
        assert_eq!(lib.attachment_path(id).unwrap().as_deref(), Some("papers/t.pdf"));
        assert!(lib.detach(id).unwrap());
        assert!(lib.list().unwrap()[0].attachments.is_empty());
        lib.remove(e.id).unwrap();
        assert_eq!(lib.attachment_path(id).unwrap(), None);
    }
}
