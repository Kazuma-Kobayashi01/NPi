//! OS notifications (`docs/architecture/notifications.md`).
//!
//! Rust owns the policy because only Rust sees every tab's frames: a tab in
//! the background is not processed by the webview. The stdout reader hands each
//! frame to [`NotifyState::observe`] (cheap: one string compare for most
//! frames), which turns three events into notifications: a run that ended
//! after a while, a question or approval waiting for the user, and a failed
//! request or dead agent process. The webview only configures the switches
//! and the language (`notify_configure`), names the visible tab
//! (`notify_set_active`) and drives the Settings row (`notify_status`,
//! `notify_request_permission`, `notify_test`). A notification carries the
//! project folder name and a generic line, never prompt, reply or file text.

mod policy;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as platform;
#[cfg(not(target_os = "macos"))]
mod other;
#[cfg(not(target_os = "macos"))]
use other as platform;

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use policy::{body, should_send, Config, Kind, Lang, Moment};
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager, State};

/// The system's answer for this app.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Granted,
    Denied,
    NotDetermined,
    /// Not a bundled app (`tauri dev`): nothing to ask.
    Unsupported,
}

impl Status {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Granted => "granted",
            Self::Denied => "denied",
            Self::NotDetermined => "notDetermined",
            Self::Unsupported => "unsupported",
        }
    }
}

/// What a frame means to us.
#[derive(Debug, PartialEq, Eq)]
enum Event {
    RunStart,
    RunEnd { failed: bool },
    Ask,
}

/// `None` for every frame that is not one of the three.
fn classify(frame_type: Option<&str>, frame: &Value) -> Option<Event> {
    match frame_type? {
        "agent_start" => Some(Event::RunStart),
        "agent_end" => {
            // omp's own rule (`isYield`, app/turn-status.js): only a run's final end counts.
            let terminal = frame
                .get("yielded")
                .and_then(Value::as_bool)
                .unwrap_or_else(|| frame.get("isTerminal").and_then(Value::as_bool) != Some(false));
            if !terminal {
                return None;
            }
            let failed = frame
                .get("messages")
                .and_then(Value::as_array)
                .and_then(|m| {
                    m.iter()
                        .rev()
                        .find(|m| m.get("role").and_then(Value::as_str) == Some("assistant"))
                })
                .is_some_and(|m| m.get("stopReason").and_then(Value::as_str) == Some("error"));
            Some(Event::RunEnd { failed })
        }
        "extension_ui_request" => matches!(
            frame.get("method").and_then(Value::as_str),
            Some("select" | "ask" | "confirm")
        )
        .then_some(Event::Ask),
        _ => None,
    }
}

#[derive(Default)]
pub struct NotifyState {
    cfg: Mutex<Config>,
    active: Mutex<Option<String>>,
    runs: Mutex<HashMap<String, Instant>>,
    last: Mutex<HashMap<(String, Kind), Instant>>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// A fresh request identifier part.
fn next_id() -> u64 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

impl NotifyState {
    /// Records the dedupe time and says whether `kind` goes out now.
    fn admit(
        &self,
        sid: &str,
        kind: Kind,
        run: Option<Duration>,
        window_focused: bool,
        now: Instant,
    ) -> bool {
        let cfg = *lock(&self.cfg);
        let tab_active = lock(&self.active).as_deref() == Some(sid);
        let mut last = lock(&self.last);
        let key = (sid.to_string(), kind);
        let moment = Moment {
            window_focused,
            tab_active,
            run,
            since_last: last.get(&key).map(|t| now.saturating_duration_since(*t)),
        };
        let send = should_send(cfg, kind, moment);
        if send {
            last.insert(key, now);
        }
        send
    }

    /// Called by the stdout reader for every parsed frame of tab `sid`.
    pub fn observe(
        &self,
        app: &AppHandle,
        sid: &str,
        project: Option<&Path>,
        frame_type: Option<&str>,
        frame: &Value,
    ) {
        let Some(event) = classify(frame_type, frame) else {
            return;
        };
        let (kind, run) = match event {
            Event::RunStart => {
                lock(&self.runs).insert(sid.to_string(), Instant::now());
                return;
            }
            Event::RunEnd { failed } => {
                let run = lock(&self.runs).remove(sid).map(|t| t.elapsed());
                (if failed { Kind::Failed } else { Kind::Done }, run)
            }
            Event::Ask => (Kind::NeedsInput, None),
        };
        self.dispatch(app, sid, project, kind, run);
    }

    /// The agent process of `sid` ended; a non-empty `reason` is a crash.
    pub fn session_exited(&self, app: &AppHandle, sid: &str, project: Option<&Path>, reason: &str) {
        lock(&self.runs).remove(sid);
        lock(&self.last).retain(|(s, _), _| s != sid);
        if !reason.is_empty() {
            self.dispatch(app, sid, project, Kind::Failed, None);
        }
    }

    fn dispatch(
        &self,
        app: &AppHandle,
        sid: &str,
        project: Option<&Path>,
        kind: Kind,
        run: Option<Duration>,
    ) {
        let focused = app
            .get_webview_window("main")
            .and_then(|w| w.is_focused().ok())
            .unwrap_or(false);
        if !self.admit(sid, kind, run, focused, Instant::now()) {
            return;
        }
        let lang = lock(&self.cfg).lang;
        let title = project
            .and_then(Path::file_name)
            .and_then(|n| n.to_str())
            .map_or_else(
                || "NPi Desktop".to_string(),
                |n| crate::text::prefix_within(n, 60).to_string(),
            );
        let sid = sid.to_string();
        // The system call may block on its answer: never on the reader thread.
        std::thread::spawn(move || platform::post(&title, body(kind, lang), &sid));
    }
}

/// Becomes the notification delegate (macOS); a no-op elsewhere.
pub fn install(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    macos::install(app);
    #[cfg(not(target_os = "macos"))]
    let _ = app;
}

/// A click on a notification: bring the window forward and name the tab.
fn clicked(app: &AppHandle, sid: &str) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
    if !sid.is_empty() {
        let _ = app.emit("notify://open", serde_json::json!({ "sessionId": sid }));
    }
}

/// The Settings switches. Anything off-schema (a language tag we do not speak) keeps the old language.
#[allow(clippy::fn_params_excessive_bools)]
#[tauri::command]
pub fn notify_configure(
    enabled: bool,
    done: bool,
    ask: bool,
    failed: bool,
    lang: String,
    state: State<'_, NotifyState>,
) {
    let mut cfg = lock(&state.cfg);
    *cfg = Config {
        enabled,
        done,
        ask,
        failed,
        lang: Lang::parse(&lang).unwrap_or(cfg.lang),
    };
}

/// The tab on screen (`None`: the start page or Settings).
#[tauri::command]
pub fn notify_set_active(tab: Option<String>, state: State<'_, NotifyState>) {
    *lock(&state.active) = tab;
}

#[tauri::command]
pub async fn notify_status() -> Result<String, String> {
    let status = tauri::async_runtime::spawn_blocking(platform::status)
        .await
        .map_err(|e| e.to_string())?;
    Ok(status.as_str().to_string())
}

/// Asks the system for permission (its prompt shows the first time only).
#[tauri::command]
pub async fn notify_request_permission() -> Result<String, String> {
    let status = tauri::async_runtime::spawn_blocking(platform::request)
        .await
        .map_err(|e| e.to_string())?;
    Ok(status.as_str().to_string())
}

/// What the Settings row shows beside the state: the system's last refusal and how this build is signed.
#[tauri::command]
pub async fn notify_diagnose() -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(|| {
        serde_json::json!({ "error": platform::last_error(), "signing": platform::signing() })
    })
    .await
    .map_err(|e| e.to_string())
}

/// The Settings "Send test notification" button. Resolves to the permission state.
#[tauri::command]
pub async fn notify_test(state: State<'_, NotifyState>) -> Result<String, String> {
    let lang = lock(&state.cfg).lang;
    let status = tauri::async_runtime::spawn_blocking(move || {
        let status = platform::status();
        if status == Status::Granted {
            platform::post("NPi Desktop", body(Kind::Test, lang), "");
        }
        status
    })
    .await
    .map_err(|e| e.to_string())?;
    Ok(status.as_str().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn state_on() -> NotifyState {
        let s = NotifyState::default();
        lock(&s.cfg).enabled = true;
        s
    }

    #[test]
    fn frames_classify_into_the_three_events_and_nothing_else() {
        assert_eq!(
            classify(Some("agent_start"), &json!({})),
            Some(Event::RunStart)
        );
        assert_eq!(
            classify(Some("agent_end"), &json!({})),
            Some(Event::RunEnd { failed: false })
        );
        assert_eq!(classify(Some("message_update"), &json!({})), None);
        assert_eq!(classify(None, &json!({})), None);
        let ask = json!({ "method": "select" });
        assert_eq!(
            classify(Some("extension_ui_request"), &ask),
            Some(Event::Ask)
        );
        assert_eq!(
            classify(Some("extension_ui_request"), &json!({ "method": "notify" })),
            None
        );
    }

    #[test]
    fn only_a_terminal_agent_end_counts_and_an_error_reply_is_a_failure() {
        assert_eq!(
            classify(Some("agent_end"), &json!({ "yielded": false })),
            None
        );
        assert_eq!(
            classify(Some("agent_end"), &json!({ "isTerminal": false })),
            None
        );
        let failed = json!({ "messages": [
            { "role": "user" },
            { "role": "assistant", "stopReason": "error" },
        ]});
        assert_eq!(
            classify(Some("agent_end"), &failed),
            Some(Event::RunEnd { failed: true })
        );
        let fine = json!({ "messages": [{ "role": "assistant", "stopReason": "stop" }] });
        assert_eq!(
            classify(Some("agent_end"), &fine),
            Some(Event::RunEnd { failed: false })
        );
    }

    #[test]
    fn the_active_tab_is_silent_only_while_the_window_has_focus() {
        let s = state_on();
        *lock(&s.active) = Some("a".into());
        let t = Instant::now();
        assert!(!s.admit("a", Kind::NeedsInput, None, true, t));
        assert!(s.admit("a", Kind::NeedsInput, None, false, t));
        assert!(s.admit("b", Kind::NeedsInput, None, true, t));
    }

    #[test]
    fn a_burst_gives_one_notification_per_tab_and_kind_and_an_exit_clears_it() {
        let s = state_on();
        let t = Instant::now();
        assert!(s.admit("a", Kind::Failed, None, false, t));
        assert!(!s.admit("a", Kind::Failed, None, false, t + Duration::from_secs(2)));
        assert!(s.admit(
            "a",
            Kind::NeedsInput,
            None,
            false,
            t + Duration::from_secs(2)
        ));
        assert!(s.admit("a", Kind::Failed, None, false, t + policy::DEDUPE));
    }

    #[test]
    fn a_done_notification_needs_a_run_of_some_length() {
        let s = state_on();
        let t = Instant::now();
        assert!(!s.admit("a", Kind::Done, Some(Duration::from_secs(2)), false, t));
        assert!(s.admit("a", Kind::Done, Some(Duration::from_secs(60)), false, t));
    }
}
