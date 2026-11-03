//! The `library_*` commands. Each names a project folder directly under the
//! Academic space; anything else is refused before a database is opened.

use super::{
    add_user_style, cite_draft, export, format as format_entries, import, styles, Entry, Format, Formatted, Group,
    Cited, ImportReport, Library, StyleInfo,
};
use super::{attach, lookup, pdf_meta};
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
    let dir = project_of(&space, &project)?;
    crate::off_ipc(move || {
        let mut entries = Library::open(&dir)?.list()?;
        attach::mark_missing(&dir, &mut entries);
        Ok(entries)
    })
    .await?
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

/// The folder of the user's own `.csl` styles.
fn styles_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    use tauri::Manager;
    app.path()
        .app_config_dir()
        .map(|d| d.join("csl-styles"))
        .map_err(|e| format!("no config folder: {e}"))
}

/// Built-in citation styles, then the user's own.
#[tauri::command]
pub async fn library_styles(app: tauri::AppHandle) -> Result<Vec<StyleInfo>, String> {
    let dir = styles_dir(&app)?;
    crate::off_ipc(move || styles(&dir)).await
}

/// The references (all, or those in `ids`) as a reference list and in-text citations in `style`.
#[tauri::command]
pub async fn library_format(
    app: tauri::AppHandle,
    space: Space<'_>,
    project: String,
    style: String,
    ids: Option<Vec<i64>>,
) -> Result<Formatted, String> {
    let dir = styles_dir(&app)?;
    with_library(&space, &project, move |lib| {
        let mut entries = lib.list()?;
        if let Some(ids) = &ids {
            entries.retain(|e| ids.contains(&e.id));
        }
        format_entries(&dir, &style, &entries)
    })
    .await
}

/// Asks for a `.csl` file, checks it and adds it to the user's styles. `None` when cancelled.
#[tauri::command]
pub async fn library_style_add_pick(app: tauri::AppHandle) -> Result<Option<StyleInfo>, String> {
    use tauri_plugin_dialog::DialogExt;
    let dir = styles_dir(&app)?;
    let (tx, rx) = std::sync::mpsc::channel();
    app.dialog()
        .file()
        .set_title("Add a citation style")
        .add_filter("CSL style", &["csl", "xml"])
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
        let size = std::fs::metadata(&path).map_err(|e| e.to_string())?.len();
        if size > super::format::MAX_STYLE_BYTES as u64 {
            return Err("the style file is too large".to_string());
        }
        let xml =
            std::fs::read_to_string(&path).map_err(|e| format!("could not read the file: {e}"))?;
        add_user_style(&dir, &xml).map(Some)
    })
    .await?
}

/// What `library_cite_draft` did with a draft.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DraftResult {
    pub path: String,
    pub cited: usize,
    pub missing: Vec<String>,
}

/// Largest draft read.
const MAX_DRAFT_BYTES: u64 = 4 * 1024 * 1024;

/// Asks for a Markdown or text draft, replaces its `[@citekey]` markers with in-text citations in
/// `style`, appends the reference list and asks where to save the result. `None` when a dialog
/// is cancelled. The draft is only read; the result is a new file.
#[tauri::command]
pub async fn library_cite_draft(
    app: tauri::AppHandle,
    space: Space<'_>,
    project: String,
    style: String,
) -> Result<Option<DraftResult>, String> {
    use tauri_plugin_dialog::DialogExt;
    let dir = styles_dir(&app)?;
    let (tx, rx) = std::sync::mpsc::channel();
    app.dialog()
        .file()
        .set_title("Choose a draft")
        .add_filter("Draft", &["md", "markdown", "txt"])
        .pick_file(move |picked| {
            let _ = tx.send(picked);
        });
    let picked = crate::off_ipc(move || {
        let Some(picked) = rx.recv().map_err(|e| format!("channel error: {e}"))? else {
            return Ok::<_, String>(None);
        };
        let path = picked
            .into_path()
            .map_err(|e| format!("invalid picked path: {e}"))?;
        if std::fs::metadata(&path).map_err(|e| e.to_string())?.len() > MAX_DRAFT_BYTES {
            return Err("the draft is too large".to_string());
        }
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("could not read the draft: {e}"))?;
        Ok(Some((path, text)))
    })
    .await??;
    let Some((source, text)) = picked else { return Ok(None) };
    let cited: Cited = with_library(&space, &project, move |lib| {
        cite_draft(&dir, &style, &lib.list()?, &text)
    })
    .await?;
    let stem = source.file_stem().and_then(|s| s.to_str()).unwrap_or("draft");
    let (tx, rx) = std::sync::mpsc::channel();
    app.dialog()
        .file()
        .set_title("Save the cited draft")
        .set_file_name(format!("{stem}.cited.md"))
        .add_filter("Markdown", &["md"])
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
        std::fs::write(&path, &cited.text).map_err(|e| format!("could not write the file: {e}"))?;
        Ok(Some(DraftResult {
            path: path.to_string_lossy().into_owned(),
            cited: cited.cited,
            missing: cited.missing,
        }))
    })
    .await?
}

/// What a PDF gave: a draft record to confirm, and the staged file it belongs to.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfDraft {
    pub csl: Value,
    /// Fields the rules filled; the form marks them "check".
    pub guessed: Vec<&'static str>,
    /// Content hash of the staged copy; `library_add_pdf` / `library_attach_pdf` take it back.
    pub hash: String,
    /// The PDF's file name without extension.
    pub name: String,
    /// No text layer: nothing was read from the page.
    pub scanned: bool,
}

/// Asks for a PDF, stages a copy and reads page 1 for a draft record. Nothing enters the
/// library or `papers/` yet. `None` when the dialog is cancelled.
#[tauri::command]
pub async fn library_pdf_pick(
    app: tauri::AppHandle,
    space: Space<'_>,
    project: String,
) -> Result<Option<PdfDraft>, String> {
    use tauri_plugin_dialog::DialogExt;
    let dir = project_of(&space, &project)?;
    let (tx, rx) = std::sync::mpsc::channel();
    app.dialog()
        .file()
        .set_title("Add a PDF")
        .add_filter("PDF", &["pdf"])
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
        let staged = attach::stage(&dir, &path)?;
        let (guess, scanned) = match crate::pdf_text::first_page(&path)? {
            crate::pdf_text::Extracted::Text(text) => (pdf_meta::guess(&text), false),
            crate::pdf_text::Extracted::Scanned => (pdf_meta::guess(""), true),
        };
        let mut csl = guess.csl;
        if csl.get("title").is_none() {
            csl["title"] = Value::String(staged.name.clone());
        }
        Ok(Some(PdfDraft {
            csl,
            guessed: guess.guessed,
            hash: staged.hash,
            name: staged.name,
            scanned,
        }))
    })
    .await?
}

/// The outcome of adding a confirmed PDF draft.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddPdfResult {
    pub entry: Option<Entry>,
    /// The reference this repeats (DOI, or title + year + first author); nothing was added.
    pub duplicate: Option<Entry>,
}

/// Adds the confirmed draft `csl` and files the staged PDF `hash` under `papers/`.
/// A repeat is reported, not added.
#[tauri::command]
pub async fn library_add_pdf(
    space: Space<'_>,
    project: String,
    csl: Value,
    hash: String,
) -> Result<AddPdfResult, String> {
    let dir = project_of(&space, &project)?;
    crate::off_ipc(move || {
        let lib = Library::open(&dir)?;
        if let Some(existing) = lib.duplicate_of(&csl)? {
            return Ok(AddPdfResult { entry: None, duplicate: Some(existing) });
        }
        let rel = attach::finalize(&dir, &hash, &attach::paper_name(&csl, "paper"))?;
        let entry = lib.add(&csl, "pdf")?;
        lib.attach(entry.id, &rel, &hash, "pdf")?;
        let mut entry = lib.get(entry.id)?.ok_or("library: row vanished")?;
        attach::mark_missing(&dir, std::slice::from_mut(&mut entry));
        Ok(AddPdfResult { entry: Some(entry), duplicate: None })
    })
    .await?
}

/// Files the staged PDF `hash` under `papers/` and attaches it to reference `id`.
#[tauri::command]
pub async fn library_attach_pdf(
    space: Space<'_>,
    project: String,
    id: i64,
    hash: String,
) -> Result<Entry, String> {
    let dir = project_of(&space, &project)?;
    crate::off_ipc(move || {
        let lib = Library::open(&dir)?;
        let entry = lib.get(id)?.ok_or("no such reference")?;
        let rel = attach::finalize(&dir, &hash, &attach::paper_name(&entry.csl, "paper"))?;
        lib.attach(id, &rel, &hash, "pdf")?;
        let mut entry = lib.get(id)?.ok_or("library: row vanished")?;
        attach::mark_missing(&dir, std::slice::from_mut(&mut entry));
        Ok(entry)
    })
    .await?
}

/// Asks for a PDF and attaches it to reference `id`. `None` when the dialog is cancelled.
#[tauri::command]
pub async fn library_attach_pick(
    app: tauri::AppHandle,
    space: Space<'_>,
    project: String,
    id: i64,
) -> Result<Option<Entry>, String> {
    use tauri_plugin_dialog::DialogExt;
    let dir = project_of(&space, &project)?;
    let (tx, rx) = std::sync::mpsc::channel();
    app.dialog()
        .file()
        .set_title("Attach a PDF")
        .add_filter("PDF", &["pdf"])
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
        let staged = attach::stage(&dir, &path)?;
        let lib = Library::open(&dir)?;
        let entry = lib.get(id)?.ok_or("no such reference")?;
        let rel = attach::finalize(&dir, &staged.hash, &attach::paper_name(&entry.csl, &staged.name))?;
        lib.attach(id, &rel, &staged.hash, "pdf")?;
        let mut entry = lib.get(id)?.ok_or("library: row vanished")?;
        attach::mark_missing(&dir, std::slice::from_mut(&mut entry));
        Ok(Some(entry))
    })
    .await?
}

/// Opens an attached file with the system's viewer. Only a file inside the project opens.
#[tauri::command]
pub async fn library_attachment_open(
    space: Space<'_>,
    project: String,
    id: i64,
) -> Result<(), String> {
    let dir = project_of(&space, &project)?;
    crate::off_ipc(move || {
        let rel = Library::open(&dir)?
            .attachment_path(id)?
            .ok_or("no such attachment")?;
        let root = dir.canonicalize().map_err(|e| e.to_string())?;
        let file = dir
            .join(&rel)
            .canonicalize()
            .map_err(|_| "the file is missing".to_string())?;
        if !file.starts_with(&root) || !file.is_file() {
            return Err("that file is outside the project".to_string());
        }
        let _gate = crate::spawn_gate::shared();
        #[allow(clippy::disallowed_methods)] // under the gate
        let opened = open::that_detached(&file);
        opened.map_err(|e| format!("could not open the file: {e}"))
    })
    .await?
}

/// Forgets an attachment (the file stays in the project).
#[tauri::command]
pub async fn library_attachment_remove(
    space: Space<'_>,
    project: String,
    id: i64,
) -> Result<bool, String> {
    with_library(&space, &project, move |lib| lib.detach(id)).await
}

/// Looks a reference up in Crossref by DOI, else by title (and first author's family name).
/// Only runs when the user asks; sends nothing but those strings.
#[tauri::command]
pub async fn library_lookup(
    doi: Option<String>,
    title: Option<String>,
    author: Option<String>,
) -> Result<Vec<lookup::Candidate>, String> {
    crate::off_ipc(move || {
        let doi = doi.unwrap_or_default();
        let title = title.unwrap_or_default();
        if !doi.trim().is_empty() {
            return lookup::by_doi(&doi);
        }
        if title.trim().chars().count() < 8 {
            return Err("enter a DOI, or a title of at least a few words".to_string());
        }
        lookup::by_title(title.trim(), author.unwrap_or_default().trim())
    })
    .await?
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
