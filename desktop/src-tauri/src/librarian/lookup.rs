//! Metadata lookup (L3), started only by a click in the reference form. The
//! hosts it may reach are `api.crossref.org` and, when Crossref has no answer,
//! `api.openalex.org` (arXiv, preprints and much Chinese work are only there),
//! over HTTPS, with no redirects; what leaves the machine is a DOI, or a title
//! (and a first author's family name) — never a PDF or any other part of the
//! library. Results are only candidates: the webview shows them against the
//! form and the user picks which fields to take.

use super::csl::{clean_markup, date_parts, fields, normalize_doi, person};
use super::tools::{title_similarity, SAME_WORK};
use serde::Serialize;
use serde_json::Value;
use std::io::Read;
use std::time::Duration;

const HOST: &str = "api.crossref.org";
const OPENALEX: &str = "api.openalex.org";
const MAX_BODY: u64 = 2 * 1024 * 1024;
const CANDIDATES: usize = 3;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Candidate {
    pub csl: Value,
    pub source: &'static str,
}

/// Percent-encodes `text` for a URL path or query value (a DOI keeps its `/`).
fn encode(text: &str, keep_slash: bool) -> String {
    let mut out = String::new();
    for b in text.bytes() {
        let plain = b.is_ascii_alphanumeric()
            || b"-._~".contains(&b)
            || (keep_slash && b == b'/')
            || (keep_slash && b"()".contains(&b));
        if plain {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

pub fn doi_url(doi: &str) -> String {
    format!(
        "https://{HOST}/works/{}/transform/application/vnd.citationstyles.csl+json",
        encode(&normalize_doi(doi), true)
    )
}

pub fn search_url(title: &str, first_author: &str) -> String {
    let mut url = format!(
        "https://{HOST}/works?rows={CANDIDATES}&select=DOI&query.bibliographic={}",
        encode(title, false)
    );
    if !first_author.is_empty() {
        url.push_str(&format!("&query.author={}", encode(first_author, false)));
    }
    url
}

pub fn openalex_doi_url(doi: &str) -> String {
    format!("https://{OPENALEX}/works/doi:{}", encode(&normalize_doi(doi), true))
}

pub fn openalex_search_url(title: &str) -> String {
    format!("https://{OPENALEX}/works?per-page={CANDIDATES}&search={}", encode(title, false))
}

/// One GET, pinned to the metadata hosts.
fn get(url: &str) -> Result<String, String> {
    let parsed = url::Url::parse(url).map_err(|e| e.to_string())?;
    if parsed.scheme() != "https" || !matches!(parsed.host_str(), Some(HOST | OPENALEX)) {
        return Err("only the Crossref and OpenAlex metadata services may be asked".into());
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("NPi-Desktop (Librarian metadata lookup)")
        .build()
        .map_err(|e| e.to_string())?;
    let response = client
        .get(url)
        .send()
        .map_err(|_| "could not reach the metadata service (are you online?)".to_string())?;
    if response.status().as_u16() == 404 {
        return Err("the metadata service has no record for that".into());
    }
    if !response.status().is_success() {
        return Err(format!("the metadata service answered {}", response.status()));
    }
    let mut body = String::new();
    response
        .take(MAX_BODY + 1)
        .read_to_string(&mut body)
        .map_err(|_| "the metadata service sent something unreadable".to_string())?;
    if body.len() as u64 > MAX_BODY {
        return Err("the metadata service sent too much".into());
    }
    Ok(body)
}

/// A CSL-JSON record from Crossref's transform answer; the service's own `id` is dropped.
pub fn parse_record(body: &str) -> Result<Value, String> {
    let mut v: Value = serde_json::from_str(body).map_err(|_| "unreadable answer".to_string())?;
    let obj = v.as_object_mut().ok_or("unexpected answer")?;
    obj.remove("id");
    for key in ["title", "subtitle", "container-title", "abstract"] {
        if let Some(Value::String(text)) = obj.get(key) {
            let clean = super::csl::clean_markup(text);
            obj.insert(key.to_string(), Value::String(clean));
        }
    }
    if fields(&Value::Object(obj.clone())).title.is_empty() {
        return Err("the record has no title".into());
    }
    Ok(v)
}

/// The DOIs of a Crossref search answer.
pub fn parse_search(body: &str) -> Vec<String> {
    serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| {
            v.pointer("/message/items").and_then(Value::as_array).map(|items| {
                items
                    .iter()
                    .filter_map(|i| i.get("DOI").and_then(Value::as_str))
                    .map(str::to_string)
                    .collect()
            })
        })
        .unwrap_or_default()
}

/// OpenAlex words are stored as `word -> positions`; this puts the abstract back in reading order.
fn openalex_abstract(index: &Value) -> Option<String> {
    let mut words: Vec<(u64, &str)> = index
        .as_object()?
        .iter()
        .flat_map(|(word, at)| at.as_array().into_iter().flatten().filter_map(Value::as_u64).map(move |n| (n, word.as_str())))
        .collect();
    words.sort_unstable();
    (!words.is_empty()).then(|| words.iter().map(|(_, w)| *w).collect::<Vec<_>>().join(" "))
}

/// A CSL-JSON record from one OpenAlex work.
pub fn parse_openalex(work: &Value) -> Result<Value, String> {
    let text = |path: &str| work.pointer(path).and_then(Value::as_str).unwrap_or("").trim().to_string();
    let title = clean_markup(&text("/display_name"));
    if title.is_empty() {
        return Err("the record has no title".into());
    }
    let kind = match text("/type").as_str() {
        "book" | "monograph" | "edited-book" => "book",
        "book-chapter" => "chapter",
        "dissertation" => "thesis",
        "report" => "report",
        "dataset" => "dataset",
        "preprint" => "manuscript",
        _ => "article-journal",
    };
    let mut csl = serde_json::json!({ "type": kind, "title": title });
    let mut put = |key: &str, value: String| {
        if !value.is_empty() {
            csl[key] = Value::String(value);
        }
    };
    put("container-title", clean_markup(&text("/primary_location/source/display_name")));
    put("publisher", text("/primary_location/source/host_organization_name"));
    put("volume", text("/biblio/volume"));
    put("issue", text("/biblio/issue"));
    put("DOI", normalize_doi(&text("/doi")));
    put("page", match (text("/biblio/first_page"), text("/biblio/last_page")) {
        (first, last) if last.is_empty() || first == last => first,
        (first, last) => format!("{first}-{last}"),
    });
    put("abstract", work.get("abstract_inverted_index").and_then(openalex_abstract).unwrap_or_default());
    if let Some(year) = work.get("publication_year").and_then(Value::as_i64).and_then(|y| i32::try_from(y).ok()) {
        csl["issued"] = date_parts(year);
    }
    let authors: Vec<Value> = work
        .get("authorships")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|a| a.pointer("/author/display_name").and_then(Value::as_str))
        .map(person)
        .collect();
    if !authors.is_empty() {
        csl["author"] = Value::Array(authors);
    }
    Ok(csl)
}

fn openalex_by_doi(doi: &str) -> Result<Vec<Candidate>, String> {
    let work: Value = serde_json::from_str(&get(&openalex_doi_url(doi))?).map_err(|_| "unreadable answer".to_string())?;
    Ok(vec![Candidate { csl: parse_openalex(&work)?, source: "openalex" }])
}

fn openalex_by_title(title: &str) -> Result<Vec<Candidate>, String> {
    let answer: Value = serde_json::from_str(&get(&openalex_search_url(title))?).map_err(|_| "unreadable answer".to_string())?;
    let works = answer.get("results").and_then(Value::as_array).cloned().unwrap_or_default();
    Ok(works.iter().filter_map(|w| parse_openalex(w).ok()).map(|csl| Candidate { csl, source: "openalex" }).collect())
}

/// Crossref's record, else OpenAlex's. When both fail the answer is Crossref's error.
pub fn by_doi(doi: &str) -> Result<Vec<Candidate>, String> {
    match get(&doi_url(doi)).and_then(|b| parse_record(&b)) {
        Ok(record) => Ok(vec![Candidate { csl: record, source: "crossref" }]),
        Err(first) => openalex_by_doi(doi).map_err(|_| first),
    }
}

fn crossref_by_title(title: &str, first_author: &str) -> Result<Vec<Candidate>, String> {
    let dois = parse_search(&get(&search_url(title, first_author))?);
    let mut out = Vec::new();
    for doi in dois.into_iter().take(CANDIDATES) {
        if let Ok(record) = get(&doi_url(&doi)).and_then(|b| parse_record(&b)) {
            out.push(Candidate { csl: record, source: "crossref" });
        }
    }
    Ok(out)
}

/// Crossref's search always answers with something, so its candidates count only if one has this title;
/// otherwise OpenAlex's come first and Crossref's stay after them.
pub fn by_title(title: &str, first_author: &str) -> Result<Vec<Candidate>, String> {
    let crossref = crossref_by_title(title, first_author);
    let found = crossref.as_deref().unwrap_or_default();
    if found.iter().any(|c| title_similarity(&fields(&c.csl).title, title) >= SAME_WORK) {
        return crossref;
    }
    let mut out = openalex_by_title(title).unwrap_or_default();
    out.extend_from_slice(found);
    out.truncate(CANDIDATES);
    if out.is_empty() {
        return Err(crossref.err().unwrap_or_else(|| "no matching record found".into()));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_name_only_the_service_and_encode_what_they_send() {
        assert_eq!(
            doi_url("https://doi.org/10.1000/ab c"),
            "https://api.crossref.org/works/10.1000/ab%20c/transform/application/vnd.citationstyles.csl+json"
        );
        let u = search_url("Deep & learning: 引用", "Smith");
        assert!(u.starts_with("https://api.crossref.org/works?rows=3"));
        assert!(u.contains("query.bibliographic=Deep%20%26%20learning%3A%20%E5%BC%95%E7%94%A8"));
        assert!(u.ends_with("&query.author=Smith"));
    }

    #[test]
    fn nothing_but_the_pinned_host_is_fetched() {
        assert!(get("http://api.crossref.org/works").is_err());
        assert!(get("https://example.com/works").is_err());
        assert!(get("https://api.crossref.org.evil.test/works").is_err());
        assert!(get("file:///etc/passwd").is_err());
    }

    #[test]
    fn openalex_is_a_pinned_host_too_and_its_work_becomes_a_record() {
        assert_eq!(openalex_doi_url("10.48550/arXiv.1706.03762"), "https://api.openalex.org/works/doi:10.48550/arxiv.1706.03762");
        assert!(openalex_search_url("a b").ends_with("per-page=3&search=a%20b"));
        assert!(get("https://api.openalex.org.evil.test/works").is_err());
        let work = serde_json::json!({
            "display_name": "Attention <i>is</i> all", "type": "preprint", "publication_year": 2017,
            "doi": "https://doi.org/10.48550/arxiv.1706.03762",
            "authorships": [{ "author": { "display_name": "Ashish Vaswani" } }],
            "primary_location": { "source": { "display_name": "arXiv" } },
            "biblio": { "first_page": "1", "last_page": "9", "volume": null },
            "abstract_inverted_index": { "world": [1], "hello": [0, 2] },
        });
        let csl = parse_openalex(&work).unwrap();
        assert_eq!(csl["title"], "Attention is all");
        assert_eq!(csl["type"], "manuscript");
        assert_eq!(csl["DOI"], "10.48550/arxiv.1706.03762");
        assert_eq!(csl["author"][0]["family"], "Vaswani");
        assert_eq!(csl["page"], "1-9");
        assert_eq!(csl["abstract"], "hello world hello");
        assert!(csl.get("volume").is_none());
        assert!(parse_openalex(&serde_json::json!({ "type": "book" })).is_err());
    }

    #[test]
    fn publisher_markup_is_taken_out_of_a_record() {
        let rec = parse_record(r#"{"title":"The <i>x</i> &amp; y","abstract":"<jats:p>Text.</jats:p>","type":"article-journal"}"#).unwrap();
        assert_eq!(rec["title"], "The x & y");
        assert_eq!(rec["abstract"], "Text.");
    }

    #[test]
    fn answers_are_parsed_into_records_and_dois() {
        let rec = parse_record(r#"{"id":"https://doi.org/10.1/x","type":"article-journal","title":"A title","DOI":"10.1/x"}"#).unwrap();
        assert_eq!(rec["title"], "A title");
        assert!(rec.get("id").is_none());
        assert!(parse_record(r#"{"type":"book"}"#).is_err());
        assert!(parse_record("not json").is_err());
        assert!(parse_record("[1]").is_err());
        assert_eq!(parse_search(r#"{"message":{"items":[{"DOI":"10.1/a"},{"x":1},{"DOI":"10.1/b"}]}}"#), ["10.1/a", "10.1/b"]);
        assert!(parse_search("garbage").is_empty());
    }
}
