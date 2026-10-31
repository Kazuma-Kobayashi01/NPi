//! Sidebar context-menu actions that need the file system: moving a saved
//! conversation to the Trash and showing a folder in Finder.

use std::path::{Path, PathBuf};

/// The Trash name for `file`: its own file name, suffixed while taken.
fn trash_target(trash: &Path, file: &Path) -> Result<PathBuf, String> {
    let name = file.file_name().and_then(|n| n.to_str()).ok_or("bad file name")?;
    let mut target = trash.join(name);
    let mut n = 1;
    while target.exists() {
        target = trash.join(format!("{n}-{name}"));
        n += 1;
    }
    Ok(target)
}

/// Moves a saved conversation (a `.jsonl` inside the sessions root, checked by
/// `saved_sessions::validate_resume`) to the user's Trash, so it can be put
/// back from there.
#[tauri::command]
pub fn delete_saved_session(
    path: String,
    profile: Option<String>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    use tauri::Manager;
    // A bare session id is valid for resume, but only a file can be trashed.
    if !Path::new(&path).is_absolute() {
        return Err("a session file path is required".to_string());
    }
    crate::saved_sessions::validate_resume(&app, &path, profile.as_deref())?;
    let trash = app
        .path()
        .home_dir()
        .map_err(|e| format!("home folder: {e}"))?
        .join(".Trash");
    let target = trash_target(&trash, Path::new(&path))?;
    std::fs::rename(&path, &target).map_err(|e| format!("could not move to the Trash: {e}"))
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
    fn a_taken_trash_name_gets_a_prefix() {
        let dir = std::env::temp_dir().join(format!("npi-trash-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.jsonl"), "x").unwrap();
        let t = trash_target(&dir, Path::new("/s/a.jsonl")).unwrap();
        assert_eq!(t.file_name().unwrap(), "1-a.jsonl");
        let free = trash_target(&dir, Path::new("/s/b.jsonl")).unwrap();
        assert_eq!(free.file_name().unwrap(), "b.jsonl");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reveal_refuses_a_relative_or_missing_path() {
        assert!(reveal_path("relative".into()).is_err());
        assert!(reveal_path("/definitely/not/here-npi".into()).is_err());
    }
}
