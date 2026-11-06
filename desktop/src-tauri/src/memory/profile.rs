//! The model's profile of the user, one per mode: `~/.local/share/npi/USER-CHAT.md`,
//! `USER-CODE.md`, `USER-ACADEMIC.md`. Where the notes (`Store`) hold facts about the user, a
//! profile holds how they like to work in that mode: style, expectations, what to avoid. The model
//! rewrites it whole (`update_user_profile`), within a size limit; Settings → Memory shows and
//! edits it. It sits beside, not instead of, what the user writes in Settings → Account.

use std::path::{Path, PathBuf};

/// Characters a profile may hold; it goes whole into every prompt of its mode.
pub const MAX_CHARS: usize = 1500;

pub const MODES: [&str; 3] = ["chat", "code", "academic"];

fn file_name(mode: &str) -> Result<String, String> {
    if MODES.contains(&mode) {
        Ok(format!("USER-{}.md", mode.to_uppercase()))
    } else {
        Err(format!("`mode` is one of {}", MODES.join(", ")))
    }
}

/// The profile file of `mode` under `root` (the data root).
pub fn path_in(root: &Path, mode: &str) -> Result<PathBuf, String> {
    file_name(mode).map(|name| root.join(name))
}

pub fn read(root: &Path, mode: &str) -> String {
    path_in(root, mode)
        .ok()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .unwrap_or_default()
}

/// Replaces the profile of `mode`; empty text removes it. Refused when it is too long or looks
/// like it holds a key.
pub fn write(root: &Path, mode: &str, text: &str) -> Result<(), String> {
    let path = path_in(root, mode)?;
    let text = text.trim();
    if text.is_empty() {
        return match std::fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(format!("profile: {e}")),
            _ => Ok(()),
        };
    }
    let count = text.chars().count();
    if count > MAX_CHARS {
        return Err(format!("the profile is {count} characters, over {MAX_CHARS}: keep what matters most, merged and short"));
    }
    if super::looks_secret(text) {
        return Err("that looks like a key or token: never keep those in a profile".into());
    }
    crate::private_fs::private_dirs(true)
        .create(root)
        .map_err(|e| format!("profile folder: {e}"))?;
    crate::json_store::write_atomic(&path, format!("{text}\n").as_bytes())
        .map_err(|e| format!("profile: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    #[test]
    fn each_mode_has_its_own_file_and_empty_text_removes_it() {
        let d = TempDir::new("memory-profile", "modes");
        write(d.path(), "chat", "  Prefers short answers in Chinese.  ").unwrap();
        write(d.path(), "academic", "Wants every claim tied to a page.").unwrap();
        assert_eq!(
            read(d.path(), "chat"),
            "Prefers short answers in Chinese.\n"
        );
        assert!(d.path().join("USER-ACADEMIC.md").is_file());
        assert_eq!(read(d.path(), "code"), "");
        write(d.path(), "chat", "").unwrap();
        assert!(!d.path().join("USER-CHAT.md").exists());
        write(d.path(), "chat", "").unwrap();
    }

    #[test]
    fn too_long_a_secret_or_an_unknown_mode_is_refused() {
        let d = TempDir::new("memory-profile", "refuse");
        assert!(write(d.path(), "chat", &"喜".repeat(MAX_CHARS + 1))
            .unwrap_err()
            .contains("over"));
        assert!(
            write(d.path(), "chat", &"喜".repeat(MAX_CHARS)).is_ok(),
            "the limit counts characters"
        );
        assert!(write(d.path(), "chat", "token sk-abcdefghijklmnop1234")
            .unwrap_err()
            .contains("key"));
        assert!(write(d.path(), "../x", "hi").is_err());
    }
}
