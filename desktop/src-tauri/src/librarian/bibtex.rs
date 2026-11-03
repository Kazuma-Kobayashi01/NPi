//! BibTeX / BibLaTeX (`@article{key, title = {..}, ...}`): the format LaTeX
//! users keep their references in. Reading handles nested braces, quoted
//! values, `@string` macros and `#` joins; accent commands lose the accent
//! mark but keep the letter, and other LaTeX is not interpreted.

use super::csl::{date_parts, person, person_text};
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::fmt::Write as _;

const TYPES: &[(&str, &str)] = &[
    ("article", "article-journal"),
    ("book", "book"),
    ("incollection", "chapter"),
    ("inbook", "chapter"),
    ("inproceedings", "paper-conference"),
    ("conference", "paper-conference"),
    ("phdthesis", "thesis"),
    ("mastersthesis", "thesis"),
    ("techreport", "report"),
    ("online", "webpage"),
    ("electronic", "webpage"),
    ("unpublished", "manuscript"),
    ("misc", "document"),
];

struct Reader {
    chars: Vec<char>,
    pos: usize,
    macros: HashMap<String, String>,
}

impl Reader {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn skip_space(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.pos += 1;
        }
    }

    fn word(&mut self, stop: &[char]) -> String {
        let start = self.pos;
        while self
            .peek()
            .is_some_and(|c| !c.is_whitespace() && !stop.contains(&c))
        {
            self.pos += 1;
        }
        self.chars[start..self.pos].iter().collect()
    }

    /// The text inside a `{..}` (the cursor on its `{`), nested braces kept.
    fn braced(&mut self) -> String {
        let (mut depth, mut out) = (0usize, String::new());
        while let Some(c) = self.peek() {
            self.pos += 1;
            match c {
                '{' => {
                    depth += 1;
                    if depth > 1 {
                        out.push(c);
                    }
                }
                '}' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        break;
                    }
                    out.push(c);
                }
                '\\' => {
                    out.push(c);
                    if let Some(next) = self.peek() {
                        out.push(next);
                        self.pos += 1;
                    }
                }
                _ => out.push(c),
            }
        }
        out
    }

    /// The text inside `".."` (the cursor on its first quote).
    fn quoted(&mut self) -> String {
        self.pos += 1;
        let (mut depth, mut out) = (0usize, String::new());
        while let Some(c) = self.peek() {
            self.pos += 1;
            match c {
                '"' if depth == 0 => break,
                '{' => depth += 1,
                '}' => depth = depth.saturating_sub(1),
                _ => {}
            }
            out.push(c);
        }
        out
    }

    /// A field value: parts of braces, quotes, numbers and macros joined by `#`.
    fn value(&mut self) -> String {
        let mut out = String::new();
        loop {
            self.skip_space();
            match self.peek() {
                Some('{') => out.push_str(&self.braced()),
                Some('"') => out.push_str(&self.quoted()),
                Some(_) => {
                    let w = self.word(&[',', '}', ')', '#']);
                    out.push_str(self.macros.get(&w.to_lowercase()).unwrap_or(&w));
                }
                None => break,
            }
            self.skip_space();
            if self.peek() == Some('#') {
                self.pos += 1;
            } else {
                break;
            }
        }
        out
    }

    /// `name = value` pairs up to the closing brace.
    fn fields(&mut self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        loop {
            self.skip_space();
            while self.peek() == Some(',') {
                self.pos += 1;
                self.skip_space();
            }
            match self.peek() {
                None => return out,
                Some('}' | ')') => {
                    self.pos += 1;
                    return out;
                }
                Some(_) => {}
            }
            let name = self.word(&['=', ',', '}']).to_lowercase();
            self.skip_space();
            if self.peek() == Some('=') {
                self.pos += 1;
                let value = self.value();
                out.push((name, value));
            }
        }
    }
}

/// Plain text of a BibTeX value: braces dropped, escapes undone, accents reduced to letters.
fn clean(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' | '}' => {}
            '~' => out.push(' '),
            '\\' => match chars.peek().copied() {
                Some(n @ ('&' | '%' | '_' | '#' | '$')) => {
                    out.push(n);
                    chars.next();
                }
                Some('\'' | '"' | '`' | '^' | '~' | '=' | '.') => {
                    chars.next();
                }
                _ => {}
            },
            _ => out.push(c),
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Names split at top-level ` and `; a name wholly in braces is an organisation.
fn names(text: &str) -> Vec<Value> {
    let (mut parts, mut depth, mut current) = (Vec::new(), 0usize, String::new());
    for word in text.split(' ') {
        if depth == 0 && word.eq_ignore_ascii_case("and") {
            parts.push(std::mem::take(&mut current));
            continue;
        }
        depth = (depth + word.matches('{').count()).saturating_sub(word.matches('}').count());
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }
    parts.push(current);
    parts
        .iter()
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .map(|p| {
            if p.starts_with('{') && p.ends_with('}') && !p[1..p.len() - 1].contains(['{', '}']) {
                json!({ "literal": clean(p) })
            } else {
                person(&clean(p))
            }
        })
        .collect()
}

fn record(kind: &str, fields: &[(String, String)]) -> Option<Value> {
    let mut out = Map::new();
    let kind = TYPES
        .iter()
        .find(|(t, _)| *t == kind)
        .map_or("document", |(_, k)| k);
    out.insert("type".to_string(), json!(kind));
    for (name, raw) in fields {
        let text = clean(raw);
        if text.is_empty() {
            continue;
        }
        let key = match name.as_str() {
            "author" | "editor" => {
                out.insert(
                    name.clone(),
                    Value::Array(names(&raw.split_whitespace().collect::<Vec<_>>().join(" "))),
                );
                continue;
            }
            "year" => {
                let digits: String = text.chars().take_while(char::is_ascii_digit).collect();
                if let (4, Ok(y)) = (digits.len(), digits.parse::<i32>()) {
                    out.insert("issued".to_string(), date_parts(y));
                }
                continue;
            }
            "pages" => {
                out.insert(
                    "page".to_string(),
                    json!(text.replace("--", "-").replace(['–', '—'], "-")),
                );
                continue;
            }
            "title" => "title",
            "journal" | "journaltitle" | "booktitle" => "container-title",
            "volume" => "volume",
            "number" | "issue" => "issue",
            "doi" => "DOI",
            "url" => "URL",
            "publisher" | "school" | "institution" => "publisher",
            "address" | "location" => "publisher-place",
            "abstract" => "abstract",
            "keywords" => "keyword",
            "isbn" => "ISBN",
            "issn" => "ISSN",
            "langid" | "language" => "language",
            "note" => "note",
            _ => continue,
        };
        out.entry(key).or_insert_with(|| json!(text));
    }
    (out.contains_key("title") || out.contains_key("DOI")).then_some(Value::Object(out))
}

/// The records of a BibTeX text; `@comment`, `@preamble` and text between entries are skipped.
pub fn parse(text: &str) -> Vec<Value> {
    let mut r = Reader {
        chars: text.chars().collect(),
        pos: 0,
        macros: HashMap::new(),
    };
    let mut records = Vec::new();
    while let Some(at) = r.chars[r.pos..].iter().position(|c| *c == '@') {
        r.pos += at + 1;
        let kind = r.word(&['{', '(']).to_lowercase();
        r.skip_space();
        if !matches!(r.peek(), Some('{' | '(')) {
            continue;
        }
        r.pos += 1;
        match kind.as_str() {
            "comment" | "preamble" => {
                r.pos -= 1;
                if r.peek() == Some('{') {
                    r.braced();
                }
            }
            "string" => {
                for (name, value) in r.fields() {
                    r.macros.insert(name, clean(&value));
                }
            }
            _ => {
                r.skip_space();
                r.word(&[',', '}']);
                if let Some(v) = record(&kind, &r.fields()) {
                    records.push(v);
                }
            }
        }
    }
    records
}

fn escape(text: &str) -> String {
    text.replace('&', "\\&")
        .replace('%', "\\%")
        .replace('_', "\\_")
        .replace('#', "\\#")
}

/// BibTeX text for `(citekey, record)` pairs.
pub fn write(records: &[(String, Value)]) -> String {
    let mut out = String::new();
    for (key, csl) in records {
        let get = |k: &str| csl.get(k).and_then(Value::as_str).unwrap_or("");
        let kind = get("type");
        let ty = TYPES
            .iter()
            .find(|(_, k)| *k == kind)
            .map_or("misc", |(t, _)| t);
        let _ = writeln!(out, "@{ty}{{{key},");
        let mut field = |name: &str, value: String, raw: bool| {
            if !value.trim().is_empty() {
                let _ = writeln!(
                    out,
                    "  {name} = {{{}}},",
                    if raw { value } else { escape(&value) }
                );
            }
        };
        for role in ["author", "editor"] {
            let list: Vec<String> = csl
                .get(role)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(|p| {
                    let text = person_text(p);
                    if p.get("literal").is_some() {
                        format!("{{{}}}", escape(&text))
                    } else {
                        escape(&text)
                    }
                })
                .collect();
            field(role, list.join(" and "), true);
        }
        field("title", get("title").to_string(), false);
        let container = if ty == "article" {
            "journal"
        } else {
            "booktitle"
        };
        field(container, get("container-title").to_string(), false);
        if let Some(y) = csl
            .pointer("/issued/date-parts/0/0")
            .and_then(Value::as_i64)
        {
            field("year", y.to_string(), true);
        }
        field("volume", get("volume").to_string(), false);
        field("number", get("issue").to_string(), false);
        field(
            "pages",
            get("page").replace('-', "--").replace("----", "--"),
            false,
        );
        for (name, source, raw) in [
            ("publisher", "publisher", false),
            ("address", "publisher-place", false),
            ("doi", "DOI", true),
            ("url", "URL", true),
            ("abstract", "abstract", false),
            ("keywords", "keyword", false),
            ("isbn", "ISBN", true),
            ("issn", "ISSN", true),
            ("langid", "language", true),
            ("note", "note", false),
        ] {
            field(name, get(source).to_string(), raw);
        }
        out.push_str("}\n\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
@string{nips = "Advances in Neural Information Processing Systems"}
@comment{ ignored @article{x, title={no}} }
@article{vaswani2017,
  author = {Vaswani, Ashish and Shazeer, Noam and {World Health Organization}},
  title = {Attention Is {All} You Need \& More},
  journal = nips # { 30},
  year = 2017,
  pages = {5998--6008},
  doi = "10.5555/3295222",
  keywords = {attention, transformer}
}
@book(dl, title = {Deep Learning}, author = {Goodfellow, Ian}, year = {2016}, publisher = {MIT Press},)
@misc{empty, note = {no title}}
"#;

    #[test]
    fn entries_are_read_with_macros_nesting_and_organisations() {
        let records = parse(SAMPLE);
        assert_eq!(records.len(), 2);
        let a = &records[0];
        assert_eq!(a["type"], "article-journal");
        assert_eq!(a["title"], "Attention Is All You Need & More");
        assert_eq!(
            a["container-title"],
            "Advances in Neural Information Processing Systems 30"
        );
        assert_eq!(
            a["author"],
            json!([{ "family": "Vaswani", "given": "Ashish" }, { "family": "Shazeer", "given": "Noam" }, { "literal": "World Health Organization" }])
        );
        assert_eq!(
            (a["page"].as_str(), a["DOI"].as_str(), a["issued"].clone()),
            (
                Some("5998-6008"),
                Some("10.5555/3295222"),
                json!({ "date-parts": [[2017]] })
            )
        );
        assert_eq!(
            (
                records[1]["type"].as_str(),
                records[1]["publisher"].as_str()
            ),
            (Some("book"), Some("MIT Press"))
        );
    }

    #[test]
    fn what_is_written_reads_back_the_same() {
        let records = parse(SAMPLE);
        let keyed: Vec<(String, Value)> = records
            .iter()
            .cloned()
            .enumerate()
            .map(|(i, v)| (format!("k{i}"), v))
            .collect();
        assert_eq!(parse(&write(&keyed)), records);
    }

    #[test]
    fn accents_keep_their_letter_and_junk_gives_no_records() {
        let r = parse("@article{a, title = {Caf\\'e na\\\"ive}, author = {M\\\"uller, Hans}}");
        assert_eq!(
            (r[0]["title"].as_str(), r[0]["author"][0]["family"].as_str()),
            (Some("Cafe naive"), Some("Muller"))
        );
        assert_eq!(parse("just text @ nothing here").len(), 0);
        assert!(parse("@article{unterminated, title = {open").len() <= 1);
        // Malformed input ends instead of looping.
        for junk in [
            "@a{k, = x}",
            "@a{k, t = }",
            "@a{k, t = # }",
            "@a{k, ) }",
            "@a(",
            "@",
            "@a{k, t = \"open",
            "@string{",
            "@a{k,,,,}",
        ] {
            let _ = parse(junk);
        }
    }
}
