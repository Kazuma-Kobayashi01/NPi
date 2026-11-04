//! The `library_*` commands. Each names a project folder directly under the
//! Academic space; anything else is refused before a database is opened.

use super::{
    add_user_style, cite_draft, export, format as format_entries, import, import_any, styles, Entry, Format, Formatted, Group,
    Cited, ImportReport, IndexOptions, IndexReport, IndexStatus, Library, StyleInfo, Trashed,
};
use super::{attach, check_citations, lookup, pdf_meta};
use crate::academic_space::AcademicSpace;
use serde_json::Value;
use std::path::{Path, PathBuf};
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

/// Ids of the references matching `query` in the project's full-text index (Chinese short words
/// included), best first; `null` while the project has no index (the window then filters by substring).
#[tauri::command]
pub async fn library_search(space: Space<'_>, project: String, query: String) -> Result<Option<Vec<i64>>, String> {
    let dir = project_of(&space, &project)?;
    crate::off_ipc(move || {
        let lib = Library::open(&dir)?;
        let entries = lib.list()?;
        lib.search_library(&entries, &query, 200)
    })
    .await?
}

/// Compares one reference with Crossref (`tools::library_align`): a preview, or with `apply` the blank
/// fields filled (`overwrite` also replaces differing ones). The user's own click in the Librarian window.
#[tauri::command]
pub async fn library_align(space: Space<'_>, project: String, citekey: String, apply: bool, overwrite: bool) -> Result<Value, String> {
    let dir = project_of(&space, &project)?;
    crate::off_ipc(move || {
        let args = serde_json::json!({ "citekey": citekey, "apply": apply, "overwrite": overwrite });
        let text = super::tools::LibraryTools::new(dir)
            .call("library_align", &args, &super::tools::Crossref)
            .ok_or("no such tool")??;
        serde_json::from_str(&text).map_err(|e| e.to_string())
    })
    .await?
}

/// Merges reference `other` into `keep`; `other` goes to the trash.
#[tauri::command]
pub async fn library_merge(space: Space<'_>, project: String, keep: i64, other: i64) -> Result<Entry, String> {
    let dir = project_of(&space, &project)?;
    crate::off_ipc(move || {
        let mut entry = Library::open(&dir)?.merge(keep, other)?;
        attach::mark_missing(&dir, std::slice::from_mut(&mut entry));
        Ok(entry)
    })
    .await?
}

/// Renames a reference's citation key.
#[tauri::command]
pub async fn library_citekey(space: Space<'_>, project: String, id: i64, key: String) -> Result<Entry, String> {
    with_library(&space, &project, move |lib| lib.set_citekey(id, &key)).await
}

/// Registers this app as the project's `npi-librarian` MCP server in `<project>/.mcp.json`, where
/// `npi` in a terminal (and any MCP client) finds it. The user's own action in the Librarian window.
#[tauri::command]
pub async fn library_mcp_register(space: Space<'_>, project: String) -> Result<Value, String> {
    let dir = project_of(&space, &project)?;
    crate::off_ipc(move || {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let path = super::mcp::register(&dir, &exe)?;
        Ok(serde_json::json!({ "path": path.to_string_lossy(), "command": exe.to_string_lossy(), "args": ["--npi-mcp-librarian", dir.to_string_lossy()] }))
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

/// Adds what a pasted text holds, whichever of the known formats it is in.
#[tauri::command]
pub async fn library_import_text(space: Space<'_>, project: String, text: String) -> Result<ImportReport, String> {
    with_library(&space, &project, move |lib| import_any(lib, &text)).await
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

/// The references (all, or those in `ids`) as BibTeX, RIS or CSL-JSON text, for the clipboard.
#[tauri::command]
pub async fn library_export_text(space: Space<'_>, project: String, format: Format, ids: Option<Vec<i64>>) -> Result<String, String> {
    with_library(&space, &project, move |lib| export_text(lib, format, ids.as_deref())).await
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

/// Largest import file read.
const MAX_IMPORT_BYTES: u64 = 16 * 1024 * 1024;

/// Asks for a reference file and imports it, whichever format its content turns out to be in. `None`
/// when the dialog is cancelled.
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
        .add_filter("References", &["ris", "bib", "bibtex", "json", "nbib", "ciw", "txt", "xml"])
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
        if std::fs::metadata(&path).map_err(|e| e.to_string())?.len() > MAX_IMPORT_BYTES {
            return Err("the file is too large".to_string());
        }
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("could not read the file: {e}"))?;
        import_any(&Library::open(&project_dir)?, &text).map(Some)
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

/// Asks for PDFs and returns their paths; the window then takes them one draft at a time, like dropped
/// ones. Nothing is read or added yet. Empty when the dialog is cancelled.
#[tauri::command]
pub async fn library_pdf_pick(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let (tx, rx) = std::sync::mpsc::channel();
    app.dialog()
        .file()
        .set_title("Add PDFs")
        .add_filter("PDF", &["pdf"])
        .pick_files(move |picked| {
            let _ = tx.send(picked);
        });
    crate::off_ipc(move || {
        let picked = rx.recv().map_err(|e| format!("channel error: {e}"))?.unwrap_or_default();
        picked
            .into_iter()
            .map(|p| p.into_path().map(|p| p.to_string_lossy().into_owned()).map_err(|e| format!("invalid picked path: {e}")))
            .collect()
    })
    .await?
}

/// A PDF dropped on the Librarian window or picked in its dialog: staged, and page 1 read for a draft
/// record; nothing enters the library or `papers/` yet. The path comes from the OS's drop event or
/// dialog; it must be a `.pdf` file, and is only read.
#[tauri::command]
pub async fn library_pdf_dropped(space: Space<'_>, project: String, path: String) -> Result<PdfDraft, String> {
    let dir = project_of(&space, &project)?;
    crate::off_ipc(move || {
        let path = std::path::PathBuf::from(path);
        let is_pdf = path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("pdf"));
        if !is_pdf || !path.is_file() {
            return Err("only a PDF file can be added".to_string());
        }
        pdf_draft(&dir, &path)
    })
    .await?
}

/// Stages `path` in the project's inbox and fills a draft from its first page.
fn pdf_draft(dir: &Path, path: &Path) -> Result<PdfDraft, String> {
    let staged = attach::stage(dir, path)?;
    let (guess, scanned) = match crate::pdf_text::first_page(path)? {
        crate::pdf_text::Extracted::Text(text) => (pdf_meta::guess(&text), false),
        crate::pdf_text::Extracted::Scanned => (pdf_meta::guess(""), true),
    };
    let mut csl = guess.csl;
    if csl.get("title").is_none() {
        csl["title"] = Value::String(staged.name.clone());
    }
    Ok(PdfDraft { csl, guessed: guess.guessed, hash: staged.hash, name: staged.name, scanned })
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
        let file = attach::open_target(&dir, &rel)?;
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

/// Finds the file of a missing attachment: first by its content under `papers/` (a rename by hand),
/// else the user picks it, and it is copied back to `papers/`. `None` when the dialog is cancelled.
#[tauri::command]
pub async fn library_attachment_locate(
    app: tauri::AppHandle,
    space: Space<'_>,
    project: String,
    id: i64,
) -> Result<Option<Entry>, String> {
    use tauri_plugin_dialog::DialogExt;
    let dir = project_of(&space, &project)?;
    let found_dir = dir.clone();
    let found = crate::off_ipc(move || {
        let lib = Library::open(&found_dir)?;
        let (ref_id, hash) = lib.attachment_owner(id)?;
        let Some(rel) = attach::find_by_hash(&found_dir, &hash) else { return Ok(None) };
        lib.relink(id, &rel, &hash)?;
        let mut entry = lib.get(ref_id)?.ok_or("library: row vanished")?;
        attach::mark_missing(&found_dir, std::slice::from_mut(&mut entry));
        Ok::<_, String>(Some(entry))
    })
    .await??;
    if found.is_some() {
        return Ok(found);
    }
    let (tx, rx) = std::sync::mpsc::channel();
    app.dialog()
        .file()
        .set_title("Locate the PDF")
        .add_filter("PDF", &["pdf"])
        .pick_file(move |picked| {
            let _ = tx.send(picked);
        });
    crate::off_ipc(move || {
        let Some(picked) = rx.recv().map_err(|e| format!("channel error: {e}"))? else {
            return Ok(None);
        };
        let path = picked.into_path().map_err(|e| format!("invalid picked path: {e}"))?;
        let staged = attach::stage(&dir, &path)?;
        let lib = Library::open(&dir)?;
        let (ref_id, _) = lib.attachment_owner(id)?;
        let mut entry = lib.get(ref_id)?.ok_or("library: row vanished")?;
        let rel = attach::finalize(&dir, &staged.hash, &attach::paper_name(&entry.csl, &staged.name))?;
        lib.relink(id, &rel, &staged.hash)?;
        entry = lib.get(ref_id)?.ok_or("library: row vanished")?;
        attach::mark_missing(&dir, std::slice::from_mut(&mut entry));
        Ok(Some(entry))
    })
    .await?
}

/// Looks a reference up in Crossref, then OpenAlex, by DOI, else by title (and first author's family name).
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

/// References removed from the library, newest removal first.
#[tauri::command]
pub async fn library_trash(space: Space<'_>, project: String) -> Result<Vec<Trashed>, String> {
    with_library(&space, &project, Library::trashed).await
}

/// Brings a removed reference back with its groups, tags, notes and files.
#[tauri::command]
pub async fn library_restore(space: Space<'_>, project: String, id: i64) -> Result<Entry, String> {
    let dir = project_of(&space, &project)?;
    crate::off_ipc(move || {
        let mut entry = Library::open(&dir)?.restore(id)?;
        attach::mark_missing(&dir, std::slice::from_mut(&mut entry));
        Ok(entry)
    })
    .await?
}

/// The user's answer to a Copilot suggestion. Accepting applies it; either way it is closed.
#[tauri::command]
pub async fn library_suggestion_resolve(
    space: Space<'_>,
    project: String,
    id: i64,
    accept: bool,
) -> Result<Entry, String> {
    let dir = project_of(&space, &project)?;
    crate::off_ipc(move || {
        let mut entry = Library::open(&dir)?.resolve_suggestion(id, accept)?;
        attach::mark_missing(&dir, std::slice::from_mut(&mut entry));
        Ok(entry)
    })
    .await?
}

/// Asks for a draft and checks its citations against the library (read only).
/// `None` when the dialog is cancelled.
#[tauri::command]
pub async fn library_check_draft_pick(
    app: tauri::AppHandle,
    space: Space<'_>,
    project: String,
) -> Result<Option<super::agent::Report>, String> {
    use tauri_plugin_dialog::DialogExt;
    let dir = project_of(&space, &project)?;
    let (tx, rx) = std::sync::mpsc::channel();
    app.dialog()
        .file()
        .set_title("Choose a draft to check")
        .add_filter("Draft", &["md", "markdown", "txt", "tex"])
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
        if std::fs::metadata(&path).map_err(|e| e.to_string())?.len() > MAX_DRAFT_BYTES {
            return Err("the draft is too large".to_string());
        }
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("could not read the draft: {e}"))?;
        let entries = Library::open(&dir)?.list()?;
        Ok(Some(check_citations(&entries, &text)))
    })
    .await?
}

/// (Re)indexes a project's papers and, with `text`, its notes, digests and drafts. `rebuild`
/// also rebuilds the search index from the blocks first. The webview names only the project.
#[tauri::command]
pub async fn research_index(
    space: Space<'_>,
    project: String,
    text: Option<bool>,
    rebuild: Option<bool>,
) -> Result<IndexReport, String> {
    let dir = project_of(&space, &project)?;
    crate::off_ipc(move || {
        let lib = Library::open(&dir)?;
        if rebuild == Some(true) {
            lib.rebuild_search()?;
        }
        lib.index_project(&dir, IndexOptions { text: text.unwrap_or(true), max_new_pdfs: None })
    })
    .await?
}

/// What the project's index holds: counts, scanned and stale files, consistency problems.
#[tauri::command]
pub async fn research_index_status(space: Space<'_>, project: String) -> Result<IndexStatus, String> {
    let dir = project_of(&space, &project)?;
    crate::off_ipc(move || Library::open(&dir)?.index_status(&dir)).await?
}

/// The project's indexed files (papers first) for the reader window.
#[tauri::command]
pub async fn research_papers(space: Space<'_>, project: String) -> Result<Vec<super::PaperRow>, String> {
    let dir = project_of(&space, &project)?;
    crate::off_ipc(move || Library::open(&dir)?.papers_list()).await?
}

/// One paper's sections, in reading order.
#[tauri::command]
pub async fn research_sections(space: Space<'_>, project: String, paper_id: i64) -> Result<Vec<super::SectionRow>, String> {
    let dir = project_of(&space, &project)?;
    crate::off_ipc(move || Library::open(&dir)?.paper_sections(paper_id)).await?
}

/// One page of a paper (all of a text file when `page` is none).
#[tauri::command]
pub async fn research_page(space: Space<'_>, project: String, paper_id: i64, page: Option<i64>) -> Result<super::PageView, String> {
    let dir = project_of(&space, &project)?;
    crate::off_ipc(move || Library::open(&dir)?.paper_page(paper_id, page)).await?
}

/// The bytes of an indexed PDF of the project's `papers/` folder, for the reader's PDF view. The
/// webview names the paper by id; the path comes from the project's own index and stays inside the project.
#[tauri::command]
pub async fn research_pdf(space: Space<'_>, project: String, paper_id: i64) -> Result<tauri::ipc::Response, String> {
    let dir = project_of(&space, &project)?;
    crate::off_ipc(move || {
        let (rel, kind) = Library::open(&dir)?.paper_file(paper_id)?;
        if kind != "pdf" || !rel.starts_with("papers/") {
            return Err("not a PDF of this project".to_string());
        }
        let path = super::research::safe_file(&dir, &rel)?;
        let len = std::fs::metadata(&path).map_err(|e| e.to_string())?.len();
        if len > crate::pdf_text::MAX_PDF_BYTES {
            return Err("the PDF is too large to open".to_string());
        }
        std::fs::read(&path).map(tauri::ipc::Response::new).map_err(|e| e.to_string())
    })
    .await?
}

/// Where an anchor lies (its paper, page and block): the jump from a citation to its evidence.
#[tauri::command]
pub async fn research_anchor(space: Space<'_>, project: String, anchor_id: i64) -> Result<Option<super::Target>, String> {
    let dir = project_of(&space, &project)?;
    crate::off_ipc(move || Library::open(&dir)?.anchor_target(anchor_id)).await?
}

/// The summaries saved for one paper (each marked stale when the file changed since).
#[tauri::command]
pub async fn research_summaries(space: Space<'_>, project: String, paper_id: i64) -> Result<Vec<super::SummaryRow>, String> {
    let dir = project_of(&space, &project)?;
    crate::off_ipc(move || Library::open(&dir)?.summaries_for(paper_id)).await?
}

/// The comparison tables saved in the project, newest first.
#[tauri::command]
pub async fn research_comparisons(space: Space<'_>, project: String) -> Result<Vec<super::Comparison>, String> {
    let dir = project_of(&space, &project)?;
    crate::off_ipc(move || Library::open(&dir)?.comparisons()).await?
}

/// Removes one comparison table (the user's own action in the reader).
#[tauri::command]
pub async fn research_comparison_delete(space: Space<'_>, project: String, id: i64) -> Result<(), String> {
    let dir = project_of(&space, &project)?;
    crate::off_ipc(move || Library::open(&dir)?.delete_comparison(id)).await?
}

/// Full-text search over the project's indexed blocks (Chinese short words included).
#[tauri::command]
pub async fn research_search(space: Space<'_>, project: String, query: String, limit: Option<usize>) -> Result<Vec<super::research::Hit>, String> {
    let dir = project_of(&space, &project)?;
    let limit = limit.unwrap_or(40).clamp(1, 100);
    crate::off_ipc(move || Library::open(&dir)?.search_blocks(&query, None, limit)).await?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::academic_space::AcademicSpace;

    fn space(name: &str) -> (crate::test_support::TempDir, AcademicSpace) {
        let d = crate::test_support::TempDir::new("librarian", name);
        std::fs::create_dir_all(d.path().join("Academic/Thesis/notes")).unwrap();
        std::fs::create_dir_all(d.path().join("Academic/Other")).unwrap();
        std::fs::create_dir_all(d.path().join("Academic/.hidden")).unwrap();
        std::fs::create_dir_all(d.path().join("outside/Evil")).unwrap();
        let space = AcademicSpace::new(d.path().join("Academic"));
        (d, space)
    }

    #[test]
    fn a_command_names_a_project_by_a_plain_folder_name_only() {
        let (_d, space) = space("project-names");
        assert!(project_of(&space, "Thesis").is_ok());
        for bad in ["", ".", "..", ".hidden", "../outside/Evil", "Thesis/notes", "Thesis\\notes", "/etc", "missing", "Thesis/.."] {
            assert!(project_of(&space, bad).is_err(), "{bad:?}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_link_in_the_academic_folder_to_a_folder_elsewhere_is_not_a_project() {
        let (d, space) = space("project-link");
        std::os::unix::fs::symlink(d.path().join("outside/Evil"), d.path().join("Academic/Linked")).unwrap();
        assert!(project_of(&space, "Linked").is_err());
    }

    #[test]
    fn two_projects_never_see_each_others_library_or_trash() {
        let (_d, space) = space("project-isolation");
        let a = project_of(&space, "Thesis").unwrap();
        let b = project_of(&space, "Other").unwrap();
        let lib_a = Library::open(&a).unwrap();
        let e = lib_a.add(&serde_json::json!({"title": "Only in Thesis"}), "manual").unwrap();
        lib_a.remove(e.id).unwrap();
        let lib_b = Library::open(&b).unwrap();
        assert!(lib_b.list().unwrap().is_empty());
        assert!(lib_b.trashed().unwrap().is_empty());
        assert!(lib_b.restore(e.id).is_err(), "an id of another project restores nothing");
        assert!(lib_b.resolve_suggestion(1, true).is_err());
        assert_eq!(lib_a.trashed().unwrap().len(), 1);
        assert_ne!(Library::path(&a), Library::path(&b));
    }

}
