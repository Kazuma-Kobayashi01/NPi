//! USD→JPY reference rate for the Office window's meter.
//!
//! Fetched here, not from the webview, so the page's CSP stays closed. The
//! source is Frankfurter (European Central Bank daily reference rates, no
//! key); the request names the currency pair and nothing else. The last
//! rate is cached in `<app cache>/fx.json` and reused for a day; when a
//! refresh fails the cached rate is returned marked stale.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

const URL: &str = "https://api.frankfurter.dev/v1/latest?base=USD&symbols=JPY";
const MAX_AGE_SECS: u64 = 24 * 60 * 60;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Cached {
    rate: f64,
    /// The rate's own date, `YYYY-MM-DD`.
    date: String,
    /// When it was fetched, seconds since the epoch.
    fetched_at: u64,
}

/// What the meter shows: `rate` is `None` when no rate was ever fetched.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FxRate {
    rate: Option<f64>,
    date: Option<String>,
    /// The rate is older than a day and could not be refreshed.
    stale: bool,
}

/// The rate and its date out of Frankfurter's `{"date":…,"rates":{"JPY":…}}`.
fn parse(body: &str) -> Option<(f64, String)> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    let rate = value.get("rates")?.get("JPY")?.as_f64()?;
    let date = value.get("date")?.as_str()?;
    let plain_date = date.len() == 10 && date.bytes().all(|b| b.is_ascii_digit() || b == b'-');
    (rate.is_finite() && rate > 0.0 && plain_date).then(|| (rate, date.to_string()))
}

const fn is_fresh(cached: &Cached, now: u64) -> bool {
    now.saturating_sub(cached.fetched_at) < MAX_AGE_SECS
}

/// The answer given `cached` and the outcome of a refresh (`None`: not tried or failed).
fn answer(cached: Option<&Cached>, fetched: Option<&Cached>, now: u64) -> FxRate {
    fetched.or(cached).map_or(
        FxRate {
            rate: None,
            date: None,
            stale: true,
        },
        |c| FxRate {
            rate: Some(c.rate),
            date: Some(c.date.clone()),
            stale: !is_fresh(c, now),
        },
    )
}

fn read_cache(path: &Path) -> Option<Cached> {
    let cached: Cached = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
    (cached.rate.is_finite() && cached.rate > 0.0).then_some(cached)
}

fn fetch() -> Option<(f64, String)> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(8))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("NPi-Desktop")
        .build()
        .ok()?;
    let response = client.get(URL).send().ok()?;
    if !response.status().is_success() {
        return None;
    }
    parse(&response.text().ok()?)
}

fn lookup(path: Option<PathBuf>) -> FxRate {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let cached = path.as_deref().and_then(read_cache);
    if cached.as_ref().is_some_and(|c| is_fresh(c, now)) {
        return answer(cached.as_ref(), None, now);
    }
    let fetched = fetch().map(|(rate, date)| Cached {
        rate,
        date,
        fetched_at: now,
    });
    if let (Some(fresh), Some(path)) = (&fetched, &path) {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(bytes) = serde_json::to_vec(fresh) {
            let _ = std::fs::write(path, bytes);
        }
    }
    answer(cached.as_ref(), fetched.as_ref(), now)
}

/// The USD→JPY rate: cached for a day, refreshed online after that.
#[tauri::command]
pub async fn fx_rate_usd_jpy() -> Result<FxRate, String> {
    let path = crate::npi_dirs::cache_dir()
        .ok()
        .map(|dir| dir.join("fx.json"));
    tauri::async_runtime::spawn_blocking(move || lookup(path))
        .await
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cached(fetched_at: u64) -> Cached {
        Cached {
            rate: 150.25,
            date: "2026-11-10".to_string(),
            fetched_at,
        }
    }

    #[test]
    fn reads_the_rate_and_date() {
        let body = r#"{"amount":1.0,"base":"USD","date":"2026-11-10","rates":{"JPY":150.25}}"#;
        assert_eq!(parse(body), Some((150.25, "2026-11-10".to_string())));
    }

    #[test]
    fn refuses_a_reply_without_a_usable_rate() {
        assert_eq!(parse(r#"{"date":"2026-11-10","rates":{}}"#), None);
        assert_eq!(parse(r#"{"date":"2026-11-10","rates":{"JPY":0}}"#), None);
        assert_eq!(parse(r#"{"date":"<b>","rates":{"JPY":150}}"#), None);
        assert_eq!(parse("not json"), None);
    }

    #[test]
    fn a_rate_is_fresh_for_a_day() {
        assert!(is_fresh(&cached(1_000), 1_000 + MAX_AGE_SECS - 1));
        assert!(!is_fresh(&cached(1_000), 1_000 + MAX_AGE_SECS));
    }

    #[test]
    fn a_failed_refresh_keeps_the_old_rate_marked_stale() {
        let old = cached(0);
        let now = MAX_AGE_SECS * 3;
        let kept = answer(Some(&old), None, now);
        assert_eq!(kept.rate, Some(150.25));
        assert!(kept.stale);
        let fresh = Cached {
            rate: 151.0,
            date: "2026-11-13".to_string(),
            fetched_at: now,
        };
        let refreshed = answer(Some(&old), Some(&fresh), now);
        assert_eq!(refreshed.rate, Some(151.0));
        assert!(!refreshed.stale);
    }

    #[test]
    fn no_rate_at_all_is_reported_as_none() {
        assert_eq!(
            answer(None, None, 5),
            FxRate {
                rate: None,
                date: None,
                stale: true
            }
        );
    }
}
