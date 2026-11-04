//! Citation styles (L2): `hayagriva`'s CSL engine over the library's records.
//!
//! Records go in as BibTeX (our own writer, then `hayagriva`'s reader), so the
//! formatter sees exactly what an export would. Built-in styles come from the
//! bundled CSL archive; a user can add `.csl` files, which are only parsed as
//! XML (CSL has no scripts) and refused unless they are independent styles.

use super::{bibtex, Entry};
use hayagriva::archive::{locales, ArchivedStyle};
use hayagriva::citationberg::{IndependentStyle, Style};
use hayagriva::io::from_biblatex_str;
use hayagriva::{
    BibliographyDriver, BibliographyRequest, BufWriteFormat, CitationItem, CitationRequest,
};
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Styles offered first; every id is a name in the bundled archive.
const BUILT_IN: &[(&str, &str)] = &[
    ("apa", "APA 7"),
    ("ieee", "IEEE"),
    ("chicago-author-date", "Chicago (author-date)"),
    ("chicago-notes", "Chicago (notes and bibliography)"),
    ("modern-language-association", "MLA"),
    ("nature", "Nature"),
    ("vancouver", "Vancouver"),
    ("harvard-cite-them-right", "Harvard"),
    ("gb-7714-2015-numeric", "GB/T 7714-2015 顺序编码制"),
    ("gb-7714-2015-author-date", "GB/T 7714-2015 著者-出版年制"),
    ("gb-7714-2015-note", "GB/T 7714-2015 脚注"),
    ("gb-7714-2005-numeric", "GB/T 7714-2005 顺序编码制"),
];

/// Largest `.csl` file accepted.
pub const MAX_STYLE_BYTES: usize = 2 * 1024 * 1024;

const USER_PREFIX: &str = "user:";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct StyleInfo {
    pub id: String,
    pub name: String,
    pub builtin: bool,
    /// `common` (the short list first offered), `archive` (the rest of the bundled archive: publishers,
    /// journal families, societies, regional standards) or `user` (a `.csl` the user added).
    pub group: &'static str,
}

/// One formatted line: plain text and HTML for the record `id`.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Line {
    pub id: i64,
    pub text: String,
    pub html: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Formatted {
    /// The reference list in the style's own order.
    pub bibliography: Vec<Line>,
    /// The in-text citation of each record, in the order given.
    pub citations: Vec<Line>,
}

fn slug(text: &str) -> String {
    let s: String = text
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect();
    s.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-")
}

/// The short list first, then every other independent style of the bundled archive by name, then the
/// user's (`<dir>/*.csl`).
pub fn styles(user_dir: &Path) -> Vec<StyleInfo> {
    let mut out: Vec<StyleInfo> = BUILT_IN
        .iter()
        .map(|(id, name)| StyleInfo { id: (*id).into(), name: (*name).into(), builtin: true, group: "common" })
        .collect();
    let common: Vec<ArchivedStyle> = BUILT_IN.iter().filter_map(|(id, _)| ArchivedStyle::by_name(id)).collect();
    let mut more: Vec<StyleInfo> = ArchivedStyle::all()
        .iter()
        .filter(|s| !common.contains(s) && matches!(s.get(), Style::Independent(_)))
        .filter_map(|s| {
            let id = s.names().first()?;
            Some(StyleInfo { id: (*id).into(), name: s.display_name().into(), builtin: true, group: "archive" })
        })
        .collect();
    more.sort_by_key(|s| s.name.to_lowercase());
    out.extend(more);
    let mut user: Vec<StyleInfo> = std::fs::read_dir(user_dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            if path.extension()?.to_str()? != "csl" {
                return None;
            }
            let stem = path.file_stem()?.to_str()?.to_string();
            let style = parse_user(&std::fs::read_to_string(&path).ok()?).ok()?;
            let title = style.info.title.value.to_string();
            Some(StyleInfo { id: format!("{USER_PREFIX}{stem}"), name: title, builtin: false, group: "user" })
        })
        .collect();
    user.sort_by_key(|s| s.name.to_lowercase());
    out.extend(user);
    out
}

fn parse_user(xml: &str) -> Result<IndependentStyle, String> {
    IndependentStyle::from_xml(xml).map_err(|e| {
        format!("not a usable CSL style (it must be an independent style): {e}")
    })
}

/// Checks `xml` and saves it as `<dir>/<slug of its title>.csl`; returns the new style.
pub fn add_user_style(user_dir: &Path, xml: &str) -> Result<StyleInfo, String> {
    if xml.len() > MAX_STYLE_BYTES {
        return Err("the style file is too large".into());
    }
    let style = parse_user(xml)?;
    let title = style.info.title.value.to_string();
    let stem = slug(&title);
    if stem.is_empty() {
        return Err("the style has no usable title".into());
    }
    std::fs::create_dir_all(user_dir).map_err(|e| format!("could not create the styles folder: {e}"))?;
    let path: PathBuf = user_dir.join(format!("{stem}.csl"));
    std::fs::write(&path, xml).map_err(|e| format!("could not save the style: {e}"))?;
    Ok(StyleInfo { id: format!("{USER_PREFIX}{stem}"), name: title, builtin: false, group: "user" })
}

fn load(user_dir: &Path, id: &str) -> Result<IndependentStyle, String> {
    if let Some(stem) = id.strip_prefix(USER_PREFIX) {
        if stem.is_empty() || stem.contains(['/', '\\', '.']) {
            return Err("unknown style".into());
        }
        let xml = std::fs::read_to_string(user_dir.join(format!("{stem}.csl")))
            .map_err(|_| "that style file is gone".to_string())?;
        return parse_user(&xml);
    }
    match ArchivedStyle::by_name(id).map(ArchivedStyle::get) {
        Some(Style::Independent(style)) => Ok(style),
        _ => Err(format!("unknown style \"{id}\"")),
    }
}

/// Formats `entries` in `style_id`.
pub fn format(user_dir: &Path, style_id: &str, entries: &[Entry]) -> Result<Formatted, String> {
    let style = load(user_dir, style_id)?;
    if entries.is_empty() {
        return Ok(Formatted { bibliography: vec![], citations: vec![] });
    }
    let bib = bibtex::write(
        &entries.iter().map(|e| (e.citekey.clone(), e.csl.clone())).collect::<Vec<_>>(),
    );
    let library = from_biblatex_str(&bib).map_err(|errors| {
        format!("could not read a record for formatting: {}", errors.first().map_or_else(String::new, ToString::to_string))
    })?;
    let locales = locales();
    let mut driver = BibliographyDriver::new();
    let mut order: Vec<&Entry> = Vec::new();
    for entry in entries {
        let Some(item) = library.get(&entry.citekey) else { continue };
        driver.citation(CitationRequest::from_items(
            vec![CitationItem::with_entry(item)],
            &style,
            &locales,
        ));
        order.push(entry);
    }
    let rendered = driver.finish(BibliographyRequest { style: &style, locale: None, locale_files: &locales });
    let id_of = |key: &str| entries.iter().find(|e| e.citekey == key).map_or(0, |e| e.id);
    let write = |content: &hayagriva::ElemChildren| {
        let mut text = String::new();
        let mut html = String::new();
        let _ = content.write_buf(&mut text, BufWriteFormat::Plain);
        let _ = content.write_buf(&mut html, BufWriteFormat::Html);
        (text, html)
    };
    let bibliography = rendered
        .bibliography
        .map(|b| {
            b.items
                .iter()
                .map(|item| {
                    let (mut text, mut html) = write(&item.content);
                    if let Some(first) = &item.first_field {
                        // Numbered styles keep the label ("[1]") apart from the entry.
                        let (label, label_html) = write(&hayagriva::ElemChildren(vec![first.clone()]));
                        text = format!("{label} {text}");
                        html = format!("{label_html} {html}");
                    }
                    Line { id: id_of(&item.key), text, html }
                })
                .collect()
        })
        .unwrap_or_default();
    let citations = rendered
        .citations
        .iter()
        .zip(order)
        .map(|(c, entry)| {
            let mut text = String::new();
            let mut html = String::new();
            let _ = c.citation.write_buf(&mut text, BufWriteFormat::Plain);
            let _ = c.citation.write_buf(&mut html, BufWriteFormat::Html);
            Line { id: entry.id, text, html }
        })
        .collect();
    Ok(Formatted { bibliography, citations })
}

/// A draft with its `[@key]` markers replaced by in-text citations and a reference list appended.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Cited {
    pub text: String,
    /// How many markers were replaced.
    pub cited: usize,
    /// Keys in the draft that are not in the library (their markers are left as written).
    pub missing: Vec<String>,
}

/// The `[@a; @b]` markers of `draft`: byte range and the keys.
pub(super) fn markers(draft: &str) -> Vec<(usize, usize, Vec<String>)> {
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(open) = draft[from..].find("[@").map(|i| i + from) {
        let Some(close) = draft[open..].find(']').map(|i| i + open) else { break };
        let keys: Vec<String> = draft[open + 1..close]
            .split(';')
            .map(str::trim)
            .filter_map(|k| k.strip_prefix('@'))
            .map(str::to_string)
            .collect();
        let valid = !keys.is_empty()
            && keys.iter().all(|k| {
                !k.is_empty() && k.chars().all(|c| c.is_alphanumeric() || "_:-.".contains(c))
            });
        if valid {
            found.push((open, close + 1, keys));
            from = close + 1;
        } else {
            from = open + 2;
        }
    }
    found
}

/// Replaces the `[@citekey]` markers of `draft` using `entries` and appends the reference list.
pub fn cite_draft(
    user_dir: &Path,
    style_id: &str,
    entries: &[Entry],
    draft: &str,
) -> Result<Cited, String> {
    let style = load(user_dir, style_id)?;
    let bib = bibtex::write(
        &entries.iter().map(|e| (e.citekey.clone(), e.csl.clone())).collect::<Vec<_>>(),
    );
    let library = if entries.is_empty() {
        hayagriva::Library::new()
    } else {
        from_biblatex_str(&bib).map_err(|errors| {
            format!("could not read a record for formatting: {}", errors.first().map_or_else(String::new, ToString::to_string))
        })?
    };
    let locales = locales();
    let mut driver = BibliographyDriver::new();
    let mut missing: Vec<String> = Vec::new();
    let mut replaced: Vec<(usize, usize)> = Vec::new();
    for (start, end, keys) in markers(draft) {
        let mut items = Vec::new();
        for key in &keys {
            match library.get(key) {
                Some(entry) => items.push(CitationItem::with_entry(entry)),
                None if !missing.contains(key) => missing.push(key.clone()),
                None => {}
            }
        }
        if items.is_empty() {
            continue;
        }
        driver.citation(CitationRequest::from_items(items, &style, &locales));
        replaced.push((start, end));
    }
    if replaced.is_empty() {
        return Ok(Cited { text: draft.to_string(), cited: 0, missing });
    }
    let rendered = driver.finish(BibliographyRequest { style: &style, locale: None, locale_files: &locales });
    let plain = |content: &hayagriva::ElemChildren| {
        let mut text = String::new();
        let _ = content.write_buf(&mut text, BufWriteFormat::Plain);
        text
    };
    let mut out = String::new();
    let mut at = 0;
    for ((start, end), citation) in replaced.iter().zip(&rendered.citations) {
        out.push_str(&draft[at..*start]);
        out.push_str(&plain(&citation.citation));
        at = *end;
    }
    out.push_str(&draft[at..]);
    if let Some(bibliography) = rendered.bibliography {
        out.push_str("\n\n## References\n\n");
        for item in bibliography.items {
            if let Some(first) = &item.first_field {
                out.push_str(&plain(&hayagriva::ElemChildren(vec![first.clone()])));
                out.push(' ');
            }
            out.push_str(&plain(&item.content));
            out.push_str("\n\n");
        }
    }
    Ok(Cited { text: out, cited: replaced.len(), missing })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::{import, Format, Library};
    use crate::test_support::TempDir;

    const BIB: &str = r#"
@article{smith2020deep, author = {Smith, John and Doe, Jane}, title = {Deep learning for citation parsing}, journal = {Journal of Machine Learning Research}, year = {2020}, volume = {21}, number = {3}, pages = {100--120}, doi = {10.1000/xyz123}}
@book{wang2019, author = {王, 芳}, title = {信息检索原理}, publisher = {科学出版社}, address = {北京}, year = {2019}}
"#;

    fn entries(dir: &TempDir) -> Vec<Entry> {
        let lib = Library::open(&dir.path().join("p")).unwrap();
        import(&lib, Format::Bibtex, BIB).unwrap();
        lib.list().unwrap()
    }

    #[test]
    fn every_built_in_style_exists_and_is_independent() {
        let dir = TempDir::new("librarian", "styles-exist");
        for (id, _) in BUILT_IN {
            load(dir.path(), id).unwrap_or_else(|e| panic!("{id}: {e}"));
        }
    }

    #[test]
    fn gb_t_7714_formats_a_chinese_book() {
        let dir = TempDir::new("librarian", "gbt");
        let out = format(dir.path(), "gb-7714-2015-numeric", &entries(&dir)).unwrap();
        assert!(
            out.bibliography.iter().any(|l| l.text.contains("王芳. 信息检索原理[M]. 北京: 科学出版社, 2019.")),
            "{:?}",
            out.bibliography
        );
        assert_eq!(out.citations.len(), 2);
    }

    #[test]
    fn apa_formats_an_english_article_with_its_doi() {
        let dir = TempDir::new("librarian", "apa");
        let out = format(dir.path(), "apa", &entries(&dir)).unwrap();
        assert!(out
            .bibliography
            .iter()
            .any(|l| l.text.starts_with("Smith, J., & Doe, J. (2020). Deep learning for citation parsing.")
                && l.text.contains("https://doi.org/10.1000/xyz123")));
        assert!(out.citations.iter().any(|l| l.text == "(Smith & Doe, 2020)"));
    }

    #[test]
    fn a_user_style_is_validated_stored_and_listed() {
        let dir = TempDir::new("librarian", "user-style");
        let user = dir.path().join("csl");
        assert!(add_user_style(&user, "<style>nope</style>").is_err());
        assert!(add_user_style(&user, "not xml").is_err());
        assert!(styles(&user).iter().all(|s| s.builtin));
    }

    #[test]
    fn a_valid_user_style_formats_the_library() {
        const CSL: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<style xmlns="http://purl.org/net/xbiblio/csl" class="in-text" version="1.0">
  <info><title>My Test Style</title><id>http://example.com/test</id><updated>2020-01-01T00:00:00+00:00</updated></info>
  <citation><layout><text variable="title"/></layout></citation>
  <bibliography><layout><text variable="title"/></layout></bibliography>
</style>"#;
        let dir = TempDir::new("librarian", "user-style-ok");
        let user = dir.path().join("csl");
        let added = add_user_style(&user, CSL).unwrap();
        assert_eq!(added.id, "user:my-test-style");
        assert_eq!(styles(&user).last().map(|s| s.name.as_str()), Some("My Test Style"));
        let out = format(&user, &added.id, &entries(&dir)).unwrap();
        assert!(out.bibliography.iter().any(|l| l.text == "信息检索原理"), "{:?}", out.bibliography);
    }

    #[test]
    fn numbered_styles_keep_their_label_in_the_list() {
        let dir = TempDir::new("librarian", "label");
        let out = format(dir.path(), "ieee", &entries(&dir)).unwrap();
        assert!(out.bibliography.iter().all(|l| l.text.starts_with('[')), "{:?}", out.bibliography);
    }

    #[test]
    fn a_draft_gets_citations_a_reference_list_and_a_report_of_unknown_keys() {
        let dir = TempDir::new("librarian", "draft");
        let all = entries(&dir);
        let keys: Vec<&str> = all.iter().map(|e| e.citekey.as_str()).collect();
        let draft = format!("As shown [@{}], and [@{}; @nobody2000] again [@ghost]. Mail me [@ here].", keys[0], keys[1]);
        let out = cite_draft(dir.path(), "apa", &all, &draft).unwrap();
        assert_eq!(out.cited, 2);
        assert_eq!(out.missing, ["nobody2000", "ghost"]);
        assert!(out.text.contains("As shown (Smith & Doe, 2020), and (王, 2019) again [@ghost]. Mail me [@ here]."), "{}", out.text);
        assert!(out.text.contains("## References"));
        assert!(out.text.contains("Smith, J., & Doe, J. (2020)."));
        let none = cite_draft(dir.path(), "apa", &all, "no markers").unwrap();
        assert_eq!((none.text.as_str(), none.cited), ("no markers", 0));
    }

    #[test]
    fn style_ids_cannot_reach_other_files() {
        let dir = TempDir::new("librarian", "style-path");
        assert!(load(dir.path(), "user:../secret").is_err());
        assert!(load(dir.path(), "user:").is_err());
        assert!(load(dir.path(), "no-such-style").is_err());
    }
}

#[cfg(test)]
mod journal_tests {
    use super::*;
    use crate::test_support::TempDir;

    fn entry() -> Entry {
        let d = TempDir::new("format", "all-styles");
        let root = d.path().join("p");
        std::fs::create_dir_all(&root).unwrap();
        let lib = crate::librarian::Library::open(&root).unwrap();
        lib.add(
            &serde_json::json!({ "type": "article-journal", "title": "Deep graph learning", "container-title": "Nature", "volume": "7", "issue": "2", "page": "1-9",
                "issued": { "date-parts": [[2020]] }, "DOI": "10.1/x", "author": [{ "family": "Smith", "given": "Anna" }, { "family": "Lee", "given": "Wei" }] }),
            "manual",
        )
        .unwrap();
        lib.list().unwrap().remove(0)
    }

    #[test]
    fn every_listed_style_formats_a_reference() {
        let dir = std::env::temp_dir();
        let listed = styles(&dir);
        assert!(listed.len() > 60, "the whole archive is offered, not a short list: {}", listed.len());
        assert_eq!(listed[0].group, "common");
        let e = entry();
        for s in &listed {
            let out = format(&dir, &s.id, std::slice::from_ref(&e)).unwrap_or_else(|err| panic!("{}: {err}", s.id));
            // A notes style may have no reference list for one entry; then it has its citation.
            assert!(!out.bibliography.is_empty() || !out.citations.is_empty(), "{} formatted nothing", s.id);
        }
    }

    /// Every `style: "…"` of the journal presets (src/app/librarian-journals.js) names a style the app has.
    #[test]
    #[ignore = "prints the bundled style ids"]
    fn list_ids() {
        for s in ArchivedStyle::all() {
            println!("ID {:?} | {}", s.names(), s.display_name());
        }
    }

    #[test]
    fn every_journal_preset_names_a_bundled_style() {
        let js = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../src/app/librarian-journals.js")).unwrap();
        let mut seen = 0;
        for line in js.lines().filter(|l| l.trim_start().starts_with("p(\"")) {
            let after = line.split("\", \"").nth(1);
            let Some(rest) = after else { continue };
            let id = rest.split('"').next().unwrap_or("");
            if id.is_empty() || line.contains("\", null,") {
                continue;
            }
            assert!(
                matches!(ArchivedStyle::by_name(id).map(ArchivedStyle::get), Some(Style::Independent(_))),
                "preset style {id:?} is not a bundled independent style"
            );
            seen += 1;
        }
        assert!(seen > 60, "read {seen} presets");
    }
}
