# Chat disclosures and text hierarchy

Code: `app/chat-collapse.js` (`OMP_COLLAPSE`), `app/tool-groups.js`
(`OMP_TOOL_GROUPS`), `design/chat/tool-group.jsx`, `tool-card.jsx`,
`assistant-bubble.jsx` and `chat-view.jsx`. `app-live.jsx` owns per-tab overrides.
Tests: `test-chat-collapse.mjs`, `test-tool-groups.mjs`, `test-stream-delta.mjs`
and `test-scroll-pin.mjs`.

## One box per turn (default)

`app/work-box.js` (`OMP_WORK_BOX`, regression `test-work-box.mjs`) and
`design/chat/work-box.jsx` fold the thinking and the tool calls between two
visible replies into **one** disclosure, like Claude's "Thought · used 3 tools":
folded it is one quiet line (`已思考 · 用了 3 个工具`); while the turn runs it shows
a spinner dot with what is happening now (`思考中…` or the running tool's title)
and the tool count. Opened, it lists the thinking text and the usual tool cards
unchanged (their own fold state still follows `foldTools`). Only the user opens
it (`collapseOverrides["work:<first _id>"] === false`); Escape closes it.

A reply that has text stays its own bubble, and its thinking moves into the box
just above it (`AssistantBubble` `hideThought`). Never inside a box: failed
tool calls, approvals, asks, rendered UI, retries, goals, jobs, compaction,
failed replies and the user's messages, which also end a box. A reply with
neither thinking nor visible content is not drawn. Display-only: the transcript
and indices are untouched, `data-msg-idx` is the box's first index, and
Collapse all / Expand all toggles the boxes (`overridesForAll(..., { work })`).

The mode is `foldModes(tweaks).work`: `box` or `rows` (today's separate
thinking rows and `ToolGroup`s, described below). Settings → Chat → Folding
"Thinking and tools" sets it; unset it follows the Interface detail step
(`app/ui-detail.js`): steps 1-3 `box`, steps 4 (Detailed) and 5 (Full) `rows`.

## Four text levels

The dark chat surface overrides the existing semantic variables only within
`.chat-wrap`: `--fg: #E5E7EB` for reply text, `--fg-2: #C0C4CC` for tool summaries
and thinking, `--fg-3: #929AA6` for time/tokens/status metadata, and
`--fg-4: #747C88` for inactive labels. `--fg-5` aliases the fourth level in chat.
Ivory and Latte retain their corresponding light-theme text variables. Semantic
warning/running colors remain intact. Parent opacity is not a hierarchy mechanism;
the existing one-time entrance animation is unchanged.

## Thinking

Only non-empty thinking text actually supplied by the backend is displayed.
Live deltas and message completion concatenate the supplied thinking blocks in
original order with the same paragraph separator used by restored history; raw
messages and content blocks are not rewritten.

The frontend tracks `thinkingActive` from existing `thinking_start`,
`thinking_delta`, `thinking_end`, text and tool-call events. A phase change alone
triggers a UI update. Events skipped after a dropped-replay snapshot cannot
rewrite the phase. Unknown snapshot activity falls back to a streaming reply
that has not started visible text; message completion ends the activity.
This is frontend projection metadata, not a new backend protocol field.

Every non-empty thought, including a single line, has one disclosure button with
`aria-expanded` and `aria-controls`. It says `思考中` while active and `已思考`
when finished. Active thinking starts open; finished thinking starts folded in
`auto` and `folded` modes. An explicit `open` preference and per-message manual
overrides still win. Duration is shown only when explicit thinking-duration
metadata exists; generic reply elapsed time is never substituted.

The expanded body is plain pre-wrapped secondary text. Selecting or clicking its
text does not toggle the disclosure. The body is memoized so subsequent reply
text deltas do not rebuild unchanged thinking content.

## Individual tools

Each tool keeps its existing summary disclosure and complete result renderer.
Running calls and errors remain open by default in every mode; a deliberate
manual toggle still wins. Completed calls follow `foldTools`: `open` keeps them
open, `folded` folds them, and `auto` retains the existing age/line thresholds.
`bodyLines` estimates output size, caching settled message objects in a WeakMap;
live objects are measured afresh. Large eval/bash content is not rescanned on
unrelated updates.

## Consecutive tool groups

`groupMessages` is a display-only projection. Only successful ordinary tool rows
(`status: "ok"`, no `isError`) are grouped. Running, failed, waiting and visible
non-tool rows break a group. Hidden retry rows neither render nor split visually
adjacent calls. Original message references and transcript indices are retained.

Each run uses a stable `tool-group:<first message _id>` key, distinct from every
individual tool key. Even one settled call keeps the same wrapper and body
container, but no group header. At two or more calls, the header displays count,
status, a short summary and the disclosure control. Appending a settled call
does not remount the earlier tool cards.

A completed group defaults folded unless the tool preference is `open` or a
child has an explicit open override. A manual group override takes precedence;
it does not overwrite individual call choices. Expanding shows full results by
default, while retaining individual manual folds. Escape folds the group and
returns focus to its header. Running and failed calls remain separate visible
cards rather than disappearing inside a completed group.

## State, global commands and scrolling

`app-live.jsx` stores `sessionId → { key → collapsed }` through `OMP_SESSION_UI`.
Message `_id` values survive get_messages merges (`mergeTranscript` carries the
live IDs forward), preserving individual and derived group identities across tab
switches. Closing the tab drops its overrides. History is not evicted to fit the
minimap.

`/collapse-all` and `/expand-all` cover both individual blocks and existing group
keys. Later calls follow defaults, while calls joining an existing group retain
its manual state. Full and explicit open preferences remain supported.

The existing pin state machine still follows output only while pinned. Manually
expanding output is a reading action and unpins the view, so the next token does
not pull the newly opened records away. `Jump to latest` restores following.
While unpinned, ChatView records the top visible row and preserves its offset
through layout updates above it. The existing horizon prevents aged tool output
from collapsing underneath a history reader.

No backend protocol, overall layout, image viewer or chart renderer is changed
by this disclosure layer.

## Filler replies

A text block with no letter, digit, emoji, code fence or list/table marker (a lone `.` or `…` that some models write between tool calls) is filler (`OMP_WORK_BOX.hasSubstance`): `groupWork` does not count it as a visible reply, `AssistantBubble` does not render or copy it once the message is no longer streaming. The transcript itself is unchanged.
