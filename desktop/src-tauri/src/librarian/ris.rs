//! RIS (`TY  - JOUR` ... `ER  - `), the plain-text format most databases export.

use super::csl::{date_parts, person, person_text};
use serde_json::{json, Map, Value};
use std::fmt::Write as _;

const TYPES: &[(&str, &str)] = &[
    ("JOUR", "article-journal"),
    ("BOOK", "book"),
    ("CHAP", "chapter"),
    ("CONF", "paper-conference"),
    ("THES", "thesis"),
    ("RPRT", "report"),
    ("ELEC", "webpage"),
    ("MGZN", "article-magazine"),
    ("NEWS", "article-newspaper"),
    ("UNPB", "manuscript"),
    ("GEN", "document"),
];

/// `(tag, value)` of a `XX  - value` line.
fn tag_line(line: &str) -> Option<(&str, &str)> {
    let tag = line
        .get(..2)
        .filter(|t| t.chars().all(|c| c.is_ascii_alphanumeric()))?;
    (line.get(2..5)? == "  -").then(|| (tag, line[5..].trim()))
}

fn set(map: &mut Map<String, Value>, key: &str, value: &str) {
    if !value.is_empty() && !map.contains_key(key) {
        map.insert(key.to_string(), json!(value));
    }
}

fn push(map: &mut Map<String, Value>, key: &str, value: Value) {
    if let Some(list) = map.entry(key).or_insert_with(|| json!([])).as_array_mut() {
        list.push(value);
    }
}

fn year_of(text: &str) -> Option<i32> {
    let digits: String = text.chars().take_while(char::is_ascii_digit).collect();
    (digits.len() == 4).then(|| digits.parse().ok()).flatten()
}

fn record(fields: &[(String, String)]) -> Option<Value> {
    let mut out = Map::new();
    let (mut start, mut end, mut py, mut da) = ("", "", "", "");
    let mut kind = "document";
    for (tag, value) in fields {
        let value = value.as_str();
        match tag.as_str() {
            "TY" => {
                kind = TYPES
                    .iter()
                    .find(|(t, _)| *t == value)
                    .map_or("document", |(_, k)| k);
            }
            "AU" | "A1" => push(&mut out, "author", person(value)),
            "A2" | "ED" => push(&mut out, "editor", person(value)),
            "TI" | "T1" => set(&mut out, "title", value),
            "T2" | "JO" | "JF" | "BT" => set(&mut out, "container-title", value),
            "JA" => set(&mut out, "container-title-short", value),
            "PY" | "Y1" if py.is_empty() => py = value,
            "DA" if da.is_empty() => da = value,
            "VL" => set(&mut out, "volume", value),
            "IS" => set(&mut out, "issue", value),
            "SP" => start = value,
            "EP" => end = value,
            "DO" => set(&mut out, "DOI", value),
            "UR" => set(&mut out, "URL", value),
            "AB" | "N2" => set(&mut out, "abstract", value),
            "KW" => push(&mut out, "keyword", json!(value)),
            "PB" => set(&mut out, "publisher", value),
            "CY" => set(&mut out, "publisher-place", value),
            "SN" => set(
                &mut out,
                if kind == "book" { "ISBN" } else { "ISSN" },
                value,
            ),
            "LA" => set(&mut out, "language", value),
            "N1" => set(&mut out, "note", value),
            _ => {}
        }
    }
    if let Some(year) = year_of(py).or_else(|| year_of(da)) {
        out.insert("issued".to_string(), date_parts(year));
    }
    match (start, end) {
        ("", _) => {}
        (s, "") => set(&mut out, "page", s),
        (s, e) => set(&mut out, "page", &format!("{s}-{e}")),
    }
    if let Some(Value::Array(words)) = out.remove("keyword") {
        let joined: Vec<&str> = words.iter().filter_map(Value::as_str).collect();
        set(&mut out, "keyword", &joined.join(", "));
    }
    out.insert("type".to_string(), json!(kind));
    (out.contains_key("title") || out.contains_key("DOI")).then_some(Value::Object(out))
}

/// The records of a RIS text. Lines without a tag continue the previous value.
pub fn parse(text: &str) -> Vec<Value> {
    let mut records = Vec::new();
    let mut fields: Vec<(String, String)> = Vec::new();
    let mut flush = |fields: &mut Vec<(String, String)>| {
        if let Some(v) = record(fields) {
            records.push(v);
        }
        fields.clear();
    };
    for line in text.trim_start_matches('\u{feff}').lines() {
        match tag_line(line) {
            Some(("ER", _)) => flush(&mut fields),
            Some(("TY", value)) => {
                flush(&mut fields);
                fields.push(("TY".to_string(), value.to_string()));
            }
            Some((tag, value)) => fields.push((tag.to_string(), value.to_string())),
            None => {
                if let (Some(last), false) = (fields.last_mut(), line.trim().is_empty()) {
                    last.1.push(' ');
                    last.1.push_str(line.trim());
                }
            }
        }
    }
    flush(&mut fields);
    records
}

fn line(out: &mut String, tag: &str, value: &str) {
    if !value.trim().is_empty() {
        let _ = writeln!(out, "{tag}  - {}", value.trim().replace('\n', " "));
    }
}

/// RIS text for `records`.
pub fn write(records: &[Value]) -> String {
    let mut out = String::new();
    for csl in records {
        let get = |k: &str| csl.get(k).and_then(Value::as_str).unwrap_or("");
        let kind = get("type");
        let ty = TYPES
            .iter()
            .find(|(_, k)| *k == kind)
            .map_or("GEN", |(t, _)| t);
        line(&mut out, "TY", ty);
        for (key, tag) in [("author", "AU"), ("editor", "ED")] {
            for p in csl.get(key).and_then(Value::as_array).into_iter().flatten() {
                line(&mut out, tag, &person_text(p));
            }
        }
        line(&mut out, "TI", get("title"));
        line(
            &mut out,
            if ty == "JOUR" { "JO" } else { "T2" },
            get("container-title"),
        );
        if let Some(y) = csl
            .pointer("/issued/date-parts/0/0")
            .and_then(Value::as_i64)
        {
            line(&mut out, "PY", &y.to_string());
        }
        line(&mut out, "VL", get("volume"));
        line(&mut out, "IS", get("issue"));
        let page = get("page");
        let (start, end) = page.split_once(['-', '–']).unwrap_or((page, ""));
        line(&mut out, "SP", start);
        line(&mut out, "EP", end.trim_start_matches(['-', '–']));
        for (key, tag) in [
            ("DOI", "DO"),
            ("URL", "UR"),
            ("abstract", "AB"),
            ("publisher", "PB"),
            ("publisher-place", "CY"),
            ("language", "LA"),
            ("note", "N1"),
        ] {
            line(&mut out, tag, get(key));
        }
        line(
            &mut out,
            "SN",
            if kind == "book" {
                get("ISBN")
            } else {
                get("ISSN")
            },
        );
        for word in get("keyword").split([',', ';']) {
            line(&mut out, "KW", word);
        }
        out.push_str("ER  - \n\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\u{feff}TY  - JOUR\nAU  - Vaswani, Ashish\nAU  - Noam Shazeer\nTI  - Attention is\n  all you need\nJO  - NeurIPS\nPY  - 2017/06/12/\nVL  - 30\nSP  - 5998\nEP  - 6008\nDO  - 10.5555/3295222\nKW  - attention\nKW  - transformer\nER  - \n\nTY  - BOOK\nTI  - Deep Learning\nAU  - Goodfellow, Ian\nPY  - 2016\nSN  - 978-0262035613\nER  - \n";

    #[test]
    fn records_are_read_with_their_fields_and_wrapped_lines() {
        let records = parse(SAMPLE);
        assert_eq!(records.len(), 2);
        let a = &records[0];
        assert_eq!(a["type"], "article-journal");
        assert_eq!(a["title"], "Attention is all you need");
        assert_eq!(
            a["author"],
            json!([{ "family": "Vaswani", "given": "Ashish" }, { "family": "Shazeer", "given": "Noam" }])
        );
        assert_eq!(
            (
                a["issued"].clone(),
                a["page"].as_str(),
                a["keyword"].as_str()
            ),
            (
                json!({ "date-parts": [[2017]] }),
                Some("5998-6008"),
                Some("attention, transformer")
            )
        );
        assert_eq!(
            (records[1]["type"].as_str(), records[1]["ISBN"].as_str()),
            (Some("book"), Some("978-0262035613"))
        );
    }

    #[test]
    fn what_is_written_reads_back_the_same() {
        let records = parse(SAMPLE);
        assert_eq!(parse(&write(&records)), records);
    }

    #[test]
    fn text_that_is_not_ris_gives_no_records() {
        assert_eq!(parse("hello\nworld").len(), 0);
        assert_eq!(parse("TY  - JOUR\nER  - \n").len(), 0);
    }
}
