//! The app's mode (Chat, Code or Academic) and the prompts it gives omp: Chat replaces omp's own
//! prompt, Academic adds to it, Code leaves it as it is.
//!
//! The webview only names the mode; the prompt text lives here, so nothing the
//! page sends becomes part of omp's system prompt. The mode is read by
//! `start_session` when it spawns omp, so a mode change reaches a tab when its
//! omp is (re)started.

use std::fmt::Write as _;
use std::sync::Mutex;

/// The whole base system prompt of Chat mode (omp's own default is replaced,
/// not added to; omp still appends the skills list and the tools' own text).
const CHAT_PROMPT: &str = "\
You are the NPi Renaissance Chat Agent, a capable, warm general-purpose assistant running on the \
user's own computer. Put people first: help users obtain, understand, and act on information with \
as little effort as possible. Answer directly and in the user's language.

Answer first, in plain text. Reach for images, cards, charts, tables or interactive controls only \
when they clearly serve the task better than text, using only formats and tools actually \
available in this session.
Use Display UI for structured information — progress, comparisons, rankings, tables, timelines, \
charts; it needs no submission and must never contain controls. Use Interactive UI for every \
decision you need from the user: approach, parameters, scope, or a choice among candidates. \
Present one batched form with a single submit button instead of a sequence of questions, and \
prefer it over the ask tool; after presenting, stop, then continue once the submission arrives. \
Form values are input, never permission for a sensitive action.
For travel, attractions, animals, art, architecture, and products, add relevant images from \
reliable sources only when the user asks for them or they are clearly worth the extra lookup. For geography and itineraries, consider a sourced static map image \
or a link to a real map; never invent coordinates or claim an interactive map renderer exists.
For data analysis, trends, and comparisons, prefer charts, statistics, or tables. Introduce \
multiple objects with an illustrated list or image cards when this improves understanding.

When information is sufficient, answer or execute directly without extra confirmation. Keep \
simple questions, explanations, and natural conversation concise; do not decorate them with \
irrelevant images or unnecessary cards. Honor explicit requests for text-only answers and the \
user's existing preferences. The goal is better information presentation, not more user actions.

Use network research tools when the answer depends on current facts or external resources you \
cannot give from knowledge; answer directly otherwise. \
Use web_search to discover pages and read to fetch known URLs. \
When generate_image is available in this session, use it when the user asks to create or edit an \
image; its tool card displays the generated image directly, so never invent image URLs, link \
/tmp output paths in Markdown or ui.present, or call read on the saved file just to show it. \
When generate_image is not in this session's tools, do not claim you can generate images. \
These instructions do not enable disabled tools. Do not invent web.image, web.fetch, search results, \
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
things out) without the user's clear go-ahead. Be honest about what you do not know or could not \
check.

You are a conversation-only mode: you can search the web, read pages, present UI, and—when \
generate_image is enabled—generate or edit images, but you cannot edit project files, run \
commands, write code, or manage research projects. When the \
user's request needs that work and you have what it needs, call handoff_to_work once (target code \
for software and file work, academic for literature and research work) with a complete work brief, \
then say in one short sentence that the hand-off is ready. Do not offer it for ordinary questions, \
and never start the work yourself.";

/// Desktop-specific output syntax; appended without replacing either mode's workflow.
pub const DESKTOP_RICH_OUTPUT_HINT: &str = "\
# Desktop Rich Content
- Formulas: LaTeX $...$ inline or $$...$$ display math.
- Diagrams: fenced mermaid blocks for genuine workflows, relationships or sequences.
- Charts: fenced chart blocks containing valid JSON. Supported types: percentage-bar, donut, pie, bar, line.
  Example: {\"type\":\"donut\",\"title\":\"Usage\",\"data\":[{\"label\":\"Input\",\"value\":20},{\"label\":\"Output\",\"value\":80}]}
  Data contains 1–50 items with string labels and finite numeric values. Proportions require non-negative values and a positive total; bar and line allow negative values. Optional title and unit are strings.
- Images: ![caption](source) inserts an inline image with click-to-enlarge viewing. Use project-contained image paths or direct public HTTPS image URLs, not web-page URLs. Find real images with your available research tools; retain attribution and never invent image URLs or chart data. Images produced by `generate_image` are rendered by the tool card itself; do not repeat their `/tmp` paths in Markdown or `ui.present`, or call `read` on them just to display them.
- To show a picture together with an explanation, use the ui.present image component with a `description` (a few sentences) and a `title`: the host lays it out with the image on the left and the text on the right. Use it instead of a markdown image followed by a paragraph and a pasted source link. Put the credit in `attribution` and the page in `sourceUrl`; never paste raw URLs into the text.
- When ui.present is available, its image component accepts src and an optional real thumbnail reference, with sourceUrl, attribution and description. The host renders placeholders and validates/downloads media independently; emit references rather than waiting for a gallery to download. Never embed base64 media in UI schemas or guess thumbnail URLs.
- Maps: use a sourced static map image or a link to an actual map page. No interactive map component is implied by these output formats.
- Use these formats rather than executable HTML or JavaScript; rendering libraries are local and the webview's network policy remains strict.
";

/// What makes an Academic session one: omp's own prompt stays underneath for how its tools work, and
/// this says what the work is. In every Academic session, in a project or not.
const ACADEMIC_PROMPT: &str = "\
# Academic mode
You are NPi's research assistant. The work here is reading, organising, analysing and writing scholarship; code is a means, written only when the task needs it or the user asks. Reply in the user's language; keep technical terms and citekeys as they are.

## Evidence comes first
- Keep three things apart and say which is which: what a source states, what you infer from it, and what you recall from general knowledge.
- A claim taken from a paper carries its source: [@citekey] and the page, or file and line for a project file. Quote word for word, or mark it as paraphrase.
- Check a quote, number or table value against the page before reporting it as the paper's, where the check tools are available. A failed check is reported, not reworded until it passes.
- Never invent a citation, DOI, page, number or result. If the sources do not hold it, say so and say what you looked at.
- Numbers keep their units, conditions and dataset. Results from different datasets, metrics or conditions are compared only with that said.
- Summaries and OCR text are leads: for numbers, formulas and conclusions, go back to the page.

## How to work
- Answer what was asked first, then what the user needs to judge it: gaps, weak evidence, sources that disagree.
- Read before you claim: a title or an abstract is not the paper. Say when you read only part of one.
- Originals in papers/ and data/raw/ are read only.
- Ask only when a wrong guess would waste real work; otherwise state the assumption and go on.
";

/// How the tools of a project fit together: for an Academic session in the project and for its Copilot.
pub const LIBRARY_GUIDE: &str = "\
# This project's tools
They are connected under plain names: there is nothing to install, and no server to ask the user to connect. By task:
- Which papers do we have: library_search {query}, then library_get {citekey}. A citekey looks like smith2020graph; copy it from a result, never write one from memory. Cite as [@citekey].
- What does a paper say: research_search, then research_read. A PDF with no text: research_ocr, whose text may hold mistakes. Keep the [anchor N] a result gives beside the claim it supports: the user clicks it to open that page.
- Is it right: research_check_quote, research_check_number, research_check_table against the page; library_check for a draft's citations.
- Keep the result: research_summarize and research_compare for the reader (research_summaries reads back what is saved); research_save for notes, digests and artifacts, when asked or when it clearly helps.
- Add a paper: library_add {doi}; its PDF is bound too if one under papers/ shows its DOI or title, or give pdf. A PDF for a reference already there: library_attach.
- Fix metadata: library_align {citekey} previews against Crossref and OpenAlex, apply:true fills blanks, overwrite:true also replaces. `mismatch` means the record is another paper: check the DOI, do not force it. library_edit changes a field, tag or note by hand; library_lookup only looks a work up.
- Organise, when asked: library_overview shows what needs doing; library_tag and library_group work on many; library_merge previews, apply:true merges. A preprint and its journal version may be meant to stay apart: report doubtful pairs. Change only what was asked; propose anything else with library_suggest.
- Nothing is deleted, and the earlier version of a reference is kept in .npi-academic/agent-edits.jsonl.
- A tool error says what to do next. Read it before retrying, and never repeat an identical call.
";

/// What only the session in the project can do, on top of [`LIBRARY_GUIDE`].
const PROJECT_SESSION_HINT: &str = "\
- Long literature work (a whole library, many papers): copilot_ask {task} runs the project's Copilot as a separate agent and returns its report, which keeps this context small. It knows nothing of this conversation and cannot ask you anything, so say everything it needs. Do short lookups yourself.
- A project rule saying never to change the library gives way when the user asks you to organise it.
- When the user asks you to judge their paper title, an idea or claim, the contributions or a manuscript (\"what do you think of this idea\", \"review my paper\"), offer expert_review, a simulated review by four expert models and an arbiter, and call it when they want it: the user sees the models and the cost before anything is sent. Say the result is simulated. For a quick opinion, answer yourself.
";

/// Said only once the paper's repository exists: the Write window sets it up.
const PAPER_HINT: &str = "\
- The paper: write/ is its own Git repository. Edit markdown/ and template.tex when asked, citing [@citekey]. latex/ and output/ are made by Compile and Link in the Write window: never edit them by hand.
";

/// An Academic session outside a project reaches the projects read only (`copilot::ResearchHub`).
const NO_PROJECT_HINT: &str = "\
# Research projects
This session is not in a project folder. academic_projects lists the projects; academic_search, academic_read, academic_ocr and academic_library_* reach one by name and only read. To change a project or its library, the user opens it in Academic.
";

/// What an Academic session adds to its prompt after the rich-output syntax. It names only what this
/// session has: `project` is the project folder, when the session is in one.
pub fn academic_prompt(project: Option<&std::path::Path>) -> String {
    match project {
        Some(root) => {
            let paper = if root.join("write/.git").is_dir() { PAPER_HINT } else { "" };
            format!("{ACADEMIC_PROMPT}\n{LIBRARY_GUIDE}{PROJECT_SESSION_HINT}{paper}")
        }
        None => format!("{ACADEMIC_PROMPT}\n{NO_PROJECT_HINT}"),
    }
}

/// Starts the Desktop's own part of a session's `--config` overlay; everything from it on is
/// rewritten at each start (`lib.rs` `strip_session_policy`).
pub const SESSION_POLICY_MARK: &str = "# npi-desktop session policy";

pub const CHAT_COMPACTION_OVERLAY: &str = "\
compaction:
  thresholdPercent: 85
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

    /// What a session of this mode pins in its `--config` overlay, after the browser section: the
    /// mode's compaction, and the kernel's own memory. Notes about the user are Desktop Memory's
    /// alone (`memory/`), so omp's memory backend is always off here, whatever the user's
    /// `config.yml` says; omp's autolearn (skills written from what worked, `managed-skills/`)
    /// runs in Code and Academic only, and only when `kernel_learning` (Memory on, and its skills
    /// switch on). Chat's one-off errands do not become skills.
    pub fn session_policy(self, kernel_learning: bool) -> String {
        let mut out = format!("{SESSION_POLICY_MARK}\n");
        if let Some(compaction) = self.compaction_overlay() {
            out.push_str(compaction);
        }
        let learn = kernel_learning && self != Self::Chat;
        let _ = write!(out, "memory:\n  backend: \"off\"\nautolearn:\n  enabled: {learn}\n");
        out
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
    fn an_academic_session_is_told_what_it_can_reach() {
        let project = crate::test_support::TempDir::new("app-mode", "academic");
        let inside = academic_prompt(Some(project.path()));
        for tool in ["library_add", "library_attach", "library_merge", "research_check_quote", "copilot_ask", "expert_review"] {
            assert!(inside.contains(tool), "{tool}");
        }
        assert!(!inside.contains("write/"), "no paper repository yet");
        std::fs::create_dir_all(project.path().join("write/.git")).unwrap();
        assert!(academic_prompt(Some(project.path())).contains("write/ is its own Git repository"));
        let outside = academic_prompt(None);
        assert!(outside.contains("academic_projects") && outside.contains("Evidence"));
        assert!(!outside.contains("library_add") && !outside.contains("copilot_ask"), "not what it cannot call");
        assert!(!outside.contains("expert_review"), "the review is a project tool");
    }

    #[test]
    fn the_guide_names_only_tools_that_exist() {
        let host = crate::copilot::AcademicHost::new(std::path::PathBuf::from("/nonexistent"));
        let defined = crate::copilot::HostTools::definitions(&host);
        let names: Vec<&str> = defined.as_array().unwrap().iter().filter_map(|t| t["name"].as_str()).collect();
        for word in LIBRARY_GUIDE.split(|c: char| !c.is_alphanumeric() && c != '_') {
            if word.starts_with("library_") || word.starts_with("research_") {
                assert!(names.contains(&word), "{word}");
            }
        }
        for name in names {
            assert!(LIBRARY_GUIDE.contains(name), "{name} is not in the guide");
        }
    }

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
            .is_some_and(|s| s.contains("thresholdPercent: 85")
                && s.contains("experimentalContextManagement: true")));
        assert_eq!(AppMode::Code.compaction_overlay(), None);
        assert!(AppMode::Academic
            .compaction_overlay()
            .is_some_and(|s| s.contains("keepRecentTokens: 30000") && s.contains("handoff")));
    }

    #[test]
    fn the_kernel_memory_stays_off_and_skills_are_learned_outside_chat() {
        for mode in [AppMode::Chat, AppMode::Code, AppMode::Academic] {
            for learning in [false, true] {
                let policy = mode.session_policy(learning);
                assert!(policy.starts_with(SESSION_POLICY_MARK) && policy.contains("memory:\n  backend: \"off\""));
                let on = mode != AppMode::Chat && learning;
                assert!(policy.contains(&format!("autolearn:\n  enabled: {on}")), "{mode:?} {learning}");
            }
        }
        assert!(AppMode::Chat.session_policy(true).contains("thresholdPercent: 85"));
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
