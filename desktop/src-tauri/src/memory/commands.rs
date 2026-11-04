//! The `memory_*` commands of Settings. Each opens the one store; the webview names a note by id.

use super::{Note, Store};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryState {
    pub enabled: bool,
    pub path: String,
    pub notes: Vec<Note>,
}

fn state() -> Result<MemoryState, String> {
    let store = Store::open_default()?;
    Ok(MemoryState { enabled: store.enabled(), path: Store::default_path()?.to_string_lossy().into_owned(), notes: store.list()? })
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
