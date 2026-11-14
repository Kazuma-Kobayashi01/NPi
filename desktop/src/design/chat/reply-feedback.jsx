/* chat/reply-feedback.jsx — 👍/👎 in an assistant reply's footer.

   A rating is kept locally per conversation and reply (app/reply-feedback.js,
   localStorage → ui-state.json) and never leaves the machine by itself. A 👎
   opens an optional reason with two ways out: Save (local only) and Tell
   assistant, which sends `Feedback on your previous reply: <reason>` as an
   ordinary message — a prompt when idle, a follow-up while a turn runs
   (`onSend("feedback", text)`, app-live.jsx `handleReplyAction`). What, if
   anything, the model remembers of it is its own call with its existing
   memory tools (docs/architecture/memory.md); nothing here writes memory.

   The icons are CSS masks (`.ass-fb-btn` in layout/chat.css), so the buttons
   add no text to a selection across the reply. */

const _RF_T = (key, vars) => window.OMP_I18N.t(key, vars);

/** `conversation`: the key of app/reply-feedback.js `conversationKey`; `ts`:
 *  the reply's omp timestamp; `excerpt`: the reply's opening, given when it
 *  is not the latest reply so the message says which one is meant. */
function ReplyFeedback({ conversation, ts, excerpt, onSend }) {
  const FB = window.OMP_REPLY_FEEDBACK;
  const Q = window.OMP_REPLY_QUOTE;
  const store = FB.shared;
  const entry = React.useSyncExternalStore(store.subscribe, () => store.get(conversation, ts));
  const [editing, setEditing] = React.useState(false);
  const [draft, setDraft] = React.useState("");
  const textRef = React.useRef(null);
  const downRef = React.useRef(null);

  React.useEffect(() => { if (editing) textRef.current?.focus(); }, [editing]);

  const rating = entry?.r;
  const toggleUp = () => {
    setEditing(false);
    if (rating === "up") store.clear(conversation, ts);
    else store.set(conversation, ts, { r: "up" });
  };
  const toggleDown = () => {
    if (rating === "down") { setEditing(false); store.clear(conversation, ts); return; }
    store.set(conversation, ts, { r: "down" });
    setDraft("");
    setEditing(true);
  };
  const openReason = () => { setDraft(entry?.t ?? ""); setEditing(true); };
  const close = () => { setEditing(false); downRef.current?.focus(); };
  const save = () => {
    store.set(conversation, ts, { r: "down", t: draft });
    close();
  };
  const tell = () => {
    const reason = draft.trim();
    if (!reason) return;
    store.set(conversation, ts, { r: "down", t: reason });
    onSend("feedback", Q.feedbackMessage({ reason, excerpt }));
    close();
  };
  const onKeyDown = (e) => {
    if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); close(); }
    else if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) { e.preventDefault(); tell(); }
  };

  return (<>
    <button type="button" className={`ass-fb-btn is-up${rating === "up" ? " is-on" : ""}`}
      aria-pressed={rating === "up"} aria-label={_RF_T("reply.good")} title={_RF_T("reply.good")} onClick={toggleUp} />
    <button type="button" ref={downRef} className={`ass-fb-btn is-down${rating === "down" ? " is-on" : ""}`}
      aria-pressed={rating === "down"} aria-label={_RF_T("reply.bad")} title={_RF_T("reply.bad")} onClick={toggleDown} />
    {rating === "down" && !editing && (
      <button type="button" className="ass-fb-reason" onClick={openReason}
        title={entry?.t ?? _RF_T("reply.addReason")}>
        {entry?.t ?? _RF_T("reply.addReason")}
      </button>
    )}
    {rating === "down" && editing && (
      <div className="ass-fb-form" role="group" aria-label={_RF_T("reply.reasonLabel")} onKeyDown={onKeyDown}>
        <textarea ref={textRef} className="ass-fb-text" rows={2} value={draft}
          maxLength={FB.MAX_REASON_CHARS}
          onChange={(e) => setDraft(e.target.value)}
          placeholder={_RF_T("reply.reasonPlaceholder")}
          aria-label={_RF_T("reply.reasonLabel")} />
        <div className="ass-fb-row">
          <button type="button" className="btn ghost" onClick={close}>{_RF_T("reply.cancel")}</button>
          <button type="button" className="btn outlined" onClick={save} title={_RF_T("reply.saveHint")}>{_RF_T("reply.save")}</button>
          <button type="button" className="btn primary" onClick={tell} disabled={!draft.trim()}
            title={_RF_T("reply.tellHint")}>{_RF_T("reply.tell")}</button>
        </div>
      </div>
    )}
  </>);
}

Object.assign(window, { ReplyFeedback });
