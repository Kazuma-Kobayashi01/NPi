//! Citation keys: `<first author><year><first title word>`, lower case
//! (`vaswani2017attention`), unique inside a library by a letter suffix.

use super::csl::{normalize_title, Fields};

const STOP_WORDS: &[&str] = &["a", "an", "the", "on", "of", "in", "for", "and", "to"];
const WORD_MAX: usize = 12;

fn alnum(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// The key a record would get before uniqueness; `ref` when it has nothing to build one from.
pub fn base(fields: &Fields) -> String {
    let title = normalize_title(&fields.title);
    let word = title
        .split(' ')
        .find(|w| !w.is_empty() && !STOP_WORDS.contains(w))
        .unwrap_or("")
        .chars()
        .take(WORD_MAX)
        .collect::<String>();
    let key = format!(
        "{}{}{}",
        alnum(&fields.first_author),
        fields.year.map_or_else(String::new, |y| y.to_string()),
        word
    );
    if key.is_empty() {
        "ref".to_string()
    } else {
        key
    }
}

/// `base`, or `base` + `a`..`z`, then `base` + 27, 28, ... — the first that `taken` says is free.
pub fn unique(base: &str, taken: impl Fn(&str) -> Result<bool, String>) -> Result<String, String> {
    if !taken(base)? {
        return Ok(base.to_string());
    }
    for n in 0u32.. {
        let suffix = u8::try_from(n)
            .ok()
            .filter(|n| *n < 26)
            .map_or_else(|| (n + 1).to_string(), |n| char::from(b'a' + n).to_string());
        let key = format!("{base}{suffix}");
        if !taken(&key)? {
            return Ok(key);
        }
    }
    Err("citekey space exhausted".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields(author: &str, year: Option<i32>, title: &str) -> Fields {
        Fields {
            title: title.into(),
            first_author: author.into(),
            year,
            ..Fields::default()
        }
    }

    #[test]
    fn a_key_joins_author_year_and_the_first_real_title_word() {
        assert_eq!(
            base(&fields("Vaswani", Some(2017), "Attention Is All You Need")),
            "vaswani2017attention"
        );
        assert_eq!(
            base(&fields(
                "O'Neil",
                Some(2016),
                "The Weapons of Math Destruction"
            )),
            "oneil2016weapons"
        );
        assert_eq!(
            base(&fields("王", Some(2020), "深度学习综述")),
            "王2020深度学习综述"
        );
        assert_eq!(base(&fields("", None, "")), "ref");
    }

    #[test]
    fn a_taken_key_gets_the_next_free_suffix() {
        let taken = ["k", "ka", "kb"];
        assert_eq!(unique("k", |key| Ok(taken.contains(&key))).unwrap(), "kc");
        assert_eq!(unique("x", |key| Ok(taken.contains(&key))).unwrap(), "x");
        let all_letters = |key: &str| {
            Ok(
                key.len() == 2 && key.starts_with('k') && key.as_bytes()[1].is_ascii_lowercase()
                    || key == "k",
            )
        };
        assert_eq!(unique("k", all_letters).unwrap(), "k27");
    }
}
