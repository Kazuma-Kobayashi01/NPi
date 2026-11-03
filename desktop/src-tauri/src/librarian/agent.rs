//! The library as the Copilot and the main chat see it (L4): search, one
//! record with its formatted reference, and a deterministic check of a draft's
//! citations against the library. Everything here only reads; what the model
//! may change goes through `suggest.rs` and waits for the user.

use super::csl::{fields, normalize_doi};
use super::pdf_meta::find_doi;
use super::{format as format_mod, Entry};
use serde::Serialize;
use serde_json::Value;
use std::path::Path;

const STOPWORDS: &[&str] = &[
    "in", "on", "by", "since", "during", "from", "after", "before", "until", "the", "of", "at", "as",
    "for", "between", "early", "late", "around", "circa", "year", "see", "and", "or", "to", "with",
    "january", "february", "march", "april", "may", "june", "july", "august", "september", "october",
    "november", "december", "fig", "figure", "table", "section", "chapter", "page", "vol", "no",
];

fn text<'a>(csl: &'a Value, key: &str) -> &'a str {
    csl.get(key).and_then(Value::as_str).unwrap_or("").trim()
}

fn author_names(csl: &Value) -> Vec<String> {
    csl.get("author")
        .and_then(Value::as_array)
        .map(|a| a.iter().map(super::csl::person_text).filter(|n| !n.is_empty()).collect())
        .unwrap_or_default()
}

/// `Smith, Doe (2020). Title. Container.` for a result line.
fn headline(e: &Entry) -> String {
    let f = fields(&e.csl);
    let names = author_names(&e.csl);
    let authors = match names.len() {
        0 => String::new(),
        1 | 2 => names.join("; "),
        _ => format!("{} et al.", names[0]),
    };
    let year = f.year.map(|y| format!(" ({y})")).unwrap_or_default();
    let mut line = format!("[{}] {authors}{year}. {}", e.citekey, f.title);
    let container = text(&e.csl, "container-title");
    if !container.is_empty() {
        line.push_str(&format!(". {container}"));
    }
    line
}

/// The references whose title, authors, year, venue, DOI, citekey, tags, notes or abstract
/// contain the query terms, best first: more terms matched ranks higher.
pub fn search<'a>(entries: &'a [Entry], query: &str, limit: usize) -> Vec<&'a Entry> {
    let terms: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    if terms.is_empty() {
        return Vec::new();
    }
    let mut scored: Vec<(usize, &Entry)> = entries
        .iter()
        .filter_map(|e| {
            let hay = [
                text(&e.csl, "title"),
                &author_names(&e.csl).join(" "),
                &fields(&e.csl).year.map(|y| y.to_string()).unwrap_or_default(),
                text(&e.csl, "container-title"),
                text(&e.csl, "DOI"),
                text(&e.csl, "publisher"),
                text(&e.csl, "abstract"),
                &e.citekey,
                &e.tags.join(" "),
                &e.notes,
            ]
            .join("\n")
            .to_lowercase();
            let hits = terms.iter().filter(|t| hay.contains(t.as_str())).count();
            (hits > 0).then_some((hits, e))
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.citekey.cmp(&b.1.citekey)));
    scored.into_iter().take(limit).map(|(_, e)| e).collect()
}

pub fn render_search(hits: &[&Entry], query: &str) -> String {
    if hits.is_empty() {
        return format!("No reference in the library matches {query:?}. The library may simply not hold it yet.");
    }
    let mut out = format!("{} reference(s) in the library for {query:?}:\n", hits.len());
    for e in hits {
        out.push_str(&headline(e));
        if !e.tags.is_empty() {
            out.push_str(&format!(" · tags: {}", e.tags.join(", ")));
        }
        if let Some(a) = e.attachments.first() {
            out.push_str(&format!(" · file: {}", a.path));
        }
        out.push('\n');
    }
    out.push_str("Use library_get with a citekey for the full record.");
    out
}

fn clip(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max).collect::<String>())
    }
}

/// One record in full, with its reference formatted in a built-in `style`.
pub fn render_get(e: &Entry, style: &str) -> String {
    let f = fields(&e.csl);
    let mut out = format!("citekey: {}\ntype: {}\ntitle: {}\n", e.citekey, f.kind, f.title);
    let names = author_names(&e.csl);
    if !names.is_empty() {
        out.push_str(&format!("authors: {}\n", names.join("; ")));
    }
    if let Some(y) = f.year {
        out.push_str(&format!("year: {y}\n"));
    }
    for (key, label) in [
        ("container-title", "venue"), ("volume", "volume"), ("issue", "issue"), ("page", "pages"),
        ("publisher", "publisher"), ("DOI", "DOI"), ("URL", "URL"),
    ] {
        let v = text(&e.csl, key);
        if !v.is_empty() {
            out.push_str(&format!("{label}: {v}\n"));
        }
    }
    let r#abstract = text(&e.csl, "abstract");
    if !r#abstract.is_empty() {
        out.push_str(&format!("abstract: {}\n", clip(r#abstract, 1500)));
    }
    if !e.tags.is_empty() {
        out.push_str(&format!("tags: {}\n", e.tags.join(", ")));
    }
    if !e.notes.trim().is_empty() {
        out.push_str(&format!("user's notes: {}\n", clip(e.notes.trim(), 4000)));
    }
    for a in &e.attachments {
        out.push_str(&format!("file: {}{}\n", a.path, if a.missing { " (missing)" } else { "" }));
    }
    match format_mod::format(Path::new(""), style, std::slice::from_ref(e)) {
        Ok(f) => {
            if let Some(line) = f.bibliography.first() {
                out.push_str(&format!("reference ({style}): {}\n", line.text));
            }
            if let Some(c) = f.citations.first() {
                out.push_str(&format!("in-text ({style}): {}\n", c.text));
            }
        }
        Err(err) => out.push_str(&format!("(could not format in {style}: {err})\n")),
    }
    out
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Finding {
    /// `key` (`[@citekey]`), `author-year` or `doi`.
    pub kind: &'static str,
    /// As written in the draft.
    pub text: String,
    /// `matched`, `year-differs`, `not-in-library` or `unknown-key`.
    pub status: &'static str,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Report {
    pub findings: Vec<Finding>,
    pub matched: usize,
    pub problems: usize,
}

fn is_cjk(c: char) -> bool {
    ('\u{4E00}'..='\u{9FFF}').contains(&c)
}

fn same_name(found: &str, family: &str) -> bool {
    let (a, b) = (found.to_lowercase(), family.to_lowercase());
    !a.is_empty() && !b.is_empty() && (a == b || (found.chars().any(is_cjk) && (b.starts_with(&a) || a.starts_with(&b))))
}

/// The first author named just before byte `end` of `draft` (`Smith`, `Smith and Doe`,
/// `Smith et al.`, `张伟等`), or None when prose precedes the year instead of a name.
fn name_before(draft: &str, end: usize) -> Option<String> {
    let head = &draft[..end];
    let tail: String = {
        let chars: Vec<char> = head.chars().collect();
        chars[chars.len().saturating_sub(90)..].iter().collect()
    };
    let trimmed = tail.trim_end_matches(|c: char| c.is_whitespace() || "(（,，;；[".contains(c));
    let mut names: Vec<String> = Vec::new();
    for token in trimmed.split_whitespace().rev() {
        let clean = token.trim_matches(|c: char| ",.;:()（）[]".contains(c));
        let lower = clean.to_lowercase();
        if ["et", "al", "and", "&", "others"].contains(&lower.as_str()) {
            continue;
        }
        let name_like = clean.chars().next().is_some_and(char::is_uppercase)
            && clean.chars().all(|c| c.is_alphabetic() || "'’-".contains(c));
        if name_like && !STOPWORDS.contains(&lower.as_str()) {
            names.push(clean.to_string());
        } else {
            break;
        }
    }
    if let Some(first) = names.last() {
        return Some(first.clone());
    }
    // CJK: the run of ideographs and enumeration commas before the year, minus a trailing 等.
    let run: String = {
        let mut chars: Vec<char> = Vec::new();
        for c in trimmed.chars().rev() {
            if is_cjk(c) || c == '、' {
                chars.push(c);
                if chars.len() >= 20 {
                    break;
                }
            } else {
                break;
            }
        }
        chars.reverse();
        chars.into_iter().collect()
    };
    let run = run.trim_end_matches('等').trim_end_matches('人').trim_end_matches('等');
    let first_segment = run.split('、').next().unwrap_or("");
    // Without a list of names, a long run is prose ending in the name: names are 2-3 characters.
    let first: String = if first_segment.chars().count() > 4 {
        let chars: Vec<char> = first_segment.chars().collect();
        chars[chars.len() - 3..].iter().collect()
    } else {
        first_segment.to_string()
    };
    (first.chars().count() >= 2).then_some(first)
}

/// `(byte index of the year, year)` for each 4-digit year written like a citation year:
/// after a space or an opening bracket/comma, not part of a number or identifier.
fn years(draft: &str) -> Vec<(usize, i32)> {
    let bytes: Vec<(usize, char)> = draft.char_indices().collect();
    let mut found = Vec::new();
    for (i, &(at, c)) in bytes.iter().enumerate() {
        if !c.is_ascii_digit() || i + 3 >= bytes.len() {
            continue;
        }
        let digits: String = bytes[i..i + 4].iter().map(|&(_, c)| c).collect();
        let after_ok = bytes.get(i + 4).is_none_or(|&(_, c)| !c.is_ascii_digit());
        let before_ok = i == 0 || " (（,，;；[\n".contains(bytes[i - 1].1);
        if after_ok && before_ok && digits.chars().all(|c| c.is_ascii_digit()) {
            if let Ok(y) = digits.parse::<i32>() {
                if (1900..=2099).contains(&y) {
                    found.push((at, y));
                }
            }
        }
    }
    found
}

/// Checks the citations a draft makes against the library. Only `[@key]` markers,
/// `Author (Year)` / `(Author, Year)` mentions and DOIs are checked; a numbered
/// citation like `[1]` names nothing and is not.
pub fn check_citations(entries: &[Entry], draft: &str) -> Report {
    let mut findings: Vec<Finding> = Vec::new();
    for (start, end, keys) in format_mod::markers(draft) {
        for key in keys {
            let known = entries.iter().find(|e| e.citekey == key);
            findings.push(Finding {
                kind: "key",
                text: draft[start..end].to_string(),
                status: if known.is_some() { "matched" } else { "unknown-key" },
                detail: known.map_or_else(|| format!("no reference with the key {key}"), headline),
            });
        }
    }
    let mut seen: Vec<(String, i32)> = Vec::new();
    for (at, year) in years(draft) {
        let Some(name) = name_before(draft, at) else { continue };
        if seen.contains(&(name.clone(), year)) {
            continue;
        }
        seen.push((name.clone(), year));
        let by_author: Vec<&Entry> = entries
            .iter()
            .filter(|e| same_name(&name, &fields(&e.csl).first_author))
            .collect();
        let (status, detail) = if let Some(e) = by_author.iter().find(|e| fields(&e.csl).year == Some(year)) {
            ("matched", headline(e))
        } else if let Some(e) = by_author.first() {
            ("year-differs", format!("the library has {}", headline(e)))
        } else {
            ("not-in-library", "no reference by that first author (invented, or not added to the library yet)".to_string())
        };
        findings.push(Finding { kind: "author-year", text: format!("{name} {year}"), status, detail });
    }
    let mut rest = draft;
    let mut dois: Vec<String> = Vec::new();
    while let Some(doi) = find_doi(rest) {
        let at = rest.to_lowercase().find(&doi).map_or(rest.len(), |i| i + doi.len());
        if !dois.contains(&doi) {
            dois.push(doi);
        }
        rest = &rest[at.min(rest.len())..];
    }
    for doi in dois {
        let known = entries.iter().find(|e| fields(&e.csl).doi.as_deref() == Some(normalize_doi(&doi).as_str()));
        findings.push(Finding {
            kind: "doi",
            text: doi,
            status: if known.is_some() { "matched" } else { "not-in-library" },
            detail: known.map_or_else(|| "no reference with that DOI".to_string(), headline),
        });
    }
    let matched = findings.iter().filter(|f| f.status == "matched").count();
    let problems = findings.len() - matched;
    Report { findings, matched, problems }
}

pub fn render_report(r: &Report) -> String {
    if r.findings.is_empty() {
        return "No checkable citations found. Only [@citekey], Author (Year) / (Author, Year) and DOIs are checked; numbered citations like [1] are not.".into();
    }
    let mut out = format!(
        "{} citation(s) checked: {} matched the library, {} need attention. Only [@citekey], Author (Year) / (Author, Year) and DOIs are checked; numbered citations like [1] are not.\n",
        r.findings.len(), r.matched, r.problems
    );
    for f in &r.findings {
        let mark = if f.status == "matched" { "ok" } else { "CHECK" };
        out.push_str(&format!("{mark} [{}] {} — {}: {}\n", f.kind, f.text, f.status, f.detail));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::{import, Format, Library};
    use crate::test_support::TempDir;

    const BIB: &str = r#"
@article{smith2020deep, author = {Smith, John and Doe, Jane}, title = {Deep learning for citation parsing}, journal = {Journal of Machine Learning Research}, year = {2020}, doi = {10.1000/xyz123}}
@book{lee2018trees, author = {Lee, Al}, title = {Trees and forests}, publisher = {MIT Press}, year = {2018}}
@book{wang2019, author = {王, 芳}, title = {信息检索原理}, publisher = {科学出版社}, address = {北京}, year = {2019}}
"#;

    fn entries(dir: &TempDir) -> Vec<Entry> {
        let lib = Library::open(&dir.path().join("p")).unwrap();
        import(&lib, Format::Bibtex, BIB).unwrap();
        lib.list().unwrap()
    }

    #[test]
    fn search_ranks_by_terms_matched_and_reads_chinese() {
        let dir = TempDir::new("librarian", "agent-search");
        let all = entries(&dir);
        assert_eq!(search(&all, "deep learning smith", 5)[0].citekey, "smith2020deep");
        let chinese = search(&all, "信息检索", 5);
        assert_eq!(chinese.len(), 1);
        assert!(chinese[0].citekey.contains("2019"), "{}", chinese[0].citekey);
        assert!(search(&all, "   ", 5).is_empty());
        assert!(render_search(&search(&all, "zzzz", 5), "zzzz").contains("No reference"));
        let shown = render_search(&search(&all, "trees", 5), "trees");
        assert!(shown.contains("[lee2018trees] Lee, Al (2018). Trees and forests"), "{shown}");
    }

    #[test]
    fn one_record_comes_with_its_formatted_reference() {
        let dir = TempDir::new("librarian", "agent-get");
        let all = entries(&dir);
        let smith = all.iter().find(|e| e.citekey == "smith2020deep").unwrap();
        let out = render_get(smith, "apa");
        assert!(out.contains("authors: Smith, John; Doe, Jane"), "{out}");
        assert!(out.contains("DOI: 10.1000/xyz123"));
        assert!(out.contains("reference (apa): Smith, J., & Doe, J. (2020)."), "{out}");
        assert!(out.contains("in-text (apa): (Smith & Doe, 2020)"));
        assert!(render_get(smith, "user:nope").contains("could not format"));
    }

    #[test]
    fn a_draft_is_checked_against_keys_author_years_and_dois() {
        let dir = TempDir::new("librarian", "agent-check");
        let all = entries(&dir);
        let draft = "As Smith and Doe (2020) showed [@smith2020deep], and (Lee, 2019) differs; \
            Jones et al. (2021) is unknown, see also [@ghost2000]. In 2020 nothing happened. \
            DOI 10.1000/xyz123 and 10.9999/nothere. 王芳(2019)指出，张伟等（2021）也提到。";
        let report = check_citations(&all, draft);
        let status = |kind: &str, text: &str| {
            report.findings.iter().find(|f| f.kind == kind && f.text.contains(text)).map(|f| f.status)
        };
        assert_eq!(status("key", "smith2020deep"), Some("matched"));
        assert_eq!(status("key", "ghost2000"), Some("unknown-key"));
        assert_eq!(status("author-year", "Smith 2020"), Some("matched"));
        assert_eq!(status("author-year", "Lee 2019"), Some("year-differs"));
        assert_eq!(status("author-year", "Jones 2021"), Some("not-in-library"));
        assert_eq!(status("author-year", "王芳 2019"), Some("matched"));
        assert_eq!(status("author-year", "张伟 2021"), Some("not-in-library"));
        assert_eq!(status("doi", "10.1000/xyz123"), Some("matched"));
        assert_eq!(status("doi", "10.9999/nothere"), Some("not-in-library"));
        assert!(report.findings.iter().all(|f| !f.text.starts_with("In ")), "prose before a year is not a name");
        assert_eq!(report.matched + report.problems, report.findings.len());
        let shown = render_report(&report);
        assert!(shown.contains("CHECK [author-year] Jones 2021"), "{shown}");
        assert!(render_report(&check_citations(&all, "nothing to see")).contains("No checkable citations"));
    }
}
