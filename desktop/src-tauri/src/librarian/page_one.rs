//! "Read page 1 with the model": for a reference whose PDF shows no DOI, page 1's text goes to the
//! model (a worker with no tools, `copilot_worker::convert`) and what it reads there comes back as a
//! record the Librarian window shows like an online lookup: the user ticks fields into the form and Save
//! writes them. It runs only on the user's click, after a confirmation saying what is sent; nothing calls
//! it on its own, and no agent tool reaches it.

use super::store::{Entry, Library};
use super::tools::{blank, person, LibraryTools};
use serde_json::{json, Map, Value};
use std::path::Path;

/// Page 1 text sent at most: the title block, authors and venue are at the top.
const MAX_CHARS: usize = 6_000;

/// What the model may fill. No abstract: a paraphrase would pass for the paper's own.
const FIELDS: &[&str] = &[
    "title",
    "author",
    "issued",
    "container-title",
    "DOI",
    "volume",
    "issue",
    "page",
    "publisher",
    "type",
];

const PROMPT: &str = "\
You read the first page of a scholarly PDF and give its bibliographic record. Answer with one JSON object and nothing else, using only these keys: \
title, author (a list of \"Family, Given\" strings in the order printed), issued (the four-digit year), container-title (journal, book or conference), \
DOI, volume, issue, page, publisher, type (a CSL type such as article-journal, paper-conference, chapter, book, report, thesis). \
Leave out every key the page does not show. Copy what is printed; never guess, translate or complete a value from memory.";

/// The fields of `entry` the model's record would fill, `[{field, from, to}]`, and that record.
pub struct Proposal {
    pub changes: Vec<Value>,
    pub csl: Value,
}

/// The PDF of `entry` to read: its first attachment that is a file under `papers/`.
fn pdf_of(root: &Path, entry: &Entry) -> Result<std::path::PathBuf, String> {
    entry
        .attachments
        .iter()
        .find_map(|a| super::attach::open_target(root, &a.path).ok())
        .ok_or_else(|| "this reference has no PDF under papers/ to read".to_string())
}

/// The model's answer as a CSL record of [`FIELDS`]; anything else in it is dropped.
fn parse(answer: &str) -> Result<Value, String> {
    let start = answer.find('{').ok_or("the model gave no record")?;
    let end = answer
        .rfind('}')
        .filter(|e| *e > start)
        .ok_or("the model gave no record")?;
    let raw: Value = serde_json::from_str(&answer[start..=end])
        .map_err(|_| "the model's record is not JSON".to_string())?;
    let mut csl = Map::new();
    for field in FIELDS {
        let Some(v) = raw.get(*field).filter(|v| !blank(Some(v))) else {
            continue;
        };
        let value = match *field {
            "author" => {
                let list = v.as_array().map_or_else(|| vec![v.clone()], Clone::clone);
                let people: Vec<Value> = list.iter().filter_map(|p| person(p).ok()).collect();
                if people.is_empty() {
                    continue;
                }
                Value::Array(people)
            }
            "issued" => {
                let year = v.as_i64().or_else(|| {
                    v.as_str()
                        .and_then(super::pdf_meta::find_year)
                        .map(i64::from)
                });
                match year.filter(|y| (1000..=2999).contains(y)) {
                    Some(y) => json!({ "date-parts": [[y]] }),
                    None => continue,
                }
            }
            // Only a DOI the page could hold, in its plain form.
            "DOI" => match v.as_str().and_then(super::pdf_meta::find_doi) {
                Some(doi) => Value::String(doi),
                None => continue,
            },
            _ => match v {
                Value::String(s) => Value::String(s.trim().to_string()),
                Value::Number(n) => Value::String(n.to_string()),
                _ => continue,
            },
        };
        csl.insert((*field).to_string(), value);
    }
    if csl.is_empty() {
        return Err("the model found nothing it could read on page 1".into());
    }
    Ok(Value::Object(csl))
}

/// The blank fields of `have` that `found` fills, as `{field, from, to}`.
fn fill(have: &Value, found: &Value) -> Vec<Value> {
    let mut changes = Vec::new();
    for field in FIELDS {
        let Some(new) = found.get(*field).filter(|v| !blank(Some(v))) else {
            continue;
        };
        let old = have.get(*field);
        // "document" is what a missing type reads as; a title is often only the file's name.
        let replaceable =
            blank(old) || (*field == "type" && old.and_then(Value::as_str) == Some("document"));
        if old == Some(new) || !replaceable {
            continue;
        }
        changes.push(
            json!({ "field": field, "from": old.cloned().unwrap_or(Value::Null), "to": new }),
        );
    }
    changes
}

fn find(lib: &Library, citekey: &str) -> Result<Entry, String> {
    LibraryTools::find(&lib.list()?, citekey).cloned()
}

/// Sends page 1 of `citekey`'s PDF to `model` (prompt, text → answer) and returns what it would fill.
pub fn propose(
    root: &Path,
    citekey: &str,
    model: impl Fn(&str, &str) -> Result<String, String>,
) -> Result<Proposal, String> {
    let lib = Library::open(root)?;
    let entry = find(&lib, citekey)?;
    let text =
        match crate::pdf_text::first_page(&pdf_of(root, &entry)?)? {
            crate::pdf_text::Extracted::Text(t) => t,
            crate::pdf_text::Extracted::Scanned => return Err(
                "page 1 has no text layer, so there is nothing to send; fill the fields in by hand"
                    .into(),
            ),
        };
    let text: String = text.chars().take(MAX_CHARS).collect();
    let csl = parse(&model(PROMPT, &text)?)?;
    let changes = fill(&entry.csl, &csl);
    if changes.is_empty() {
        return Err("the model read nothing on page 1 that the reference lacks".into());
    }
    Ok(Proposal { changes, csl })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_answer_keeps_only_known_fields_in_csl_form() {
        let csl = parse("Here it is:\n```json\n{\"title\": \" Graph nets \", \"author\": [\"Smith, Ann\", \"Bo Li\"], \"issued\": \"2021\", \"DOI\": \"https://doi.org/10.1000/XYZ.1\", \"abstract\": \"no\", \"volume\": 7}\n```").unwrap();
        assert_eq!(csl["title"], "Graph nets");
        assert_eq!(
            csl["author"][0],
            json!({ "family": "Smith", "given": "Ann" })
        );
        assert_eq!(csl["author"][1]["family"], "Li");
        assert_eq!(csl["issued"]["date-parts"][0][0], 2021);
        assert!(csl["DOI"].as_str().unwrap().starts_with("10.1000/"));
        assert_eq!(csl["volume"], "7");
        assert!(csl.get("abstract").is_none());
        assert!(parse("{\"DOI\": \"not a doi\"}").is_err());
        assert!(parse("sorry").is_err());
    }

    #[test]
    fn only_blank_fields_are_filled() {
        let have = json!({ "title": "Mine", "type": "document", "volume": "" });
        let found =
            json!({ "title": "Theirs", "type": "article-journal", "volume": "3", "page": "1-9" });
        let changes = fill(&have, &found);
        let fields: Vec<&str> = changes
            .iter()
            .map(|c| c["field"].as_str().unwrap())
            .collect();
        assert_eq!(fields, ["volume", "page", "type"]);
    }

    #[test]
    fn page_one_goes_to_the_model_and_nothing_is_written() {
        let d = crate::test_support::TempDir::new("librarian", "page-one");
        let root = super::super::tools::tests::project(&d);
        let pdf = root.join("papers/p.pdf");
        std::fs::write(
            &pdf,
            crate::pdf_text::tiny_pdf("A Study of Graphs by Ann Smith 2021"),
        )
        .unwrap();
        let lib = Library::open(&root).unwrap();
        let e = lib
            .add(&json!({ "type": "document", "title": "" }), "pdf")
            .unwrap();
        lib.attach(
            e.id,
            "papers/p.pdf",
            &super::super::attach::file_hash(&pdf).unwrap(),
            "pdf",
        )
        .unwrap();
        let seen = std::cell::RefCell::new(String::new());
        let p = propose(&root, &e.citekey, |_, text| {
            seen.replace(text.to_string());
            Ok("{\"title\": \"A Study of Graphs\", \"author\": [\"Smith, Ann\"], \"issued\": 2021}".into())
        })
        .unwrap();
        assert!(seen.borrow().contains("Graphs"));
        assert_eq!(p.changes.len(), 3);
        assert_eq!(p.csl["title"], "A Study of Graphs");
        assert_eq!(
            Library::open(&root).unwrap().list().unwrap()[1].csl["title"],
            "",
            "nothing is written"
        );
    }
}
