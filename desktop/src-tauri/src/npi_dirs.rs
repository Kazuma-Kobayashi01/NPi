//! Where the desktop app keeps its own files, beside the `npi` command line's
//! (`omp/packages/utils/src/dirs.ts`): settings in `~/.local/share/npi/desktop`, the Chat
//! profile and instructions in `~/.local/share/npi` itself, caches in `~/.cache/npi/desktop`.
//! Work the user keeps stays in `~/Documents/NPi` (`workspace_dir`, `worktree.rs`).
//!
//! Earlier builds used Tauri's per-app folders (`~/Library/Application Support/dev.npi.desktop`,
//! `~/Library/Caches/dev.npi.desktop`); [`migrate`] moves what they held once. The webview's own
//! data (`WebKit/`) stays where macOS puts it: `WKWebView` owns it.

use crate::profiles::DATA_ROOT;
use std::path::{Path, PathBuf};

/// The cache root relative to the home folder, shared with the command line.
pub const CACHE_ROOT: &str = ".cache/npi";

/// Files of the old config folder that belong to the data root, not to `desktop/`.
const DATA_ROOT_FILES: [&str; 2] = ["chat-profile.json", "chat-instructions.md"];

/// The old data folder's chat image cache, moved with the other caches.
const CHAT_IMAGES: &str = "chat-images";

/// The chats folder of the oldest builds: old sessions' working directory, so it stays put.
const LEGACY_CHATS: &str = "Chats";

/// The webview's own data in the old cache folder: `WKWebView` keeps using it.
const WEBKIT: &str = "WebKit";

fn home() -> Result<PathBuf, String> {
    std::env::var_os("HOME")
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)
        .filter(|h| h.is_absolute())
        .ok_or_else(|| "HOME is not set to an absolute folder".to_string())
}

/// `~/.local/share/npi`: the command line's data root.
pub fn data_root() -> Result<PathBuf, String> {
    home().map(|h| h.join(DATA_ROOT))
}

/// `~/.local/share/npi/desktop`: the app's settings stores.
pub fn config_dir() -> Result<PathBuf, String> {
    home().map(|h| config_in(&h))
}

/// `~/.cache/npi/desktop`: what the app can rebuild (browser instances, overlays, image cache).
pub fn cache_dir() -> Result<PathBuf, String> {
    home().map(|h| cache_in(&h))
}

fn config_in(home: &Path) -> PathBuf {
    home.join(DATA_ROOT).join("desktop")
}

fn cache_in(home: &Path) -> PathBuf {
    home.join(CACHE_ROOT).join("desktop")
}

/// Move what the old Tauri folders held. Settings go to `config_dir` (the Chat profile and
/// instructions to `data_root`) unless something is already there, so it runs once in effect;
/// old caches are deleted, as they rebuild. Best-effort: an entry that fails stays where it was.
pub fn migrate(legacy_config: Option<&Path>, legacy_cache: Option<&Path>) {
    let Ok(home) = home() else { return };
    migrate_in(&home, legacy_config, legacy_cache);
}

fn migrate_in(home: &Path, legacy_config: Option<&Path>, legacy_cache: Option<&Path>) {
    let config = config_in(home);
    let cache = cache_in(home);
    if let Some(entries) = legacy_config.and_then(|dir| std::fs::read_dir(dir).ok()) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(text) = name.to_str() else { continue };
            let target = match text {
                LEGACY_CHATS => continue,
                CHAT_IMAGES => cache.join(CHAT_IMAGES),
                t if DATA_ROOT_FILES.contains(&t) => home.join(DATA_ROOT).join(t),
                _ => config.join(&name),
            };
            move_unless_present(&entry.path(), &target);
        }
    }
    if let Some(entries) = legacy_cache.and_then(|dir| std::fs::read_dir(dir).ok()) {
        for entry in entries.flatten() {
            if entry.file_name() == WEBKIT {
                continue;
            }
            let path = entry.path();
            let gone = if path.is_dir() {
                std::fs::remove_dir_all(&path)
            } else {
                std::fs::remove_file(&path)
            };
            if let Err(e) = gone {
                eprintln!(
                    "[omp-desktop] old cache {} not removed: {e}",
                    path.display()
                );
            }
        }
    }
}

fn move_unless_present(from: &Path, to: &Path) {
    if to.exists() {
        return;
    }
    let moved = to
        .parent()
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| std::fs::rename(from, to));
    if let Err(e) = moved {
        eprintln!(
            "[omp-desktop] {} not moved to {}: {e}",
            from.display(),
            to.display()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn read(path: &Path) -> String {
        std::fs::read_to_string(path).unwrap()
    }

    #[test]
    fn old_settings_move_beside_the_command_line_and_old_caches_go() {
        let d = TempDir::new("npi-dirs", "migrate");
        let home = d.path();
        let old_config = home.join("Library/Application Support/dev.npi.desktop");
        let old_cache = home.join("Library/Caches/dev.npi.desktop");
        write(&old_config.join("open-tabs.json"), "tabs");
        write(&old_config.join("approval-rules/p.json"), "rules");
        write(&old_config.join("chat-profile.json"), "me");
        write(&old_config.join("chat-images/a.png"), "img");
        write(&old_config.join("Chats/s.jsonl"), "chat");
        write(&old_cache.join("browser/1/pid"), "1");
        write(&old_cache.join("WebKit/x"), "web");

        migrate_in(home, Some(&old_config), Some(&old_cache));

        let config = home.join(".local/share/npi/desktop");
        assert_eq!(read(&config.join("open-tabs.json")), "tabs");
        assert_eq!(read(&config.join("approval-rules/p.json")), "rules");
        assert_eq!(read(&home.join(".local/share/npi/chat-profile.json")), "me");
        assert_eq!(
            read(&home.join(".cache/npi/desktop/chat-images/a.png")),
            "img"
        );
        assert_eq!(read(&old_config.join("Chats/s.jsonl")), "chat");
        assert!(!old_cache.join("browser").exists());
        assert_eq!(read(&old_cache.join("WebKit/x")), "web");
    }

    #[test]
    fn a_setting_already_in_the_new_folder_wins() {
        let d = TempDir::new("npi-dirs", "keeps-new");
        let home = d.path();
        let old_config = home.join("old");
        write(&old_config.join("open-tabs.json"), "old");
        write(&home.join(".local/share/npi/desktop/open-tabs.json"), "new");

        migrate_in(home, Some(&old_config), None);
        migrate_in(home, Some(&old_config), None);

        assert_eq!(
            read(&home.join(".local/share/npi/desktop/open-tabs.json")),
            "new"
        );
        assert_eq!(read(&old_config.join("open-tabs.json")), "old");
    }
}
