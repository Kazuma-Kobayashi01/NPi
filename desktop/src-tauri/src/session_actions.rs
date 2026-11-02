//! Sidebar context-menu actions that need the file system: deleting a saved
//! conversation and showing a folder in Finder.

use std::path::Path;
use std::sync::Arc;
use tauri::State;

/// Validates that `path` is an absolute session file located strictly within
/// `profile`'s sessions root (with `profile` resolved via `ProfileStore`,
/// where None or "default" resolves to built-in).
fn validate_session_path(
    app: &tauri::AppHandle,
    store: &crate::profiles::ProfileStore,
    path: &str,
    profile: Option<&str>,
) -> Result<(), String> {
    if !Path::new(path).is_absolute() {
        return Err("a session file path is required".to_string());
    }
    let resolved = store.resolve(profile)?;
    crate::saved_sessions::validate_resume(app, path, resolved)
}

/// Deletes a saved conversation for good: a `.jsonl` file inside the sessions
/// root, checked by `validate_session_path`.
#[tauri::command]
pub fn delete_saved_session(
    path: String,
    profile: Option<String>,
    store: State<'_, Arc<crate::profiles::ProfileStore>>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    validate_session_path(&app, &store, &path, profile.as_deref())?;
    std::fs::remove_file(&path).map_err(|e| format!("could not delete: {e}"))
}

/// Deletes multiple saved conversations: validates each path against the sessions
/// root before removing. Returns the list of successfully deleted file paths.
#[tauri::command]
pub fn batch_delete_saved_sessions(
    paths: Vec<String>,
    profile: Option<String>,
    store: State<'_, Arc<crate::profiles::ProfileStore>>,
    app: tauri::AppHandle,
) -> Result<Vec<String>, String> {
    let resolved = store.resolve(profile.as_deref())?;
    let mut deleted = Vec::new();
    for p in paths {
        if !Path::new(&p).is_absolute() {
            continue;
        }
        if crate::saved_sessions::validate_resume(&app, &p, resolved).is_err() {
            continue;
        }
        if std::fs::remove_file(&p).is_ok() {
            deleted.push(p);
        }
    }
    Ok(deleted)
}

/// Renames a saved conversation by appending a title record to the session file.
#[tauri::command]
pub fn rename_saved_session(
    path: String,
    new_title: String,
    profile: Option<String>,
    store: State<'_, Arc<crate::profiles::ProfileStore>>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let trimmed = new_title.trim();
    if trimmed.is_empty() {
        return Err("title cannot be empty".to_string());
    }
    validate_session_path(&app, &store, &path, profile.as_deref())?;
    crate::saved_sessions::rename_session_file(Path::new(&path), trimmed)
        .map_err(|e| format!("could not rename: {e}"))
}
#[derive(Debug, PartialEq, Eq)]
struct ExportMessage {
    role: String,
    timestamp: String,
    text: String,
}

fn extract_all_content_text(content: Option<&serde_json::Value>) -> String {
    let Some(content) = content else {
        return String::new();
    };
    if let Some(text) = content.as_str() {
        return text.trim().to_string();
    }
    if let Some(arr) = content.as_array() {
        let mut parts = Vec::new();
        for block in arr {
            if let Some(s) = block.as_str() {
                let trimmed = s.trim();
                if !trimmed.is_empty() {
                    parts.push(trimmed.to_string());
                }
            } else if block.get("type").and_then(|t| t.as_str()) == Some("text") {
                if let Some(text) = block.get("text").and_then(|s| s.as_str()) {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        parts.push(trimmed.to_string());
                    }
                }
            }
        }
        return parts.join("\n\n");
    }
    String::new()
}

fn format_session_markdown(
    title: &str,
    session_id: &str,
    timestamp: &str,
    cwd: &str,
    messages: &[ExportMessage],
) -> String {
    let mut out = String::new();
    out.push_str(&format!("# {title}\n\n"));

    out.push_str("> **Session Overview**\n");
    if !session_id.is_empty() {
        out.push_str(&format!("> - **ID**: `{session_id}`\n"));
    }
    if !timestamp.is_empty() {
        out.push_str(&format!("> - **Date**: {timestamp}\n"));
    }
    if !cwd.is_empty() {
        out.push_str(&format!("> - **Workspace**: `{cwd}`\n"));
    }
    out.push_str(&format!("> - **Messages**: {}\n\n", messages.len()));

    for msg in messages {
        out.push_str("---\n\n");
        let (icon_role, header_title) = if msg.role == "user" {
            ("👤", "User")
        } else {
            ("🤖", "Assistant")
        };

        if !msg.timestamp.is_empty() {
            out.push_str(&format!("### {icon_role} {header_title} · {}\n\n", msg.timestamp));
        } else {
            out.push_str(&format!("### {icon_role} {header_title}\n\n"));
        }

        out.push_str(&msg.text);
        out.push_str("\n\n");
    }

    out
}

/// Exports a saved conversation to a structured Markdown file.
#[tauri::command]
pub fn export_session_markdown(
    path: String,
    output_path: String,
    profile: Option<String>,
    store: State<'_, Arc<crate::profiles::ProfileStore>>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    if !Path::new(&path).is_absolute() {
        return Err("a session file path is required".to_string());
    }
    let out = Path::new(&output_path);
    if !out.is_absolute() {
        return Err("output path must be absolute".to_string());
    }
    validate_session_path(&app, &store, &path, profile.as_deref())?;
    let file = std::fs::File::open(&path).map_err(|e| format!("could not open session file: {e}"))?;
    let reader = std::io::BufReader::new(file);

    let mut session_id = String::new();
    let mut explicit_title = String::new();
    let mut fallback_title = String::new();
    let mut session_timestamp = String::new();
    let mut cwd = String::new();
    let mut messages: Vec<ExportMessage> = Vec::new();

    use std::io::BufRead;
    for line_res in reader.lines() {
        let Ok(line) = line_res else { continue };
        if line.trim().is_empty() {
            continue;
        }
        let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        let event_type = val.get("type").and_then(|t| t.as_str()).unwrap_or("");
        match event_type {
            "title" => {
                if let Some(t) = val.get("title").and_then(|s| s.as_str()) {
                    let trimmed = t.trim();
                    if !trimmed.is_empty() {
                        explicit_title = trimmed.to_string();
                    }
                }
            }
            "session" => {
                if let Some(id) = val.get("id").and_then(|s| s.as_str()) {
                    session_id = id.to_string();
                }
                if let Some(ts) = val.get("timestamp").and_then(|s| s.as_str()) {
                    session_timestamp = ts.to_string();
                }
                if let Some(dir) = val.get("cwd").and_then(|s| s.as_str()) {
                    cwd = dir.to_string();
                }
            }
            "message" => {
                let Some(msg) = val.get("message") else { continue };
                let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("");
                if role != "user" && role != "assistant" {
                    continue;
                }
                let content_text = extract_all_content_text(msg.get("content"));
                if content_text.trim().is_empty() {
                    continue;
                }
                if role == "user" && fallback_title.is_empty() {
                    fallback_title = if content_text.chars().count() > 60 {
                        format!("{}…", content_text.chars().take(60).collect::<String>())
                    } else {
                        content_text.clone()
                    };
                }
                let ts = if let Some(s) = val.get("timestamp").and_then(|s| s.as_str()) {
                    s.to_string()
                } else if let Some(s) = msg.get("timestamp").and_then(|s| s.as_str()) {
                    s.to_string()
                } else if let Some(num) = msg.get("timestamp").and_then(|n| n.as_i64()) {
                    let secs = (num / 1000) as u64;
                    let millis = (num % 1000) as u32;
                    crate::saved_sessions::format_epoch_iso(secs, millis)
                } else {
                    String::new()
                };

                messages.push(ExportMessage {
                    role: role.to_string(),
                    timestamp: ts,
                    text: content_text,
                });
            }
            _ => {}
        }
    }

    let title = if !explicit_title.is_empty() {
        explicit_title
    } else if !fallback_title.is_empty() {
        fallback_title
    } else {
        "Untitled Conversation".to_string()
    };

    let md = format_session_markdown(
        &title,
        &session_id,
        &session_timestamp,
        &cwd,
        &messages,
    );

    if let Some(parent) = out.parent() {
        if !parent.as_os_str().is_empty() && !parent.exists() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("could not create parent directory: {e}"))?;
        }
    }

    std::fs::write(out, md).map_err(|e| format!("could not write markdown: {e}"))?;
    Ok(())
}

/// Shows an existing absolute path in Finder.
#[tauri::command]
pub fn reveal_path(path: String) -> Result<(), String> {
    let p = Path::new(&path);
    if !p.is_absolute() || !p.exists() {
        return Err(format!("no such path: {path}"));
    }
    let mut cmd = std::process::Command::new("open");
    cmd.arg("-R").arg(p);
    crate::spawn_gate::status(&mut cmd)
        .map_err(|e| format!("open: {e}"))
        .and_then(|s| if s.success() { Ok(()) } else { Err("Finder could not open it".to_string()) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reveal_refuses_a_relative_or_missing_path() {
        assert!(reveal_path("relative".into()).is_err());
        assert!(reveal_path("/definitely/not/here-npi".into()).is_err());
    }

    #[test]
    fn test_extract_all_content_text_string() {
        let val = serde_json::json!("Hello world");
        assert_eq!(extract_all_content_text(Some(&val)), "Hello world");
    }

    #[test]
    fn test_extract_all_content_text_blocks() {
        let val = serde_json::json!([
            {"type": "text", "text": "Part 1"},
            {"type": "image", "url": "data:..."},
            {"type": "text", "text": "Part 2"}
        ]);
        assert_eq!(extract_all_content_text(Some(&val)), "Part 1\n\nPart 2");
    }

    #[test]
    fn test_format_session_markdown() {
        let messages = vec![
            ExportMessage {
                role: "user".to_string(),
                timestamp: "2026-09-04T00:00:01Z".to_string(),
                text: "Hello".to_string(),
            },
            ExportMessage {
                role: "assistant".to_string(),
                timestamp: "2026-09-04T00:00:02Z".to_string(),
                text: "Hi there!".to_string(),
            },
        ];
        let md = format_session_markdown("Test Chat", "sess-1", "2026-09-04T00:00:00Z", "/repo", &messages);
        assert!(md.contains("# Test Chat"));
        assert!(md.contains("> - **ID**: `sess-1`"));
        assert!(md.contains("### 👤 User · 2026-09-04T00:00:01Z"));
        assert!(md.contains("### 🤖 Assistant · 2026-09-04T00:00:02Z"));
        assert!(md.contains("Hi there!"));
    }
}
