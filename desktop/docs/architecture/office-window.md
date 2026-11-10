# Office window

The Virtual Office of one Code tab as its own OS window: a fuel-pump meter of what the company spent, the pull-request funnel, a card per office, and per office a page of read-only member conversations. Kernel side: `omp/docs/virtual-office.md` ("For GUI hosts").

## Switching it on (per tab)

Virtual Office is a tab's own switch, not a profile setting. omp starts every RPC session with it off; `set_office_enabled { enabled }` turns it on for that session only (the `office` tool and the general manager's brief appear without a restart).

- The tab entry in the sidebar's right-click menu: "Turn Virtual Office on/off for this tab" (Code tabs with a folder).
- The sidebar nav's **Office** item (Code mode, active tab has a folder): turns it on and opens the window, or opens the window when it is already on. A green dot marks a tab with Office on.
- `live.js` `setOfficeEnabled(sessionId, on)` stores `office` on the tab entry and sends the command; `_initFetch` sends it again whenever the tab's omp (re)starts, since omp forgets it.
- `open-tabs.json` keeps it per tab (`OpenTab.office`, `project-nav.js` `tabLayout`/`restoreSpec`), so a relaunch restores it. New tabs start with it off.

## Opening

`OMP_BRIDGE.openOffice(sessionId)` → `OMP_OPEN_PANEL("office", { project: sessionId })`: a panel window (`panel_window.rs` kind `office`, label `panel-office-<hex of the session id>`), so one window per tab and a repeat open focuses it. Shortcut `desktop.office.open`, Cmd+Shift+O / Ctrl+Shift+O (`app/keymap.js`): opens the active tab's window, or briefly says on the nav item that this tab has no Office.

Unlike the linked panels (history, plan, git) it does not go through `panel-link.js`: it needs nothing of the main window and keeps working while the main window is hidden or shows another tab.

## Data (`app/office-live.js`)

The window listens to `agent://line/<sid>` itself (Rust broadcasts every line to every window) and reads only two kinds of line:

- `office_state` frames: the whole `OfficeState`, pushed by omp on every change (roster, pull request, memory, usage booked, a desk claimed or released), coalesced to one per 150 ms.
- responses to its own requests, whose ids start with `office:`. The main window resolves responses by its own ids and drops replies to ids it never sent, so neither window takes the other's.

It requests `get_office_state` on open, on window focus (another process, such as `npi office merge` in a shell, is not pushed) and just after local midnight. `office_command` runs the owner's actions; `get_subagent_messages { sessionFile, fromByte }` reads a member's conversation. omp lets a host read the transcript of any run in the company's ledger, so past runs no longer in the live registry still open.

`agent://exit/<sid>` or an unanswered request makes the window **offline**: a banner, the last figures frozen, a Retry button.

## What is shown (`app/office-model.js`, `app/office-meter.js`)

Everything comes from `OfficeState`; nothing is inferred from other frames.

- **Meter**: cost (USD and JPY), tokens (input + output + cache writes, not cache reads) and the unit price per million tokens, for **Today** (the local day omp names in `usage.day`, from 00:00) or **All time**; the choice is a per-window `localStorage` key. Between two readings the dial rolls toward the new one over 1.8 s (`aim`/`shown`); a lower reading (new day, other range) shows at once. The lamp pulses while any member works. Under it, a bar of who spent the range: the general manager (`general-manager` in `usage.members`) and each office.
- **Funnel**: members at a desk → worker PRs in review → office PRs in review → **waiting for you** (highlighted) → merged.
- **Cards**: office number and phase (`activity[].phase`), members leader first with capability, model, status (working = at a desk, failed = their last run failed) and their spend; office subtotal; the leader's current task; the approved office PR with Merge / Send back; open PRs; lessons kept (opens the memory dialog).
- **Office page** (click a card; Esc or Back returns): a column per member (min 320 px, the page scrolls sideways past that). Each column lists the member's runs from `state.runs`: who assigned it, when, the task, the conversation (assistant text and tool calls, failed tool results), and the outcome with the PR it left. The latest run is open; earlier ones open on demand. An open run in progress is re-read every 1.2 s from its last byte; a finished one once. The column follows the bottom until the reader scrolls up ("Back to latest"). No input, no stop button.

## Owner actions

All are `/office` commands through `office_command`, so the rules are omp's and its refusal text is shown as it is:

- Merge an approved office PR (confirmation with the PR's description), send one back (a reason is required).
- Roster drawer: company size, add a worker, capability, model (free text with the profile's recent models as suggestions; empty = default), make leader, remove (confirmed). The roster is global, shared by every tab and the CLI.
- Memory dialog: lessons and merged work; Forget everything (confirmed).

## Exchange rate (`fx_rate.rs`)

`fx_rate_usd_jpy` fetches the USD→JPY ECB reference rate from Frankfurter (`api.frankfurter.dev`, no key; the request carries the currency pair only) in Rust, so the page's CSP stays closed. Cached in `<app cache>/fx.json` and reused for a day; a failed refresh returns the cached rate marked `stale` (shown amber), and without any rate JPY reads `¥—`. The window asks on open and every six hours.

## Subagent subscription claims (`agent/subscription.rs`)

omp keeps one `set_subagent_subscription` level per process, and the main window lowers it when it leaves a tab. To keep that from cutting another window off, windows no longer send the command themselves: `subagent_subscription_claim { sessionId, level }` records the calling window's need and sends omp the highest level any window claimed. An office window's claims are released when it is destroyed. The Office window itself reads conversations from transcripts and needs no subagent frames today; the claim table is what lets it (or any other window) ask for them without fighting the main window.

## Not built

Window position memory, an OS notification when an office PR waits for the owner, a cross-process view when two tabs run a general manager on the same repository (each window shows its own tab's pushes), and diffs of a PR (files and lines are in the kernel's `office diff`, not shown here).

Verified by `tests/test-office-model.mjs` (view model and meter), `test-project-nav.mjs` (the tab flag in the layout), Rust tests in `fx_rate.rs`, `agent/subscription.rs` and `open_tabs.rs`, the frontend build, and the window rendered in a browser against a fake shell. Not yet run inside the built app.
