//! The Librarian as an MCP server (stdio, JSON-RPC 2.0), so any agent that speaks MCP can use a
//! project's reference library, `npi` in a terminal included.
//!
//! The app binary becomes the server with `--npi-mcp-librarian <project folder>` (no window, no
//! Tauri): the same tools the in-app Copilot has for the library and for checking quotes, plus the
//! network and editing tools of `tools.rs` (`library_lookup`, `library_align`, `library_add`,
//! `library_edit`), which the app's own Academic sessions have as built-in host tools. stdout carries
//! only the protocol; messages go to stderr.

use super::tools::{Crossref, LibraryTools, Lookup};
use crate::copilot::CopilotHost;
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

const SERVER_NAME: &str = "npi-librarian";
const SUPPORTED: &[&str] = &["2025-06-18", "2025-03-26", "2024-11-05"];
/// The tools of the in-app Copilot this server also offers: the library and read-only checks.
const SHARED: &[&str] = &[
    "library_search", "library_get", "library_check", "library_suggest", "research_search", "research_read",
    "research_check_quote", "research_check_number", "research_check_table", "research_ocr", "research_summaries",
];

const INSTRUCTIONS: &str = "The reference library of one research project. Workflow: library_search {query} to find references, then library_get {citekey} for one (a citekey looks like smith2020graph; copy it from a search result, never invent one). To add a paper: library_add {doi}. To fix metadata: library_align {citekey} previews the differences with Crossref, and the same call with apply: true writes them (blank fields only; overwrite: true also replaces differing values). To change a field, tag or note: library_edit. library_lookup only asks Crossref. library_check checks a draft's citations. Errors say what to do next: read them before retrying and do not repeat an identical call. Nothing is ever deleted.";

// ------------------------------------------------------------------ protocol

/// Serves `project` until `input` ends.
pub fn serve(project: &Path, input: impl BufRead, mut output: impl Write, lookup: &dyn Lookup) -> std::io::Result<()> {
    let root = std::fs::canonicalize(project)?;
    let host = CopilotHost::new(root.clone());
    for line in input.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        if let Some(reply) = handle(&line, &root, &host, lookup) {
            writeln!(output, "{reply}")?;
            output.flush()?;
        }
    }
    Ok(())
}

fn error(id: &Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

/// One incoming line; `None` for a notification (they get no answer).
fn handle(line: &str, root: &Path, host: &CopilotHost, lookup: &dyn Lookup) -> Option<Value> {
    let Ok(msg) = serde_json::from_str::<Value>(line) else {
        return Some(error(&Value::Null, -32700, "parse error"));
    };
    let id = msg.get("id").cloned();
    let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
    let params = msg.get("params").cloned().unwrap_or(Value::Null);
    let Some(id) = id else { return None };
    let result = match method {
        "initialize" => {
            let asked = params.get("protocolVersion").and_then(Value::as_str).unwrap_or("");
            let version = if SUPPORTED.contains(&asked) { asked } else { SUPPORTED[1] };
            json!({
                "protocolVersion": version,
                "capabilities": { "tools": {} },
                "serverInfo": { "name": SERVER_NAME, "version": env!("CARGO_PKG_VERSION") },
                "instructions": INSTRUCTIONS,
            })
        }
        "ping" => json!({}),
        "tools/list" => json!({ "tools": tools() }),
        "tools/call" => {
            let name = params.get("name").and_then(Value::as_str).unwrap_or("");
            let args = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
            match call(name, &args, root, host, lookup) {
                Some(Ok(text)) => json!({ "content": [{ "type": "text", "text": text }], "isError": false }),
                Some(Err(text)) => json!({ "content": [{ "type": "text", "text": text }], "isError": true }),
                None => return Some(error(&id, -32602, &format!("unknown tool: {name}"))),
            }
        }
        _ => return Some(error(&id, -32601, "method not found")),
    };
    Some(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
}

/// The tools, as MCP lists them (`inputSchema`, not the host-tool `parameters`).
fn tools() -> Vec<Value> {
    let mut out: Vec<Value> = crate::copilot::tool_definitions()
        .as_array()
        .into_iter()
        .flatten()
        .filter(|d| d["name"].as_str().is_some_and(|n| SHARED.contains(&n)))
        .map(|d| json!({ "name": d["name"], "description": d["description"], "inputSchema": d["parameters"] }))
        .collect();
    out.extend(LibraryTools::definitions().into_iter().map(|d| json!({ "name": d["name"], "description": d["description"], "inputSchema": d["parameters"] })));
    out
}

/// `None`: not a tool of this server.
fn call(name: &str, args: &Value, root: &Path, host: &CopilotHost, lookup: &dyn Lookup) -> Option<Result<String, String>> {
    match name {
        n if LibraryTools::NAMES.contains(&n) => LibraryTools::new(root.to_path_buf()).call(n, args, lookup),
        n if SHARED.contains(&n) => {
            let frame = host.call("mcp", n, args);
            let text = frame["result"]["content"][0]["text"].as_str().unwrap_or("").to_string();
            Some(if frame["isError"].as_bool().unwrap_or(false) { Err(text) } else { Ok(text) })
        }
        _ => None,
    }
}

// ------------------------------------------------------------------ registration

/// Adds this app as the `npi-librarian` server to `<project>/.mcp.json`, where omp (and so `npi` in a
/// terminal) finds it; other servers in the file stay as they are. Returns the file's path.
pub fn register(project: &Path, exe: &Path) -> Result<PathBuf, String> {
    let root = std::fs::canonicalize(project).map_err(|e| e.to_string())?;
    let path = root.join(".mcp.json");
    let mut doc: Value = match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).map_err(|e| format!(".mcp.json is not valid JSON ({e}); not changed"))?,
        Err(_) => json!({}),
    };
    if !doc.is_object() {
        return Err(".mcp.json is not a JSON object; not changed".into());
    }
    let servers = doc.as_object_mut().expect("object").entry("mcpServers").or_insert_with(|| json!({}));
    let servers = servers.as_object_mut().ok_or(".mcp.json has a malformed mcpServers; not changed")?;
    servers.insert(
        SERVER_NAME.to_string(),
        json!({ "command": exe.to_string_lossy(), "args": ["--npi-mcp-librarian", root.to_string_lossy()] }),
    );
    let text = serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?;
    std::fs::write(&path, format!("{text}\n")).map_err(|e| e.to_string())?;
    Ok(path)
}

/// Entry point of `--npi-mcp-librarian <folder>`: serves stdin/stdout until stdin closes.
pub fn run(project: &Path) -> Result<(), String> {
    if !project.is_dir() {
        return Err(format!("{}: not a folder", project.display()));
    }
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    serve(project, stdin.lock(), stdout.lock(), &Crossref).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::store::Library;
    use crate::librarian::lookup::Candidate;
    use crate::test_support::TempDir;

    struct Fake(Vec<Candidate>);
    impl Lookup for Fake {
        fn by_doi(&self, _: &str) -> Result<Vec<Candidate>, String> {
            Ok(self.0.clone())
        }
        fn by_title(&self, _: &str, _: &str) -> Result<Vec<Candidate>, String> {
            Ok(self.0.clone())
        }
    }

    fn project(d: &TempDir) -> PathBuf {
        let root = d.path().join("p");
        std::fs::create_dir_all(root.join("papers")).unwrap();
        let lib = Library::open(&root).unwrap();
        lib.add(&json!({ "type": "article-journal", "title": "Deep graph learning", "author": [{ "family": "Smith", "given": "A" }], "volume": "9" }), "manual").unwrap();
        root
    }

    fn talk(root: &Path, lookup: &dyn Lookup, lines: &[Value]) -> Vec<Value> {
        let input = lines.iter().map(|l| format!("{l}\n")).collect::<String>();
        let mut out = Vec::new();
        serve(root, input.as_bytes(), &mut out, lookup).unwrap();
        String::from_utf8(out).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect()
    }

    fn text_of(reply: &Value) -> String {
        reply["result"]["content"][0]["text"].as_str().unwrap_or("").to_string()
    }

    #[test]
    fn the_protocol_answers_requests_and_ignores_notifications() {
        let d = TempDir::new("mcp", "protocol");
        let root = project(&d);
        let replies = talk(
            &root,
            &Fake(vec![]),
            &[
                json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-06-18" } }),
                json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
                json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }),
                json!({ "jsonrpc": "2.0", "id": 3, "method": "nope" }),
                json!({ "jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": { "name": "library_search", "arguments": { "query": "graph" } } }),
                json!({ "jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": { "name": "research_save", "arguments": {} } }),
                json!({ "jsonrpc": "2.0", "id": 6, "method": "tools/call", "params": { "name": "library_edit", "arguments": { "citekey": "nope" } } }),
            ],
        );
        assert_eq!(replies.len(), 6, "the notification gets no answer");
        assert_eq!(replies[0]["result"]["protocolVersion"], "2025-06-18");
        assert_eq!(replies[0]["result"]["serverInfo"]["name"], "npi-librarian");
        assert!(replies[0]["result"]["instructions"].as_str().unwrap().contains("library_search"));
        let names: Vec<&str> = replies[1]["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
        for n in LibraryTools::NAMES {
            assert!(names.contains(&n), "{n}");
        }
        assert!(names.contains(&"library_search"));
        assert!(!names.contains(&"research_save") && !names.contains(&"research_compare"), "no tool that writes files");
        assert!(replies[1]["result"]["tools"][0]["inputSchema"].is_object());
        assert_eq!(replies[2]["error"]["code"], -32601);
        assert!(text_of(&replies[3]).contains("Deep graph learning"));
        assert_eq!(replies[4]["error"]["code"], -32602, "a tool of the Copilot that is not offered here");
        assert_eq!(replies[5]["result"]["isError"], true);
        assert!(text_of(&replies[5]).contains("library_search"), "an error says what to do next");
    }

    #[test]
    fn registering_keeps_other_servers_and_refuses_a_broken_file() {
        let d = TempDir::new("mcp", "register");
        let root = project(&d);
        std::fs::write(root.join(".mcp.json"), r#"{"mcpServers":{"other":{"command":"x"}}}"#).unwrap();
        let path = register(&root, Path::new("/Applications/NPi Desktop.app/Contents/MacOS/omp-desktop")).unwrap();
        let doc: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(doc["mcpServers"]["other"]["command"], "x");
        assert_eq!(doc["mcpServers"]["npi-librarian"]["args"][0], "--npi-mcp-librarian");
        std::fs::write(root.join(".mcp.json"), "{not json").unwrap();
        assert!(register(&root, Path::new("/x")).is_err());
        assert_eq!(std::fs::read_to_string(root.join(".mcp.json")).unwrap(), "{not json");
    }
}
