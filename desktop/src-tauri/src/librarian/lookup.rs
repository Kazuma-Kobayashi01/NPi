//! Metadata lookup (L3), started only by a click in the reference form. The
//! one host it may reach is `api.crossref.org`, over HTTPS, with no redirects;
//! what leaves the machine is a DOI, or a title (and a first author's family
//! name) — never a PDF or any other part of the library. Results are only
//! candidates: the webview shows them against the form and the user picks
//! which fields to take.

use super::csl::{fields, normalize_doi};
use serde::Serialize;
use serde_json::Value;
use std::io::Read;
use std::time::Duration;

const HOST: &str = "api.crossref.org";
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

/// One GET, pinned to the metadata host.
fn get(url: &str) -> Result<String, String> {
    let parsed = url::Url::parse(url).map_err(|e| e.to_string())?;
    if parsed.scheme() != "https" || parsed.host_str() != Some(HOST) {
        return Err("only the Crossref metadata service may be asked".into());
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

pub fn by_doi(doi: &str) -> Result<Vec<Candidate>, String> {
    let record = parse_record(&get(&doi_url(doi))?)?;
    Ok(vec![Candidate { csl: record, source: "crossref" }])
}

pub fn by_title(title: &str, first_author: &str) -> Result<Vec<Candidate>, String> {
    let dois = parse_search(&get(&search_url(title, first_author))?);
    let mut out = Vec::new();
    for doi in dois.into_iter().take(CANDIDATES) {
        if let Ok(record) = get(&doi_url(&doi)).and_then(|b| parse_record(&b)) {
            out.push(Candidate { csl: record, source: "crossref" });
        }
    }
    if out.is_empty() {
        return Err("no matching record found".into());
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
