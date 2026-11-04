//! Link: the template filled in with the compiled parts, into one complete `.tex`. No model takes part.

use super::compile::pending;
use super::{drafts, hash, is_abstract, tex_of, Pipeline, BIB, LATEX, OUTPUT, TEMPLATE};
use std::path::Path;

const ABSTRACT_MARK: &str = "%%NPI-ABSTRACT%%";
const SECTIONS_MARK: &str = "%%NPI-SECTIONS%%";
const BIB_MARK: &str = "%%NPI-BIBLIOGRAPHY%%";
pub(super) const MAIN: &str = "main.tex";

/// The complete text, and the hash of what it was made from.
fn assemble(dir: &Path) -> Result<(String, String), String> {
    let read = |rel: &str| std::fs::read_to_string(dir.join(rel)).map_err(|e| format!("{rel}: {e}"));
    let template = read(TEMPLATE)?;
    for mark in [ABSTRACT_MARK, SECTIONS_MARK, BIB_MARK] {
        if !template.contains(mark) {
            return Err(format!("{TEMPLATE} has lost its {mark} line"));
        }
    }
    let (mut summary, mut sections) = (String::new(), Vec::new());
    for name in drafts(dir)? {
        let tex = read(&tex_of(&name))?;
        if is_abstract(&name) {
            summary = tex.trim_end().to_string();
        } else {
            sections.push(tex.trim_end().to_string());
        }
    }
    let bib = read(&format!("{LATEX}/{BIB}"))?;
    let sections = sections.join("\n\n");
    let source = hash(&format!("{template}\0{summary}\0{sections}\0{bib}"));
    // One pass per mark, in the template only: a section that quotes a mark stays as it is.
    let main = template
        .replace(ABSTRACT_MARK, "\u{0}a")
        .replace(SECTIONS_MARK, "\u{0}s")
        .replace(BIB_MARK, "\u{0}b")
        .replace("\u{0}a", &summary)
        .replace("\u{0}s", &sections)
        .replace("\u{0}b", "\\bibliography{references}");
    Ok((main, source))
}

/// `none`, `stale` (something changed since) or `fresh`.
pub(super) fn state(dir: &Path, pipeline: &Pipeline) -> &'static str {
    match (&pipeline.linked, dir.join(OUTPUT).join(MAIN).is_file()) {
        (Some(done), true) if assemble(dir).is_ok_and(|(_, source)| source == *done) => "fresh",
        (Some(_), true) => "stale",
        _ => "none",
    }
}

/// Writes `output/main.tex` and `output/references.bib`. Refused while a draft is not compiled.
pub(super) fn run(dir: &Path, pipeline: &mut Pipeline, now: u64) -> Result<(), String> {
    let waiting = pending(dir, pipeline, false)?;
    if !waiting.is_empty() {
        return Err(format!("compile first: {}", waiting.join(", ")));
    }
    let (main, source) = assemble(dir)?;
    let output = dir.join(OUTPUT);
    std::fs::create_dir_all(&output).map_err(|e| format!("{OUTPUT}: {e}"))?;
    std::fs::write(output.join(MAIN), main).map_err(|e| format!("{MAIN}: {e}"))?;
    std::fs::copy(dir.join(LATEX).join(BIB), output.join(BIB)).map_err(|e| format!("{BIB}: {e}"))?;
    pipeline.linked = Some(source);
    pipeline.linked_at = Some(now);
    pipeline.save(dir)
}

#[cfg(test)]
mod tests {
    use super::super::compile::compile_one;
    use super::super::scaffold;
    use super::*;
    use crate::test_support::TempDir;

    fn compiled(d: &TempDir) -> (std::path::PathBuf, Pipeline) {
        let dir = scaffold(d.path()).unwrap();
        std::fs::write(dir.join("markdown/00-abstract.md"), "We study graphs.").unwrap();
        std::fs::write(dir.join("markdown/01-introduction.md"), "# Intro\n\nHello.").unwrap();
        std::fs::write(dir.join("markdown/02-end.md"), "# End\n\nBye.").unwrap();
        let mut p = Pipeline::default();
        let agent = |_: &str, text: &str| -> Result<String, String> { Ok(text.replace("# ", "\\section ")) };
        for n in drafts(&dir).unwrap() {
            compile_one(&dir, &n, &mut p, &agent).unwrap();
        }
        std::fs::write(dir.join("latex/references.bib"), "@book{a}\n").unwrap();
        (dir, p)
    }

    #[test]
    fn the_parts_land_in_the_template_in_order() {
        let d = TempDir::new("writing", "link");
        let (dir, mut p) = compiled(&d);
        assert_eq!(state(&dir, &p), "none");
        run(&dir, &mut p, 5).unwrap();
        let main = std::fs::read_to_string(dir.join("output/main.tex")).unwrap();
        assert!(main.contains("\\begin{abstract}\nWe study graphs.\n\\end{abstract}"));
        assert!(main.find("Hello").unwrap() < main.find("Bye").unwrap());
        assert!(main.contains("\\bibliography{references}") && !main.contains("NPI-"));
        assert_eq!(std::fs::read_to_string(dir.join("output/references.bib")).unwrap(), "@book{a}\n");
        assert_eq!((state(&dir, &p), p.linked_at), ("fresh", Some(5)));
        std::fs::write(dir.join("latex/sections/02-end.tex"), "edited by hand").unwrap();
        assert_eq!(state(&dir, &p), "stale");
    }

    #[test]
    fn a_draft_not_compiled_or_a_broken_template_stops_it() {
        let d = TempDir::new("writing", "link-stop");
        let (dir, mut p) = compiled(&d);
        std::fs::write(dir.join("markdown/02-end.md"), "changed").unwrap();
        assert!(run(&dir, &mut p, 1).unwrap_err().contains("02-end.md"));
        assert!(!dir.join("output/main.tex").exists());
        let d2 = TempDir::new("writing", "link-template");
        let (dir, mut p) = compiled(&d2);
        std::fs::write(dir.join("template.tex"), "no marks").unwrap();
        assert!(run(&dir, &mut p, 1).unwrap_err().contains("NPI"));
    }
}
