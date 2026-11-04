# Writing (the `write/` folder and the Write window)

The paper of an Academic project lives in `<project>/write/`, a Git repository of its own. New projects get the files from the scaffold (`writing::scaffold`); the repository is made by the Write window's "Set up write/" (`write_init`), which also fills in an existing project.

```
write/
  README.md  template.tex  .gitignore  .pipeline.json
  markdown/         00-abstract.md, 01-introduction.md, …  (the user's; run in name order)
  latex/            abstract.tex, sections/<stem>.tex, references.bib  (generated)
  output/           main.tex, references.bib  (generated)
```

## The pipeline (`src-tauri/src/writing/`)

- **Draft**: the user writes in Markdown, in VS Code or Typora, citing `[@citekey]`.
- **Compile** (`compile.rs`, `write_compile`): commits the drafts ("Draft"), then for each file whose text hash differs from `.pipeline.json` runs one tool-less agent (`copilot_worker::convert`: omp with `--no-tools`, only the text goes in) that answers with the LaTeX fragment. The answer loses a code fence and is refused if it is a whole document; a check records citations that did not come through and text that went missing as warnings on the file. Files one at a time; each finished one is recorded, so a failure or "Stop after this file" keeps the rest. LaTeX of a removed Markdown file is removed. `references.bib` is not the model's: it is exported from the library with exactly what the drafts cite, and cited keys the library lacks are reported. The result is committed ("Compile n of m").
- **Link** (`link.rs`, `write_link`): no model. Refused while a draft is not compiled. Fills `%%NPI-ABSTRACT%%`, `%%NPI-SECTIONS%%` and `%%NPI-BIBLIOGRAPHY%%` of `template.tex` into `output/main.tex`, copies `references.bib` beside it, commits ("Link"). `link` state is `none`, `stale` (template, LaTeX or bib changed since, hand edits included) or `fresh`.

Git mutation goes through `workspace::repo_init` / `commit_all` (the `git` binary, hooks off, NPi's identity only where the machine has none); reads through `git_browser::status`.

## The window (`design/write/write-window.jsx`, `app/write-pipeline.js`)

Panel kind `write`, opened from the Academic toolbar. It only manages: the three-stage progress bar, Compile, Link, Compile all, Stop, the Git line, the files with their state and warnings, adding a section, and opening `write/` or a file in VS Code, Typora or the Finder (`write_open`: `open -a`; only a plain file of `markdown/` or `output/`, no link out). A Compile runs on with the window closed: `write_status` carries its progress and the window polls once a second while there is one.

## Limits

One paper per project. Compiling needs the default or the tab's profile's model; 2 worker runs at once across the app, so a busy Copilot can refuse a file. No PDF is built: the output is LaTeX. Checked with a real model (`real_compile_and_link`, ignored by default) on two small files; the window ran in a browser against a fake bridge, so `open -a`, the toolbar button and a long Compile in the real app are unchecked.
