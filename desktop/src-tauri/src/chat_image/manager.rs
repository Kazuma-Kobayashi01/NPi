//! Bounded, cancellable request lifecycle for chat images.
//!
//! Only remote transfers pass through the admission scheduler: four may run at
//! once, at most sixteen more wait in a priority queue (thumbnails jump ahead
//! of queued full requests), and a queued waiter can be withdrawn by
//! `chat_image_cancel` before it ever opens a socket. Local files, data URLs
//! and cache hits never take a transfer slot.
//!
//! Cache writes are returned to the caller as [`PendingCache`] so the IPC
//! command can move them to a blocking worker: the first display never waits
//! for the 8 MiB payload plus metadata to be fsynced, hashed again and evicted.

use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::task::{Context, Poll, Waker};
use std::time::{Duration, Instant};

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;

use super::cache::{self, CachedAsset};
use super::network::{self, detect_image_mime};
use super::{
    image_result, CancelToken, ChatImageResult, ChatImageTiming, CANCELLED_MESSAGE, TOTAL_TIMEOUT,
};

const MAX_ACTIVE_TRANSFERS: usize = 4;
const MAX_QUEUED_TRANSFERS: usize = 16;
const MAX_TRACKED_REQUESTS: usize = 64;
const MAX_REQUEST_ID_LEN: usize = 128;
const TIMEOUT_MESSAGE: &str = "image request timed out";
/// Bounded tombstones for cancels that arrived before their read invocation
/// registered (two Tauri commands, no ordering guarantee). Unique request ids
/// mean a tombstone is consumed by at most the one read it belongs to.
const MAX_CANCELLED_TOMBSTONES: usize = 64;

/// Queue class for a request. `Thumbnail` is for inline/preview surfaces and
/// is dispatched ahead of queued `Full` transfers; it does not bypass the
/// active limit or preflight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatImagePriority {
    Thumbnail,
    Full,
}

impl ChatImagePriority {
    /// `None` at the IPC boundary means [`Self::Full`]; an unknown string is a
    /// contract error rather than a silent downgrade.
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "thumbnail" => Ok(Self::Thumbnail),
            "full" => Ok(Self::Full),
            other => Err(format!("unknown image priority: {other}")),
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::Thumbnail => "thumbnail",
            Self::Full => "full",
        }
    }
}

/// One image request from the IPC boundary. The project root is captured by
/// the command from the trusted tab registry, never sent by the webview.
pub struct ChatImageQuery {
    pub session_id: String,
    pub request_id: Option<String>,
    pub conversation_key: String,
    pub source: String,
    pub priority: ChatImagePriority,
    pub project_root: Option<PathBuf>,
}

pub struct ChatImageOutcome {
    pub result: ChatImageResult,
    /// Present on a fresh read or a legacy-metadata upgrade. The caller
    /// persists it off the first-display path.
    pub pending_cache: Option<PendingCache>,
    token: CancelToken,
    registration: Option<RequestGuard>,
}

impl ChatImageOutcome {
    /// Split the response from its cache write. A pending write takes the
    /// registration with it, so `chat_image_cancel` can still reach the token
    /// until the background worker rechecks it. With no write to queue the
    /// registration is dropped here as usual.
    pub fn into_parts(self) -> (ChatImageResult, Option<PendingCache>) {
        let Self {
            result,
            pending_cache,
            token,
            registration,
        } = self;
        if token.is_cancelled() {
            drop(registration);
            return (result, None);
        }
        match pending_cache {
            Some(mut pending) => {
                // Move (never clone) the guard: it owns the token identity
                // that the worker's cancellation check must observe.
                pending.registration = registration;
                (result, Some(pending))
            }
            None => {
                drop(registration);
                (result, None)
            }
        }
    }
}

/// A cache write that has not happened yet. `bytes == None` is a metadata-only
/// upgrade of a pre-integrity entry whose payload is already on disk.
pub struct PendingCache {
    conv_hash: String,
    source_hash: String,
    source: String,
    mime_type: String,
    bytes: Option<Vec<u8>>,
    sha256: String,
    token: CancelToken,
    /// Kept live until the worker has finished (or skipped) the write, so a
    /// cancel after handoff can still mark `token`.
    registration: Option<RequestGuard>,
}

impl PendingCache {
    /// Blocking on purpose: only ever called from `spawn_blocking`. A request
    /// cancelled after the command queued this write but before the worker ran
    /// leaves the cache untouched; the registration drops with this value.
    pub fn persist(self, cache_root: &Path) -> Result<(), String> {
        if self.token.is_cancelled() {
            return Ok(());
        }
        cache::persist(
            cache_root,
            &self.conv_hash,
            &self.source_hash,
            &self.source,
            &self.mime_type,
            self.bytes.as_deref(),
            &self.sha256,
        )
    }
}

pub struct ChatImageManager {
    cache_root: Option<PathBuf>,
    scheduler: Arc<Scheduler>,
    requests: Arc<Mutex<RequestState>>,
}

#[derive(Default)]
struct RequestState {
    live: HashMap<RequestKey, CancelToken>,
    cancelled: VecDeque<RequestKey>,
}

#[derive(Clone, PartialEq, Eq, Hash)]
struct RequestKey {
    session_id: String,
    request_id: String,
}

struct RequestGuard {
    requests: Arc<Mutex<RequestState>>,
    key: RequestKey,
    token: CancelToken,
}

impl Drop for RequestGuard {
    fn drop(&mut self) {
        let mut state = self
            .requests
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state
            .live
            .get(&self.key)
            .is_some_and(|current| current.same(&self.token))
        {
            let _ = state.live.remove(&self.key);
        }
    }
}

impl ChatImageManager {
    pub fn new(cache_root: Option<PathBuf>) -> Self {
        Self {
            cache_root,
            scheduler: Arc::new(Scheduler::default()),
            requests: Arc::new(Mutex::new(RequestState::default())),
        }
    }

    pub fn cache_root(&self) -> Option<&Path> {
        self.cache_root.as_deref()
    }

    /// Cancel one request. Returns whether a live registration existed; an
    /// already-finished request is not an error.
    pub fn cancel(&self, session_id: &str, request_id: &str) -> bool {
        if session_id.is_empty() || request_id.is_empty() {
            return false;
        }
        let key = RequestKey {
            session_id: session_id.to_string(),
            request_id: request_id.to_string(),
        };
        let mut state = self.requests();
        if let Some(token) = state.live.remove(&key) {
            // Set the flag under the registry lock so `into_parts` cannot
            // decide to persist cache bytes for a request being cancelled.
            let waker = token.mark_cancelled();
            drop(state);
            if let Some(waker) = waker {
                waker.wake();
            }
            return true;
        }
        // The matching `chat_image_read` invocation may not have reached
        // `register` yet: Tauri does not order two commands' execution. A
        // bounded tombstone makes that later registration fail as cancelled
        // instead of resurrecting the request.
        if !state.cancelled.contains(&key) {
            if state.cancelled.len() >= MAX_CANCELLED_TOMBSTONES {
                state.cancelled.pop_front();
            }
            state.cancelled.push_back(key);
        }
        false
    }

    fn requests(&self) -> MutexGuard<'_, RequestState> {
        self.requests
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn register(&self, session_id: &str, request_id: &str) -> Result<CancelToken, String> {
        if request_id.is_empty() {
            return Err("requestId must not be empty".to_string());
        }
        if request_id.len() > MAX_REQUEST_ID_LEN {
            return Err("requestId is too long".to_string());
        }
        let key = RequestKey {
            session_id: session_id.to_string(),
            request_id: request_id.to_string(),
        };
        let mut state = self.requests();
        if let Some(index) = state.cancelled.iter().position(|entry| entry == &key) {
            let _ = state.cancelled.remove(index);
            return Err(CANCELLED_MESSAGE.to_string());
        }
        if state.live.contains_key(&key) {
            return Err("duplicate image request id".to_string());
        }
        if state.live.len() >= MAX_TRACKED_REQUESTS {
            return Err("too many active image requests".to_string());
        }
        let token = CancelToken::new();
        let _ = state.live.insert(key, token.clone());
        drop(state);
        Ok(token)
    }

    #[cfg(test)]
    fn unregister(&self, key: &RequestKey, token: &CancelToken) {
        let mut state = self.requests();
        if state
            .live
            .get(key)
            .is_some_and(|current| current.same(token))
        {
            let _ = state.live.remove(key);
        }
    }

    /// Resolve one image. Cache hits and local/data images resolve without a
    /// transfer permit; remote reads are admitted, preflighted and fetched
    /// under the shared total deadline.
    pub async fn read(&self, query: ChatImageQuery) -> Result<ChatImageOutcome, String> {
        let started = Instant::now();
        if query.conversation_key.is_empty() {
            return Err("conversationKey must not be empty".to_string());
        }
        if query.source.is_empty() {
            return Err("image source must not be empty".to_string());
        }
        let deadline = started + TOTAL_TIMEOUT;

        let conv_hash = crate::json_store::hex_sha256(query.conversation_key.as_bytes());
        let source_hash = crate::json_store::hex_sha256(query.source.as_bytes());

        let (token, registration) = match query.request_id.as_deref() {
            Some(request_id) => {
                let token = self.register(&query.session_id, request_id)?;
                let guard = RequestGuard {
                    requests: Arc::clone(&self.requests),
                    key: RequestKey {
                        session_id: query.session_id.clone(),
                        request_id: request_id.to_string(),
                    },
                    token: token.clone(),
                };
                (token, Some(guard))
            }
            None => (CancelToken::new(), None),
        };
        if token.is_cancelled() {
            return Err(CANCELLED_MESSAGE.to_string());
        }

        let cache_started = Instant::now();
        let cached = match &self.cache_root {
            Some(root) => cache::read(root, &conv_hash, &source_hash, &query.source)?,
            None => None,
        };
        let cache_ms = milliseconds(cache_started.elapsed());
        if token.is_cancelled() {
            return Err(CANCELLED_MESSAGE.to_string());
        }

        let context = ReadCtx {
            query: &query,
            token: token.clone(),
            conv_hash,
            source_hash,
            cache_ms,
            started,
            deadline,
        };

        if let Some(asset) = cached {
            let (result, pending_cache) = context.cached(asset);
            if token.is_cancelled() {
                return Err(CANCELLED_MESSAGE.to_string());
            }
            return Ok(ChatImageOutcome {
                result,
                pending_cache,
                token,
                registration,
            });
        }

        let resolved = if query.source.starts_with("data:") {
            context.data_url()?
        } else if query.source.starts_with("https://") {
            if token.is_cancelled() {
                return Err(CANCELLED_MESSAGE.to_string());
            }
            context.remote(self).await?
        } else if query.source.starts_with("http://") {
            return Err("only HTTPS images are permitted; plain HTTP is forbidden".to_string());
        } else {
            let root = query
                .project_root
                .as_deref()
                .ok_or_else(|| "this tab has no project folder".to_string())?;
            context.project(root)?
        };
        if token.is_cancelled() {
            return Err(CANCELLED_MESSAGE.to_string());
        }
        let ResolvedImage {
            result,
            pending_cache,
        } = resolved;
        Ok(ChatImageOutcome {
            result,
            pending_cache: Some(pending_cache),
            token,
            registration,
        })
    }
}

/// One request's immutable inputs plus its already-measured cache lookup.
/// Helpers consume it so the source-specific branch builds the result, timing
/// log and pending cache write in one place.
struct ReadCtx<'a> {
    query: &'a ChatImageQuery,
    token: CancelToken,
    conv_hash: String,
    source_hash: String,
    cache_ms: u64,
    started: Instant,
    deadline: Instant,
}

struct ResolvedImage {
    result: ChatImageResult,
    pending_cache: PendingCache,
}

impl ReadCtx<'_> {
    fn cached(self, asset: CachedAsset) -> (ChatImageResult, Option<PendingCache>) {
        let CachedAsset {
            bytes,
            mime_type,
            sha256,
            legacy,
        } = asset;
        let timing = ChatImageTiming {
            validation: 0,
            transfer: 0,
            cache: self.cache_ms,
            total: milliseconds(self.started.elapsed()),
        };
        let result = image_result(
            &bytes,
            &mime_type,
            &self.source_hash,
            sha256.clone(),
            true,
            timing,
        );
        log_request(
            self.query,
            &self.source_hash,
            &timing,
            bytes.len(),
            true,
            "cache-hit",
        );
        let conv_hash = self.conv_hash;
        let source_hash = self.source_hash;
        let source = self.query.source.clone();
        let token = self.token;
        let pending_cache = legacy.then(|| PendingCache {
            conv_hash,
            source_hash: source_hash.clone(),
            source,
            mime_type,
            bytes: None,
            sha256,
            token,
            registration: None,
        });
        (result, pending_cache)
    }

    fn data_url(self) -> Result<ResolvedImage, String> {
        let (bytes, mime_type) = super::decode_data_url(&self.query.source)?;
        Ok(self.bytes(bytes, mime_type, 0, 0, false, "data-url"))
    }

    fn project(self, root: &Path) -> Result<ResolvedImage, String> {
        let (bytes, mime_type) = read_project_image(root, &self.query.source)?;
        Ok(self.bytes(bytes, mime_type, 0, 0, false, "local"))
    }

    async fn remote(self, manager: &ChatImageManager) -> Result<ResolvedImage, String> {
        let permit = await_with_deadline(
            self.deadline,
            manager
                .scheduler
                .acquire(self.token.clone(), self.query.priority, self.deadline),
        )
        .await??;
        let fetched = await_with_deadline(
            self.deadline,
            network::fetch_public_https_image(&self.query.source, &self.token, self.deadline),
        )
        .await;
        drop(permit);
        let fetched = match fetched {
            Ok(fetched) => fetched,
            Err(timeout) => return Err(timeout),
        };
        let fetched = match fetched {
            Ok(fetched) => fetched,
            Err(error) if error.is_cancelled() => return Err(CANCELLED_MESSAGE.to_string()),
            Err(error) => {
                let message = error.into_message();
                let timing = ChatImageTiming {
                    cache: self.cache_ms,
                    total: milliseconds(self.started.elapsed()),
                    ..ChatImageTiming::default()
                };
                log_failure(self.query, &self.source_hash, &timing, &message);
                return Err(message);
            }
        };
        Ok(self.bytes(
            fetched.bytes,
            fetched.mime_type,
            fetched.validation_ms,
            fetched.transfer_ms,
            false,
            "transfer",
        ))
    }

    fn bytes(
        self,
        bytes: Vec<u8>,
        mime_type: &str,
        validation_ms: u64,
        transfer_ms: u64,
        cache_hit: bool,
        outcome: &str,
    ) -> ResolvedImage {
        let timing = ChatImageTiming {
            validation: validation_ms,
            transfer: transfer_ms,
            cache: self.cache_ms,
            total: milliseconds(self.started.elapsed()),
        };
        let sha256 = crate::json_store::hex_sha256(&bytes);
        let result = image_result(
            &bytes,
            mime_type,
            &self.source_hash,
            sha256.clone(),
            cache_hit,
            timing,
        );
        log_request(
            self.query,
            &self.source_hash,
            &timing,
            bytes.len(),
            cache_hit,
            outcome,
        );
        let pending_cache = PendingCache {
            conv_hash: self.conv_hash,
            source_hash: self.source_hash,
            source: self.query.source.clone(),
            mime_type: mime_type.to_string(),
            bytes: Some(bytes),
            sha256,
            token: self.token,
            registration: None,
        };
        ResolvedImage {
            result,
            pending_cache,
        }
    }
}

/// Decode one local project image. `project_file` owns the trust decisions
/// (root, traversal, `.git`, size); this narrows the result to bytes plus the
/// signature MIME so the request pipeline does not repeat its match arms.
fn read_project_image(root: &Path, source: &str) -> Result<(Vec<u8>, &'static str), String> {
    let preview = crate::project_file::read(root, source)?;
    match preview.kind {
        crate::project_file::PreviewKind::Image => {
            let data = preview
                .data
                .expect("Image preview contract guarantees data");
            let bytes = STANDARD
                .decode(&data)
                .map_err(|e| format!("failed to decode image data: {e}"))?;
            let mime_type = detect_image_mime(&bytes)
                .ok_or_else(|| format!("unsupported image format in project file: {source}"))?;
            Ok((bytes, mime_type))
        }
        crate::project_file::PreviewKind::Missing => Err(format!("image file not found: {source}")),
        crate::project_file::PreviewKind::Outside => Err(format!(
            "image path resolves outside project folder: {source}"
        )),
        crate::project_file::PreviewKind::TooLarge => {
            Err(format!("image exceeds maximum size of 8 MiB: {source}"))
        }
        other => Err(format!("file is not an image ({other:?}): {source}")),
    }
}

fn milliseconds(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

/// Apply the request's wall-clock deadline to an await. `timeout_at` uses the
/// shared Tauri Tokio runtime's timer; on expiry it drops the wrapped future,
/// so a stalled admission queue or a blocked DNS worker releases the caller
/// (and any held transfer permit) without needing an external wake.
async fn await_with_deadline<F: Future>(deadline: Instant, future: F) -> Result<F::Output, String> {
    tokio::time::timeout_at(deadline.into(), future)
        .await
        .map_err(|_| TIMEOUT_MESSAGE.to_string())
}

/// The host is the only part of a remote source safe to log: no path, query
/// or fragment can carry a token. Local paths and data URLs never reach the
/// URL parser here.
fn source_host(source: &str) -> String {
    if !(source.starts_with("https://") || source.starts_with("http://")) {
        return "-".to_string();
    }
    url::Url::parse(source)
        .ok()
        .and_then(|url| url.host_str().map(str::to_owned))
        .unwrap_or_else(|| "-".to_string())
}

/// Timing logs deliberately carry the host and hashes only: never the path,
/// query string or fragment of the source URL.
fn log_request(
    query: &ChatImageQuery,
    source_hash: &str,
    timing: &ChatImageTiming,
    bytes: usize,
    cache_hit: bool,
    outcome: &str,
) {
    let host = source_host(&query.source);
    eprintln!(
        "[chat-image] {outcome} host={host} source={source_hash} priority={} cacheHit={cache_hit} validationMs={} transferMs={} cacheMs={} totalMs={} bytes={bytes}",
        query.priority.name(),
        timing.validation,
        timing.transfer,
        timing.cache,
        timing.total,
    );
}

fn log_failure(query: &ChatImageQuery, source_hash: &str, timing: &ChatImageTiming, message: &str) {
    let host = source_host(&query.source);
    eprintln!(
        "[chat-image] error host={host} source={source_hash} priority={} cacheMs={} totalMs={} reason={message}",
        query.priority.name(),
        timing.cache,
        timing.total,
    );
}

// ── admission scheduler ─────────────────────────────────────────────────────

#[derive(Default)]
struct SchedulerState {
    active: usize,
    next_ticket: u64,
    queue: Vec<Queued>,
}

struct Queued {
    ticket: u64,
    priority: ChatImagePriority,
    waiter: Arc<Waiter>,
}

struct Waiter {
    granted: AtomicBool,
    waker: Mutex<Option<Waker>>,
    token: CancelToken,
}

impl Waiter {
    const fn new(token: CancelToken) -> Self {
        Self {
            granted: AtomicBool::new(false),
            waker: Mutex::new(None),
            token,
        }
    }

    fn wake(&self) {
        let waker = self
            .waker
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        if let Some(waker) = waker {
            waker.wake();
        }
    }
}

#[derive(Default)]
struct Scheduler {
    state: Mutex<SchedulerState>,
}

impl Scheduler {
    fn state(&self) -> MutexGuard<'_, SchedulerState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn acquire(
        self: &Arc<Self>,
        token: CancelToken,
        priority: ChatImagePriority,
        deadline: Instant,
    ) -> Acquire {
        Acquire {
            scheduler: Arc::clone(self),
            token,
            priority,
            deadline,
            waiter: None,
            ticket: 0,
            finished: false,
        }
    }

    /// Grant every free slot to the best queued waiter: thumbnails first, then
    /// tickets in arrival order. Cancelled waiters are dropped without a slot.
    fn dispatch(&self) {
        loop {
            let granted = {
                let mut state = self.state();
                state
                    .queue
                    .retain(|queued| !queued.waiter.token.is_cancelled());
                if state.active >= MAX_ACTIVE_TRANSFERS {
                    return;
                }
                let mut best: Option<usize> = None;
                for (index, queued) in state.queue.iter().enumerate() {
                    if best.is_none_or(|current| outranks(queued, &state.queue[current])) {
                        best = Some(index);
                    }
                }
                let Some(index) = best else {
                    return;
                };
                let queued = state.queue.remove(index);
                state.active += 1;
                queued.waiter.granted.store(true, Ordering::SeqCst);
                drop(state);
                queued.waiter
            };
            granted.wake();
        }
    }

    fn abandon(&self, waiter: &Waiter, ticket: u64) {
        let mut state = self.state();
        if waiter.granted.load(Ordering::SeqCst) {
            state.active = state.active.saturating_sub(1);
            drop(state);
            self.dispatch();
        } else {
            state.queue.retain(|queued| queued.ticket != ticket);
        }
    }

    fn release(&self) {
        {
            let mut state = self.state();
            state.active = state.active.saturating_sub(1);
        }
        self.dispatch();
        // Waiters whose whole-request deadline passed while they queued must
        // observe it even when the freed permit went to someone else.
        self.wake_all();
    }

    fn wake_all(&self) {
        let waiters: Vec<Arc<Waiter>> = self
            .state()
            .queue
            .iter()
            .map(|queued| Arc::clone(&queued.waiter))
            .collect();
        for waiter in waiters {
            waiter.wake();
        }
    }
}

const fn outranks(candidate: &Queued, chosen: &Queued) -> bool {
    match (candidate.priority, chosen.priority) {
        (ChatImagePriority::Thumbnail, ChatImagePriority::Full) => true,
        (ChatImagePriority::Full, ChatImagePriority::Thumbnail) => false,
        _ => candidate.ticket < chosen.ticket,
    }
}

struct Permit {
    scheduler: Arc<Scheduler>,
}

impl Drop for Permit {
    fn drop(&mut self) {
        self.scheduler.release();
    }
}

/// Future returned by [`Scheduler::acquire`]. It owns the queue slot so
/// dropping the request future (task abort, app shutdown) cannot leak a
/// permit; on cancellation it either removes the queue entry or returns an
/// already-granted permit before reporting [`CANCELLED_MESSAGE`].
struct Acquire {
    scheduler: Arc<Scheduler>,
    token: CancelToken,
    priority: ChatImagePriority,
    deadline: Instant,
    waiter: Option<Arc<Waiter>>,
    ticket: u64,
    finished: bool,
}

impl Acquire {
    fn abandon(&mut self) {
        if self.finished {
            return;
        }
        self.finished = true;
        if let Some(waiter) = self.waiter.take() {
            self.scheduler.abandon(&waiter, self.ticket);
        }
    }
}

impl Future for Acquire {
    type Output = Result<Permit, String>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        if this.token.is_cancelled() {
            this.abandon();
            return Poll::Ready(Err(CANCELLED_MESSAGE.to_string()));
        }

        // A grant that landed before the deadline still hands over the permit;
        // the transfer then re-checks whatever budget is left.
        if this
            .waiter
            .as_ref()
            .is_some_and(|waiter| waiter.granted.load(Ordering::SeqCst))
        {
            this.finished = true;
            return Poll::Ready(Ok(Permit {
                scheduler: Arc::clone(&this.scheduler),
            }));
        }
        if Instant::now() >= this.deadline {
            this.abandon();
            return Poll::Ready(Err(TIMEOUT_MESSAGE.to_string()));
        }

        if this.waiter.is_none() {
            let granted_immediately = {
                let mut state = this.scheduler.state();
                if state.active < MAX_ACTIVE_TRANSFERS && state.queue.is_empty() {
                    state.active += 1;
                    true
                } else {
                    if state.queue.len() >= MAX_QUEUED_TRANSFERS {
                        return Poll::Ready(Err(
                            "too many pending image downloads; try again".to_string()
                        ));
                    }
                    let ticket = state.next_ticket;
                    state.next_ticket = state.next_ticket.wrapping_add(1);
                    let waiter = Arc::new(Waiter::new(this.token.clone()));
                    state.queue.push(Queued {
                        ticket,
                        priority: this.priority,
                        waiter: Arc::clone(&waiter),
                    });
                    this.ticket = ticket;
                    this.waiter = Some(waiter);
                    drop(state);
                    false
                }
            };
            if granted_immediately {
                this.finished = true;
                return Poll::Ready(Ok(Permit {
                    scheduler: Arc::clone(&this.scheduler),
                }));
            }
        }

        this.scheduler.dispatch();

        if this
            .waiter
            .as_ref()
            .is_some_and(|waiter| waiter.granted.load(Ordering::SeqCst))
        {
            this.finished = true;
            return Poll::Ready(Ok(Permit {
                scheduler: Arc::clone(&this.scheduler),
            }));
        }

        // Park on both the grant (dispatch wakes the waiter) and cancellation
        // (the token wakes this same task).
        {
            let waiter = this
                .waiter
                .as_ref()
                .expect("a queued acquire always holds its waiter");
            *waiter
                .waker
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(cx.waker().clone());
        }
        this.token.park(cx.waker());

        // A grant or cancel that landed between the check and the park must
        // not leave the task asleep.
        if this
            .waiter
            .as_ref()
            .is_some_and(|waiter| waiter.granted.load(Ordering::SeqCst))
        {
            this.finished = true;
            return Poll::Ready(Ok(Permit {
                scheduler: Arc::clone(&this.scheduler),
            }));
        }
        if this.token.is_cancelled() {
            this.abandon();
            return Poll::Ready(Err(CANCELLED_MESSAGE.to_string()));
        }
        if Instant::now() >= this.deadline {
            this.abandon();
            return Poll::Ready(Err(TIMEOUT_MESSAGE.to_string()));
        }
        Poll::Pending
    }
}

impl Drop for Acquire {
    fn drop(&mut self) {
        self.abandon();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::future::Future;
    use std::pin::Pin;
    use std::task::Waker;

    fn poll_once<F: Future>(future: Pin<&mut F>) -> Poll<F::Output> {
        let waker = Waker::noop();
        let mut context = Context::from_waker(waker);
        future.poll(&mut context)
    }

    fn admit(scheduler: &Arc<Scheduler>) -> Permit {
        let mut future = Box::pin(scheduler.acquire(
            CancelToken::new(),
            ChatImagePriority::Full,
            before_deadline(),
        ));
        match poll_once(future.as_mut()) {
            Poll::Ready(result) => result.unwrap(),
            Poll::Pending => panic!("expected an immediate permit"),
        }
    }

    fn before_deadline() -> Instant {
        Instant::now() + Duration::from_secs(20)
    }

    #[test]
    fn admission_caps_active_transfers_and_queues_the_rest() {
        let scheduler = Arc::new(Scheduler::default());
        let mut permits: Vec<Permit> = (0..MAX_ACTIVE_TRANSFERS)
            .map(|_| admit(&scheduler))
            .collect();

        let mut queued = Box::pin(scheduler.acquire(
            CancelToken::new(),
            ChatImagePriority::Full,
            before_deadline(),
        ));
        assert!(poll_once(queued.as_mut()).is_pending());
        assert_eq!(scheduler.state().active, MAX_ACTIVE_TRANSFERS);
        assert_eq!(scheduler.state().queue.len(), 1);

        drop(permits.pop());
        let Poll::Ready(Ok(granted)) = poll_once(queued.as_mut()) else {
            panic!("expected the queued transfer to be admitted");
        };
        assert_eq!(scheduler.state().active, MAX_ACTIVE_TRANSFERS);
        drop(granted);
        assert_eq!(scheduler.state().active, MAX_ACTIVE_TRANSFERS - 1);
    }

    #[test]
    fn thumbnail_jumps_ahead_of_queued_full_requests() {
        let scheduler = Arc::new(Scheduler::default());
        let mut permits: Vec<Permit> = (0..MAX_ACTIVE_TRANSFERS)
            .map(|_| admit(&scheduler))
            .collect();

        let mut full = Box::pin(scheduler.acquire(
            CancelToken::new(),
            ChatImagePriority::Full,
            before_deadline(),
        ));
        assert!(poll_once(full.as_mut()).is_pending());
        let mut thumbnail = Box::pin(scheduler.acquire(
            CancelToken::new(),
            ChatImagePriority::Thumbnail,
            before_deadline(),
        ));
        assert!(poll_once(thumbnail.as_mut()).is_pending());
        assert_eq!(scheduler.state().queue.len(), 2);

        drop(permits.pop());
        let Poll::Ready(Ok(thumbnail_permit)) = poll_once(thumbnail.as_mut()) else {
            panic!("expected the thumbnail to be admitted first");
        };
        // While the thumbnail holds its permit, the earlier full request is
        // still queued.
        assert!(poll_once(full.as_mut()).is_pending());

        drop(thumbnail_permit);
        assert!(matches!(poll_once(full.as_mut()), Poll::Ready(Ok(_))));
    }

    #[test]
    fn cancelled_active_transfer_frees_its_slot_to_a_priority_waiter() {
        use super::super::with_cancel;

        let scheduler = Arc::new(Scheduler::default());
        let _permits: Vec<Permit> = (0..MAX_ACTIVE_TRANSFERS - 1)
            .map(|_| admit(&scheduler))
            .collect();
        let transfer_permit = admit(&scheduler);
        assert_eq!(scheduler.state().active, MAX_ACTIVE_TRANSFERS);

        let mut full = Box::pin(scheduler.acquire(
            CancelToken::new(),
            ChatImagePriority::Full,
            before_deadline(),
        ));
        assert!(poll_once(full.as_mut()).is_pending());
        let mut thumbnail = Box::pin(scheduler.acquire(
            CancelToken::new(),
            ChatImagePriority::Thumbnail,
            before_deadline(),
        ));
        assert!(poll_once(thumbnail.as_mut()).is_pending());
        assert_eq!(scheduler.state().queue.len(), 2);

        // Stand-in for an admitted HTTP transfer: it holds a permit while its
        // future is pending, exactly like `read` across a network await.
        let token = CancelToken::new();
        let transfer = with_cancel(&token, async move {
            let _held = transfer_permit;
            std::future::pending::<()>().await;
        });
        let mut transfer = Box::pin(transfer);
        assert!(poll_once(transfer.as_mut()).is_pending());

        token.cancel();
        match poll_once(transfer.as_mut()) {
            Poll::Ready(Err(_)) => {}
            _ => panic!("expected the in-flight transfer to be dropped"),
        }

        // Dropping the cancelled transfer released its permit, and the
        // thumbnail took it ahead of the earlier full request.
        assert_eq!(scheduler.state().queue.len(), 1);
        assert_eq!(scheduler.state().active, MAX_ACTIVE_TRANSFERS);
        let Poll::Ready(Ok(thumbnail_permit)) = poll_once(thumbnail.as_mut()) else {
            panic!("expected the thumbnail to take the freed slot");
        };
        assert!(poll_once(full.as_mut()).is_pending());

        drop(thumbnail_permit);
        assert!(matches!(poll_once(full.as_mut()), Poll::Ready(Ok(_))));
    }

    #[test]
    fn cancellation_withdraws_a_queued_waiter() {
        let scheduler = Arc::new(Scheduler::default());
        let permits: Vec<Permit> = (0..MAX_ACTIVE_TRANSFERS)
            .map(|_| admit(&scheduler))
            .collect();
        let token = CancelToken::new();
        let mut queued =
            Box::pin(scheduler.acquire(token.clone(), ChatImagePriority::Full, before_deadline()));
        assert!(poll_once(queued.as_mut()).is_pending());
        assert_eq!(scheduler.state().queue.len(), 1);

        token.cancel();
        match poll_once(queued.as_mut()) {
            Poll::Ready(Err(message)) => assert_eq!(message, CANCELLED_MESSAGE),
            _ => panic!("expected cancellation"),
        }
        assert!(scheduler.state().queue.is_empty());
        assert_eq!(scheduler.state().active, MAX_ACTIVE_TRANSFERS);
        drop(permits);
    }

    #[test]
    fn admission_deadline_rejects_before_work_starts() {
        let scheduler = Arc::new(Scheduler::default());
        let mut future = Box::pin(
            scheduler.acquire(
                CancelToken::new(),
                ChatImagePriority::Full,
                Instant::now()
                    .checked_sub(Duration::from_millis(1))
                    .unwrap(),
            ),
        );
        match poll_once(future.as_mut()) {
            Poll::Ready(Err(message)) => assert!(message.contains("timed out")),
            _ => panic!("expected admission timeout"),
        }
        assert_eq!(scheduler.state().active, 0);
    }

    #[test]
    fn stalled_awaits_time_out_without_an_external_wake() {
        tauri::async_runtime::block_on(async {
            let deadline = Instant::now() + Duration::from_millis(20);
            match await_with_deadline(deadline, std::future::pending::<()>()).await {
                Err(message) => assert_eq!(message, TIMEOUT_MESSAGE),
                Ok(()) => panic!("a pending future must time out"),
            }

            let scheduler = Arc::new(Scheduler::default());
            let _permits: Vec<Permit> = (0..MAX_ACTIVE_TRANSFERS)
                .map(|_| admit(&scheduler))
                .collect();
            let deadline = Instant::now() + Duration::from_millis(20);
            let queued = scheduler.acquire(CancelToken::new(), ChatImagePriority::Full, deadline);
            // `Acquire` may win the race with its own poll-time deadline
            // check, turning the outer timeout into Ok(Err(..)); both are the
            // same consumer-visible timeout, while a permit is a failure.
            match await_with_deadline(deadline, queued).await {
                Ok(Ok(_)) => panic!("a queued admission must not receive a permit"),
                Ok(Err(message)) | Err(message) => assert_eq!(message, TIMEOUT_MESSAGE),
            }
            assert!(scheduler.state().queue.is_empty());
            assert_eq!(scheduler.state().active, MAX_ACTIVE_TRANSFERS);
        });
    }

    #[test]
    fn queue_bound_rejects_excess_waiters() {
        let scheduler = Arc::new(Scheduler::default());
        let permits: Vec<Permit> = (0..MAX_ACTIVE_TRANSFERS)
            .map(|_| admit(&scheduler))
            .collect();
        let mut queued: Vec<Pin<Box<Acquire>>> = (0..MAX_QUEUED_TRANSFERS)
            .map(|_| {
                Box::pin(scheduler.acquire(
                    CancelToken::new(),
                    ChatImagePriority::Full,
                    before_deadline(),
                ))
            })
            .collect();
        for future in &mut queued {
            assert!(poll_once(future.as_mut()).is_pending());
        }

        let mut overflow = Box::pin(scheduler.acquire(
            CancelToken::new(),
            ChatImagePriority::Full,
            before_deadline(),
        ));
        match poll_once(overflow.as_mut()) {
            Poll::Ready(Err(message)) => assert!(message.contains("too many pending")),
            _ => panic!("expected queue rejection"),
        }
        drop(permits);
    }

    #[test]
    fn request_ids_are_unique_while_live_and_cancellable() {
        let manager = ChatImageManager::new(None);
        let token = manager.register("session-1", "request-1").unwrap();
        assert!(manager.register("session-1", "request-1").is_err());
        assert!(manager.cancel("session-1", "request-1"));
        assert!(token.is_cancelled());
        // A second cancel finds no live registration and tombstones the id,
        // so that id cannot be reused by a later read.
        assert!(!manager.cancel("session-1", "request-1"));
        assert_eq!(
            manager.register("session-1", "request-1").unwrap_err(),
            CANCELLED_MESSAGE
        );

        // Unrelated identity assertions use a fresh id: request ids are
        // globally unique per request, never reused after cancellation.
        let token = manager.register("session-1", "request-2").unwrap();
        assert!(manager.register("session-1", "request-2").is_err());
        let key = RequestKey {
            session_id: "session-1".to_string(),
            request_id: "request-2".to_string(),
        };
        let other = CancelToken::new();
        manager.unregister(&key, &other);
        assert!(!token.is_cancelled());
        manager.unregister(&key, &token);
        assert!(manager.register("session-1", "request-2").is_ok());
    }

    #[test]
    fn cancel_before_registration_tombstones_the_later_read() {
        let manager = ChatImageManager::new(None);
        // The cancel command ran before the read invocation was polled.
        assert!(!manager.cancel("session-1", "late-request"));
        assert_eq!(
            manager.register("session-1", "late-request").unwrap_err(),
            CANCELLED_MESSAGE
        );
        // Consumed on the first registration attempt: a genuinely new read
        // with the same id is not pre-cancelled forever.
        assert!(manager.register("session-1", "late-request").is_ok());
    }

    #[test]
    fn cancel_tombstones_are_bounded() {
        let manager = ChatImageManager::new(None);
        for index in 0..=MAX_CANCELLED_TOMBSTONES {
            assert!(!manager.cancel("session-1", &format!("request-{index}")));
        }
        // The oldest tombstone was evicted, the newest still applies.
        assert!(manager.register("session-1", "request-0").is_ok());
        assert_eq!(
            manager
                .register("session-1", &format!("request-{MAX_CANCELLED_TOMBSTONES}"))
                .unwrap_err(),
            CANCELLED_MESSAGE
        );
    }

    #[test]
    fn cancelled_outcome_does_not_start_a_cache_write() {
        let bytes: &[u8] = b"\x89PNG\r\n\x1a\n\x00\x00";
        let result = image_result(
            bytes,
            "image/png",
            "source-hash",
            crate::json_store::hex_sha256(bytes),
            false,
            ChatImageTiming::default(),
        );
        let pending = || PendingCache {
            conv_hash: "conv".to_string(),
            source_hash: "source".to_string(),
            source: "diagram.png".to_string(),
            mime_type: "image/png".to_string(),
            bytes: Some(bytes.to_vec()),
            sha256: crate::json_store::hex_sha256(bytes),
            token: CancelToken::new(),
            registration: None,
        };

        let token = CancelToken::new();
        token.cancel();
        let outcome = ChatImageOutcome {
            result: result.clone(),
            pending_cache: Some(pending()),
            token,
            registration: None,
        };
        let (_, pending_cache) = outcome.into_parts();
        assert!(pending_cache.is_none());

        let outcome = ChatImageOutcome {
            result,
            pending_cache: Some(pending()),
            token: CancelToken::new(),
            registration: None,
        };
        let (_, pending_cache) = outcome.into_parts();
        assert!(pending_cache.is_some());
    }

    #[test]
    fn cancel_after_handoff_prevents_the_queued_cache_write() {
        let temp = crate::test_support::TempDir::new("chat-image-manager", "cancel-after-handoff");
        let manager = ChatImageManager::new(Some(temp.path().to_path_buf()));
        let token = manager.register("session-1", "request-1").unwrap();
        let registration = RequestGuard {
            requests: Arc::clone(&manager.requests),
            key: RequestKey {
                session_id: "session-1".to_string(),
                request_id: "request-1".to_string(),
            },
            token: token.clone(),
        };
        let bytes: &[u8] = b"\x89PNG\r\n\x1a\n\x00\x00";
        let result = image_result(
            bytes,
            "image/png",
            "source-hash",
            crate::json_store::hex_sha256(bytes),
            false,
            ChatImageTiming::default(),
        );
        let pending = PendingCache {
            conv_hash: "conv".to_string(),
            source_hash: "source".to_string(),
            source: "diagram.png".to_string(),
            mime_type: "image/png".to_string(),
            bytes: Some(bytes.to_vec()),
            sha256: crate::json_store::hex_sha256(bytes),
            token: token.clone(),
            registration: None,
        };
        let outcome = ChatImageOutcome {
            result,
            pending_cache: Some(pending),
            token: token.clone(),
            registration: Some(registration),
        };

        let (_, pending) = outcome.into_parts();
        let pending = pending.expect("a pending cache write keeps the handoff");
        // The live registration moved into the write, so cancel still reaches
        // the exact token the worker checks.
        assert!(manager.cancel("session-1", "request-1"));
        assert!(token.is_cancelled());
        pending.persist(temp.path()).unwrap();
        assert!(!temp.path().join("conv").join("source.bin").exists());
        assert!(!manager.cancel("session-1", "request-1"));
    }

    #[test]
    fn priority_parser_defaults_and_rejects_unknown_values() {
        assert_eq!(
            ChatImagePriority::parse("thumbnail").unwrap(),
            ChatImagePriority::Thumbnail
        );
        assert_eq!(
            ChatImagePriority::parse("full").unwrap(),
            ChatImagePriority::Full
        );
        assert!(ChatImagePriority::parse("huge").is_err());
    }
}
