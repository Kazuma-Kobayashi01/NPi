# Computer Use

Code: `src/app/computer-use.js` (`OMP_COMPUTER_USE`), `src/app/use-computer-use.jsx`, `src/design/settings/rows-computer-use.jsx`, `src/app/computer-rpc.js`, `src-tauri/src/computer_use.rs` (+ `computer_use/macos.rs`), `src-tauri/src/computer_overlay.rs` (+ `state.rs`, `windows.rs`, `native.rs`), `src/computer-use/`. Kernel side: `omp/packages/coding-agent/src/tools/computer.ts` plus `tools/computer/{supervisor,worker,protocol}.ts` and the RPC wire definitions.

## Two owners, one feature

| Owner | Holds |
|---|---|
| omp profile config (`computer.enabled`) | whether this profile may use Computer Use at all |
| macOS TCC, charged to the app that launched omp | what the agent process may actually observe and control |
| the tab's omp process | whether this conversation has a live controller right now |

The profile switch is written through omp's own CLI (`computer_use_set` → `omp config set computer.enabled`), never to `localStorage`: a new tab reads it from omp. Enabling writes intent only; it never prompts. Only `computer_use_request_permission` prompts, and only from a user gesture. Queries never prompt, and the app never distinguishes "denied" from "never asked".

`ComputerUseConfig` is `{ supported, enabled, permissions: { screenRecording, accessibility, inputMonitoring }, responsibleApp }`. `responsibleApp` names the process macOS holds responsible — the packaged app, or, under `tauri dev`, a note that macOS may charge the launching terminal.

## Live state

`get_computer_use` returns `{ enabled, active, running, capabilities? }` and is the only truth for the UI's ready state. It never calls a model and never executes a tool. When the profile is enabled it asks the real supervisor for native `DesktopCapabilities`, so a granted TCC record that the agent process cannot use still reads as not ready.

- `set_computer_use` applies a session-scoped override and reconciles the same teardown path `/computer` uses; disabling closes the controller and cancels its work.
- `stop_computer_use` cancels computer work only (`cancelWork`: abort frame per run plus pending tool calls). It never aborts the agent turn, and it never disables the profile.
- Every change emits `computer_use_state`, which the reader forwards as `computer-use://state`; the frontend also refreshes on mount, window focus and a manual Recheck.
- A tab with no live process is skipped, not failed: `send_command` rejecting (the bridge holds no entry for a dead process) is reported as `absent`, and that conversation's next process reads `computer.enabled` itself. Its stored startup error — a joined stderr tail — is never turned into a Computer Use error.
- macOS records a TCC answer after the request returns, so a request leaves the row saying "Waiting for macOS" and re-reads twice; only Recheck decides "not granted".

`live.js` (`computerUse*` bridge methods) reaches background tabs through `app/computer-rpc.js`: listeners are attached before the command is written, correlate on that command's id, and unlisten afterwards. The main transcript listener only follows the active tab, so it cannot serve Settings.

## Operation indicators

The kernel emits `computer_activity` **before** each native mutation. A host that paints the indicator opts in with `set_computer_activity_ack { enabled: true }` once its process is ready; only then does the agent wait for the paint. A client that never sends that command keeps fire-and-forget activity, so a newer kernel can never freeze an older desktop's input.

```
worker #beginActivity → supervisor → subagentEventBus "computer:activity"
  → RPC computer_activity frame → reader (per session, tab-independent)
  → computer_overlay::observe → three local indicator webviews
  → computer-use://painted → reader-side ack → computer_activity_ack
     → supervisor → worker resolves → native mutation proceeds
```

Rules that keep it honest:

- Indicators are driven by real native calls, never by guessing at eval source text; nothing is emitted for read-only helpers.
- Geometry comes from the worker: global logical top-left bounds for the target window (the display for desktop-level actions) and the mapped cursor point. The cursor shows where the operation lands; the user's pointer never moves.
- The border and cursor windows are click-through, non-focusable and protected from screen capture, so they cannot swallow input or appear in the agent's own screenshots. Only the small stop panel accepts a click.
- An unpainted indicator fails the call within 5 s instead of acting invisibly; the overlay's own 3 s watchdog reports the failure first, so the agent gets a reason rather than a bare timeout. A paint failure reports `computer-use://error` and stops the operation.
- Overlay state is keyed by session and revision: an older finish cannot clear a newer operation, Stop cannot be resurrected by a late update, and the newest active operation owns the visible indicators.
- The reader clears a session's indicators when a `computer_use_state` frame reports it stopped or disabled, and on process exit; stopping a tab (`stop_session`), closing it and app exit all clear too.

## Limits

- macOS only. Other platforms report `supported: false`, accept disabling, and reject enabling, permission requests and settings links.
- The app is ad-hoc signed, so its designated requirement is a CDHash. **Every rebuild is a different app to TCC**: the System Settings switch keeps showing "on" from the previous build while the new binary is denied. `computer_use_reset_permission` runs `tccutil reset <service> <bundle id>` and the row offers it as "Reset & request"; a stable Developer ID signature is the only way to stop re-authorizing after each build.
- A conversation without eval tools (for example a restricted session) can be enabled but never becomes ready — Settings says so rather than guessing.
- macOS may require quitting and reopening the app after a permission change; the app never restarts itself.
- Turning the profile off does not revoke a grant; System Settings owns that.
