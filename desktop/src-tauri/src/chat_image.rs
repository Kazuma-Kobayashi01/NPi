//! Chat image loader and conversation-private persistent storage.
//!
//! Provides the core caching and loading logic for the `chat_image_read` IPC command:
//! - Local project files read via [`crate::project_file`], rooted in the tab folder.
//! - Remote HTTPS downloads validated against public-only network rules in [`network`].
//! - Persists successful assets under user-private storage per hashed conversation key
//!   and source with atomic writes, ensuring offline availability even if the original
//!   project file is later deleted.

mod network;

use std::path::Path;

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use serde::{Deserialize, Serialize};

use network::{detect_image_mime, fetch_public_https_image, MAX_IMAGE_BYTES};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ChatImageResult {
    pub data: String,
    pub mime_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CachedImageMeta {
    source: String,
    mime_type: String,
    size: u64,
}

fn read_cache(
    cache_root: &Path,
    conv_hash: &str,
    source_hash: &str,
) -> Result<Option<ChatImageResult>, String> {
    let conv_dir = cache_root.join(conv_hash);
    let bin_path = conv_dir.join(format!("{source_hash}.bin"));
    let meta_path = conv_dir.join(format!("{source_hash}.meta"));

    let meta_bytes = match std::fs::read(&meta_path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("failed to read image cache metadata: {e}")),
    };
    let meta: CachedImageMeta = serde_json::from_slice(&meta_bytes)
        .map_err(|e| format!("corrupt image cache metadata: {e}"))?;

    let bytes = match std::fs::read(&bin_path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("failed to read cached image bytes: {e}")),
    };

    if bytes.len() as u64 != meta.size {
        return Err(format!(
            "cached image byte size mismatch (expected {}, got {})",
            meta.size,
            bytes.len()
        ));
    }

    Ok(Some(ChatImageResult {
        data: STANDARD.encode(&bytes),
        mime_type: meta.mime_type,
    }))
}

fn write_cache(
    cache_root: &Path,
    conv_hash: &str,
    source_hash: &str,
    source: &str,
    mime_type: &str,
    bytes: &[u8],
) -> Result<(), String> {
    let conv_dir = cache_root.join(conv_hash);
    crate::private_fs::private_dirs(true)
        .create(&conv_dir)
        .map_err(|e| format!("failed to create image cache directory: {e}"))?;

    let bin_path = conv_dir.join(format!("{source_hash}.bin"));
    let meta_path = conv_dir.join(format!("{source_hash}.meta"));

    crate::json_store::write_atomic(&bin_path, bytes)
        .map_err(|e| format!("failed to write cached image bytes: {e}"))?;

    let meta = CachedImageMeta {
        source: source.to_string(),
        mime_type: mime_type.to_string(),
        size: bytes.len() as u64,
    };
    let meta_json = serde_json::to_vec(&meta)
        .map_err(|e| format!("failed to serialize image cache metadata: {e}"))?;

    crate::json_store::write_atomic(&meta_path, &meta_json)
        .map_err(|e| format!("failed to write cached image metadata: {e}"))?;

    Ok(())
}

fn read_data_url(source: &str) -> Result<ChatImageResult, String> {
    let rest = source
        .strip_prefix("data:")
        .ok_or_else(|| "not a data URL".to_string())?;
    let (header, b64_data) = rest
        .split_once(',')
        .ok_or_else(|| "malformed data URL".to_string())?;

    if !header.ends_with(";base64") {
        return Err("only base64 data URLs are supported".to_string());
    }

    let trimmed_b64 = b64_data.trim();
    if trimmed_b64.len() > (MAX_IMAGE_BYTES as usize).saturating_mul(4) / 3 + 4 {
        return Err("data URL image exceeds 8 MiB limit".to_string());
    }

    let bytes = STANDARD
        .decode(trimmed_b64)
        .map_err(|e| format!("invalid base64 in data URL: {e}"))?;

    if bytes.len() as u64 > MAX_IMAGE_BYTES {
        return Err("data URL image exceeds 8 MiB limit".to_string());
    }

    let detected_mime = detect_image_mime(&bytes)
        .ok_or_else(|| "unsupported image format in data URL".to_string())?;

    Ok(ChatImageResult {
        data: STANDARD.encode(&bytes),
        mime_type: detected_mime.to_string(),
    })
}

pub fn read_chat_image(
    cache_root: &Path,
    project_root: Option<&Path>,
    conversation_key: &str,
    source: &str,
) -> Result<ChatImageResult, String> {
    if conversation_key.is_empty() {
        return Err("conversationKey must not be empty".to_string());
    }
    if source.is_empty() {
        return Err("image source must not be empty".to_string());
    }

    let conv_hash = crate::json_store::hex_sha256(conversation_key.as_bytes());
    let source_hash = crate::json_store::hex_sha256(source.as_bytes());

    if let Some(cached) = read_cache(cache_root, &conv_hash, &source_hash)? {
        return Ok(cached);
    }

    if source.starts_with("data:") {
        return read_data_url(source);
    }

    if source.starts_with("https://") {
        let (bytes, mime) = fetch_public_https_image(source)?;
        write_cache(cache_root, &conv_hash, &source_hash, source, mime, &bytes)?;
        return Ok(ChatImageResult {
            data: STANDARD.encode(&bytes),
            mime_type: mime.to_string(),
        });
    }

    if source.starts_with("http://") {
        return Err("only HTTPS images are permitted; plain HTTP is forbidden".to_string());
    }

    let root = project_root.ok_or_else(|| "this tab has no project folder".to_string())?;
    let preview = crate::project_file::read(root, source)?;
    match preview.kind {
        crate::project_file::PreviewKind::Image => {
            let data = preview.data.expect("Image preview contract guarantees data");
            let mime = preview
                .mime_type
                .expect("Image preview contract guarantees mimeType");
            let bytes = STANDARD
                .decode(&data)
                .map_err(|e| format!("failed to decode image data: {e}"))?;

            write_cache(cache_root, &conv_hash, &source_hash, source, mime, &bytes)?;

            Ok(ChatImageResult {
                data,
                mime_type: mime.to_string(),
            })
        }
        crate::project_file::PreviewKind::Missing => {
            Err(format!("image file not found: {source}"))
        }
        crate::project_file::PreviewKind::Outside => {
            Err(format!("image path resolves outside project folder: {source}"))
        }
        crate::project_file::PreviewKind::TooLarge => {
            Err(format!("image exceeds maximum size of 8 MiB: {source}"))
        }
        other => Err(format!("file is not an image ({other:?}): {source}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_round_trip_stores_and_reloads_image() {
        let temp_dir = std::env::temp_dir().join(format!(
            "npi_img_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let conv_key = "profile-1:session-abc";
        let source = "https://example.com/test.png";
        let fake_png = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR";

        let conv_hash = crate::json_store::hex_sha256(conv_key.as_bytes());
        let source_hash = crate::json_store::hex_sha256(source.as_bytes());

        write_cache(
            &temp_dir,
            &conv_hash,
            &source_hash,
            source,
            "image/png",
            fake_png,
        )
        .unwrap();

        let cached = read_cache(&temp_dir, &conv_hash, &source_hash)
            .unwrap()
            .unwrap();
        assert_eq!(cached.mime_type, "image/png");
        assert_eq!(cached.data, STANDARD.encode(fake_png));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn read_chat_image_caches_project_file_and_survives_deletion() {
        let root = std::env::temp_dir().join(format!(
            "npi_proj_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let cache = root.join("cache");
        let project = root.join("project");
        std::fs::create_dir_all(&project).unwrap();

        let img_path = project.join("diagram.png");
        let fake_png = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR";
        std::fs::write(&img_path, fake_png).unwrap();

        let res = read_chat_image(&cache, Some(&project), "conv-1", "diagram.png").unwrap();
        assert_eq!(res.mime_type, "image/png");
        assert_eq!(res.data, STANDARD.encode(fake_png));

        std::fs::remove_file(&img_path).unwrap();

        let reloaded = read_chat_image(&cache, Some(&project), "conv-1", "diagram.png").unwrap();
        assert_eq!(reloaded.data, res.data);
        assert_eq!(reloaded.mime_type, "image/png");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn read_data_url_parses_and_validates_base64() {
        let fake_png = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR";
        let data_url = format!("data:image/png;base64,{}", STANDARD.encode(fake_png));
        let res = read_data_url(&data_url).unwrap();
        assert_eq!(res.mime_type, "image/png");
        assert_eq!(res.data, STANDARD.encode(fake_png));

        assert!(read_data_url("data:image/png,rawtext").is_err());
    }

    #[test]
    fn rejects_plain_http_source() {
        let temp_dir = std::env::temp_dir().join("npi_test_rej_http");
        assert!(read_chat_image(
            &temp_dir,
            None,
            "c1",
            "http://example.com/insecure.png"
        )
        .is_err());
    }
}
