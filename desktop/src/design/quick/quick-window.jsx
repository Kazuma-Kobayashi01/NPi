/* quick/quick-window.jsx — the Option+Space panel (src-tauri/src/quick_ask.rs):
   answers in a floating window, without bringing the app forward. An input bar,
   then the messages once there are any, then a capsule along the bottom edge
   for the mode, model and thinking level.

   live.js runs here for one ordinary conversation of the panel's own, saved like
   any other. A mode change takes effect on the next message, which opens a new
   conversation; until then the one on screen stays. "Continue in window" hands
   the conversation to the main window. */

const _QW_MODES = [["chat", "Chat"], ["code", "Code"], ["academic", "Academic"]];
const _QW_MAX_INPUT_ROWS = 6;

/* The window gives the page its glass on macOS (quick_ask_glass.rs): Liquid Glass, or blur
   before macOS 26; the card and the capsule then paint none of their own (quick.css). */
if (window.OMP_PANEL?.kind === "quick") {
  document.documentElement.dataset.backdrop = new URLSearchParams(location.search).get("backdrop") || "none";
}

/** The card and the capsule, as rectangles of the window with their corner radius. */
function glassShapes(root) {
  return [...root.querySelectorAll(".quick-glass, .quick-capsule")].map(el => {
    const r = el.getBoundingClientRect();
    return { x: r.left, y: r.top, width: r.width, height: r.height, radius: parseFloat(getComputedStyle(el).borderTopLeftRadius) || 0 };
  });
}

/** The window follows the page's height, and its glass the card and the capsule. */
function useQuickResize(rootRef) {
  React.useEffect(() => {
    const invoke = window.__TAURI__?.core?.invoke;
    const root = rootRef.current;
    if (!invoke || !root) return undefined;
    const report = () => {
      const height = root.getBoundingClientRect().height;
      if (height) invoke("quickask_resize", { height: Math.ceil(height), shapes: glassShapes(root) }).catch(console.error);
    };
    const observer = new ResizeObserver(report);
    observer.observe(root);
    root.querySelectorAll(".quick-glass, .quick-capsule").forEach(el => observer.observe(el));
    return () => observer.disconnect();
  }, [rootRef]);
}

function QuickInput({ value, onChange, onSubmit, inputRef, disabled }) {
  const T = window.OMP_I18N.t;
  const resize = () => {
    const el = inputRef.current;
    if (!el) return;
    el.style.height = "auto";
    const line = parseFloat(getComputedStyle(el).lineHeight) || 26;
    el.style.height = `${Math.min(el.scrollHeight, line * _QW_MAX_INPUT_ROWS)}px`;
  };
  React.useLayoutEffect(resize, [value, inputRef]);
  const onKey = e => {
    if (e.key === "Enter" && !e.shiftKey && !e.isComposing) { e.preventDefault(); onSubmit(); }
  };
  return (
    <textarea ref={inputRef} className="quick-input" rows={1} value={value} autoFocus spellCheck={false}
      placeholder={T("Ask anything…")} aria-label={T("Ask anything…")} disabled={disabled}
      onChange={e => onChange(e.target.value)} onKeyDown={onKey} />
  );
}

function QuickCapsule({ mode, onMode, models, model, onModel, levels, level, onLevel }) {
  const T = window.OMP_I18N.t;
  const key = m => `${m.provider}/${m.id}`;
  return (
    <div className="quick-capsule">
      {_QW_MODES.map(([id, label], i) => (
        <button key={id} type="button" className={`quick-mode${mode === id ? " on" : ""}`} aria-pressed={mode === id}
          title={`⌘${i + 1}`} onClick={() => onMode(id)}>{label}</button>
      ))}
      {models.length > 0 && <>
        <span className="quick-sep" />
        <select className="quick-select" aria-label={T("Model")} value={model ? key(model) : ""}
          onChange={e => onModel(models.find(m => key(m) === e.target.value))}>
          {models.map(m => <option key={key(m)} value={key(m)}>{m.name ?? m.id}</option>)}
        </select>
      </>}
      {levels.length > 0 && (
        <select className="quick-select" aria-label={T("Thinking")} value={level ?? ""} onChange={e => onLevel(e.target.value)}>
          {levels.map(l => <option key={l} value={l}>{l}</option>)}
        </select>
      )}
    </div>
  );
}

function QuickWindow({ bridge }) {
  const T = window.OMP_I18N.t;
  const { Icon, ChatView } = window;
  const rootRef = React.useRef(null);
  const inputRef = React.useRef(null);
  const [snap, setSnap] = React.useState(null);
  const [text, setText] = React.useState("");
  const [mode, setMode] = React.useState("chat");
  // The mode of the conversation on screen; a difference means the next message opens a new one.
  const [sessionMode, setSessionMode] = React.useState(null);
  const [levels, setLevels] = React.useState([]);
  useQuickResize(rootRef);

  React.useEffect(() => bridge.onUpdate(setSnap), [bridge]);

  const hide = React.useCallback(() => { window.__TAURI__?.core.invoke("quickask_hide").catch(console.error); }, []);
  const focus = React.useCallback(() => { inputRef.current?.focus(); }, []);

  const sessionId = snap?.activeSessionId ?? null;
  const messages = snap?.messages ?? [];
  const streaming = Boolean(snap?.isStreaming);
  const started = messages.length > 0;
  const current = snap?.sessions?.find(s => s.id === sessionId);

  const start = React.useCallback(async next => {
    await bridge.quickNewSession(next);
    setSessionMode(next);
  }, [bridge]);

  // A process from the first time the panel is shown: the list of models comes from it.
  const opened = React.useRef(false);
  const onShown = React.useCallback(() => {
    focus();
    if (opened.current) return;
    opened.current = true;
    start("chat").catch(console.error);
  }, [focus, start]);
  React.useEffect(() => {
    let off = null;
    window.__TAURI__?.event.listen("quickask://focus", onShown).then(fn => { off = fn; }).catch(console.error);
    window.addEventListener("focus", focus);
    return () => { off?.(); window.removeEventListener("focus", focus); };
  }, [onShown, focus]);

  React.useEffect(() => {
    if (!sessionId) return;
    bridge.thinkingLevels().then(setLevels).catch(() => setLevels([]));
  }, [bridge, sessionId, snap?.model?.id]);

  const send = async () => {
    const message = text.trim();
    if (!message || streaming) return;
    setText("");
    try {
      if (mode !== sessionMode) await start(mode);
      await bridge.send(message);
    } catch (error) {
      console.error("[quick] send failed:", error);
      setText(message);
    }
  };
  const newConversation = () => { setText(""); start(mode).catch(console.error); focus(); };
  const continueInWindow = () => { bridge.quickContinue(sessionMode ?? mode).catch(console.error); };

  React.useEffect(() => {
    const onKey = e => {
      if (e.key === "Escape") { e.preventDefault(); hide(); return; }
      if (e.metaKey && !e.ctrlKey && !e.altKey && ["1", "2", "3"].includes(e.key)) {
        e.preventDefault();
        setMode(_QW_MODES[Number(e.key) - 1][0]);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [hide]);

  const logo = <span className="quick-avatar"><Icon name="sparkle" size={16} color="#fff" /></span>;
  return (
    <div className="quick" ref={rootRef}>
      <div className="quick-glass">
        <div className="quick-bar" data-tauri-drag-region>
          {logo}
          <QuickInput value={text} onChange={setText} onSubmit={send} inputRef={inputRef} />
          {streaming ? (
            <button type="button" className="quick-round stop" title={T("Stop")} aria-label={T("Stop")} onClick={() => bridge.abort()}><span className="quick-stop" /></button>
          ) : started ? (
            <button type="button" className="quick-round" title={T("New conversation")} aria-label={T("New conversation")} onClick={newConversation}><Icon name="plus" size={15} /></button>
          ) : null}
        </div>
        {started && <>
          <div className="quick-messages">
            <ChatView messages={messages} streaming={streaming} hasProjectPath={false} foldModes={{}}
              collapseOverrides={{}} onToggleCollapse={() => {}} followOutput
              onAskAnswer={(id, value) => bridge.answerAsk(id, value)}
              onAskDialogAnswer={(id, answers) => bridge.answerAskDialog(id, answers)}
              onConfirmAsk={(id, confirmed) => bridge.answerConfirm(id, confirmed)}
              onCancelAsk={id => bridge.cancelAsk(id)}
              onGrantApproval={(prompt, scope) => bridge.approveAndRemember(prompt, scope)}
              onStopRetry={() => bridge.stopRetry()}
              onUiSubmit={bridge.submitUi} onUiCancel={bridge.cancelUi} onUiRendered={bridge.renderedUi} />
          </div>
          <div className="quick-foot">
            <span>{mode !== sessionMode ? T("The next message starts a new conversation in this mode.") : T("↩ send · esc close · ⌘1–3 switch mode")}</span>
            <button type="button" className="quick-link" disabled={streaming || !current?.sessionFile} onClick={continueInWindow}>{T("Continue in window")}</button>
          </div>
        </>}
      </div>
      <QuickCapsule mode={mode} onMode={next => { setMode(next); focus(); }}
        models={snap?.models ?? []} model={snap?.model} onModel={m => m && bridge.setModel(m)}
        levels={levels} level={snap?.thinkingLevel} onLevel={l => bridge.setThinkingLevel(l)} />
    </div>
  );
}

window.QuickWindow = QuickWindow;
