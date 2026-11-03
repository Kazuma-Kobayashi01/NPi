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
const MAX_PDF_BYTES: u64 = 64 * 1024 * 1024;
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
    // One page, Helvetica, the text shown at 12 pt; xref offsets computed.
    let stream = format!("BT /F1 12 Tf 72 700 Td ({text}) Tj ET");
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F1 5 0 R >> >> >>".to_string(),
        format!("<< /Length {} >>\nstream\n{stream}\nendstream", stream.len()),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
    ];
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
