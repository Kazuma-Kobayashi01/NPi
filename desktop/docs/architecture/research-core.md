# Research Core: the paper index and full-text search

Code: `src-tauri/src/librarian/research.rs` (index, search, quote check, tests in `research_tests.rs`), `librarian/cjk.rs` (the Chinese-aware split), `pdf_text.rs` `pages` (per-page extraction), `librarian/store.rs` (schema v5), `copilot.rs` (`research_search`, `research_check_quote`), `librarian/commands.rs` (`research_index`, `research_index_status`). Plan: `docs/PLAN_3.0.md` sections 4, 7 and 8.

The index lives in the project's `.npi-academic/research.db`, next to the library (`librarian.md`). It reads files and writes nothing but that database: the originals in `papers/`, `notes/`, `digests/` and `drafts/` stay as they are (a test compares every file outside `.npi-academic/` byte for byte and by modification time before and after).

## Schema v5

`PRAGMA user_version` goes from 4 to 5. The existing backup rule applies: a database of version 1 to 4 is copied to `research.db.bak-v<old>` before it is migrated; a newer version is refused.

|Table|Holds|
|---|---|
|`papers`|`path` (project-relative, `/`), `kind` (`pdf`/`text`), `file_hash` (SHA-256), `file_size`, `file_mtime`, `title`/`authors`/`year`/`doi` **as extracted** (the first-page rules of `pdf_meta.rs`; not verified), `parse_status` (`ok`, `partial`, `scanned`, `failed`, `missing`), `parse_note`, `page_count`, `ref_id`|
|`sections`|one row per heading, in order, with its level|
|`blocks`|`kind` (`heading`, `paragraph`, `table`, `formula`, `caption`), `text`, `page` (1-based; `NULL` for text files), `section_id`, `ordinal`|
|`anchors`|one per block: `page`, `exact_quote` (the block text), `quote_hash` (SHA-256 of its normalised form), `locator_quality` (`page` for a PDF, `block` for a text file; never a made-up box), `file_hash` it was made from, `stale`|
|`search_units`|one row per block and per library reference: `norm_title`, `norm_text` (the normalised columns below)|
|`search_fts`|FTS5, external content over `search_units`, `unicode61 remove_diacritics 2`|
|`research_meta`|`indexed_at` (an index exists), `norm_version`|

A paper links to a library reference (`papers.ref_id`) when an attachment of that reference has the same file hash and the reference is live; the link is refreshed at every indexing run.

## Indexing (`Library::index_project`)

- Files: PDFs under `papers/`; with `IndexOptions::text` also `.md`, `.txt` and `.tex` under `notes/`, `digests/` and `drafts/`. Symlinks and dot-files are skipped, at most 2000 files, text files up to 2 MB, PDFs up to 64 MB (`pdf_text::MAX_PDF_BYTES`; a larger file is recorded as `failed`, not partly read).
- A file whose size and modification time are those recorded is skipped without reading it; one that differs is hashed, and if the hash is the same only the time is updated. This keeps a search that refreshes the index cheap.
- A PDF is parsed once (`pdf_text::pages`: `pdf-inspector`, behind `catch_unwind`) into per-page Markdown. The index is the cache of this text; the older whole-document cache under `.npi-academic/cache/pdf/` is still used by `research_read` and the plain scan, and the index does not write to it. A page with no usable text layer (empty or garbled, `needs_ocr`) is not indexed: all pages like that make the paper `scanned`, some make it `partial` and the note lists the pages. Nothing is guessed, there is no OCR.
- Page Markdown becomes blocks: `#` headings (also sections), `|` tables, `$$` / `\[` formulas, `Figure N` / `Table N` / `图N` / `表N` captions, other paragraphs; a paragraph over 2000 characters is cut at a sentence end or space.
- Re-indexing a changed file replaces its sections, blocks and search units in one transaction. Its old anchors are **kept** with `stale = 1` and `block_id = NULL`, so a citation that names an anchor id can say "the text changed" instead of dangling. Stale anchors are not deleted automatically. A file that disappeared is marked `missing`, its blocks are dropped and its anchors marked stale.
- `max_new_pdfs` limits the PDFs parsed in one call (the Copilot's search uses 4, as the plain scan did); the report counts the rest as `waiting`.

Blocks and anchors are the user's work trail (citations point at anchor ids), not a rebuildable cache; only `search_units` and `search_fts` can be rebuilt.

## Search

Normalisation (`cjk.rs`, version `norm_version`): Latin and digit words are lower-cased tokens; every run of Han, kana or Hangul characters is indexed as each character (unigram) plus each pair of neighbours (bigram). This is why 文献 is found where `unicode61` (one token per run) and `trigram` (needs three characters) find nothing.

A query goes through the same split and is built **only from quoted tokens**, never passed on as FTS syntax (`"`, `*`, `NEAR`, `AND`, `col:` typed by a caller are plain punctuation or words):

- a CJK run of two or more characters becomes the AND of its bigrams (文献检索 needs 文献, 献检 and 检索); one character is its unigram;
- a Latin word is a **prefix** match (`ab` finds `abc` and `about`, not `cab`: matching inside words is not supported);
- whitespace separated parts are alternatives (OR), ranked by bm25 with the title/heading column weighted 4:1; pieces of one mixed word (`BERT模型`) must all match.

Because bigrams are not required to be adjacent in the right order, 文献检索 also matches a text holding 文献 and 献检 and 检索 separately; the snippet shows the real text. The excerpt is cut from the original block text around the first query term (offsets of the normalised column are never used).

`Library::search_blocks(query, folder, limit)` returns `Hit { paper_id, path, page, section, block_id, anchor_id, source_level, score, snippet }`. `source_level` is `original` (papers), `digest`, `note` or `draft`. Project scope: the database is the project's, every hit is joined to a `papers` row of it, and the optional `folder` prefix (for example `papers`) filters in SQL before the limit. `anchor_id` is the live anchor of the block (a stale one is never returned).

Library references (title; authors, year, venue, publisher, DOI, abstract, citekey, tags, notes) are search units too. `Library::search_library(entries, query, limit)` brings them up to date and returns reference ids, or `None` when the project has no index yet; `library_search` (Copilot) uses it and falls back to the substring scan of `librarian::search` when there is no index or the index finds nothing. The Librarian window's own filter is unchanged.

Rebuild and check: `Library::rebuild_search` re-derives every search unit from the blocks and the library and runs FTS5 `rebuild`; it runs by itself when `norm_version` differs from the code's. `Library::check_search` runs FTS5 `integrity-check` and compares counts (blocks without units, units without blocks, references without units). The units are kept in step with the FTS table by triggers (`search_units_ai/ad/au`), so a unit insert, update or delete never needs a manual FTS call.

## Quote check (`Library::check_quote`, tool `research_check_quote`)

Given a path, a page (required for a PDF) and a quote, the answer is deterministic and only about text:

- `Matched { block_id, anchor_id, page }`: the quote is in the text of that page after both are normalised (`cjk::quote_form`: lower case, one space between words, no space beside CJK, ligatures and typographic quotes/dashes plain, a hyphen at a line break joined). The blocks of the page are joined in reading order, so a quote may span blocks of one page but not pages.
- `NotFound`; `Stale` (the file's size/time/hash no longer match the indexed ones, or the file is gone: re-index first); `Unreadable { status }` (`scanned`, `failed`).
- A file never indexed is indexed first (that file only). The path must be a plain relative path under `papers/`, `notes/`, `digests/` or `drafts/` with no symlink on the way.

It does not judge whether a quote supports a claim, and "matched" means "in the indexed text of the file", not that the paper is real or its DOI verified.

## Copilot and commands

- `research_search`: when the project has an index it first indexes new and changed files (4 PDFs at most), then answers from the index with `path p.N [anchor id]: snippet`, plus notes for waiting and scanned PDFs. Without an index, or if the index finds nothing, the plain file scan answers as before. The chat hub's `academic_search` shares this code.
- `research_check_quote`: Copilot only (the hub neither lists nor answers it). Names stay underscore-only.
- `research_index(project, text?, rebuild?)` → `IndexReport { added, updated, unchanged, removed, waiting, scanned, failed }`; `research_index_status(project)` → `IndexStatus { indexed, papers, blocks, anchors, staleAnchors, scanned, partial, failed, staleFiles, unindexed, problems }`. Both take a project folder name resolved by `AcademicSpace::project_named`, never a path, and run through `off_ipc`. No UI uses them yet.

## Not built yet

Sections as a tree (flat per paper, in reading order), digests and `Digest`/`Artifact` tables, cursors, a filter language beyond the folder prefix, matching inside Latin words, a UI, automatic indexing on file changes (indexing runs on the commands, on a Copilot search once an index exists, and when a quote is checked), deleting old stale anchors, OCR, Docling/GROBID, vector search. Verified by unit tests only (including a real SQLite FTS5 and `pdf-inspector` on generated PDFs); not run against real papers, long documents or the real app, so parse quality on two-column layouts and tables is unmeasured.

## Reader, summaries, number checks and OCR

**Reader** (`librarian/reader.rs`, `design/librarian/reader-window.jsx`): read-only commands `research_papers`, `research_sections`, `research_page`, `research_anchor` and `research_search` (the webview names rows by id, never by path). The reader is its own window (panel kind `reader`, `panel_window.rs`); `?anchor=<id>` or the `reader://goto` event opens the page a citation's anchor lies on and highlights the block.

**Summaries and comparison tables** (`librarian/summaries.rs`, schema v6): `summaries` holds a paper's L2 summary and its sections' L1 summaries with the paper's `source_hash` at the time; a changed file shows them as *stale* and nothing rewrites them. A fact or a table cell that carries a quote is checked with `check_quote`: `matched`, else `unverified`; no quote is `model` (the model's reading, never shown as verified). A cell may be `not_reported` or `not_comparable` and then holds no value. Copilot tools: `research_summarize`, `research_summaries`, `research_compare`.

**Numbers and tables**: `research_check_number` compares a number as the paper spells it (thousand separators, full-width digits, the minus sign and a percent sign are ignored; it must stand alone; an optional unit right after it and a context word in the same block); `research_check_table` finds a Markdown table on the page and compares one cell. Both are deterministic and say nothing about meaning.

**OCR** (`ocr.rs`, macOS only): a PDF without a text layer, or pages of one, are drawn with CoreGraphics and read by the Vision framework on the machine (zh-Hans, zh-Hant, en-US; at most 80 pages per file). The paper gets parse status `ocr` (listed in `IndexReport.ocr` / `IndexStatus.ocr`), its anchors `locator_quality = ocr`, and the reader says the text may contain mistakes. Where OCR is unavailable or finds nothing the file stays `scanned`.
