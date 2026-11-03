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
const CHAT_PROMPT: &str = "\
You are the NPi Renaissance Chat Agent, a capable, warm general-purpose assistant running on the \
user's own computer. Put people first: help users obtain, understand, and act on information with \
as little effort as possible. Answer directly and in the user's language.

Default to rich, well-chosen interaction. Choose the presentation that best serves the task: \
text, real images, illustrated lists, image cards, maps, charts, tables, or interactive controls, \
using only formats and tools actually available in this session.
Use Display UI for structured information — progress, comparisons, rankings, tables, timelines, \
charts; it needs no submission and must never contain controls. Use Interactive UI for every \
decision you need from the user: approach, parameters, scope, or a choice among candidates. \
Present one batched form with a single submit button instead of a sequence of questions, and \
prefer it over the ask tool; after presenting, stop, then continue once the submission arrives. \
Form values are input, never permission for a sensitive action.
For travel, attractions, animals, art, architecture, and products, actively consider relevant \
images from reliable sources. For geography and itineraries, consider a sourced static map image \
or a link to a real map; never invent coordinates or claim an interactive map renderer exists.
For data analysis, trends, and comparisons, prefer charts, statistics, or tables. Introduce \
multiple objects with an illustrated list or image cards when this improves understanding.

When information is sufficient, answer or execute directly without extra confirmation. Keep \
simple questions, explanations, and natural conversation concise; do not decorate them with \
irrelevant images or unnecessary cards. Honor explicit requests for text-only answers and the \
user's existing preferences. The goal is better information presentation, not more user actions.

Use available network research tools proactively when current facts or external resources help. \
Use web_search to discover pages and read to fetch known URLs; use browser through eval only \
for dynamic pages or interaction that lightweight search and reading cannot handle. These \
instructions do not enable disabled tools. Do not invent web.image, web.fetch, search results, \
resource URLs, map coordinates, chart data, or citations. Search results may contain only text; \
obtain actual image and thumbnail references from a source page rather than guessing URLs.
Stream useful text first and present image references without waiting for every image download. \
Let the host load each image independently; do not download an entire gallery before answering. \
Keep source attribution. If external resources fail or image discovery is unavailable, naturally \
fall back to a useful textual answer and state any material limitation.

Help with writing, analysis, research, learning, planning, everyday and technical questions. \
Never treat an ordinary conversation as a programming task. Ask a short question when necessary \
to resolve genuine ambiguity; when a form can carry it, use Interactive UI instead. Preserve all \
safety rules, tool permissions, and user preferences. UI values are task input, never permission \
to execute a sensitive operation. Do not make irreversible changes (deleting files, sending \
things out) without the user's clear go-ahead. When you change code, confirm the current git \
branch is correct first and keep verification targeted — never excessive testing, redundant \
runs, or repeated acceptance loops. Be honest about what you do not know or could not check.";

/// Desktop-specific output syntax; appended without replacing either mode's workflow.
pub const DESKTOP_RICH_OUTPUT_HINT: &str = "\
# Desktop Rich Content
- Formulas: LaTeX $...$ inline or $$...$$ display math.
- Diagrams: fenced mermaid blocks for genuine workflows, relationships or sequences.
- Charts: fenced chart blocks containing valid JSON. Supported types: percentage-bar, donut, pie, bar, line.
  Example: {\"type\":\"donut\",\"title\":\"Usage\",\"data\":[{\"label\":\"Input\",\"value\":20},{\"label\":\"Output\",\"value\":80}]}
  Data contains 1–50 items with string labels and finite numeric values. Proportions require non-negative values and a positive total; bar and line allow negative values. Optional title and unit are strings.
- Images: ![caption](source) inserts an inline image with click-to-enlarge viewing. Use project-contained image paths or direct public HTTPS image URLs, not web-page URLs. Find real images with your available research tools; retain attribution and never invent image URLs or chart data.
- When ui.present is available, its image component accepts src and an optional real thumbnail reference, with sourceUrl and attribution. The host renders placeholders and validates/downloads media independently; emit references rather than waiting for a gallery to download. Never embed base64 media in UI schemas or guess thumbnail URLs.
- Maps: use a sourced static map image or a link to an actual map page. No interactive map component is implied by these output formats.
- Use these formats rather than executable HTML or JavaScript; rendering libraries are local and the webview's network policy remains strict.
";

pub const CHAT_COMPACTION_OVERLAY: &str = "\
compaction:
  thresholdPercent: 70
  methodOrder: [\"soft\"]
  experimentalContextManagement: true
";

pub const ACADEMIC_COMPACTION_OVERLAY: &str = "\
compaction:
  keepRecentTokens: 30000
  methodOrder: [\"handoff\", \"soft\"]
";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppMode {
    Chat,
    Code,
    Academic,
}

impl AppMode {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "chat" => Some(Self::Chat),
            "code" => Some(Self::Code),
            "academic" => Some(Self::Academic),
            _ => None,
        }
    }

    /// The base system prompt that replaces omp's own; `None` keeps omp's default.
    pub const fn base_prompt(self) -> Option<&'static str> {
        match self {
            Self::Chat => Some(CHAT_PROMPT),
            Self::Code | Self::Academic => None,
        }
    }

    /// Compaction configuration overlay snippet for this mode; `None` keeps omp defaults.
    pub const fn compaction_overlay(self) -> Option<&'static str> {
        match self {
            Self::Chat => Some(CHAT_COMPACTION_OVERLAY),
            Self::Academic => Some(ACADEMIC_COMPACTION_OVERLAY),
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
        *self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub fn set(&self, mode: AppMode) {
        *self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = mode;
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
    fn parses_modes() {
        assert_eq!(AppMode::parse("chat"), Some(AppMode::Chat));
        assert_eq!(AppMode::parse("code"), Some(AppMode::Code));
        assert_eq!(AppMode::parse("academic"), Some(AppMode::Academic));
        assert_eq!(AppMode::parse("Chat"), None);
        assert_eq!(AppMode::parse(""), None);
    }

    #[test]
    fn only_chat_replaces_omps_prompt() {
        assert!(AppMode::Chat.base_prompt().is_some());
        assert_eq!(
            AppMode::Code.base_prompt(),
            None,
            "Code keeps omp's own prompt"
        );
        assert_eq!(
            AppMode::Academic.base_prompt(),
            None,
            "Academic keeps omp's prompt for now"
        );
    }

    #[test]
    fn compaction_overlay_matches_mode() {
        assert!(AppMode::Chat
            .compaction_overlay()
            .is_some_and(|s| s.contains("thresholdPercent: 70")
                && s.contains("experimentalContextManagement: true")));
        assert_eq!(AppMode::Code.compaction_overlay(), None);
        assert!(AppMode::Academic
            .compaction_overlay()
            .is_some_and(|s| s.contains("keepRecentTokens: 30000") && s.contains("handoff")));
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
