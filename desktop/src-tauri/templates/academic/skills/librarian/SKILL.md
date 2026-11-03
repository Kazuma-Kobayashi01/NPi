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
