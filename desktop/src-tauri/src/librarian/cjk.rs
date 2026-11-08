//! Text normalisation for the Research Core's search index and quote checks.
//!
//! FTS5's `unicode61` keeps a run of Chinese characters as one token and
//! `trigram` finds nothing shorter than three characters, so neither finds the
//! two-character word 文献 (`docs/archive/PLAN_3.0.md` section 7). The index therefore
//! stores a normalised column: Latin words lower-cased as they are, and every
//! CJK run as its characters (unigrams) plus each pair of neighbours
//! (bigrams). A query goes through the same split: a CJK run of n >= 2
//! characters becomes the AND of its bigrams (so 文献检索 needs 文献, 献检
//! and 检索), a single CJK character its unigram, and a Latin word is a
//! prefix match. [`VERSION`] is bumped whenever this split changes, which
//! makes the index rebuild itself.

/// The version of the split below; stored with the index.
pub const VERSION: &str = "1";

/// Han, kana and Hangul: scripts written without spaces between words.
pub fn is_cjk(c: char) -> bool {
    matches!(c as u32,
        0x3040..=0x30FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xAC00..=0xD7AF
        | 0xF900..=0xFAFF | 0x20000..=0x2FA1F)
}

#[derive(Debug, PartialEq, Eq)]
enum Token {
    Word(String),
    Run(Vec<char>),
}

fn tokens(text: &str) -> Vec<Token> {
    let mut out = Vec::new();
    let mut word = String::new();
    let mut run: Vec<char> = Vec::new();
    for c in text.chars() {
        if is_cjk(c) {
            if !word.is_empty() {
                out.push(Token::Word(std::mem::take(&mut word)));
            }
            run.push(c);
        } else if c.is_alphanumeric() {
            if !run.is_empty() {
                out.push(Token::Run(std::mem::take(&mut run)));
            }
            word.extend(c.to_lowercase());
        } else {
            if !word.is_empty() {
                out.push(Token::Word(std::mem::take(&mut word)));
            }
            if !run.is_empty() {
                out.push(Token::Run(std::mem::take(&mut run)));
            }
        }
    }
    if !word.is_empty() {
        out.push(Token::Word(word));
    }
    if !run.is_empty() {
        out.push(Token::Run(run));
    }
    out
}

/// The text of the normalised column: words, then CJK unigrams and bigrams, space separated.
pub fn index_text(text: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    for token in tokens(text) {
        match token {
            Token::Word(w) => out.push(w),
            Token::Run(run) => {
                for (i, c) in run.iter().enumerate() {
                    out.push(c.to_string());
                    if let Some(next) = run.get(i + 1) {
                        out.push(format!("{c}{next}"));
                    }
                }
            }
        }
    }
    out.join(" ")
}

/// An FTS5 `MATCH` expression for a user's query, built only from quoted
/// tokens: nothing the caller typed is ever read as FTS syntax. Whitespace
/// separated parts are alternatives (more matches rank higher); inside one
/// part, a mixed word such as `BERT模型` needs all of its pieces. `None` when the
/// query holds nothing searchable.
pub fn match_expression(query: &str) -> Option<String> {
    let quote = |t: &str| format!("\"{}\"", t.replace('"', "\"\""));
    let mut alternatives = Vec::new();
    for part in query.split_whitespace() {
        let mut needs: Vec<String> = Vec::new();
        for token in tokens(part) {
            match token {
                Token::Word(w) => needs.push(format!("{}*", quote(&w))),
                Token::Run(run) if run.len() == 1 => needs.push(quote(&run[0].to_string())),
                Token::Run(run) => {
                    for pair in run.windows(2) {
                        needs.push(quote(&pair.iter().collect::<String>()));
                    }
                }
            }
        }
        match needs.len() {
            0 => {}
            1 => alternatives.push(needs.remove(0)),
            _ => alternatives.push(format!("({})", needs.join(" AND "))),
        }
    }
    (!alternatives.is_empty()).then(|| alternatives.join(" OR "))
}

/// The words of `query` as they are looked for in the original text (lower-cased,
/// CJK runs kept whole), longest first: the excerpt of a hit is cut around the first.
pub fn highlight_terms(query: &str) -> Vec<Vec<char>> {
    let mut terms: Vec<Vec<char>> = tokens(query)
        .into_iter()
        .map(|t| match t {
            Token::Word(w) => w.chars().collect(),
            Token::Run(r) => r,
        })
        .collect();
    terms.sort_by_key(|t| std::cmp::Reverse(t.len()));
    terms
}

/// `text` in the form two copies of the same passage agree on, however a PDF broke
/// its lines: lower case, one space between words, no space beside a CJK character,
/// ligatures and typographic quotes and dashes in plain form, and a hyphen at a line
/// break joined back (`recur- rence` -> `recurrence`).
pub fn quote_form(text: &str) -> String {
    let mut plain = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '\u{00AD}' | '\u{200B}' | '\u{FEFF}' => {}
            '\u{FB00}' => plain.push_str("ff"),
            '\u{FB01}' => plain.push_str("fi"),
            '\u{FB02}' => plain.push_str("fl"),
            '\u{FB03}' => plain.push_str("ffi"),
            '\u{FB04}' => plain.push_str("ffl"),
            '\u{2018}' | '\u{2019}' => plain.push('\''),
            '\u{201C}' | '\u{201D}' => plain.push('"'),
            '\u{2010}' | '\u{2011}' | '\u{2012}' | '\u{2013}' | '\u{2014}' | '\u{2212}' => plain.push('-'),
            c if c.is_whitespace() => plain.push(' '),
            c => plain.extend(c.to_lowercase()),
        }
    }
    let chars: Vec<char> = plain.chars().collect();
    let mut out = String::with_capacity(plain.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == ' ' {
            let prev = out.chars().last();
            while chars.get(i + 1) == Some(&' ') {
                i += 1;
            }
            let next = chars.get(i + 1).copied();
            let drop = match (prev, next) {
                (None, _) | (_, None) => true,
                (Some(p), Some(n)) => is_cjk(p) || is_cjk(n),
            };
            if !drop {
                out.push(' ');
            }
        } else if c == '-'
            && out.chars().last().is_some_and(char::is_alphabetic)
            && chars.get(i + 1) == Some(&' ')
            && chars[i + 1..].iter().find(|c| **c != ' ').is_some_and(|n| n.is_lowercase())
        {
            // A hyphen followed by a line break: join the halves.
            while chars.get(i + 1) == Some(&' ') {
                i += 1;
            }
        } else {
            out.push(c);
        }
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cjk_run_is_indexed_as_unigrams_and_bigrams_and_latin_as_words() {
        assert_eq!(index_text("文献检索 Graph-Net"), "文 文献 献 献检 检 检索 索 graph net");
    }

    #[test]
    fn queries_become_quoted_tokens_only() {
        assert_eq!(match_expression("文献").as_deref(), Some("\"文献\""));
        assert_eq!(match_expression("文献检索").as_deref(), Some("(\"文献\" AND \"献检\" AND \"检索\")"));
        assert_eq!(match_expression("检").as_deref(), Some("\"检\""));
        assert_eq!(match_expression("ab cd").as_deref(), Some("\"ab\"* OR \"cd\"*"));
        assert_eq!(match_expression("BERT模型").as_deref(), Some("(\"bert\"* AND \"模型\")"));
        // FTS syntax typed by a caller is only punctuation.
        assert_eq!(
            match_expression("\"a\" OR NEAR(b c) -d").as_deref(),
            Some("\"a\"* OR \"or\"* OR (\"near\"* AND \"b\"*) OR \"c\"* OR \"d\"*")
        );
        assert_eq!(match_expression(" *-() "), None);
    }

    #[test]
    fn the_quote_form_ignores_layout() {
        assert_eq!(quote_form("The recur-\nrence  of “this”"), "the recurrence of \"this\"");
        assert_eq!(quote_form("state-of-the-art ﬁnding"), "state-of-the-art finding");
        assert_eq!(quote_form("文献\n检索 ok"), "文献检索ok");
        assert_eq!(quote_form("2020 - 2021"), "2020 - 2021");
    }
}
