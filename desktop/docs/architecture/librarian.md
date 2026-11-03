# Librarian (reference library)

Plan and phases: `docs/PLAN_LIBRARIAN.md`. This file records what exists. Only L0 (the data layer) is built; there is no tab or UI yet.

## Storage

One library per Academic project, in `<project>/.npi-academic/research.db` (SQLite through `rusqlite`; `PRAGMA user_version` is the schema version, a newer one is refused). Table `refs`: the CSL-JSON record (`csl_json`) plus the columns it is compared and searched by (`title`, `title_norm`, `author`, `author_norm`, `year`, `doi`, `kind`), `citekey` (unique), `source` (`manual` / `ris` / `bibtex` / `csl-json`), timestamps and `deleted_at`. Deleting only sets `deleted_at`: the row stays, so a citekey is never reused. The database is the user's work, not a cache.

## Rules

- Citekey: `<first author><year><first title word>`, lower case, a letter suffix when taken. Editing a record keeps its citekey (drafts cite it).
- Duplicate: same DOI, or same normalised title + year + first author, among live rows. Import reports repeats (also within one file) and adds nothing for them; nothing is merged automatically.
- Formats: RIS, BibTeX/BibLaTeX and CSL-JSON in and out. BibTeX reading handles nesting, quotes, `@string` and `#`; accent commands keep the letter and lose the mark, other LaTeX is not interpreted. CSL-JSON export puts the citekey in `id`.
- The webview sends a project *name* (`AcademicSpace::project_named`), a reference id, CSL-JSON or import text. Never a path or SQL.

## Commands (`librarian/commands.rs`)

`library_list`, `library_add`, `library_update`, `library_remove`, `library_import` (`format`: `ris` | `bibtex` | `csl-json`; returns `{ added, duplicates }`), `library_export` (optional `ids`). All run through `off_ipc`.

## Not built yet

Groups, tags, attachments and notes (L1), the tab, search (FTS5), citation styles (L2), PDF import and online metadata (L3), Copilot library tools (L4).
