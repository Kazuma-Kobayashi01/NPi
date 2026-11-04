//! Public network validation, Cloudflare `DoH` fallback, and cancellable HTTPS
//! image fetching.
//!
//! Everything below the URL parser is async so cancellation can drop the
//! reqwest future, which closes the connection (and a partially read body)
//! instead of letting the transfer run to completion for a discarded caller.
//! The trust rules are unchanged: HTTPS only, no userinfo, system DNS answers
//! must be public (Fake-IP ranges re-resolve through pinned Cloudflare `DoH`),
//! every redirect hop is revalidated, and the client resolves only the pinned
//! addresses (no second, rebindable lookup).

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs as _};
use std::time::{Duration, Instant};

use serde::Deserialize;
use url::{Host, Url};

use super::{with_cancel, CancelToken};

pub(super) const MAX_IMAGE_BYTES: u64 = 8 * 1024 * 1024;

const MAX_REDIRECTS: usize = 5;
const MAX_DOH_BYTES: u64 = 64 * 1024;
/// A retry is only useful with enough budget left for a second transfer.
const RETRY_MIN_BUDGET: Duration = Duration::from_secs(2);
const CONNECT_TIMEOUT_CAP: Duration = Duration::from_secs(10);
const DOH_TIMEOUT_CAP: Duration = Duration::from_secs(10);

/// The longest `Retry-After` honoured; a host asking for more is given up on.
const MAX_RETRY_AFTER: Duration = Duration::from_secs(4);
/// Pause before retrying a rate-limited request that sent no `Retry-After`; doubles per attempt.
const RATE_LIMIT_BACKOFF: Duration = Duration::from_millis(800);
/// Retries of a 429/503 (other transient failures keep their single retry).
const RATE_LIMIT_RETRIES: u8 = 2;
/// Requests to one host start at least this far apart, so a gallery does not hit it in one burst.
const HOST_SPACING: Duration = Duration::from_millis(250);

const CLOUDFLARE_DOH_HOST: &str = "cloudflare-dns.com";
const CLOUDFLARE_BOOTSTRAP_IPS: &[IpAddr] = &[
    IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)),
    IpAddr::V4(Ipv4Addr::new(1, 0, 0, 1)),
    IpAddr::V6(Ipv6Addr::new(0x2606, 0x4700, 0x4700, 0, 0, 0, 0, 0x1111)),
    IpAddr::V6(Ipv6Addr::new(0x2606, 0x4700, 0x4700, 0, 0, 0, 0, 0x1001)),
];

const fn user_agent() -> &'static str {
    concat!("NPi-Desktop/", env!("CARGO_PKG_VERSION"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FetchErrorKind {
    Cancelled,
    Fatal,
    Transient,
}

/// A transfer failure. `Transient` may be retried once (connect/timeout, 408,
/// 429, 5xx, a truncated body); `Fatal` never is: the URL, DNS answer, size or
/// image signature failed validation, and retrying would repeat the same
/// refusal. Messages never echo the URL (and therefore no query string).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FetchError {
    kind: FetchErrorKind,
    message: String,
    /// The server's `Retry-After` on a 429/503, capped at [`MAX_RETRY_AFTER`].
    retry_after: Option<Duration>,
}

impl FetchError {
    fn cancelled() -> Self {
        Self {
            kind: FetchErrorKind::Cancelled,
            message: "image request cancelled".to_string(),
            retry_after: None,
        }
    }

    fn fatal(message: impl Into<String>) -> Self {
        Self {
            kind: FetchErrorKind::Fatal,
            message: message.into(),
            retry_after: None,
        }
    }

    fn transient(message: impl Into<String>) -> Self {
        Self {
            kind: FetchErrorKind::Transient,
            message: message.into(),
            retry_after: None,
        }
    }

    /// A 429 or 503: the host is asking for fewer or slower requests.
    fn is_rate_limited(&self) -> bool {
        self.message.ends_with("429 Too Many Requests") || self.message.ends_with("503 Service Unavailable")
    }

    pub(super) fn is_cancelled(&self) -> bool {
        self.kind == FetchErrorKind::Cancelled
    }

    pub(super) fn into_message(self) -> String {
        self.message
    }
}

pub(super) struct FetchedImage {
    pub bytes: Vec<u8>,
    pub mime_type: &'static str,
    pub validation_ms: u64,
    pub transfer_ms: u64,
}

#[derive(Debug, Deserialize)]
struct CloudflareDnsResponse {
    #[serde(rename = "Status")]
    status: i32,
    #[serde(rename = "Answer")]
    answer: Option<Vec<CloudflareDnsAnswer>>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CloudflareDnsAnswer {
    #[serde(rename = "type")]
    record_type: u16,
    data: String,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum DnsResolutionDecision {
    UseDirect(Vec<SocketAddr>),
    QueryDoH,
    Deny(String),
}

/// Whether `ip` falls within the 198.18.0.0/15 benchmarking range used by Fake-IP proxies.
pub(super) const fn is_fake_ip_benchmark(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            o[0] == 198 && (o[1] == 18 || o[1] == 19)
        }
        IpAddr::V6(_) => false,
    }
}

/// Whether `ip` is a publicly routable IP address.
pub(super) fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_public_ipv4(v4),
        IpAddr::V6(v6) => is_public_ipv6(v6),
    }
}

pub(super) fn is_public_ipv4(v4: Ipv4Addr) -> bool {
    let o = v4.octets();
    !(o[0] == 0
        || o[0] == 10
        || (o[0] == 100 && (o[1] & 0xc0) == 64)
        || o[0] == 127
        || (o[0] == 169 && o[1] == 254)
        || (o[0] == 172 && (16..=31).contains(&o[1]))
        || (o[0] == 192 && o[1] == 0 && (o[2] == 0 || o[2] == 2))
        || (o[0] == 192 && o[1] == 168)
        || (o[0] == 198 && (o[1] == 18 || o[1] == 19 || (o[1] == 51 && o[2] == 100)))
        || (o[0] == 203 && o[1] == 0 && o[2] == 113)
        || o[0] >= 224)
}

pub(super) fn is_public_ipv6(v6: Ipv6Addr) -> bool {
    let s = v6.segments();
    // IPv4-mapped IPv6 (::ffff:0:0/96)
    if s[0..5] == [0; 5] && s[5] == 0xffff {
        let v4 = Ipv4Addr::new(
            (s[6] >> 8) as u8,
            (s[6] & 0xff) as u8,
            (s[7] >> 8) as u8,
            (s[7] & 0xff) as u8,
        );
        return is_public_ipv4(v4);
    }

    // Must be in Global Unicast 2000::/3
    if (s[0] & 0xe000) != 0x2000 {
        return false;
    }

    // Exclude IETF special 2001::/23 (covers Teredo 2001:0000::/32, benchmark 2001:2::/48)
    if s[0] == 0x2001 && (s[1] & 0xfe00) == 0 {
        return false;
    }
    // Exclude documentation 2001:db8::/32 and 3fff::/20
    if (s[0] == 0x2001 && s[1] == 0x0db8) || (s[0] == 0x3fff && (s[1] & 0xf000) == 0) {
        return false;
    }
    // Exclude deprecated 6to4 2002::/16
    if s[0] == 0x2002 {
        return false;
    }

    true
}

pub(super) fn evaluate_system_dns_answers(
    domain: &str,
    addrs: &[SocketAddr],
) -> DnsResolutionDecision {
    if addrs.is_empty() {
        return DnsResolutionDecision::Deny(format!("DNS returned no addresses for {domain}"));
    }

    let mut has_fake_ip = false;
    let mut has_other_non_public = false;
    let mut first_non_public = None;

    for sa in addrs {
        let ip = sa.ip();
        if is_fake_ip_benchmark(ip) {
            has_fake_ip = true;
        } else if !is_public_ip(ip) {
            has_other_non_public = true;
            if first_non_public.is_none() {
                first_non_public = Some(ip);
            }
        }
    }

    if has_other_non_public {
        return DnsResolutionDecision::Deny(format!(
            "DNS resolution returned non-public IP {} for {domain}",
            first_non_public.unwrap()
        ));
    }

    if has_fake_ip {
        return DnsResolutionDecision::QueryDoH;
    }

    DnsResolutionDecision::UseDirect(addrs.to_vec())
}

pub(super) fn parse_doh_answers(json_str: &str, port: u16) -> Result<Vec<SocketAddr>, String> {
    let resp: CloudflareDnsResponse =
        serde_json::from_str(json_str).map_err(|e| format!("invalid Cloudflare DoH JSON: {e}"))?;

    if let Some(err) = resp.error {
        return Err(format!("Cloudflare DoH error: {err}"));
    }

    if resp.status != 0 {
        if resp.status == 3 {
            return Err("domain does not exist (NXDOMAIN)".to_string());
        }
        return Err(format!(
            "Cloudflare DoH returned DNS status code {}",
            resp.status
        ));
    }

    let mut addrs = Vec::new();
    if let Some(answers) = resp.answer {
        for ans in answers {
            if ans.record_type == 1 || ans.record_type == 28 {
                if let Ok(ip) = ans.data.trim().parse::<IpAddr>() {
                    if is_public_ip(ip) && !is_fake_ip_benchmark(ip) {
                        addrs.push(SocketAddr::new(ip, port));
                    }
                }
            }
        }
    }

    Ok(addrs)
}

pub(super) fn detect_image_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else if bytes.starts_with(b"BM") {
        Some("image/bmp")
    } else if bytes.starts_with(b"\x00\x00\x01\x00") {
        Some("image/x-icon")
    } else if bytes.len() >= 12
        && &bytes[4..8] == b"ftyp"
        && (&bytes[8..12] == b"avif" || &bytes[8..12] == b"avis")
    {
        Some("image/avif")
    } else if is_svg(bytes) {
        Some("image/svg+xml")
    } else {
        None
    }
}

fn is_svg(bytes: &[u8]) -> bool {
    std::str::from_utf8(bytes).is_ok_and(|s| {
        let trimmed = s.trim_start_matches('\u{feff}').trim_start();
        trimmed.starts_with("<svg") || (trimmed.starts_with("<?xml") && trimmed.contains("<svg"))
    })
}

fn milliseconds(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn remaining(deadline: Instant) -> Result<Duration, FetchError> {
    let left = deadline.saturating_duration_since(Instant::now());
    if left.is_zero() {
        return Err(FetchError::transient("image request timed out"));
    }
    Ok(left)
}

/// URL-level preflight, before any socket is opened: HTTPS, no userinfo, and a
/// literal host IP must already be public. Domain hosts are resolved (and
/// pinned) in [`resolve_domain_addrs`]; every redirect target repeats this.
fn validate_url_target(url: &Url) -> Result<(), FetchError> {
    if url.scheme() != "https" {
        return Err(FetchError::fatal("only HTTPS URLs are permitted"));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(FetchError::fatal(
            "URL userinfo credentials are not permitted",
        ));
    }
    match url.host() {
        Some(Host::Ipv4(v4)) if !is_public_ipv4(v4) => Err(FetchError::fatal(format!(
            "non-public IP address {v4} is denied"
        ))),
        Some(Host::Ipv6(v6)) if !is_public_ipv6(v6) => Err(FetchError::fatal(format!(
            "non-public IP address {v6} is denied"
        ))),
        Some(_) => Ok(()),
        None => Err(FetchError::fatal("URL missing host")),
    }
}

fn describe_send_error(error: &reqwest::Error) -> FetchError {
    if error.is_timeout() {
        FetchError::transient("request timed out")
    } else if error.is_connect() {
        FetchError::transient("connection failed")
    } else {
        FetchError::transient("network request failed")
    }
}

fn describe_body_error(error: &reqwest::Error) -> FetchError {
    if error.is_timeout() {
        FetchError::transient("response body timed out")
    } else {
        FetchError::transient("response body read failed")
    }
}

/// The `Retry-After` header as whole seconds (the HTTP-date form is not used), within [`MAX_RETRY_AFTER`].
fn retry_after(headers: &reqwest::header::HeaderMap) -> Option<Duration> {
    let secs: u64 = headers.get(reqwest::header::RETRY_AFTER)?.to_str().ok()?.trim().parse().ok()?;
    Some(Duration::from_secs(secs).min(MAX_RETRY_AFTER))
}

/// The wait before the retry after `attempt` failures of `error`: the host's own hint, else a doubling backoff with a little jitter.
fn retry_delay(error: &FetchError, attempt: u8) -> Duration {
    if let Some(hint) = error.retry_after {
        return hint;
    }
    let base = RATE_LIMIT_BACKOFF * 2u32.pow(u32::from(attempt.saturating_sub(1)));
    let jitter = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::from(d.subsec_millis()) % 250);
    base + Duration::from_millis(jitter)
}

/// When the next request to `host` may start, and the slot it takes.
fn host_slot(host: &str) -> Instant {
    static NEXT: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, Instant>>> =
        std::sync::OnceLock::new();
    let mut map = NEXT
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let now = Instant::now();
    map.retain(|_, at| *at + Duration::from_secs(30) > now);
    let start = map.get(host).map_or(now, |at| (*at).max(now));
    map.insert(host.to_string(), start + HOST_SPACING);
    start
}

fn http_status_error(status: reqwest::StatusCode) -> FetchError {
    let code = status.as_u16();
    let retryable = code == 408 || code == 425 || code == 429 || (500..=599).contains(&code);
    let message = format!("HTTP error status: {status}");
    if retryable {
        FetchError::transient(message)
    } else {
        FetchError::fatal(message)
    }
}

/// Read at most `cap` bytes, watching the cancel token between chunks. The
/// per-client `timeout` bounds a stalled server; cancellation drops the
/// response future immediately.
async fn read_body_capped(
    response: &mut reqwest::Response,
    cap: u64,
    token: &CancelToken,
    limit_message: &str,
) -> Result<Vec<u8>, FetchError> {
    let mut bytes = Vec::new();
    loop {
        let chunk = with_cancel(token, response.chunk())
            .await
            .map_err(|_| FetchError::cancelled())?
            .map_err(|e| describe_body_error(&e))?;
        let Some(chunk) = chunk else {
            break;
        };
        if bytes.len() as u64 + chunk.len() as u64 > cap {
            return Err(FetchError::fatal(limit_message));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

async fn resolve_domain_addrs(
    domain: &str,
    port: u16,
    token: &CancelToken,
    deadline: Instant,
) -> Result<Vec<SocketAddr>, FetchError> {
    let lookup_domain = domain.to_string();
    let join = with_cancel(
        token,
        tauri::async_runtime::spawn_blocking(move || {
            (lookup_domain.as_str(), port)
                .to_socket_addrs()
                .map(std::iter::Iterator::collect::<Vec<_>>)
        }),
    )
    .await
    .map_err(|_| FetchError::cancelled())?;
    let resolved = join
        .map_err(|e| FetchError::transient(format!("DNS lookup task failed: {e}")))?
        .map_err(|e| FetchError::transient(format!("DNS resolution failed for {domain}: {e}")))?;

    match evaluate_system_dns_answers(domain, &resolved) {
        DnsResolutionDecision::UseDirect(addrs) => Ok(addrs),
        DnsResolutionDecision::QueryDoH => {
            resolve_via_cloudflare_doh(domain, port, token, deadline).await
        }
        DnsResolutionDecision::Deny(reason) => Err(FetchError::fatal(reason)),
    }
}

async fn resolve_via_cloudflare_doh(
    domain: &str,
    port: u16,
    token: &CancelToken,
    deadline: Instant,
) -> Result<Vec<SocketAddr>, FetchError> {
    let bootstrap: Vec<SocketAddr> = CLOUDFLARE_BOOTSTRAP_IPS
        .iter()
        .map(|ip| SocketAddr::new(*ip, 443))
        .collect();

    let budget = remaining(deadline)?;
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(budget.min(DOH_TIMEOUT_CAP))
        .connect_timeout(budget.min(CONNECT_TIMEOUT_CAP))
        .resolve_to_addrs(CLOUDFLARE_DOH_HOST, &bootstrap)
        .build()
        .map_err(|_| FetchError::fatal("failed to build DoH client"))?;

    let mut addrs =
        query_cloudflare_doh_record(&client, domain, "A", port, token, deadline).await?;
    if addrs.is_empty() {
        addrs = query_cloudflare_doh_record(&client, domain, "AAAA", port, token, deadline).await?;
    }
    if addrs.is_empty() {
        return Err(FetchError::fatal(format!(
            "Cloudflare DoH returned no public IP addresses for {domain}"
        )));
    }
    Ok(addrs)
}

async fn query_cloudflare_doh_record(
    client: &reqwest::Client,
    domain: &str,
    record_type: &str,
    port: u16,
    token: &CancelToken,
    deadline: Instant,
) -> Result<Vec<SocketAddr>, FetchError> {
    remaining(deadline)?;
    let url = format!("https://{CLOUDFLARE_DOH_HOST}/dns-query?name={domain}&type={record_type}");
    let response = with_cancel(
        token,
        client
            .get(url.as_str())
            .header(reqwest::header::ACCEPT, "application/dns-json")
            .header(reqwest::header::USER_AGENT, user_agent())
            .send(),
    )
    .await
    .map_err(|_| FetchError::cancelled())?
    .map_err(|_| FetchError::transient("Cloudflare DoH request failed"))?;

    let status = response.status();
    if !status.is_success() {
        return Err(FetchError::transient(format!(
            "Cloudflare DoH HTTP error status: {status}"
        )));
    }

    let mut response = response;
    let body = read_body_capped(
        &mut response,
        MAX_DOH_BYTES,
        token,
        "Cloudflare DoH response exceeds 64 KiB",
    )
    .await?;
    let body = String::from_utf8(body)
        .map_err(|_| FetchError::fatal("invalid UTF-8 in Cloudflare DoH response"))?;
    parse_doh_answers(&body, port).map_err(FetchError::fatal)
}

/// Fetch one public HTTPS image. Transient failures get a single retry; a 429
/// or 503 gets up to [`RATE_LIMIT_RETRIES`], after the host's `Retry-After` or
/// a growing pause. Validation failures and cancellation never retry. Requests
/// to one host start [`HOST_SPACING`] apart.
pub(super) async fn fetch_public_https_image(
    url_str: &str,
    token: &CancelToken,
    deadline: Instant,
) -> Result<FetchedImage, FetchError> {
    let host = Url::parse(url_str)
        .ok()
        .and_then(|u| u.host_str().map(str::to_ascii_lowercase))
        .unwrap_or_default();
    let mut attempt = 0u8;
    loop {
        if token.is_cancelled() {
            return Err(FetchError::cancelled());
        }
        let wait = host_slot(&host).saturating_duration_since(Instant::now());
        if !wait.is_zero() {
            with_cancel(token, tokio::time::sleep(wait))
                .await
                .map_err(|_| FetchError::cancelled())?;
        }
        match fetch_once(url_str, token, deadline).await {
            Ok(image) => return Ok(image),
            Err(error) => {
                let limited = error.is_rate_limited();
                let allowed = if limited { RATE_LIMIT_RETRIES } else { 1 };
                attempt += 1;
                let delay = if limited { retry_delay(&error, attempt) } else { Duration::ZERO };
                let budget_left = deadline.saturating_duration_since(Instant::now());
                if error.kind != FetchErrorKind::Transient
                    || attempt > allowed
                    || budget_left <= RETRY_MIN_BUDGET + delay
                {
                    return Err(error);
                }
                if !delay.is_zero() {
                    with_cancel(token, tokio::time::sleep(delay))
                        .await
                        .map_err(|_| FetchError::cancelled())?;
                }
            }
        }
    }
}

fn build_client(
    host: &str,
    addrs: &[SocketAddr],
    deadline: Instant,
) -> Result<reqwest::Client, FetchError> {
    let budget = remaining(deadline)?;
    reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(budget)
        .connect_timeout(budget.min(CONNECT_TIMEOUT_CAP))
        .resolve_to_addrs(host, addrs)
        .build()
        .map_err(|_| FetchError::fatal("failed to build HTTP client"))
}

async fn send_request(
    client: &reqwest::Client,
    url: &Url,
    token: &CancelToken,
) -> Result<reqwest::Response, FetchError> {
    with_cancel(
        token,
        client
            .get(url.as_str())
            .header(reqwest::header::USER_AGENT, user_agent())
            .send(),
    )
    .await
    .map_err(|_| FetchError::cancelled())?
    .map_err(|e| describe_send_error(&e))
}

async fn read_image_response(
    response: reqwest::Response,
    token: &CancelToken,
    validation: Duration,
) -> Result<FetchedImage, FetchError> {
    let status = response.status();
    if !status.is_success() {
        let mut error = http_status_error(status);
        error.retry_after = retry_after(response.headers());
        return Err(error);
    }
    if let Some(length) = response.content_length() {
        if length > MAX_IMAGE_BYTES {
            return Err(FetchError::fatal(format!(
                "image size ({length} bytes) exceeds maximum 8 MiB"
            )));
        }
    }

    let transfer_started = Instant::now();
    let mut response = response;
    let bytes = read_body_capped(
        &mut response,
        MAX_IMAGE_BYTES,
        token,
        "image size exceeds maximum 8 MiB",
    )
    .await?;
    let transfer = transfer_started.elapsed();

    let mime_type = detect_image_mime(&bytes)
        .ok_or_else(|| FetchError::fatal("unsupported image format or invalid signature"))?;
    if mime_type == "image/svg+xml" {
        return Err(FetchError::fatal("remote SVG images are not permitted"));
    }

    Ok(FetchedImage {
        bytes,
        mime_type,
        validation_ms: milliseconds(validation),
        transfer_ms: milliseconds(transfer),
    })
}

async fn fetch_once(
    url_str: &str,
    token: &CancelToken,
    deadline: Instant,
) -> Result<FetchedImage, FetchError> {
    let mut validation = Duration::ZERO;
    let mut current_url =
        Url::parse(url_str).map_err(|e| FetchError::fatal(format!("invalid image URL: {e}")))?;
    let mut redirects = 0usize;

    loop {
        if token.is_cancelled() {
            return Err(FetchError::cancelled());
        }
        remaining(deadline)?;
        validate_url_target(&current_url)?;

        let port = current_url.port_or_known_default().unwrap_or(443);
        let validation_started = Instant::now();
        let (host_str, addrs) = match current_url.host() {
            Some(Host::Ipv4(v4)) => (v4.to_string(), vec![SocketAddr::new(IpAddr::V4(v4), port)]),
            Some(Host::Ipv6(v6)) => (v6.to_string(), vec![SocketAddr::new(IpAddr::V6(v6), port)]),
            Some(Host::Domain(domain)) => (
                domain.to_string(),
                resolve_domain_addrs(domain, port, token, deadline).await?,
            ),
            None => return Err(FetchError::fatal("URL missing host")),
        };
        validation += validation_started.elapsed();

        // One request per hop: redirects are followed by this loop, so each
        // target is preflighted and pinned exactly like the first URL.
        let client = build_client(&host_str, &addrs, deadline)?;
        let response = send_request(&client, &current_url, token).await?;
        let status = response.status();
        if status.is_redirection() {
            redirects += 1;
            if redirects > MAX_REDIRECTS {
                return Err(FetchError::fatal("too many HTTP redirects"));
            }
            let location = response
                .headers()
                .get(reqwest::header::LOCATION)
                .ok_or_else(|| {
                    FetchError::fatal(format!(
                        "HTTP redirect {} missing Location header",
                        status.as_u16()
                    ))
                })?
                .to_str()
                .map_err(|_| FetchError::fatal("invalid redirect Location header"))?;
            current_url = current_url
                .join(location)
                .map_err(|e| FetchError::fatal(format!("invalid redirect target: {e}")))?;
            continue;
        }
        return read_image_response(response, token, validation).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_ip_benchmark_identification() {
        assert!(is_fake_ip_benchmark("198.18.0.1".parse().unwrap()));
        assert!(is_fake_ip_benchmark("198.18.1.170".parse().unwrap()));
        assert!(is_fake_ip_benchmark("198.19.255.255".parse().unwrap()));

        assert!(!is_fake_ip_benchmark("198.20.0.1".parse().unwrap()));
        assert!(!is_fake_ip_benchmark("192.168.1.1".parse().unwrap()));
        assert!(!is_fake_ip_benchmark("8.8.8.8".parse().unwrap()));
    }

    #[test]
    fn dns_resolution_decision_triggers_doh_only_for_pure_fake_ip() {
        let public_sa = SocketAddr::new("93.184.216.34".parse().unwrap(), 443);
        let fake_sa = SocketAddr::new("198.18.1.170".parse().unwrap(), 443);
        let private_sa = SocketAddr::new("192.168.1.5".parse().unwrap(), 443);
        let loopback_sa = SocketAddr::new("127.0.0.1".parse().unwrap(), 443);

        assert_eq!(
            evaluate_system_dns_answers("example.com", &[public_sa]),
            DnsResolutionDecision::UseDirect(vec![public_sa])
        );

        assert_eq!(
            evaluate_system_dns_answers("httpbin.org", &[fake_sa]),
            DnsResolutionDecision::QueryDoH
        );

        match evaluate_system_dns_answers("corp.internal", &[private_sa]) {
            DnsResolutionDecision::Deny(msg) => assert!(msg.contains("192.168.1.5")),
            other => panic!("expected Deny, got {other:?}"),
        }

        match evaluate_system_dns_answers("localhost", &[loopback_sa]) {
            DnsResolutionDecision::Deny(msg) => assert!(msg.contains("127.0.0.1")),
            other => panic!("expected Deny, got {other:?}"),
        }

        match evaluate_system_dns_answers("mixed.host", &[fake_sa, private_sa]) {
            DnsResolutionDecision::Deny(msg) => assert!(msg.contains("192.168.1.5")),
            other => panic!("expected Deny, got {other:?}"),
        }

        match evaluate_system_dns_answers("none.host", &[]) {
            DnsResolutionDecision::Deny(msg) => assert!(msg.contains("no addresses")),
            other => panic!("expected Deny, got {other:?}"),
        }
    }

    #[test]
    fn doh_json_parser_extracts_public_ips_and_filters_private() {
        let json_ok = r#"{
            "Status": 0,
            "Answer": [
                { "name": "httpbin.org.", "type": 1, "data": "3.137.48.32" },
                { "name": "httpbin.org.", "type": 1, "data": "10.0.0.1" },
                { "name": "httpbin.org.", "type": 1, "data": "198.18.1.1" },
                { "name": "httpbin.org.", "type": 28, "data": "2606:4700:4700::1111" }
            ]
        }"#;

        let addrs = parse_doh_answers(json_ok, 443).unwrap();
        assert_eq!(addrs.len(), 2);
        assert_eq!(
            addrs[0],
            SocketAddr::new("3.137.48.32".parse().unwrap(), 443)
        );
        assert_eq!(
            addrs[1],
            SocketAddr::new("2606:4700:4700::1111".parse().unwrap(), 443)
        );

        let json_nxdomain = r#"{ "Status": 3, "Answer": [] }"#;
        assert!(parse_doh_answers(json_nxdomain, 443).is_err());

        let json_err = r#"{ "Status": 2, "error": "Server failure" }"#;
        assert!(parse_doh_answers(json_err, 443).is_err());
    }

    #[test]
    fn public_ipv4_accepts_public_addresses() {
        for ip in [
            "8.8.8.8",
            "1.1.1.1",
            "93.184.216.34",
            "140.82.121.4",
            "151.101.1.140",
        ] {
            assert!(
                is_public_ip(ip.parse().unwrap()),
                "{ip} should be accepted as public"
            );
        }
    }

    #[test]
    fn public_ipv4_rejects_private_and_reserved_addresses() {
        for ip in [
            "0.0.0.0",
            "10.0.0.1",
            "10.255.255.255",
            "100.64.0.1",
            "100.127.255.254",
            "127.0.0.1",
            "127.255.255.255",
            "169.254.169.254",
            "172.16.0.1",
            "172.31.255.255",
            "192.0.0.1",
            "192.0.2.1",
            "192.168.0.1",
            "192.168.255.255",
            "198.18.0.1",
            "198.19.255.255",
            "198.51.100.1",
            "203.0.113.1",
            "224.0.0.1",
            "239.255.255.255",
            "240.0.0.1",
            "255.255.255.255",
        ] {
            assert!(
                !is_public_ip(ip.parse().unwrap()),
                "{ip} should be rejected as non-public"
            );
        }
    }

    #[test]
    fn public_ipv6_accepts_global_unicast() {
        for ip in [
            "2606:4700:4700::1111",
            "2001:4860:4860::8888",
            "2a00:1450:4009:820::200e",
        ] {
            assert!(
                is_public_ip(ip.parse().unwrap()),
                "{ip} should be accepted as public"
            );
        }
    }

    #[test]
    fn public_ipv6_rejects_non_global_and_special() {
        for ip in [
            "::",
            "::1",
            "::2",
            "100::1",
            "2001:0::1",
            "2001:2::1",
            "2001:db8::1",
            "2002::1",
            "3fff::1",
            "fc00::1",
            "fd12:3456:789a::1",
            "fe80::1",
            "fec0::1",
            "ff02::1",
            "::ffff:127.0.0.1",
            "::ffff:10.0.0.1",
            "::ffff:192.168.1.1",
            "::ffff:169.254.1.1",
        ] {
            assert!(
                !is_public_ip(ip.parse().unwrap()),
                "{ip} should be rejected as non-public"
            );
        }
    }

    #[test]
    fn detects_supported_image_signatures() {
        assert_eq!(
            detect_image_mime(b"\x89PNG\r\n\x1a\n\x00\x00"),
            Some("image/png")
        );
        assert_eq!(detect_image_mime(b"\xff\xd8\xff\xe0"), Some("image/jpeg"));
        assert_eq!(detect_image_mime(b"GIF89a\x01\x00"), Some("image/gif"));
        assert_eq!(detect_image_mime(b"GIF87a\x01\x00"), Some("image/gif"));
        assert_eq!(
            detect_image_mime(b"RIFF\x00\x00\x00\x00WEBPVP8"),
            Some("image/webp")
        );
        assert_eq!(detect_image_mime(b"BM\x36\x00"), Some("image/bmp"));
        assert_eq!(
            detect_image_mime(b"\x00\x00\x01\x00\x01\x00"),
            Some("image/x-icon")
        );
        assert_eq!(
            detect_image_mime(b"\x00\x00\x00\x20ftypavif\x00\x00"),
            Some("image/avif")
        );
        assert_eq!(
            detect_image_mime(b"<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>"),
            Some("image/svg+xml")
        );
        assert_eq!(
            detect_image_mime(
                b"<?xml version=\"1.0\"?><svg xmlns=\"http://www.w3.org/2000/svg\"></svg>"
            ),
            Some("image/svg+xml")
        );
        assert_eq!(detect_image_mime(b"random binary bytes"), None);
    }

    #[test]
    fn url_preflight_rejects_non_https_userinfo_and_private_hosts() {
        for (url, kind) in [
            ("http://example.com/insecure.png", FetchErrorKind::Fatal),
            (
                "https://user:pass@example.com/img.png",
                FetchErrorKind::Fatal,
            ),
            ("https://127.0.0.1/img.png", FetchErrorKind::Fatal),
            ("https://192.168.1.1/img.png", FetchErrorKind::Fatal),
            ("https://[::1]/img.png", FetchErrorKind::Fatal),
            ("https://example.com/img.png", FetchErrorKind::Fatal),
        ] {
            let parsed = Url::parse(url).unwrap();
            let result = validate_url_target(&parsed);
            if url == "https://example.com/img.png" {
                assert!(result.is_ok(), "public domain host must pass preflight");
            } else {
                assert_eq!(result.unwrap_err().kind, kind);
            }
        }
    }

    #[test]
    fn retry_after_is_whole_seconds_within_the_cap() {
        let mut h = reqwest::header::HeaderMap::new();
        h.insert(reqwest::header::RETRY_AFTER, "2".parse().unwrap());
        assert_eq!(retry_after(&h), Some(Duration::from_secs(2)));
        h.insert(reqwest::header::RETRY_AFTER, "600".parse().unwrap());
        assert_eq!(retry_after(&h), Some(MAX_RETRY_AFTER));
        h.insert(reqwest::header::RETRY_AFTER, "Wed, 21 Oct 2026 07:28:00 GMT".parse().unwrap());
        assert_eq!(retry_after(&h), None);
    }

    #[test]
    fn the_rate_limit_pause_follows_the_hint_else_doubles() {
        let mut e = http_status_error(reqwest::StatusCode::TOO_MANY_REQUESTS);
        assert!(retry_delay(&e, 1) >= RATE_LIMIT_BACKOFF);
        assert!(retry_delay(&e, 2) >= RATE_LIMIT_BACKOFF * 2);
        e.retry_after = Some(Duration::from_secs(1));
        assert_eq!(retry_delay(&e, 2), Duration::from_secs(1));
    }

    #[test]
    fn requests_to_one_host_are_spaced() {
        let first = host_slot("spacing.test");
        let second = host_slot("spacing.test");
        assert!(second >= first + HOST_SPACING);
        assert!(host_slot("other.test") <= second);
    }

    #[test]
    fn retry_classification_separates_transient_from_validation() {
        assert_eq!(
            http_status_error(reqwest::StatusCode::SERVICE_UNAVAILABLE).kind,
            FetchErrorKind::Transient
        );
        assert_eq!(
            http_status_error(reqwest::StatusCode::TOO_MANY_REQUESTS).kind,
            FetchErrorKind::Transient
        );
        assert_eq!(
            http_status_error(reqwest::StatusCode::NOT_FOUND).kind,
            FetchErrorKind::Fatal
        );
        assert_eq!(FetchError::fatal("bad").kind, FetchErrorKind::Fatal);
        assert!(http_status_error(reqwest::StatusCode::TOO_MANY_REQUESTS).is_rate_limited());
        assert!(!http_status_error(reqwest::StatusCode::BAD_GATEWAY).is_rate_limited());
        assert!(FetchError::cancelled().is_cancelled());
    }
}
