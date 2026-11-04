//! The `write_*` commands. Each names an Academic project; the webview names no path but a plain file of
//! `markdown/` or `output/`. A Compile keeps running with its window closed; the window asks how it stands.

use super::compile::{compile_one, pending, references, sweep, write_bib};
use super::link::{self, MAIN};
use super::{scaffold, sections, Pipeline, Section, FOLDER, MARKDOWN, OUTPUT};
use crate::academic_space::AcademicSpace;
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use tauri::State;

type Space<'a> = State<'a, Arc<AcademicSpace>>;

/// How the Compile of a project stands.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    pub done: usize,
    pub total: usize,
    pub current: String,
    #[serde(skip)]
    stop: Arc<AtomicBool>,
}

static RUNS: LazyLock<Mutex<HashMap<PathBuf, Run>>> = LazyLock::new(Mutex::default);

fn run_of(project: &Path) -> Option<Run> {
    RUNS.lock().ok()?.get(project).cloned()
}

fn update_run(project: &Path, change: impl FnOnce(&mut Run)) {
    if let Some(run) = RUNS.lock().ok().as_mut().and_then(|r| r.get_mut(project)) {
        change(run);
    }
}

/// Ends the record of a run when the run ends, however it ends.
struct RunGuard(PathBuf);

impl Drop for RunGuard {
    fn drop(&mut self) {
        if let Ok(mut runs) = RUNS.lock() {
            runs.remove(&self.0);
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoState {
    pub branch: Option<String>,
    pub head: Option<String>,
    pub changed: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteStatus {
    /// `write/` exists and is a Git repository; nothing else is filled in until it is.
    pub ready: bool,
    pub repo: Option<RepoState>,
    pub sections: Vec<Section>,
    /// Cited keys the library does not hold.
    pub missing: Vec<String>,
    /// `none`, `stale` or `fresh`.
    pub link: &'static str,
    pub linked_at: Option<u64>,
    pub main: Option<String>,
    pub run: Option<Run>,
}

fn project_of(space: &AcademicSpace, name: &str) -> Result<PathBuf, String> {
    space.project_named(name).ok_or_else(|| format!("no Academic project called \"{name}\""))
}

fn is_repo(dir: &Path) -> bool {
    dir.join(".git").is_dir()
}

fn status_of(project: &Path) -> Result<WriteStatus, String> {
    let dir = project.join(FOLDER);
    let run = run_of(project);
    if !is_repo(&dir) {
        return Ok(WriteStatus { ready: false, repo: None, sections: Vec::new(), missing: Vec::new(), link: "none", linked_at: None, main: None, run });
    }
    let pipeline = Pipeline::load(&dir);
    let view = crate::git_browser::status::status(&dir)?;
    let repo = RepoState { branch: view.repo.branch, head: view.repo.head.map(|h| h.chars().take(7).collect()), changed: view.changed };
    let (_, missing) = references(project, &dir)?;
    let main = dir.join(OUTPUT).join(MAIN);
    Ok(WriteStatus {
        ready: true,
        repo: Some(repo),
        sections: sections(&dir, &pipeline)?,
        missing,
        link: link::state(&dir, &pipeline),
        linked_at: pipeline.linked_at,
        main: main.is_file().then(|| format!("{FOLDER}/{OUTPUT}/{MAIN}")),
        run,
    })
}

/// What the Write window shows.
#[tauri::command]
pub async fn write_status(space: Space<'_>, project: String) -> Result<WriteStatus, String> {
    let dir = project_of(&space, &project)?;
    crate::off_ipc(move || status_of(&dir)).await?
}

/// Makes `write/`, with what is missing of its files, a Git repository with a first commit.
#[tauri::command]
pub async fn write_init(space: Space<'_>, project: String) -> Result<WriteStatus, String> {
    let root = project_of(&space, &project)?;
    crate::off_ipc(move || {
        let dir = scaffold(&root)?;
        if !is_repo(&dir) {
            crate::workspace::repo_init(&dir)?;
            crate::workspace::commit_all(&dir, "Start the paper")?;
        }
        status_of(&root)
    })
    .await?
}

/// The outcome of a Compile.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompileReport {
    pub compiled: usize,
    pub stopped: bool,
    pub missing: Vec<String>,
}

fn compile(project: &Path, everything: bool, profile: Option<&str>) -> Result<CompileReport, String> {
    let dir = project.join(FOLDER);
    if !is_repo(&dir) {
        return Err("write/ is not a Git repository yet".into());
    }
    let stop = Arc::new(AtomicBool::new(false));
    {
        let mut runs = RUNS.lock().map_err(|e| e.to_string())?;
        if runs.contains_key(project) {
            return Err("a Compile of this project is running already".into());
        }
        runs.insert(project.to_path_buf(), Run { stop: Arc::clone(&stop), ..Run::default() });
    }
    let _guard = RunGuard(project.to_path_buf());
    // What is compiled can always be compared with the draft it came from.
    crate::workspace::commit_all(&dir, "Draft")?;
    let mut pipeline = Pipeline::load(&dir);
    sweep(&dir, &mut pipeline)?;
    let names = pending(&dir, &pipeline, everything)?;
    update_run(project, |r| r.total = names.len());
    let agent = |prompt: &str, text: &str| crate::copilot_worker::convert(project, profile, prompt, text);
    let mut compiled = 0;
    let mut failure = None;
    for name in &names {
        if stop.load(Ordering::SeqCst) {
            break;
        }
        update_run(project, |r| r.current.clone_from(name));
        if let Err(e) = compile_one(&dir, name, &mut pipeline, &agent) {
            failure = Some(e);
            break;
        }
        compiled += 1;
        update_run(project, |r| r.done = compiled);
    }
    let missing = write_bib(project, &dir)?;
    // What did get done is kept, a failure or not.
    crate::workspace::commit_all(&dir, &format!("Compile {compiled} of {}", names.len()))?;
    match failure {
        Some(e) => Err(e),
        None => Ok(CompileReport { compiled, stopped: compiled < names.len(), missing }),
    }
}

/// Compiles what changed, or `everything`. Returns when it is done.
#[tauri::command]
pub async fn write_compile(space: Space<'_>, project: String, everything: Option<bool>, profile: Option<String>) -> Result<CompileReport, String> {
    let root = project_of(&space, &project)?;
    crate::off_ipc(move || compile(&root, everything.unwrap_or(false), profile.as_deref())).await?
}

/// Stops a Compile after the file it is on.
#[tauri::command]
pub async fn write_cancel(space: Space<'_>, project: String) -> Result<(), String> {
    let root = project_of(&space, &project)?;
    if let Some(run) = run_of(&root) {
        run.stop.store(true, Ordering::SeqCst);
    }
    Ok(())
}

/// Writes the complete LaTeX and commits it.
#[tauri::command]
pub async fn write_link(space: Space<'_>, project: String) -> Result<WriteStatus, String> {
    let root = project_of(&space, &project)?;
    crate::off_ipc(move || {
        let dir = root.join(FOLDER);
        if !is_repo(&dir) {
            return Err("write/ is not a Git repository yet".to_string());
        }
        if run_of(&root).is_some() {
            return Err("a Compile is running".to_string());
        }
        let mut pipeline = Pipeline::load(&dir);
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        link::run(&dir, &mut pipeline, now)?;
        crate::workspace::commit_all(&dir, "Link")?;
        status_of(&root)
    })
    .await?
}

/// `name` as a new Markdown file after the last one: `03-discussion.md` for "Discussion".
fn next_file(existing: &[String], name: &str) -> Result<String, String> {
    let slug: String = name.trim().chars().map(|c| if c.is_alphanumeric() { c.to_lowercase().next().unwrap_or(c) } else { '-' }).collect();
    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        return Err("give the section a name".into());
    }
    let last = existing.iter().filter_map(|n| n.split('-').next()?.parse::<u32>().ok()).max();
    Ok(format!("{:02}-{slug}.md", last.map_or(0, |n| n + 1)))
}

/// Adds a Markdown file for a new section.
#[tauri::command]
pub async fn write_add_section(space: Space<'_>, project: String, name: String) -> Result<WriteStatus, String> {
    let root = project_of(&space, &project)?;
    crate::off_ipc(move || {
        let dir = root.join(FOLDER);
        let file = next_file(&super::drafts(&dir)?, &name)?;
        std::fs::write(dir.join(MARKDOWN).join(file), format!("# {}\n\n", name.trim())).map_err(|e| e.to_string())?;
        status_of(&root)
    })
    .await?
}

/// `write/`, or the plain file `rel` of its `markdown/` or `output/` folders.
fn target_of(dir: &Path, rel: Option<&str>) -> Result<PathBuf, String> {
    let Some(rel) = rel else { return Ok(dir.to_path_buf()) };
    let path = Path::new(rel);
    let plain = path.components().count() == 2 && path.components().all(|c| matches!(c, std::path::Component::Normal(_)));
    if !plain || !(path.starts_with(MARKDOWN) || path.starts_with(OUTPUT)) {
        return Err("only a file of markdown/ or output/ opens".into());
    }
    let file = dir.join(path).canonicalize().map_err(|_| "that file is missing".to_string())?;
    let root = dir.canonicalize().map_err(|e| e.to_string())?;
    if !file.starts_with(root) || !file.is_file() {
        return Err("that file is outside write/".into());
    }
    Ok(file)
}

/// The macOS application an `app` id names; `None` for the Finder.
fn application(app: &str) -> Result<Option<&'static str>, String> {
    match app {
        "vscode" => Ok(Some("Visual Studio Code")),
        "typora" => Ok(Some("Typora")),
        "finder" => Ok(None),
        other => Err(format!("unknown application: {other}")),
    }
}

/// Opens `write/`, or one file of it, in VS Code, Typora or the Finder.
#[tauri::command]
pub async fn write_open(space: Space<'_>, project: String, app: String, file: Option<String>) -> Result<(), String> {
    let root = project_of(&space, &project)?;
    crate::off_ipc(move || {
        let target = target_of(&root.join(FOLDER), file.as_deref())?;
        let application = application(&app)?;
        let mut cmd = std::process::Command::new("/usr/bin/open");
        if let Some(name) = application {
            cmd.args(["-a", name]);
        } else if target.is_file() {
            cmd.arg("-R");
        }
        cmd.arg(&target);
        let output = crate::spawn_gate::output(&mut cmd).map_err(|e| format!("could not open: {e}"))?;
        if output.status.success() {
            Ok(())
        } else {
            Err(format!("{} could not be opened: is it installed?", application.unwrap_or("The Finder")))
        }
    })
    .await?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    #[test]
    fn a_new_section_comes_after_the_last() {
        let have = ["00-abstract.md".to_string(), "03-methods.md".to_string()];
        assert_eq!(next_file(&have, "Related Work").unwrap(), "04-related-work.md");
        assert_eq!(next_file(&[], "讨论").unwrap(), "00-讨论.md");
        assert!(next_file(&have, " /// ").is_err());
    }

    #[test]
    fn only_a_plain_file_of_the_two_folders_opens() {
        let d = TempDir::new("writing", "open");
        let dir = crate::writing::scaffold(d.path()).unwrap();
        assert_eq!(target_of(&dir, None).unwrap(), dir);
        assert!(target_of(&dir, Some("markdown/00-abstract.md")).is_ok());
        for bad in ["../x", "/etc/passwd", "template.tex", "markdown/../../x", "latex/abstract.tex", "markdown/nope.md"] {
            assert!(target_of(&dir, Some(bad)).is_err(), "{bad}");
        }
        std::fs::write(d.path().join("secret.md"), "x").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(d.path().join("secret.md"), dir.join("markdown/link.md")).unwrap();
        #[cfg(unix)]
        assert!(target_of(&dir, Some("markdown/link.md")).is_err(), "a link out of write/");
        assert!(application("evil").is_err());
    }

    #[test]
    fn a_project_without_a_repository_says_so_and_compile_refuses() {
        let d = TempDir::new("writing", "no-repo");
        assert!(!status_of(d.path()).unwrap().ready);
        assert!(compile(d.path(), false, None).unwrap_err().contains("Git"));
    }

    /// With a real model: `npi` next to the test binary, then `cargo test real_compile -- --ignored`.
    #[test]
    #[ignore = "needs a configured model"]
    fn real_compile_and_link() {
        let d = TempDir::new("writing", "real");
        let project = d.path();
        let dir = scaffold(project).unwrap();
        crate::workspace::repo_init(&dir).unwrap();
        crate::workspace::commit_all(&dir, "Start").unwrap();
        std::fs::write(dir.join("markdown/00-abstract.md"), "# Abstract\n\nWe measure 50% of graphs_1.\n").unwrap();
        std::fs::write(dir.join("markdown/01-introduction.md"), "# Introduction\n\nGraphs matter [@nobody2020x].\n\n$$a^2+b^2=c^2$$\n\n## Scope\n\n- one\n- two\n").unwrap();
        let report = compile(project, false, None).unwrap();
        println!("{report:?}");
        assert_eq!(report.compiled, 2);
        assert_eq!(report.missing, ["nobody2020x"]);
        let mut pipeline = Pipeline::load(&dir);
        link::run(&dir, &mut pipeline, 1).unwrap();
        crate::workspace::commit_all(&dir, "Link").unwrap();
        println!("{}", std::fs::read_to_string(dir.join("output/main.tex")).unwrap());
        println!("{:?}", pipeline.warnings);
        println!("{}", String::from_utf8_lossy(&std::process::Command::new("git").args(["log", "--oneline"]).current_dir(&dir).output().unwrap().stdout));
    }

    #[test]
    fn the_pipeline_commits_each_step_in_its_own_repository() {
        if !crate::git_browser::fixture::git_available() {
            return;
        }
        let d = TempDir::new("writing", "repo");
        let project = d.path();
        let dir = scaffold(project).unwrap();
        crate::workspace::repo_init(&dir).unwrap();
        assert!(crate::workspace::commit_all(&dir, "Start").unwrap());
        assert!(!crate::workspace::commit_all(&dir, "Nothing").unwrap(), "no empty commits");
        let s = status_of(project).unwrap();
        assert!(s.ready);
        assert_eq!(s.repo.unwrap().changed, 0);
        std::fs::write(dir.join("markdown/01-introduction.md"), "changed").unwrap();
        assert_eq!(status_of(project).unwrap().repo.unwrap().changed, 1);
    }
}
