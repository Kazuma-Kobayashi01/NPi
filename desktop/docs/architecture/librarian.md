# Librarian (reference library)

Plan and phases: `docs/PLAN_LIBRARIAN.md`. This file records what exists. L0 (data layer) and L1 (the window) are built; L2 and later are not.

## Storage

One library per Academic project, in `<project>/.npi-academic/research.db` (SQLite through `rusqlite`; `PRAGMA user_version` is the schema version, a newer one is refused). Schema 2 (a version-1 file is copied to `research.db.bak-v1` before it is migrated). Table `refs`: the CSL-JSON record (`csl_json`) plus the columns it is compared and searched by (`title`, `title_norm`, `author`, `author_norm`, `year`, `doi`, `kind`), `citekey` (unique), `source` (`manual` / `ris` / `bibtex` / `csl-json`), `notes`, timestamps and `deleted_at`. `groups` (unique name, ignoring case) and `group_refs` hold the manual groups; `ref_tags` the tags (a tag is its text, unique per reference ignoring case). Deleting only sets `deleted_at`: the row stays, so a citekey is never reused. The database is the user's work, not a cache.

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

## Not built yet

Attachments (PDF files per reference), FTS5 search, citation styles and formatted bibliographies (L2), PDF import and online metadata (L3), Copilot library tools (L4), merging duplicates by hand, a trash view, a tab in the tab bar (it is a window). Never run in the real app: the commands were exercised by Rust tests, the window in a browser against a fake bridge.
