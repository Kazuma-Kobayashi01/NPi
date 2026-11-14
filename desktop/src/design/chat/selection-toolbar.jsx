/* chat/selection-toolbar.jsx — the floating toolbar over text selected in an
   assistant reply: Quote (into the composer draft), Ask (quote + question as
   a message) and, while a turn runs, Steer (quote + note as a steer).

   ChatView mounts one instance. `onAction(kind, text)` is app-live.jsx's
   `handleReplyAction`: "quote" gets the Markdown blockquote, "ask" and
   "steer" the finished message (quote, blank line, the typed words). The
   text helpers (clip, blockquote, withQuote) are app/reply-quote.js; the
   2000-character bound is applied there, on the selection, before it is
   shown or sent.

   Shown after a pointer selection ends (mouseup) or a keyboard selection
   (Shift/arrows, Select all); hidden by Escape, a press outside, a scroll,
   a resize or the selection going away. Tab, while the toolbar is up and
   focus is in the page, moves into it; ←/→ move between its buttons. */

const _ST_T = (key, vars) => window.OMP_I18N.t(key, vars);

const _ST_elementOf = (node) => (node?.nodeType === 1 ? node : node?.parentElement ?? null);
// Not selectable for a toolbar: the copy buttons of code blocks and the
// bubble's own controls, and the plan reply, whose blocks take clicks.
const _ST_EXCLUDED = "button, .plan-annot-blocks, .ass-actions, .ass-failure";

/** The selection as `{ text, rect }` when it starts in the markdown of an
 *  assistant reply and ends in the same reply (or at the very start of what
 *  follows it, as a triple-click does); null otherwise. */
function _ST_replySelection() {
  const sel = window.getSelection();
  if (!sel || sel.isCollapsed || sel.rangeCount === 0) return null;
  const range = sel.getRangeAt(0);
  const start = _ST_elementOf(range.startContainer);
  const body = start?.closest(".row.assistant .ass-body");
  if (!body || !start.closest(".md-content") || start.closest(_ST_EXCLUDED)) return null;
  const end = _ST_elementOf(range.endContainer);
  const endsInside = end && body.contains(end) && end.closest(".md-content") && !end.closest(_ST_EXCLUDED);
  if (!endsInside && range.endOffset !== 0) return null;
  const text = window.OMP_REPLY_QUOTE.clip(sel.toString());
  if (!text) return null;
  const r = range.getBoundingClientRect();
  return { text, rect: { left: r.left, top: r.top, bottom: r.bottom, width: r.width } };
}

const _ST_NAV_KEYS = new Set(["Shift", "ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "Home", "End", "PageUp", "PageDown"]);

function SelectionToolbar({ streaming, onAction }) {
  const [sel, setSel] = React.useState(null);
  const [mode, setMode] = React.useState("bar"); // "bar" | "ask" | "steer"
  const [note, setNote] = React.useState("");
  const rootRef = React.useRef(null);
  const inputRef = React.useRef(null);
  const latest = React.useRef(null);
  latest.current = { sel, mode };

  const hide = React.useCallback((clearSelection) => {
    setSel(null);
    setMode("bar");
    setNote("");
    if (clearSelection) window.getSelection()?.removeAllRanges();
  }, []);

  // Escape also takes the focus out of the toolbar; a press outside is the
  // start of a new selection or a click elsewhere.
  window.usePopoverDismiss(!!sel, rootRef, () => hide(false), () => {
    if (rootRef.current?.contains(document.activeElement)) document.activeElement.blur();
    hide(true);
  });

  React.useEffect(() => {
    const inToolbar = (node) => !!rootRef.current?.contains(node);
    const evaluate = () => {
      if (latest.current.mode !== "bar" || inToolbar(document.activeElement)) return;
      setSel(_ST_replySelection());
    };
    const onMouseUp = (e) => { if (!inToolbar(e.target)) setTimeout(evaluate, 0); };
    const onKeyUp = (e) => {
      if (inToolbar(e.target)) return;
      if (_ST_NAV_KEYS.has(e.key) || ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "a")) evaluate();
    };
    let raf = 0;
    const onSelectionChange = () => {
      cancelAnimationFrame(raf);
      raf = requestAnimationFrame(() => {
        const { sel: shown, mode: m } = latest.current;
        if (shown && m === "bar" && !inToolbar(document.activeElement) && !_ST_replySelection()) setSel(null);
      });
    };
    const onScroll = (e) => {
      if (latest.current.mode === "bar" && !inToolbar(e.target)) setSel(prev => (prev ? null : prev));
    };
    const onResize = () => hide(false);
    const onKeyDown = (e) => {
      if (e.key !== "Tab" || e.shiftKey || e.ctrlKey || e.altKey || e.metaKey) return;
      const { sel: shown, mode: m } = latest.current;
      const active = document.activeElement;
      if (!shown || m !== "bar" || inToolbar(active)) return;
      if (active && active !== document.body && !active.closest?.(".chat-scroll")) return;
      const first = rootRef.current?.querySelector("button");
      if (!first) return;
      e.preventDefault();
      first.focus();
    };
    document.addEventListener("mouseup", onMouseUp);
    document.addEventListener("keyup", onKeyUp);
    document.addEventListener("keydown", onKeyDown);
    document.addEventListener("selectionchange", onSelectionChange);
    document.addEventListener("scroll", onScroll, true);
    window.addEventListener("resize", onResize);
    return () => {
      cancelAnimationFrame(raf);
      document.removeEventListener("mouseup", onMouseUp);
      document.removeEventListener("keyup", onKeyUp);
      document.removeEventListener("keydown", onKeyDown);
      document.removeEventListener("selectionchange", onSelectionChange);
      document.removeEventListener("scroll", onScroll, true);
      window.removeEventListener("resize", onResize);
    };
  }, [hide]);

  // Above the selection, centred on it; below when there is no room above.
  React.useLayoutEffect(() => {
    const el = rootRef.current;
    if (!el || !sel) return;
    const GAP = 8, MARGIN = 8;
    const { offsetWidth: w, offsetHeight: h } = el;
    let top = sel.rect.top - h - GAP;
    if (top < MARGIN) top = Math.min(sel.rect.bottom + GAP, window.innerHeight - h - MARGIN);
    const left = Math.max(MARGIN, Math.min(sel.rect.left + sel.rect.width / 2 - w / 2, window.innerWidth - w - MARGIN));
    el.style.top = `${Math.round(top)}px`;
    el.style.left = `${Math.round(left)}px`;
  }, [sel, mode]);

  React.useEffect(() => { if (mode !== "bar") inputRef.current?.focus(); }, [mode]);

  if (!sel) return null;

  const quote = () => { onAction("quote", window.OMP_REPLY_QUOTE.blockquote(sel.text)); hide(true); };
  const openInput = (next) => { setNote(""); setMode(next); };
  const submit = (e) => {
    e.preventDefault();
    if (!note.trim()) return;
    onAction(mode, window.OMP_REPLY_QUOTE.withQuote(sel.text, note));
    hide(true);
  };
  // Buttons act on the stored text and keep the page's selection and focus
  // as they are; only the note field takes focus.
  const onMouseDown = (e) => { if (e.target.tagName !== "INPUT") e.preventDefault(); };
  const onKeyDown = (e) => {
    if (mode !== "bar" || (e.key !== "ArrowLeft" && e.key !== "ArrowRight")) return;
    const all = [...rootRef.current.querySelectorAll("button")];
    const i = all.indexOf(document.activeElement);
    if (i < 0) return;
    e.preventDefault();
    all[(i + (e.key === "ArrowRight" ? 1 : all.length - 1)) % all.length].focus();
  };

  return ReactDOM.createPortal(
    <div ref={rootRef} className="sel-bar" role="toolbar" aria-label={_ST_T("reply.selectionBar")}
      onMouseDown={onMouseDown} onKeyDown={onKeyDown}>
      {mode === "bar" ? (<>
        <button type="button" className="sel-bar-btn" onClick={quote} title={_ST_T("reply.quoteHint")}>{_ST_T("reply.quote")}</button>
        <button type="button" className="sel-bar-btn" onClick={() => openInput("ask")} title={_ST_T("reply.askHint")}>{_ST_T("reply.ask")}</button>
        {streaming && (
          <button type="button" className="sel-bar-btn" onClick={() => openInput("steer")} title={_ST_T("reply.steerHint")}>{_ST_T("reply.steer")}</button>
        )}
      </>) : (
        <form className="sel-bar-form" onSubmit={submit}>
          <input ref={inputRef} className="sel-bar-input" value={note} maxLength={2000}
            onChange={(e) => setNote(e.target.value)}
            placeholder={_ST_T(mode === "ask" ? "reply.askPlaceholder" : "reply.steerPlaceholder")}
            aria-label={_ST_T(mode === "ask" ? "reply.askPlaceholder" : "reply.steerPlaceholder")} />
          <button type="submit" className="sel-bar-btn is-primary" disabled={!note.trim()}>
            {_ST_T(mode === "ask" ? "reply.askSend" : "reply.steerSend")}
          </button>
        </form>
      )}
    </div>,
    document.body,
  );
}

Object.assign(window, { SelectionToolbar });
