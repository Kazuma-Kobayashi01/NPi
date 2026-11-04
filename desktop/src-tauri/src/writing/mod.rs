//! The paper of an Academic project, in its `write/` folder, a Git repository of its own:
//! Markdown drafts, then Compile (an agent turns each into LaTeX; `compile.rs`), then Link (the
//! template filled into one complete `.tex`; `link.rs`). The Write window only manages this: it
//! starts the steps and shows where they stand, and the writing happens in an editor.

mod compile;
pub mod commands;
mod link;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const FOLDER: &str = "write";
const MARKDOWN: &str = "markdown";
const LATEX: &str = "latex";
const SECTIONS: &str = "latex/sections";
const OUTPUT: &str = "output";
const TEMPLATE: &str = "template.tex";
const BIB: &str = "references.bib";
const STATE: &str = ".pipeline.json";

/// The files of a new `write/` folder: (path inside it, content). Existing ones are left alone.
pub const FILES: &[(&str, &str)] = &[
    ("README.md", include_str!("../../templates/academic/write/README.md")),
    ("template.tex", include_str!("../../templates/academic/write/template.tex")),
    (".gitignore", include_str!("../../templates/academic/write/gitignore")),
    ("markdown/00-abstract.md", include_str!("../../templates/academic/write/00-abstract.md")),
    ("markdown/01-introduction.md", include_str!("../../templates/academic/write/01-introduction.md")),
];

/// Creates what is missing of `<project>/write/`, without Git.
pub fn scaffold(project: &Path) -> Result<PathBuf, String> {
    let dir = project.join(FOLDER);
    for sub in [MARKDOWN, SECTIONS, OUTPUT] {
        std::fs::create_dir_all(dir.join(sub)).map_err(|e| format!("{sub}: {e}"))?;
    }
    for (rel, content) in FILES {
        let path = dir.join(rel);
        if !path.exists() {
            std::fs::write(&path, content).map_err(|e| format!("{rel}: {e}"))?;
        }
    }
    Ok(dir)
}

/// What the last Compile and Link left, kept with the files in `.pipeline.json`.
#[derive(Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", default)]
struct Pipeline {
    /// Markdown file name → the hash of the text its LaTeX was made from.
    compiled: BTreeMap<String, String>,
    /// Markdown file name → what the check after compiling it found.
    warnings: BTreeMap<String, Vec<String>>,
    /// The hash of everything Link last read.
    linked: Option<String>,
    linked_at: Option<u64>,
}

impl Pipeline {
    fn load(dir: &Path) -> Self {
        std::fs::read_to_string(dir.join(STATE)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    }

    fn save(&self, dir: &Path) -> Result<(), String> {
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(dir.join(STATE), format!("{text}\n")).map_err(|e| format!("{STATE}: {e}"))
    }
}

fn hash(text: &str) -> String {
    Sha256::digest(text.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

/// The Markdown files of the paper, in name order.
fn drafts(dir: &Path) -> Result<Vec<String>, String> {
    let list = std::fs::read_dir(dir.join(MARKDOWN)).map_err(|e| format!("{MARKDOWN}: {e}"))?;
    let mut names: Vec<String> = list
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.ends_with(".md") && !n.starts_with('.'))
        .collect();
    names.sort();
    Ok(names)
}

/// `00-abstract.md` is the abstract; so is `abstract.md` or `摘要.md`.
fn is_abstract(name: &str) -> bool {
    let stem = name.trim_end_matches(".md").trim_start_matches(|c: char| c.is_ascii_digit() || c == '-' || c == '_');
    stem.eq_ignore_ascii_case("abstract") || stem == "摘要"
}

/// Where the LaTeX of the Markdown file `name` goes, inside `write/`.
fn tex_of(name: &str) -> String {
    if is_abstract(name) {
        format!("{LATEX}/abstract.tex")
    } else {
        format!("{SECTIONS}/{}.tex", name.trim_end_matches(".md"))
    }
}

/// The citekeys of `[@a; @b]` markers in `text`, in order, each once.
fn cited_keys(text: &str) -> Vec<String> {
    let mut keys: Vec<String> = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find("[@") {
        let after = &rest[at + 1..];
        let Some(end) = after.find(']') else { break };
        for key in after[..end].split(';').filter_map(|k| k.trim().strip_prefix('@')) {
            let key = key.split(|c: char| c.is_whitespace() || c == ',').next().unwrap_or("");
            if !key.is_empty() && !keys.iter().any(|k| k == key) {
                keys.push(key.to_string());
            }
        }
        rest = &after[end..];
    }
    keys
}

/// One Markdown file as the window shows it.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Section {
    pub file: String,
    pub title: String,
    pub words: usize,
    /// `new` (never compiled), `stale` (changed since) or `fresh`.
    pub state: &'static str,
    pub warnings: Vec<String>,
}

/// Words, the way a writer counts them: a Han character is one, a run of anything else is one.
fn word_count(text: &str) -> usize {
    let han = text.chars().filter(|c| ('\u{4e00}'..='\u{9fff}').contains(c)).count();
    let latin = text.split(|c: char| c.is_whitespace() || ('\u{4e00}'..='\u{9fff}').contains(&c)).filter(|w| w.chars().any(char::is_alphanumeric)).count();
    han + latin
}

fn title_of(text: &str, name: &str) -> String {
    text.lines()
        .find_map(|l| l.strip_prefix('#').map(|t| t.trim_start_matches('#').trim().to_string()))
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| name.trim_end_matches(".md").to_string())
}

fn sections(dir: &Path, pipeline: &Pipeline) -> Result<Vec<Section>, String> {
    drafts(dir)?
        .into_iter()
        .map(|file| {
            let text = std::fs::read_to_string(dir.join(MARKDOWN).join(&file)).map_err(|e| format!("{file}: {e}"))?;
            let state = match pipeline.compiled.get(&file) {
                None => "new",
                Some(done) if dir.join(tex_of(&file)).is_file() && *done == hash(&text) => "fresh",
                Some(_) => "stale",
            };
            let warnings = pipeline.warnings.get(&file).cloned().unwrap_or_default();
            Ok(Section { title: title_of(&text, &file), words: word_count(&text), state, warnings, file })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    #[test]
    fn a_new_folder_has_its_files_and_keeps_what_the_user_wrote() {
        let d = TempDir::new("writing", "scaffold");
        let dir = scaffold(d.path()).unwrap();
        assert_eq!(drafts(&dir).unwrap(), ["00-abstract.md", "01-introduction.md"]);
        std::fs::write(dir.join("template.tex"), "mine").unwrap();
        scaffold(d.path()).unwrap();
        assert_eq!(std::fs::read_to_string(dir.join("template.tex")).unwrap(), "mine");
    }

    #[test]
    fn the_abstract_and_the_sections_have_their_places() {
        assert_eq!(tex_of("00-abstract.md"), "latex/abstract.tex");
        assert_eq!(tex_of("摘要.md"), "latex/abstract.tex");
        assert_eq!(tex_of("02-methods.md"), "latex/sections/02-methods.tex");
    }

    #[test]
    fn citation_markers_give_their_keys_once() {
        assert_eq!(cited_keys("a [@x] b [@y; @z] c [@x, p. 3] [see] [1]"), ["x", "y", "z"]);
        assert!(cited_keys("no cites, [@ unterminated").is_empty());
    }

    #[test]
    fn counting_and_titles() {
        assert_eq!(word_count("deep learning 方法 is good"), 6);
        assert_eq!(title_of("\n## Methods\n\ntext", "02-x.md"), "Methods");
        assert_eq!(title_of("no heading", "02-x.md"), "02-x");
    }

    #[test]
    fn a_section_is_new_then_fresh_then_stale() {
        let d = TempDir::new("writing", "states");
        let dir = scaffold(d.path()).unwrap();
        let mut p = Pipeline::default();
        assert_eq!(sections(&dir, &p).unwrap()[1].state, "new");
        let text = std::fs::read_to_string(dir.join("markdown/01-introduction.md")).unwrap();
        p.compiled.insert("01-introduction.md".into(), hash(&text));
        assert_eq!(sections(&dir, &p).unwrap()[1].state, "stale", "its LaTeX is not there");
        std::fs::write(dir.join("latex/sections/01-introduction.tex"), "x").unwrap();
        assert_eq!(sections(&dir, &p).unwrap()[1].state, "fresh");
        std::fs::write(dir.join("markdown/01-introduction.md"), "changed").unwrap();
        assert_eq!(sections(&dir, &p).unwrap()[1].state, "stale");
    }

    #[test]
    fn the_state_survives_a_save_and_a_broken_file_is_a_fresh_start() {
        let d = TempDir::new("writing", "state");
        let mut p = Pipeline::default();
        p.linked = Some("h".into());
        p.save(d.path()).unwrap();
        assert_eq!(Pipeline::load(d.path()), p);
        std::fs::write(d.path().join(STATE), "{nope").unwrap();
        assert_eq!(Pipeline::load(d.path()), Pipeline::default());
    }
}
