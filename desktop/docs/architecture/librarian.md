# Librarian (reference library)

Plan and phases: `docs/PLAN_LIBRARIAN.md`. This file records what exists. L0 (data layer), L1 (the window), L2 (citation styles) and L3 (PDF import, files, online lookup) are built; L4 is not.

## Storage

One library per Academic project, in `<project>/.npi-academic/research.db` (SQLite through `rusqlite`; `PRAGMA user_version` is the schema version, a newer one is refused). Schema 3 (a version-1 or version-2 file is copied to `research.db.bak-v<n>` before it is migrated). Table `attachments` (v3): `ref_id`, project-relative `path`, content `hash`, `kind`, `added_at`. Table `refs`: the CSL-JSON record (`csl_json`) plus the columns it is compared and searched by (`title`, `title_norm`, `author`, `author_norm`, `year`, `doi`, `kind`), `citekey` (unique), `source` (`manual` / `ris` / `bibtex` / `csl-json`), `notes`, timestamps and `deleted_at`. `groups` (unique name, ignoring case) and `group_refs` hold the manual groups; `ref_tags` the tags (a tag is its text, unique per reference ignoring case). Deleting only sets `deleted_at`: the row stays, so a citekey is never reused. The database is the user's work, not a cache.

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
- **Online lookup** (`library_lookup`, `librarian/lookup.rs`): only when the user clicks "Look up online…". The one host is `api.crossref.org` over HTTPS with no redirects, 15 s timeout, 2 MiB answer cap; it sends a DOI, or a title (at least 8 characters) and the first author's family name, never a PDF or other library content. A DOI gives one record (Crossref's CSL-JSON transform); a title gives up to 3 candidates. The form shows the found fields against its own, ticks the ones that differ, and "Take the ticked fields" changes the form only; Save keeps it.

## Not built yet

FTS5 search (search is a substring match in the webview), a trash view, merging duplicates by hand, scanned-PDF OCR, OpenAlex as a second source, EndNote XML / `.nbib` / Web of Science import, a "model reads page 1" option for PDFs without DOI, dragging a PDF onto the window, a tab in the tab bar (it is a window), L4 (Copilot library tools and reference checking). The PDF rules and Crossref were not run against real PDFs or the real service (the network call is exercised only by URL and parsing tests); the window was run in a browser against a fake bridge, not in the real app.
