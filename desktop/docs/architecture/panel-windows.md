# Secondary windows

The Librarian, reader, Write, usage statistics, history, approvals, Todo and plan, and the git browser open as their own OS windows (`src-tauri/src/panel_window.rs`): resizable, zoomable, closed with Cmd+W. Settings, the command palette, the prompt history and the file preview stay inside the main window.

## Opening

`window.OMP_OPEN_PANEL(kind, { project, profile, title, anchor, path })` (`app/panel-mode.js`) calls `open_panel_window`, which opens `index.html?panel=<kind>&…` in a window labelled `panel-<kind>` (or `panel-<kind>-<hex of project>` for a project's Librarian, reader or Write), or brings the open one forward. Only the kinds in `KINDS` open; `navigation_guard.rs` lets only a `panel-` window load a `?panel=` page. Without a desktop shell `OMP_OPEN_PANEL` resolves false and `app-live.jsx` shows the old in-app overlay instead.

A panel page sets `window.OMP_PANEL`, keeps `live.js` inert (no tabs, no listeners, no close guard) and renders `design/panel-root.jsx`. The head of a panel is a Tauri drag region; the native title bar carries the traffic lights (`ui/window-chrome.js`, `layout/window-chrome.css`, which also makes each overlay fill its window).

## Panels that only call commands

`librarian`, `reader`, `write` and `stats` need nothing of the main window: they call `library_*`, `research_*`, `write_*` and stats commands through their own `OMP_BRIDGE`, with the project and profile from the URL.

## Panels that show the main window's tab (`app/panel-link.js`)

`history`, `rules`, `plan` and `git` show the tab the main window has active and change it, and only the main window's `live.js` knows that tab. They talk to it through app events (`core:event` of the `default` capability, which covers `main` and `panel-*`):

- **Snapshots.** `app-live.jsx` publishes a JSON snapshot per kind while that kind's window is open: `history` `{ activeCwd }`, `rules` `{ activeProject, sessions, approvalModes }`, `plan` `{ kanban, planMeta }`, `git` `{ project, branch, edits, path, seq }`. `publish` skips a snapshot equal to the last one sent. A panel subscribes on mount and says `panel-hello`; the main window answers with the latest snapshot and adds the kind to its open set (`onPresence`), which is what makes it compute snapshots at all (the finished-edit count of the git browser walks the transcript). `panel-bye` on unload takes it out.
- **Calls.** The panel's bridge is `OMP_PANEL_LINK.proxy(kind)`: each method of `CALLS[kind]` emits `panel-call` with an id and waits for `panel-reply:<id>`. The main window runs only names listed for that kind, first as an action it `serve`s (`history.resume`, `plan.abort`, `git.sendToAgent`), else as the method of its own `OMP_BRIDGE`, so `listApprovalRules`, `setApprovalModeDefault` (which restarts tabs), `gitBrowserStatus`, `worktreeRemove` and the rest act on the active tab exactly as from the overlay. Any other name is refused.
- A call that ends the panel (resuming a session, sending review comments to the agent) is answered before the panel window closes, and the main window takes focus.
- The git window remounts per project and per open (`seq`), so an edit card's path is selected each time; the other three follow the active tab as it changes.

## The Office window

Kind `office` opens per tab and reads that tab's omp itself, not through `panel-link.js`: see `office-window.md`.

## Not built

Settings as a window: it uses most of the main window's state and handlers (profiles, keymap, widgets, terminal, panes, logins, Computer Use), so it stays an in-window app tab. No panel restores its window position.

Verified by `tests/test-panel-link.mjs` (two vm windows on a fake event bus) and the frontend build; the windows themselves were not run in the real app here.
