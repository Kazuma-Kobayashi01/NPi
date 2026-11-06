//! The interface's own settings (what pages keep in `localStorage`: the tweaks, small
//! per-window choices), also kept in `~/.local/share/npi/desktop/ui-state.json`, so they live
//! beside the command line's data instead of only in the store of `WKWebView`.
//!
//! `localStorage` stays the live copy every window shares. `app/ui-store.js` writes each change
//! through `ui_state_set`, and every page starts with this file's contents as
//! `window.__NPI_UI_STATE__` (an init script built at launch) to put back keys `WebKit` lost. A key
//! `localStorage` has wins over the file, so a window opened later with that launch-time copy
//! only fills gaps and sends back what changed since.

use crate::json_store;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::plugin::{Builder, TauriPlugin};
use tauri::Runtime;

const FILE: &str = "ui-state.json";
const MAX_KEYS: usize = 256;
const MAX_KEY_BYTES: usize = 256;
const MAX_VALUE_BYTES: usize = 64 * 1024;

type Entries = BTreeMap<String, String>;

pub struct UiStateStore {
    /// `None` without a config folder: changes then stay in `localStorage` only.
    path: Option<PathBuf>,
    /// One read-modify-write at a time.
    lock: Mutex<()>,
}

impl UiStateStore {
    pub fn new() -> Self {
        Self {
            path: file_path(),
            lock: Mutex::new(()),
        }
    }
}

fn file_path() -> Option<PathBuf> {
    crate::npi_dirs::config_dir().ok().map(|d| d.join(FILE))
}

fn load(path: &Path) -> Entries {
    json_store::read_or_default(path)
}

/// Apply `changes` (`None` removes a key) within the size limits; nothing changes on an error.
fn apply(entries: &mut Entries, changes: BTreeMap<String, Option<String>>) -> Result<(), String> {
    let mut next = entries.clone(); // all-or-nothing: the original stays if a change is refused
    for (key, value) in changes {
        if key.is_empty() || key.len() > MAX_KEY_BYTES {
            return Err(format!("ui-state key must be 1–{MAX_KEY_BYTES} bytes"));
        }
        match value {
            Some(v) if v.len() > MAX_VALUE_BYTES => {
                return Err(format!(
                    "ui-state value for {key} is over {MAX_VALUE_BYTES} bytes"
                ));
            }
            Some(v) => {
                next.insert(key, v);
            }
            None => {
                next.remove(&key);
            }
        }
    }
    if next.len() > MAX_KEYS {
        return Err(format!("ui-state holds at most {MAX_KEYS} keys"));
    }
    *entries = next;
    Ok(())
}

fn init_script(entries: &Entries) -> String {
    let json = serde_json::to_string(entries).unwrap_or_else(|_| "{}".to_string());
    format!("window.__NPI_UI_STATE__ = {json};")
}

/// Hands every page the saved settings before its own scripts run.
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    let entries = file_path().map_or_else(Entries::new, |p| load(&p));
    Builder::new("ui-state")
        .js_init_script(init_script(&entries))
        .build()
}

#[tauri::command]
pub fn ui_state_set(
    store: tauri::State<'_, UiStateStore>,
    entries: BTreeMap<String, Option<String>>,
) -> Result<(), String> {
    let Some(path) = store.path.as_deref() else {
        return Ok(());
    };
    let _guard = store
        .lock
        .lock()
        .map_err(|_| "ui-state lock poisoned".to_string())?;
    let mut saved = load(path);
    apply(&mut saved, entries)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("ui-state folder: {e}"))?;
    }
    let json = serde_json::to_vec_pretty(&saved).map_err(|e| e.to_string())?;
    json_store::write_atomic(path, &json).map_err(|e| format!("ui-state: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn changes(pairs: &[(&str, Option<&str>)]) -> BTreeMap<String, Option<String>> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), v.map(str::to_string)))
            .collect()
    }

    #[test]
    fn changes_set_and_remove_keys() {
        let mut entries = Entries::new();
        apply(&mut entries, changes(&[("a", Some("1")), ("b", Some("2"))])).unwrap();
        apply(&mut entries, changes(&[("a", None), ("b", Some("3"))])).unwrap();
        assert_eq!(entries, Entries::from([("b".to_string(), "3".to_string())]));
    }

    #[test]
    fn a_refused_change_leaves_everything_as_it_was() {
        let mut entries = Entries::from([("a".to_string(), "1".to_string())]);
        let huge = "x".repeat(MAX_VALUE_BYTES + 1);
        assert!(apply(&mut entries, changes(&[("a", None), ("b", Some(&huge))])).is_err());
        assert_eq!(entries.get("a").map(String::as_str), Some("1"));
        let many: Vec<String> = (0..=MAX_KEYS).map(|i| format!("k{i}")).collect();
        let pairs: Vec<(&str, Option<&str>)> =
            many.iter().map(|k| (k.as_str(), Some("v"))).collect();
        assert!(apply(&mut entries, changes(&pairs)).is_err());
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn the_init_script_hands_pages_the_saved_values_as_json() {
        let entries = Entries::from([("k".to_string(), "\"</script>\u{2028}".to_string())]);
        let script = init_script(&entries);
        let json = script
            .strip_prefix("window.__NPI_UI_STATE__ = ")
            .and_then(|s| s.strip_suffix(';'))
            .unwrap();
        let back: Entries = serde_json::from_str(json).unwrap();
        assert_eq!(back, entries);
    }
}
