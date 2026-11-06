//! The `memory_*` commands of Settings. Each opens the one store; the webview names a note by id and
//! a profile by mode.

use super::{profile, Note, Store, Usage};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryState {
    pub enabled: bool,
    /// Code and Academic sessions write skills from what worked (omp's autolearn) while Memory is on.
    pub skill_learning: bool,
    pub path: String,
    pub notes: Vec<Note>,
    pub usage: Usage,
    /// The profile of each mode (`chat`, `code`, `academic`), and the limit they share.
    pub profiles: BTreeMap<&'static str, String>,
    pub profile_limit: usize,
}

fn state() -> Result<MemoryState, String> {
    let store = Store::open_default()?;
    let root = crate::npi_dirs::data_root()?;
    Ok(MemoryState {
        enabled: store.enabled(),
        skill_learning: store.skill_learning(),
        path: Store::default_path()?.to_string_lossy().into_owned(),
        notes: store.list()?,
        usage: store.usage()?,
        profiles: profile::MODES.iter().map(|m| (*m, profile::read(&root, m).trim().to_string())).collect(),
        profile_limit: profile::MAX_CHARS,
    })
}

#[tauri::command]
pub async fn memory_state() -> Result<MemoryState, String> {
    crate::off_ipc(state).await?
}

#[tauri::command]
pub async fn memory_set_enabled(on: bool) -> Result<MemoryState, String> {
    crate::off_ipc(move || {
        Store::open_default()?.set_enabled(on)?;
        state()
    })
    .await?
}

#[tauri::command]
pub async fn memory_set_skill_learning(on: bool) -> Result<MemoryState, String> {
    crate::off_ipc(move || {
        Store::open_default()?.set_skill_learning(on)?;
        state()
    })
    .await?
}

/// The user's own edit of a mode's profile; empty text clears it.
#[tauri::command]
pub async fn memory_set_profile(mode: String, text: String) -> Result<MemoryState, String> {
    crate::off_ipc(move || {
        profile::write(&crate::npi_dirs::data_root()?, &mode, &text)?;
        state()
    })
    .await?
}

#[tauri::command]
pub async fn memory_update(id: i64, text: String) -> Result<MemoryState, String> {
    crate::off_ipc(move || {
        Store::open_default()?.update(id, &text)?;
        state()
    })
    .await?
}

#[tauri::command]
pub async fn memory_delete(id: i64) -> Result<MemoryState, String> {
    crate::off_ipc(move || {
        Store::open_default()?.delete(id)?;
        state()
    })
    .await?
}

#[tauri::command]
pub async fn memory_clear() -> Result<MemoryState, String> {
    crate::off_ipc(|| {
        Store::open_default()?.clear()?;
        state()
    })
    .await?
}
