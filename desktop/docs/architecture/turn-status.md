# Turn outcome and background work

`src/app/turn-status.js` (`window.OMP_TURN_STATUS`, regression `test-turn-status.mjs`) decides, `live.js` holds the state. Everything it reads is omp ≥ 18.3.1, below the floor; the one command it adds is `abort_retry` (in `ALLOWED_COMMAND_TYPES`).

- **Run state** (`runStateOf`): failed > waiting-user > retrying > running > background > idle, shown by `TabRunDot` (tab bar, group chip, sidebar; `groupRunState` ranks the same way). `retrying` = a live retry row (below); the updater counts it busy. `background` = the agent yielded but omp reports background work (an async bash/task/eval job) that will wake it: `state.asyncPending`, per tab (the four snapshot places), set by every `get_state.hasPendingAsyncWork` (each `agent_end` asks for one, and so does an activation), cleared by `session_settled` and by a dead process (`_endDeadProcessState`). The composer footer notes it, and the updater counts it as busy (a restart kills the job). Like every run state, a background tab's dot stays at its snapshot until the tab is activated: only the active tab's frames are processed.
- **Failed requests**: `message_end` keeps a failed message's failure hidden (`pendingFailure`) because omp may still retry that attempt (`agent_end` with `yielded: false`). A yield (`isYield`) whose last assistant message has `stopReason: "error"` shows it on that bubble (`_showFailure`, `chat/assistant-bubble.jsx`). `prompt_result {agentInvoked: true, status: "error"}` follows its `agent_end` and adds omp's cleaned error text and its `retryable` verdict (the "temporary" chip). Ordinary prompts carry no id, so `prompt_result` is matched to the last failure, not correlated. After a tab switch the adapter rebuilds the block from get_messages (`errorMessage`, `errorStatus`, minus omp's `raw-http-request=` lines), but only for a failure that ended its run (`finalFailures`): omp keeps some failed attempts it continued from, and marks recovered/superseded retries `retryRecovery`. `retryable` is never persisted; the merge carries it over from the live bubble its copy replaces. An output-less failure leaves `get_messages` whenever omp rebuilds its context — a resume, a compaction, a tool-output prune at `agent_end` (`buildSessionContext` drops `isEmptyErrorTurn` and `retryRecovery` messages); the merge then keeps its live bubble, so it is gone only where no live transcript exists (a relaunch, a session opened from history).
- **Automatic retries** (measured on omp 18.6.0 against a local provider answering 503): after a failed request omp emits `turn_end`, `auto_retry_start {attempt, maxAttempts, delayMs, errorMessage}`, drops the attempt from its context, sleeps `delayMs` (`retry.baseDelayMs` 500 doubling, capped at 8 s, up to 25 % shorter by jitter — `calculateRetryBackoffDelayMs`; rate limits use their own windows) and starts the next attempt with `agent_start`/`turn_start` — no `agent_end` in between. One attempt can itself take ~30 s while the provider layer retries the HTTP call underneath, invisibly. `retryStarted` keeps one live-only `kind: "retry"` row at the end of the transcript (omp's `attempt`/`maxAttempts` for the title, its own `failed` count — omp's counter restarts after a fallback model and survives an `abort` — `phase` `waiting` → `attempting` at the next `turn_start`, `since`, `delayMs`) and marks the failed bubble `retried`, plus `superseded` (not rendered, not in the minimap or its count) when it showed nothing. The row lives in `messages`, so it is per tab and snapshotted with them; `get_state` reports no retry, so a relaunch loses it. `retryFinished` removes the row and tags the reply `retries: {failed, outcome}` (`recovered` / `failed` / `stopped`, the chip in the reply header) at the `message_end` of the attempt that answered (the run may go on with more turns). A row still there at the run's final yield means no recovery: `retryYielded` uses `agent_end`'s last assistant only when it failed or was aborted (that list may leave out the final attempt, and its last assistant can then be an earlier reply that answered), else the run's last bubble; it shows a `superseded` final attempt again (a cancel between attempts ends on it), reveals its failure without waiting for a `prompt_result` (a run no prompt started gets none), and gives an attempt stopped in flight (`stopReason: "aborted"`, which ends the run with no `auto_retry_end`) the row's failure. `auto_retry_end` only records `ended` (hides the row, ends `retrying`): it precedes the final `agent_end` on a cancel or give-up and follows even that `agent_end` on success. The get_messages merge carries `retries` (and a stopped attempt's failure) onto the persisted copy (`_withLiveFields`). A dead process clears the row (`retryCleared`).
- **Which bubble is which** (all in `turn-status.js`, regression `test-turn-status.mjs`): a retried attempt is the newest bubble with a `pendingFailure` or `aborted` (`message_end` marks `stopReason: "aborted"`: omp also retries a stream that stalled or dropped without content). Only an answer by omp's own rule (`producedOutput`, its `assistantTurnProducedOutput`: actionable content, not an empty stop) ends the retries at its `message_end`; after an empty stop omp keeps retrying. The yield's fallback scan stops only at a user bubble omp accepted (it has a `ts`; a prompt refused during the wait or a local command has none), skips desktop notes (no `ts`), and before the row accepts only the `retried` attempt. The merge keeps a `retried` attempt hidden even when omp kept it in its context (a failed tool-call turn): `superseded`/`retried` survive and the copy's failure stays pending.
- **Stop** (`OMP_BRIDGE.stopRetry`, the row's button): `abort_retry` while the row waits — it only cancels the sleep, so the run ends on the last failure (`prompt_result` error, `retryable: true`) without `abort`'s other effects (goal mode, title generation, compaction, handoff). Sent while an attempt is in flight it is a no-op and omp keeps retrying (measured), so the `attempting` phase sends the id-less `abort`, as Esc does.
- **Job rows**: the `async-result` custom message (the job result that wakes the agent) adds one live-only `kind: "job"` row per job (`finishedJobsOf`) above the reply it triggers. No omp message stands behind a row, so the merge keeps it in place; a relaunch drops it.
- Not used: `prompt_result` for the auto-rename gates (`lastTurnOk` from `message_end` also covers runs no prompt started, such as a job's wake-up) and `get_state.isSettled`. The planned consumer of an id-correlated `prompt_result` plus `session_settled` is #18's on-close routine.

## Complete chat history

`live.js` keeps the full displayed transcript; the minimap does not impose a
169-entry history limit. Switching tabs and resuming a session requests
`get_messages { fullHistory: true }`: all persisted message entries on the
current branch, including those before context compaction. Sibling branches
are not mixed into the chat. This is display history, not a change to what
the model receives as context.

`mergeTranscript` restores missing older messages without duplicating matched
live bubbles or dropping their stable IDs, tool cards, or pending prompts.
`ChatView` allows scrolling to the first message and retains the reader's
position when new output arrives while they are reading history.

### Generative UI

`ui.present` custom messages become independent `kind: "ui"` cards, never
collapsible tool output. `app/generative-ui.js` mirrors the kernel's bounded,
data-only version-1 schema; `design/chat/generative-ui.jsx` renders React text,
native controls and the existing Settings primitives. Display mode supports
information components and non-blocking suggestions, with no submit button.
Interactive mode requires exactly one submit button and can include ranking,
project-relative path pickers, and a 2–6-step wizard. Invalid schemas or render
failures show a local text error without taking down the transcript.

Draft changes stay local (bounded per-identity cache across tab remounts).
Lifecycle status is `waiting_input` (with an orthogonal `rendered: boolean`
milestone acknowledged via idempotent `render_ui` after React mounts) or
terminal `submitted` / `cancelled` / `expired`. `runStateOf` classifies
unfinished forms (`waiting_input` or `submitted` with `continuation: "pending"`)
as `waiting-user`, suppressing automatic `/rename` and PR auto-send.
Full-history `ui.submit` and `ui.state` receipts fold into the original card;
if continuation admission was interrupted after durable submission, the
read-only card exposes **Resume task** to retry continuation without
re-submitting new values.

Display updates arrive as `ui.update` and replace only the original card's schema
at its existing transcript position. Revision numbers prevent stale replay from
overwriting newer content; an update without a loaded original remains visible
where it arrived. Display identity is `[session_id, ui_id]`, so update tool-call
identities do not reset tabs, collapsibles or locally chosen suggestions.
Interactive drafts remain bound to the full session/UI/task/message identity.
Display renderers have independent React component boundaries.

Suggestion chips/cards send `prompt ?? label` through `sendUiSuggestion`: an idle
tab uses the normal user-prompt path; a running tab uses the existing follow-up
queue and queue strip. Selection is kept only in the bounded local drafts cache,
disables the other choices, and never grants permission or answers an approval.
Unavailable tabs cannot send suggestions. Project path pickers reuse `listFiles`
and `list_project_files` (the composer's `@` completion source); the UI validates
normalized path format/cardinality, while the kernel also checks existence,
project containment and file/directory kind at submission.

Wizard Next validates only the current step's visible controls; final submission
validates the complete values object, and terminal cards show every step read-only.
Ranking supports up/down buttons, Alt+ArrowUp/ArrowDown and the shared pointer
reorder hook. Tables have numeric-aware ascending/descending headers, a filter
for more than eight rows, and CSV export through the shared `CopyButton`.

The Desktop sends `set_generative_ui {enabled:true}` and
`set_interaction_policy {policy}` (`standard` | `aggressive` | `minimal`) to
each newly spawned process before sending prompts. OMP registers `ui.present`
alongside other host tools (`ui_present` on the wire). When interactive `ui.present`
succeeds, `AgentSession` aborts the remaining tool batch with
`TERMINAL_TOOL_RESULT_ABORT_REASON` and gates subsequent model calls until
`submit_ui` or `cancel_ui` settles the interaction, or a new user prompt /
branch transition expires it. `cancel_ui` records `status: "cancelled"` without
invoking the model. Inherited forked or unbound legacy forms project as
read-only `expired` history.
The local end-to-end fixture is
`omp/packages/coding-agent/test/fixtures/generative-ui-rpc-agent.ts`: it runs
the real SDK, RPC and persistence with a deterministic local provider.
`desktop/tests/generative-ui-fixture.html` separately exercises all six native
controls, rejected submissions, and isolated rendering errors without any
model or real-user data. Neither fixture changes actual subagent settings.


## Compaction lifecycle

`live.js` tracks `isCompacting` per tab from `get_state` and OMP's
`auto_compaction_start/end` and `manual_compaction_start/end` events.
The explicit Compact action locks immediately, before its start event.
Completion, failure or cancellation unlocks sending and requests fresh state
and full display history; each state response also requests the active model
context separately, without `fullHistory`, for the Context card. These active
messages never merge into the displayed transcript.

The composer disables send, steer, follow-up and queue-promotion actions while
compaction is active. Its draft remains editable and is not cleared by blocked
submission. The bridge also refuses prompt/queue/goal sends during this phase.
An IPC failure or process exit releases the lock. No kernel compaction or
history-persistence behavior changes.



## Remote status for all tabs

`src-tauri/src/remote/tracker.rs` keeps process-level busy and open-dialog
counts independently of the active webview tab. The stdout reader feeds its
already parsed frames immediately after `ext_ui::record`: `agent_start` starts
timing a turn; a yielding `agent_end` (`yielded`, else `isTerminal`, else yes)
clears busy and unanswered dialogs and reports the elapsed time, while one that
continues into another run changes nothing. `select`, `confirm`, `input`,
`editor` and `ask` requests with string ids (at most 256 bytes) count as
questions, at most 64 open per session; duplicate ids do not open another one.
omp's own `cancel` request (`targetId`) closes that question. Auto-approved
tool prompts and retired readers are excluded. Details of the remote consumer:
`remote-access.md`.

`AgentBridge::send` and the atomic approval-and-grant path remove an answered id
only after writing and flushing the response successfully. Natural process exit,
explicit stop, and replacement remove the session. Unrelated streaming frames
need no tracker lock or parse.
Question membership uses a hash set; updates never scan other tabs or dialogs.
Clearing a finished turn releases its remaining questions.

One replaceable `Arc<dyn TrackerSink>` receives committed changes after both
tracker locks are released. Sinks must only enqueue work; transport and push
fan-out belong to the remote manager, not the stdout reader. The tracker reports
every timed finish; push consumers apply the 30-second notification threshold.

## Streaming replies (delta frames)

Every omp process runs with `set_event_filter { events: null, messageUpdates:
"delta" }`, written by `start_session` (`agent/mod.rs` `DELTA_EVENT_FILTER`)
right after the spawn, before the frontend sends anything; omp 18.13.2 (the
floor) is the first whose delta frames can be rebuilt. A `message_update` then
carries `message: { role }` and the event without its `partial` snapshot:
`*_start` the empty block, `*_delta` the increment, `text_end` /
`thinking_end` the final text (`content`) plus the block's metadata (a
signature), `toolcall_end` the whole call, `image_end` the image block
(`content`), `start` (a restarted message) the message whole. A
whole-snapshot frame grows with the reply, so streaming one
cost bytes quadratic in its length.

- **Frontend**: `app/stream-delta.js` (`window.OMP_STREAM_DELTA`, regression
  `test-stream-delta.mjs`) applies an event to the message's blocks
  (`applyEvent`, in place) and projects them onto the bubble (`view`).
  `live.js` keeps the blocks in `streamAcc` (`{ mid, blocks, seed }`, per tab
  in the snapshot places), not on the bubble, which the transcript and the
  phone carry. `applyEvent` reports whether text or thinking changed, so a
  tool call's streamed arguments (its card comes from `tool_execution_*`)
  never cost a render. Which accumulator a frame streams into, and whether it
  is skipped, is `accFor`: an update for another `messageId` than
  `streamAcc`'s restarts the bubble's content on that message — from the
  backend's copy when `streamSnapshot` names it (and with its `ts`), otherwise
  empty (only what streams from there on, the bubble's `ts` unchanged until
  `message_end`).
- **Dropped replay**: a frame the frontend never saw cannot be made up by a
  later one, and a long reply overflows a background tab's journal. The stdout
  reader applies every assistant message frame to `agent/inflight.rs` as well
  (after the journal push, like `ext_ui`), and a reactivated tab whose replay
  came back `dropped` loads it (`streaming_message`: `{ messageId, seq,
  message }`, `streamSnapshot`, assigned only if the switch is still current).
  The bubble of that `messageId` starts from the copy — a `message_start` in
  the replay, an update whose start was evicted, or a bubble that was
  streaming when the tab was left — and that message's frames at or below
  `seq` are skipped. `seed` is the copy object itself, so every new dropped
  replay reseeds, a tab restored with an older seed too. `message_end` still
  carries the whole message and settles the bubble as before. The reader
  hands its parsed frame to `inflight.rs`, which moves blocks out of it
  rather than copying them.
