# Reply interactions: selection toolbar and feedback

Code: `app/reply-quote.js` (`OMP_REPLY_QUOTE`), `app/reply-feedback.js` (`OMP_REPLY_FEEDBACK`), `design/chat/selection-toolbar.jsx`, `design/chat/reply-feedback.jsx`, `design/chat/assistant-bubble.jsx` (footer), `design/chat/chat-view.jsx` (mounts the toolbar, passes `conversation`, `latestReply`, `onReplyAction`), `app-live.jsx` `handleReplyAction`, `design/composer.jsx` (`promptInsert.quote`). Styles `.sel-bar*`, `.ass-fb-*` in `layout/chat.css`. Strings `reply.*` in `app/i18n/ui.js`. Tests: `test-reply-quote.mjs`, `test-reply-feedback.mjs`.

## Selection toolbar

`ChatView` mounts one `SelectionToolbar` (a portal into `<body>`, fixed position). It looks at the document selection after a pointer selection ends (`mouseup`) or a keyboard one (Shift/arrows/Select all) and shows only when the selection starts inside the markdown (`.md-content`) of an assistant reply and ends in the same reply (a triple-click may end at the start of the next block). Copy buttons and other buttons, the reply footer, failure text and the plan-mode annotation blocks are excluded. There is no segmenting in `plan-annotations.jsx` to reuse (it splits a plan into marked blocks, not DOM selections); the selection text is taken from the DOM as rendered.

The text is `OMP_REPLY_QUOTE.clip`ped: LF line ends, no blank runs, at most 2000 characters, an ellipsis when cut (never inside a surrogate pair). The toolbar keeps that text; it does not re-read the selection when a button is pressed.

|Action|Result|
|---|---|
|Quote|`onReplyAction("quote", blockquote)` → `promptInsert {quote: true}` → the composer appends it to the tab's draft as its own paragraph (`appendBlock`), focuses, sends nothing.|
|Ask|an inline field; Enter sends `> quote\n\nquestion` (`withQuote`).|
|Steer|only while a turn runs; the same message, as a steer.|

`handleReplyAction` sends ordinary user messages: `bridge.send` when idle, `bridge.followUp` while a turn runs, `bridge.steer` for Steer (a prompt if the turn ended while the note was typed). They skip goal and plan framing and are never approvals. Dismissal: Escape (prevented, so the window keymap does not abort the turn), a press outside, a scroll (while no note is being typed), a resize, or the selection going away. Tab from the page moves into the toolbar, ←/→ between its buttons.

## Feedback

👍/👎 sit in the reply's footer beside Copy, for replies with text, a conversation file and an omp `ts`. A rating is `{ r: "up"|"down", t?: reason }` under `npi.replyFeedback` in localStorage (which `app/ui-store.js` mirrors to `ui-state.json`): `{ "<session file name>": { "<reply ts>": entry } }`. The reply's omp timestamp is stable across reloads, the UI's `_id` is not. `ui-state.json` allows 256 keys and 64 KiB a value, so everything is one key held under 48 KiB: the conversation written longest ago is dropped first (a lone conversation sheds its oldest ratings). Reasons are at most 500 characters. Another window's change reaches open bubbles through the `storage` event.

A 👎 saves the rating at once and opens an optional reason: **Save** keeps it on this device only; **Tell assistant** also sends `Feedback on your previous reply: <reason>` through `handleReplyAction("feedback", …)` (a prompt or follow-up like Ask). For a reply that is not the newest, the message is `Feedback on your earlier reply:` plus a quote of its opening, so the model knows which one. Nothing here writes memory: whether a lasting preference is kept is the model's own call with its memory tools (`memory.md`).
