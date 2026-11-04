---
name: librarian
description: Work with the project's reference library - find references, cite only what it holds, check a draft's citations, and propose tags or groups.
---

# Librarian

The project has a reference library (the Librarian window). Use it instead of memory for anything about which papers exist.

1. **Find**: `library_search` (or `academic_library_search` from the main chat) by title, author, year, venue, DOI, tag or note. Then `library_get <citekey>` for the full record, the user's notes and the formatted reference. Read an attached PDF with `research_read` on its `papers/` path.
2. **Cite**: write `[@citekey]` in drafts; the user turns it into formatted citations with "Cite a draft…". Cite only citekeys the library returned. If the library lacks a reference, say it is not in the library; do not invent its details.
3. **Check**: `library_check` on a draft (text or `drafts/…` path) reports `[@key]`, Author (Year) and DOI citations that match, differ in year, or are not in the library. Report the findings as they are; "not in the library" means unverified, not necessarily wrong.
4. **Organise**: `library_suggest` proposes a tag, group or note for a reference with a one-line reason. It changes nothing until the user accepts it in the Librarian window. Do not propose the same thing twice.

## Built-in tools and how to call them

The library tools of this project are connected for you under plain names (no MCP, no `project` argument, no approval prompt). Do not search for a server or ask the user to connect one.

- `library_search {"query": "graph attention"}` → citekeys, authors, years. `library_get {"citekey": "smith2020graph"}` → the full record. Copy citekeys from a result; never write one from memory.
- `library_add {"doi": "10.1038/nature14539"}` adds a paper from Crossref (or `{"csl": {...}}` for one you have read yourself); one already in the library is reported, not added twice.
- `library_align {"citekey": "smith2020graph"}` previews how Crossref differs; add `"apply": true` to fill the blank fields (`"overwrite": true` to replace differing ones). `{"all": true}` covers the first 20 references that lack a DOI or venue. `mismatch` means Crossref's record is another paper (often a wrong DOI): check it with `library_lookup`, do not force it.
- `library_edit {"citekey": "...", "set": {"volume": "7"}, "tags_add": ["gnn"], "note_append": "..."}` changes one reference. Nothing is deleted; the earlier version goes to `.npi-academic/agent-edits.jsonl`.
- When a call returns an error, read it: it names what to give or what to call next. Do not repeat the same call.
