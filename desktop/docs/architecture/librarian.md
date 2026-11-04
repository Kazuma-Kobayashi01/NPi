# Librarian (reference library)

Plan and phases: `docs/PLAN_LIBRARIAN.md`. This file records what exists. L0 (data layer), L1 (the window), L2 (citation styles), L3 (PDF import, files, online lookup) and L4 (the library as tools for the Copilot) are built.

## Storage

One library per Academic project, in `<project>/.npi-academic/research.db` (SQLite through `rusqlite`; `PRAGMA user_version` is the schema version, a newer one is refused). Schema 5 (v5 adds the Research Core tables, see `research-core.md`; a version-1 to 4 file is copied to `research.db.bak-v<n>` before it is migrated; the connection waits up to 5 s for a writer). Table `suggestions` (v4): `ref_id`, `kind` (`tag` / `group` / `note`), `value`, `reason`, `status` (`pending` / `accepted` / `dismissed`), timestamps; a resolved row stays as the log of what was applied. Table `attachments` (v3): `ref_id`, project-relative `path`, content `hash`, `kind`, `added_at`. Table `refs`: the CSL-JSON record (`csl_json`) plus the columns it is compared and searched by (`title`, `title_norm`, `author`, `author_norm`, `year`, `doi`, `kind`), `citekey` (unique), `source` (`manual` / `ris` / `bibtex` / `csl-json`), `notes`, timestamps and `deleted_at`. `groups` (unique name, ignoring case) and `group_refs` hold the manual groups; `ref_tags` the tags (a tag is its text, unique per reference ignoring case). Deleting only sets `deleted_at`: the row stays, so a citekey is never reused; the trash lists those rows and restoring clears it (groups, tags, notes and files were never touched). The database is the user's work, not a cache.

## Rules

- Citekey: `<first author><year><first title word>`, lower case, a letter suffix when taken. Editing a record keeps its citekey (drafts cite it).
- Duplicate: same DOI, or same normalised title + year + first author, among live rows. Import reports repeats (also within one file) and adds nothing for them; nothing is merged automatically.
- Formats: RIS, BibTeX/BibLaTeX and CSL-JSON in and out. BibTeX reading handles nesting, quotes, `@string` and `#`; accent commands keep the letter and lose the mark, other LaTeX is not interpreted. CSL-JSON export puts the citekey in `id`.
- The webview sends a project *name* (`AcademicSpace::project_named`), a reference id, CSL-JSON or import text. Never a path or SQL.

## Commands (`librarian/commands.rs`)

References: `library_list`, `library_add`, `library_update`, `library_remove`, `library_import` (text), `library_import_pick` (Rust opens the file dialog and reads a `.ris` / `.bib` / `.json` file, 16 MiB at most), `library_export` (text), `library_export_save` (Rust opens the save dialog and writes it; the webview never names a path). Formats: `ris` | `bibtex` | `csl-json`; an import returns `{ added, duplicates }`.
Organising: `library_groups`, `library_group_add/rename/remove`, `library_member` (put in or take out of a group), `library_meta` (replace tags and notes; tags are cleaned, de-duplicated ignoring case, 30 of 40 characters at most; a note is 64 KiB at most). Removing a group removes only the memberships.
All run through `off_ipc`; the webview reaches them through `OMP_BRIDGE.library(name, project, args)`, which lets only the listed names through.

## The window (`design/librarian/`, `app/librarian.js`)

`LibrarianWindow` opens in the Settings overlay frame from Academic's ⋯ menu ("Librarian") while a project tab is active: groups and tags on the left, a searchable and sortable list in the middle, one reference's form on the right (edits stay in the form until Save; a group tick applies at once). Search is a case-insensitive substring match of every term over title, authors, year, journal, DOI, publisher, keywords, citekey, tags and notes (so Chinese words work); it runs in the webview over the loaded list, not in SQLite. Export writes what the list currently shows. "Copy citation" gives `Author (Year). Title. Journal.` until citation styles exist. `app/librarian.js` holds the pure parts (`test-librarian.mjs`); the form keeps every CSL field it does not show.

## Citation styles (L2, `librarian/format.rs`)

`hayagriva` 0.10 with its bundled CSL archive (see `THIRD_PARTY_NOTICES.md`). Records go in as BibTeX (our writer, then hayagriva's reader), so the formatter sees what an export would. Built-in styles offered first: APA 7, IEEE, Chicago (author-date, notes), MLA, Nature, Vancouver, Harvard, GB/T 7714-2015 (numeric, author-date, note) and 2005 numeric; checked with Chinese and English records (`王芳. 信息检索原理[M]. 北京: 科学出版社, 2019.`). The user can add `.csl` files (`library_style_add_pick`): parsed as XML only (no scripts), refused unless an independent style, 2 MiB at most, stored in `<app config>/csl-styles/<slug of title>.csl` and listed as `user:<slug>`; a style id never names a path. Commands: `library_styles`, `library_format` (reference list and in-text citations for all or some ids; numbered styles keep their `[1]` label), `library_cite_draft` (Rust opens a Markdown/text draft, replaces `[@key]` / `[@a; @b]` with in-text citations, appends `## References`, and opens a save dialog; unknown keys are reported and their markers left as written). The window has a style picker (kept per project in `localStorage`), "Copy citation" / "Copy reference" for one entry (plain text and HTML), "Copy list" for what the list shows, and "Cite a draft…".

## PDFs and online metadata (L3)

- **Draft from a PDF** (`library_pdf_pick`, `pdf_text::first_page`, `librarian/pdf_meta.rs`): Rust opens the file dialog, stages a copy in `<project>/.npi-academic/inbox/<hash>.pdf` (a staged file nobody confirmed is removed after 7 days) and reads page 1 for a draft by plain rules: first DOI or arXiv id, a year outside identifiers, the title (a Markdown heading, else the first line that is not page furniture, joined with a wrapped second line) and a line of names. Every field the rules filled is listed in `guessed` and marked "check" in the form. A scanned PDF gives an empty draft named after the file. Checked on text fixtures (English, Chinese, nothing), not on real journal PDFs.
- **Confirm** (`library_add_pdf`): nothing enters the library or `papers/` before the user presses "Add to library". Then the record is added (source `pdf`) and the PDF is copied to `papers/<year>-<author>-<short title>.pdf` (a number is added if another file has the name, the same file is reused); the original is never touched. A repeat (same DOI, or normalised title + year + first author) is reported, nothing is added, and the form offers "Attach the PDF to it" (`library_attach_pdf`).
- **Attachments**: `library_attach_pick` (attach a PDF to an existing reference), `library_attachment_open` (only a file inside the project opens, through the spawn gate), `library_attachment_remove` (forgets the link; the file stays). A file that moved shows as "missing", not silently dropped.
- **Online lookup** (`library_lookup`, `librarian/lookup.rs`): only when the user clicks "Look up online…". The hosts are `api.crossref.org` and, when it has no record or none with this title, `api.openalex.org` (preprints, arXiv and much Chinese work are only there), over HTTPS with no redirects, 15 s timeout, 2 MiB answer cap; it sends a DOI, or a title (at least 8 characters) and the first author's family name, never a PDF or other library content. A DOI gives one record (Crossref's CSL-JSON transform); a title gives up to 3 candidates. The form shows the found fields against its own, ticks the ones that differ, and "Take the ticked fields" changes the form only; Save keeps it.

## The library as tools for the Copilot (L4, `librarian/agent.rs`, `suggest.rs`, `copilot.rs`)

The Copilot gets four more host tools, bound to its project: `library_search` (title, authors, year, venue, DOI, citekey, tags, notes, abstract; more matched terms rank higher), `library_get` (one record by citekey with the user's notes, tags, attached files and its reference and in-text citation in a built-in style), `library_check` and `library_suggest`. The main chat gets the read-only three as `academic_library_search`, `academic_library_get` and `academic_library_check` (it cannot suggest). The webview cannot register or call any of them.

- **Check** (`library_check`, also "Check a draft…" in the window): deterministic, over `[@citekey]` markers, `Author (Year)` / `(Author, Year)` mentions (English, and Chinese like `张伟等（2021）`) and DOIs. Each is `matched`, `year-differs` (the library has that first author with another year), `not-in-library` (possibly invented, or not added yet) or `unknown-key`. Words like "In 2020" are not read as an author. Numbered citations (`[1]`) name nothing and are not checked. The model only reports the findings; it does not judge whether a source supports a claim.
- **Suggest** (`library_suggest`): a tag, group or note for a reference becomes a `pending` row and changes nothing. The reference shows "Suggestions from the Copilot" with Accept / Dismiss (`library_suggestion_resolve`); accepting applies it through the same calls the form uses (a tag is added, a note appended, a group joined or created), dismissing closes it. The same suggestion is not queued twice, 12 wait per reference at most, and the window says how many are waiting when it opens.
- **Rules and skill**: the Copilot prompt and the project's `.omp/RULES.md` template say to cite only what the library holds, to mark anything else unverified, and to change the library only when asked to organise it; `.omp/skills/librarian/SKILL.md` is scaffolded for new projects (existing projects keep what they have).

## Trash (`library_trash`, `library_restore`)

"Trash" in the window lists what was removed, newest first, and "Restore" brings a reference back with its groups, tags, notes and files. There is no permanent delete: removed rows are kept (the plan's "project recycle area"), and a restored reference may repeat a live one; nothing merges them.

## Permission tests

Negative cases are tests, not promises (`cargo test --lib`): a command names a project only by a plain folder name (`..`, `.hidden`, `a/b`, an absolute path and a link to a folder outside the space are all refused: `commands.rs`); two projects never see each other's references, trash or suggestions; an attachment opens only when, with links resolved, it is a file inside the project's `papers/` (`..`, absolute paths, other folders, directories and a link or a tampered database path that leaves the project are refused), and a `papers/` or inbox folder replaced by a link carries nothing outside (`attach.rs`); the Copilot's library tools reach only their own project's database, read drafts only from the read folders (no `..`, absolute path, `.omp/`, `.npi-academic/` or link out) and can only suggest; the chat's library tools take a plain project name, cannot suggest or save, and do not answer the Copilot's tool names (`copilot.rs`). Not covered: the dialog-driven writes (export, cite-a-draft) and the `set_host_tools` handshake end to end.

## Copy as… (journal and regulation presets)

`librarian/copy-menu.jsx` is the Librarian window's "Copy as…" popover: what to copy (reference list, in-text citation, or BibTeX / RIS / CSL-JSON text through `library_export_text`), for the selected reference or the whole list, and a search box over `app/librarian-journals.js` (`OMP_JOURNALS`): about 80 presets of journals, publishers and regulations, each `{ label, style, exact, match, note }`. A click on a result sets that style and copies; `exact` is false for "closest bundled style", `style: null` means none is bundled and the note says to add the journal's `.csl`. The table says which style a journal is named after, not what an editor accepts. `format::styles` lists every independent style of the bundled hayagriva archive (group `common` for the short list, `archive`, `user`); a Rust test formats a reference in each of them and checks that every preset's style id is bundled and independent.

## Search

The window ranks its list with the project's FTS5 index when one exists (`library_search`; short Chinese words work) and still filters by substring, which finds fragments inside words and is all there is until the project is indexed.

## Built-in library tools of Academic sessions (`librarian/tools.rs`, `copilot::AcademicHost`)

An Academic-mode session whose folder is an Academic project gets that project's tools as **built-in host tools** (answered by Rust, like the Copilot's: no approval prompt, no MCP to connect, no `project` argument): everything the Copilot has (`research_*`, `library_*`) plus `library_lookup` (Crossref), `library_align`, `library_add` and `library_edit`. `app_mode::ACADEMIC_LIBRARY_HINT` is appended to the session's system prompt with the calling order and examples, and the `librarian` skill repeats them. Permissions are wide but bounded: no tool deletes; every change to a reference is first written, as it was, to `.npi-academic/agent-edits.jsonl`; `library_align` previews unless `apply`, fills blanks unless `overwrite`, and never applies a record whose title differs (`mismatch`); `library_add` never adds a duplicate. Errors are written for the model: a mistyped citekey gets the nearest ones, a missing argument is named. Other Academic sessions (not in a project folder) keep the read-only hub (`academic_*`); Chat has none.

## The Librarian as an MCP server (`librarian/mcp.rs`)

The app binary is also an MCP server for one project: `omp-desktop --npi-mcp-librarian <project folder>` (headless, stdio, JSON-RPC 2.0; `main.rs` handles the flag before Tauri starts, like `--npi-python-host`). The Librarian window's **Connect agents** button (`library_mcp_register`) adds it as `npi-librarian` to `<project>/.mcp.json`, which omp's `mcp-json` provider reads, so `npi` started in that folder (and any other MCP client) sees it; other servers in the file are kept, a file that is not valid JSON is refused untouched.

Tools: the Copilot's `library_search/get/check/suggest` and the read-only research ones (`research_search`, `research_read`, `research_check_quote/number/table`, `research_ocr`, `research_summaries`), none that writes files; plus the four of `tools.rs` (the same ones the app's Academic sessions have built in): `library_lookup` (Crossref by DOI, or by title and first author; at most 3 candidates) and `library_align`. `library_align` takes a `citekey` or `all: true` (the first 20 references lacking a DOI or venue); it asks Crossref by DOI, else by title and first author, and accepts a record only when the shorter title's words are at least 85% inside the other (a subtitle does not matter); a record with another title is reported as `mismatch` and never applied. `apply` defaults to false (a field-by-field preview); with `apply: true` blank fields are filled and a user's value stays; `overwrite: true` also replaces differing values. Only the DOI or the title and first author leave the machine, to the pinned Crossref host (`lookup.rs`). 

Checked with a hand-written MCP client against the debug binary and the real Crossref (lookup by DOI and by title); the alignment logic by unit tests with a fake lookup. Not checked: omp actually connecting to it from `.mcp.json`.

## Working with the list (`librarian-window.jsx`, `lib-hooks.jsx`, `lib-batch.jsx`, `app/librarian.js`)

- **Unsaved edits**: `settled()` saves the open form before anything that re-opens it from what is stored (moving the selection, attaching, locating or unlinking a file, a group tick, a suggestion, renaming the key, aligning, closing the window). If saving fails the form stays with its error and nothing moves on.
- **Selection**: click, Cmd-click, Shift-click, arrows, Shift-arrows, Cmd+A. A filter that hides part of a multiple selection takes those out. Several selected: group, tag, align, trash; two selected can be merged (`Library::merge`: the kept one's values win, blanks are filled, tags, notes, groups, files and pending suggestions move, the other goes to the trash).
- **Built-in filters** (`smartCounts`): missing metadata, no PDF, possible duplicates, suggestions waiting. A row shows when it has none.
- **Citation key**: editable (`library_citekey`; plain characters, a key that is used, trash included, is refused). `[@key]` beside it, and "[@key]" in Copy as…, copy how a Markdown draft cites it.
- **Drag to a group**: a row (or the whole selection it belongs to) dragged onto a sidebar group joins it. Pointer events, not HTML5 drag and drop, because the panel window's native file drop takes every HTML5 drag; a move under 5 px stays a click (`useLibRowDrag`).
- **Keys**: Esc leaves the search, then a multiple selection; it never closes the separate window. Cmd+F is the search, Cmd+S saves the form. The sort is remembered.
- Publisher markup in titles (`<i>`, `&amp;`) is cleaned on the way in (`csl::clean_markup`) and hidden on the way out (`plainText`), so old rows read clean too.
- A new reference is a form until "Add to library", so an abandoned one leaves nothing. It goes through the import path and is refused as a repeat.

## Adding what exists elsewhere

- **Import…** (`lib-import.jsx`) takes a file or pasted text. `transfer::import_any` judges the format by the content, not the name: BibTeX, RIS, CSL-JSON, PubMed `.nbib`, Web of Science plain text, EndNote XML (`tagged.rs`; these last three are read only). Pasted DOIs, doi.org links, arXiv ids and links (`parseIdentifiers`; an arXiv id is looked up as its DOI) are looked up one at a time; the ones not found stay in the box. Repeats are reported, not added.
- **PDFs**: "Add PDF…" (several at once) and dropping on the window feed one queue, one draft at a time, with what is waiting shown and skippable. `library_pdf_dropped` takes a path from the OS's drop event or dialog, checks it is a `.pdf` and only reads it.
- A filed PDF is indexed by itself a moment later (`research_index` with `text: false`), and the Reader, asked for a file the index has not reached, indexes first.
- **Missing files**: "Locate…" (`library_attachment_locate`) looks under `papers/` for the recorded content hash (a rename by hand), else asks for the file, copies it back to `papers/` and repoints the row.

## Not built yet

A "model reads page 1" option for PDFs without a DOI, batch-adding a folder of PDFs without a form each, a tab in the tab bar (it is a window). The PDF rules were not run against real journal PDFs. OpenAlex's parser and the three new import formats were checked on hand-written samples, not on real exports or the live service. The Copilot tools have not met a real model (the `set_host_tools` handshake is still unverified end to end). The window's latest interactions were run in a browser against a fake bridge, not in the real app: native file drop, the dialogs and closing with an unsaved form are unchecked there.

## PDFs from an agent (`librarian/pdf_tools.rs`)

`library_attach` binds a PDF to a reference: `citekey` and `path` (a project file, or an absolute path to a PDF, which is copied into `papers/`, the original untouched; a file already under `papers/` is recorded where it is), `citekey` alone (looked for under `papers/`), or `all: true` (every reference without one, 50 at most). `library_add` takes `pdf`, and without it binds a PDF it finds; a repeat still takes the PDF, and a PDF that fails leaves the reference added with a note. Finding: a file under `papers/` that no reference holds is the paper when page 1 shows its DOI or whole title (12 characters at least, ignoring case, spaces and punctuation), or its file name holds the title; a scan counts by file name only. Two candidates bind nothing and are reported.

Any `.pdf` path an agent names can be copied in, the Copilot's included: only a file starting `%PDF`, and into this project only.

## Organising, and the Copilot as a tool (`librarian/organize_tools.rs`, `copilot_worker.rs`)

`library_overview` (what needs doing), `library_group`, `library_tag` (many references at once) and `library_merge` (preview, then `apply`; the other goes to the Trash) join `library_lookup/align/add/edit/attach` in `LibraryTools`. Academic sessions, the MCP server and now the Copilot all have them: the Copilot's pane runs `AcademicHost`, so it has the network tools too and can organise the library when asked, and `library_suggest` stays for what nobody asked for. Its prompt says so; a project's `RULES.md` that still has the old "never change the library" line word for word gets the new line in the prompt (the file is not touched, a rule the user wrote stays).

`copilot_ask {task}` runs the Copilot as a separate agent and returns its report. It is a built-in tool of an Academic session in a project (`copilot_worker::MainHost`, with that session's profile) and the one tool of a second MCP server, `omp-desktop --npi-mcp-copilot <project folder>` (default profile), which "Connect agents" registers as `npi-copilot` with an 11-minute `timeout` (omp's default is 30 s). A run is omp with `--no-tools`, `--no-session` and the rest of the side-chat flags, cwd `.npi-academic/copilot-run`, `yolo` because only the host tools exist and nobody can answer a prompt. It ends at `prompt_result`; a dialog is cancelled; 10 minutes at most, 2 runs at once per process, no conversation between runs, and its tools do not include `copilot_ask`.

**Approval**: omp gives a tool with no declared tier the highest one, so in `write` mode it asks about host tools too. The reader answers those prompts itself for the tools the worker registered (`answer_host_approval`), before the notification step; omp's own tools still ask.

Checked end to end with the debug binary and a real model on a temporary project (grouped and tagged two references through `copilot_ask`, which also exercises `set_host_tools`). Not checked: `copilot_ask` from inside the app, or `npi` reaching `npi-copilot` through `.mcp.json`.

## Letting a terminal `npi` call the MCP tools without prompts

omp asks before each MCP tool call (MCP tools are in the `write` approval tier). To allow the Librarian's tools, put them in `~/.omp/agent/config.yml`:

```yaml
tools:
  approval:
    mcp__npi-librarian__library_search: allow
    mcp__npi-librarian__library_get: allow
    mcp__npi-librarian__library_lookup: allow
    mcp__npi-librarian__library_align: allow
    mcp__npi-librarian__library_add: allow
    mcp__npi-librarian__library_edit: allow
```

(or run `npi` with `--approval-mode yolo`). The app's own Academic sessions do not need this.
