//! Screen capture and temporary file management for NPi Desktop.
//! Captures are stored isolated under `/tmp/npi/{session_id}/` and
//! cleaned up when the session or tab is closed.

use std::fs;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenCaptureResult {
    pub path: String,
    pub size_bytes: u64,
    pub timestamp: u64,
}

fn sanitize_session_id(id: &str) -> String {
    let clean: String = id.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    if clean.is_empty() {
        "default".to_string()
    } else {
        clean
    }
}

pub fn session_tmp_dir(session_id: &str) -> PathBuf {
    let clean_id = sanitize_session_id(session_id);
    Path::new("/tmp").join("npi").join(clean_id)
}

#[tauri::command]
pub fn capture_screen_to_tmp(session_id: String) -> Result<ScreenCaptureResult, String> {
    let dir = session_tmp_dir(&session_id);
    fs::create_dir_all(&dir).map_err(|e| format!("failed to create session tmp dir: {e}"))?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let filename = format!("screenshot-{now}.png");
    let target = dir.join(filename);

    #[cfg(target_os = "macos")]
    {
        // -x: muted (no camera shutter sound)
        // -C: capture cursor if supported
        let mut cmd = std::process::Command::new("screencapture");
        cmd.arg("-x").arg("-C").arg(&target);
        let status = crate::spawn_gate::status(&mut cmd).map_err(|e| format!("screencapture failed: {e}"))?;
        if !status.success() {
            return Err("screencapture exited with failure".to_string());
        }
    }

    #[cfg(target_os = "linux")]
    {
        let mut cmd = std::process::Command::new("grim");
        cmd.arg(&target);
        let status = crate::spawn_gate::status(&mut cmd).map_err(|e| format!("grim failed: {e}"))?;
        if !status.success() {
            return Err("grim exited with failure".to_string());
        }
    }

    #[cfg(target_os = "windows")]
    {
        return Err("Windows screen capture via tmp command pending".to_string());
    }

    let meta = fs::metadata(&target).map_err(|e| format!("failed to read capture metadata: {e}"))?;
    let size_bytes = meta.len();
    if size_bytes == 0 {
        let _ = fs::remove_file(&target);
        return Err("captured image was empty".to_string());
    }

    Ok(ScreenCaptureResult {
        path: target.to_string_lossy().to_string(),
        size_bytes,
        timestamp: now,
    })
}

pub fn cleanup_session_tmp(session_id: &str) {
    let dir = session_tmp_dir(session_id);
    if dir.is_dir() {
        let _ = fs::remove_dir_all(&dir);
    }
}
