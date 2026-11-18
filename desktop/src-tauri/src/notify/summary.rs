//! The one line a banner carries after its lead: the first sentence of the
//! reply, the question or the error, as plain text. Pure.

/// Characters a line keeps; a longer one ends in "…".
pub const MAX_CHARS: usize = 90;

/// The first sentence of the first prose line of Markdown `text`, without
/// its markup, at most [`MAX_CHARS`]; `None` when there is no prose (only
/// headings, code, tables, rules or blank lines). A heading is a label, not
/// a summary, so it is passed over.
pub fn line(text: &str) -> Option<String> {
    let mut fenced = false;
    for raw in text.lines() {
        let l = raw.trim();
        if l.starts_with("```") || l.starts_with("~~~") {
            fenced = !fenced;
            continue;
        }
        if fenced || l.is_empty() || l.starts_with(['|', '<', '#']) || is_rule(l) {
            continue;
        }
        let plain = inline(strip_marker(l));
        let first = sentence(plain.trim());
        if !first.is_empty() {
            return Some(cap(first));
        }
    }
    None
}

/// `---`, `***`, `___` (spaces allowed).
fn is_rule(l: &str) -> bool {
    l.chars().filter(|c| !c.is_whitespace()).count() >= 3
        && l.chars().all(|c| matches!(c, '-' | '*' | '_' | ' '))
}

/// Without a quote, list or task marker.
fn strip_marker(l: &str) -> &str {
    let mut s = l.trim_start_matches(['>', ' ']);
    for bullet in ["- ", "* ", "+ "] {
        if let Some(rest) = s.strip_prefix(bullet) {
            s = rest;
            break;
        }
    }
    let digits = s.len() - s.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    if digits > 0 {
        if let Some(rest) = s[digits..]
            .strip_prefix(". ")
            .or_else(|| s[digits..].strip_prefix(") "))
        {
            s = rest;
        }
    }
    for task in ["[ ] ", "[x] ", "[X] "] {
        if let Some(rest) = s.strip_prefix(task) {
            s = rest;
            break;
        }
    }
    s.trim_start()
}

/// Links and images as their text; bold, strike and code marks dropped.
fn inline(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        let rest = &s[i..];
        let label = rest.strip_prefix("![").or_else(|| rest.strip_prefix('['));
        if let Some(r) = label {
            if let Some(close) = r.find(']') {
                if let Some(end) = r[close + 1..].strip_prefix('(').and_then(|u| u.find(')')) {
                    out.push_str(&r[..close]);
                    i += rest.len() - r.len() + close + 2 + end + 1;
                    continue;
                }
            }
        }
        if rest.starts_with("**") || rest.starts_with("__") || rest.starts_with("~~") {
            i += 2;
            continue;
        }
        let Some(c) = rest.chars().next() else { break };
        if c != '`' {
            out.push(c);
        }
        i += c.len_utf8();
    }
    out
}

/// Up to and with the first sentence end (`。！？`, or `.!?` before a space
/// or the end); a trailing colon is dropped.
fn sentence(s: &str) -> &str {
    let mut it = s.char_indices().peekable();
    while let Some((i, c)) = it.next() {
        let end = match c {
            '。' | '！' | '？' => true,
            '.' | '!' | '?' => it.peek().is_none_or(|(_, n)| n.is_whitespace()),
            _ => false,
        };
        if end {
            return &s[..i + c.len_utf8()];
        }
    }
    s.trim_end_matches([':', '：']).trim_end()
}

fn cap(s: &str) -> String {
    if s.chars().count() <= MAX_CHARS {
        return s.to_string();
    }
    let mut out: String = s.chars().take(MAX_CHARS - 1).collect();
    out.truncate(out.trim_end().len());
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_sentence_of_a_conclusion_first_reply() {
        let reply = "Effort 拖动条改好了。截图里那种橙色是写死的。\n\n**改了什么**：\n- 颜色";
        assert_eq!(line(reply).as_deref(), Some("Effort 拖动条改好了。"));
        assert_eq!(
            line("Fixed the race in `spawn.rs`. Tests pass.").as_deref(),
            Some("Fixed the race in spawn.rs.")
        );
        assert_eq!(line("v1.2 is out").as_deref(), Some("v1.2 is out"));
    }

    #[test]
    fn markup_code_tables_and_rules_are_not_the_line() {
        let reply = "## 结论\n```rust\nfn main() {}\n```\n---\n| a | b |\n> **Done**: see [the doc](https://x/y) and ![pic](p.png)";
        assert_eq!(line(reply).as_deref(), Some("Done: see the doc and pic"));
        assert_eq!(
            line("1. [x] 第一步完成；第二步待定"),
            Some("第一步完成；第二步待定".into())
        );
        assert_eq!(line("> 改动如下：\n- a").as_deref(), Some("改动如下"));
        assert_eq!(line("```\nonly code\n```\n\n"), None);
        assert_eq!(line("note [1] here").as_deref(), Some("note [1] here"));
    }

    #[test]
    fn a_long_line_is_cut_on_a_character_with_an_ellipsis() {
        let long = "长".repeat(200);
        let got = line(&long).unwrap_or_default();
        assert_eq!(got.chars().count(), MAX_CHARS);
        assert!(got.ends_with('…'));
    }
}
