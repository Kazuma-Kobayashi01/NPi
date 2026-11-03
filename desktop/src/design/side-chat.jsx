/* SideChat owns one temporary, chat-only worker for its mounted lifetime.
   Hiding the pane does not detach its stream or reset the conversation.
   With `copilotProject` the pane is the Academic Copilot of that project. */
(function () {
  const { Icon, MarkdownContent, ModelMenu } = window;
  const T = (key, vars) => window.OMP_I18N.t(key, vars);
  const empty = { phase: "idle", messages: [], liveText: null, error: null, model: "", level: "" };
  const labels = { idle: "Waiting to open", starting: "Connecting…", ready: "Ready",
    sending: "Switching model…", generating: "Generating…", stopping: "Stopping…",
    clearing: "Clearing…", error: "Request failed", failed: "Connection failed", unavailable: "Desktop app unavailable" };
  const dotOf = { ready: "ok", error: "bad", failed: "bad", unavailable: "bad",
    starting: "busy", sending: "busy", generating: "busy", stopping: "busy", clearing: "busy" };
  // Copilot starters: they fill the box (the user finishes the sentence), never send.
  const starters = [
    { label: "Summarize my notes", text: "Summarize the notes in notes/ and list what is still missing." },
    { label: "Search the project", text: "Search the project for " },
    { label: "Compare papers", text: "Compare the papers on " },
  ];
  const BOX_MAX = 160;

  const CopilotGlyph = () => (
    <svg width="15" height="15" viewBox="0 0 18 18" fill="none" aria-hidden="true">
      <path d="M9 3.5 1.75 7 9 10.5 16.25 7 9 3.5Z" stroke="currentColor" strokeWidth="1.4" strokeLinejoin="round" />
      <path d="M5 9v3c0 1.1 1.8 2 4 2s4-.9 4-2V9M16.25 7v4.25" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );

  function SideChat({ open, onClose, model = "", profile = null, copilotProject = "", bridge = null, favorites = [], onManageModels }) {
    const M = window.OMP_MODELS;
    const copilot = !!copilotProject;
    const name = T(copilot ? "Academic Copilot" : "SideChat");
    const folder = copilotProject ? copilotProject.split(/[\\/]/).filter(Boolean).pop() : "";
    const [view, setView] = React.useState(empty);
    const [draft, setDraft] = React.useState("");
    const [retryKey, setRetryKey] = React.useState(0);
    const [picked, setPicked] = React.useState(null);     // this pane's own choice; null: the setting's model
    const [catalog, setCatalog] = React.useState([]);
    const [away, setAway] = React.useState(false);        // scrolled off the bottom
    const controllerRef = React.useRef(null);
    const scrollRef = React.useRef(null);
    const stickRef = React.useRef(true);
    const lastTopRef = React.useRef(0);
    const boxRef = React.useRef(null);
    const wanted = window.OMP_SIDECHAT.wantedModel(picked, model);

    React.useEffect(() => {
      if (!window.__TAURI__) {
        setView({ ...empty, phase: "unavailable", error: `${name} requires the desktop app. Web preview does not provide simulated replies.` });
        return undefined;
      }
      const controller = window.OMP_SIDECHAT.create({
        sessionId: `${copilot ? "copilot" : "sidechat"}-${crypto.randomUUID()}`, profile, model: wanted,
        ...(copilot ? { startCommand: "academic_copilot_start", startArgs: { project: copilotProject } } : {}),
        invoke: (command, args) => window.__TAURI__.core.invoke(command, args),
        listen: (eventName, handler) => window.__TAURI__.event.listen(eventName, handler),
        onChange: setView,
      });
      controllerRef.current = controller;
      setView(controller.getState());
      return () => {
        controllerRef.current = null;
        controller.dispose().catch(error => console.error(`[${name}] Failed to stop the chat process`, error));
      };
    }, [profile, retryKey, copilotProject]); // eslint-disable-line react-hooks/exhaustive-deps

    // The pick (else the Settings default) reaches the worker with the next message.
    React.useEffect(() => { controllerRef.current?.setModel(wanted); }, [wanted, profile, retryKey, copilotProject]);
    React.useEffect(() => { if (open) controllerRef.current?.start(); }, [open, profile, retryKey, copilotProject]);

    // The model list comes from omp's CLI, like the start page's: no session needed.
    React.useEffect(() => {
      let live = true;
      Promise.resolve(bridge?.modelsCatalog?.(profile)).then(r => { if (live && r?.ok && Array.isArray(r.value)) setCatalog(r.value); });
      return () => { live = false; };
    }, [bridge, profile]);

    const effective = wanted || view.model;
    const current = React.useMemo(() => {
      if (!effective) return null;
      return catalog.find(m => M.keyOf(m) === effective) ?? window.OMP_SIDECHAT.stubModel(effective);
    }, [catalog, effective]);

    // Follow the stream while the reader is at the bottom; a jump button otherwise.
    // Only the reader moving up lets go of the bottom: content growing under a programmatic scroll
    // (a delta lands before its scroll event) is not the reader leaving.
    const onScroll = () => {
      const el = scrollRef.current;
      if (!el) return;
      const near = el.scrollHeight - el.scrollTop - el.clientHeight < 80;
      if (near) stickRef.current = true;
      else if (el.scrollTop < lastTopRef.current) stickRef.current = false;
      lastTopRef.current = el.scrollTop;
      setAway(!stickRef.current);
    };
    const toBottom = () => {
      const el = scrollRef.current;
      if (el) { el.scrollTop = el.scrollHeight; lastTopRef.current = el.scrollTop; stickRef.current = true; setAway(false); }
    };
    // Streamed deltas arrive many times a frame: follow once per frame, and only while the reader is at the bottom.
    const rafRef = React.useRef(0);
    React.useEffect(() => {
      if (!open || !stickRef.current || rafRef.current) return;
      rafRef.current = requestAnimationFrame(() => {
        rafRef.current = 0;
        const el = scrollRef.current;
        if (el && stickRef.current) { el.scrollTop = el.scrollHeight; lastTopRef.current = el.scrollTop; }
      });
    }, [open, view.messages, view.liveText, view.phase]);
    React.useEffect(() => () => { if (rafRef.current) cancelAnimationFrame(rafRef.current); }, []);

    // The box grows with its text, up to a cap.
    React.useLayoutEffect(() => {
      const box = boxRef.current;
      if (!box) return;
      box.style.height = "auto";
      const full = box.scrollHeight;
      box.style.height = `${Math.min(full, BOX_MAX)}px`;
      box.style.overflowY = full > BOX_MAX ? "auto" : "hidden";
    }, [draft, open]);
    React.useEffect(() => { if (open) boxRef.current?.focus({ preventScroll: true }); }, [open]);

    const canSend = view.phase === "ready" || view.phase === "error";
    const generating = view.phase === "generating" || view.phase === "stopping";
    async function handleSend() {
      const text = draft.trim();
      if (!text || !canSend) return;
      const sentDraft = draft;
      stickRef.current = true;
      if (await controllerRef.current.send(text)) setDraft(current => current === sentDraft ? "" : current);
    }
    function handleKeyDown(event) {
      if (event.nativeEvent.isComposing || event.keyCode === 229) return;
      if (event.key === "Enter" && !event.shiftKey) { event.preventDefault(); handleSend(); }
      else if (event.key === "Escape") {
        event.preventDefault();
        if (generating) controllerRef.current?.stop(); else onClose?.();
      }
    }
    const fill = text => { setDraft(text); boxRef.current?.focus(); };
    if (!open) return null;

    const hasContent = view.messages.length > 0 || view.liveText !== null;
    const pick = m => setPicked(M.keyOf(m));

    return (
      <aside className="sidechat-pane" aria-label={T("{name} conversation", { name })}>
        <header className="sc-head">
          <span className="sc-icon" aria-hidden="true">{copilot ? <CopilotGlyph /> : <Icon name="thinking" size={14} />}</span>
          <div className="sc-titles">
            <span className="sc-title">{name}</span>
            {folder && <span className="sc-sub" title={copilotProject}>{folder}</span>}
          </div>
          <span className={`sc-dot ${dotOf[view.phase] ?? ""}`} role="img" aria-label={T(labels[view.phase])} title={T(labels[view.phase])} />
          <button type="button" className="sc-ibtn" onClick={() => controllerRef.current.clear()}
            disabled={!canSend || !hasContent} title={T("Clear conversation")} aria-label={T("Clear conversation")}>
            <Icon name="trash" size={13} />
          </button>
          <button type="button" className="sc-ibtn" onClick={onClose} title={T("Close {name} (Esc)", { name })} aria-label={T("Close {name}", { name })}>
            <Icon name="close" size={13} />
          </button>
        </header>

        <div className="sc-body-wrap">
          <div className="sc-body" ref={scrollRef} onScroll={onScroll} role="log" aria-live="polite" aria-label={T("{name} messages", { name })}>
            {!hasContent && (
              <div className="sc-empty">
                <div className="sc-empty-title">{T(copilot ? "Ask about this project" : "A private scratchpad")}</div>
                <p className="sc-empty-text">
                  {copilot
                    ? T("Searches and reads your notes, digests and text papers, and can save notes. No network or shell; PDFs are not readable yet.")
                    : T("Separate from your project and the main session. No tools.")}
                </p>
                {copilot && (
                  <div className="sc-starters">
                    {starters.map(s => (
                      <button key={s.label} type="button" className="sc-starter" onClick={() => fill(T(s.text))} disabled={!canSend}>{T(s.label)}</button>
                    ))}
                  </div>
                )}
              </div>
            )}
            {view.messages.map(message => (
              <div key={message.id} className={`sc-msg ${message.role === "user" ? "user" : "assistant"}`}>
                {message.role === "user"
                  ? <div className="sc-user-text">{message.text}</div>
                  : <MarkdownContent text={message.text} streaming={false} />}
              </div>
            ))}
            {view.liveText !== null && (
              <div className="sc-msg assistant">
                {view.liveText ? <MarkdownContent text={view.liveText} streaming={true} /> : (
                  <span className="sc-thinking"><span className="pulse-dot" /> Thinking…</span>
                )}
              </div>
            )}
            {view.error && <div role="alert" className="sc-error">{view.error}</div>}
            {view.phase === "failed" && (
              <button type="button" className="sc-retry" onClick={() => setRetryKey(key => key + 1)}>{T("Retry connection")}</button>
            )}
          </div>
          {away && <button type="button" className="sc-jump" onClick={toBottom} title={T("Jump to latest")} aria-label={T("Jump to latest")}>
            <Icon name="arrowUp" size={13} style={{ transform: "rotate(180deg)" }} />
          </button>}
        </div>

        <div className="sc-composer">
          <div className={`sc-box${draft.trim() ? " has-text" : ""}`}>
            <textarea ref={boxRef} className="sc-input" aria-label={T("{name} message", { name })} rows={1} value={draft} disabled={!canSend && !generating}
              placeholder={T(copilot ? "Ask about this project…" : "Message…")}
              onChange={event => setDraft(event.target.value)} onKeyDown={handleKeyDown} />
            <div className="sc-foot">
              <ModelMenu models={catalog} current={current} favorites={favorites} level={view.level}
                loadLevels={() => controllerRef.current?.thinkingLevels() ?? Promise.resolve([])}
                loadRecent={() => bridge?.recentModels?.(profile)}
                onPick={pick} onSetLevel={level => controllerRef.current?.setThinkingLevel(level)} onManage={onManageModels} />
              <span className="sc-grow" />
              <span role="status" className="sc-status">{view.phase === "ready" ? "" : labels[view.phase]}</span>
              {generating ? (
                <button type="button" className="sc-send stop" disabled={view.phase === "stopping"}
                  onClick={() => controllerRef.current.stop()} title={T("Stop (Esc)")} aria-label={T("Stop generating")}>
                  <Icon name="stop" size={11} />
                </button>
              ) : (
                <button type="button" className="sc-send" disabled={!canSend || !draft.trim()}
                  onClick={handleSend} title={T("Send (Enter)")} aria-label={T("Send {name} message", { name })}>
                  <Icon name="arrowUp" size={13} />
                </button>
              )}
            </div>
          </div>
        </div>
      </aside>
    );
  }
  window.SideChat = SideChat;
})();
