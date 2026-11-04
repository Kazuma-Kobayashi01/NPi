//! Compile: an agent turns each changed Markdown file into one LaTeX file. Only the text goes to the
//! model and only the answer comes back; the agent has no tools. `references.bib` is not the model's:
//! it is written from the project's library, so no reference is made up.

use super::{cited_keys, drafts, hash, is_abstract, tex_of, Pipeline, BIB, LATEX, MARKDOWN, SECTIONS};
use crate::librarian::{export, Format, Library};
use std::path::Path;

const PROMPT: &str = "\
You convert one part of a research paper from Markdown to LaTeX. Answer with the LaTeX and nothing else: no explanation, no code fence.
- This is a fragment that goes into a document: no \\documentclass, \\usepackage, \\begin{document} or \\bibliography.
- Keep every sentence as written, in its own language. Do not summarise, add, reorder, improve or translate.
- # is \\section, ## is \\subsection, ### is \\subsubsection. Give each \\label only if the Markdown names one.
- A citation [@key] is \\cite{key}; [@a; @b] is \\cite{a,b}; keep the keys exactly. [@key, p. 3] is \\cite[p.~3]{key}.
- Math stays math: $...$ inline, $$...$$ as \\begin{equation}...\\end{equation}.
- Lists, tables (booktabs), figures and footnotes become their LaTeX forms. An image path stays as written.
- Escape what LaTeX treats as special in plain text: % & _ # $ { } ~ ^ \\.
";
const ABSTRACT: &str = "- This is the abstract: only its text goes in, with no \\section and no abstract environment.\n";

/// What the model answered, without a code fence it was told not to add. `Err` when it is a whole document.
pub(super) fn clean_answer(answer: &str) -> Result<String, String> {
    let text = answer.trim();
    let inner = text.strip_prefix("```").map_or(text, |rest| {
        let body = rest.split_once('\n').map_or("", |(_, body)| body);
        body.trim_end().strip_suffix("```").unwrap_or(body)
    });
    let inner = inner.trim();
    if inner.is_empty() {
        return Err("the agent answered with nothing".into());
    }
    if inner.contains("\\documentclass") || inner.contains("\\begin{document}") {
        return Err("the agent wrote a whole document instead of a fragment".into());
    }
    Ok(format!("{inner}\n"))
}

/// What a person should look at: citations that did not come through, and text that went missing.
pub(super) fn check(markdown: &str, tex: &str) -> Vec<String> {
    let mut found = Vec::new();
    let lost: Vec<String> = cited_keys(markdown).into_iter().filter(|k| !tex.contains(k.as_str())).collect();
    if !lost.is_empty() {
        found.push(format!("citations missing from the LaTeX: {}", lost.join(", ")));
    }
    let count = |text: &str| text.chars().filter(|c| c.is_alphanumeric()).count();
    if count(tex) * 10 < count(markdown) * 7 {
        found.push("the LaTeX is much shorter than the Markdown: text may be missing".to_string());
    }
    found
}

/// The files to compile: those not compiled from their present text, or all with `everything`.
pub(super) fn pending(dir: &Path, pipeline: &Pipeline, everything: bool) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for name in drafts(dir)? {
        let text = std::fs::read_to_string(dir.join(MARKDOWN).join(&name)).map_err(|e| format!("{name}: {e}"))?;
        let fresh = dir.join(tex_of(&name)).is_file() && pipeline.compiled.get(&name) == Some(&hash(&text));
        if everything || !fresh {
            out.push(name);
        }
    }
    Ok(out)
}

/// Compiles one file with `agent` (system prompt, text → answer) and records it.
pub(super) fn compile_one(
    dir: &Path,
    name: &str,
    pipeline: &mut Pipeline,
    agent: &dyn Fn(&str, &str) -> Result<String, String>,
) -> Result<(), String> {
    let text = std::fs::read_to_string(dir.join(MARKDOWN).join(name)).map_err(|e| format!("{name}: {e}"))?;
    let prompt = if is_abstract(name) { format!("{PROMPT}{ABSTRACT}") } else { PROMPT.to_string() };
    let tex = clean_answer(&agent(&prompt, &text).map_err(|e| format!("{name}: {e}"))?).map_err(|e| format!("{name}: {e}"))?;
    let target = dir.join(tex_of(name));
    std::fs::write(&target, &tex).map_err(|e| format!("{name}: {e}"))?;
    let warnings = check(&text, &tex);
    if warnings.is_empty() {
        pipeline.warnings.remove(name);
    } else {
        pipeline.warnings.insert(name.to_string(), warnings);
    }
    pipeline.compiled.insert(name.to_string(), hash(&text));
    pipeline.save(dir)
}

/// Takes out the LaTeX of Markdown files that are gone.
pub(super) fn sweep(dir: &Path, pipeline: &mut Pipeline) -> Result<(), String> {
    let names = drafts(dir)?;
    let wanted: Vec<String> = names.iter().map(|n| tex_of(n)).collect();
    if let Ok(list) = std::fs::read_dir(dir.join(SECTIONS)) {
        for entry in list.flatten() {
            let rel = format!("{SECTIONS}/{}", entry.file_name().to_string_lossy());
            if !wanted.contains(&rel) {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
    pipeline.compiled.retain(|n, _| names.contains(n));
    pipeline.warnings.retain(|n, _| names.contains(n));
    pipeline.save(dir)
}

/// Writes `latex/references.bib` with what the drafts cite and returns the keys the library lacks.
pub(super) fn write_bib(project: &Path, dir: &Path) -> Result<Vec<String>, String> {
    let (bib, missing) = references(project, dir)?;
    std::fs::write(dir.join(LATEX).join(BIB), bib).map_err(|e| format!("{BIB}: {e}"))?;
    Ok(missing)
}

/// The BibTeX of what the drafts cite, and the cited keys the library does not hold.
pub(super) fn references(project: &Path, dir: &Path) -> Result<(String, Vec<String>), String> {
    let mut keys: Vec<String> = Vec::new();
    for name in drafts(dir)? {
        let text = std::fs::read_to_string(dir.join(MARKDOWN).join(&name)).map_err(|e| format!("{name}: {e}"))?;
        for key in cited_keys(&text) {
            if !keys.contains(&key) {
                keys.push(key);
            }
        }
    }
    let entries = Library::open(project)?.list()?;
    let held: Vec<_> = entries.into_iter().filter(|e| keys.contains(&e.citekey)).collect();
    let missing = keys.into_iter().filter(|k| !held.iter().any(|e| e.citekey == *k)).collect();
    Ok((export(Format::Bibtex, &held), missing))
}

#[cfg(test)]
mod tests {
    use super::super::scaffold;
    use super::*;
    use crate::test_support::TempDir;
    use serde_json::json;

    #[test]
    fn a_fence_goes_and_a_whole_document_or_nothing_is_refused() {
        assert_eq!(clean_answer("```latex\n\\section{A}\n```").unwrap(), "\\section{A}\n");
        assert_eq!(clean_answer("\\section{A}").unwrap(), "\\section{A}\n");
        assert!(clean_answer("\\documentclass{article}").is_err());
        assert!(clean_answer("  ").is_err());
    }

    #[test]
    fn the_check_names_lost_citations_and_lost_text() {
        assert!(check("Good [@a].", "Good \\cite{a}.").is_empty());
        assert!(check("Good [@a] and [@b].", "Good \\cite{a}.")[0].contains('b'));
        assert!(check(&"word ".repeat(100), "word").iter().any(|w| w.contains("shorter")));
    }

    #[test]
    fn only_what_changed_is_compiled_and_what_was_done_stays_done() {
        let d = TempDir::new("writing", "compile");
        let dir = scaffold(d.path()).unwrap();
        std::fs::write(dir.join("markdown/02-end.md"), "# End\n\nIt ends [@a].").unwrap();
        let mut p = Pipeline::default();
        let names = pending(&dir, &p, false).unwrap();
        assert_eq!(names.len(), 3);
        let agent = |prompt: &str, text: &str| -> Result<String, String> {
            assert!(prompt.contains("fragment"));
            Ok(format!("\\section{{x}}\n{}", text.replace("[@a]", "\\cite{a}")))
        };
        for n in &names {
            compile_one(&dir, n, &mut p, &agent).unwrap();
        }
        assert!(dir.join("latex/abstract.tex").is_file() && dir.join("latex/sections/02-end.tex").is_file());
        assert!(pending(&dir, &p, false).unwrap().is_empty());
        assert_eq!(pending(&dir, &p, true).unwrap().len(), 3);
        std::fs::write(dir.join("markdown/02-end.md"), "# End\n\nChanged.").unwrap();
        assert_eq!(pending(&dir, &p, false).unwrap(), ["02-end.md"]);
        let failing = |_: &str, _: &str| -> Result<String, String> { Err("no model".into()) };
        assert!(compile_one(&dir, "02-end.md", &mut p, &failing).unwrap_err().contains("02-end.md"));
        assert_eq!(pending(&dir, &p, false).unwrap(), ["02-end.md"], "a failure records nothing");
        std::fs::remove_file(dir.join("markdown/02-end.md")).unwrap();
        sweep(&dir, &mut p).unwrap();
        assert!(!dir.join("latex/sections/02-end.tex").exists());
        assert!(!p.compiled.contains_key("02-end.md"));
    }

    #[test]
    fn the_bibliography_comes_from_the_library_and_says_what_it_lacks() {
        let d = TempDir::new("writing", "bib");
        let project = d.path();
        let key = Library::open(project).unwrap().add(&json!({ "type": "book", "title": "Graphs", "author": [{ "family": "Smith", "given": "A" }], "issued": { "date-parts": [[2020]] } }), "manual").unwrap().citekey;
        let dir = scaffold(project).unwrap();
        std::fs::write(dir.join("markdown/02-x.md"), format!("See [@{key}] and [@nobody2000x].")).unwrap();
        let missing = write_bib(project, &dir).unwrap();
        assert_eq!(missing, ["nobody2000x"]);
        let bib = std::fs::read_to_string(dir.join("latex/references.bib")).unwrap();
        assert!(bib.contains(&key) && bib.contains("Graphs") && !bib.contains("nobody"));
    }
}
