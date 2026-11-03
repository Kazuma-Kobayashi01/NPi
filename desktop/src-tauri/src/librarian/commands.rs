//! The `library_*` commands. Each names a project folder directly under the
//! Academic space; anything else is refused before a database is opened.

use super::{export, import, Entry, Format, ImportReport, Library};
use crate::academic_space::AcademicSpace;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;

type Space<'a> = State<'a, Arc<AcademicSpace>>;

fn project_of(space: &AcademicSpace, name: &str) -> Result<PathBuf, String> {
    space
        .project_named(name)
        .ok_or_else(|| format!("no Academic project called \"{name}\""))
}

/// Opens `name`'s library on a blocking thread and runs `work` on it.
async fn with_library<T: Send + 'static>(
    space: &Space<'_>,
    name: &str,
    work: impl FnOnce(&Library) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    let project = project_of(space, name)?;
    crate::off_ipc(move || work(&Library::open(&project)?)).await?
}

#[tauri::command]
pub async fn library_list(space: Space<'_>, project: String) -> Result<Vec<Entry>, String> {
    with_library(&space, &project, Library::list).await
}

#[tauri::command]
pub async fn library_add(space: Space<'_>, project: String, csl: Value) -> Result<Entry, String> {
    with_library(&space, &project, move |lib| lib.add(&csl, "manual")).await
}

#[tauri::command]
pub async fn library_update(
    space: Space<'_>,
    project: String,
    id: i64,
    csl: Value,
) -> Result<Entry, String> {
    with_library(&space, &project, move |lib| lib.update(id, &csl)).await
}

#[tauri::command]
pub async fn library_remove(space: Space<'_>, project: String, id: i64) -> Result<bool, String> {
    with_library(&space, &project, move |lib| lib.remove(id)).await
}

/// Adds what `text` holds that the library lacks; repeats are reported, not added.
#[tauri::command]
pub async fn library_import(
    space: Space<'_>,
    project: String,
    format: Format,
    text: String,
) -> Result<ImportReport, String> {
    with_library(&space, &project, move |lib| import(lib, format, &text)).await
}

/// The library (or the references in `ids`) as RIS, BibTeX or CSL-JSON text.
#[tauri::command]
pub async fn library_export(
    space: Space<'_>,
    project: String,
    format: Format,
    ids: Option<Vec<i64>>,
) -> Result<String, String> {
    with_library(&space, &project, move |lib| {
        let mut entries = lib.list()?;
        if let Some(ids) = ids {
            entries.retain(|e| ids.contains(&e.id));
        }
        Ok(export(format, &entries))
    })
    .await
}
