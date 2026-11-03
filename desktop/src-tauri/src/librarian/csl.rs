//! CSL-JSON, the library's one record format: a `serde_json::Value` object
//! (`type`, `title`, `author`, `issued`, `DOI`, ...). The few fields the
//! library indexes and compares are read here; everything else is carried
//! through untouched so export gives back what import took in.

use serde_json::{json, Value};

/// What the library indexes of one record.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Fields {
    pub title: String,
    pub first_author: String,
    pub year: Option<i32>,
    pub doi: Option<String>,
    pub kind: String,
}

fn text<'a>(csl: &'a Value, key: &str) -> &'a str {
    csl.get(key).and_then(Value::as_str).unwrap_or("").trim()
}

/// The family name of the first author, else its literal name.
fn first_author(csl: &Value) -> String {
    let Some(first) = csl.get("author").and_then(|a| a.get(0)) else {
        return String::new();
    };
    ["family", "literal", "given"]
        .iter()
        .find_map(|k| {
            first
                .get(k)
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty())
        })
        .unwrap_or("")
        .to_string()
}

/// The year of `issued` (`{"date-parts": [[2020, 5]]}`, or a `raw` string
/// starting with a 4-digit year).
fn year(csl: &Value) -> Option<i32> {
    let issued = csl.get("issued")?;
    if let Some(y) = issued.pointer("/date-parts/0/0") {
        return y
            .as_i64()
            .and_then(|y| i32::try_from(y).ok())
            .or_else(|| y.as_str()?.trim().parse().ok());
    }
    let raw = issued.get("raw")?.as_str()?.trim();
    let digits: String = raw.chars().take_while(char::is_ascii_digit).collect();
    (digits.len() == 4).then(|| digits.parse().ok()).flatten()
}

pub fn fields(csl: &Value) -> Fields {
    let doi = normalize_doi(text(csl, "DOI"));
    Fields {
        title: text(csl, "title").to_string(),
        first_author: first_author(csl),
        year: year(csl),
        doi: (!doi.is_empty()).then_some(doi),
        kind: match text(csl, "type") {
            "" => "document".to_string(),
            kind => kind.to_string(),
        },
    }
}

/// A DOI as compared: lower case, without a `doi:` or resolver prefix.
pub fn normalize_doi(doi: &str) -> String {
    let doi = doi.trim().to_lowercase();
    let doi = doi.strip_prefix("doi:").unwrap_or(&doi).trim();
    [
        "https://doi.org/",
        "http://doi.org/",
        "https://dx.doi.org/",
        "http://dx.doi.org/",
    ]
    .iter()
    .find_map(|p| doi.strip_prefix(p))
    .unwrap_or(doi)
    .trim()
    .to_string()
}

/// A title as compared: lower case letters and digits of any script, one
/// space between words.
pub fn normalize_title(title: &str) -> String {
    title
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// `{"family": .., "given": ..}` for a name written `Family, Given` or
/// `Given Family`; a name without either shape is kept as a `literal`.
pub fn person(name: &str) -> Value {
    let name = name.trim();
    if let Some((family, given)) = name.split_once(',') {
        return json!({ "family": family.trim(), "given": given.trim() });
    }
    match name.rsplit_once(' ') {
        Some((given, family)) => json!({ "family": family.trim(), "given": given.trim() }),
        None => json!({ "literal": name }),
    }
}

/// `Family, Given` (or the literal) for writing a name back out.
pub fn person_text(person: &Value) -> String {
    let get = |k: &str| person.get(k).and_then(Value::as_str).unwrap_or("").trim();
    match (get("family"), get("given")) {
        ("", "") => get("literal").to_string(),
        (family, "") => family.to_string(),
        ("", given) => given.to_string(),
        (family, given) => format!("{family}, {given}"),
    }
}

pub fn date_parts(year: i32) -> Value {
    json!({ "date-parts": [[year]] })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_read_the_indexed_values() {
        let csl = json!({
            "type": "article-journal", "title": " Attention ",
            "author": [{ "family": "Vaswani", "given": "A." }],
            "issued": { "date-parts": [[2017, 6]] }, "DOI": "https://doi.org/10.5555/ABC",
        });
        assert_eq!(
            fields(&csl),
            Fields {
                title: "Attention".into(),
                first_author: "Vaswani".into(),
                year: Some(2017),
                doi: Some("10.5555/abc".into()),
                kind: "article-journal".into(),
            }
        );
    }

    #[test]
    fn a_record_without_those_values_still_has_fields() {
        let f =
            fields(&json!({ "issued": { "raw": "2019-03-04" }, "author": [{ "literal": "WHO" }] }));
        assert_eq!(
            (f.year, f.first_author.as_str(), f.kind.as_str()),
            (Some(2019), "WHO", "document")
        );
        assert_eq!(f.doi, None);
        assert_eq!(fields(&json!({ "issued": { "raw": "n.d." } })).year, None);
    }

    #[test]
    fn dois_and_titles_compare_after_normalising() {
        assert_eq!(normalize_doi(" doi:10.1/X "), "10.1/x");
        assert_eq!(normalize_doi("http://dx.doi.org/10.1/X"), "10.1/x");
        assert_eq!(
            normalize_title("Deep  Learning: A Survey!"),
            "deep learning a survey"
        );
        assert_eq!(normalize_title("深度学习：综述"), "深度学习 综述");
    }

    #[test]
    fn names_go_both_ways() {
        assert_eq!(
            person("Hinton, Geoffrey E."),
            json!({ "family": "Hinton", "given": "Geoffrey E." })
        );
        assert_eq!(
            person("Yann LeCun"),
            json!({ "family": "LeCun", "given": "Yann" })
        );
        assert_eq!(person("UNESCO"), json!({ "literal": "UNESCO" }));
        assert_eq!(person_text(&person("Yann LeCun")), "LeCun, Yann");
        assert_eq!(person_text(&json!({ "literal": "UNESCO" })), "UNESCO");
    }
}
