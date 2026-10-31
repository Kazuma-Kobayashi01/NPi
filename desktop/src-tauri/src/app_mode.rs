//! The app's mode (Chat or Code) and the system prompt Chat mode gives omp.
//! Code mode leaves omp's own coding prompt as it is.
//!
//! The webview only names the mode; the prompt text lives here, so nothing the
//! page sends becomes part of omp's system prompt. The mode is read by
//! `start_session` when it spawns omp, so a mode change reaches a tab when its
//! omp is (re)started.

use std::sync::Mutex;

/// The whole base system prompt of Chat mode (omp's own default is replaced,
/// not added to; omp still appends the skills list and the tools' own text).
const CHAT_PROMPT: &str = "You are NPi, a capable, warm general-purpose assistant that runs on the \
user's own computer. Help with writing, analysis, research, learning, planning, everyday questions \
and technical ones alike. Answer directly and in the user's language; keep a natural, human tone \
and format only as much as the content needs. Ask one short question when a request is ambiguous \
instead of guessing. You have tools (files, shell, web) and skills: use them when they clearly help \
the user, say what you did, and never treat an ordinary conversation as a programming task. Do not \
make irreversible changes (deleting files, sending things out) without the user's clear go-ahead. \
Always confirm the current git branch is correct before any modification or deletion. \
Never engage in excessive testing, redundant runs, or repetitive verification that waste resources for minimal return. \
Be honest about what you do not know or could not check.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppMode {
    Chat,
    Code,
}

impl AppMode {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "chat" => Some(Self::Chat),
            "code" => Some(Self::Code),
            _ => None,
        }
    }

    /// The base system prompt that replaces omp's own; `None` keeps omp's default.
    pub const fn base_prompt(self) -> Option<&'static str> {
        match self {
            Self::Chat => Some(CHAT_PROMPT),
            Self::Code => None,
        }
    }
}

/// The current mode; Code until the webview says otherwise.
pub struct AppModeStore(Mutex<AppMode>);

impl AppModeStore {
    pub const fn new() -> Self {
        Self(Mutex::new(AppMode::Code))
    }

    pub fn get(&self) -> AppMode {
        *self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub fn set(&self, mode: AppMode) {
        *self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = mode;
    }
}

impl Default for AppModeStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Days since 1970-01-01 → (year, month, day), proleptic Gregorian (Hinnant).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

/// `YYYY-MM-DD` (UTC) for `secs` since the epoch.
pub fn date_utc(secs: u64) -> String {
    let (y, m, d) = civil_from_days(i64::try_from(secs / 86_400).unwrap_or(0));
    format!("{y:04}-{m:02}-{d:02}")
}

/// What omp's own prompt told the model about where it runs; the replaced
/// base prompt needs it said again.
pub fn environment(cwd: &str, now_secs: u64) -> String {
    let mut out = String::from("\n\nEnvironment:\n- Platform: macOS\n");
    if !cwd.is_empty() {
        out.push_str(&format!("- Working directory: {cwd}\n"));
    }
    out.push_str(&format!("- Today (UTC): {}", date_utc(now_secs)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_only_the_two_modes() {
        assert_eq!(AppMode::parse("chat"), Some(AppMode::Chat));
        assert_eq!(AppMode::parse("code"), Some(AppMode::Code));
        assert_eq!(AppMode::parse("Chat"), None);
        assert_eq!(AppMode::parse(""), None);
    }

    #[test]
    fn only_chat_replaces_omps_prompt() {
        assert!(AppMode::Chat.base_prompt().is_some_and(|p| p.contains("general-purpose")));
        assert_eq!(AppMode::Code.base_prompt(), None, "Code keeps omp's own prompt");
    }

    #[test]
    fn dates_are_civil_and_the_environment_names_the_folder() {
        assert_eq!(date_utc(0), "1970-01-01");
        assert_eq!(date_utc(1_793_448_000), "2026-10-31");
        assert_eq!(date_utc(951_782_400), "2000-02-29");
        let e = environment("/p", 0);
        assert!(e.contains("Working directory: /p") && e.contains("1970-01-01"));
        assert!(!environment("", 0).contains("Working directory"));
    }

    #[test]
    fn store_starts_in_code_and_keeps_the_last_mode() {
        let s = AppModeStore::new();
        assert_eq!(s.get(), AppMode::Code);
        s.set(AppMode::Chat);
        assert_eq!(s.get(), AppMode::Chat);
    }
}
