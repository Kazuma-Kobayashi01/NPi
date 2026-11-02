//! Conversation-private image cache: integrity-checked reads, atomic writes
//! and a global size bound.
//!
//! Layout (unchanged from the first release, so existing caches keep working):
//! `<root>/<conversation-hash>/<source-hash>.bin` plus a `.meta` sidecar.
//! Entries written before this module gained a content hash have no `version`
//! and no `sha256`; they are accepted only after size, MIME signature and
//! source validation, and the manager rewrites their metadata in the
//! background. A corrupt entry is a cache miss, never an error surfaced to the
//! webview: the image is fetched again and atomically replaces it.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::network::{detect_image_mime, MAX_IMAGE_BYTES};

/// Total bytes of cached image payloads across every conversation. Older
/// entries (by file mtime) are evicted first once a write crosses the bound.
pub(super) const MAX_CACHE_BYTES: u64 = 256 * 1024 * 1024;

/// Serializes writers and eviction. Reads are lock-free: every file in the
/// cache is written through `write_atomic`, so a reader sees either the old or
/// the new complete file, never a partial one.
static CACHE_WRITE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CachedImageMeta {
    /// 0 (absent) = pre-integrity cache entry.
    #[serde(default)]
    version: u32,
    source: String,
    mime_type: String,
    size: u64,
    #[serde(default)]
    sha256: String,
}

pub(super) struct CachedAsset {
    pub bytes: Vec<u8>,
    pub mime_type: String,
    pub sha256: String,
    /// Parsed from a metadata sidecar without `version`/`sha256`.
    pub legacy: bool,
}

fn bin_path(cache_root: &Path, conv_hash: &str, source_hash: &str) -> PathBuf {
    cache_root
        .join(conv_hash)
        .join(format!("{source_hash}.bin"))
}

fn meta_path(cache_root: &Path, conv_hash: &str, source_hash: &str) -> PathBuf {
    cache_root
        .join(conv_hash)
        .join(format!("{source_hash}.meta"))
}

/// Read and validate one entry. `Ok(None)` covers absence and every form of
/// corruption or mismatch (wrong source, size, MIME signature, digest): the
/// caller treats it as a miss and refetches.
pub(super) fn read(
    cache_root: &Path,
    conv_hash: &str,
    source_hash: &str,
    source: &str,
) -> Result<Option<CachedAsset>, String> {
    let meta_bytes = match std::fs::read(meta_path(cache_root, conv_hash, source_hash)) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("failed to read image cache metadata: {e}")),
    };
    let Ok(meta) = serde_json::from_slice::<CachedImageMeta>(&meta_bytes) else {
        return Ok(None);
    };

    // The hash names the source, but a planted or colliding sidecar must not
    // make one image's bytes stand in for another's.
    if meta.source.as_str() != source {
        return Ok(None);
    }
    if meta.size > MAX_IMAGE_BYTES {
        return Ok(None);
    }

    // Check the payload's declared size before reading it: a corrupt or
    // replaced file must never make this load an unbounded buffer.
    let payload = bin_path(cache_root, conv_hash, source_hash);
    match std::fs::metadata(&payload) {
        Ok(metadata) if metadata.len() == meta.size => {}
        Ok(_) => return Ok(None),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("failed to inspect cached image bytes: {e}")),
    }

    let bytes = match std::fs::read(&payload) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("failed to read cached image bytes: {e}")),
    };

    if bytes.len() as u64 != meta.size {
        return Ok(None);
    }
    let Some(detected) = detect_image_mime(&bytes) else {
        return Ok(None);
    };
    if detected != meta.mime_type.as_str() {
        return Ok(None);
    }

    let legacy = meta.version < 2;
    let sha256 = crate::json_store::hex_sha256(&bytes);
    if !legacy && meta.sha256 != sha256 {
        return Ok(None);
    }

    Ok(Some(CachedAsset {
        bytes,
        mime_type: meta.mime_type,
        sha256,
        legacy,
    }))
}

/// Persist `bytes` then prune the global cache back under [`MAX_CACHE_BYTES`].
/// Runs on a blocking worker: never on the first-display path.
pub(super) fn persist(
    cache_root: &Path,
    conv_hash: &str,
    source_hash: &str,
    source: &str,
    mime_type: &str,
    bytes: Option<&[u8]>,
    sha256: &str,
) -> Result<(), String> {
    if bytes.is_some_and(|bytes| bytes.len() as u64 > MAX_IMAGE_BYTES) {
        return Err("refusing to cache an image over 8 MiB".to_string());
    }

    let _guard = CACHE_WRITE_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    let conv_dir = cache_root.join(conv_hash);
    crate::private_fs::private_dirs(true)
        .create(&conv_dir)
        .map_err(|e| format!("failed to create image cache directory: {e}"))?;

    if let Some(bytes) = bytes {
        crate::json_store::write_atomic(&bin_path(cache_root, conv_hash, source_hash), bytes)
            .map_err(|e| format!("failed to write cached image bytes: {e}"))?;
    }

    let meta = CachedImageMeta {
        version: 2,
        source: source.to_string(),
        mime_type: mime_type.to_string(),
        size: bytes.map_or_else(
            || {
                // Metadata upgrade: keep the size already on disk.
                std::fs::metadata(bin_path(cache_root, conv_hash, source_hash))
                    .map_or(0, |m| m.len())
            },
            |bytes| bytes.len() as u64,
        ),
        sha256: sha256.to_string(),
    };
    let meta_json = serde_json::to_vec(&meta)
        .map_err(|e| format!("failed to serialize image cache metadata: {e}"))?;
    crate::json_store::write_atomic(&meta_path(cache_root, conv_hash, source_hash), &meta_json)
        .map_err(|e| format!("failed to write image cache metadata: {e}"))?;

    // Eviction failure leaves a valid entry in place; only the bound is lost,
    // so it is logged rather than failing the request's background write.
    if let Err(e) = enforce_limit(cache_root, MAX_CACHE_BYTES) {
        eprintln!("[chat-image] cache eviction failed: {e}");
    }
    Ok(())
}

/// Delete the oldest payloads until their total fits `limit`. Only `.bin`
/// files count; each evicted payload takes its sidecar with it. The entry just
/// written is the newest, so it is the last to go.
fn enforce_limit(cache_root: &Path, limit: u64) -> std::io::Result<()> {
    let mut entries: Vec<(SystemTime, PathBuf, u64)> = Vec::new();
    let mut total = 0u64;

    let conversations = match std::fs::read_dir(cache_root) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    for conversation in conversations {
        let conversation = conversation?;
        if !conversation.file_type()?.is_dir() {
            continue;
        }
        for file in std::fs::read_dir(conversation.path())? {
            let file = file?;
            let path = file.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("bin") {
                continue;
            }
            let Ok(metadata) = file.metadata() else {
                continue;
            };
            total = total.saturating_add(metadata.len());
            entries.push((
                metadata.modified().unwrap_or(UNIX_EPOCH),
                path,
                metadata.len(),
            ));
        }
    }

    if total <= limit {
        return Ok(());
    }
    entries.sort_by_key(|(modified, _, _)| *modified);
    for (_, path, size) in entries {
        if total <= limit {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            total = total.saturating_sub(size);
            let _ = std::fs::remove_file(path.with_extension("meta"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    const FAKE_PNG: &[u8] = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR";

    fn hash(value: &str) -> String {
        crate::json_store::hex_sha256(value.as_bytes())
    }

    fn legacy_meta(dir: &Path, conv: &str, src: &str, source: &str, mime: &str, size: u64) {
        std::fs::create_dir_all(dir.join(conv)).unwrap();
        std::fs::write(dir.join(conv).join(format!("{src}.bin")), FAKE_PNG).unwrap();
        let meta = format!(r#"{{"source":"{source}","mimeType":"{mime}","size":{size}}}"#);
        std::fs::write(dir.join(conv).join(format!("{src}.meta")), meta).unwrap();
    }

    #[test]
    fn round_trip_writes_integrity_metadata_and_reloads() {
        let temp = TempDir::new("chat-image-cache", "round-trip");
        let conv = hash("profile-1:session-abc");
        let source = "https://example.com/test.png";
        let src = hash(source);
        let sha = crate::json_store::hex_sha256(FAKE_PNG);

        persist(
            temp.path(),
            &conv,
            &src,
            source,
            "image/png",
            Some(FAKE_PNG),
            &sha,
        )
        .unwrap();

        let cached = read(temp.path(), &conv, &src, source).unwrap().unwrap();
        assert_eq!(cached.mime_type, "image/png");
        assert_eq!(cached.bytes.as_slice(), FAKE_PNG);
        assert_eq!(cached.sha256, sha);
        assert!(!cached.legacy);
    }

    #[test]
    fn corrupt_payload_is_a_miss_not_a_wrong_image() {
        let temp = TempDir::new("chat-image-cache", "corrupt");
        let conv = hash("c1");
        let source = "https://example.com/a.png";
        let src = hash(source);
        let sha = crate::json_store::hex_sha256(FAKE_PNG);
        persist(
            temp.path(),
            &conv,
            &src,
            source,
            "image/png",
            Some(FAKE_PNG),
            &sha,
        )
        .unwrap();

        // Bit flip of the same length: size matches, the digest and signature
        // checks must still reject it.
        let payload = temp.path().join(&conv).join(format!("{src}.bin"));
        let mut bytes = std::fs::read(&payload).unwrap();
        bytes[8] ^= 0xff;
        std::fs::write(&payload, bytes).unwrap();
        assert!(read(temp.path(), &conv, &src, source).unwrap().is_none());

        // Truncation (size mismatch) and a lost sidecar are misses too.
        std::fs::write(&payload, &FAKE_PNG[..4]).unwrap();
        assert!(read(temp.path(), &conv, &src, source).unwrap().is_none());
        std::fs::remove_file(temp.path().join(&conv).join(format!("{src}.meta"))).unwrap();
        assert!(read(temp.path(), &conv, &src, source).unwrap().is_none());
    }

    #[test]
    fn source_or_mime_mismatch_never_hits() {
        let temp = TempDir::new("chat-image-cache", "mismatch");
        let conv = hash("c1");
        let source = "https://example.com/a.png";
        let src = hash(source);
        let sha = crate::json_store::hex_sha256(FAKE_PNG);
        persist(
            temp.path(),
            &conv,
            &src,
            source,
            "image/png",
            Some(FAKE_PNG),
            &sha,
        )
        .unwrap();

        assert!(
            read(temp.path(), &conv, &src, "https://example.com/other.png")
                .unwrap()
                .is_none()
        );

        // Sidecar MIME does not match the bytes' actual signature.
        let meta = temp.path().join(&conv).join(format!("{src}.meta"));
        let forged = format!(
            r#"{{"version":2,"source":"{source}","mimeType":"image/jpeg","size":{},"sha256":"{sha}"}}"#,
            FAKE_PNG.len()
        );
        std::fs::write(&meta, forged).unwrap();
        assert!(read(temp.path(), &conv, &src, source).unwrap().is_none());
    }

    #[test]
    fn legacy_entries_are_accepted_without_a_digest_then_upgraded() {
        let temp = TempDir::new("chat-image-cache", "legacy");
        let conv = hash("c1");
        let source = "diagram.png";
        let src = hash(source);
        legacy_meta(
            temp.path(),
            &conv,
            &src,
            source,
            "image/png",
            FAKE_PNG.len() as u64,
        );

        let cached = read(temp.path(), &conv, &src, source).unwrap().unwrap();
        assert!(cached.legacy);
        assert_eq!(cached.bytes.as_slice(), FAKE_PNG);

        persist(
            temp.path(),
            &conv,
            &src,
            source,
            "image/png",
            None,
            &cached.sha256,
        )
        .unwrap();
        let upgraded = read(temp.path(), &conv, &src, source).unwrap().unwrap();
        assert!(!upgraded.legacy);
        assert_eq!(upgraded.bytes.as_slice(), FAKE_PNG);
    }

    #[test]
    fn eviction_removes_oldest_payloads_until_under_the_bound() {
        let temp = TempDir::new("chat-image-cache", "evict");
        let conv = hash("c1");
        for (index, source) in ["a.png", "b.png", "c.png"].iter().enumerate() {
            let src = hash(source);
            let sha = crate::json_store::hex_sha256(FAKE_PNG);
            persist(
                temp.path(),
                &conv,
                &src,
                source,
                "image/png",
                Some(FAKE_PNG),
                &sha,
            )
            .unwrap();
            // Distinct mtimes so eviction order is deterministic.
            let payload = temp.path().join(&conv).join(format!("{src}.bin"));
            let file = std::fs::OpenOptions::new()
                .write(true)
                .open(&payload)
                .unwrap();
            file.set_modified(UNIX_EPOCH + std::time::Duration::from_secs(1_000 + index as u64))
                .unwrap();
        }

        let keep_two = (FAKE_PNG.len() as u64) * 2;
        enforce_limit(temp.path(), keep_two).unwrap();

        let remaining: Vec<_> = std::fs::read_dir(temp.path().join(&conv))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| {
                Path::new(name)
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("bin"))
            })
            .collect();
        assert_eq!(remaining.len(), 2);
        let oldest = temp
            .path()
            .join(&conv)
            .join(format!("{}.bin", hash("a.png")));
        assert!(!oldest.exists());
        let oldest_meta = temp
            .path()
            .join(&conv)
            .join(format!("{}.meta", hash("a.png")));
        assert!(!oldest_meta.exists());
    }

    #[test]
    fn oversized_payloads_are_refused() {
        let temp = TempDir::new("chat-image-cache", "oversize");
        let payload = vec![0u8; usize::try_from(MAX_IMAGE_BYTES).unwrap() + 1];
        let sha = crate::json_store::hex_sha256(b"x");
        let error = persist(
            temp.path(),
            "conv",
            "src",
            "source",
            "image/png",
            Some(&payload),
            &sha,
        )
        .unwrap_err();
        assert!(error.contains("8 MiB"));
    }
}
