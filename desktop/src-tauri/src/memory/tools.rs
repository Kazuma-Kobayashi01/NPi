//! What the model can do with Memory: host tools added on top of whatever host tools the session
//! already has (`MemoryHost`) — notes (`remember`, `memory_lookup`, `forget_memory`), the mode's
//! profile (`update_user_profile`) and past conversations (`search_conversations`). The prompt block
//! that goes with them is `prompt.rs`.

use super::{profile, Store};
use crate::copilot::{frame, HostTools};
use crate::session_search::{Index, Scope};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;

/// Hits a search returns.
const SEARCH_LIMIT: usize = 8;

fn tool(name: &str, label: &str, description: &str, properties: &Value, required: &[&str]) -> Value {
    json!({
        "name": name,
        "label": label,
        "description": description,
        "parameters": { "type": "object", "properties": properties, "required": required, "additionalProperties": false },
        "loadMode": "essential"
    })
}

fn definitions(search: bool) -> Vec<Value> {
    let mut all = vec![
        tool(
            "remember",
            "Remember",
            "Save or update a short note about the user that lasts across conversations. `kind`: you (who they are), topic (an interest or subject), area (work or a project), person (someone in their life). `title`: the subject, short and stable, such as Travel or Paper Agent; the same title replaces that note, so merge what it said. `text`: one short sentence in the user's language. Example: {\"kind\": \"area\", \"title\": \"Paper Agent\", \"text\": \"Builds a personal agent that helps with reading papers.\"}",
            &json!({ "kind": { "type": "string", "enum": super::KINDS }, "title": { "type": "string" }, "text": { "type": "string" } }),
            &["kind", "title", "text"],
        ),
        tool(
            "memory_lookup",
            "Look in memory",
            "Find notes about the user holding every word of `query`, for when the notes in your prompt are not all of them.",
            &json!({ "query": { "type": "string" } }),
            &["query"],
        ),
        tool(
            "forget_memory",
            "Forget",
            "Delete the note with this `title`, when the user asks you to forget it.",
            &json!({ "title": { "type": "string" } }),
            &["title"],
        ),
        tool(
            "update_user_profile",
            "Update profile",
            &format!("Replace your profile of how the user likes to work in this mode with `text`: the whole profile, the old one merged with what changed, at most {} characters. Empty text clears it.", profile::MAX_CHARS),
            &json!({ "text": { "type": "string" } }),
            &["text"],
        ),
    ];
    if search {
        all.push(tool(
            "search_conversations",
            "Search past conversations",
            "Find what you and the user said in earlier conversations (never tool output) holding the words of `query`; separate alternatives with spaces. `scope`: here (default: every chat in Chat; this folder's conversations in Code and Academic) or all. Gives dated excerpts with the conversation's title; this conversation may be among them.",
            &json!({ "query": { "type": "string" }, "scope": { "type": "string", "enum": ["here", "all"] } }),
            &["query"],
        ));
    }
    all
}

/// Told of each change the model makes (a note, the profile), as JSON, so the page can say so.
pub type Noticed = Box<dyn Fn(Value) + Send + Sync>;

/// Where a session's past conversations are: the index file and the scope of `here`.
pub struct Search {
    pub index: PathBuf,
    pub here: Scope,
}

/// What a session's memory tools work on.
pub struct Context {
    /// `memory.db`.
    pub db: PathBuf,
    /// The data root, where the profiles are.
    pub root: PathBuf,
    pub mode: &'static str,
    /// The tab, kept with each note it writes.
    pub session: String,
    pub search: Option<Search>,
    pub noticed: Option<Noticed>,
}

/// A session's host tools with the memory tools on top.
pub struct MemoryHost {
    inner: Arc<dyn HostTools>,
    cx: Context,
}

impl MemoryHost {
    pub fn new(inner: Arc<dyn HostTools>, cx: Context) -> Self {
        Self { inner, cx }
    }

    fn tell(&self, change: Value) {
        if let Some(tell) = &self.cx.noticed {
            tell(change);
        }
    }

    fn run(&self, tool: &str, args: &Value) -> Result<String, String> {
        let text = |key: &str| args.get(key).and_then(Value::as_str).unwrap_or("");
        let cx = &self.cx;
        match tool {
            "remember" => {
                let (note, existed) = Store::open(&cx.db)?.save(text("kind"), text("title"), text("text"), cx.mode, Some(&cx.session))?;
                self.tell(json!({ "note": note, "replaced": existed }));
                Ok(if existed { "Updated." } else { "Remembered." }.to_string())
            }
            "memory_lookup" => Store::open(&cx.db)?.search(text("query")).map(|notes| {
                if notes.is_empty() {
                    "No note holds that.".to_string()
                } else {
                    notes.iter().map(|n| format!("[{}] {}: {}", n.kind, n.title, n.text)).collect::<Vec<_>>().join("\n")
                }
            }),
            "forget_memory" => Store::open(&cx.db)?.forget(text("title")).map(|gone| if gone { "Forgotten." } else { "There is no note with that title." }.to_string()),
            "update_user_profile" => {
                profile::write(&cx.root, cx.mode, text("text"))?;
                self.tell(json!({ "profile": cx.mode, "text": text("text").trim() }));
                Ok(format!("Profile saved ({} of {} characters).", text("text").trim().chars().count(), profile::MAX_CHARS))
            }
            "search_conversations" => {
                let search = cx.search.as_ref().ok_or("search is not available here")?;
                let everywhere = Scope { root: search.here.root.clone(), cwds: None }; // `all`: the same folder, no filter
                let scope = if text("scope") == "all" { &everywhere } else { &search.here };
                let hits = Index::open(&search.index)?.search(text("query"), scope, SEARCH_LIMIT)?;
                if hits.is_empty() {
                    return Ok("Nothing in past conversations holds that.".to_string());
                }
                Ok(hits.iter().map(|h| format!("- {} · {} · {}\n  {}: {}", h.when, h.title, h.cwd, h.role, h.excerpt)).collect::<Vec<_>>().join("\n"))
            }
            other => Err(format!("unknown tool: {other}")),
        }
    }
}

impl HostTools for MemoryHost {
    fn definitions(&self) -> Value {
        let mut all = self.inner.definitions().as_array().cloned().unwrap_or_default();
        all.extend(definitions(self.cx.search.is_some()));
        Value::Array(all)
    }

    fn call(&self, id: &str, tool: &str, args: &Value) -> Value {
        if !definitions(self.cx.search.is_some()).iter().any(|d| d["name"] == tool) {
            return self.inner.call(id, tool, args);
        }
        frame(id, self.run(tool, args))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    struct Inner;
    impl HostTools for Inner {
        fn definitions(&self) -> Value {
            json!([{ "name": "inner_tool", "description": "A tool of the session.", "parameters": { "type": "object", "properties": {} } }])
        }
        fn call(&self, id: &str, tool: &str, _: &Value) -> Value {
            frame(id, Ok(format!("inner {tool}")))
        }
    }

    fn context(d: &TempDir, mode: &'static str, search: Option<Search>, noticed: Option<Noticed>) -> Context {
        Context { db: d.path().join("memory.db"), root: d.path().to_path_buf(), mode, session: "tab-7".into(), search, noticed }
    }

    fn names(host: &MemoryHost) -> Vec<String> {
        host.definitions().as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap().to_string()).collect()
    }

    #[test]
    fn the_tools_sit_on_top_of_the_sessions_own_and_keep_the_mode() {
        let d = TempDir::new("memory-tools", "host");
        let heard = Arc::new(std::sync::Mutex::new(Vec::new()));
        let log = Arc::clone(&heard);
        let tell: Noticed = Box::new(move |change| log.lock().unwrap().push(change));
        let host = MemoryHost::new(Arc::new(Inner), context(&d, "academic", None, Some(tell)));
        assert_eq!(names(&host), ["inner_tool", "remember", "memory_lookup", "forget_memory", "update_user_profile"]);
        assert!(host.call("1", "inner_tool", &json!({})).to_string().contains("inner inner_tool"));
        assert!(host.call("2", "remember", &json!({ "kind": "you", "title": "Study", "text": "Studies in Osaka." })).to_string().contains("Remembered"));
        assert!(host.call("3", "remember", &json!({ "kind": "you", "title": "Study", "text": "Studies in Kyoto." })).to_string().contains("Updated"));
        let note = &Store::open(&d.path().join("memory.db")).unwrap().list().unwrap()[0];
        assert_eq!((note.mode.as_str(), note.session.as_deref()), ("academic", Some("tab-7")));
        assert!(host.call("4", "memory_lookup", &json!({ "query": "kyoto" })).to_string().contains("Studies in Kyoto"));
        assert!(host.call("5", "forget_memory", &json!({ "title": "Study" })).to_string().contains("Forgotten"));
        assert!(host.call("6", "update_user_profile", &json!({ "text": "Wants page numbers." })).to_string().contains("Profile saved"));
        assert_eq!(profile::read(d.path(), "academic"), "Wants page numbers.\n");
        let heard = heard.lock().unwrap();
        assert_eq!((heard[0]["replaced"].as_bool(), heard[1]["replaced"].as_bool()), (Some(false), Some(true)), "each write is told, new then replaced");
        assert_eq!(heard[2]["profile"], "academic");
    }

    #[test]
    fn search_is_offered_only_with_a_scope_and_keeps_to_it_unless_asked_for_all() {
        let d = TempDir::new("memory-tools", "search");
        let root = d.path().join("sessions");
        for (folder, cwd, text) in [("a", "/u/Code/One", "hokkaido trip in winter"), ("b", "/u/Code/Two", "hokkaido ski rental")] {
            std::fs::create_dir_all(root.join(folder)).unwrap();
            let lines = [
                format!(r#"{{"type":"session","id":"{folder}","timestamp":"2026-11-01T00:00:00Z","cwd":"{cwd}"}}"#),
                format!(r#"{{"type":"message","timestamp":"2026-11-01T00:00:00Z","message":{{"role":"user","content":"{text}"}}}}"#),
            ];
            std::fs::write(root.join(folder).join(format!("x_{folder}.jsonl")), lines.join("\n")).unwrap();
        }
        assert!(!names(&MemoryHost::new(Arc::new(Inner), context(&d, "code", None, None))).contains(&"search_conversations".to_string()));
        let here = Scope { root, cwds: Some(vec![crate::saved_sessions::normalize_cwd("/u/Code/One")]) };
        let host = MemoryHost::new(Arc::new(Inner), context(&d, "code", Some(Search { index: d.path().join("index.db"), here }), None));
        let near = host.call("1", "search_conversations", &json!({ "query": "hokkaido" })).to_string();
        assert!(near.contains("winter") && !near.contains("ski"));
        let far = host.call("2", "search_conversations", &json!({ "query": "hokkaido", "scope": "all" })).to_string();
        assert!(far.contains("winter") && far.contains("ski"));
        assert!(host.call("3", "search_conversations", &json!({ "query": "tokyo" })).to_string().contains("Nothing"));
    }

    /// With a real model: `npi` next to the test binary, then run the test binary with `--ignored`.
    #[test]
    #[ignore = "needs a configured model"]
    fn real_model_takes_notes_and_uses_them() {
        let d = TempDir::new("memory-tools", "real");
        let store = Store::open(&d.path().join("memory.db")).unwrap();
        let host = || Arc::new(MemoryHost::new(Arc::new(Inner), context(&d, "chat", None, None))) as Arc<dyn HostTools>;
        let prompt = || super::super::prompt::block(&store, d.path(), "chat", false);
        let first = crate::copilot_worker::run(d.path(), None, &prompt(), Some(host()), "我在大阪留学，研究电阻抗成像，最近在做一个帮我读论文的 Agent。先简单说一句：多层感知机是什么？").unwrap();
        println!("--- first\n{first}");
        for n in store.list().unwrap() {
            println!("[{}] {}: {}", n.kind, n.title, n.text);
        }
        assert!(!store.list().unwrap().is_empty(), "something lasting was said");
        let second = crate::copilot_worker::run(d.path(), None, &prompt(), Some(host()), "你觉得我做的项目下一步该怎么走？一句话。").unwrap();
        println!("--- second\n{second}");
        let plain = store.list().unwrap().len();
        crate::copilot_worker::run(d.path(), None, &prompt(), Some(host()), "1+1 等于几？").unwrap();
        assert_eq!(store.list().unwrap().len(), plain, "nothing lasting, no note");
    }
}
