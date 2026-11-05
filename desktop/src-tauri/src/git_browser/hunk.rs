//! Discarding one change of a file: the rest of its edits stay. The view the user acted on is
//! worked out again here, and the line they named must still be what they saw.

use super::file::{self, FileKind, Line, LineKind};
use super::StatusKind;
use crate::workspace;
use std::path::Path;

/// The run of changed lines around `index`, as `(old start, old count, new start, new count)`,
/// 0-based, in lines of each side.
fn hunk_at(lines: &[Line], index: usize) -> (usize, usize, usize, usize) {
    let changed = |l: &Line| l.kind != LineKind::Ctx;
    let first = lines[..index]
        .iter()
        .rposition(|l| !changed(l))
        .map_or(0, |i| i + 1);
    let last = lines[index..]
        .iter()
        .position(|l| !changed(l))
        .map_or(lines.len(), |i| index + i);
    let run = &lines[first..last];
    let before = &lines[..first];
    let number = |n: Option<u32>| n.map_or(0, |n| n as usize);
    let old_start = number(before.iter().rev().find_map(|l| l.old));
    let new_start = number(before.iter().rev().find_map(|l| l.new));
    let olds = run.iter().filter(|l| l.kind == LineKind::Rem).count();
    let news = run.iter().filter(|l| l.kind == LineKind::Add).count();
    (old_start, olds, new_start, news)
}

/// `after` with `news` lines at `new_start` replaced by `olds` lines of `before` at `old_start`.
fn splice(
    before: &[u8],
    after: &[u8],
    (old_start, olds, new_start, news): (usize, usize, usize, usize),
) -> Option<Vec<u8>> {
    let lines = |b: &'_ [u8]| {
        b.split_inclusive(|c| *c == b'\n')
            .map(<[u8]>::to_vec)
            .collect::<Vec<_>>()
    };
    let (old_lines, new_lines) = (lines(before), lines(after));
    let crlf = after.windows(2).any(|w| w == b"\r\n");
    let mut out = Vec::with_capacity(after.len());
    new_lines
        .get(..new_start)?
        .iter()
        .for_each(|l| out.extend_from_slice(l));
    for line in old_lines.get(old_start..old_start + olds)? {
        match line.strip_suffix(b"\n") {
            // The work-tree file has CRLF and Git stores LF: the restored lines match their file.
            Some(body) if crlf && !body.ends_with(b"\r") => {
                out.extend_from_slice(body);
                out.extend_from_slice(b"\r\n");
            }
            _ => out.extend_from_slice(line),
        }
    }
    new_lines
        .get(new_start + news..)?
        .iter()
        .for_each(|l| out.extend_from_slice(l));
    Some(out)
}

/// Undoes the change holding the line numbered `old` / `new` whose text is `text`, in the working
/// tree only. Added, deleted and renamed files have one all-or-nothing change: discard the file.
pub fn discard_hunk(
    path: &Path,
    rel_path: &str,
    old: Option<u32>,
    new: Option<u32>,
    text: &str,
) -> Result<(), String> {
    const STALE: &str = "the file changed: look at it again";
    let view = file::file(path, rel_path)?;
    if view.kind != FileKind::Text || view.status != Some(StatusKind::Modified) || view.truncated {
        return Err(
            "only a change in a modified text file can be discarded on its own".to_string(),
        );
    }
    let index = view
        .lines
        .iter()
        .position(|l| l.kind != LineKind::Ctx && l.old == old && l.new == new && l.text == text)
        .ok_or(STALE)?;
    let range = hunk_at(&view.lines, index);

    let (_gate, repo, work_dir) = super::open(path)?;
    let (abs, rel) = workspace::validate_relative_path(&work_dir, &view.path)?;
    if std::fs::symlink_metadata(&abs)
        .map_err(|e| e.to_string())?
        .is_symlink()
    {
        return Err("a symlink has no lines to discard".to_string());
    }
    let tree = repo
        .head_tree()
        .map_err(|e| format!("failed to read HEAD tree: {e}"))?;
    let file::Side::Bytes(before) = file::blob_at(&repo, &tree, &rel)? else {
        return Err(STALE.to_string());
    };
    let after = std::fs::read(&abs).map_err(|e| format!("failed to read {rel}: {e}"))?;
    let result = splice(&before, &after, range).ok_or(STALE)?;
    std::fs::write(&abs, result).map_err(|e| format!("failed to write {rel}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    fn repo(dir: &TempDir, text: &str) -> std::path::PathBuf {
        let root = dir.path().canonicalize().unwrap();
        workspace::repo_init(&root).unwrap();
        std::fs::write(root.join("a.txt"), text).unwrap();
        workspace::commit_all(&root, "first").unwrap();
        root
    }

    fn discard(root: &Path, text: &str) -> Result<(), String> {
        let view = file::file(root, "a.txt").unwrap();
        let line = view
            .lines
            .iter()
            .find(|l| l.text == text && l.kind != LineKind::Ctx)
            .unwrap();
        discard_hunk(root, "a.txt", line.old, line.new, text)
    }

    #[test]
    fn one_change_goes_and_the_others_stay() {
        let dir = TempDir::new("hunk", "two");
        let root = repo(&dir, "a\nb\nc\nd\ne\nf\ng\n");
        std::fs::write(root.join("a.txt"), "a\nB\nc\nd\ne\nF\ng\nh\n").unwrap();
        discard(&root, "B").unwrap();
        assert_eq!(
            std::fs::read_to_string(root.join("a.txt")).unwrap(),
            "a\nb\nc\nd\ne\nF\ng\nh\n"
        );
        discard(&root, "h").unwrap();
        assert_eq!(
            std::fs::read_to_string(root.join("a.txt")).unwrap(),
            "a\nb\nc\nd\ne\nF\ng\n"
        );
    }

    #[test]
    fn a_removed_line_comes_back_in_its_place() {
        let dir = TempDir::new("hunk", "removed");
        let root = repo(&dir, "a\nb\nc\n");
        std::fs::write(root.join("a.txt"), "a\nc\n").unwrap();
        discard(&root, "b").unwrap();
        assert_eq!(
            std::fs::read_to_string(root.join("a.txt")).unwrap(),
            "a\nb\nc\n"
        );
    }

    #[test]
    fn a_stale_view_changes_nothing() {
        let dir = TempDir::new("hunk", "stale");
        let root = repo(&dir, "a\nb\n");
        std::fs::write(root.join("a.txt"), "a\nB\n").unwrap();
        let error = discard_hunk(&root, "a.txt", None, Some(2), "not what is there").unwrap_err();
        assert!(error.contains("changed"));
        assert_eq!(
            std::fs::read_to_string(root.join("a.txt")).unwrap(),
            "a\nB\n"
        );
    }

    #[test]
    fn a_new_file_is_discarded_whole() {
        let dir = TempDir::new("hunk", "added");
        let root = repo(&dir, "a\n");
        std::fs::write(root.join("b.txt"), "x\n").unwrap();
        assert!(discard_hunk(&root, "b.txt", None, Some(1), "x")
            .unwrap_err()
            .contains("modified"));
    }

    #[test]
    fn splicing_keeps_crlf_in_a_crlf_file() {
        let out = splice(b"a\nb\n", b"a\r\nB\r\n", (1, 1, 1, 1)).unwrap();
        assert_eq!(out, b"a\r\nb\r\n");
    }
}
