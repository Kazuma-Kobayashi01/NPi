# Third-party notices

NPi Desktop is released under the MIT License (see `LICENSE`). It is built on, and
ships with, the open-source software below. Their notices must travel with any
copy of NPi you redistribute.

## Derived from

### omp (oh-my-pi) — the agent core, shipped as the `npi` command line

MIT License · Copyright (c) 2025 Mario Zechner · https://github.com/can1357/oh-my-pi

NPi's command line is a modified build of omp (renamed, with a tiered settings
panel and a changed terminal layout). The full license is in `omp/LICENSE`, and
the licenses of the packages omp bundles are in the omp repository
(`packages/natives/THIRD-PARTY-NOTICES.txt` and each package's license).

### omp-desktop — the desktop shell this app was forked from

MIT License · Copyright (c) 2026 Miroslav Drbal · https://github.com/apoc/omp-desktop

Parts of the Tauri shell, the agent bridge and the interface code descend from
omp-desktop. The full license is in `desktop/LICENSE`.

> Permission is hereby granted, free of charge, to any person obtaining a copy of
> this software and associated documentation files (the "Software"), to deal in
> the Software without restriction, including without limitation the rights to
> use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies
> of the Software, and to permit persons to whom the Software is furnished to do
> so, subject to the conditions that the above copyright notice and this
> permission notice be included in all copies or substantial portions of the
> Software. THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND.

## Bundled in the app

| Component | License | Notes |
|---|---|---|
| [Tauri](https://tauri.app) 2 and its plugins | MIT OR Apache-2.0 | the native shell |
| Rust crates (637) | mostly MIT or Apache-2.0 | complete list: [`docs/licenses/rust-crates.md`](docs/licenses/rust-crates.md); a few are MPL-2.0 or Unicode-3.0, each under its own terms |
| [React](https://react.dev) 19 | MIT | `desktop/src/react*.js` |
| [@babel/standalone](https://babeljs.io) | MIT | used only when running from source (`babel.min.js`) |
| [marked](https://marked.js.org) | MIT | Markdown rendering |
| [highlight.js](https://highlightjs.org) | BSD-3-Clause | code highlighting |
| [xterm.js](https://xtermjs.org) and its fit addon | MIT | the terminal panel |
| [Bun](https://bun.sh) | MIT | the runtime embedded in the `npi` binary |

## Fonts

Bundled as subset woff2 files in `desktop/src/assets/fonts/`, under the
**SIL Open Font License, Version 1.1** (full text: [`licenses/OFL-1.1.txt`](licenses/OFL-1.1.txt)).

| Font | Copyright |
|---|---|
| Noto Sans, Noto Serif | Copyright 2022 The Noto Project Authors (https://github.com/notofonts/latin-greek-cyrillic) |
| Noto Sans SC | (c) 2014-2021 Adobe (http://www.adobe.com/), with Reserved Font Name 'Source' |
| Noto Serif SC | (c) 2017-2024 Adobe (http://www.adobe.com/) |

## Citation styles

The Librarian formats references with **hayagriva** (MIT OR Apache-2.0,
https://github.com/typst/hayagriva). Its bundled archive of CSL styles and
locales (APA, IEEE, Chicago, MLA, Nature, Vancouver, Harvard, GB/T 7714 and
others, taken from the Citation Style Language project,
https://github.com/citation-style-language/styles) is under the **Creative
Commons Attribution-ShareAlike 3.0** license
(https://creativecommons.org/licenses/by-sa/3.0/); each style keeps its
authors in its own metadata. Styles a user adds are theirs and stay outside the
app.

## Colour

**Catppuccin** (Latte, Frappé, Macchiato, Mocha) — MIT License ·
Copyright (c) 2021 Catppuccin · https://github.com/catppuccin/catppuccin ·
palette values in `desktop/src/design/themes.css`.

## Artwork

The engravings behind the conversation and in the README banner are works in
the public domain, taken from Wikimedia Commons; the list is in
`desktop/src/assets/wallpapers/CREDITS.md`. The files in this repository are
single-colour renderings made for NPi.

## Quotations

Quotations in the README are from works in the public domain.
