//! Moving references in and out of a library as text: RIS, BibTeX or CSL-JSON.

use super::{bibtex, csl, ris, Entry, Library};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Format {
    Ris,
    Bibtex,
    CslJson,
}

impl Format {
    const fn source(self) -> &'static str {
        match self {
            Self::Ris => "ris",
            Self::Bibtex => "bibtex",
            Self::CslJson => "csl-json",
        }
    }
}

/// A record of an import that repeated one already in the library.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Duplicate {
    pub title: String,
    /// The id of the reference it repeats.
    pub of: i64,
}

#[derive(Debug, Default, Serialize)]
pub struct ImportReport {
    pub added: Vec<Entry>,
    pub duplicates: Vec<Duplicate>,
}

/// CSL-JSON: an array of records, `{"items": [...]}`, or one record.
fn parse_csl_json(text: &str) -> Result<Vec<Value>, String> {
    let value: Value = serde_json::from_str(text.trim_start_matches('\u{feff}'))
        .map_err(|e| format!("not CSL-JSON: {e}"))?;
    let list = match value {
        Value::Array(list) => list,
        Value::Object(mut map) if map.get("items").is_some_and(Value::is_array) => {
            match map.remove("items") {
                Some(Value::Array(list)) => list,
                _ => Vec::new(),
            }
        }
        object @ Value::Object(_) => vec![object],
        _ => Vec::new(),
    };
    Ok(list.into_iter().filter(Value::is_object).collect())
}

pub fn parse(format: Format, text: &str) -> Result<Vec<Value>, String> {
    let records = match format {
        Format::Ris => ris::parse(text),
        Format::Bibtex => bibtex::parse(text),
        Format::CslJson => parse_csl_json(text)?,
    };
    if records.is_empty() {
        return Err("no references found in that text".to_string());
    }
    Ok(records)
}

/// Adds every record of `text` that is not already in the library; the ones
/// that are come back as duplicates, for the user to decide.
pub fn import(library: &Library, format: Format, text: &str) -> Result<ImportReport, String> {
    let mut report = ImportReport::default();
    for record in parse(format, text)? {
        match library.duplicate_of(&record)? {
            Some(existing) => report.duplicates.push(Duplicate {
                title: csl::fields(&record).title,
                of: existing.id,
            }),
            None => report.added.push(library.add(&record, format.source())?),
        }
    }
    Ok(report)
}

/// `entries` as text, each under its library citekey.
pub fn export(format: Format, entries: &[Entry]) -> String {
    match format {
        Format::Ris => ris::write(&entries.iter().map(|e| e.csl.clone()).collect::<Vec<_>>()),
        Format::Bibtex => bibtex::write(
            &entries
                .iter()
                .map(|e| (e.citekey.clone(), e.csl.clone()))
                .collect::<Vec<_>>(),
        ),
        Format::CslJson => {
            let items: Vec<Value> = entries
                .iter()
                .map(|e| {
                    let mut item = e.csl.clone();
                    item["id"] = Value::String(e.citekey.clone());
                    item
                })
                .collect();
            serde_json::to_string_pretty(&items).unwrap_or_else(|_| "[]".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    const BIB: &str = "@article{a, author = {Smith, Jo}, title = {Graphs}, year = {2020}, doi = {10.1/x}}\n@book{b, author = {Lee, Al}, title = {Trees}, year = {2018}}\n";

    #[test]
    fn importing_twice_adds_once_and_reports_the_repeats() {
        let dir = TempDir::new("librarian", "import-twice");
        let lib = Library::open(dir.path()).unwrap();
        let first = import(&lib, Format::Bibtex, BIB).unwrap();
        assert_eq!(
            first
                .added
                .iter()
                .map(|e| e.citekey.as_str())
                .collect::<Vec<_>>(),
            ["smith2020graphs", "lee2018trees"]
        );
        let again = import(&lib, Format::Bibtex, BIB).unwrap();
        assert_eq!(again.added.len(), 0);
        assert_eq!(
            again
                .duplicates
                .iter()
                .map(|d| (d.title.as_str(), d.of))
                .collect::<Vec<_>>(),
            [("Graphs", 1), ("Trees", 2)]
        );
        assert_eq!(lib.list().unwrap().len(), 2);
    }

    #[test]
    fn a_repeat_inside_one_file_is_caught_too() {
        let dir = TempDir::new("librarian", "repeat-in-file");
        let lib = Library::open(dir.path()).unwrap();
        let report = import(&lib, Format::Bibtex, &format!("{BIB}{BIB}")).unwrap();
        assert_eq!((report.added.len(), report.duplicates.len()), (2, 2));
    }

    #[test]
    fn every_format_leaves_and_re_enters_the_library_unchanged() {
        for format in [Format::Ris, Format::Bibtex, Format::CslJson] {
            let dir = TempDir::new("librarian", &format!("roundtrip-{format:?}"));
            let lib = Library::open(&dir.path().join("a")).unwrap();
            import(&lib, Format::Bibtex, BIB).unwrap();
            let text = export(format, &lib.list().unwrap());
            let other = Library::open(&dir.path().join("b")).unwrap();
            let report = import(&other, format, &text).unwrap();
            assert_eq!(report.added.len(), 2, "{format:?}");
            let strip = |entries: Vec<Entry>| {
                entries
                    .into_iter()
                    .map(|e| (e.citekey, e.csl["title"].clone(), e.csl["issued"].clone()))
                    .collect::<Vec<_>>()
            };
            assert_eq!(
                strip(other.list().unwrap()),
                strip(lib.list().unwrap()),
                "{format:?}"
            );
        }
    }

    #[test]
    fn csl_json_takes_an_array_an_items_object_or_one_record() {
        assert_eq!(
            parse(Format::CslJson, r#"[{"title":"A"},3,{"title":"B"}]"#)
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            parse(Format::CslJson, r#"{"items":[{"title":"A"}]}"#)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(parse(Format::CslJson, r#"{"title":"A"}"#).unwrap().len(), 1);
        assert!(parse(Format::CslJson, "nope")
            .unwrap_err()
            .contains("CSL-JSON"));
        assert!(parse(Format::Ris, "nothing here").is_err());
    }
}
