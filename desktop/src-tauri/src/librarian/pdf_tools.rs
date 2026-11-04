//! Binding PDFs to references from an agent: `library_attach`, and what `library_add` does by itself.
//! A file already under `papers/` is recorded where it is; one elsewhere is copied in under the same
//! name a PDF added in the Librarian window gets. The original is never touched.
//!
//! Finding a reference's PDF needs no path: a file under `papers/` that no reference holds is its PDF
//! when page 1 shows its DOI or whole title, or the file's name holds the title. Only one file may
//! qualify; two candidates bind nothing and are reported.

use super::attach::{self, PAPERS};
use super::csl::fields;
use super::tools::LibraryTools;
use super::{Entry, Library};
use crate::pdf_text::{first_page, Extracted};
use serde_json::{json, Value};
use std::path::{Component, Path, PathBuf};

/// A title shorter than this says too little to recognise a paper by.
const MIN_TITLE_CHARS: usize = 12;
const MAX_FOLDER_DEPTH: usize = 3;
/// References worked through by one `all: true` call.
const MAX_ALL: usize = 50;

pub(super) fn definition() -> Value {
    json!({
        "name": "library_attach",
        "label": "Attach PDF",
        "description": "Bind a PDF to a reference already in the library, so the Reader and the Librarian open it. `citekey` and `path`: a project file, or an absolute path to a PDF, which is copied into papers/. `citekey` alone: found under papers/ by its DOI or title. `all: true`: the same for every reference without a PDF. Example: {\"citekey\": \"smith2020graph\", \"path\": \"papers/smith.pdf\"}.",
        "parameters": {
            "type": "object",
            "properties": {
                "citekey": { "type": "string" },
                "path": { "type": "string", "description": "A .pdf file, relative to the project or absolute." },
                "all": { "type": "boolean" }
            },
            "additionalProperties": false
        },
        "loadMode": "essential"
    })
}

/// Letters and digits, lower case: what stays of a title across line breaks, hyphens and capitals.
fn squeezed(text: &str) -> String {
    text.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

/// Whether a PDF with this first page (`None` for a scan) and file stem is the work in `csl`.
fn is_paper_of(csl: &Value, page: Option<&str>, stem: &str) -> bool {
    let title = squeezed(&fields(csl).title);
    let long_enough = title.chars().count() >= MIN_TITLE_CHARS;
    let doi = csl.get("DOI").and_then(Value::as_str).map(squeezed).filter(|d| !d.is_empty());
    let on_page = page.map(squeezed).is_some_and(|text| {
        doi.as_ref().is_some_and(|d| text.contains(d.as_str())) || (long_enough && text.contains(&title))
    });
    on_page || (long_enough && squeezed(stem).contains(&title))
}

/// The PDFs under `papers/` as project-relative paths, in name order.
fn papers(root: &Path) -> Vec<String> {
    fn walk(dir: &Path, rel: &str, depth: usize, out: &mut Vec<String>) {
        let Ok(list) = std::fs::read_dir(dir) else { return };
        for entry in list.flatten() {
            let (Ok(kind), Ok(name)) = (entry.file_type(), entry.file_name().into_string()) else { continue };
            let child = format!("{rel}/{name}");
            if kind.is_dir() && depth < MAX_FOLDER_DEPTH {
                walk(&entry.path(), &child, depth + 1, out);
            } else if kind.is_file() && name.to_lowercase().ends_with(".pdf") {
                out.push(child);
            }
        }
    }
    let mut out = Vec::new();
    walk(&root.join(PAPERS), PAPERS, 0, &mut out);
    out.sort();
    out
}

/// One PDF no reference holds, read once however many references are matched against it.
struct Loose {
    rel: String,
    stem: String,
    page: Option<String>,
}

fn loose_papers(root: &Path, entries: &[Entry]) -> Vec<Loose> {
    let held = |rel: &str| entries.iter().flat_map(|e| &e.attachments).any(|a| a.path == rel);
    papers(root)
        .into_iter()
        .filter(|rel| !held(rel))
        .map(|rel| {
            let path = root.join(&rel);
            let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            let page = match first_page(&path) {
                Ok(Extracted::Text(text)) => Some(text),
                _ => None,
            };
            Loose { rel, stem, page }
        })
        .collect()
}

/// What looking for a reference's PDF found.
enum Found {
    One(String),
    None,
    Several(Vec<String>),
}

fn find_paper(csl: &Value, loose: &[Loose]) -> Found {
    let mut hits: Vec<String> = loose.iter().filter(|l| is_paper_of(csl, l.page.as_deref(), &l.stem)).map(|l| l.rel.clone()).collect();
    match hits.len() {
        0 => Found::None,
        1 => Found::One(hits.remove(0)),
        _ => Found::Several(hits),
    }
}

impl LibraryTools {
    /// Makes `path` a file under `papers/` of this project and returns its relative path and hash.
    fn filed(&self, path: &str, entry: &Entry) -> Result<(String, String), String> {
        let given = Path::new(path.trim());
        let full = if given.is_absolute() { given.to_path_buf() } else { self.root.join(given) };
        let is_pdf = full.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("pdf"));
        if !is_pdf {
            return Err(format!("{path} is not a .pdf file"));
        }
        let real: PathBuf = full.canonicalize().map_err(|_| format!("{path} does not exist. library_get shows a reference's files; papers/ holds the project's PDFs."))?;
        if !real.is_file() {
            return Err(format!("{path} is not a file"));
        }
        let root = self.root.canonicalize().map_err(|e| e.to_string())?;
        if let Ok(inside) = real.strip_prefix(root.join(PAPERS)) {
            let plain = inside.components().all(|c| matches!(c, Component::Normal(_)));
            let rel = format!("{PAPERS}/{}", inside.to_string_lossy());
            if plain {
                return Ok((rel, attach::file_hash(&real)?));
            }
        }
        let staged = attach::stage(&root, &real)?;
        let rel = attach::finalize(&root, &staged.hash, &attach::paper_name(&entry.csl, &staged.name))?;
        Ok((rel, staged.hash))
    }

    fn bind(&self, lib: &Library, entry: &Entry, path: &str) -> Result<String, String> {
        let (rel, hash) = self.filed(path, entry)?;
        lib.attach(entry.id, &rel, &hash, "pdf")?;
        Ok(rel)
    }

    /// `library_attach`.
    pub(super) fn attach_tool(&self, args: &Value) -> Result<String, String> {
        let lib = Library::open(&self.root)?;
        let entries = lib.list()?;
        let citekey = args.get("citekey").and_then(Value::as_str).map(str::trim).filter(|k| !k.is_empty());
        let path = args.get("path").and_then(Value::as_str).map(str::trim).filter(|p| !p.is_empty());
        if let Some(key) = citekey {
            let entry = Self::find(&entries, key)?;
            if let Some(path) = path {
                let rel = self.bind(&lib, entry, path)?;
                return Ok(format!("[@{}] now has {rel}.", entry.citekey));
            }
            let loose = loose_papers(&self.root, &entries);
            return match find_paper(&entry.csl, &loose) {
                Found::One(rel) => self.bind(&lib, entry, &rel).map(|rel| format!("[@{}] now has {rel}.", entry.citekey)),
                Found::None => Err(format!("no PDF under papers/ shows the DOI or title of [@{}]. Give `path`.", entry.citekey)),
                Found::Several(list) => Err(format!("several PDFs could be [@{}]: {}. Give `path`.", entry.citekey, list.join(", "))),
            };
        }
        if !args.get("all").and_then(Value::as_bool).unwrap_or(false) {
            return Err("give a `citekey` (with `path`, or to look under papers/), or `all: true`".into());
        }
        if path.is_some() {
            return Err("`path` goes with one `citekey`".into());
        }
        let loose = loose_papers(&self.root, &entries);
        let bare: Vec<&Entry> = entries.iter().filter(|e| e.attachments.is_empty()).take(MAX_ALL).collect();
        let (mut bound, mut doubtful, mut none) = (Vec::new(), Vec::new(), 0);
        for entry in bare {
            match find_paper(&entry.csl, &loose) {
                Found::One(rel) => bound.push(format!("[@{}] {}", entry.citekey, self.bind(&lib, entry, &rel)?)),
                Found::Several(list) => doubtful.push(format!("[@{}]: {}", entry.citekey, list.join(", "))),
                Found::None => none += 1,
            }
        }
        let mut report = format!("Bound {} PDFs; {none} references have none under papers/.", bound.len());
        for line in bound {
            report.push_str(&format!("\n{line}"));
        }
        for line in doubtful {
            report.push_str(&format!("\nseveral candidates, not bound: {line}"));
        }
        Ok(report)
    }

    /// What `library_add` says about the PDF of `entry`: `pdf` if given, else one found under `papers/`.
    /// A problem here is a note: the reference itself went in.
    pub(super) fn bind_on_add(&self, lib: &Library, entry: &Entry, pdf: Option<&str>) -> String {
        if !entry.attachments.is_empty() && pdf.is_none() {
            return String::new();
        }
        if let Some(path) = pdf {
            return self.bind(lib, entry, path).map_or_else(|e| format!(" The PDF was not attached: {e}"), |rel| format!(" Attached {rel}."));
        }
        let Ok(entries) = lib.list() else { return String::new() };
        match find_paper(&entry.csl, &loose_papers(&self.root, &entries)) {
            Found::One(rel) => self.bind(lib, entry, &rel).map_or_else(|e| format!(" The PDF was not attached: {e}"), |rel| format!(" Attached {rel}, found under papers/.")),
            Found::None => String::new(),
            Found::Several(list) => format!(" Several PDFs could be this one, none attached: {}. Use library_attach with `path`.", list.join(", ")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdf_text::tiny_pdf;
    use crate::test_support::TempDir;

    const TITLE: &str = "Graph attention for tomography";

    fn setup(d: &TempDir) -> (PathBuf, LibraryTools, Library) {
        let root = d.path().join("p");
        std::fs::create_dir_all(root.join(PAPERS)).unwrap();
        let lib = Library::open(&root).unwrap();
        (root.clone(), LibraryTools::new(root), lib)
    }

    fn entry(lib: &Library, title: &str) -> Entry {
        lib.add(&json!({ "type": "article-journal", "title": title, "DOI": "10.1234/abc.5" }), "manual").unwrap()
    }

    fn files(lib: &Library, id: i64) -> Vec<String> {
        lib.get(id).unwrap().unwrap().attachments.into_iter().map(|a| a.path).collect()
    }

    #[test]
    fn a_paper_is_known_by_its_doi_its_title_or_its_file_name() {
        let csl = json!({ "title": "Graph attention, for tomography!", "DOI": "10.1234/abc.5" });
        assert!(is_paper_of(&csl, Some("Journal\ndoi: 10.1234/ABC.5\nwords"), "x"));
        assert!(is_paper_of(&csl, Some("GRAPH ATTENTION\nFOR TOMOGRAPHY\nA. Author"), "x"));
        assert!(is_paper_of(&csl, None, "2020-Smith-Graph-attention-for-tomography"));
        assert!(!is_paper_of(&csl, Some("Another paper entirely"), "other"));
        assert!(!is_paper_of(&json!({ "title": "Graphs" }), Some("Graphs are everywhere"), "Graphs"), "too short to tell");
    }

    #[test]
    fn a_file_of_papers_is_recorded_in_place_and_one_elsewhere_is_copied_in() {
        let d = TempDir::new("pdf-tools", "path");
        let (root, tools, lib) = setup(&d);
        let e = entry(&lib, TITLE);
        std::fs::write(root.join("papers/mine.pdf"), tiny_pdf("words")).unwrap();
        let said = tools.attach_tool(&json!({ "citekey": e.citekey, "path": "papers/mine.pdf" })).unwrap();
        assert!(said.contains("papers/mine.pdf"));
        assert_eq!(files(&lib, e.id), ["papers/mine.pdf"]);
        assert_eq!(std::fs::read_dir(root.join(PAPERS)).unwrap().count(), 1, "no second copy");

        let outside = d.path().join("download.pdf");
        std::fs::write(&outside, tiny_pdf("other words")).unwrap();
        tools.attach_tool(&json!({ "citekey": e.citekey, "path": outside.to_str().unwrap() })).unwrap();
        assert_eq!(files(&lib, e.id).len(), 2);
        assert!(outside.is_file(), "the original stays");
        assert!(root.join(&files(&lib, e.id)[1]).is_file());
    }

    #[test]
    fn what_is_not_a_pdf_or_not_there_is_refused_and_says_so() {
        let d = TempDir::new("pdf-tools", "bad");
        let (root, tools, lib) = setup(&d);
        let e = entry(&lib, TITLE);
        std::fs::write(root.join("notes.txt"), "x").unwrap();
        std::fs::write(root.join("fake.pdf"), "not a pdf").unwrap();
        let call = |path: &str| tools.attach_tool(&json!({ "citekey": e.citekey, "path": path })).unwrap_err();
        assert!(call("notes.txt").contains(".pdf"));
        assert!(call("papers/none.pdf").contains("does not exist"));
        assert!(call("fake.pdf").contains("not a PDF"));
        assert!(tools.attach_tool(&json!({ "citekey": "nokey", "path": "x.pdf" })).unwrap_err().contains("library_search"));
        assert!(tools.attach_tool(&json!({})).is_err());
        assert!(files(&lib, e.id).is_empty());
    }

    #[test]
    fn one_candidate_binds_and_two_bind_nothing() {
        let d = TempDir::new("pdf-tools", "find");
        let (root, tools, lib) = setup(&d);
        let e = entry(&lib, TITLE);
        std::fs::write(root.join("papers/a.pdf"), tiny_pdf("nothing to see here")).unwrap();
        assert!(tools.attach_tool(&json!({ "citekey": e.citekey })).unwrap_err().contains("path"));
        std::fs::write(root.join("papers/Graph attention for tomography.pdf"), tiny_pdf("one")).unwrap();
        std::fs::write(root.join("papers/Graph attention for tomography v2.pdf"), tiny_pdf("two")).unwrap();
        assert!(tools.attach_tool(&json!({ "citekey": e.citekey })).unwrap_err().contains("several"));
        std::fs::remove_file(root.join("papers/Graph attention for tomography v2.pdf")).unwrap();
        tools.attach_tool(&json!({ "citekey": e.citekey })).unwrap();
        assert_eq!(files(&lib, e.id), ["papers/Graph attention for tomography.pdf"]);
    }

    #[test]
    fn adding_a_reference_binds_its_pdf_by_itself_or_as_told() {
        use super::super::lookup::Candidate;
        use super::super::tools::Lookup;
        struct None_;
        impl Lookup for None_ {
            fn by_doi(&self, _: &str) -> Result<Vec<Candidate>, String> {
                Err("no".into())
            }
            fn by_title(&self, _: &str, _: &str) -> Result<Vec<Candidate>, String> {
                Err("no".into())
            }
        }
        let d = TempDir::new("pdf-tools", "add");
        let (root, tools, lib) = setup(&d);
        std::fs::write(root.join("papers/scan.pdf"), tiny_pdf("Graph attention for tomography")).unwrap();
        let add = |args: Value| tools.call("library_add", &args, &None_).unwrap().unwrap();
        let said = add(json!({ "csl": { "type": "book", "title": TITLE } }));
        assert!(said.contains("Attached papers/scan.pdf"), "{said}");
        let first = lib.list().unwrap()[0].clone();
        assert_eq!(files(&lib, first.id), ["papers/scan.pdf"]);

        let plain = add(json!({ "csl": { "type": "book", "title": "Nothing on disk for this one" } }));
        assert!(!plain.contains("ttached"), "{plain}");

        std::fs::write(root.join("own.pdf"), tiny_pdf("words")).unwrap();
        let told = add(json!({ "csl": { "type": "book", "title": "A third book with a file" }, "pdf": "own.pdf" }));
        assert!(told.contains("Attached papers/"), "{told}");

        let bad = add(json!({ "csl": { "type": "book", "title": "A fourth book, wrong file" }, "pdf": "none.pdf" }));
        assert!(bad.starts_with("added") && bad.contains("not attached"), "the reference still goes in: {bad}");

        std::fs::write(root.join("again.pdf"), tiny_pdf("more words")).unwrap();
        let repeat = add(json!({ "csl": { "type": "book", "title": "A fourth book, wrong file" }, "pdf": "again.pdf" }));
        assert!(repeat.contains("not added again") && repeat.contains("Attached"), "a repeat still takes the PDF: {repeat}");
    }

    #[test]
    fn all_binds_what_it_can_and_a_held_file_is_not_taken_twice() {
        let d = TempDir::new("pdf-tools", "all");
        let (root, tools, lib) = setup(&d);
        let a = entry(&lib, TITLE);
        let b = lib.add(&json!({ "type": "book", "title": "A long book about trees" }), "manual").unwrap();
        std::fs::write(root.join("papers/x.pdf"), tiny_pdf("doi 10.1234/abc.5")).unwrap();
        let said = tools.attach_tool(&json!({ "all": true })).unwrap();
        assert!(said.starts_with("Bound 1 PDFs; 1 references"), "{said}");
        assert_eq!(files(&lib, a.id), ["papers/x.pdf"]);
        assert!(files(&lib, b.id).is_empty());
        let again = tools.attach_tool(&json!({ "all": true })).unwrap();
        assert!(again.starts_with("Bound 0"), "{again}");
    }
}
