//! A first guess at a reference from the text of a PDF's first page: DOI,
//! arXiv id, year, title and authors, by plain rules. It is only a draft the
//! user confirms: every field it fills is reported in `guessed`, and a PDF
//! that gives nothing gives an empty record.

use super::csl::{date_parts, normalize_doi, person};
use serde::Serialize;
use serde_json::{json, Map, Value};

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Guess {
    pub csl: Value,
    /// The fields the rules filled (`title`, `author`, `issued`, `DOI`, `URL`).
    pub guessed: Vec<&'static str>,
}

/// The first DOI in `text` (`10.<4-9 digits>/<suffix>`), without trailing punctuation.
pub fn find_doi(text: &str) -> Option<String> {
    let mut from = 0;
    while let Some(i) = text[from..].find("10.").map(|i| i + from) {
        let rest = &text[i + 3..];
        let digits = rest.chars().take_while(char::is_ascii_digit).count();
        if (4..=9).contains(&digits) && rest[digits..].starts_with('/') {
            let suffix: String = rest[digits + 1..]
                .chars()
                .take_while(|c| !c.is_whitespace() && !"\"<>[]{}|\\^`".contains(*c))
                .collect();
            let suffix = suffix.trim_end_matches(['.', ',', ';', ':', ')', '\'']);
            if !suffix.is_empty() {
                return Some(normalize_doi(&format!("10.{}/{suffix}", &rest[..digits])));
            }
        }
        from = i + 3;
    }
    None
}

/// An arXiv id (`arXiv:2301.12345` or the old `hep-th/9901001` form after `arXiv:`).
pub fn find_arxiv(text: &str) -> Option<String> {
    let lower = text.to_ascii_lowercase();
    let at = lower.find("arxiv:")? + 6;
    let id: String = text[at..]
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || ".-/".contains(*c))
        .collect();
    let id = id.trim_end_matches('.');
    (id.len() >= 7 && id.chars().any(|c| c.is_ascii_digit())).then(|| id.to_string())
}

/// The first plausible publication year (1900–2099) in `text`, outside any DOI or arXiv id.
pub fn find_year(text: &str) -> Option<i32> {
    let stripped: String = text
        .split_whitespace()
        .filter(|w| !w.contains("10.") && !w.to_ascii_lowercase().contains("arxiv"))
        .collect::<Vec<_>>()
        .join(" ");
    let chars: Vec<char> = stripped.chars().collect();
    for i in 0..chars.len().saturating_sub(3) {
        let w = &chars[i..i + 4];
        let boundary_before = i == 0 || !chars[i - 1].is_ascii_digit();
        let boundary_after = chars.get(i + 4).is_none_or(|c| !c.is_ascii_digit());
        if boundary_before && boundary_after && w.iter().all(char::is_ascii_digit) {
            let year: i32 = w.iter().collect::<String>().parse().ok()?;
            if (1900..=2099).contains(&year) {
                return Some(year);
            }
        }
    }
    None
}

fn clean_line(line: &str) -> String {
    line.trim()
        .trim_start_matches('#')
        .trim()
        .replace(['*', '_'], "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// A line that is page furniture or a heading word rather than a title.
fn furniture(line: &str) -> bool {
    let lower = line.to_lowercase();
    let letters = line.chars().filter(|c| c.is_alphabetic()).count();
    letters < 8
        || line.len() > 300
        || lower.contains("http")
        || lower.contains("doi")
        || lower.contains("arxiv")
        || lower.contains("copyright")
        || lower.contains('©')
        || lower.starts_with("page ")
        || lower.starts_with("vol")
        || (find_year(line).is_some() && line.contains(['卷', '期', '年']))
        || lower.starts_with("abstract")
        || lower.starts_with("摘要")
        || lower.starts_with("keywords")
        || lower.contains("received")
        || lower.contains("accepted")
        || lower.contains("preprint")
        || lower.contains("proceedings")
}

/// A line of names: short, mostly letters, separated by commas, "and" or 、.
fn looks_like_authors(line: &str) -> bool {
    let separators = line.matches([',', '，', '、', ';']).count() + line.matches(" and ").count();
    let digits = line.chars().filter(char::is_ascii_digit).count();
    line.len() <= 200 && digits * 6 < line.len() && (separators >= 1 || line.split_whitespace().count() <= 3)
        && !line.to_lowercase().contains("university")
        && !line.to_lowercase().contains("institute")
        && !line.contains('@')
}

fn split_authors(line: &str) -> Vec<Value> {
    let cleaned: String = line
        .chars()
        .filter(|c| !"*†‡§¶0123456789".contains(*c))
        .collect();
    cleaned
        .replace(" and ", ",")
        .replace(['，', '、', ';'], ",")
        .split(',')
        .map(str::trim)
        .filter(|n| !n.is_empty() && n.len() < 60)
        .map(|n| {
            // A name in CJK is one family+given block: keep it as written.
            if n.chars().any(|c| c as u32 > 0x2E80) { json!({ "literal": n }) } else { person(n) }
        })
        .collect()
}

/// A draft record from the first page's Markdown/text.
pub fn guess(text: &str) -> Guess {
    let mut csl = Map::new();
    let mut guessed: Vec<&'static str> = Vec::new();
    let lines: Vec<String> = text
        .lines()
        .map(clean_line)
        .filter(|l| !l.is_empty())
        .take(60)
        .collect();

    let heading = text
        .lines()
        .map(str::trim)
        .find(|l| l.starts_with('#'))
        .map(clean_line)
        .filter(|l| !furniture(l));
    let title_at = heading
        .as_ref()
        .and_then(|h| lines.iter().position(|l| l == h))
        .or_else(|| lines.iter().position(|l| !furniture(l)));
    if let Some(i) = title_at {
        let mut title = lines[i].clone();
        // A long title wraps onto the next line; take it while it still reads as one.
        if let Some(next) = lines.get(i + 1) {
            if title.len() < 90 && !furniture(next) && !looks_like_authors(next) && next.len() < 90 {
                title = format!("{title} {next}");
            }
        }
        csl.insert("title".into(), Value::String(title));
        guessed.push("title");
        let after = i + 1 + usize::from(csl["title"].as_str().is_some_and(|t| t.len() > lines[i].len()));
        if let Some(authors) = lines.get(after).filter(|l| looks_like_authors(l)) {
            let people = split_authors(authors);
            if !people.is_empty() {
                csl.insert("author".into(), Value::Array(people));
                guessed.push("author");
            }
        }
    }
    if let Some(doi) = find_doi(text) {
        csl.insert("DOI".into(), Value::String(doi));
        guessed.push("DOI");
    } else if let Some(id) = find_arxiv(text) {
        csl.insert("URL".into(), Value::String(format!("https://arxiv.org/abs/{id}")));
        guessed.push("URL");
    }
    if let Some(year) = find_year(text) {
        csl.insert("issued".into(), date_parts(year));
        guessed.push("issued");
    }
    csl.insert("type".into(), Value::String("article-journal".into()));
    Guess { csl: Value::Object(csl), guessed }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_doi_is_found_and_trimmed() {
        assert_eq!(find_doi("see https://doi.org/10.1038/nature12373."), Some("10.1038/nature12373".into()));
        assert_eq!(find_doi("DOI: 10.1000/xyz-123 (2020)"), Some("10.1000/xyz-123".into()));
        assert_eq!(find_doi("version 10.5 and page 10.2"), None);
        assert_eq!(find_doi("10.123/short"), None);
    }

    #[test]
    fn an_arxiv_id_and_a_year_are_found_outside_identifiers() {
        assert_eq!(find_arxiv("arXiv:2301.12345v2 [cs.CL]"), Some("2301.12345v2".into()));
        assert_eq!(find_arxiv("no id"), None);
        assert_eq!(find_year("arXiv:2301.12345 Published 2021 in"), Some(2021));
        assert_eq!(find_year("pages 12345 and 1899"), None);
        assert_eq!(find_year("doi 10.1234/abc2019 then 2018"), Some(2018));
    }

    #[test]
    fn a_first_page_gives_title_authors_doi_and_year() {
        let page = "Journal of Things, Vol. 12\n\n# Deep learning for citation parsing\n\nJohn Smith, Jane Doe\n\nDepartment of Computer Science, University of X\n\nAbstract We study... Published 2020. https://doi.org/10.1000/xyz123\n";
        let g = guess(page);
        assert_eq!(g.csl["title"], "Deep learning for citation parsing");
        assert_eq!(g.csl["author"][0]["family"], "Smith");
        assert_eq!(g.csl["author"][1]["given"], "Jane");
        assert_eq!(g.csl["DOI"], "10.1000/xyz123");
        assert_eq!(g.csl["issued"]["date-parts"][0][0], 2020);
        assert_eq!(g.guessed, ["title", "author", "DOI", "issued"]);
    }

    #[test]
    fn a_chinese_first_page_keeps_names_as_written() {
        let g = guess("计算机学报 2021年第44卷\n\n基于深度学习的文献检索方法研究\n\n张伟，李明\n\n摘要 本文研究...\n");
        assert_eq!(g.csl["title"], "基于深度学习的文献检索方法研究");
        assert_eq!(g.csl["author"][0]["literal"], "张伟");
        assert_eq!(g.csl["issued"]["date-parts"][0][0], 2021);
    }

    #[test]
    fn nothing_recognisable_gives_an_empty_draft() {
        let g = guess("1 2 3\n---\n");
        assert!(g.csl.get("title").is_none());
        assert!(g.guessed.is_empty());
    }
}
