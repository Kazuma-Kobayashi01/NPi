//! Chat mode's host tool: the hand-off to Code or Academic. (Memory comes from `memory/tools.rs`.)
//!
//! Chat keeps only tools that change nothing (web, reading, UI, memory). When the model
//! judges the request needs real work (files, commands, code, a research
//! project) it calls `handoff_to_work` with a work prompt it wrote. Nothing
//! starts: Rust emits `chat://handoff` and the page shows a card above the
//! composer; the user's click opens a tab in that mode with the prompt in the
//! draft, unsent (`src/app/handoff.js`).

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};

use crate::copilot::{frame, HostTools};

const MAX_PROMPT_CHARS: usize = 8_000;
const MAX_TITLE_CHARS: usize = 80;

pub struct ChatHandoff {
    app: AppHandle,
    session_id: String,
}

impl ChatHandoff {
    pub const fn new(app: AppHandle, session_id: String) -> Self {
        Self { app, session_id }
    }
}

/// The offer a call describes, or why it is refused: `(target, title, prompt)`.
fn parse(args: &Value) -> Result<(&'static str, String, String), String> {
    let target = match args.get("target").and_then(Value::as_str) {
        Some("code") => "code",
        Some("academic") => "academic",
        _ => return Err("target must be \"code\" or \"academic\"".to_string()),
    };
    let prompt = args.get("prompt").and_then(Value::as_str).unwrap_or("").trim();
    if prompt.is_empty() || prompt.chars().count() > MAX_PROMPT_CHARS {
        return Err(format!("prompt must be 1 to {MAX_PROMPT_CHARS} characters"));
    }
    let title: String = args
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .chars()
        .take(MAX_TITLE_CHARS)
        .collect();
    Ok((target, title, prompt.to_string()))
}

impl HostTools for ChatHandoff {
    fn definitions(&self) -> Value {
        json!([{
            "name": "handoff_to_work",
            "label": "Offer hand-off",
            "description": "Offer the user a one-click hand-off of this conversation to a work mode, where the actual work (editing files, running commands, writing code, managing a research project) is done. Chat itself cannot do that work. Call it once, only when the request clearly needs such work and you have gathered what the work needs; target \"code\" for software and file work in a project folder, \"academic\" for literature and research-project work. Write `prompt` as a complete, self-contained work brief for the next agent, in the user's language: goal, relevant facts and decisions from this chat, constraints and preferences, expected deliverable. Nothing starts until the user clicks the card; the brief is put in the new tab's prompt box unsent. After calling, tell the user in one short sentence that the hand-off is ready, and do not start the work yourself.",
            "parameters": {
                "type": "object",
                "properties": {
                    "target": { "type": "string", "enum": ["code", "academic"], "description": "Which work mode takes over." },
                    "title": { "type": "string", "description": "A short label for the card (at most 80 characters)." },
                    "prompt": { "type": "string", "description": "The complete work brief the next agent starts from." }
                },
                "required": ["target", "prompt"],
                "additionalProperties": false
            },
            "loadMode": "essential"
        }])
    }

    fn call(&self, id: &str, tool: &str, args: &Value) -> Value {
        let outcome = match (tool, parse(args)) {
            ("handoff_to_work", Ok((target, title, prompt))) => {
                match self.app.emit(
                    "chat://handoff",
                    json!({ "sessionId": self.session_id, "target": target, "title": title, "prompt": prompt }),
                ) {
                    Ok(()) => Ok("The hand-off card is shown to the user. Tell them it is ready; do not start the work.".to_string()),
                    Err(e) => Err(format!("could not show the hand-off: {e}")),
                }
            }
            ("handoff_to_work", Err(e)) => Err(e),
            (other, _) => Err(format!("unknown tool: {other}")),
        };
        frame(id, outcome)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_two_work_modes_with_a_prompt_are_offered() {
        let ok = parse(&json!({ "target": "code", "title": " Fix it ", "prompt": " do x " })).unwrap();
        assert_eq!(ok, ("code", "Fix it".to_string(), "do x".to_string()));
        assert!(parse(&json!({ "target": "chat", "prompt": "x" })).is_err());
        assert!(parse(&json!({ "target": "academic" })).is_err());
        assert!(parse(&json!({ "target": "academic", "prompt": "  " })).is_err());
        let long = "x".repeat(MAX_PROMPT_CHARS + 1);
        assert!(parse(&json!({ "target": "code", "prompt": long })).is_err());
    }

    #[test]
    fn the_title_is_cut_not_refused() {
        let title = "t".repeat(200);
        let (_, t, _) = parse(&json!({ "target": "code", "title": title, "prompt": "x" })).unwrap();
        assert_eq!(t.chars().count(), MAX_TITLE_CHARS);
    }
}
