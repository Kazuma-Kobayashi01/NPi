# Session model

One tab = one omp process. `lib.rs::setup` spawns nothing; the frontend reopens the previous run's tabs (see *Reopening tabs on relaunch*), and the session column renders `design/empty-workspace.jsx` whenever `sessions` is empty — at a launch with nothing to reopen and after the last tab closes. It is a welcome screen, not a launcher (opening projects is the sidebar's and tab bar's job): a canvas "digital rain" behind an ASCII-art OMP-DESKTOP logo (pure geometry in `app/matrix-rain.js`, regression `test-matrix-rain.mjs`; static logo only under `prefers-reduced-motion`), plus any note filed while no tab existed (`workspaceNotes`, e.g. a failed "Open with").

The profiles are managed in Settings → Agent → Profiles (`design/settings/rows-profiles.jsx`: startup default, use in this tab, new, rename, remove); the top bar has no profile menu. Each tab also owns its **profile**: `omp --profile <id>` isolates auth/sessions/settings/caches under `~/.omp/profiles/<id>/`. The profile list lives in `<app config>/profiles.json` (`src-tauri/src/profiles.rs`); the reserved id `default` means *no* `--profile` flag, i.e. omp's own `~/.omp/agent`. Without an app config folder (`lib.rs` `config_dir`: unresolvable or relative) the list lives in memory for the app run, starting with just the built-in profile; there is deliberately no temp-dir fallback, since another account could plant a list there and pick the profile tabs start in. The same memory-only mode covers `open-tabs.json`, `recent-projects.json`, `keybindings.json`, `terminal.json`, `approval-mode.json`, `browser.json` and the `approval-rules/` grants: the app stays usable, it just remembers nothing past quitting. Rail widgets are the exception: without a config folder they are disabled (`widgets.md`), not run from memory. Ids are slugified at creation and immutable — rename changes the label only. A profile is fixed at spawn, so `OMP_BRIDGE.switchSessionProfile(id, profileId)` stops and respawns that one tab's process (its transcript is dropped with it); `saved_sessions` resolves its root per profile, so history/resume stay inside the tab's own tree. `delete_profile` only unlists — it never deletes `~/.omp/profiles/<id>/`, and `OMP_BRIDGE.deleteProfile` refuses ids still in use by an open tab (only the frontend knows the tab set).

`create_profile` also seeds `~/.omp/profiles/<id>/agent/models.yml` with a single keyless provider (`providers: {anthropic: {auth: none}}`, see `profiles::seed_bootstrap`) — RPC mode is non-interactive and omp exits at startup once no model resolves, so without the seed a fresh profile's tab would die before ever reaching `/login`. It never overwrites an existing `models.yml` (a re-created id, or a user who already wrote one, keeps theirs), and `clear_profile_bootstrap` removes it after a successful login, but only while the file is still byte-identical to what it seeded.

The same menu ticks one profile as the **startup default**, stored as `ProfilesFile::startup` — a *pointer*, not a reordering, since `normalize` pins the built-in entry first and ids are the join key for on-disk data. `None`/absent means the built-in profile. It governs any tab opened with no active tab to inherit from — including every tab at launch — and whose recent projects the empty workspace lists; `openSession` otherwise inherits the active tab's profile, and running tabs are never moved. The pointer is re-validated against the list on every load and inside every `mutate`, so deleting or hand-editing away the ticked profile degrades to the built-in one instead of dangling. `list_profiles` returns `{profiles, startupId}` in one payload so the menu can't render a checkmark against a stale default.

**Composer and plan mode are per tab too** (#28), but desktop-only state omp never sees, so they live in React rather than in the bridge snapshot: `app-live.jsx` keeps a `plans` map and `design/composer.jsx` a `drafts` map (text, attachments, collapsed pastes, prompt-history recall position, in-flight image count), both keyed by session id via `app/session-ui.js` (regression `test-session-ui.mjs`) and pruned when a tab closes. Both key on `activeProject.id` (the tab being rendered, `""` with none open — a no-op key). The `Composer` deliberately stays mounted across tab switches rather than being re-keyed: a remount would re-fire its `draftInsert`/`promptInsert` effects, and an image still being prepared must land in the tab it was pasted into (`addFiles` captures the id), not in whichever tab is active when it resolves. Purely visual state (caret, slash selection, mention list) is reset on a switch instead. Plan state is not reset by `/new` or a profile respawn inside the same tab. The column beside the chat is per tab the same way (`app-live.jsx` `sides`: the rail, the conversation tree or the browser panel; `docs/architecture/browser.md`).

The composer `+` menu selects images or appends a folder address using
`OMP_BRIDGE.pickFolder` (the existing OS directory picker only, not
`openProject`). Whitespace-containing paths are quoted in the `@` reference.
The asynchronous result updates the originating open tab's latest draft;
cancellation leaves it unchanged and a closed tab is never recreated.
The menu and `ComposerAttachments` viewer are keyed by session id. Image
preview reuses `ImageLightbox`, including modal focus, Escape and previous/
next navigation; the thumbnail remove button is a separate sibling.
Completed assistant output has an always-visible footer `CopyButton`,
copying the original Markdown rather than rendered DOM or action labels.

The mode switch (Chat, Code, Academic) is the picker `ModeSwitch` at the very top of the left sidebar (`design/mode-switch.jsx`, rendered first by `ChatSidebar` and `ProjectSidebar`; it is hidden with the sidebar). It shows the current mode with a chevron and opens a menu (`role=menu`, `menuitemradio`) of the three with a one-line hint each and a check on the current; arrows/Home/End move, Enter picks, Escape closes and returns focus, a click outside closes. The window chrome keeps just the sidebar toggle and back/forward. `app-live.jsx` normalises `t.appMode` to `appModeName` (anything but `chat`/`academic` is Code), sends it through `bridge.setAppMode`, and passes it to the start page, whose greeting (`app/greetings.js`) is Academic-worded. Academic currently shares Code's project tabs and sidebar; only Chat has its own folder space.

## Chat rich interaction and image ownership

A new Chat session starts with thinking off (`agent/spawn.rs` `chat_flags`, `--thinking=off`). The level can be raised in the session; a resumed chat keeps its own.

`app_mode.rs::CHAT_PROMPT` is the English, human-centered Chat base prompt,
not a second runtime. Chat favors sourced visual explanations and useful
Display UI, but keeps ordinary conversation concise, honors text-only
preferences, and uses Interactive UI only for necessary bounded input.
Permissions and mandatory Ask gates are unchanged. Available research
surfaces are `web_search` (text-source discovery), `read` (known URLs), and
the enabled eval `browser` prelude for dynamic pages. When `generate_image.enabled`
is true in the session's profile/config scope, `generate_image` is added to Chat's
`--tools` allowlist and rendered directly on its tool card; otherwise Chat keeps
`--tools=web_search,read`. There is no dedicated image-search tool or interactive
map renderer; real source-page images and static map images can be displayed
without pretending otherwise.

`OMP_CHAT_IMAGE` supplies the shared `MarkdownMediaContext` for Markdown
and `ui.present` image components. A component's `src`, optional real
`thumbnail`, source-page URL, attribution and stable UI id are persisted as
metadata, not base64 image payloads. Each component renders its placeholder
immediately and independently, preserving source order while text streams.
A distinct thumbnail loads first; a full-image failure retains that preview.
An undecodable image becomes a retryable error instead of endless loading.

The disk-cache conversation key is `[profileId, sessionFile ?? tabId]`;
a saved conversation must not include the ephemeral tab id in that key.
The in-memory store is still tied to the active tab and disposed on switch.
Subscribers share requests by source; one unmount does not abort another
subscriber's transfer, while the last subscriber cancels its pending host
request. Failed sources retry explicitly, not in an automatic frontend loop.
Completed unpinned entries have a 24-entry / 32 MiB memory-cache budget.

`chat_image_read` accepts a request id and `thumbnail`/`full` priority;
`chat_image_cancel` is scoped by tab id plus request id. The Rust manager
admits four remote transfers and sixteen queued transfers, dispatching
queued thumbnails first. One 20-second deadline covers admission and
network resolution/redirect/body work; only transient transfer failures
retry once inside that budget. Cancellation drops the active HTTP future.
A system DNS lookup already running on a blocking worker cannot be killed,
but its result is discarded and no longer holds the caller or transfer slot.

Remote images retain HTTPS-only public DNS/IP pinning, per-redirect
validation, the 8 MiB body cap and remote-SVG rejection. The WebView still
receives only host-validated data URLs under the unchanged CSP. Cache reads
validate source, size, MIME signature and SHA-256; old entries without a
digest are validated and upgraded. Atomic persistence and oldest-file
eviction to a 256 MiB disk budget run off the first-display path.

Host timing logs cover validation, transfer, cache lookup/write and total
request time using hosts and hashes rather than full source URLs. Frontend
debug records cover thumbnail/full receipt and decode; search logs record
elapsed time, provider, source count and failure status without query text.
These measurements are diagnostic samples, not a guaranteed network SLA.

## Plan usage refresh

`app/use-quotas.jsx` reads subscription limits for the active profile on entry.
Live `session_settled` events from each tab, including background tabs, force
an invalidation and read for that tab's profile; replay does not repeat it.
There is no periodic poll. Manual refresh uses the same invalidation path.
The kernel's provider failure cooldown still applies. A provider failure can
leave no report, and the CLI does not expose enough information here to
distinguish every such case from an account without subscription limits.
Concurrent requests are serialized per profile, and explicit errors appear
alongside retained same-profile data.

The Usage statistics panel uses `npi stats --json --range` for the active
profile: 24 hours, 7 days, 30 days, or all locally available history. Its
token totals sum uncached input, output, cache read and cache write; model
rows preserve those components and provider identity. Costs are estimates.

## Context usage

`get_state.contextUsage` reports `{ tokens, contextWindow, percent }`.
`tokens` already includes the fixed system/tool context and active messages,
including `compactionSummary`; it is not just the conversation token count.
The kernel uses its tokenizer and provider usage anchors. Its richer internal
`getContextBreakdown` is not exposed by RPC.

The bridge reads active context separately with `get_messages` **without**
`fullHistory`, keeps it per tab as `contextMessages`, and passes it to
`OMP_USAGE.contextBreakdown(rpcState, contextMessages)`. The full-history
transcript is for display only: discarded historical summaries must not be
summed into current usage. Manual and automatic compaction completion refresh
both state and active messages; compaction start/end and `isCompacting` from
state govern the in-progress UI.

The Context card separates active `compactionSummary.summary` text as
**Previous compaction**, deducting its estimate from Messages rather than
counting it twice. System, tool, summary and ordinary-message text estimates
are reconciled together when they exceed the reported used total, so a large
fixed-prompt estimate cannot incorrectly consume all of Messages. Remaining
unclassified overhead stays in Messages. All parts are estimates, not exact
provider billing. Images and opaque/native payloads have no separate token
count in this RPC and are not priced by base64/ciphertext length.
`tokensBefore` and optional `tokensAfter` are whole-context before/after
compaction metadata, never the token size of the summary itself.

The card's **Details** button opens **Context details**
(`design/context-insight-modal.jsx`; donut, trend and chips in
`design/context-insight-charts.jsx`, loaded before it): KPI tiles, the whole
context as a donut (the card's fixed parts plus the Messages split), a
per-request trend with turn bands and a hover detail strip, tool and command
chips, and file activity. Its data is `ctxBreakdown.insight`,
which `live.js` (`_contextBreakdown`) computes with
`OMP_CONTEXT_INSIGHT.analyze(contextMessages, messagesTokens)`
(`app/context-insight.js`) from the same active context, so it travels in the
same snapshot field and never sees discarded history. It splits the Messages
share into your messages / answers / reasoning / tool calls / tool results /
other, with the text estimates scaled together (up or down) to exactly the
Messages total: about 4 characters a token runs low on code and JSON, and an
unscaled residual read as half the context being "overhead". Images are
counted, not sized. It lists per-request context from each assistant
message's `usage` (the last 60 of all requests, cached share, cache-hit rate),
tool calls and errors, files read and edited (`read`/`write`/`edit`/`ast_edit`
paths, read ranges stripped) and shell programs (`bash` calls and `!`
executions, one per segment of a quote-aware split; heredoc bodies, quoted
text and wrapper words such as `sudo`/`env` are skipped). Ideas after
dsh-context (Apache-2.0); no code shared.

## Todo and plan rail card

The ambient rail's bottom `TodoPlanCard` (`design/side-panel.jsx`, wired through `AmbientRail` in `design/chrome.jsx`) receives the active session's `kanban` and counts from `app-live.jsx`. Its header uses the same local disclosure interaction as other `SideCard` cards: initially folded, click/Enter/Space toggles tasks in place rather than opening `PlanKanban`. Expanded content shows phase headings, task statuses and notes, updates with the active session data, and scrolls within a bounded height. Empty plans display an idle state. The separate **Todo and plan** More-menu action still opens the full plan view.


## SideChat

SideChat owns a separate ephemeral worker, not a project tab. `sidechat_start`
starts it with a fixed conversational prompt, a project-independent working
directory, and no tools, extensions, skills, rules, context files or session
persistence. It shares the selected profile's model credentials, not the main
conversation's messages, instructions or memory.

The pane stays mounted while hidden so its event listeners and transcript
survive closing and reopening during a reply. Actual disposal stops its worker.
The controller consumes the normal `{seq, text}` event envelopes and rebuilds
delta messages with `OMP_STREAM_DELTA`. Startup, command failures and process
exit are visible errors, not successful readiness or indefinite thinking.

Settings → Chat → SideChat selects `sidechatModel` (`provider/model`); empty
uses the worker's default. Model selection is acknowledged before sending the
next prompt. Stop and clear target only the independent worker.

