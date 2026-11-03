# Academic space

`~/Documents/NPi/Academic` holds one folder per research project. Code and Chat have their own folders beside it: `~/Documents/NPi/Code` (where "new project" opens the picker) and `~/Documents/NPi/Chat` (the folder every chat runs in, `chat_space.rs`). Nothing is migrated: older folders (`NPi/Chats`, projects opened from elsewhere) keep working where they are, Chat's old app-data folder stays a legacy chat folder.

## Tabs and sidebar

A project is an ordinary omp session whose cwd is a folder **directly under** the Academic folder. `app-live.jsx` sorts tabs into three spaces by path (`spaceOf`: chat, academic, code) and each mode's sidebar, start-page project list and recent rows show only its own space; a tab of another space never shows in this mode (the start page does, as with Chat). `academic_dir` (Rust) returns the folder; the webview only compares paths.

## Scaffolding (`academic_space.rs`)

`open_project(kind: "academic")` opens the native picker in the Academic folder (macOS "New Folder" names a new project) and `AcademicSpace::prepare` scaffolds the picked folder when it is a direct child of the space. Anything else (the space itself, a deeper folder, a folder elsewhere) is left alone. Scaffolding only adds what is missing and never overwrites a file the user edited.

```
<project>/
  papers/            originals, read only
  notes/             reading notes
  digests/           L1 section digests, L2 paper digests
  artifacts/         comparison tables, argument maps
  data/raw/          source data, read only
  data/processed/    derived data + the script/steps that made it
  drafts/            the user's own writing
  .npi-academic/project.json   { "schema": 1 }
  .omp/AGENTS.md     layout and workflow (omp reads the nearest non-empty .omp/)
  .omp/RULES.md      sticky hard rules (no invented citations, originals immutable, ...)
  .omp/skills/       literature-intake, paper-digest, data-processing
```

Templates live in `src-tauri/templates/academic/` and are embedded with `include_str!`. The rules are advice to the model, not enforcement: nothing stops an unrestricted tool from writing into `papers/`. Enforcement belongs to the host tools below.

## Academic Copilot (`copilot.rs`)

In Academic, beside an open project, the toolbar's side-pane button opens the **Academic Copilot** for that project (`SideChat` with `copilotProject`; elsewhere it stays the plain SideChat). The pane starts a worker through `academic_copilot_start(sessionId, project, profile)`; Rust checks that `project` is a direct child of the Academic folder (`AcademicSpace::project`, canonical) and binds the worker to it. Not yet offered in Chat/Code: that needs a project picker.

The pane (`design/side-chat.jsx`, styles `layout/sidechat.css`) is shared by SideChat and the Copilot: a one-line header (title, the project folder for the Copilot, a phase dot, clear and close), an empty state (the Copilot adds three starters that fill the box but never send), messages in the chat's tokens, and one auto-growing composer whose footer carries the model picker on the left (the shared `ModelMenu`, opening upward, favorites and "Manage models…" like the Composer's) and the phase and send/stop button on the right. Esc stops a running reply, otherwise closes the pane. The picked model is the pane's own and lasts as long as the pane; until one is picked the Settings → Chat → SideChat model is used (`OMP_SIDECHAT.wantedModel`). The controller applies it with the next message (`set_model`, shown as "Switching model…"), never mid-reply. The catalog comes from `bridge.modelsCatalog`, so no session is needed to list models.

**Height chain.** The pane is a grid item of `.stage`, whose single row is `auto`. Without `min-height: 0` (and `contain: size`) the item's automatic minimum is its content, so every message grew the row, the stage and the hidden `.window`'s scroll height: the message list never got a bounded height (no inner scrollbar) and the caret or a focus could scroll the hidden window, shifting the whole app. The pane now stays at the row's height; `.sc-body` is the one scroll area (`overflow-y: auto`, `overscroll-behavior: contain`, `overflow-anchor: none`, `scrollbar-gutter: stable`, thin styled scrollbar). Following the stream is coalesced to one scroll per animation frame and only while the reader is at the bottom (only an upward move lets go of the bottom, so a delta landing before the scroll event does not).

**Thinking level.** The model menu's Effort row works in the pane: `controller.thinkingLevels()` asks omp (`get_available_thinking_levels`), `setThinkingLevel(level)` validates against that list (never a level omp did not list) and sends `set_thinking_level` when the worker is idle, else with the next message. The level shown follows `thinking_level_changed` and `get_state`; it reads `off` when unset and is empty (no Effort row, no level on the button) for a model with no thinking. A model switch refreshes the list and re-applies the pane's pick where the new model still lists it; the pick lasts for the pane's lifetime.

The worker is a SideChat-style omp (`--no-tools`, no extensions, skills, rules or context files, cwd in the app cache, no persistence), so nothing in the project can add a capability. Its system prompt is `COPILOT_PROMPT` plus the project's `.omp/RULES.md` (default rules when absent), read by Rust.

Its only tools come over omp's Host Tool RPC, registered by Rust (`AgentBridge::start_session_with_host` writes `set_host_tools` first thing; the webview's command whitelist does not include it) and answered by the stdout reader: a `host_tool_call` frame is executed by `CopilotHost::call` and replied to with `host_tool_result` on stdin, never emitted to the page. Names use underscores because providers restrict tool names.

|Tool|What|Limits|
|---|---|---|
|`research_search`|passages (blank-line blocks) containing any query term, ranked by how many; `file:line: snippet`|text files under `papers/ notes/ digests/ artifacts/ data/ drafts/`, 2 MB each, 2000 files, 20 hits; symlinks skipped|
|`research_read`|a text file from a line, or a folder listing|same folders; project-relative only, no `..`, symlinks may not leave the project; 24k chars per call|
|`library_search` / `library_get` / `library_check`|the project's reference library (see `librarian.md`)|read only; 20 hits; a draft by path comes from the read folders, 2 MB|
|`library_suggest`|a tag, group or note for a reference, as a pending proposal|changes nothing until the user accepts it|
|`research_save`|`note`/`digest`/`artifact`/`data` into `notes/ digests/ artifacts/ data/processed/`|plain file name, text extension, 256 KB, no overwrite unless `overwrite: true`, never through a symlink|

Everything else (`papers/` and `data/raw/` writes, `.omp/`, `.npi-academic/`, paths outside the project, the network, a shell) has no tool at all.

PDFs are searched and read as extracted text (`pdf_text.rs`, below); known gaps: a scanned PDF has no text layer and OCR is not available; search is a plain term match, not FTS5 (planned in `docs/PLAN_3.0.md`); `research_propose_ops`, authorisation per paper and calling the Copilot from the main chat are not built. The tools were exercised only by unit tests: a real omp run needs model credentials, which the self-hosting rules keep out of probes, so the `set_host_tools` handshake and the model's use of the tools are unverified end to end.

## The Academic library as tools for the main chat (`copilot::ResearchHub`)

Every normal tab (Chat, Code, Academic) also starts with three read-only host tools over the same Host Tool RPC: `academic_projects` (names of the projects directly under `NPi/Academic`), `academic_search(project, query)` and `academic_read(project, path)`. They run the Copilot's own search/read with its limits (read folders only, no `..`, no symlink escape, text files, no PDF text) on a project named by a plain folder name (`AcademicSpace::project_named`, never a path); nothing can be written from here (`research_save` stays Copilot-only). The agent is expected to use them instead of memory for questions about the user's papers and notes. `start_session` passes the hub to `AgentBridge::start_session_with_host`; the reader's `host` is a `dyn HostTools`, so the Copilot worker (`CopilotHost`) and the main tabs share one path. Like the Copilot's tools, the `set_host_tools` handshake and a model actually calling them are unit-tested on the Rust side only; a live omp run is unverified.

## PDF text

`pdf_text.rs` turns a PDF into Markdown with `pdf-inspector` (pure Rust, MIT, on `lopdf`). Checked on generated PDFs only (English, a table inside two columns, two columns alone, Chinese, an image-only page): CJK text is read correctly (`lopdf`'s own `extract_text` garbles it), two plain columns come out in reading order, an image-only PDF is reported as scanned. Weak spot: a table beside a column can make it merge the two columns' lines. Not tried on real LaTeX or journal PDFs, nor timed in a release build.

- A PDF over 64 MiB is refused; parsing runs behind `catch_unwind`, so a malformed file is an error, never a panic in the caller's thread.
- The text is cached in `<project>/.npi-academic/cache/pdf/<hash of path>.<size>-<mtime>.md` (an empty file means scanned); a changed file misses and replaces its entry. The cache is rebuildable and holds nothing the user wrote.
- `research_search` extracts at most 4 uncached PDFs per call and says how many are waiting; `research_read` extracts the one it is asked for. A scanned PDF is named in the search result and refused by read.
- `PyMuPDF` / MuPDF were left out: AGPL.
