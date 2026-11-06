# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- `[anchor N]` in an answer of an Academic project's tab (Academic session or Copilot) is a link: a click opens the reader on that page with the passage marked. The project's tool guide asks the model to keep the anchors the research tools give beside the claims they support.
- Librarian: "Read page 1 with the model…" for a reference whose PDF shows no DOI. After a confirmation saying what is sent, page 1's text goes to the profile's model (no tools); what it reads shows like an online lookup, blank fields ticked, and only Save writes it.
- History, approvals, Todo and plan, and the git browser open as their own windows, like the Librarian. They show the main window's current tab and act on it there (`app/panel-link.js`); Settings stays inside the main window.

- Model roster in Settings → Models: flagship, high, standard, economy and arbiter roles; unset fast, subtask and thinking roles follow their corresponding tiers. Auto subagent batches naming one configured tier can dispatch directly.
- Academic `expert_review`: parallel model-roster experts and an anonymous arbiter for titles, ideas, contributions and manuscripts. The App requires a confirmation card showing models, providers and estimated tokens; MCP callers must obtain consent before declaring confirmation. Simulated reports are saved in the project's research database.
- The assistant can look back at past conversations (`search_conversations`): every chat in Chat, the project's conversations in Code and Academic, or everything when asked. Only what was said is searched, never tool output, Chinese included.
- A profile of how you like to work in each mode, kept by the assistant: `~/.local/share/npi/USER-CHAT.md`, `USER-CODE.md`, `USER-ACADEMIC.md`, shown and editable in Settings → Memory.
- Memory shows how full it is, in the prompt and in Settings: notes of the limit, characters of the prompt, and what was left out.
- In Code and Academic the assistant may save the steps of a long task as a skill; each new one is shown with Undo, and Settings → Skills lists them with Delete (moved aside, not erased).
- Alibaba's international pay-as-you-go provider is listed as `Alibaba (International)`, separate from `Alibaba(China)`.
- Quick ask (Option+Space) is now a floating panel that answers in place: it takes the keyboard without bringing the app forward, keeps its conversation, shows messages, tool calls and approvals, and switches mode, model and thinking from a capsule along its bottom edge. "Continue in window" opens the same conversation in the main window.
- Worktree tasks in Code: a project's branch button opens a tab on its own worktree and branch, so tasks do not touch each other's files. The git browser shows how far it is from the main checkout's branch and merges it, asks the Agent for a PR, or removes it.
- Review in the git browser: comment on a line and send the comments to the Agent together; undo one change of a modified file and keep the rest.
- Rewind takes files back too: the kernel snapshots a Code project's files before each prompt, and Rewind offers to restore them with the conversation.
- Settings → Usage: what is left of each subscription as horizontal bars with their reset times, then the usage statistics, which no longer need their own window.
- Memory: in Chat, Code and Academic the model keeps short notes about you (field, work, interests, projects, people) and reads them when a conversation starts. Stored in SQLite at `~/.local/share/npi/memory.db`; on by default, with every note editable in Settings → Memory. Chat's old `MEMORY.md` is taken in once.
- Agents can bind PDFs: `library_attach` (a path, a look under `papers/`, or every reference at once), and `library_add` binds a reference's PDF by itself.
- Write: a paper's own Git repository in `write/` and a window that manages it. Markdown drafts, Compile (an agent writes the LaTeX of each changed file; `references.bib` comes from the library), Link (one complete `main.tex`), a pipeline progress bar, and opening in VS Code or Typora.
- "Open in Reader" on a reference in the literature column.
- The Academic Copilot can organise the library when asked: `library_overview`, `library_group`, `library_tag`, `library_merge`, and the lookup, align, add and edit tools.
- `copilot_ask`: an Academic session hands long literature work to the Copilot and gets its report; also an MCP server (`--npi-mcp-copilot`, registered as `npi-copilot` by "Connect agents").
- Librarian: drag a reference (or the selection) onto a group in the sidebar to add it.
- Librarian: select several references (Cmd-click, Shift-click, arrows, Cmd+A) to group, tag, align, trash or merge them; built-in filters for missing metadata, no PDF, possible duplicates and waiting suggestions; an editable citation key; "Align metadata…" with a preview before anything changes; "Read" on an attachment.
- Librarian import takes pasted text: DOIs and arXiv ids are looked up, and BibTeX, RIS, CSL-JSON, PubMed `.nbib`, Web of Science and EndNote XML are recognised by their content, in files too.
- Librarian: OpenAlex answers when Crossref has no record, several PDFs can be picked or dropped at once, "Locate…" finds a missing attachment, `[@key]` can be copied, DOI and URL fields open in the browser, Cmd+F searches and Cmd+S saves.
- Librarian "Copy as…": copy the selected reference or the whole list in the form a journal or a regulation asks for, in one click. Type a journal, publisher or rule (Phys Rev Lett, Elsevier, GB/T 7714, ICMJE…) and a click on the result sets its citation style and copies; 80 presets (`src/app/librarian-journals.js`) say which style each is named after and mark the closest bundled style where the journal's own is not included (Science, PNAS, Optica, Wiley and OUP say how to add their .csl). What to copy: reference list, in-text citation, BibTeX, RIS or CSL-JSON text. The style list now offers the whole bundled CSL archive (about 90 styles: publishers, journal families, societies, regional standards), grouped; recent styles are one click away.
- Academic sessions in an Academic project now have the project's library tools built in (host tools: no MCP to connect, no `project` argument, no approval prompt): the Copilot's tools plus `library_lookup`, `library_align`, `library_add` and `library_edit`. A prompt hint and the `librarian` skill give the calling order with examples, and tool errors say what to do next (a mistyped citekey gets the nearest ones). No tool deletes; every agent change to a reference is first kept in `.npi-academic/agent-edits.jsonl`. The MCP server offers the same four tools. See `docs/architecture/librarian.md`.
- Librarian MCP server: `omp-desktop --npi-mcp-librarian <project folder>`, registered in `<project>/.mcp.json` by the Librarian window's "Connect agents" button, so `npi` in a terminal (or any MCP client) can search and read the project's library. Two tools reach the network and exist only here: `library_lookup` (Crossref by DOI or title) and `library_align` (compare a reference with Crossref; preview by default, `apply` fills blank fields, `overwrite` replaces differing ones; a DOI whose record has another title is never applied). See `docs/architecture/librarian.md`.
- The Academic reader shows the paper's own PDF pages (PDF.js, Apache-2.0, vendored in `src/vendor/pdfjs/`; selectable text, zoom, page box). A search hit or a citation's anchor scrolls to its page and marks the passage; the extracted-text view stays as a second tab. The PDF is read through `research_pdf` (the webview names the paper by id).
- `research_ocr` (the Copilot) and `academic_ocr` (the main Academic tab): read up to 6 pages of a scanned PDF with the system's OCR and return the text, marked as a recognition. Host tool calls now run on their own thread, so a slow tool does not hold up the tab's events.
- Academic top bar: Librarian, Reader and a literature-column toggle are buttons; the literature column at the far right lists the project's references (search, group filter, "cite in prompt", open in the Librarian) and is shown by default.
- Academic reader: its own window (Academic toolbar → Reader) with the project's indexed papers and sections, one page of a paper at a time, full-text search, and a jump from a citation's anchor to the page. "Index project" builds the index from the window.
- Summaries and comparison tables in the reader: the Copilot saves a paper's summary (L2) and its sections' summaries (L1) and comparison tables (`research_summarize`, `research_summaries`, `research_compare`). Each fact or cell with a quote is checked against the page and marked as matching the original, not found, or the model's reading; "not reported" and "not comparable" keep no value; a changed file marks a summary stale. New `research_check_number` and `research_check_table` tools check numbers and table cells against a page. Library schema is now v6 (a backup of an older file is kept).
- OCR for scanned PDFs on macOS (Vision, on the machine): pages without a text layer are recognised, indexed and marked "OCR".
- The Librarian window ranks its search with the project's FTS5 index (short Chinese words work) when one exists.
- `docs/PLAN_ACADEMIC_NEXT.md`: the five items above, the smaller gaps, what waits for 3.1 and what is unverified.
- Chat is conversation-only: it keeps web search, reading, the generative UI, a `remember` tool for its memory file and a `handoff_to_work` tool; the coding tools, MCP tools and library lookups are no longer sent with each request. When the model judges a request needs real work it offers a card above the composer; the button asks for a folder, opens a Code or Academic tab on it and puts the model's work brief in the prompt box, unsent. Chat's prompt now answers first in text and uses web, images and forms only when they help, and compaction starts at 85% instead of 70%.
- Librarian and usage statistics open as their own resizable windows (native traffic lights, Cmd+W closes them) instead of floating inside the main window. Cmd+W in the main window now hides it to the menu-bar icon; closing a tab stays on Ctrl+W.
- Quick ask: Option+Space opens a small prompt box over whatever you are doing; the prompt goes to a new chat or to the tab on screen. The app also has a menu-bar icon, and on macOS Cmd+W and the red button now close the window to it (the Dock icon goes while the window is hidden; click the menu-bar icon to bring it back).
- Notifications on macOS: the Settings row shows what the system said when it refused, and explains that an ad-hoc signed build is treated as a new app by every build. `scripts/macos-local-signing.sh` and `src-tauri/tauri.localsign.conf.json` sign local builds with one stable certificate so the permission is kept.
- Research Core: a per-project index of `papers/` PDFs (and notes, digests, drafts) in `research.db` (schema v5): blocks with page-level anchors, scanned PDFs recorded and never guessed, a changed file marks its old anchors stale, and FTS5 search that finds short Chinese words such as 文献 as well as English. The Copilot's `research_search` and `library_search` use it once a project is indexed (hits show `file p.N` and an anchor id), and a new `research_check_quote` tool verifies that a quote is on a page of a paper. `research_index` / `research_index_status` commands (no UI yet). See `docs/architecture/research-core.md`.
- Librarian trash: removed references are kept and listed under "Trash", and "Restore" brings one back with its groups, tags, notes and files.
- Librarian for the Academic Copilot: it can search the reference library, read a reference with its formatted citation, check a draft's citations against the library (matched, other year, not in the library) and suggest tags, groups or notes that you accept or dismiss on the reference. "Check a draft…" runs the same check from the Librarian window; the main chat gets the read-only tools. New projects also get a Librarian skill.
- Librarian citation styles: APA, IEEE, Chicago, MLA, Nature, Vancouver, Harvard and GB/T 7714 (numeric, author-date, note) through a CSL engine, your own `.csl` files, "Copy citation" / "Copy reference" / "Copy list" in the chosen style, and "Cite a draft…" which turns `[@citekey]` in a Markdown draft into citations plus a reference list.
- Librarian PDFs: "Add PDF…" reads page 1 into a draft (DOI, title, authors, year; the guessed fields are marked) that is added only when you confirm, then files the PDF under `papers/`. PDFs can be attached to any reference and opened from it, and "Look up online…" compares a record with Crossref and takes only the fields you tick (it runs only when you click and sends only a DOI or title).
### Changed

- One memory: the kernel's own memory pipeline and its per-folder notes no longer run in the app's tabs whatever the CLI's config says; in Code and Academic it may still write skills from what worked (Settings → Memory → Write skills from what worked), and turning Memory off stops that too. Each note the assistant keeps about you shows for a moment at the bottom of the window, with Undo. Memory now fits as many Chinese notes in a prompt as English ones, and remembers which conversation wrote each note.
- Files moved beside the command line's: settings to `~/.local/share/npi/desktop`, the Chat profile and instructions to `~/.local/share/npi`, caches to `~/.cache/npi/desktop`, worktree tasks to `~/Documents/NPi/Code/.worktrees`. Interface settings are kept in `~/.local/share/npi/desktop/ui-state.json` too. The old folders are moved over on the first start.
- New Chat conversations start with thinking off; it can still be raised per conversation.
- Academic sessions have a prompt of their own (research work and evidence rules) instead of only a coding prompt, and one guide to the project's tools. The Copilot's prompt no longer leaves out its tools or denies the network it has, and tool descriptions are shorter.
- The Copilot can read `write/`.

### Fixed

- Settings → Skills listed none of the skills the assistant had written: it looked in omp's folder instead of NPi's.
- The model is no longer told to read a memory summary that does not exist, to work in a paper repository that has not been set up, or to use memory tools it does not have; files that moved are marked missing in library tool answers; `.mcp.json` follows the app when it moves.
- Librarian: the group checkboxes no longer stretch and scatter their labels, and a long style name no longer pushes "Copy as…" out of its popover.
- Host tools no longer stop for approval: omp asked about them in `write` mode, though the Copilot's pane has no prompt to answer.
- Librarian: edits in the form are saved, not lost, when the selection moves, the window closes, or a file, group or suggestion changes the reference. Esc no longer closes the window, "Copy" works, abandoned "New reference" forms leave no empty row, a new reference that repeats one is refused, and publisher markup no longer shows in titles.
- A PDF added to the Librarian is indexed by itself, and the Reader indexes a file it has not reached instead of saying to.
- Librarian "Copy" (the buttons at the bottom of a reference and "Copy as…") failed with "could not copy": WebKit refuses `navigator.clipboard` once the click's gesture has been spent on an `await`. Rust now writes the clipboard (`clipboard_write`, `NSPasteboard`; text and HTML for a paste into Word).
- The Librarian's head wrapped its buttons onto two lines because the title took half of the width; the buttons stay on one line and the style name beside "Copy as…" is dropped in a narrow window.
- Images the model finds no longer fail on a host's HTTP 429: the fetch honours `Retry-After` (up to 4 s), retries a 429/503 twice with a growing pause, and starts requests to one host 250 ms apart.
- The quick-ask "New Chat / Current chat" menu was cut off by the window edge; it opens below the bar and the window grows for it.
- The quick-ask bar draws its own glass (frosted tint, specular rims, inner glow, a light flowing around the edge) instead of the system's backdrop.
- After Rewind (or Fork) the sidebar listed the conversation twice. A Rewind to the first prompt moves the tab onto an empty session omp writes only at the next message, so the saved list could not tie the tab to its old conversation. The tab now remembers the file it came from and the sidebar hides that conversation; the Chat list also refreshes when a chat moves onto another file.
- Context compaction rows: "nothing to compact" and a cancelled compaction no longer show as a red "compaction failed", omp's reason is shown in the row, a skipped automatic pass leaves no row, and a row no longer stays "running" after its process died. The row is translated.

### Added

- Academic: PDFs in `papers/` are now searched and read as extracted text (Chinese included) by the Copilot and the main chat's `academic_*` tools; text is cached under `.npi-academic/cache/pdf/`, and a scanned PDF is reported as having no text.
- Librarian: Academic's ⋯ menu opens a reference library window for the project: groups and tags, search, sort, an edit form with notes, import of `.ris` / `.bib` / `.json` and export to BibTeX, RIS or CSL-JSON through native file dialogs. Behind it is a per-project `research.db` with duplicate detection (DOI, or title + year + first author) and citation keys behind `library_*` commands.
- The main chat's agent can search and read the Academic library: three read-only host tools (`academic_projects`, `academic_search`, `academic_read`) are registered in every tab, scoped to projects under `NPi/Academic` and never able to write.
- Interface language: Settings → General → Language (Auto, 简体中文, 繁體中文, English (US), 日本語). Covers the Settings page (titles, descriptions, search), the sidebar and mode switch, window tooltips, the start page and its greetings, the composer basics, SideChat / Academic Copilot, the thinking-and-tools box and the subagent setup form; it never changes the model's reply language. Other text stays English for now (`docs/architecture/i18n.md`).
- Academic Copilot: in Academic, the side-pane button opens a per-project assistant that runs without omp's own tools and only has three Rust-executed host tools (`research_search`, `research_read`, `research_save`) over the project's local files; originals and raw data are read-only, nothing reaches the network or a shell. PDFs are not readable yet and search is a plain term match (`docs/architecture/academic-space.md`).
- Academic mode: a third segment (mortarboard icon) beside Chat and Code in the top-left mode switch, with scholarly start-page greetings. It shares Code's project tabs and sidebar for now and starts omp with the Academic mode settings (`app_mode.rs`).
- Composer attachment menu offers images and folder addresses. Folder picks append a quoted path reference to the originating tab's draft without switching projects or reading directory contents.
- Pending composer images open the shared lightbox with keyboard navigation, Escape dismissal and focus restoration; removing an image remains a separate action.

### Changed

- The `ui.present` image component takes an optional `description`: with it the card is laid out with the picture on the left and the title, explanation and credit on the right (stacked in a narrow column). Chat's output hints tell the model to use it instead of a markdown image followed by a paragraph and a pasted source link. Needs the kernel built from this tree.
- Settings: the Advanced destination is gone. Its rows moved to where they belong: the side column, "New tabs show", project sidebar and column widths into Code, the keyboard shortcuts into General.
- Settings → Models: the picker's models are a tidy grid per vendor (a checkbox per model, a count, All / None per vendor, a filter, Clear all); models that share a name show their id.
- Secondary windows (Settings, Librarian, Git, file preview, Approvals, Usage, Plan) have traffic lights at the top left and drag by their head: red closes, yellow folds the window down to its head, green fills the screen (a double click on the head does the same). They replace the × buttons.
- macOS: the red button and Cmd+W hide the main window and leave the app, its agents and terminals running; a click on the Dock icon brings it back. Cmd+Q still quits (and still asks first while a tab is busy).
- The profile menu left the top bar in every mode: Settings → Agent → Profiles lists the profiles and does the same (new default for new tabs, use in this tab, new, rename, remove).
- Academic mode: the Terminal and Subagents buttons leave the top bar and sit in the ⋯ menu.
- The sidebar's account line no longer shows the provider's plan name (for example `· plus`); it is the profile's photo and name only.
- The Chat | Code | Academic switch is now a picker at the top of the sidebar: the current mode with a chevron, opening a menu of the three with a one-line hint each and a check on the current (arrow keys, Enter, Escape).
- The Context card breaks the window down further, like Claude's: system tools, MCP tools (per server), skills and rules get their own rows with counts, and the MCP, skills and rules rows fold open to their names.
- The browser panel and the agent's browser now always run the Chromium bundled with the app (Chrome for Testing `chrome-headless-shell` 156.0.8078.80, fetched by `scripts/fetch-chromium.sh` into `resources/chromium/`, about 205 MB unpacked). The app no longer uses an installed Chrome, Chromium, Edge or Brave.
- SideChat / Academic Copilot: fixed the page shaking and the missing inner scrollbar (the pane now keeps the stage row's height, so the message list scrolls inside it and the window no longer grows with the conversation; following a stream is once per frame and only at the bottom), and the model menu now has the Effort row to change the thinking level (hidden for a model without thinking).
- System notifications now go through the OS properly. On macOS they use `UNUserNotificationCenter` (the system asks for permission once; Settings → Session → Notifications shows allowed / blocked / not asked and has a test button; clicking a banner focuses the window and opens its tab) instead of `osascript`, whose banners belonged to Script Editor. Rust decides from every tab's frames, not only the active one: a run that took 8 s or more finished, a question or approval is waiting, or a request failed or the agent died, when the window is not in front or the event is in another tab, one banner per tab and kind per 20 s. The banner shows the project folder and a fixed sentence in zh-CN, zh-TW, en-US or ja-JP, never reply text. Off by default (`docs/architecture/notifications.md`).
- The folded thinking/tools line is now a capsule (icon, one muted line, a count while running, a chevron that turns when opened, a gentle shimmer while a step runs). A reply that is only punctuation (a lone `.` or `…` some models write between tool calls) is treated as filler: it no longer shows as an empty bubble with a copy button and it joins the work box.
- Chat folds each turn's thinking and tool calls into one quiet line ("已思考 · 用了 3 个工具"; while running, a spinner with the current step) that opens to the thinking and the usual tool cards, instead of a row per thought and per tool. Failures, approvals, asks and rendered UI stay visible outside it. Interface detail steps 4 (Detailed) and 5 (Full), or Settings → Chat → Folding "Thinking and tools" = rows, bring back the separate rows. Step descriptions in Settings say what each does.
- SideChat and the Academic Copilot pane redesigned: one-line header with a phase dot, a compact empty state (the Copilot offers three starter prompts), chat-style messages, an auto-growing composer, and a model picker at the composer's bottom left (favorites, Manage models). The pick is the pane's own; Settings → Chat → SideChat stays the default. The close button had no icon (an unknown icon name) and now shows one.
- Generative UI restyled (Display and Interactive): soft cards with a status dot, radio-dot option cards, custom checkbox rows and select, chip-style recorded values, tinted badges/alerts, framed tables, themed hover/focus/invalid/disabled states in every theme, narrow-width and reduced-motion handling; wire format and behaviour unchanged. Display components moved to `layout/generative-ui-display.css`.
- omp's Auto subagent model/thinking question now shows as one compact form: a searchable model list (current model first, last pick remembered) and a row of thinking-level pills with a single **Start agents** button, instead of two radio lists plus typing; the reply to omp is unchanged.
- The Chat | Code | Academic switch moved from the window's top-left to the top of the left sidebar as a labelled segmented control with a sliding active thumb and arrow-key navigation (`design/mode-switch.jsx`).
- Work folders: new Code projects default to `~/Documents/NPi/Code`, Chat runs in `~/Documents/NPi/Chat` (was `Chats`; nothing is migrated), Academic projects live in `~/Documents/NPi/Academic`. Academic has its own sidebar and start-page projects, and a folder picked there is scaffolded with the standard research folders plus project rules and skills under `.omp/` (`docs/architecture/academic-space.md`).
- Chat mode's base prompt now makes Interactive UI the default way to ask the user anything (approach, parameters, candidate choice) with one batched form, reserves Display UI for read-only structured information, and demotes the `ask` tool to a fallback; git-branch and verification rules are scoped to code changes instead of every conversation.
- Completed assistant replies expose a persistent, borderless copy action below the output, preserving the original Markdown and existing success/failure feedback.

### Removed

- Settings → Browser → Chromium (a custom browser path, `browser.json`, `browser_settings` / `browser_set_path` / `browser_pick_path`) and automatic discovery of installed browsers. An old `browser.json` is ignored.

## [2.0.0.378] - 2026-11-02

Desktop 2.0.0.378 bundles Kernel 2.0.10, the offline managed Python runtime,
and progressive sourced images in Markdown and Generative UI.

### Added

- Progressive `image` components in Display/Interactive UI reuse the Markdown image loader: real thumbnail references, independent placeholders, full-image upgrades, attribution and explicit retry without blocking text.
- Shared per-conversation image subscriptions cancel unused requests, deduplicate sources, bound unpinned memory, and preserve stable saved-session cache identities across tab reopen.
- Host-side media scheduling: four active / sixteen queued remote transfers, thumbnail priority, cancellation through queued/active requests, a 20-second transfer budget and one transient retry. Source/MIME/SHA-256-verified cache entries persist asynchronously with a 256 MiB disk budget; HTTPS/public-IP/redirect/SVG restrictions and WebView CSP are preserved.
- Generative UI (`ui.present`): version-1 data-only interactive forms in chat with 11 native controls (`text`, `input`, `textarea`, `number`, `slider`, `checkbox`, `switch`, `select`, `radio`, `multiselect`, `button`).
- Full interaction lifecycle (`waiting_input` + `rendered` display milestone, `submitted` with `continuation: "pending" | "admitted"`, `cancelled`, `expired`), task/message identity binding, same-batch dependent-tool pause, cancellation without extra model calls, expiry on new user tasks or branch switches, and session-safe history restoration.
- Settings → Agent → Execution behaviors: `Interaction policy` (`standard` | `aggressive` | `minimal`, default `standard`), decoupled from models/providers while preserving tool approvals and mandatory subagent Auto Ask gates.
- Invalid or unsupported forms degrade to an isolated text error. Component counts, text, options and numeric ranges are bounded; generated HTML, scripts and commands are never executed.
- Embedded Python runtime for the `python` tool: pinned CPython 3.12.16 with stdlib and pip, plus uv 0.13.6, packaged as a reproducible 38.6 MB archive with CPython, pip/vendored, uv (Apache-2.0/MIT) and Tcl/Tk license notices.
- Lazy, content-addressed runtime installation with atomic environment versions, an offline `venv` built from bundled ensurepip wheels, a shared `uv` cache, and a single install lock; old runtimes remain usable by live sessions.
- `--npi-python-host` one-shot JSON protocol on the desktop executable for `ensure`, `environment`, and `packages` list/install, with resource-directory override and push-style progress.
- Source checkout build gate: the generated runtime archive is gitignored, `scripts/build-python-runtime.sh` prepares it, and `scripts/check-python-runtime.mjs` fails the build with a concrete command when the archive is missing or its size/SHA-256 does not match the committed manifest.

### Changed

- Chat mode now uses an English NPi Renaissance human-centered rich interaction policy: sourced visual answers and Display UI when useful, necessary Interactive UI only, concise ordinary chat and text-only preferences respected. Tool guidance names actual `web_search`, `read` and eval browser capabilities rather than nonexistent image-search/fetch/map tools.
- Python execution now uses the desktop-managed absolute interpreter and environment. `omp` no longer unpacks the archive, creates a virtual environment, or falls back to a system/project interpreter when the desktop host is configured.

## [2.0.0.258] - 2026-11-02

### Added

- Subagent default model policy with three tiers: Auto (interactive two-question Ask dialog selecting model and thinking before spawn), Sync (strictly mirrors primary conversation model and thinking level), and Custom (explicit model and thinking level designated from profile catalog).
- Subagent delegation appetite tiers: Unlimited ≤32 (text highlighted in rose red), Aggressive ≤8, Moderate ≤4 (default), Conservative ≤2, and Off 0 (hard exclusion of task tool and preflight spawn rejection across task, eval, and workpool).
- Project and open conversation right-click context menu Close option, preserving closed project conversations beneath Recent for direct resumption.
- Context card Previous compaction category classifying active compaction summaries separately, balancing text estimates to avoid false zero-message displays.

### Changed

- Desktop version 2.0.0.258 with bundled kernel 2.0.5.

### Fixed

- Context compaction lifecycle synchronization via RPC events, preventing sending, steering, follow-ups, and queue promotion while compaction is active while retaining user drafts.
- Custom subagent model validation rejecting unconfigured models or unsupported thinking levels without silent fallback.

## [2.0.0.230] - 2026-11-02

### Added

- Mode-specific compaction settings overlay: Chat mode applies `thresholdPercent: 70`, `methodOrder: ["soft"]`, and experimental context management; Academic mode configures `keepRecentTokens: 30000` with `handoff` and `soft` summarization.
- Chat space memory hot reload and update broadcasting: saving profile memory now emits change notifications so running and subsequent sessions dynamically track user preferences.

### Changed

- Desktop version 2.0.0.230 with bundled kernel 2.0.1.

### Fixed

- Project sidebar conversations remain separate, collapsible child rows even for a single conversation. Closing a project keeps saved main conversations accessible beneath its Recent entry, with direct resume and branch-family deduplication.
- Project and open-conversation context menus offer Close. Closing a project releases its tabs through the existing busy-close guard and leaves saved conversations available under Recent.
- Context cards refresh after automatic and manual compaction, classify the active summary as Previous compaction, and balance text estimates without letting fixed-prompt estimates erase current messages.
- Sending, steering, follow-ups and queue promotion pause during compaction; composer drafts remain editable and are preserved until sending resumes.
- Conversations search icons retain shared icon sizing instead of covering the page; action menus and narrow layouts remain usable.
- Saved-session actions resolve the selected profile before path validation. History entries retain their source profile when opened, and failed sidebar deletions display an error instead of silently doing nothing.
- The Chat sidebar groups related branch files into one main conversation row while Conversation Tree and conversation management retain individual historical files.
- Resuming an already active conversation from Settings dismisses Settings and shows the selected conversation.
- Sidebar conversation deletion now confirms and removes the complete conversation tree, including its branch files, after closing owning tabs through the busy-close guard; failures and partial completion remain visible.
- Context usage display in adapter eliminates the arbitrary 200k fallback; unknown context windows present exact token counts without deceptive percentage rings.
- Manual and automatic compaction completion triggers immediate state and message history synchronization over RPC.
- Plan usage refreshes after a conversation settles, including background tabs, and manual refresh invalidates the active profile's cached reports. Profile-scoped requests serialize, and explicit read errors are shown.
- Usage statistics provide 24h, 7d, 30d and total-history views for the active profile, including total tokens and per-provider/model input, output and cache usage.
## [2.0.0.130] - 2026-11-01

### Added

- Computer Use has its own Settings destination: profile-scoped enablement written to omp's `computer.enabled`, macOS Screen Recording and Accessibility onboarding requested only from an explicit click, optional Input Monitoring for Escape cancellation, live worker readiness, and a Stop that ends only the current computer operation.
- Native operation indicators during Computer Use: a theme-coloured border on the target window (the display for desktop-level actions), a stop prompt naming the app and action, and an agent-only cursor at the real operation point. Native input waits until the indicator is painted; the indicators never move the user's pointer, take focus, or appear in the agent's screenshots.

### Changed

- Desktop version 2.0.0.130; the bundled kernel reports the same version.

### Fixed

- Computer Use no longer reports "Needs attention" for a conversation whose agent process is not running: the tab applies the profile setting when it starts, and a startup failure is never echoed back as a Computer Use error. A refused change now names one bounded line instead of a stderr dump.
- Requesting a macOS grant shows "Waiting for macOS" until the answer is recorded, and re-reads twice, instead of reporting the grant as missing while the prompt is still on screen.
- The "paint the indicator before acting" barrier is now opt-in (`set_computer_activity_ack`), so a kernel with the barrier no longer freezes input for a desktop build that does not confirm paints. The indicator's own 3 s watchdog reports a failed paint with a reason instead of leaving the agent on a bare timeout.
- Computer Use can recover from a grant that no longer matches the running build (ad-hoc signing binds TCC to the CDHash, so every rebuild is a new app to macOS while System Settings keeps showing the old grant as on): the permission rows offer "Reset & request", which clears this bundle's record with `tccutil reset` and asks again.

## [2.0.0.78] - 2026-11-01

### Added

- Streamlined workflow pipeline: abstract high-level stage badges (调研 ➔ 编辑 ➔ 编译 ➔ 验证) with subtle downward-expanding concurrent branch chips; configurable via Settings.
- SideChat right-column alignment: standard stage grid column positioned on the right alongside subagent panes with resize handles and memory isolation.
- Enhanced Markdown typography and rich content: safe formatting for `<kbd>`, `<mark>`, `<sub>`, `<sup>`, `<ruby>`, `<details>/<summary>`, GFM callout alert badges (NOTE, TIP, WARNING, IMPORTANT, CAUTION), and `<audio>/<video>` multimedia support under verified CSP.
- Reorganized Settings interface: restructured into 7 clearly delineated, intuitive tabs (Account, General, Chat, Models, Agent, Code, Advanced) with telemetry and MCP/Skill management logically grouped.

### Fixed

- SideChat now reads the actual RPC event envelopes, preserves replies while hidden, and reports startup, model, request and process errors instead of remaining stuck thinking.
- Dedicated, temporary chat-only worker with no project tools or persisted conversation; Settings → Chat → SideChat selects its model independently. Send remains inside the input box; Stop cancels generation and Clear resets only SideChat.

## [2.0.0.60] - 2026-11-01

### Added

- Context compaction optimization: manual "Compact Context" button on the sidebar Context card, plus structured Subagent Compactor handoff prompts with protected user invariants, technical decisions, and touched artifacts.
- Settings MCP & Skill management UI: custom settings rows for viewing, toggling, and adding stdio/http MCP servers, plus listing installed skills with one-click editor opening.
- System notifications integration: OS-native desktop notifications for turn completion and pending approvals when the window is unfocused, adhering to system-managed Focus/DND policies.
- Screen capture capability: isolated `/tmp/npi/{session_id}/` screenshot storage with automatic session cleanup on tab close.
- Subagent workflow improvements: live model thinking stream passthrough (`thinking…`) in subagent inspectors while preserving the 32 concurrent task ceiling for Academic workloads.
- SideChat ephemeral scratchpad: lightweight parallel drawer for quick questions and algorithm exploration without polluting the main project context.
- Workflow visualization: collapsible floating horizontal pipeline displaying sequential phase progression and concurrent parallel task branches.

## [1.3.1] - 2026-11-01

### Added

- Rich assistant replies: inline project/public HTTPS images with source links, persistent conversation-private image caching and click-to-enlarge viewing.
- Offline LaTeX formulas, Mermaid diagrams, and declarative percentage-bar, pie, donut, bar and line charts, with streaming and theme-aware rendering.
- Public-image downloads validate and pin destination IPs; Fake-IP DNS networks use a narrowly gated Cloudflare HTTPS DNS lookup without exposing full image URLs.

### Changed

- Chat uses four semantic text levels, with the requested dark palette and existing light-theme equivalents instead of opacity-based dimming.
- Consecutive successful tool calls form compact, expandable groups. Running, failed and confirmation rows stay outside these groups; group and individual manual states remain independent.
- Real thinking content has one accessible disclosure: open while thinking, folded when finished unless kept open by preference or manual override. Ordered thinking phases and reading position survive streamed updates.

### Fixed

- Native assistant image blocks are no longer dropped during streaming, message completion or history restoration.
- Display-mode LaTeX formulas are centered in the chat reading column while preserving horizontal scrolling for wide expressions.

## [1.3.0] - 2026-11-01

### Changed

- Code-mode engineering workflow now assesses scope, researches before deciding, and asks explicitly before substantial unapproved changes. Investigation-only requests remain read-only, and already-approved decisions are not asked again.
- Alibaba(China) thinking choices and request parameters follow reviewed model-specific contracts, including native Qwen3.8 efforts and mandatory thinking.
- Floating model menus use a responsive two-column layout; folded tool summaries use tighter spacing.
- Completed subagent rows are muted, italic and struck through, with long text contained in narrow panes.
- The bottom Todo and plan card expands its live task list in place instead of opening the full plan view.


## [1.0.1-hotfix01] - 2026-10-31

### Fixed

- Bundled CLI: Ctrl+C now cancels active main-session work instead of only clearing input, leaving the session available for the next command. Cancellation no longer arms the idle double-tap exit timer.

## [1.0.1] - 2026-10-31

First NPi Desktop release (macOS, Apple silicon).

### Added

- A humanist, minimalist interface: native macOS window and sidebar, single top bar, Chat and Code modes, themes (Graphite, Ivory, Catppuccin), Noto fonts, chat wallpapers, five interface-detail steps, three chat widths.
- Chat space with its own conversations, instructions, profile and memory; Settings for Account, Chat, Models, Agent, Code and Advanced.
- Model picker by vendor, plan usage and spend, right panel, terminal and subagents as columns, sidebar right-click menu, hover tips, run status.
- The bundled `npi` command line (a reworked omp): settings tiered into everyday, "show more" and Advanced.

### Removed

- Remote access, the in-app updater and the upstream themes.

## [1.2.0-client] - 2026-11-01

### Fixed

- Subagent manager cards now contain long names, task summaries and metrics within their bounds, including narrow panes.
- Alibaba(China) thinking choices now follow the bundled kernel's reviewed model contracts, including native Qwen3.8 effort levels and mandatory-thinking models that cannot be switched off.
- Chat history no longer drops older messages after filling the 169-cell minimap. Users can scroll back to the first message; tab switches and resumed conversations also restore history from before context compaction on the current branch.

### Added

- The ambient rail has a compact **Changes** card for the GitHub PR on the project's checked-out branch, with its title, repository/number, draft state and diff counts. Click the title to open it in your browser; the menu offers GitHub links, copy link, changed files, review comments and opt-in **Auto-fix** for failed CI and unresolved reviews. The card also shows the PR's CI: the step in progress with a spinner (or the outcome) and how many checks finished, a green/red dot per finished check (a progress bar for large matrices), and on click the full list of checks, each opening its CI page; while CI runs the card updates every 15 seconds. Discovery works with Auto-fix off and refreshes automatically on tab or branch changes, turn completion and returning focus, without a manual Refresh button. Only enabled Auto-fix tabs, or a PR whose CI is still running, keep checking in the background; with Auto-fix off, a card you can't see (focus layout, tree or browser panel open, window minimized) makes no GitHub requests and updates when it comes back. Repositories still reached through a renamed or transferred URL, and fork PRs from common branch names such as `main`, are found too. **Settings → Rail → Changes** uses the same persisted toggle as the other built-in cards; turning it off hides the card, stops monitoring and revokes Auto-fix in every tab. No GitHub App, webhook server or tunnel is required. Auto-fix asks the agent to fix, verify, commit and push changes and resolve only addressed threads. It is off by default, active-tab only, with existing tool approvals unchanged; requires authenticated GitHub CLI and a synchronized upstream branch. Without GitHub CLI (`gh`) the card stays hidden and appears once `gh` is installed; a signed-out `gh` shows an error asking for `gh auth login`. Auto-fix waits for GitHub to catch up after the agent pushes instead of pausing, the agent's own reply in a review thread does not send that thread again, and switching tabs while a fix prompt is being sent leaves Auto-fix on. Turning it off prevents new fixes, and Stop disables it too.

### Changed

- Completed subagent rows now use muted gray, italic, struck-through names and summaries while remaining selectable; running and failed agents keep their distinct presentation.
- The bottom **Todo and plan** rail card now expands and collapses its live task list in place instead of opening the full plan view.
- The floating model menu shared by the composer and welcome page now arranges model choices in two columns, falling back to one column in narrow windows while preserving vendor navigation and selection shortcuts.
- Folded tool summaries use tighter vertical padding and line height without changing expanded cards, failed-tool visibility, or ordinary message spacing.
- Tool calls in chat now fold into one-line summary disclosures showing the tool, target, diff line counts and status, and unmount their body while folded. **Standard** interface detail starts tool calls folded (failed calls still open); **Full** opens them all.
- Streaming replies use omp's delta `message_update` frames: each frame carries only the new text instead of the whole reply so far. Replaying a recorded 8.7 MB session into a release build: 944 MB → 25 MB sent from omp to the window, a background tab catches up 6.7× faster (11.5 s → 1.7 s) with 4× less window CPU, and the window's JavaScript memory drops from 3 GB to 150 MB. A tab that comes back to a reply still streaming after a long time in the background shows it whole.
- OMP Desktop now requires omp 18.13.2 or newer. A tab started against an older omp does not start; its note names the installed and the required version, and an `omp update` takes effect for the next tab without restarting the app.
- Endless status animations (the tab's running dot, live dots, the activity radar, "running" chips, shimmer text) step through a few frames per keyframe interval instead of redrawing at the display's refresh rate, and the status bar and model pill dots and the activity radar move only while the agent is working (the dots used to pulse all the time, the radar until the next tool call). Together with the delta frames, the window's CPU use while the agent streams drops by three quarters (11.0 s → 2.8 s per 30 s, most of it in the GPU process), and an idle window no longer keeps the GPU busy (3.5 s → 0.08 s per 15 s).

## [0.12.1] - 2026-10-21

### Added

- Remote access: Settings → Remote access → Notifications has an "Only while the desktop is locked" switch (off by default), so the phone is notified only when you are away from the computer. Works on macOS, Windows and Linux; on Linux it needs a screen locker that reports the lock to systemd-logind (GNOME and KDE Plasma do). "Send test notification" ignores it.

## [0.12.0] - 2026-10-20

### Added

- Rail widgets: a plugin can ask for notice with a top-level `"attention": "<key>"`. Until you click the card, its border and title dot pulse in the status colour (static with reduced motion), and when it turns up while the window is visible but not focused the dock icon bounces once (widgets pause while the window is hidden or minimized, so nothing bounces then). The card head gets an "Acknowledge" button for keyboard and screen reader users. The acknowledged key is remembered across restarts, per project for a tab plugin; see `docs/widgets.md`.
- Rail widgets: a plugin entry in `widgets.json` can set `"insert": { "target": "new-tab", "project": "~/GIT/work/inbox" }`. Clicking `+` on one of its items then opens a new tab in that folder and puts the text into the new tab's prompt box (never sent); ⌥-click appends to the current tab's draft as before. The default stays `"draft"`. A missing, relative or non-existent `project` shows an error card and falls back to the draft. Only `widgets.json` sets the target; plugin output cannot. See `docs/widgets.md`.
- Remote access: read your tabs from a phone on your network or VPN, answer the agent's questions, send, steer and stop, and get a push notification when a question waits or a long turn finishes. Off by default; Settings → Remote access picks the network interface, port and host name, uses a generated or your own TLS certificate, pairs a phone with a one-time QR code, and lists and revokes paired phones. Windows, macOS and Linux; push needs a certificate the phone trusts. The installed iOS app pairs by scanning the same QR code with the rear camera, because its storage is separate from Safari. The phone page follows the desktop's theme and accent colour.
- Remote access, phone transcript: opening a session jumps to its newest message and stays there while new text arrives; scrolling up keeps your place and shows a "↓ Newest" button that jumps back.

### Changed

- Remote access always uses HTTPS. The plain-HTTP switch has been removed, and an old `allowHttp` setting is ignored rather than exposing pairing codes and device tokens without encryption.
- Remote access internals are split by responsibility into smaller phone UI, Settings row and Rust modules, with the existing protocol and settings unchanged.

### Fixed

- macOS: tools the agent starts (`kubectl`, MCP servers, widget plugins) could not reach hosts on the local network, even with OMP Desktop switched on under System Settings → Privacy & Security → Local Network. The kernel dropped their connections (`reason: NECP`) without asking. The app now declares `NSLocalNetworkUsageDescription`, so macOS shows its Local Network prompt on first use.
- The native window can now shrink to 480 × 320 instead of stopping at 960 × 640. Tiling window managers such as AeroSpace can fit it into narrower columns and shorter rows without overlapping adjacent windows. The normal 1440 × 900 startup size and saved side-column settings are unchanged.
- Remote access: an expired or already-used pairing link no longer prevents a paired phone from reconnecting with its saved token. Invalid port edits return to the saved value instead of remaining in the Settings field.

## [0.11.0] - 2026-10-17

### Added

- Resizable columns (#41): drag the line between the chat and the project sidebar, the subagent pane, the ambient rail or the conversation tree to change that column's width. The widths are saved and come back on the next launch. Double-click a line (or focus it and press Enter) for the default width; with a line focused, the arrow keys move it 16 px (64 px with Shift) and Home/End jump to the narrowest and widest. A column grows only into room the chat can spare, so the chat keeps at least 420 px; in a smaller window the columns narrow first, down to their minimum widths. Settings → Layout → Column widths shows each width, and "Reset all widths" puts every column back, the browser panel included. The browser panel's edge takes the same keys.
- File preview (#39): click the path on a read card, or a link to a project file in the agent's reply, to see the file in a preview window. Code is syntax-highlighted, and the lines the agent read are tinted and scrolled into view (a link's `#L12` or `:12-20` works the same way). Images show whole. "Open in git browser" switches to the git browser, and so does Ctrl/⌘-click. Only files inside the tab's project open; a symlink pointing out of the project is refused. Text files over 1 MiB and images over 8 MiB show as too large; a longer text file shows its first 20,000 lines, each cut at 500 characters. A link written as a Windows path (`C:/proj/a.rs`) or a bare `main.rs:12` opens the preview too. Images the agent read appear as thumbnails on the read card and open in the image viewer. A write card now names its file and its size, and its path opens the git browser like an edit card's.
- A Settings page (#37) that opens as a tab after your project tabs: the cog in the status bar, `Ctrl+,` (`⌘,` on macOS) or `/settings`. Every setting is in one place, grouped into Appearance, Layout, Chat, Composer, Rail, Terminal, Browser, Agent, Session, Approvals and Shortcuts, with a search over all of them (`Ctrl+F`). A dot marks a setting you changed; reset it, or a whole section, with one click. Changes apply at once. The tab shows how many settings differ from the defaults, `Ctrl+W` closes it, `Ctrl+Tab` reaches it after your project tabs, and Esc there never stops the agent. Picking a project tab leaves Settings open where you were.
- The shortcut editor now lives in Settings → Shortcuts; `Ctrl+/` and `/shortcuts` open it there. A binding that comes from omp's own `keybindings.yml` can be changed too; the change is saved as a desktop override.
- New settings: **Reopen tabs on launch** (on by default; off starts with no tabs and leaves the saved list alone until you open or close a tab), **Ask before closing a working tab** (off by default; asks before closing a tab whose agent is working, retrying, waiting for your answer or running background jobs, and before quitting with such tabs, also from the window's close button, Alt+F4 or the OS), **Name tabs automatically** (on by default; off sends no automatic `/rename`), **Send with** `Enter` or `Ctrl+Enter` (for the prompt box, an ask's answer box and a subagent's steer box; the other chord adds a new line), **Collapse pastes longer than** 2–50 lines (default 5), **Follow new output** (on by default; off keeps the transcript where you scrolled while the agent works), terminal **Scrollback** (1,000 to 50,000 lines, default 5,000; applies to open terminals too), **New tabs show** the rail, the conversation tree or the browser beside the chat, **Auto-compact context** and **Retry failed requests** (both on by default; off turns omp's automatic compaction or retries off in every tab).
- Settings → Approvals sets the tool approval mode and the per-project overrides, the same controls as the approvals panel, and shows how many always-allow rules apply, with a link to the panel.
- Settings → Agent sets the startup profile and whether omp keeps working on a goal on its own between turns.
- Settings → Session shows the installed version, the last update check and a version you skipped, which "Clear skipped" offers again.

### Changed

- The floating Tweaks panel is gone; everything it held is in Settings. `/tweaks` is now `/settings`. The status bar's autosave switch and the scanlines setting are removed.
- A picked accent colour now also sets the matching soft, glow and dark shades (focus rings, glows, the terminal's selection), instead of leaving the theme's own shades next to it.

### Fixed

- Typography on Windows: interface text now uses Segoe UI Variable (Segoe UI on Windows 10) and code uses Cascadia Mono, Cascadia Code or Consolas, instead of falling through the macOS-only fonts to Arial and Consolas. Chinese and Japanese characters still take Windows' fallback font for the display language.
- Windows title-bar buttons (minimize, maximize, close) were coloured red, yellow and green by leftover prototype rules. They now use the theme's text colour, draw as line icons instead of text glyphs, and sit flush with no gaps or rounded corners.
- When the app's settings folder can't be found, "always allow in this project" approvals are now kept only until the app closes, instead of being read from and saved to the shared temporary folder, where another user on the same machine could have placed approvals of their own.
- When the app's settings folder can't be found, the profile list, open tabs, recent projects and shortcut changes are now kept only until the app closes, instead of being read from and saved to the shared temporary folder. Another user on the same machine could have placed files there that chose the folder and profile the app's agent starts in at launch. Settings → Shortcuts then shows no settings file path.
- Opening the git browser, or a file in it, no longer runs commands that the repository's own config attaches to files as clean or process filters (`filter.<name>.clean` / `process` in `.git/config`). A downloaded or extracted repository could use them to run a program just because it was previewed. Filters from your own git config, such as Git LFS installed with `git lfs install`, still apply, and so do line-ending conversions. A submodule now shows as changed only when it has a different commit checked out, not for edits inside it.
- A project folder can no longer start its own `omp` in place of yours (security). With an empty or relative entry in `PATH` (`.`, `bin`, or a stray `::`), a tab opened on a project that shipped an executable `omp` could run it with your permissions. omp is now looked up only in absolute `PATH` folders, its version is checked on that exact file, and the session runs that same file. omp, and so every command the agent runs, also gets `PATH` without those entries, so a project can't supply the interpreter an npm-installed omp starts with either: a project-local tool reached only through a relative entry such as `./node_modules/.bin` now needs its path spelled out. An `omp` on `PATH` that you aren't allowed to run is passed over for the next one, as a shell does.
- The example git widget (`examples/widgets/git.sh`) no longer runs programs named by the project's own git configuration. A repository could make it run a command of its choice: a clean or process filter when the widget checked for changed files, or a signature checker (`gpg.program`) when it listed commits. The widget now turns off every filter the project's configuration defines or overrides (filters from your own git configuration, such as git-lfs, keep working), never shows commit signatures, never fetches missing objects of a partial clone, and no longer counts a submodule as changed for edits inside it, only for a different checked-out commit. It now needs git 2.26 or later. Copy the script to `~/.omp-desktop/widgets/` again to get the fix.
- "Allow for this session" / "Always allow in this project" on a tool-approval prompt now always applies to the tab and project whose agent asked. A prompt that reached another tab during a fast tab switch could create the standing rule for the tab you had just switched to. A tab now ignores events from a session it has already switched away from, and the app grants a rule only for an approval prompt that tab's agent is still waiting on, with the tool and project that agent asked about.
- Stage and discard in the git browser can no longer run a `git` that the project itself ships (security). With an empty or relative entry in `PATH` (`.`, `bin`, or a stray `::`), an executable `git` in the project could run with your permissions. `git` is now looked up only in absolute `PATH` folders and run by that exact path, with `PATH` minus those entries for the helpers it starts (credential helpers, ssh, an editor). A `git` reachable only through a relative entry is no longer found: stage and discard then report that git was not found.
- Rail widgets: a plugin's script could be swapped between the approval check and the start of the run, and the swapped-in code then ran under the old approval. Runs now execute verified copies of the plugin's scripts, kept in the app's cache directory under the approved fingerprint, made from exactly the bytes that were checked and compared with the originals again before every run. Anything named by absolute path that loads files next to itself (a widget's `$(dirname "$0")/…` or relative `import`, a launcher like `npx.cmd`) no longer finds them; keep a widget in one file and start launchers by name. A native program named first in `command` (on Windows: a path ending in `.exe`), files over 8 MiB and folders still run from their own path (`docs/widgets.md`).
- The browser panel's Chromium could be controlled by other user accounts on the same computer: its debugging port and the agent's connection point accepted anyone on the machine. Chromium now opens no debugging port at all and talks to the app over a private pipe, and the app answers the agent's browser connections only from programs running as your own user.
- "Allow for this session" / "Always allow in this project" on a tool-approval prompt now approves the prompt and saves the rule in one step. Before, a failure in between, or the tab's agent restarting at that moment, could leave a saved rule for a prompt that was never approved, or send the approval to the restarted agent. The rule is now saved only after the approval reached the agent that asked; if it could not be delivered, nothing is saved.
- Windows and macOS: a program the app started at the same moment as the browser panel's Chromium (an agent, a widget, the terminal's shell, git) could inherit Chromium's private pipe and with it control the browser behind the app's back. The pipe is now open to Chromium alone: the app starts no other program while Chromium is being started.

## [0.10.1] - 2026-10-16

### Fixed

- macOS no longer says OMP Desktop "is damaged and can't be opened" when the app was downloaded with a browser (#60). The app is now signed ad hoc, so on first launch macOS asks you to allow it in System Settings → Privacy & Security → Open Anyway instead. It is still not notarised by Apple.

## [0.10.0] - 2026-10-15

### Added

- A browser panel next to the chat (#58): see and use the same browser the tab's agent works in. Open it with the globe button in the status bar, `Ctrl+Shift+B` (`Shift+⌘B` on macOS; rebindable in the shortcuts screen) or `/browser [url]`. The panel follows the page the agent is on and has page tabs, back/forward/reload and an address field; drag its left edge to resize it or switch to full view. Mouse, scroll, keyboard, paste and input methods reach the page, while the app's own panel and tab shortcuts keep working. **Annotate** lets you click an element or drag over an area and add a comment; it goes out with your next message (or the goal you start) as text plus a screenshot of the area. What comes from the page is marked as page content, so a page can't give the agent commands. Localhost links in chat open in the panel (`⌘`/`Ctrl`+click opens your normal browser; switch it off under Tweaks → Browser). The panel uses Google Chrome, Chromium, Edge, Brave or the Chrome for Testing omp downloads; the app finds one itself, and the `chromium` menu under Tweaks → Browser lets you choose another. Each tab gets its own browser with a fresh profile and no extensions, started only when needed and closed with the tab. The agent works in the page you are looking at, so its navigation replaces yours.

### Changed

- The conversation tree is now open or closed per tab, like the browser panel: switching tabs shows each tab's own side column (the rail, the tree or the browser).
- The status bar now runs along the whole bottom of the window, under the side panels too, and stays on one line: when space is short it drops the tokens/s, cost and context meter cells, in that order, instead of wrapping or cutting off its buttons.

### Fixed

- Icons now grow and shrink with the font size set in Tweaks, like the text next to them. Until now they stayed the same size, so at a larger font size the icons in the git browser, the session history, the Tweaks panel, the tabs, the sidebar and the chat looked too small and were hard to read. Icon-only buttons and the Tweaks panel grow with them. A crowded status bar no longer squeezes its icons either.
- The model pill below the prompt box no longer sits lower than the thinking, plan mode and goal pills next to it.
- A terminal shell path in `terminal.json` wrapped in quotes, as Windows' "Copy as path" gives it, now works.

## [0.9.0] - 2026-10-15

### Added

- Choose omp's tool approval mode in the app (#57). The approvals panel, opened from the ✓ button in the status bar, sets how often the agent asks before it acts: `always ask` (it asks before every file change and every command; only reads run on their own), `write` (the default: it edits files on its own and asks before shell commands and anything else that runs code) or `yolo` (every tool runs without asking, shell commands included, and omp's own stop for dangerous commands such as `rm -rf /` is skipped too; shown in amber). The mode is a default for all projects, and each project can have its own. A change restarts the agent in the tabs it affects, and the conversation picks up where it was; a tab that is busy switches when its turn ends, and until then the status bar shows both modes (`write ⇢ yolo`). Your saved approval rules are kept.
- A git browser for the tab's project. The git button in the status bar, `Ctrl+Shift+G` (`Shift+⌘G` on macOS; rebindable in the shortcuts screen) or a click on the file path in an edit card opens it over the whole window, in three panes. **Files**: the project tree with each changed file marked (modified, added, deleted, renamed, untracked), a "changed" switch that shows only those, and a filter by name; ignored files are left out. **File**: the whole file with its change against the last commit inline — staged and unstaged together, removed lines in place, added lines highlighted, old and new line numbers — opened on its first change, with buttons to jump between changes, stage, discard, and copy the diff. **History**: the commits of every branch, remote branch and tag as a graph, with the branch you are on, how far it is ahead of or behind its upstream, and your uncommitted work as the top row. Click a commit to browse it: the files pane then lists the files it changed and the file pane shows a file as of that commit with that commit's change inline; click the top row to get back. It refreshes by itself when the branch changes, when the agent edits a file and after a stage or discard. Very large repositories, files and histories are shown up to a limit (20,000 files, 20,000 lines per file, 2 MiB per file, 500 commits), and a capped view says so.

### Changed

- The status bar no longer shows the model and thinking level: the pills below the prompt box already show both, so the status bar has more room for the rest.
- The status bar's git button opens the git browser instead of the Changes panel, which is gone; its stage and discard buttons are in the browser's file pane.
- Reading the repository (status, file contents, history) no longer runs the `git` program; the app reads the repository itself. Stage and discard still run `git`.

### Fixed

- Arrow Up no longer replaces a prompt you are typing with your last sent one (#55). Up and Down now move the caret in a typed draft, wherever it is. Up brings back earlier prompts only from an empty prompt box, as in omp's own editor. While you step through them, Up on the first line and Down on the last line move between prompts, and Down past the newest one empties the box again. `Ctrl+↑` still opens the searchable picker at any time.

## [0.8.0] - 2026-10-13

### Added

- A shell in every tab (#56). The terminal button in the status bar, `` Ctrl+` `` / `` ⌘` `` (rebindable in the shortcuts screen) or `/terminal` opens a real shell in a panel below the chat, in the tab's project folder (your home folder when the tab has none) and with your normal shell profile. Hiding the panel keeps the shell running, so output produced meanwhile is there when you come back; closing the tab ends the shell and everything it started, and switching the tab's profile keeps it. After `exit`, press Enter in the panel to start a new one. A program that prints faster than the panel can draw is slowed down instead of making the app sluggish. Drag the panel's top edge to resize it (the height is kept). Under Tweaks → Terminal you pick the shell new terminals start from a menu: `default` and the shells installed on this computer — on Windows `cmd`, `powershell`, `pwsh`, Git Bash (`bash`) and `nu`; on Linux and macOS `sh`, `bash`, `zsh`, `fish`, `dash`, `ksh` and `nu` — or any other program through "other…"; there you can also open the panel with every new tab. `Ctrl+C`, `Esc` and other `Ctrl`/`Alt` keys in the terminal go to the shell, not to the agent; the toggle-terminal and next/previous-tab shortcuts, `⌘` shortcuts and (outside macOS) `Ctrl+Shift` shortcuts still reach the app. Copy and paste with `Ctrl+Shift+C` / `Ctrl+Shift+V` (`⌘C` / `⌘V` on macOS).

### Fixed

- Tweaks: a long path no longer runs off the panel. The `plugins` row under Rail shows the full path to `widgets.json` on its own line, wrapped.

## [0.7.0] - 2026-10-13

### Added

- Send a waiting message right away (#47). While the agent works, each steer and follow-up listed above the prompt box has a ⚡ button: it stops the agent's current step, as Esc does, and the agent continues at once with the message (and any other steers waiting). Before, a steer waited for the agent's next step and a follow-up until the agent was done.
- Drop images onto the prompt box (#50). Drag image files from the file manager onto the prompt box to attach them, as with the attach button or a paste: they show as thumbnails you can remove, keep the text you already typed, and go out with the next message, text or not. Other files are ignored. A file dropped anywhere else in the window is refused, and so is a link dropped outside a text field (it would have opened in your browser).
- Collapse all / Expand all (#43): `/collapse-all` and `/expand-all` in the command bridge and the prompt box's `/` menu fold or unfold every tool card and thinking block in the current tab at once; blocks that arrive later follow your settings again. Both can be given a key in the shortcuts screen.
- `/tweaks` opens the Tweaks panel from the command bridge or the prompt box, like the cog in the status bar.
- Tweaks → Rail: show or hide the context gauge, agent radar, subagents card and minimap individually; the choice is kept across restarts (#51).
- Rail widgets (#49): your own cards in the ambient rail, between the subagents card and the minimap. List a program in `<app config>/widgets.json`; it prints JSON to stdout (status, numbers, progress, key/value rows, text, lists, links) and the desktop draws it as a card. Nothing runs until you approve the plugin's exact command and script in the rail; if either changes later, it stops and asks again. Approved plugins refresh by themselves when you switch to a tab, when a turn ends, when the branch changes, on their refresh button or every `intervalMs`; a turn that ends in a background tab refreshes its cards when you switch to it. At most four run at once, a hidden card or rail runs nothing, and closing a tab or quitting stops its plugins. List items can add text to your prompt or open a link. Each plugin card has its own toggle under Tweaks → Rail. Examples in `examples/widgets/`, setup and trust model in `docs/widgets.md`.
- omp extension widgets in the rail (#52). What an omp extension shows with `ctx.ui.setStatus` and `ctx.ui.setWidget` used to be dropped; it now appears in the rail after the plugin cards: one `status` card with a row per status line (omp's own `/review` and `/annotate` report their progress there), and one card per widget with its lines in monospace, both marked `extension`. Each tab shows its own, including what a background tab published meanwhile; they stay through `/new`, branch and fork, as in omp's terminal UI, and go when the tab's omp process ends. Each card can be hidden under Tweaks → Rail. Example extension in `docs/widgets.md`.

### Changed

- Thinking and tool output fold separately (#43). Under Tweaks → Chat, *thinking* and *tool output* each choose auto (fold a long block once enough newer ones follow it, as before), open (never fold on their own) or folded (start folded). Folded tool output still shows a running command, an edit's diff and a call that failed. They replace the auto-collapse switch; if you had turned it off, both start as open.
- The collapse button on tool cards and thinking now shows what a click does: a down chevron on a collapsed block expands it, and an up chevron on an open block collapses it. Before, a collapsed block showed a right chevron and an open one a down chevron, which read as "expand". The button is also larger and grows with the font size set under Tweaks → Look, like the text beside it.

## [0.6.1] - 2026-10-12

### Added

- Reorder tabs and projects by dragging (#40). Drag a project along the tab bar or the project sidebar, or a conversation within its project in the tab-bar dropdown or the sidebar; a line shows where it will land, and Esc cancels. `Ctrl+Shift+PageUp` / `Ctrl+Shift+PageDown` move the current tab's project one place left or right (rebindable in the shortcuts screen). The order is kept when the app restarts.
- Tool cards and thinking can be collapsed (#44). Everything starts expanded; once enough newer tool calls and thinking blocks have followed a long one, it folds down to its first few lines (a shell command to its last few). Short ones never fold on their own. Each has a chevron button to collapse or expand it by hand, which always wins and is kept when you switch tabs. Nothing folds while you are scrolled up reading. Set how many newer blocks to wait for, or turn auto-collapse off, under Tweaks → Chat.

### Changed

- A tab for a project with one conversation now shows the project's folder before the conversation's title (`omp-desktop · fix tab close`), in the tab bar and the project sidebar (#46). The folder is shortened first when space runs out, and left out while the conversation is still named after its folder.
- A project's conversations are listed newest-opened first, in the tab-bar dropdown and the project sidebar (#38), until you drag them into another order.
- A tool call one of your approval rules let through now shows what was approved, not just the tool: a compact "auto-approved" row under the tool's card names the tool and what the call does — the command, the file, the action — and a click unfolds everything the approval prompt would have shown (#42). Before, the conversation only said "Auto-approved bash via your approval rule" in a full reply bubble.

### Fixed

- A project with several conversations now has a close button (×) on its tab-bar chip, like a single tab (#45). It closes one conversation — the one you are in, or the newest when you are in another project — never the whole project; its tooltip names the conversation.

## [0.6.0] - 2026-10-04

### Added

- Messages waiting for the agent are listed above the prompt box: steers (Enter while the agent works) and follow-ups (Ctrl+Q), in the order the agent will get them. Each row can be removed (✕), taken back into the prompt box to edit (✎, text only), or — for a follow-up — sent as a steer right away (↑), which also starts the agent again after you stopped it. A message the agent already took cannot be pulled back; the row then says "already delivered". The list is omp's own queue, so it stays correct across tab switches. A steer or follow-up omp refuses leaves a note with its text.
- Talk to a running subagent, or stop it, from the subagent manager. Open an agent while it works: a message box under its details sends it a message as if you were its user (Enter sends, Shift+Enter starts a new line). The agent reads it at its next step; while the agent is open in the manager, the message then shows up as a `you ›` line in its Output tab. A message the agent refuses stays in the box with a note saying why; so does text you were still typing when the agent finished. The stop button asks once more before it ends the agent; the agent that started it gets an aborted result and carries on.
- Pick the thinking level from a menu (#10): click the thinking pill under the prompt box. It lists the levels the current model supports, each with what it means, and ticks the one in use. Shift+Tab still cycles through the levels.
- When the agent asks you questions, they now arrive together in one card: every question of the ask at once, with each option's description, the recommended option marked, previews where the agent gave them, and a box to type your own answer to each. Questions that take several answers have checkboxes. Nothing is sent until you press Submit, which waits until every pick-one question has an answer; a lone pick-one question still answers on click. Cancel stops the turn, as Esc does. Once sent, the card shrinks to the answers you gave. Before, the questions came one at a time, and every click on a multi-answer question added another prompt to the chat.
- A tab whose agent has finished its reply but still has a background job running (a backgrounded shell command, task or eval) now shows it: a hollow ring in the tab bar and the project sidebar, and a note under the prompt box, until omp reports the work done. The agent picks the job's result up by itself; a row naming the finished job (its command and run time) then appears above the reply it triggered. The update dialog counts such a tab as busy, since restarting would kill the job.
- A conversation tree with the prompt-cache status of every prompt. Open it from the branch button in the right rail or the status bar, or type `/tree` (or `/branch`): it replaces the right rail and lists the conversation as a git-style graph, one row per prompt, oldest at the top, with branches in separate lanes and a divider at each context compaction. Each row says whether asking that prompt again would reuse the provider's prompt cache — cached, expiring in under 10 minutes, partly cached (the part up to an earlier cache anchor, which omp re-marks every 15th prompt, is still cached), or cold — and what asking it again would cost now and after the cache expires; when the provider reports no cache lifetime, the rows show no cache colours or prices. Select a row to **Branch here** (a new conversation without that prompt and everything after it; the prompt goes back into the prompt box), **Fork after the reply** (a new conversation that keeps the prompt and its reply), or, for a prompt that only exists in another conversation of the same family, **Open in new tab**. Branch and fork are disabled while the tab's agent works. A new omp process (a tab opened from the tree, or a tab reopened after a relaunch) writes the conversation to the cache again, because its system prompt differs (with a memory backend, the recalled memories): the tree prices **Open in new tab** that way and shows points cached by an earlier process as cold, tagged "earlier process".
- When a request fails and omp retries it by itself (an overloaded or unreachable provider, say), the conversation now says so: a row with the attempt (such as 3 of 10), the provider's error, a countdown to the next try or how long the current try has been waiting, and a Stop button — between tries it stops retrying and shows the failure, during a try it stops that try as Esc does. Meanwhile the tab bar and the project sidebar show a spinning amber dot, and the update dialog counts the tab as busy. Failed tries no longer leave empty replies behind: the reply says after how many failed attempts it came, or how many there were before omp gave up or you stopped it.
- Goal mode: give the agent a goal it keeps working toward until it calls it done. Click the new **goal** pill under the prompt box (or type `/goal`, optionally followed by the goal), optionally pick a token budget (presets or your own, such as `300k`), and what you send becomes the goal. A strip above the prompt box then shows the goal, its status, the tokens used against the budget and the time spent, with pause, resume and drop (drop asks first); once the agent completes the goal, the strip keeps the final usage until you dismiss it. The conversation marks each change: goal set, paused, resumed, budget reached, complete, dropped, and every turn omp started by itself to continue the goal. Esc pauses a running goal, as in omp; resume picks it up again. A goal that was running when OMP Desktop closed comes back paused. The **auto-continue** switch in the strip is omp's own setting (`goal.continuationModes`) for the tab's profile: with it on, omp keeps working on the goal between turns by itself; with it off (omp's default when an app such as OMP Desktop drives it), each turn needs a message from you, and the strip says so while it waits. Plan mode and goal mode exclude each other, as in omp. Until omp fixes it, the agent's own goal calls ask "Allow tool: goal"; allowing it for the session or project stops the prompts.

### Changed

- OMP Desktop now requires omp 18.4.11 or newer. A tab started against an older omp does not start; its note names the installed and the required version, and an `omp update` takes effect for the next tab without restarting the app. The list of waiting messages, subagent steering, the question card, the conversation tree's fork and goal mode (see Added) rely on RPC commands that older omp versions lack.
- A steer or follow-up now appears in the conversation when the agent actually reads it, not when you send it, and no longer pulls the view down if you are reading further up. Until then it is in the list above the prompt box.
- Pick-from-a-list prompts from extensions, such as `/review`'s pickers, no longer have a box for typed text: they only accept one of their own options.

### Fixed

- The chat stays on its latest line when the area below it grows — the list of waiting messages, attached images, or a long draft — instead of hiding the end of the conversation behind it until the next update.
- A tab that fails to start — omp too old or not on `PATH`, or its profile deleted meanwhile — now says why, in the current tab or on the empty workspace, when it was opened with `+`, from a recent project or from the history panel. Before, nothing happened.
- After picking a model whose highest thinking level is lower than the current one, the thinking pill shows the lowered level right away instead of at the next prompt.
- A model without thinking showed `thinking · auto`; it now shows `off`, as omp does. The status bar showed `xhigh` as `max`, and omp's own `max` level as `—`.
- An answer you typed to the agent's question reached the agent as if you had picked an option of that name; it now arrives as your own answer.
- A request the provider rejected — after omp's own retries — left an empty reply, which vanished entirely after a tab switch. The reply now shows the failure: the HTTP status, provider and model, the provider's own message, and its full response behind "provider response". "temporary" marks a failure omp considers transient, such as an overloaded provider, where sending again later may work. A failed request that produced no output is not kept across a restart: omp leaves it out of the conversation it resumes.
- After switching tabs and back, the conversation keeps its order: a reply that followed a tool call could jump above it, and every later entry shifted along with it, the last one sometimes vanishing. Notes the app adds itself (an automatic tool approval, a login result) and messages omp dropped from its own context (an attempt it retried) no longer disappear on a tab switch either.

## [0.5.0] - 2026-10-03

### Added

- Rename a conversation from the tab bar or the project sidebar (#32): double-click a tab or a sidebar row, or use the pencil on a sidebar row or in a grouped tab's dropdown, then type the name and press Enter (Escape cancels). The name is saved with the conversation, so it survives a restart or resume, and from then on the automatic title never overwrites it. It works while the agent is running too, and leaves nothing in the chat. Typing `/rename <title>` still works the same way.
- Copy buttons on agent output (#33). Hover an assistant message, a code block, a bash output, an eval cell's code or output, or an edit's diff, and click the copy icon to put its raw text on the clipboard: a message copies as the markdown the agent wrote, a code block as the exact fence content, and a bash output in full — including lines scrolled out of the card, without omp's wall-time and exit-code notes. The button briefly shows "copied".
- Skills can be picked in the middle of a message (#30), the way omp's own editor does it: typing `/` after other text (`review this change /`) opens a list of skills only, which narrows as you type a name or any part of a hyphenated name (`/rev` finds `skill:code-review`). Picking one inserts `/skill:<name>` at that spot and keeps the rest of your text, which omp passes to the skill as its instructions. Escape closes the list without touching the message. A message that invokes a skill this way also reaches omp as a skill while the agent is still working, and plan mode sends it as typed, like a `/skill:<name>` at the start.
- The model picker lists your recently used models first (#11): up to five, newest first, in a "recently used" group above the full list, which stays unchanged below. The order is omp's own — the same one its terminal model selector uses — so a model you switched to in the terminal, in another tab, or by `/model` counts too. It is kept per profile, includes only models you can pick right now, and follows the filter as you type.
- The app reopens the tabs that were open when it last closed (#17) — after quitting, a restart, or an in-app update. Each tab comes back in its place with its project, profile and conversation, and the tab you were on is active again. A tab that had no conversation yet reopens empty on its folder. A tab whose folder no longer exists, or that fails to start, is not reopened; a note in the chat says which. Folders opened with "Open with OMP Desktop" at launch open after the restored tabs and become the active tab.

### Fixed

- Pasted text stays readable (#31). A sent message keeps its line breaks instead of running them together into one line. A long paste (more than 5 lines or 500 characters) still goes into the prompt box as a `[paste #N +K lines]` placeholder, but a chip above the box now shows it: click the chip to read the pasted text, or "expand" to turn it back into editable text in the prompt. Once a draft is taller than the prompt box, it scrolls instead of hiding the rest.
- After an in-app update on Windows, the relaunched app connects to omp again instead of every tab staying silent until a manual restart (#34). The MSI installer starts the new version with only the machine-wide PATH, so an omp installed for the current user (`%LOCALAPPDATA%\omp`, on the user PATH) was not found. omp is now spawned with the machine and user PATH from the registry appended to whatever PATH the app was started with. The same goes for `git`, so the Changes panel also keeps working with a per-user Git install (winget `--scope user`, scoop).
- The history panel's filter row offers one chip per project that has saved conversations, each with its count, instead of only "All Projects" and the current project (#35). The current project's chip comes first, and stays there with a count of 0 when it has none; the rest follow their most recent conversation. Two folders with the same name show their parent folder (hover a chip for its full path), and a long list of projects scrolls inside the filter row.

## [0.4.2] - 2026-10-02

### Fixed

- The prompt box and plan mode belong to each tab instead of being shared by all of them (#28). Switching projects no longer carries over a half-typed message (which Enter would then send to the other project), its attached images or collapsed pastes, plan mode, or plan comments. Each tab keeps its own until you come back; an image still being prepared when you switch lands in the tab it was pasted into.

## [0.4.1] - 2026-09-30

### Added

- A new conversation's tab is named after its folder at first, then renames itself to the conversation's generated title once the first exchange finishes (omp's RPC mode never auto-titles, so the app asks for one with `/rename` after the first turn); the title is then refreshed after 5 and 10 further turns to keep up with the work, then left alone, and a `/rename` you type yourself (`/rename Title` or `/rename:Title`) always wins. The project sidebar's single-tab card shows that name too. Resumed conversations keep their saved name. `/new` in the same tab drops the old title and earns a rename of its own (#13).

### Fixed

- Release builds stamp every script and stylesheet URL with the app version (`?v=X.Y.Z`), so a webview that cached a previous version's files after an in-app update can no longer serve them: the WebView2 data folder survives the updater, and embedded assets carry no cache validators, which could leave an updated app running a mix of old and new frontend modules until the cache happened to evict.
- The Linux `.AppImage` starts when another user mounts it — `firejail --appimage`, the AppImage catalog's test, root-extracted copies — instead of failing with `AppRun.wrapped: Permission denied`. Tauri CLI before 2.12 shipped that launcher as owner/group-only (`0770`); builds now use Tauri 2.12, and a release fails if any file in the AppImage is limited to its owner.

## [0.4.0] - 2026-09-30

### Added

- Project sidebar (toggle with `Ctrl+B` / `⌘B`, the tab-bar button, or the *project sidebar* tweak): open tabs grouped by project — hover a project to start a new conversation there — plus the recently opened folders of the active tab's profile; click one to open it, hover to drop it from the list (#27).
- Tabs on the same folder and profile collapse into one tab-bar chip with a count; its dropdown switches between them or starts a new conversation in that project — a second tab on the same folder and profile, i.e. a duplicate of that project card (#27, #12).

### Changed

- Opening a recent project, or resuming a conversation from history, focuses its tab when it is already open instead of starting a second copy (#27). `+` / `Ctrl+T` still always opens a new tab.
- A folder picked with `+` / `Ctrl+T` is opened at its real (symlink-resolved) path, the same form "Open with OMP Desktop" and the recent-projects list use, so an open project is always recognised (#27).
- The app no longer opens an empty "OMP Desktop" tab at launch — an agent running in whatever directory the app happened to start in, with no project behind it. With no tab open (at launch, or after closing the last one) the main area shows an animated "digital rain" welcome screen with an OMP-DESKTOP logo (static when the system asks for reduced motion); projects open from the sidebar or the tab bar, and no agent process runs until one does.

## [0.3.3] - 2026-09-25

### Added

- Middle-clicking a project tab closes it, like in a browser (#24).
- Clicking an image attached to a message pops it out into an enlarged viewer; with several images, switch between them with the arrow buttons or ←/→ keys, close with Esc or a click (#25).

### Fixed

- Conversation history is now ordered by when a session was last active (its most recent message), not by when it was created — a session you continue days after starting now correctly sorts to the top (#23).
- Raw HTML in agent replies and plans is shown as text instead of being rendered, so markup like `<img onerror=…>` can no longer run scripts in the app.

### Changed

- Release builds no longer allow inline scripts or inline event handlers (Content Security Policy), as a second line of defence against injected markup.

## [0.3.2] - 2026-09-24

### Fixed

- Markdown links whose scheme is hidden behind HTML character references (e.g. `&#106;avascript:`) now render as plain text instead of clickable links.
- A code block's language tag can no longer inject HTML attributes into the rendered page.

### Changed

- Faster startup in release builds: the interface ships precompiled and uses React's production build, so the first screen appears in about 0.1 s instead of 1.7 s, and the bundled frontend files shrink from 5.4 MB to 1.3 MB. `npm run dev` is unchanged.
- Stricter Content Security Policy: `eval` is no longer allowed (nothing in the app needed it).
- All dependencies updated: React 19.3 (was 18.3), marked 18 (was 12), highlight.js 11.12, Babel 7.29.9, Tauri 2.11.6 and its plugins, `gix` 0.87, `notify` 8, `sha2` 0.11, `windows-sys` 0.61, Tauri CLI 2.11.5. This also clears two `quick-xml` security advisories.

## [0.3.1] - 2026-09-24

### Added

- In-app updates (#19) — the tab bar now shows the running version (it used to show a hardcoded placeholder) and, once a newer release is published, an update pill. The pill, the version label, `/check-updates` and a rebindable **Check for updates** shortcut open a panel with the release notes and **install & restart**, which downloads the signed update, verifies it, closes every tab's omp process and relaunches. It warns first if any tab is mid-turn. Windows, macOS and the Linux AppImage update themselves; `.deb`/`.rpm` and source builds only get the notice and a link to the release page. The app checks 15 s after launch and then every 6 h; background checks never open anything, and **skip this version** hides the notice until a newer version appears. Automatic checks can be turned off in Tweaks → Session. Installs from before this release have to be updated by hand once.

### Changed

- Release builds are now signed for the updater. The release workflow needs the `TAURI_SIGNING_PRIVATE_KEY` secret (plus `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` if the key has one) and fails early without the key. Once every platform has built, it publishes `latest.json`, using this changelog's section for the version as release notes. It won't publish a feed that is missing a platform, or one whose tag disagrees with the app version. A manual run of the workflow now names its release after the `tag` input; it used to name it after the branch. Releases only build after the full test suite passes (Rust tests on Windows/Linux/macOS plus the JS regression scripts), and CI now runs that same suite; it previously ran only `cargo check`.

## [0.3.0] - 2026-09-24

### Added

- Usage statistics panel (#22) — a status-bar button surfaces cost/token telemetry for the active tab's profile, sourced from `omp stats --json` (requests, error rate, cost, cache rate, tokens in/out, avg throughput/TTFT), broken down by model, by project folder, and by agent type (main/subagent/advisor). Covers a rolling last-24-hours window and the active tab's own profile only — omp's `--json` output has no flag for a wider time range, and resolves against one profile per invocation the same as every other per-tab omp spawn — so switching tabs to a different profile and reopening the panel shows that profile's own last-24h usage, not a merge across profiles.
- Prompt history in the composer (#16) — Arrow Up/Down recall the tab's previously sent prompts (bash/readline-style: only when the caret sits on the first/last line, so multi-line edits still use the arrows normally), and Ctrl+Up/Cmd+Up opens a scrollable picker over the last 100 (configurable in Tweaks → Session) prompts, click or Enter to load one into the composer without sending. History is per-tab and in-memory only — never written to disk — gathered as you send, and backfilled from the transcript whenever a tab is (re)activated or a saved session is resumed, so switching back to an older tab or resuming from history repopulates it instead of starting empty.
- Minimap legend (#21) — the session minimap grid colored each cell by role/tool with no key explaining what the colors meant. A small legend row now sits under the grid, mapping color to meaning: you, assistant, ask, tool.
- Subagent manager — live introspection of the agents a `task` call spawns. A **subagents** card in the right rail shows how many are running, their combined tokens/cost/tool calls, and what each is doing right now (current tool + intent, elapsed time). Opening the manager (the card's button, the Tweaks **split** layout, or the new inspect icon on any agent row of a task card) adds a column with totals, a swimlane timeline of recent activity (the last few tool calls of up to 8 agents active in the past ~2 minutes), live/done/failed filters, and agents grouped by the task call that spawned them — nested agents indented under their parent, and each group can jump the chat to its task card. Clicking an agent opens an inspector: its task and assignment, the tool it is running now, tokens/cost/wall time/requests/model/context, recent tools, recent output plus a live event stream, and its full transcript. Finished agents stay listed for the rest of the session (omp itself forgets them), and a background tab re-syncs on return, marking agents that finished meanwhile as ended. The live event stream is only requested while an agent is being inspected.

### Changed

- Removed the "peer session" rail card, split pane and `split` tab chip — prototype leftovers that never had live data behind them. The Tweaks `split` layout now opens the subagent manager instead.

### Fixed

- The `/` palette and ⌘K bridge never received omp's slash-command list on connect: the backend's command allowlist was missing `get_available_commands`, so the request introduced for #8 was rejected before reaching omp and the list only filled in if omp happened to push an `available_commands_update`. It is now allowlisted, and a Rust test checks every `_send`/`_sendWithResponse` call site in `live.js` against the allowlist (failing on any call site whose command type it cannot read).

## [0.2.2] - 2026-09-23

### Fixed

- Image attachment in the composer — the attach-image icon had no click handler and clipboard paste only ever read plain text, so both silently did nothing (#7). The icon now opens a native file picker; pasting an image (screenshot, copied file, or a bitmap alongside real text from a spreadsheet/rich-text app) now attaches it as a thumbnail and sends it to the agent as an image content block, with the accompanying text preserved. On Linux (WebKitGTK), the synchronous paste event never carried image data at all even after that fix — only `text/*` — so clipboard image paste still silently did nothing there; pasting now falls back to the async Clipboard API when the synchronous path finds no image, which WebKitGTK does populate correctly.
- Clicking a link in agent output (e.g. a GitHub URL) navigated the app's own window in place, with no back control — the only recovery was quitting and losing every tab's session state (#14). The webview now stays on the app for every internal navigation; `http(s)`/`mailto` links are handed to the system's default browser/mail client instead. Everything else is refused: a same-origin file-path link no longer reboots the app in place, and a `javascript:`/`data:` link (which a webview would otherwise execute before any navigation even starts) is rendered as plain text instead of a clickable link.
- Chat force-scrolled to the bottom on every streamed update while the agent worked, with no way to read earlier content mid-run (#20). The transcript now auto-scrolls only while already pinned to the bottom; scrolling up unpins it (per tab) and shows a "Jump to latest" button. A new streamed message no longer forces the view back on its own — scrolling back down to the bottom, sending, steering, queueing a follow-up (which now shows in the transcript immediately rather than once the agent gets to it), clicking the button, or the transcript being reset (`/new`, a profile switch) all re-pin.
- A tab opened from history with a long (often auto-generated) session title could span nearly half the window width and squeeze out every other tab (#15). Tab labels now truncate with an ellipsis at 200px and show the full title on hover.
- The `/` composer palette and ⌘K command bridge only ever listed a static 12-entry desktop command list — omp's real slash commands (`/security`, `/fast`, `/mcp`, …) and every discovered skill (`/skill:<name>`) were invisible, with no way to run them except typing the full name from memory (#8). Both now merge that static list with omp's live `get_available_commands` (fetched on connect, refreshed on `available_commands_update`, scoped per tab/profile). A desktop entry (`/plan`, `/history`, …) still runs its own handler; picking an omp-native entry inserts `/name ` into the composer so you can add arguments and send it as a normal prompt, which omp executes.

## [0.2.1] - 2026-09-19

### Added

- "Open with OMP Desktop" on a folder — Finder, Explorer (right-click a folder or a folder's background) and Linux file managers can hand a directory to the app, which opens it as a project tab. Opening a folder while the app is running adds a tab to the existing window and brings it to the front; opening one while it is closed starts it with that folder's tab instead of the empty launch tab. Opening a folder from the command line (`omp-desktop /path/to/project`) does the same.
- Keyboard shortcuts — every desktop action (tabs, panels, model cycling, session history, plan mode) is now a bindable, rebindable chord; the main ones ship with a default binding out of the box. The Shortcuts screen (`Ctrl+/` or `/shortcuts`) lists all actions with their effective chords, their source layer (omp config / desktop overlay / registry default), and lets you rebind, add a second chord, clear, or reset. Rebinds are stored in `<app config>/keybindings.json`; omp's own `~/.omp/agent/keybindings.yml` is read-only and respected as the base layer. Dispatch honours the focused element: typing characters in the composer is never intercepted, but `Ctrl+`/`Alt+`/`Super+` combos and `Escape`/`Shift+Tab`/Fn keys work from anywhere. The command bridge's own Escape handling (back out of the model/login view, then close) now runs standalone instead of behind a global override, so `Escape` from a drilled-in view returns to the command list before it closes the bridge.

## [0.2.0] - 2026-09-17

### Added

- Per-tab omp profiles — each tab runs its own `omp --profile <id>` (separate auth, sessions, settings, caches); create, rename and delete them from the title-bar selector, and tick one as the default that new tabs and the next launch start in. The built-in `default` profile keeps using `~/.omp/agent`; switching a tab's profile restarts that tab's agent only.
- Approval rules — "Allow for this session" / "Always allow in this project" on approval prompts, plus a panel to review and revoke them. Sessions now start with `--approval-mode write`, so exec-tier tools ask instead of auto-approving.
- Changes panel — `git status`/`git diff` for the active tab's project with per-file accept/reject, bounded so large changesets stay responsive.
- `@`-mention autocomplete for project file paths in the composer.
- Conversation history and resume are scoped to the tab's own profile.
- `confirm` and `editor` prompts render as chat cards (previously auto-cancelled and invisible); `input` no longer uses a blocking browser dialog.
- Per-tab run state in the tab bar: running / waiting-on-you / idle / failed.
- Switching back to a background tab recovers the tool cards, prompts and streaming state that arrived while it was away.
- Credentials (tokens, API keys, auth headers) are redacted from agent output and logs.
- Hardening: only known RPC command types reach the agent, and saved-session identity comes from the session filename.
- `test-rpc.mjs --evidence` writes a redacted golden snapshot of the RPC surface to `docs/agents/evidence/`.

### Fixed

- Closing a tab or quitting now kills the agent's whole process tree — no orphaned subagents or tool-call children.
- Enter that commits an IME candidate no longer sends a half-typed message.
- Bridge button in the title bar sat ~7px above the window controls.
- Ask prompts could be answered twice, or answered after the agent had already cancelled them.
- "Agent process exited" / "Agent failed to start" notices rendered as blank bubbles, hiding why a session died.
- A tab whose agent dies during startup now explains itself instead of going blank. Any death before the agent's first output — a rejected flag, an exec failure, or no model resolving — previously read as a clean, silent exit; the tab now shows omp's own stderr reason, including the exact command to run when it's a missing-credentials profile. Background tabs report it too, when next selected.
- A freshly created profile's tab died silently at startup (no credentials, so omp had nothing to boot) and `/login` plus the provider list had nothing to talk to. `create_profile` now seeds a minimal `models.yml` so the tab boots far enough to run `/login`; the seed is removed once login succeeds.
- Several prompts raised in one turn overwrote each other, wedging the tab with tool cards stuck at "running".
- Approval rules could be silently lost when two windows saved at once: a stale-lock takeover let the previous holder delete the new owner's lock file, admitting a third writer mid-write.
- Lock contention on the rules and profile files surfaced as `File exists (os error 17)` instead of a readable "another window is updating this file; try again".
- A failed session spawn (omp missing from PATH, an exec error) left a dead tab in the tab bar with no agent behind it, and a failure partway through startup could leave a running omp process that nothing could reach or shut down.

### Changed

- Large diffs and long tool-output streams are now bounded in memory (journal capped at 8 MiB per session; `git diff` read through an early-stopping pipe).

## [0.1.3] - 2026-09-13

### Added

- Conversation history panel (`Ctrl+H`/`⌘H`, `/history` command, or the clock icon in the tab bar) — browse, search, and resume past omp sessions persisted under `~/.omp/agent/sessions`. Backend adds `list_saved_sessions` (async, off the Tauri main thread) and a validated `resume` path on `start_session`; `resume` values are checked against the sessions directory before being forwarded to omp's argv, rejecting flag-shaped or out-of-tree paths.

### Fixed

- Tab switch drops all tool cards from chat — `get_messages` returns only text entries; tool/ask/compact cards live exclusively in live event state. Fixed by merging `get_messages` ground-truth text into the existing snapshot (preserving tool cards in-place) instead of replacing `state.messages` wholesale. `activeToolCards` indices are rebuilt after merge so in-flight `tool_execution_update` events continue landing correctly.
- Minimap cell stuck pulsating after tab switch — `streamingBubble` restored from snapshot was never cleared when `get_state` reported `isStreaming: false` (turn completed while away); `_applyRpcState` now retires the bubble and strips `streaming: true` entries from `state.messages` immediately, before `get_messages` arrives.
- Model picker empty / no models listed — `get_available_models` returns ~2 MiB (837 models), overflowing the RPC v1 1 MiB physical-frame cap and failing with `RPC response exceeded the transport limit`, leaving `state.models` empty. The bridge now negotiates protocol v2 (`negotiate_protocol`) in `_initFetch` before the large fetches and reassembles the resulting `rpc_chunk` sequence (base64 byte-segments → strict UTF-8 → JSON) in `handleLine`. Reassembly validates chunkId/index/count/byteLength, rejects interleaved or duplicate frames, and enforces the 64 MiB ceiling.
- Built `.app`/`.dmg` shows an empty model picker (and no working session) when launched from Finder/Dock/DMG, while `tauri dev` works — macOS hands a GUI-launched bundle a minimal `PATH` (`/usr/bin:/bin:/usr/sbin:/sbin`) that omits Homebrew, so `Command::new("omp")` in `agent/spawn.rs` failed to resolve `/opt/homebrew/bin/omp`; the default session never spawned and `get_available_models` never ran. `spawn_omp` and the `rpc-ui` probe now override the child `PATH`, appending common install dirs (`/opt/homebrew/{bin,sbin}`, `/usr/local/{bin,sbin}`, `~/.local/bin`, `~/.cargo/bin`) after the inherited entries (de-duplicated, inherited PATH still wins).

## [0.1.2] - 2026-05-11

### Fixed

- macOS freeze (spinning beach ball + high CPU) when opening a project folder via the + button — `blocking_pick_folder` was called from a command-handler thread, deadlocking against the main RunLoop; switched to callback-based `pick_folder` with an async command and `spawn_blocking` channel bridge

## [0.1.1] - 2026-05-10

### Added

- `/login` command with OAuth provider picker (fetches providers via `get_login_providers` RPC)
- Ask tool rendered as inline chat bubble with `rpc-ui` mode support _(requires [can1357/oh-my-pi#994](https://github.com/can1357/oh-my-pi/pull/994) to be merged)_

### Performance

- Fixed 13×13 minimap grid (169 cells); oldest row of 13 messages evicted at turn boundary once the grid is full, keeping memory and render cost bounded in long sessions
- `React.memo` on all bubble components (UserBubble, AssistantBubble, ToolCard, AskBubble, CompactRow); only the live streaming tail re-renders per token — stable history bails out
- Stable `_id` stamped on every message object in `live.js`; bubbles keyed by `_id` instead of array index, eliminating remount/fade-in blink when the oldest row is evicted
- `useCallback` on `handleAnnotate` and `handleAskAnswer` in App to stabilize function-prop refs and preserve memo bailouts for AssistantBubble and AskBubble

## [0.1.0] - 2026-05-10

### Added

- Initial Tauri 2 shell: spawns `omp --mode rpc` per tab, no bundler, JSX transpiled in-browser via `@babel/standalone`
- GitHub Actions CI (cargo check + cargo test on win/linux/mac) and release pipeline
- Per-tab omp process isolation — one process per tab, preserved across switches via session snapshots
- Model picker as a separate bridge view with on-load fetch and refresh button
- Markdown rendering with syntax highlighting (marked v12 + highlight.js) in chat
- Plan mode: full intent → drafting → review → running → done lifecycle with inline block annotations
- Slash command palette with arrow-key navigation, fuzzy filter, and Enter execution
- `/new` command to start a fresh omp session in the current tab
- Steer: send a message to the agent mid-turn without waiting for completion
- Compact tool cards: full expand/collapse card showing live progress and final result
- Task/quick_task tool cards: collapsible subagent panel with live-stream view on row click
- Eval cell tool cards: stream code and output live; syntax highlight on completion
- Auto-scroll chat to bottom as the agent streams output
- Minimap: dense grid heatmap with chat-bubble cross-highlight and per-kind tooltips
- Long paste collapse into `[paste #N +K lines]` inline tokens in the composer
- macOS-style traffic light window controls on Windows (DWM frameless)
- Autosave toggle button in the status bar
- Font size slider in the tweaks panel (75–150%, step 5)
- Git branch chip in the title bar via `gix` + `notify`
- OMP icon pack v1 as app icons across all platforms
- MIT license

### Fixed

- Black screen on startup — disable Tauri CSP hash injection, remove Google Fonts CDN link
- Git HEAD watcher — watch `.git/` directory instead of `HEAD` file to survive atomic rename on Linux/macOS
- Window drag — replaced custom handler with `data-tauri-drag-region`
- Diff block overflow — contained within chat column width
- Composer textarea: single-line default via `field-sizing: content`; focus restored after send; textarea stays enabled during streaming for steer input
- Phantom textarea scrollbar hidden at min-height
- Window control symbols: always colored red/yellow/green, no hover background bleed
- Tweaks panel: persist settings to `localStorage`; retheme to use app CSS variables
- Token and context gauge percentages truncated to one decimal place
- Stream line accumulation — handle in-place growing lines without duplication
- ToolCard: remove duplicate `return` statement; expand individual subagent rows, not the card header
- Plan annotations always reaching the prompt; `sendFeedback` working with annotations and no body text
- Plan running→done state transition
- Message history preserved across tab switches
- Tab name retained from folder path when `omp sessionName` is absent
- `_handleResponse` in `live.js` — missing closing brace caused silent IIFE syntax error
- Thinking level values aligned to valid RPC set (`off | minimal | low | medium | high | xhigh`)
- Rust agent: race-safe sessions, lock-free per-session stdin writes, no orphan child processes on hot-reload

### Changed

- Project renamed from `omp-desktop` to `Oh My Pi Desktop`
- Split large files into focused modules: `agent.rs` → `agent/`, `app.jsx` → `app-live.jsx` + `src/app/`, monolithic CSS and chat/UI/tweaks components into dedicated directories
- Plan mode moved from a dedicated side panel into the chat timeline
