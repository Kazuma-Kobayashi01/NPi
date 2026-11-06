//! One Git worktree per task: a Code tab can run in its own checkout on its own branch, so several
//! tasks in one project do not touch each other's files. The worktrees the app makes hold the user's
//! work, so they live beside the Code projects, in `~/Documents/NPi/Code/.worktrees/<repo>/<task>`
//! (not the command line's sibling `.agent-worktrees`, which `npi worktree clear` empties). What a
//! worktree is comes from Git itself and that folder, so there is no registry to fall out of step.

use crate::workspace::{git_output, git_succeeds};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Created {
    pub path: String,
    pub branch: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Info {
    /// The checkout the worktree came from: the tab groups under this project.
    pub main: String,
    pub branch: String,
    /// The branch the main checkout is on now, which a merge goes into.
    pub target: String,
    /// Commits on `branch` that `target` lacks.
    pub ahead: u32,
    /// Commits on `target` that `branch` lacks.
    pub behind: u32,
    /// Files with uncommitted changes.
    pub dirty: u32,
}

fn base_dir() -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME").ok_or("HOME is not set")?;
    Ok(PathBuf::from(home).join("Documents/NPi/Code/.worktrees"))
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).trim().to_string()
}

fn path_text(path: &Path) -> Result<String, String> {
    path.to_str()
        .map(str::to_string)
        .ok_or_else(|| format!("not a UTF-8 path: {}", path.display()))
}

/// The main checkout of the repository holding `path`.
fn main_root(path: &Path) -> Result<PathBuf, String> {
    let out = git_output(
        path,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        "rev-parse",
    )?;
    let common = PathBuf::from(text(&out));
    match common.file_name() {
        Some(name) if name == ".git" => common
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| "the repository has no main checkout".to_string()),
        _ => Err("the repository has no main checkout".to_string()),
    }
}

fn branch_of(path: &Path) -> Result<String, String> {
    let name = text(&git_output(
        path,
        &["rev-parse", "--abbrev-ref", "HEAD"],
        "rev-parse",
    )?);
    if name == "HEAD" {
        return Err(format!("{} is not on a branch", path.display()));
    }
    Ok(name)
}

/// Short lowercase base-36: a branch name that sorts by time and is easy to read out.
fn base36(mut n: u64) -> String {
    let mut out = Vec::new();
    while n > 0 {
        out.push(char::from_digit(u32::try_from(n % 36).unwrap_or(0), 36).unwrap_or('0'));
        n /= 36;
    }
    out.iter().rev().collect()
}

/// A folder name for the repository at `root`: readable, and unique to that path.
fn repo_folder(root: &Path) -> String {
    let name = root.file_name().and_then(|n| n.to_str()).unwrap_or("repo");
    let mut hash = String::new();
    for byte in &Sha256::digest(root.to_string_lossy().as_bytes())[..4] {
        let _ = write!(hash, "{byte:02x}");
    }
    let name: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    format!("{name}-{hash}")
}

/// A new worktree of the repository holding `project`, on a new branch from its `HEAD`. Uncommitted
/// changes stay where they are: the task starts from what is committed.
fn create_in(base: &Path, project: &Path) -> Result<Created, String> {
    let root = main_root(project)?;
    if !git_succeeds(
        &root,
        &["rev-parse", "--verify", "--quiet", "HEAD"],
        "rev-parse",
    )? {
        return Err(
            "this repository has no commits yet: commit once, then start a worktree task"
                .to_string(),
        );
    }
    let parent = base.join(repo_folder(&root));
    std::fs::create_dir_all(&parent)
        .map_err(|e| format!("failed to create {}: {e}", parent.display()))?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    for attempt in 0..u64::from(u8::MAX) {
        let slug = format!("task-{}", base36(now + attempt));
        let branch = format!("npi/{slug}");
        let path = parent.join(&slug);
        let taken = path.exists()
            || git_succeeds(
                &root,
                &[
                    "show-ref",
                    "--verify",
                    "--quiet",
                    &format!("refs/heads/{branch}"),
                ],
                "show-ref",
            )?;
        if taken {
            continue;
        }
        let target = path_text(&path)?;
        git_output(
            &root,
            &["worktree", "add", "--quiet", "-b", &branch, &target, "HEAD"],
            "worktree add",
        )?;
        return Ok(Created {
            path: target,
            branch,
        });
    }
    Err("could not pick a free branch name".to_string())
}

pub fn create(project: &Path) -> Result<Created, String> {
    create_in(&base_dir()?, project)
}

/// Whether `path` is a worktree NPi made: only those are grouped, merged and removed from here. A
/// worktree the user keeps somewhere else stays an ordinary project.
fn managed(base: &Path, path: &Path) -> bool {
    match (base.canonicalize(), path.canonicalize()) {
        (Ok(base), Ok(path)) => path.starts_with(base),
        _ => false,
    }
}

fn info_in(base: &Path, path: &Path) -> Result<Option<Info>, String> {
    if !managed(base, path) {
        return Ok(None);
    }
    let main = main_root(path)?;
    let branch = branch_of(path)?;
    let target = branch_of(&main)?;
    let counts = text(&git_output(
        path,
        &[
            "rev-list",
            "--left-right",
            "--count",
            &format!("{target}...{branch}"),
        ],
        "rev-list",
    )?);
    let mut counts = counts.split_whitespace().map(|n| n.parse().unwrap_or(0));
    let (behind, ahead) = (counts.next().unwrap_or(0), counts.next().unwrap_or(0));
    let status = git_output(path, &["status", "--porcelain"], "status")?;
    let dirty = u32::try_from(
        status
            .split(|b| *b == b'\n')
            .filter(|l| !l.is_empty())
            .count(),
    )
    .unwrap_or(u32::MAX);
    Ok(Some(Info {
        main: path_text(&main)?,
        branch,
        target,
        ahead,
        behind,
        dirty,
    }))
}

pub fn info(path: &Path) -> Result<Option<Info>, String> {
    info_in(&base_dir()?, path)
}

/// Why a merge stopped on conflicts, after undoing it: the main checkout is left as it was.
fn abandon_merge(main: &Path, branch: &str, target: &str) -> Result<String, String> {
    let names = text(&git_output(
        main,
        &["diff", "--name-only", "--diff-filter=U"],
        "diff",
    )?);
    git_output(main, &["merge", "--abort"], "merge --abort")?;
    Ok(format!(
        "merging {branch} into {target} conflicts in: {}. Nothing was changed. Merge {target} into the worktree and resolve it there, then merge again.",
        names.lines().collect::<Vec<_>>().join(", ")
    ))
}

fn merge_in(base: &Path, path: &Path) -> Result<Info, String> {
    let current = info_in(base, path)?.ok_or("this tab is not on a worktree task")?;
    if current.dirty > 0 {
        return Err(format!(
            "{} files in the worktree are not committed: commit them first",
            current.dirty
        ));
    }
    if current.ahead == 0 {
        return Err(format!(
            "{} has nothing that {} lacks",
            current.branch, current.target
        ));
    }
    let main = PathBuf::from(&current.main);
    let merge = git_output(
        &main,
        &["merge", "--no-edit", "--no-gpg-sign", &current.branch],
        "merge",
    );
    if let Err(error) = merge {
        let unmerged = git_output(&main, &["diff", "--name-only", "--diff-filter=U"], "diff")?;
        if unmerged.is_empty() {
            return Err(error);
        }
        return Err(abandon_merge(&main, &current.branch, &current.target)?);
    }
    info_in(base, path)?.ok_or_else(|| "the worktree is gone".to_string())
}

pub fn merge(path: &Path) -> Result<Info, String> {
    merge_in(&base_dir()?, path)
}

fn remove_in(base: &Path, path: &Path, force: bool) -> Result<(), String> {
    let current = info_in(base, path)?.ok_or("this tab is not on a worktree task")?;
    if !force && (current.dirty > 0 || current.ahead > 0) {
        return Err(format!(
            "{} has {} uncommitted files and {} unmerged commits",
            current.branch, current.dirty, current.ahead
        ));
    }
    let main = PathBuf::from(&current.main);
    let target = path_text(path)?;
    git_output(
        &main,
        &["worktree", "remove", "--force", &target],
        "worktree remove",
    )?;
    git_output(&main, &["branch", "-D", &current.branch], "branch -D")?;
    if let Some(parent) = path.parent() {
        let _ = std::fs::remove_dir(parent); // only succeeds when no other task is left in it
    }
    Ok(())
}

pub fn remove(path: &Path, force: bool) -> Result<(), String> {
    remove_in(&base_dir()?, path, force)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;
    use crate::workspace;

    fn repo(dir: &TempDir) -> (PathBuf, PathBuf) {
        let root = dir.path().canonicalize().unwrap();
        let project = root.join("proj");
        let base = root.join("wt");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::create_dir_all(&base).unwrap();
        workspace::repo_init(&project).unwrap();
        std::fs::write(project.join("a.txt"), "one\n").unwrap();
        workspace::commit_all(&project, "first").unwrap();
        (project, base)
    }

    #[test]
    fn a_task_gets_its_own_checkout_and_branch() {
        let dir = TempDir::new("worktree", "create");
        let (project, base) = repo(&dir);
        std::fs::write(project.join("a.txt"), "uncommitted\n").unwrap();
        let task = create_in(&base, &project).unwrap();
        assert!(task.branch.starts_with("npi/task-"));
        assert_eq!(
            std::fs::read_to_string(Path::new(&task.path).join("a.txt")).unwrap(),
            "one\n"
        );
        let info = info_in(&base, Path::new(&task.path)).unwrap().unwrap();
        assert_eq!(
            (
                info.main.as_str(),
                info.branch.as_str(),
                info.target.as_str(),
                info.ahead,
                info.behind,
                info.dirty
            ),
            (
                project.to_str().unwrap(),
                task.branch.as_str(),
                "main",
                0,
                0,
                0
            )
        );
        assert_eq!(info_in(&base, &project).unwrap(), None);
        let second = create_in(&base, &project).unwrap();
        assert_ne!(second.path, task.path);
    }

    #[test]
    fn a_repository_with_no_commits_is_told_so() {
        let dir = TempDir::new("worktree", "unborn");
        let project = dir.path().join("empty");
        std::fs::create_dir_all(&project).unwrap();
        workspace::repo_init(&project).unwrap();
        assert!(create_in(dir.path(), &project)
            .unwrap_err()
            .contains("no commits"));
    }

    #[test]
    fn committed_work_merges_back_and_dirty_work_does_not() {
        let dir = TempDir::new("worktree", "merge");
        let (project, base) = repo(&dir);
        let task = create_in(&base, &project).unwrap();
        let path = Path::new(&task.path);
        assert!(merge_in(&base, path).unwrap_err().contains("nothing"));
        std::fs::write(path.join("b.txt"), "new\n").unwrap();
        assert!(merge_in(&base, path).unwrap_err().contains("not committed"));
        workspace::commit_all(path, "b").unwrap();
        assert_eq!(info_in(&base, path).unwrap().unwrap().ahead, 1);
        assert_eq!(merge_in(&base, path).unwrap().ahead, 0);
        assert_eq!(
            std::fs::read_to_string(project.join("b.txt")).unwrap(),
            "new\n"
        );
    }

    #[test]
    fn a_conflict_leaves_the_main_checkout_as_it_was() {
        let dir = TempDir::new("worktree", "conflict");
        let (project, base) = repo(&dir);
        let task = create_in(&base, &project).unwrap();
        let path = Path::new(&task.path);
        std::fs::write(path.join("a.txt"), "theirs\n").unwrap();
        workspace::commit_all(path, "theirs").unwrap();
        std::fs::write(project.join("a.txt"), "ours\n").unwrap();
        workspace::commit_all(&project, "ours").unwrap();
        let error = merge_in(&base, path).unwrap_err();
        assert!(error.contains("a.txt"), "{error}");
        assert_eq!(
            std::fs::read_to_string(project.join("a.txt")).unwrap(),
            "ours\n"
        );
        assert!(git_output(&project, &["status", "--porcelain"], "status")
            .unwrap()
            .is_empty());
    }

    #[test]
    fn removal_refuses_unfinished_work_unless_forced() {
        let dir = TempDir::new("worktree", "remove");
        let (project, base) = repo(&dir);
        let task = create_in(&base, &project).unwrap();
        let path = Path::new(&task.path);
        std::fs::write(path.join("b.txt"), "new\n").unwrap();
        assert!(remove_in(&base, path, false)
            .unwrap_err()
            .contains("uncommitted"));
        remove_in(&base, path, true).unwrap();
        assert!(!path.exists());
        let branches = git_output(&project, &["branch", "--list", "npi/*"], "branch").unwrap();
        assert!(branches.is_empty());
    }
}
