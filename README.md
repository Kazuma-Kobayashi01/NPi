<p align="center">
  <img src="docs/assets/banner.png" alt="NPi — a humanist desktop for the omp coding agent" width="100%">
</p>

<h1 align="center">NPi Desktop</h1>

<p align="center"><sub>NeoPi Desktop · 1.0.1 · for macOS on Apple silicon</sub></p>

<p align="center"><em>Tools for thinking should feel like a study, not a cockpit.</em></p>

<p align="center">
  <img src="https://img.shields.io/badge/version-1.0.1-C8A96A?style=flat-square&labelColor=1f1f1d" alt="version 1.0.1">
  <img src="https://img.shields.io/badge/macOS-Apple%20silicon-6E7F99?style=flat-square&labelColor=1f1f1d&logo=apple&logoColor=E9E4D6" alt="macOS Apple silicon">
  <img src="https://img.shields.io/badge/Tauri-2-C8A96A?style=flat-square&labelColor=1f1f1d&logo=tauri&logoColor=E9E4D6" alt="Tauri 2">
  <img src="https://img.shields.io/badge/Rust-stable-8E8E88?style=flat-square&labelColor=1f1f1d&logo=rust&logoColor=E9E4D6" alt="Rust stable">
  <img src="https://img.shields.io/badge/React-19-7FA3D6?style=flat-square&labelColor=1f1f1d&logo=react&logoColor=E9E4D6" alt="React 19">
  <img src="https://img.shields.io/badge/TypeScript-5-6E7F99?style=flat-square&labelColor=1f1f1d&logo=typescript&logoColor=E9E4D6" alt="TypeScript 5">
  <img src="https://img.shields.io/badge/Bun-1.4-D8CBB0?style=flat-square&labelColor=1f1f1d&logo=bun&logoColor=E9E4D6" alt="Bun 1.4">
  <img src="https://img.shields.io/badge/omp-18.14%20core-A99BCF?style=flat-square&labelColor=1f1f1d" alt="omp 18.14 core">
  <img src="https://img.shields.io/badge/themes-Graphite%20·%20Ivory%20·%20Catppuccin-8FB58A?style=flat-square&labelColor=1f1f1d" alt="themes">
  <img src="https://img.shields.io/badge/fonts-Noto%20Sans%20·%20Serif-8E8E88?style=flat-square&labelColor=1f1f1d" alt="fonts">
  <img src="https://img.shields.io/badge/license-MIT-C8A96A?style=flat-square&labelColor=1f1f1d" alt="license MIT">
</p>

<p align="center">· &nbsp; · &nbsp; ·</p>

NPi is a quiet place to work with an AI agent. It is a desktop app and a
command line, built on the [omp](https://github.com/can1357/oh-my-pi) coding
agent, and shaped by one idea: **tools for thinking should feel like a study,
not a cockpit.** Few controls, generous margins, a serif that is pleasant to
read, and nothing that asks for attention it has not earned.

The proportions come from the Renaissance — the portico of the logo, the
Vitruvian Man above, the six engravings that sit faintly behind the
conversation. The craft comes from restraint.

<p align="center">· · ·</p>

## Two rooms

**Chat** is for everything that is not code: writing, research, learning,
planning, a question at midnight. Conversations are listed by day, in a space of
their own, with a memory that stays on your computer and instructions you can
write in your own words.

**Code** is for projects. Open a folder and the agent reads, edits and runs
inside it, with the approvals you choose. The side panel shows where you are
(folder, branch), how full the context is, and how much of your plan you have
used.

Switching between the two always lands on that room's start page, greeted by the
hour — *Night owl! Kazuma* — or by a line of its own.

<p align="center">· · ·</p>

## What is in it

- **A native window.** A translucent sidebar, one top bar, a centred reading
  column, a prompt box that is a single line until you need more.
- **Themes** — Graphite and Ivory, and the four Catppuccin flavours (Latte,
  Frappé, Macchiato, Mocha). Noto Sans and Noto Serif, bundled; Chinese included.
- **Five levels of detail**, from the barest window to the fullest, and three
  widths for the conversation.
- **Models by vendor.** A small picker beside the prompt lists the models you
  ticked; the rest are one step away. Sign-in is omp's own OAuth.
- **Honest numbers.** Plan limits as the share *used*, spend and tokens for the
  conversation, the context window split into its parts.
- **Terminal, browser and subagents** as columns that open at the right.
- **A right-click menu** in the sidebar: pin, mark unread, rename, archive,
  delete. An archive you can manage in Settings.
- **Hover tips** that name a button and its shortcut.

<p align="center">· · ·</p>

## The stack

| Layer | Made with |
|---|---|
| Shell | [Tauri 2](https://tauri.app) — a native macOS window with a translucent sidebar |
| Back end | Rust: the agent bridge, sessions, terminal, browser and settings stores |
| Front end | React 19, plain scripts with no bundler; Babel only while developing |
| Agent | omp 18.14 (as `npi`), run over its RPC protocol, one process per tab |
| Command line | TypeScript on Bun, compiled to one binary |
| Type | Noto Sans / Noto Serif, subset and bundled |
| Look | Graphite · Ivory · Catppuccin, and six public-domain engravings |

<p align="center">· · ·</p>

## Install

Open `NPi-Desktop-1.0.1-arm64.dmg` and drag NPi Desktop to Applications. The app
is not notarized: the first time, right-click it and choose **Open**.

The command line travels inside the app. To use it in a terminal:

```bash
ln -sf "/Applications/NPi Desktop.app/Contents/MacOS/npi" ~/.local/bin/npi
npi --version
```

<p align="center">· · ·</p>

## Where things live

| What | Where |
|---|---|
| Settings, sign-ins, conversations, caches | `~/.local/share/npi` |
| Your workspace, and the Chat folder | `~/Documents/NPi` |
| The Chat assistant's profile and instructions | the app's config folder |
| Project-local configuration | `.omp/` inside the project, as in omp |

NPi keeps to itself: its data is separate from a stock omp install, and there is
no remote access, no tracking, and no self-update that reaches for the network.
New versions arrive with new releases of the app.

<p align="center">· · ·</p>

## The command line

`npi` is omp, renamed and quieter. Its settings are tiered: the ones people
change are always listed, the rest appear under **Show More Options**, and the
long tail lives in **Advanced**. Nothing is removed — a hidden setting still
reads from the config file. The prompt sits on the bottom row from the first
frame.

<p align="center">· · ·</p>

## Build from source

```bash
# the app
cd desktop
npm install
npm test
npx tauri build --config src-tauri/tauri.dist.conf.json --bundles app,dmg

# the command line (an omp checkout on the `npi` branch, built with Bun)
cd ../omp
bun install && bun run build:native
cd packages/coding-agent && bun run build      # → dist/npi
cp dist/npi ../../../desktop/src-tauri/binaries/npi-aarch64-apple-darwin
```

The app and the command line share one version number: raise it in
`desktop/src-tauri/tauri.conf.json` (and `Cargo.toml`, `package.json`) and in
`NPI_VERSION` in `omp/packages/utils/src/dirs.ts` together.

```
desktop/    the Tauri app: React front end (no bundler), Rust back end
omp/        the command line (its own repository, branch npi; not part of this one)
branding/   the logo and icon sources
docs/       this page's banner
dist/       release builds (not tracked)
```

<p align="center">· · ·</p>

## Thanks

- **omp** by Mario Zechner and Stencil Labs, and **omp-desktop** by Miroslav
  Drbal, both MIT — NPi is built on their work. See `omp/LICENSE` and
  `desktop/LICENSE`.
- **Noto** fonts (SIL Open Font License), **Catppuccin** (MIT).
- The artworks behind the conversation are in the public domain, from Wikimedia
  Commons; see `desktop/src/assets/wallpapers/CREDITS.md`. The banner uses
  Leonardo's *Vitruvian Man*.

---

*中文简介*：NPi 是基于 omp 的桌面应用和命令行，面向 macOS（Apple 芯片）。设计取向是人文主义与极简：Chat 用来日常对话，Code 用来做项目；数据存放在 `~/.local/share/npi`，工作区在 `~/Documents/NPi`，与 omp 完全隔离，也没有远程访问和联网自更新。
