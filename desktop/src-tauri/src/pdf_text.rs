//! Text of a PDF, as Markdown, for the Academic tools (search and read in
//! `copilot.rs`). A PDF is untrusted input: it is size-capped, parsed behind
//! `catch_unwind` (a panic on a malformed file must not take the caller's
//! thread down) and a scanned one is reported as such rather than guessed.
//!
//! Extraction takes a noticeable time per page, so the text of a PDF is
//! cached beside the project's other derived data
//! (`.npi-academic/cache/pdf/`), keyed by path, size and modification time.
//! The cache is rebuildable and holds nothing the user wrote.

use pdf_inspector::{PdfError, PdfType};
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use std::panic::AssertUnwindSafe;
use std::path::Path;
use std::time::UNIX_EPOCH;

/// Largest PDF read; a larger file is refused, not partly read.
pub const MAX_PDF_BYTES: u64 = 64 * 1024 * 1024;
const CACHE_DIR: &str = ".npi-academic/cache/pdf";

/// What a PDF gave.
#[derive(Debug, PartialEq, Eq)]
pub enum Extracted {
    Text(String),
    /// No text layer on any page: it needs OCR, which the app does not have.
    Scanned,
}

fn describe(e: &PdfError) -> String {
    match e {
        PdfError::Encrypted => "the PDF is encrypted".to_string(),
        other => format!("the PDF could not be read ({other})"),
    }
}

/// Extracts `path` without the cache.
pub fn extract(path: &Path) -> Result<Extracted, String> {
    let len = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
    if len > MAX_PDF_BYTES {
        return Err("the PDF is too large".to_string());
    }
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let result =
        std::panic::catch_unwind(AssertUnwindSafe(|| pdf_inspector::process_pdf_mem(&bytes)))
            .map_err(|_| "the PDF could not be read (its structure is malformed)".to_string())?
            .map_err(|e| describe(&e))?;
    let markdown = result.markdown.unwrap_or_default();
    if matches!(result.pdf_type, PdfType::Scanned | PdfType::ImageBased)
        || markdown.trim().is_empty()
    {
        return Ok(Extracted::Scanned);
    }
    Ok(Extracted::Text(markdown))
}

/// A PDF page by page, for the Research Core's index.
#[derive(Debug, PartialEq, Eq)]
pub enum Paged {
    /// `(1-based page, Markdown)` of each page that has text, and the pages left out
    /// because they have no reliable text layer (they need OCR).
    Pages {
        pages: Vec<(u32, String)>,
        ocr_pages: Vec<u32>,
        page_count: u32,
    },
    Scanned,
}

/// The text of every page of the PDF `bytes` (no cache: the index is the cache). The caller
/// has checked the size against [`MAX_PDF_BYTES`].
pub fn pages(bytes: &[u8]) -> Result<Paged, String> {
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        pdf_inspector::extract_pages_markdown_mem(bytes, None)
    }))
    .map_err(|_| "the PDF could not be read (its structure is malformed)".to_string())?
    .map_err(|e| describe(&e))?;
    let page_count = u32::try_from(result.pages.len()).unwrap_or(u32::MAX);
    let mut pages = Vec::new();
    let mut ocr_pages = Vec::new();
    for p in result.pages {
        let number = p.page + 1;
        // Garbled text is not indexed either: a quote check on it would be a guess.
        if p.markdown.trim().is_empty() || p.needs_ocr {
            ocr_pages.push(number);
        } else {
            pages.push((number, p.markdown));
        }
    }
    if pages.is_empty() {
        return Ok(Paged::Scanned);
    }
    Ok(Paged::Pages { pages, ocr_pages, page_count })
}

/// The text of page 1 of `path` (the Librarian reads a PDF's title and DOI there); no cache.
pub fn first_page(path: &Path) -> Result<Extracted, String> {
    let len = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
    if len > MAX_PDF_BYTES {
        return Err("the PDF is too large".to_string());
    }
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let options = pdf_inspector::PdfOptions::new().pages([1]);
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        pdf_inspector::process_pdf_mem_with_options(&bytes, options)
    }))
    .map_err(|_| "the PDF could not be read (its structure is malformed)".to_string())?
    .map_err(|e| describe(&e))?;
    let markdown = result.markdown.unwrap_or_default();
    if markdown.trim().is_empty() {
        return Ok(Extracted::Scanned);
    }
    Ok(Extracted::Text(markdown))
}

/// The cache file of `path` (project-relative `rel`): `<hash of rel>.<size>-<mtime>.md`.
fn cache_name(rel: &str, path: &Path) -> Option<(String, String)> {
    let meta = std::fs::metadata(path).ok()?;
    let mtime = meta
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_secs();
    let digest = Sha256::digest(rel.as_bytes());
    let mut stem = String::new();
    for b in &digest[..8] {
        let _ = write!(stem, "{b:02x}");
    }
    let name = format!("{stem}.{}-{mtime}.md", meta.len());
    Some((stem, name))
}

/// A cache entry stores a scanned PDF as an empty file.
fn from_cache(text: String) -> Extracted {
    if text.is_empty() {
        Extracted::Scanned
    } else {
        Extracted::Text(text)
    }
}

/// The cached text of `path` in `project`, if it is current.
pub fn cached(project: &Path, rel: &str, path: &Path) -> Option<Extracted> {
    let (_, name) = cache_name(rel, path)?;
    std::fs::read_to_string(project.join(CACHE_DIR).join(name))
        .ok()
        .map(from_cache)
}

/// Extracts `path` and caches the outcome (older entries of the same file go).
pub fn extract_cached(project: &Path, rel: &str, path: &Path) -> Result<Extracted, String> {
    if let Some(hit) = cached(project, rel, path) {
        return Ok(hit);
    }
    let out = extract(path)?;
    if let Some((stem, name)) = cache_name(rel, path) {
        let dir = project.join(CACHE_DIR);
        if std::fs::create_dir_all(&dir).is_ok() {
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for old in entries.flatten().filter(|e| {
                    e.file_name()
                        .to_string_lossy()
                        .starts_with(&format!("{stem}."))
                }) {
                    let _ = std::fs::remove_file(old.path());
                }
            }
            let body = match &out {
                Extracted::Text(t) => t.as_str(),
                Extracted::Scanned => "",
            };
            let _ = std::fs::write(dir.join(name), body);
        }
    }
    Ok(out)
}

#[cfg(test)]
pub fn tiny_pdf(text: &str) -> Vec<u8> {
    pages_pdf(&[text])
}

/// A PDF with one page per entry of `texts` (Helvetica, shown at 12 pt; xref offsets computed).
#[cfg(test)]
pub fn pages_pdf(texts: &[&str]) -> Vec<u8> {
    // Objects: 1 catalog, 2 pages, 3 font, then a page and its content stream per text.
    let kids: Vec<String> = (0..texts.len()).map(|i| format!("{} 0 R", 4 + 2 * i)).collect();
    let mut objects = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        format!("<< /Type /Pages /Kids [{}] /Count {} >>", kids.join(" "), texts.len()),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
    ];
    for (i, text) in texts.iter().enumerate() {
        let stream = format!("BT /F1 12 Tf 72 700 Td ({text}) Tj ET");
        objects.push(format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents {} 0 R /Resources << /Font << /F1 3 0 R >> >> >>", 5 + 2 * i));
        objects.push(format!("<< /Length {} >>\nstream\n{stream}\nendstream", stream.len()));
    }
    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (i, body) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend(format!("{} 0 obj\n{body}\nendobj\n", i + 1).bytes());
    }
    let xref = out.len();
    out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).bytes());
    for o in offsets {
        out.extend(format!("{o:010} 00000 n \n").bytes());
    }
    out.extend(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .bytes(),
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    #[test]
    fn a_text_pdf_gives_its_words() {
        let d = TempDir::new("pdf-text", "words");
        let p = d.path().join("a.pdf");
        std::fs::write(&p, tiny_pdf("Attention is all you need")).unwrap();
        let Extracted::Text(t) = extract(&p).unwrap() else {
            panic!("expected text")
        };
        assert!(t.contains("Attention is all you need"), "{t}");
    }

    #[test]
    fn pages_are_numbered_from_one() {
        let Paged::Pages { pages, page_count, .. } = pages(&tiny_pdf("page one words")).unwrap() else {
            panic!("expected text")
        };
        assert_eq!((page_count, pages[0].0), (1, 1));
        assert!(pages[0].1.contains("page one words"));
    }

    #[test]
    fn junk_and_missing_files_are_errors_not_panics() {
        let d = TempDir::new("pdf-text", "junk");
        let p = d.path().join("junk.pdf");
        std::fs::write(&p, b"%PDF-1.4\nnot really\n").unwrap();
        assert!(extract(&p).is_err());
        std::fs::write(&p, b"hello").unwrap();
        assert!(extract(&p).is_err());
        assert!(extract(&d.path().join("missing.pdf")).is_err());
    }

    #[test]
    fn the_cache_is_used_while_the_file_is_unchanged_and_replaced_when_it_changes() {
        let d = TempDir::new("pdf-text", "cache");
        let p = d.path().join("papers/a.pdf");
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, tiny_pdf("first version")).unwrap();
        assert!(cached(d.path(), "papers/a.pdf", &p).is_none());
        let first = extract_cached(d.path(), "papers/a.pdf", &p).unwrap();
        assert!(matches!(&first, Extracted::Text(t) if t.contains("first version")));
        assert_eq!(cached(d.path(), "papers/a.pdf", &p), Some(first));
        // A different file (size changes) misses, and the old entry is dropped.
        std::fs::write(&p, tiny_pdf("second version, longer")).unwrap();
        assert!(cached(d.path(), "papers/a.pdf", &p).is_none());
        let second = extract_cached(d.path(), "papers/a.pdf", &p).unwrap();
        assert!(matches!(&second, Extracted::Text(t) if t.contains("second version")));
        let entries = std::fs::read_dir(d.path().join(CACHE_DIR)).unwrap().count();
        assert_eq!(entries, 1);
    }
}
