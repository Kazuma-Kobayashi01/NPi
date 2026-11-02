//! Chat image loader and conversation-private persistent storage.
//!
//! Provides the core caching and loading logic for the `chat_image_read` IPC command:
//! - Local project files read via [`crate::project_file`], rooted in the tab folder.
//! - Remote HTTPS downloads validated against public-only network rules in [`network`].
//! - Persists successful assets under user-private storage per hashed conversation key
//!   and source with atomic writes, ensuring offline availability even if the original
//!   project file is later deleted.
//!
//! Every request may carry a caller-generated id. [`ChatImageManager`] tracks those
//! ids so `chat_image_cancel` can interrupt a queued admission wait or an in-flight
//! HTTP transfer instead of merely discarding its result.

mod cache;
mod manager;
mod network;

pub use manager::{ChatImageManager, ChatImagePriority, ChatImageQuery};

use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use serde::Serialize;

use network::{detect_image_mime, MAX_IMAGE_BYTES};

/// The rejection message for a request the caller cancelled. The frontend
/// ignores exactly this message: the image simply unmounted.
pub const CANCELLED_MESSAGE: &str = "image request cancelled";

/// Bound on the total request budget (admission wait, preflight, redirects and
/// body). Exceeding it is a timeout, not a hard failure worth retrying forever.
pub const TOTAL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ChatImageResult {
    /// Base64 bytes (unchanged contract for existing callers).
    pub data: String,
    /// MIME from the bytes' signature, never from the URL or file extension.
    pub mime_type: String,
    /// SHA-256 of the decoded bytes: stable identity for the same asset even
    /// when storage metadata changes. The webview may key `<img>` state on it.
    pub asset_id: String,
    /// SHA-256 of the source string (path or URL) within its conversation.
    pub source_hash: String,
    /// Whether the bytes came from the on-disk conversation cache.
    pub cache_hit: bool,
    pub timing: ChatImageTiming,
}

#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ChatImageTiming {
    /// URL parsing, DNS and public-address preflight.
    #[serde(rename = "validationMs")]
    pub validation: u64,
    /// HTTP request, redirects and body (including one transient retry).
    #[serde(rename = "transferMs")]
    pub transfer: u64,
    /// Conversation cache lookup (and metadata validation).
    #[serde(rename = "cacheMs")]
    pub cache: u64,
    /// Whole request, including any admission wait.
    #[serde(rename = "totalMs")]
    pub total: u64,
}

/// Cooperative cancellation shared by a request's queued admission future and
/// its network future. One task polls them in sequence, so a single waker slot
/// is enough: the last pending future's waker is the one that must run.
#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<CancelState>);

#[derive(Debug, Default)]
struct CancelState {
    cancelled: AtomicBool,
    waker: Mutex<Option<Waker>>,
}

impl CancelToken {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Set the flag and hand back the parked waker without running it. The
    /// request registry holds its lock across this call so a cache-write
    /// decision cannot interleave with a cancellation.
    pub(crate) fn mark_cancelled(&self) -> Option<Waker> {
        self.0.cancelled.store(true, Ordering::SeqCst);
        self.0
            .waker
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
    }

    /// Wake the owning task so it can observe the flag and drop the transfer.
    #[cfg(test)]
    pub(crate) fn cancel(&self) {
        if let Some(waker) = self.mark_cancelled() {
            waker.wake();
        }
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        self.0.cancelled.load(Ordering::SeqCst)
    }

    fn same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    fn park(&self, waker: &Waker) {
        let mut slot = self
            .0
            .waker
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if self.is_cancelled() {
            drop(slot);
            waker.wake_by_ref();
        } else {
            *slot = Some(waker.clone());
        }
    }
}

/// Marker for a cancelled request; never retried and never cached.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cancelled;

/// Poll `future` until it completes or `token` is cancelled. Dropping the
/// future (which happens on cancellation) tears down the underlying reqwest
/// request, including a partially read response body.
pub async fn with_cancel<F: Future>(
    token: &CancelToken,
    future: F,
) -> Result<F::Output, Cancelled> {
    let mut future = Box::pin(future);
    std::future::poll_fn(|cx: &mut Context<'_>| {
        if token.is_cancelled() {
            return Poll::Ready(Err(Cancelled));
        }
        match future.as_mut().poll(cx) {
            Poll::Ready(value) => Poll::Ready(Ok(value)),
            Poll::Pending => {
                token.park(cx.waker());
                Poll::Pending
            }
        }
    })
    .await
}

fn decode_data_url(source: &str) -> Result<(Vec<u8>, &'static str), String> {
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
    let max_base64_len = usize::try_from(MAX_IMAGE_BYTES)
        .unwrap_or(usize::MAX)
        .saturating_mul(4)
        / 3
        + 4;
    if trimmed_b64.len() > max_base64_len {
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

    Ok((bytes, detected_mime))
}

/// Build the IPC result. `asset_id` hashes the decoded bytes, so a cache hit,
/// a refetch and an upgraded legacy cache entry all report the same id.
fn image_result(
    bytes: &[u8],
    mime_type: &str,
    source_hash: &str,
    asset_id: String,
    cache_hit: bool,
    timing: ChatImageTiming,
) -> ChatImageResult {
    ChatImageResult {
        data: STANDARD.encode(bytes),
        mime_type: mime_type.to_string(),
        asset_id,
        source_hash: source_hash.to_string(),
        cache_hit,
        timing,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::future::Future;

    fn block_on<F: Future>(future: F) -> F::Output {
        use std::sync::Arc as StdArc;
        use std::task::{Context, Poll, Wake, Waker};

        struct Signal(std::thread::Thread);
        impl Wake for Signal {
            fn wake(self: StdArc<Self>) {
                self.0.unpark();
            }
            fn wake_by_ref(self: &StdArc<Self>) {
                self.0.unpark();
            }
        }

        let signal = StdArc::new(Signal(std::thread::current()));
        let waker = Waker::from(signal);
        let mut context = Context::from_waker(&waker);
        let mut future = std::pin::pin!(future);
        loop {
            match future.as_mut().poll(&mut context) {
                Poll::Ready(value) => return value,
                Poll::Pending => std::thread::park(),
            }
        }
    }

    #[test]
    fn cancelled_token_ends_an_otherwise_pending_future() {
        let token = CancelToken::new();
        let waiter = token.clone();
        std::thread::spawn(move || waiter.cancel());
        assert_eq!(
            block_on(with_cancel(&token, std::future::pending::<()>())),
            Err(Cancelled)
        );
    }

    #[test]
    fn data_url_parses_and_validates_base64() {
        let fake_png = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR";
        let data_url = format!("data:image/png;base64,{}", STANDARD.encode(fake_png));
        let (bytes, mime) = decode_data_url(&data_url).unwrap();
        assert_eq!(mime, "image/png");
        assert_eq!(bytes.as_slice(), fake_png.as_slice());

        assert!(decode_data_url("data:image/png,rawtext").is_err());
        assert!(decode_data_url("data:image/png;base64,@@@").is_err());
    }

    #[test]
    fn image_result_identity_is_content_addressed() {
        let bytes = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR";
        let sha = crate::json_store::hex_sha256(bytes);
        let a = image_result(
            bytes,
            "image/png",
            "source-a",
            sha.clone(),
            false,
            ChatImageTiming::default(),
        );
        let b = image_result(
            bytes,
            "image/png",
            "source-b",
            sha,
            true,
            ChatImageTiming::default(),
        );
        assert_eq!(a.asset_id, crate::json_store::hex_sha256(bytes));
        assert_eq!(a.asset_id, b.asset_id);
        assert_ne!(a.source_hash, b.source_hash);
    }
}
