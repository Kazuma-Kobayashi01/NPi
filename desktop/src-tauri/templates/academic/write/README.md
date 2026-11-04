# Writing

This folder is a Git repository of its own. The Write window runs its pipeline.

1. **Draft**: write in `markdown/`, one file per part (`00-abstract.md`, `01-introduction.md`, …). Files run in name order. Cite with `[@citekey]`.
2. **Compile**: an agent turns each changed Markdown file into LaTeX, in `latex/abstract.tex` and `latex/sections/`. `latex/references.bib` is written from the project's library, holding what the drafts cite.
3. **Link**: `template.tex` is filled in to make the complete `output/main.tex`, with `output/references.bib`.

Each step is committed, so a compile can be compared with the draft before it. Edit `markdown/` and `template.tex`; what Compile and Link write is replaced the next time they run.
