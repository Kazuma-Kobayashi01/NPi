//! Sidebar context-menu actions that need the file system: deleting a saved
//! conversation and showing a folder in Finder.

use std::path::Path;

/// Deletes a saved conversation for good: a `.jsonl` file inside the sessions
/// root, checked by `saved_sessions::validate_resume`. The app asks first.
#[tauri::command]
pub fn delete_saved_session(
    path: String,
    profile: Option<String>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    // A bare session id is valid for resume, but only a file can be deleted.
    if !Path::new(&path).is_absolute() {
        return Err("a session file path is required".to_string());
    }
    crate::saved_sessions::validate_resume(&app, &path, profile.as_deref())?;
    std::fs::remove_file(&path).map_err(|e| format!("could not delete: {e}"))
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
}
