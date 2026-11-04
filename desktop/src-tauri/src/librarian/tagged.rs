//! Reading what databases other than RIS's export: PubMed's `.nbib` (MEDLINE tags), Web of Science's
//! plain text and EndNote's XML. All three are read only: the library writes RIS, BibTeX and CSL-JSON.

use super::csl::{clean_markup, date_parts, person};
use serde_json::{json, Map, Value};

fn set(out: &mut Map<String, Value>, key: &str, value: &str) {
    let value = value.trim();
    if !value.is_empty() && !out.contains_key(key) {
        out.insert(key.to_string(), json!(value));
    }
}

fn set_year(out: &mut Map<String, Value>, text: &str) {
    let found = text
        .split(|c: char| !c.is_ascii_digit())
        .find(|part| part.len() == 4)
        .and_then(|part| part.parse().ok());
    if let Some(year) = found {
        out.insert("issued".to_string(), date_parts(year));
    }
}

fn finish(mut out: Map<String, Value>, kind: &str, authors: Vec<Value>) -> Option<Value> {
    if !authors.is_empty() {
        out.insert("author".to_string(), Value::Array(authors));
    }
    out.insert("type".to_string(), json!(kind));
    (out.contains_key("title") || out.contains_key("DOI")).then_some(Value::Object(out))
}

/// `Smith J`: PubMed's short form, where the initials come last.
fn short_person(name: &str) -> Value {
    match name.trim().rsplit_once(' ') {
        Some((family, given)) => json!({ "family": family.trim(), "given": given.trim() }),
        None => json!({ "literal": name.trim() }),
    }
}

/// `(tag, value)` of a MEDLINE line `TAG - value` (the tag is padded to four characters).
fn nbib_line(line: &str) -> Option<(&str, &str)> {
    let (tag, value) = (line.get(..4)?, line.get(4..)?);
    let tag = tag.trim_end();
    let tagged = !tag.is_empty() && tag.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit());
    (tagged && (value.starts_with("- ") || value == "-")).then(|| (tag, value[1..].trim()))
}

fn nbib_record(fields: &[(String, String)]) -> Option<Value> {
    let mut out = Map::new();
    let (mut full, mut short) = (Vec::new(), Vec::new());
    let mut kind = "article-journal";
    for (tag, value) in fields {
        let value = value.as_str();
        match tag.as_str() {
            "TI" | "BTI" => set(&mut out, "title", value),
            "FAU" => full.push(person(value)),
            "AU" => short.push(short_person(value)),
            "JT" | "TA" => set(&mut out, "container-title", value),
            "DP" => set_year(&mut out, value),
            "VI" => set(&mut out, "volume", value),
            "IP" => set(&mut out, "issue", value),
            "PG" => set(&mut out, "page", value),
            "AB" => set(&mut out, "abstract", value),
            "LA" => set(&mut out, "language", value),
            "PB" => set(&mut out, "publisher", value),
            "PMID" => set(&mut out, "PMID", value),
            "LID" | "AID" if value.ends_with("[doi]") => set(&mut out, "DOI", value.trim_end_matches("[doi]")),
            "PT" if value.eq_ignore_ascii_case("Book") => kind = "book",
            _ => {}
        }
    }
    finish(out, kind, if full.is_empty() { short } else { full })
}

/// The records of a PubMed `.nbib` text. A line led by six spaces continues the value above it.
pub fn parse_nbib(text: &str) -> Vec<Value> {
    let mut records = Vec::new();
    let mut fields: Vec<(String, String)> = Vec::new();
    for line in text.trim_start_matches('\u{feff}').lines() {
        if let Some((tag, value)) = nbib_line(line) {
            if tag == "PMID" && !fields.is_empty() {
                records.extend(nbib_record(&fields));
                fields.clear();
            }
            fields.push((tag.to_string(), value.to_string()));
        } else if line.starts_with(' ') {
            if let Some(last) = fields.last_mut() {
                last.1.push(' ');
                last.1.push_str(line.trim());
            }
        }
    }
    records.extend(nbib_record(&fields));
    records
}

fn wos_kind(pt: &str, dt: &str) -> &'static str {
    match (pt, dt.to_ascii_lowercase()) {
        ("B", _) => "book",
        (_, dt) if dt.contains("proceedings") => "paper-conference",
        ("S", _) => "chapter",
        _ => "article-journal",
    }
}

fn wos_record(fields: &[(String, Vec<String>)]) -> Option<Value> {
    let get = |tag: &str| fields.iter().find(|(t, _)| t == tag).map(|(_, v)| v.as_slice());
    let joined = |tag: &str| get(tag).map(|v| v.join(" ")).unwrap_or_default();
    let mut out = Map::new();
    set(&mut out, "title", &joined("TI"));
    set(&mut out, "container-title", &joined("SO"));
    set(&mut out, "abstract", &joined("AB"));
    set(&mut out, "volume", &joined("VL"));
    set(&mut out, "issue", &joined("IS"));
    set(&mut out, "DOI", &joined("DI"));
    set(&mut out, "publisher", &joined("PU"));
    set(&mut out, "keyword", &joined("DE"));
    set(&mut out, "language", &joined("LA"));
    set_year(&mut out, &joined("PY"));
    match (joined("BP"), joined("EP")) {
        (start, _) if start.is_empty() => {}
        (start, end) if end.is_empty() => set(&mut out, "page", &start),
        (start, end) => set(&mut out, "page", &format!("{start}-{end}")),
    }
    let names = get("AF").or_else(|| get("AU")).unwrap_or_default();
    finish(out, wos_kind(&joined("PT"), &joined("DT")), names.iter().map(|n| person(n)).collect())
}

/// The records of a Web of Science plain-text export: `XX value`, three spaces continue it, `ER` ends a record.
pub fn parse_wos(text: &str) -> Vec<Value> {
    let mut records = Vec::new();
    let mut fields: Vec<(String, Vec<String>)> = Vec::new();
    for line in text.trim_start_matches('\u{feff}').lines() {
        if let Some(more) = line.strip_prefix("   ") {
            if let Some((_, values)) = fields.last_mut() {
                values.push(more.trim().to_string());
            }
            continue;
        }
        let tag = line.get(..2).filter(|t| t.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()));
        match tag {
            Some("ER") => {
                records.extend(wos_record(&fields));
                fields.clear();
            }
            Some("FN" | "VR" | "EF") => {}
            Some(tag) if line.len() == 2 || line.as_bytes()[2] == b' ' => {
                fields.push((tag.to_string(), vec![line[2..].trim().to_string()]));
            }
            _ => {}
        }
    }
    records
}

/// The text inside each `<tag>…</tag>` of `xml`, markup and entities taken out. `<tagged>` is not `<tag>`.
fn elements(xml: &str, tag: &str) -> Vec<String> {
    let (open, close) = (format!("<{tag}"), format!("</{tag}>"));
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(at) = rest.find(&open) {
        let after = &rest[at + open.len()..];
        if !after.starts_with(['>', ' ']) {
            rest = after;
            continue;
        }
        let Some(body) = after.find('>').map(|end| &after[end + 1..]) else { break };
        let Some(end) = body.find(&close) else { break };
        out.push(clean_markup(&body[..end]));
        rest = &body[end + close.len()..];
    }
    out
}

fn first(xml: &str, tag: &str) -> String {
    elements(xml, tag).into_iter().next().unwrap_or_default()
}

fn endnote_kind(name: &str) -> &'static str {
    match name.to_ascii_lowercase().as_str() {
        "book" | "edited book" => "book",
        "book section" => "chapter",
        "conference paper" | "conference proceedings" => "paper-conference",
        "thesis" => "thesis",
        "report" | "government document" => "report",
        "web page" => "webpage",
        _ => "article-journal",
    }
}

fn endnote_record(xml: &str) -> Option<Value> {
    let mut out = Map::new();
    set(&mut out, "title", &first(xml, "title"));
    let container = first(xml, "secondary-title");
    set(&mut out, "container-title", if container.is_empty() { first(xml, "full-title") } else { container }.as_str());
    set(&mut out, "volume", &first(xml, "volume"));
    set(&mut out, "issue", &first(xml, "number"));
    set(&mut out, "page", &first(xml, "pages"));
    set(&mut out, "abstract", &first(xml, "abstract"));
    set(&mut out, "publisher", &first(xml, "publisher"));
    set(&mut out, "DOI", &first(xml, "electronic-resource-num"));
    set(&mut out, "URL", &first(xml, "url"));
    set(&mut out, "keyword", &elements(xml, "keyword").join(", "));
    set_year(&mut out, &first(xml, "year"));
    let kind_tag = xml.find("<ref-type").map(|at| &xml[at..]).unwrap_or_default();
    let kind = kind_tag.split('"').nth(1).unwrap_or_default();
    finish(out, endnote_kind(kind), elements(xml, "author").iter().map(|n| person(n)).collect())
}

/// The records of an EndNote XML export.
pub fn parse_endnote(text: &str) -> Vec<Value> {
    let mut records = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find("<record>") {
        let body = &rest[at + "<record>".len()..];
        let Some(end) = body.find("</record>") else { break };
        records.extend(endnote_record(&body[..end]));
        rest = &body[end..];
    }
    records
}

#[cfg(test)]
mod tests {
    use super::*;

    const NBIB: &str = "PMID- 111\nTI  - A long title\n      that wraps\nAB  - Words.\nFAU - Smith, Jo\nAU  - Smith J\nFAU - Lee, Al\nJT  - Nature\nDP  - 2020 Jan\nVI  - 12\nIP  - 3\nPG  - 1-9\nLID - 10.1/x [doi]\nPT  - Journal Article\n\nPMID- 222\nTI  - Second\nAU  - Wang F\nDP  - 2019\n";

    #[test]
    fn pubmed_records_are_read_with_their_wrapped_lines() {
        let list = parse_nbib(NBIB);
        assert_eq!(list.len(), 2);
        let a = &list[0];
        assert_eq!(a["title"], "A long title that wraps");
        assert_eq!(a["author"][1]["family"], "Lee");
        assert_eq!(a["DOI"], "10.1/x");
        assert_eq!(a["container-title"], "Nature");
        assert_eq!(a["issued"]["date-parts"][0][0], 2020);
        assert_eq!(list[1]["author"][0], json!({ "family": "Wang", "given": "F" }));
    }

    #[test]
    fn web_of_science_records_are_read() {
        let text = "FN Clarivate\nVR 1.0\nPT J\nAU Smith, J\n   Lee, A\nAF Smith, Jo\n   Lee, Al\nTI A title\n   goes on\nSO NATURE\nPY 2021\nVL 5\nBP 10\nEP 20\nDI 10.2/y\nER\n\nPT B\nTI A book\nPY 2000\nER\n\nEF\n";
        let list = parse_wos(text);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0]["title"], "A title goes on");
        assert_eq!(list[0]["author"][1]["given"], "Al");
        assert_eq!(list[0]["page"], "10-20");
        assert_eq!(list[1]["type"], "book");
    }

    #[test]
    fn endnote_xml_is_read_and_its_style_tags_and_entities_go() {
        let text = r#"<?xml version="1.0"?><xml><records><record><ref-type name="Journal Article">17</ref-type><contributors><authors><author><style face="normal">Smith, Jo</style></author><author>Lee, Al</author></authors></contributors><titles><title><style>Graphs &amp; trees</style></title><secondary-title>Nature</secondary-title></titles><pages>1-2</pages><volume>3</volume><dates><year>2018</year></dates><electronic-resource-num>10.3/z</electronic-resource-num><urls><related-urls><url>https://a.test</url></related-urls></urls></record><record><ref-type name="Thesis">32</ref-type><titles><title>Mine</title></titles></record></records></xml>"#;
        let list = parse_endnote(text);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0]["title"], "Graphs & trees");
        assert_eq!(list[0]["author"][0]["family"], "Smith");
        assert_eq!(list[0]["container-title"], "Nature");
        assert_eq!(list[0]["URL"], "https://a.test");
        assert_eq!(list[1]["type"], "thesis");
    }

    #[test]
    fn nothing_usable_gives_nothing() {
        assert!(parse_nbib("hello\nworld").is_empty());
        assert!(parse_wos("hello\nER\n").is_empty());
        assert!(parse_endnote("<xml></xml>").is_empty());
    }
}
