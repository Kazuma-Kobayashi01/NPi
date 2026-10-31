# Collapsible tool cards and thinking (#44, #43)

Code: `src/app/chat-collapse.js` (rules, `window.OMP_COLLAPSE`), `src/design/ui/collapse-toggle.jsx` (control), `chat/tool-card.jsx` / `chat/assistant-bubble.jsx` (rendering), `chat/chat-view.jsx` (wiring), `app-live.jsx` (per-tab toggles, Collapse all / Expand all), Settings → Chat (`app/settings-schema.js`; `docs/architecture/settings.md`). Tests: `tests/test-chat-collapse.mjs`, `tests/test-tool-output.mjs` (`isError`).

## Blocks

A *block* is a tool card's body or an assistant reply's thinking. `bodyLines(msg)` estimates its rendered lines from the data (bash and eval lines wrapped at 100 chars as `pre-wrap` does, diff lines + header/track, search hits, subagent rows, thinking as prose wrapped at 100 chars), so the decision is known before layout.

`collapseStates` runs on every streaming frame, so the estimate is cached per message object (`WeakMap`): a settled entry is never changed in place — live.js replaces it with a new object on every update — so it is measured once. A live entry (running tool, streaming reply) is measured afresh every time, because live.js grows `streamingBubble` in place and that object itself sits in the transcript after `message_start` and after a get_messages merge (`live.js` `[...merged, streamingBubble]`). A new in-place write to a settled transcript entry would leave its cached size stale.

| Lines | Toggle | Collapses in "auto" | Ages older blocks |
|---|---|---|---|
| ≤ `PREVIEW_LINES` (3) | no — nothing to hide | no | no |
| ≤ `SHORT_LINES` (8) | yes | never | yes |
| longer | yes | yes | yes |

## Rule

`collapseStates(messages, { thinking, tool, after, overrides, horizonId, hidden })` → `_id → collapsed` for every block with a toggle. Thinking and tool cards each follow their own mode (Settings → Chat *Thinking* / *Tool output*, tweak keys `foldThinking` / `foldTools`):

- **auto**: blocks start expanded. A long block collapses once `after` newer blocks (tweak *collapse after*, 1–30, default 5) follow it; a running tool card or a streaming reply never collapses on its own. Blocks longer than `PREVIEW_LINES` count as newer, whatever their own mode.
- **open**: never collapses on its own.
- **folded**: tool calls and thinking start collapsed, short blocks and a streaming reply's thinking included. Failed tool calls (`isError`) remain open so errors are not hidden.

The user's toggle (`overrides`) always wins, both ways, in every mode. Messages the chat hides (`OMP_TURN_STATUS.isHidden`) neither collapse nor count.

`foldModes(tweaks)` reads explicit `foldThinking` / `foldTools` preferences first, then the active interface-detail step (`OMP_UI_DETAIL`, where Standard sets `foldTools: "folded"` and Full sets `foldTools: "open"`). An explicitly stored legacy `autoCollapse` boolean still takes precedence over an unset key.

## Collapse all / Expand all

`/collapse-all`, `/expand-all` (palette and composer) and the unbound keymap actions `desktop.chat.collapseAll` / `desktop.chat.expandAll` replace the tab's toggles with `overridesForAll(messages, collapsed)`: one toggle per block that has one, plus every live block (running tool, streaming reply) even while it is still too short for one, so it keeps the asked-for state once it grows. A block that arrives later has no toggle, so it follows the modes again.

## Not collapsing under the reader

While the view is unpinned (the user scrolled up, `app/scroll-pin.js`), ChatView passes `horizonId` = the last message's `_id` when they left the bottom: blocks after it do not age older ones, so the text being read does not fold away. Re-pinning clears it and the backlog collapses while the view sticks to the bottom. A horizon no longer in the transcript freezes nothing.

## Toggles per tab

`app-live.jsx` keeps `sessionId → { _id → collapsed }` (`OMP_SESSION_UI.entryOf`/`updateEntry`/`pruneEntries`, like plan mode), so a tab switch — which remounts ChatView — keeps them. `_id` is live.js's per-message counter; `mergeTranscript` copies the live entry's `_id` onto its get_messages copy (`_withLiveFields`), or the merge after a tab switch would re-key the bubble and drop its toggle. Closing a tab drops its toggles. History is not evicted to fit the minimap.

## Rendering

Tool calls render as summary disclosures (`button.tool-disclosure` with `aria-expanded` and `aria-controls`): a collapsed tool card unmounts its body instead of clipping a multi-line preview, leaving the tool name, target, diff line counts (`+N −M`), status/duration, and chevron on one line. Clicking the summary toggles the body. Thinking continues to use `CollapseToggle` and a line clamp.
