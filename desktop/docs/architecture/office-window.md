# Office window

The Virtual Office of one Code tab as its own OS window: a fuel-pump meter of what the company spent, the pull-request funnel, a card per office, and per office a page of its members' read-only conversations; the roster is edited in a drawer. Kernel side: `omp/docs/virtual-office.md` ("For GUI hosts").

## Switching it on (per tab)

Virtual Office is a tab's own switch, not a profile setting. omp starts every RPC session with it off; `set_office_enabled { enabled }` turns it on for that session only (the `office` tool and the general manager's brief appear without a restart).

- The tab entry in the sidebar's right-click menu: "Turn Virtual Office on/off for this tab" (Code tabs with a folder).
- The sidebar nav's **Office** item (Code mode, active tab has a folder): its label opens the window, or turns Office on and opens it when it is off; its inline switch (`role="switch"`) turns it on or off without opening anything.
- The Office window itself: a header switch, and when Office is off a page explaining it with **Enable Office**. The window does not tell omp; it emits `office://set-enabled { sessionId, enabled }` to the main window (`emitTo("main", …)`), whose listener (main-window boot block of `live.js` only) calls `setOfficeEnabled`, so the flag is stored like any other switch. The switch shows a pending state until omp's `office_state` carries the new value; it lapses after 10 s (the send fails while the tab's omp is down).
- `live.js` `setOfficeEnabled(sessionId, on)` stores `office` on the tab entry and sends the command; `_spawnSession` sends it again whenever any tab's omp (re)starts — background-restored tabs and profile/approval-mode respawns included — since omp forgets it.
- `open-tabs.json` keeps it per tab (`OpenTab.office`, `project-nav.js` `tabLayout`/`restoreSpec`), so a relaunch restores it. New tabs start with it off.

## Opening

`OMP_BRIDGE.openOffice(sessionId)` in `live.js` passes the tab's `profile` alongside `sessionId` to `OMP_OPEN_PANEL("office", { project: sessionId, profile })`: a panel window (`panel_window.rs` kind `office`, label `panel-office-<hex of the session id>`), so one window per tab and a repeat open focuses it. `panel-root` passes `profile` to `OfficeWindow`, which fetches the profile's models via the Tauri command `models_catalog` (the same catalog Settings → Subagent Custom model uses; no longer `model_usage_list` and not a hardcoded list). Shortcut `desktop.office.open`, Cmd+Shift+O / Ctrl+Shift+O (`app/keymap.js`): opens the active tab's window; on a Code tab without Office the nav item reads "Virtual Office is off — click to turn on" for a moment, and a click turns it on and opens the window. On a tab that cannot have Office it does nothing.

Unlike the linked panels (history, plan, git) it does not go through `panel-link.js`: it needs nothing of the main window and keeps working while the main window is hidden or shows another tab.

## Data (`app/office-live.js`)

The window listens to `agent://line/<sid>` itself (Rust broadcasts every line to every window) and reads only two kinds of line:

- `office_state` frames: the whole `OfficeState`, pushed by omp on every change (roster, pull request, memory, a desk claimed or released), coalesced to one per 150 ms. Booked usage is not such a change.
- `office_usage` frames: `{ checkout, member, day, amount }`, what one member spent since the previous frame, coalesced to one per member and day every 250 ms. The window bills it locally (`book` in `app/office-meter.js`) into `state.usage`, so a working member never makes omp re-read the company (git, pull requests, memory, ledger) or push the whole state; a frame for another checkout is ignored. omp sends queued deltas before reading a state, and the next `office_state` replaces the local totals with the ledger's.
- responses to its own requests, whose ids start with `office:`. The main window resolves responses by its own ids and drops replies to ids it never sent, so neither window takes the other's.

It requests `get_office_state` on open and on window focus (another process, such as `npi office merge` in a shell, is not pushed). Just after local midnight it starts "today" over itself (`rollDay`) without asking omp. `office_command` runs the owner's actions; `get_subagent_messages { sessionFile, fromByte }` reads a member's conversation. omp lets a host read the transcript of any run in the company's ledger, so past runs no longer in the live registry still open.

`agent://exit/<sid>` or an unanswered request makes the window **offline**: a banner, the last figures frozen, a Retry button. Before the first state the window shows an outline of the meter, funnel and cards; with Office off it shows the off page instead of figures. The header pill reads General manager online, Office off (omp answers, Office is off) or Offline.

## What is shown (`app/office-model.js`, `app/office-meter.js`)

Everything comes from `OfficeState`; nothing is inferred from other frames.

- **Meter**: cost (USD and JPY), tokens (input + output + cache writes, not cache reads) and the unit price per million tokens, for **Today** (the local day omp names in `usage.day`, from 00:00) or **All time**; the choice is a per-window `localStorage` key. Between two readings the dial rolls toward the new one over 1.8 s (`aim`/`shown`); a lower reading (new day, other range) shows at once. The lamp pulses while any member works. Under it, a bar of who spent the range: the general manager (`general-manager` in `usage.members`) and each office.
- **Funnel**: members at a desk → worker PRs in review → office PRs in review → **waiting for you** (highlighted) → merged.
- **Cards**: office number and phase (`activity[].phase`), members leader first with capability, model (or "capability tier" when none is set) and effort when set, status (working = at a desk, failed = their last run failed) and their spend; office subtotal; the leader's current task; the approved office PR with Merge / Send back; open PRs; lessons kept (opens the memory dialog).
- **Office page** (click a card; Esc or Back returns): a column per member (min 320 px, the page scrolls sideways past that). Each column lists the member's runs from `state.runs`: who assigned it, when, the task, the conversation (assistant text and tool calls, failed tool results), and the outcome with the PR it left. The latest run is open; earlier ones open on demand. An open run in progress is re-read every 1.2 s from its last byte; a finished one once. The column follows the bottom until the reader scrolls up ("Back to latest"). No input, no stop button.

## Owner actions

All are `/office` commands through `office_command`, so the rules are omp's and its refusal text is shown as it is. There are no kernel changes: the commands use the CLI's `/office` grammar (`omp/packages/coding-agent/src/office/commands.ts`).

- Merge an approved office PR (confirmation with the PR's description), send one back (a reason is required).
- Roster drawer: the owner's editor for members' model, thinking effort, capability, and roles, as well as each office's worker count and company scale:
  - Member row: model select (first entry represents the capability tier and sends `set <name> model default`; otherwise selecting a catalog model sends `set <name> model <selector>`; a roster model missing from the profile's catalog is still shown as such; if `models_catalog` fails, the drawer shows "Models unavailable: <error>" and only the capability tier and the member's current model are offered), thinking effort select ("default effort" sends `set <name> thinking default`; levels are the selected model's supported levels from the catalog with `off` first and `auto` omitted, or the standard `off`…`max` set when running on the capability tier or the model is unknown; an unsupported current level stays visible and marked; pure helper `effortChoices(catalog, model)` in `app/office-model.js` computes these choices, tested in `tests/test-office-model.mjs`), capability select (sends `set <name> capability common|professional|expert`), Make leader (sends `set <name> role leader`), and confirmed Remove (sends `remove <name>`).
  - Per-office controls + company scale: worker-count stepper (sends `size <office> <n>`, bounded 3–6), Add a worker (sends `add <office>`), and company scale (size segment sends `scale team|studio|enterprise`).
  - Command lifecycle: exactly one roster command runs at a time; all roster controls are disabled while a command is running, and the issuing row or office is marked pending. If a command is refused or malformed, its own message (the first paragraph of the result text, without the usage appendix) is shown inline under the row or office that issued it. Controls always display the roster from the latest `office_state` without optimistic updates: a refused change snaps back to the state pushed by omp. The roster is global, shared by every tab and the CLI.
- Memory dialog: lessons and merged work; Forget everything (confirmed).

How omp dispatches a member (`omp/docs/virtual-office.md`): on the member's model and effort; with no model, on the capability's roster tier (expert → flagship, professional → high, common → standard) in the Auto subagent mode, else the Custom subagent model, else the general manager's model; an effort the model lacks is fitted to the model. Dispatch never asks the subagent-model question and the `office` tool is not approval-gated: the roster is the owner's choice.

## Exchange rate (`fx_rate.rs`)

`fx_rate_usd_jpy` fetches the USD→JPY ECB reference rate from Frankfurter (`api.frankfurter.dev`, no key; the request carries the currency pair only) in Rust, so the page's CSP stays closed. Cached in `<app cache>/fx.json` and reused for a day; a failed refresh returns the cached rate marked `stale` (shown amber), and without any rate JPY reads `¥—`. The window asks on open and every six hours.

## Subagent subscription claims (`agent/subscription.rs`)

omp keeps one `set_subagent_subscription` level per process, and the main window lowers it when it leaves a tab. To keep that from cutting another window off, windows no longer send the command themselves: `subagent_subscription_claim { sessionId, level }` records the calling window's need and sends omp the highest level any window claimed. An office window's claims are released when it is destroyed. The Office window itself reads conversations from transcripts and needs no subagent frames today; the claim table is what lets it (or any other window) ask for them without fighting the main window.

## Not built

Window position memory, an OS notification when an office PR waits for the owner, a cross-process view when two tabs run a general manager on the same repository (each window shows its own tab's pushes), and diffs of a PR (files and lines are in the kernel's `office diff`, not shown here).

Verified by `tests/test-office-model.mjs` (view model, meter, local billing of `office_usage` deltas and roster `effortChoices`), `test-project-nav.mjs` (the tab flag in the layout), Rust tests in `fx_rate.rs`, `agent/subscription.rs` and `open_tabs.rs`, and the frontend build. Run in a `tauri dev` build under a temporary HOME with the omp 2.0.10 kernel (frames traced in Rust, page text read back): a restored tab with Office sends `set_office_enabled` at spawn; the sidebar item opens the window, which asks `get_office_state` and renders the company; the window's switch turns it off through the main window and the `office_state` that follows shows the off page, Enable Office turns it back on; the sidebar switch does the same; `open-tabs.json` keeps the flag and a relaunch restores it off (nothing sent) or on; the shortcut's hint and its click work. Not seen: screenshots, and dispatch on the new kernel (Bun unavailable to build omp). The roster drawer was run the same way with the installed `npi/2.0.10` dispatch kernel (2026-11-10): picking a catalog model sent `set <name> model anthropic/claude-sonnet-4-5` and the effort list became that model's levels (off, minimal–xhigh); effort `high`, capability `expert` and the worker-count stepper (`size 1 4`, then `size 1 3`) each came back in the pushed `office_state` and in `npi office status`; an injected invalid effort (`thinking bogus`) and capability (`capability boss`) showed omp's first line ("Expected a thinking level: off, minimal, low, medium, high, xhigh, max." / "Expected a capability: common, professional, expert.") under the member's row and the selects stayed on the roster's values; choosing capability tier sent `set <name> model default`.
