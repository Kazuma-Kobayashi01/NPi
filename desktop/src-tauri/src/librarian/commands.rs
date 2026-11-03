//! The `library_*` commands. Each names a project folder directly under the
//! Academic space; anything else is refused before a database is opened.

use super::{export, import, Entry, Format, Group, ImportReport, Library};
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

fn export_text(lib: &Library, format: Format, ids: Option<&[i64]>) -> Result<String, String> {
    let mut entries = lib.list()?;
    if let Some(ids) = ids {
        entries.retain(|e| ids.contains(&e.id));
    }
    Ok(export(format, &entries))
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
        export_text(lib, format, ids.as_deref())
    })
    .await
}

/// Asks where to save, then writes the export there. `None` when the dialog is cancelled.
#[tauri::command]
pub async fn library_export_save(
    app: tauri::AppHandle,
    space: Space<'_>,
    project: String,
    format: Format,
    ids: Option<Vec<i64>>,
) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let text = with_library(&space, &project, move |lib| {
        export_text(lib, format, ids.as_deref())
    })
    .await?;
    let (ext, label) = match format {
        Format::Ris => ("ris", "RIS"),
        Format::Bibtex => ("bib", "BibTeX"),
        Format::CslJson => ("json", "CSL-JSON"),
    };
    let (tx, rx) = std::sync::mpsc::channel();
    app.dialog()
        .file()
        .set_title("Export references")
        .set_file_name(format!("references.{ext}"))
        .add_filter(label, &[ext])
        .save_file(move |picked| {
            let _ = tx.send(picked);
        });
    crate::off_ipc(move || {
        let Some(picked) = rx.recv().map_err(|e| format!("channel error: {e}"))? else {
            return Ok(None);
        };
        let path = picked
            .into_path()
            .map_err(|e| format!("invalid picked path: {e}"))?;
        std::fs::write(&path, text).map_err(|e| format!("could not write the file: {e}"))?;
        Ok(Some(path.to_string_lossy().into_owned()))
    })
    .await?
}

/// The format a file name says it is in.
fn format_of(path: &std::path::Path) -> Option<Format> {
    match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "ris" => Some(Format::Ris),
        "bib" | "bibtex" => Some(Format::Bibtex),
        "json" => Some(Format::CslJson),
        _ => None,
    }
}

/// Largest import file read.
const MAX_IMPORT_BYTES: u64 = 16 * 1024 * 1024;

/// Asks for a `.ris`, `.bib` or `.json` file and imports it. `None` when the dialog is cancelled.
#[tauri::command]
pub async fn library_import_pick(
    app: tauri::AppHandle,
    space: Space<'_>,
    project: String,
) -> Result<Option<ImportReport>, String> {
    use tauri_plugin_dialog::DialogExt;
    let project_dir = project_of(&space, &project)?;
    let (tx, rx) = std::sync::mpsc::channel();
    app.dialog()
        .file()
        .set_title("Import references")
        .add_filter("References", &["ris", "bib", "bibtex", "json"])
        .pick_file(move |picked| {
            let _ = tx.send(picked);
        });
    crate::off_ipc(move || {
        let Some(picked) = rx.recv().map_err(|e| format!("channel error: {e}"))? else {
            return Ok(None);
        };
        let path = picked
            .into_path()
            .map_err(|e| format!("invalid picked path: {e}"))?;
        let format = format_of(&path).ok_or("that file is not .ris, .bib or .json")?;
        if std::fs::metadata(&path).map_err(|e| e.to_string())?.len() > MAX_IMPORT_BYTES {
            return Err("the file is too large".to_string());
        }
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("could not read the file: {e}"))?;
        import(&Library::open(&project_dir)?, format, &text).map(Some)
    })
    .await?
}

#[tauri::command]
pub async fn library_groups(space: Space<'_>, project: String) -> Result<Vec<Group>, String> {
    with_library(&space, &project, Library::groups).await
}

#[tauri::command]
pub async fn library_group_add(
    space: Space<'_>,
    project: String,
    name: String,
) -> Result<Group, String> {
    with_library(&space, &project, move |lib| lib.add_group(&name)).await
}

#[tauri::command]
pub async fn library_group_rename(
    space: Space<'_>,
    project: String,
    id: i64,
    name: String,
) -> Result<(), String> {
    with_library(&space, &project, move |lib| lib.rename_group(id, &name)).await
}

/// Removes a group and its memberships; the references stay.
#[tauri::command]
pub async fn library_group_remove(
    space: Space<'_>,
    project: String,
    id: i64,
) -> Result<bool, String> {
    with_library(&space, &project, move |lib| lib.remove_group(id)).await
}

#[tauri::command]
pub async fn library_member(
    space: Space<'_>,
    project: String,
    id: i64,
    group: i64,
    member: bool,
) -> Result<Entry, String> {
    with_library(&space, &project, move |lib| {
        lib.set_member(id, group, member)?;
        lib.get(id)?.ok_or_else(|| "no such reference".to_string())
    })
    .await
}

/// Replaces a reference's tags and notes.
#[tauri::command]
pub async fn library_meta(
    space: Space<'_>,
    project: String,
    id: i64,
    tags: Vec<String>,
    notes: String,
) -> Result<Entry, String> {
    with_library(&space, &project, move |lib| lib.set_meta(id, &tags, &notes)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn the_extension_names_the_format() {
        assert_eq!(format_of(Path::new("a/refs.RIS")), Some(Format::Ris));
        assert_eq!(format_of(Path::new("refs.bib")), Some(Format::Bibtex));
        assert_eq!(format_of(Path::new("refs.json")), Some(Format::CslJson));
        assert_eq!(format_of(Path::new("refs.txt")), None);
        assert_eq!(format_of(Path::new("refs")), None);
    }
}
