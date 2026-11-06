//! One run of the Academic Copilot with no pane: an agent hands it a task and gets its final answer.
//! `convert` is the same run with no tools, for a text job (the Write window's Compile).
//! `copilot_ask` reaches this as a built-in tool of an Academic session ([`MainHost`]) and over MCP
//! (`librarian/mcp.rs`, `--npi-mcp-copilot`), for `npi` in a terminal or any other MCP client.
//!
//! The run is the Copilot's worker: omp with none of its own tools, only [`AcademicHost`]'s, bound to
//! the project. It keeps no conversation, nobody can answer it, and it cannot call itself: the tools
//! it gets do not include `copilot_ask`.

use crate::agent::{spawn_omp, Launch};
use crate::approval_mode::ApprovalMode;
use crate::copilot::{frame, AcademicHost, HostTools};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{channel, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// A run longer than this is stopped.
const DEADLINE: Duration = Duration::from_mins(10);
/// Runs at once in this process: a model may ask for many in parallel, and each is a whole agent.
/// Runs at once: two Copilot asks, or one multi-expert review's four experts (`expert_review`).
const MAX_RUNS: usize = 6;
const MAX_TASK_CHARS: usize = 8_000;
static RUNNING: AtomicUsize = AtomicUsize::new(0);

const COPILOT_PROMPT: &str = "\
You are NPi Academic Copilot, a literature assistant bound to one research project. You work only through the tools below: no shell, no file edits outside research_save, and the network only for metadata lookups (Crossref, OpenAlex). Never claim otherwise. You cannot move, rename or delete files.
Answer from the project's files: cite the file and line, or [@citekey] and page, for every claim taken from them, quote exactly, and say plainly when the files do not hold the answer. A PDF is read as its extracted text, and line numbers refer to that text. Keep what a paper states apart from your own inference.
Cite only references the library holds; one it does not hold is unverified, and you give no DOI, page number or venue it does not state.
A summary or comparison cell without a checked quote is shown as your reading, never as verified: give the exact quote and page for every claim taken from a paper, and use not_reported or not_comparable instead of a plausible guess.
Reply in the user's language.

";

const UNATTENDED: &str = "\n# This run\nAnother agent gave you this task and nobody can answer questions. Do what you can, do not ask. End with a short report: what you found or changed, by citekey and file, and what you could not do.\n";

/// The rule new projects used to get. In a project that still has it word for word, the prompt carries
/// the current one; a rule the user wrote is left as it is.
const OLD_RULE: &str = "10. Never change the library yourself; propose tags, groups or notes with library_suggest and let the user decide.";
const NEW_RULE: &str = "10. Change the library only when asked to organise it (library_edit, library_tag, library_group, library_align, library_add, library_merge); for what nobody asked for, propose with library_suggest and let the user decide.";

/// The Copilot's system prompt for `root`: its role, then the project's rules.
pub fn system_prompt(root: &Path) -> String {
    let rules = crate::academic_space::AcademicSpace::rules(root).replace(OLD_RULE, NEW_RULE);
    format!("{COPILOT_PROMPT}{}\n# Project rules\n{rules}", crate::app_mode::LIBRARY_GUIDE)
}

/// Writes the settings of an isolated worker into `dir` and returns the file.
pub fn write_policy(dir: &Path) -> Result<PathBuf, String> {
    crate::private_fs::private_dirs(true).create(dir).map_err(|e| format!("worker folder: {e}"))?;
    let config = dir.join("runtime.yml");
    std::fs::write(&config, "includeWorkspaceTree: false\nworkspace:\n  additionalDirectories: []\nplan:\n  enabled: false\n  defaultOnStartup: false\ngit:\n  enabled: false\n").map_err(|e| format!("worker policy: {e}"))?;
    Ok(config)
}

/// What a frame of the run means for it.
#[derive(Debug, PartialEq, Eq)]
enum Step {
    Go,
    Ask(String),
    Done(Result<String, String>),
}

fn assistant_text(message: &Value) -> Option<String> {
    if message.get("role").and_then(Value::as_str) != Some("assistant") {
        return None;
    }
    let parts: Vec<&str> = message
        .get("content")?
        .as_array()?
        .iter()
        .filter(|p| p.get("type").and_then(Value::as_str) == Some("text"))
        .filter_map(|p| p.get("text").and_then(Value::as_str))
        .collect();
    let text = parts.join("").trim().to_string();
    (!text.is_empty()).then_some(text)
}

/// Follows one frame; `answer` holds the last thing the model said.
fn observe(frame: &Value, answer: &mut Option<String>) -> Step {
    match frame.get("type").and_then(Value::as_str).unwrap_or("") {
        "message_end" => {
            if let Some(text) = frame.get("message").and_then(assistant_text) {
                *answer = Some(text);
            }
            Step::Go
        }
        "response" if frame.get("command").and_then(Value::as_str) == Some("prompt") && frame.get("success") == Some(&json!(false)) => {
            Step::Done(Err(frame.get("error").and_then(Value::as_str).unwrap_or("the Copilot refused the task").to_string()))
        }
        "prompt_result" => match frame.get("status").and_then(Value::as_str) {
            Some("error") => Step::Done(Err(frame.pointer("/error/message").and_then(Value::as_str).unwrap_or("the Copilot's model request failed").to_string())),
            _ => Step::Done(answer.take().ok_or_else(|| "the Copilot finished without an answer".to_string())),
        },
        // setStatus and setWidget expect nothing; a dialog would wait for a person who is not there.
        "extension_ui_request" => match (frame.get("method").and_then(Value::as_str), frame.get("id").and_then(Value::as_str)) {
            (Some("setStatus" | "setWidget" | "notify" | "setTitle"), _) | (_, None) => Step::Go,
            (_, Some(id)) => Step::Ask(id.to_string()),
        },
        _ => Step::Go,
    }
}

#[derive(Debug)]
struct Slot;

impl Slot {
    fn take() -> Result<Self, String> {
        if RUNNING.fetch_add(1, Ordering::SeqCst) >= MAX_RUNS {
            RUNNING.fetch_sub(1, Ordering::SeqCst);
            return Err(format!("{MAX_RUNS} Copilot runs are going already; wait for one to finish"));
        }
        Ok(Self)
    }
}

impl Drop for Slot {
    fn drop(&mut self) {
        RUNNING.fetch_sub(1, Ordering::SeqCst);
    }
}

fn write_line(stdin: &Mutex<std::process::ChildStdin>, line: &Value) {
    if let Ok(mut w) = stdin.lock() {
        // A failed write means omp is gone; the reading loop reports that.
        let _ = writeln!(w, "{line}");
    }
}

/// Runs `task` as the Copilot of `root` with `profile`'s model, and returns its final answer.
pub fn ask(root: &Path, profile: Option<&str>, task: &str) -> Result<String, String> {
    let task = task.trim();
    if task.is_empty() {
        return Err("`task` is empty: say what the Copilot should do".into());
    }
    if task.chars().count() > MAX_TASK_CHARS {
        return Err(format!("`task` is over {MAX_TASK_CHARS} characters: name the files or citekeys instead of pasting them"));
    }
    let prompt = format!("{}{UNATTENDED}", system_prompt(root));
    let host: Arc<dyn HostTools> = Arc::new(AcademicHost::new(root.to_path_buf()));
    run(root, profile, &prompt, Some(host), task)
}

/// A one-shot text job for a worker with no tools at all: `text` goes in, its answer comes back.
pub fn convert(root: &Path, profile: Option<&str>, prompt: &str, text: &str) -> Result<String, String> {
    run(root, profile, prompt, None, text)
}

pub fn run(root: &Path, profile: Option<&str>, prompt: &str, host: Option<Arc<dyn HostTools>>, task: &str) -> Result<String, String> {
    run_on(root, profile, None, prompt, host, task)
}

/// A model for one run: `provider/id`, with an optional `:level` of thinking (a model role's value).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pick {
    pub provider: String,
    pub id: String,
    pub thinking: Option<String>,
}

const THINKING_LEVELS: [&str; 7] = ["off", "minimal", "low", "medium", "high", "xhigh", "max"];

impl Pick {
    /// `provider/id[:level]`; a trailing `:word` is a thinking level only when it names one, since
    /// model ids may hold colons of their own (`qwen/qwen3-coder:exacto`).
    pub fn parse(selector: &str) -> Option<Self> {
        let (provider, rest) = selector.trim().split_once('/')?;
        let (id, thinking) = match rest.rsplit_once(':') {
            Some((id, level)) if THINKING_LEVELS.contains(&level) => (id, Some(level.to_string())),
            _ => (rest, None),
        };
        (!provider.is_empty() && !id.is_empty()).then(|| Self { provider: provider.to_string(), id: id.to_string(), thinking })
    }

    pub fn label(&self) -> String {
        format!("{}/{}", self.provider, self.id)
    }
}

/// Waits for omp's answer to the command sent with `id`, setting the run's model before the task.
fn await_response(rx: &std::sync::mpsc::Receiver<String>, id: &str, until: Instant) -> Result<(), String> {
    loop {
        let line = rx.recv_timeout(until.saturating_duration_since(Instant::now())).map_err(|_| "the worker did not answer".to_string())?;
        let Ok(frame) = serde_json::from_str::<Value>(&line) else { continue };
        if frame.get("type").and_then(Value::as_str) == Some("response") && frame.get("id").and_then(Value::as_str) == Some(id) {
            if frame.get("success") == Some(&json!(true)) {
                return Ok(());
            }
            return Err(frame.get("error").and_then(Value::as_str).unwrap_or("refused").to_string());
        }
    }
}

/// [`run`] on `pick`'s model (and thinking) instead of the profile's default.
pub fn run_on(root: &Path, profile: Option<&str>, pick: Option<&Pick>, prompt: &str, host: Option<Arc<dyn HostTools>>, task: &str) -> Result<String, String> {
    let _slot = Slot::take()?;
    let dir = root.join(".npi-academic").join("copilot-run");
    let config = write_policy(&dir)?;
    let cwd = dir.to_str().ok_or("the project path is not UTF-8")?;
    let (mut child, supervisor) = spawn_omp(Launch {
        cwd: Some(cwd),
        resume: None,
        profile,
        // Yolo so the host tools run: nobody is there to answer a prompt, and omp's own tools are off (`sidechat`).
        mode: ApprovalMode::Yolo,
        app_mode: "academic",
        config: Some(&config),
        prompt: Some(prompt),
        append_prompt: None,
        sidechat: true,
    })?;
    let stdin = Arc::new(Mutex::new(child.stdin.take().ok_or("no stdin")?));
    let stdout = child.stdout.take().ok_or("no stdout")?;
    let stderr = child.stderr.take().ok_or("no stderr")?;
    let tail = Arc::new(Mutex::new(String::new()));
    let tail_writer = Arc::clone(&tail);
    std::thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            if let Ok(mut t) = tail_writer.lock() {
                t.clone_from(&line);
            }
        }
    });
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    let until = Instant::now() + DEADLINE;
    if let Some(pick) = pick {
        write_line(&stdin, &json!({ "id": "pick-model", "type": "set_model", "provider": pick.provider, "modelId": pick.id }));
        let picked = await_response(&rx, "pick-model", until).map_err(|e| format!("{}: {e}", pick.label()));
        let thought = picked.and_then(|()| pick.thinking.as_ref().map_or(Ok(()), |level| {
            write_line(&stdin, &json!({ "id": "pick-thinking", "type": "set_thinking_level", "level": level }));
            await_response(&rx, "pick-thinking", until).map_err(|e| format!("{} thinking {level}: {e}", pick.label()))
        }));
        if let Err(e) = thought {
            supervisor.kill_tree();
            let _ = child.wait();
            return Err(e);
        }
    }
    if let Some(host) = &host {
        write_line(&stdin, &json!({ "type": "set_host_tools", "tools": host.definitions() }));
    }
    write_line(&stdin, &json!({ "type": "prompt", "message": task }));
    let mut answer = None;
    let outcome = loop {
        let line = match rx.recv_timeout(until.saturating_duration_since(Instant::now())) {
            Ok(line) => line,
            Err(RecvTimeoutError::Timeout) => break Err("the Copilot ran out of time and was stopped".to_string()),
            Err(RecvTimeoutError::Disconnected) => {
                let last = tail.lock().map(|t| t.clone()).unwrap_or_default();
                break Err(format!("the Copilot stopped early: {last}"));
            }
        };
        let Ok(parsed) = serde_json::from_str::<Value>(&line) else { continue };
        if parsed.get("type").and_then(Value::as_str) == Some("host_tool_call") {
            // Its own thread, as in a tab (`agent/reader.rs`): a slow tool must not hold up the run.
            let Some(host) = host.clone() else { continue };
            let stdin = Arc::clone(&stdin);
            std::thread::spawn(move || {
                let Some(id) = parsed.get("id").and_then(Value::as_str) else { return };
                let tool = parsed.get("toolName").and_then(Value::as_str).unwrap_or("");
                write_line(&stdin, &host.call(id, tool, parsed.get("arguments").unwrap_or(&Value::Null)));
            });
            continue;
        }
        match observe(&parsed, &mut answer) {
            Step::Go => {}
            Step::Ask(id) => write_line(&stdin, &json!({ "type": "extension_ui_response", "id": id, "cancelled": true })),
            Step::Done(result) => break result,
        }
    };
    supervisor.kill_tree();
    let _ = child.wait();
    outcome
}

/// `copilot_ask` as a tool definition (host tools say `parameters`; `mcp.rs` renames it).
pub fn definition() -> Value {
    json!({
        "name": "copilot_ask",
        "label": "Ask the Copilot",
        "description": "Hand a task to the project\'s Copilot, a separate agent with this project\'s literature tools, and get its report back. Use it for many-step work that would fill your context: organising the library, summarising or comparing a set of papers, checking a draft against its sources. It knows nothing of this conversation and cannot ask you anything, so say the goal, the citekeys or files, and what to report. It may take minutes. Example: {\"task\": \"Fill the missing metadata of every reference, tag them by method, and list the duplicates without merging them.\"}",
        "parameters": {
            "type": "object",
            "properties": { "task": { "type": "string" } },
            "required": ["task"],
            "additionalProperties": false
        },
        "loadMode": "essential"
    })
}

/// The built-in tools of an Academic session in a project: the Copilot's own, plus `copilot_ask`.
pub struct MainHost {
    tools: AcademicHost,
    root: PathBuf,
    profile: Option<String>,
}

impl MainHost {
    pub fn new(root: PathBuf, profile: Option<&str>) -> Self {
        Self { tools: AcademicHost::new(root.clone()), root, profile: profile.map(str::to_string) }
    }
}

impl HostTools for MainHost {
    fn definitions(&self) -> Value {
        let mut all = self.tools.definitions().as_array().cloned().unwrap_or_default();
        all.push(definition());
        Value::Array(all)
    }
    fn call(&self, id: &str, tool: &str, args: &Value) -> Value {
        if tool != "copilot_ask" {
            return self.tools.call(id, tool, args);
        }
        let task = args.get("task").and_then(Value::as_str).unwrap_or("");
        frame(id, ask(&self.root, self.profile.as_deref(), task))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    fn say(text: &str) -> Value {
        json!({ "type": "message_end", "message": { "role": "assistant", "content": [{ "type": "thinking", "thinking": "hm" }, { "type": "text", "text": text }] } })
    }

    #[test]
    fn the_answer_is_the_last_thing_the_model_said_when_the_prompt_closes() {
        let mut answer = None;
        assert_eq!(observe(&say("first"), &mut answer), Step::Go);
        assert_eq!(observe(&json!({ "type": "agent_end" }), &mut answer), Step::Go, "a retry may follow agent_end");
        assert_eq!(observe(&say("final"), &mut answer), Step::Go);
        assert_eq!(observe(&json!({ "type": "message_end", "message": { "role": "toolResult", "content": [{ "type": "text", "text": "no" }] } }), &mut answer), Step::Go);
        assert_eq!(observe(&json!({ "type": "prompt_result", "status": "completed" }), &mut answer), Step::Done(Ok("final".into())));
    }

    #[test]
    fn a_failure_or_silence_is_an_error() {
        let mut answer = None;
        assert_eq!(
            observe(&json!({ "type": "prompt_result", "status": "error", "error": { "message": "429" } }), &mut answer),
            Step::Done(Err("429".into()))
        );
        assert!(matches!(observe(&json!({ "type": "prompt_result", "status": "aborted" }), &mut answer), Step::Done(Err(_))));
        assert!(matches!(observe(&json!({ "type": "response", "command": "prompt", "success": false, "error": "no model" }), &mut answer), Step::Done(Err(_))));
        assert_eq!(observe(&json!({ "type": "response", "command": "prompt", "success": true }), &mut answer), Step::Go);
    }

    #[test]
    fn a_dialog_is_cancelled_and_a_status_line_is_not_a_dialog() {
        let mut answer = None;
        assert_eq!(observe(&json!({ "type": "extension_ui_request", "id": "9", "method": "select" }), &mut answer), Step::Ask("9".into()));
        assert_eq!(observe(&json!({ "type": "extension_ui_request", "id": "1", "method": "setStatus" }), &mut answer), Step::Go);
    }

    #[test]
    fn only_a_few_runs_go_at_once_and_a_slot_comes_back() {
        let mut slots: Vec<_> = (0..MAX_RUNS).map(|_| Slot::take().unwrap()).collect();
        assert!(Slot::take().unwrap_err().contains("wait"));
        drop(slots.pop());
        let replacement = Slot::take().unwrap();
        drop((slots, replacement));
    }

    #[test]
    fn a_bad_task_is_refused_before_anything_starts() {
        let d = TempDir::new("copilot-worker", "task");
        assert!(ask(d.path(), None, "  ").unwrap_err().contains("task"));
        assert!(ask(d.path(), None, &"x".repeat(MAX_TASK_CHARS + 1)).unwrap_err().contains("citekeys"));
    }

    #[test]
    fn the_copilot_cannot_call_itself_but_the_main_session_can() {
        let d = TempDir::new("copilot-worker", "names");
        let names = |v: Value| -> Vec<String> { v.as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap().to_string()).collect() };
        let worker = names(AcademicHost::new(d.path().to_path_buf()).definitions());
        assert!(worker.iter().any(|n| n == "library_group") && !worker.iter().any(|n| n == "copilot_ask"));
        assert!(names(MainHost::new(d.path().to_path_buf(), None).definitions()).iter().any(|n| n == "copilot_ask"));
    }

    #[test]
    fn an_old_rule_word_for_word_gives_way_and_a_users_own_stays() {
        let d = TempDir::new("copilot-worker", "rules");
        let root = d.path();
        std::fs::create_dir_all(root.join(".omp")).unwrap();
        std::fs::write(root.join(".omp/RULES.md"), format!("{OLD_RULE}\n11. Never touch group X.\n")).unwrap();
        let prompt = system_prompt(root);
        assert!(prompt.contains(NEW_RULE), "the rules file: {prompt}");
        assert!(!prompt.contains(OLD_RULE) && prompt.contains("Never touch group X"));
    }
}
