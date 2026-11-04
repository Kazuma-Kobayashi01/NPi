//! The Academic space: `~/Documents/NPi/Academic`, one folder per research
//! project.
//!
//! A project is an ordinary omp session whose cwd is its folder, so the
//! sidebar tells Academic tabs from Code tabs by that path. Picking a folder
//! directly under the space scaffolds it (`prepare`): the standard folders,
//! and the project's rules, context and skills under `.omp/`, which omp
//! discovers natively. Scaffolding only adds what is missing and never
//! overwrites a file the user may have edited.

use std::path::{Path, PathBuf};

/// Created inside every project.
const DIRS: &[&str] = &[
    "papers",
    "notes",
    "digests",
    "artifacts",
    "data/raw",
    "data/processed",
    "drafts",
    ".npi-academic",
];

/// `(path inside the project, content)`: written only when absent.
const FILES: &[(&str, &str)] = &[
    (".npi-academic/project.json", "{\n  \"schema\": 1\n}\n"),
    (
        ".omp/AGENTS.md",
        include_str!("../templates/academic/AGENTS.md"),
    ),
    (
        ".omp/RULES.md",
        include_str!("../templates/academic/RULES.md"),
    ),
    (
        ".omp/skills/literature-intake/SKILL.md",
        include_str!("../templates/academic/skills/literature-intake/SKILL.md"),
    ),
    (
        ".omp/skills/paper-digest/SKILL.md",
        include_str!("../templates/academic/skills/paper-digest/SKILL.md"),
    ),
    (
        ".omp/skills/data-processing/SKILL.md",
        include_str!("../templates/academic/skills/data-processing/SKILL.md"),
    ),
    (
        ".omp/skills/librarian/SKILL.md",
        include_str!("../templates/academic/skills/librarian/SKILL.md"),
    ),
];

const DEFAULT_RULES: &str = include_str!("../templates/academic/RULES.md");

pub struct AcademicSpace {
    root: PathBuf,
}

impl AcademicSpace {
    pub const fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// Creates the space folder if needed.
    pub fn ensure_root(&self) -> Result<&Path, String> {
        std::fs::create_dir_all(&self.root).map_err(|e| format!("academic folder: {e}"))?;
        Ok(&self.root)
    }

    /// Adds the standard folders and files of a project that are missing.
    fn scaffold(project: &Path) -> Result<(), String> {
        for dir in DIRS {
            std::fs::create_dir_all(project.join(dir)).map_err(|e| format!("{dir}: {e}"))?;
        }
        crate::writing::scaffold(project)?;
        for (rel, content) in FILES {
            let path = project.join(rel);
            if path.exists() {
                continue;
            }
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| format!("{rel}: {e}"))?;
            }
            std::fs::write(&path, content).map_err(|e| format!("{rel}: {e}"))?;
        }
        Ok(())
    }

    /// The canonical folder of `path` when it is a project (a folder directly
    /// under the space), else `None`.
    pub fn project(&self, path: &Path) -> Option<PathBuf> {
        let root = self.root.canonicalize().ok()?;
        let path = path.canonicalize().ok()?;
        (path.parent() == Some(root.as_path()) && path.is_dir()).then_some(path)
    }

    /// The names of the projects (folders directly under the space, hidden ones left out).
    pub fn project_names(&self) -> Vec<String> {
        let Ok(entries) = std::fs::read_dir(&self.root) else {
            return Vec::new();
        };
        let mut names: Vec<String> = entries
            .flatten()
            .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|n| !n.starts_with('.'))
            .collect();
        names.sort();
        names
    }

    /// The project called `name`: a plain folder name, never a path.
    pub fn project_named(&self, name: &str) -> Option<PathBuf> {
        let plain = !name.is_empty() && !name.starts_with('.') && !name.contains(['/', '\\']);
        if !plain {
            return None;
        }
        self.project(&self.root.join(name))
    }

    /// The project's `.omp/RULES.md` (cut to a prompt-sized length), or the
    /// default rules when it has none.
    pub fn rules(project: &Path) -> String {
        let text = std::fs::read_to_string(project.join(".omp/RULES.md"))
            .unwrap_or_else(|_| DEFAULT_RULES.to_string());
        crate::text::prefix_within(&text, 16 * 1024).to_string()
    }

    /// Scaffolds `path` when it is a folder directly under the space (a
    /// project); anything else (the space itself, a deeper folder, a folder
    /// elsewhere) is left alone. Resolves to whether it is a project.
    pub fn prepare(&self, path: &Path) -> Result<bool, String> {
        self.ensure_root()?;
        let Some(path) = self.project(path) else {
            return Ok(false);
        };
        Self::scaffold(&path)?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    #[test]
    fn a_folder_under_the_space_gets_the_standard_layout_and_rules() {
        let d = TempDir::new("academic", "layout");
        let space = AcademicSpace::new(d.path().join("Academic"));
        let project = d.path().join("Academic").join("Thesis");
        std::fs::create_dir_all(&project).unwrap();
        assert!(space.prepare(&project).unwrap());
        for dir in DIRS {
            assert!(project.join(dir).is_dir(), "{dir}");
        }
        for (rel, _) in FILES {
            assert!(project.join(rel).is_file(), "{rel}");
        }
        assert!(std::fs::read_to_string(project.join(".omp/RULES.md"))
            .unwrap()
            .contains("papers/"));
    }

    #[test]
    fn scaffolding_keeps_what_the_user_edited() {
        let d = TempDir::new("academic", "keeps-edits");
        let space = AcademicSpace::new(d.path().join("Academic"));
        let project = d.path().join("Academic").join("P");
        std::fs::create_dir_all(project.join(".omp")).unwrap();
        std::fs::write(project.join(".omp/RULES.md"), "mine").unwrap();
        assert!(space.prepare(&project).unwrap());
        assert_eq!(
            std::fs::read_to_string(project.join(".omp/RULES.md")).unwrap(),
            "mine"
        );
    }

    #[test]
    fn only_a_direct_child_of_the_space_is_a_project() {
        let d = TempDir::new("academic", "direct-child");
        let space = AcademicSpace::new(d.path().join("Academic"));
        let root = d.path().join("Academic");
        let deeper = root.join("P").join("notes");
        let elsewhere = d.path().join("Other");
        std::fs::create_dir_all(&deeper).unwrap();
        std::fs::create_dir_all(&elsewhere).unwrap();
        assert!(!space.prepare(&root).unwrap());
        assert!(!space.prepare(&deeper).unwrap());
        assert!(!space.prepare(&elsewhere).unwrap());
        assert!(!space.prepare(&d.path().join("missing")).unwrap());
        assert!(!elsewhere.join(".omp").exists());
    }
}
