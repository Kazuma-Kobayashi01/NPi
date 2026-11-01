/* SideChat owns one temporary, chat-only worker for its mounted lifetime.
   Hiding the pane does not detach its stream or reset the conversation. */
(function () {
  const { Icon, MarkdownContent } = window;
  const empty = { phase: "idle", messages: [], liveText: null, error: null, model: "" };
  const labels = { idle: "Waiting to open", starting: "Connecting…", ready: "Ready",
    sending: "Switching model…", generating: "Generating…", stopping: "Stopping…",
    clearing: "Clearing…", error: "Request failed", failed: "Connection failed", unavailable: "Desktop app unavailable" };

  function SideChat({ open, onClose, model = "", profile = null }) {
    const [view, setView] = React.useState(empty);
    const [draft, setDraft] = React.useState("");
    const [retryKey, setRetryKey] = React.useState(0);
    const controllerRef = React.useRef(null);
    const scrollRef = React.useRef(null);

    React.useEffect(() => {
      if (!window.__TAURI__) {
        setView({ ...empty, phase: "unavailable", error: "SideChat requires the desktop app. Web preview does not provide simulated replies." });
        return;
      }
      const controller = window.OMP_SIDECHAT.create({
        sessionId: `sidechat-${crypto.randomUUID()}`, profile, model,
        invoke: (command, args) => window.__TAURI__.core.invoke(command, args),
        listen: (name, handler) => window.__TAURI__.event.listen(name, handler),
        onChange: setView,
      });
      controllerRef.current = controller;
      setView(controller.getState());
      return () => {
        controllerRef.current = null;
        controller.dispose().catch(error => console.error("[SideChat] Failed to stop the chat process", error));
      };
    }, [profile, retryKey]);

    React.useEffect(() => { controllerRef.current?.setModel(model); }, [model, profile]);
    React.useEffect(() => { if (open) controllerRef.current?.start(); }, [open, profile, retryKey]);
    React.useEffect(() => {
      if (open && scrollRef.current) scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }, [open, view.messages, view.liveText, view.phase]);

    const canSend = view.phase === "ready" || view.phase === "error";
    const generating = view.phase === "generating" || view.phase === "stopping";
    async function handleSend() {
      const text = draft.trim();
      if (!text || !canSend) return;
      const sentDraft = draft;
      if (await controllerRef.current.send(text)) setDraft(current => current === sentDraft ? "" : current);
    }
    function handleKeyDown(event) {
      if (event.key === "Enter" && !event.shiftKey && !event.nativeEvent.isComposing && event.keyCode !== 229) {
        event.preventDefault();
        handleSend();
      }
    }
    if (!open) return null;

    return (
      <aside className="sidechat-pane" aria-label="SideChat conversation" style={{
        width: "100%", height: "100%", display: "flex", flexDirection: "column",
        background: "var(--bg-window)", borderLeft: "1px solid var(--line)", minWidth: 0, overflow: "hidden"
      }}>
        <div className="sidechat-head" style={{
          display: "flex", alignItems: "center", justifyContent: "space-between", padding: "10px 14px",
          borderBottom: "1px solid var(--line)", background: "var(--bg-elevated)", flexShrink: 0
        }}>
          <div style={{ display: "flex", alignItems: "center", gap: 8, minWidth: 0 }}>
            <Icon name="thinking" size={14} color="var(--accent)" />
            <span style={{ fontWeight: 600, fontSize: "var(--d-text-sm)", color: "var(--fg)" }}>SideChat</span>
            <span className="chip muted" title={view.model || model || "Use the chat process default model"}
              style={{ fontSize: 10, padding: "0 4px", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
              {view.model || model || "Default model"}
            </span>
          </div>
          <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
            <button type="button" className="btn ghost icon" onClick={() => controllerRef.current.clear()}
              disabled={!canSend} title="Clear conversation" aria-label="Clear conversation" style={{ padding: 4, height: 24, width: 24 }}>
              <Icon name="trash" size={12} color="var(--fg-4)" />
            </button>
            <button type="button" className="btn ghost icon" onClick={onClose} title="Close SideChat (Esc)"
              aria-label="Close SideChat" style={{ padding: 4, height: 24, width: 24 }}>
              <Icon name="x" size={12} color="var(--fg-4)" />
            </button>
          </div>
        </div>
        <div className="sidechat-body" ref={scrollRef} style={{
          flex: 1, overflowY: "auto", padding: "14px 12px", display: "flex", flexDirection: "column", gap: 12
        }}>
          {view.messages.length === 0 && (
            <div style={{ color: "var(--fg-4)", fontSize: "var(--d-text-sm)", lineHeight: 1.6 }}>
              SideChat is a temporary, independent conversation. It does not share the main session's context or use project tools. Choose a model in Settings → Chat → SideChat.
            </div>
          )}
          {view.messages.map(message => (
            <div key={message.id} style={{ alignSelf: message.role === "user" ? "flex-end" : "flex-start", maxWidth: "92%" }}>
              <div style={{ padding: "8px 12px", borderRadius: 8,
                background: message.role === "user" ? "var(--bg-elevated)" : "color-mix(in oklab, var(--line) 15%, transparent)",
                border: "1px solid var(--line-soft, var(--line))", fontSize: "var(--d-text-sm)", color: "var(--fg-2)" }}>
                <MarkdownContent text={message.text} streaming={false} />
              </div>
            </div>
          ))}
          {view.liveText !== null && (
            <div style={{ alignSelf: "flex-start", maxWidth: "92%", padding: "8px 12px", borderRadius: 8,
              background: "color-mix(in oklab, var(--line) 15%, transparent)", border: "1px solid var(--line-soft, var(--line))",
              fontSize: "var(--d-text-sm)", color: "var(--fg-2)" }}>
              {view.liveText ? <MarkdownContent text={view.liveText} streaming={true} /> : (
                <span style={{ display: "inline-flex", alignItems: "center", gap: 6, color: "var(--fg-3)" }}>
                  <span className="pulse-dot" style={{ width: 5, height: 5 }} /> Thinking…
                </span>
              )}
            </div>
          )}
          {view.error && <div role="alert" style={{ color: "var(--danger, #d85b62)", fontSize: "var(--d-text-sm)", whiteSpace: "pre-wrap" }}>{view.error}</div>}
        </div>
          {view.phase === "failed" && <button type="button" className="btn ghost" onClick={() => setRetryKey(key => key + 1)}>Retry connection</button>}
        <div className="sidechat-composer" style={{ padding: "10px 12px 14px", borderTop: "1px solid var(--line-soft, var(--line))",
          background: "var(--bg-app)", flexShrink: 0 }}>
          <div className="sidechat-input-box" style={{ position: "relative", display: "flex", flexDirection: "column",
            background: "var(--bg-input, var(--bg-elevated))", border: `1px solid ${draft.trim() ? "var(--accent)" : "var(--line)"}`,
            borderRadius: 10, padding: "8px 10px 6px", transition: "border-color 150ms ease" }}>
            <textarea className="sidechat-textarea" aria-label="SideChat message"
              placeholder="Type a message… (Enter to send, Shift+Enter for a new line)" rows={2} value={draft} disabled={!canSend}
              onChange={event => setDraft(event.target.value)} onKeyDown={handleKeyDown} style={{
                width: "100%", background: "transparent", border: 0, outline: "none", resize: "none", fontFamily: "inherit",
                fontSize: "var(--d-text-sm)", color: "var(--fg)", lineHeight: 1.45, maxHeight: 120, padding: 0
              }} />
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginTop: 4, paddingTop: 4 }}>
              <span role="status" style={{ fontSize: 10, color: "var(--fg-4)" }}>{labels[view.phase]}</span>
              {generating ? (
                <button type="button" className="btn ghost" disabled={view.phase === "stopping"}
                  onClick={() => controllerRef.current.stop()} title="Stop generating" style={{ height: 26, padding: "0 8px" }}>Stop</button>
              ) : (
                <button type="button" className="sidechat-send-btn" disabled={!canSend || !draft.trim()}
                  onClick={handleSend} title="Send (Enter)" aria-label="Send SideChat message" style={{
                    display: "inline-flex", alignItems: "center", justifyContent: "center", width: 26, height: 26,
                    borderRadius: "50%", border: 0, cursor: canSend && draft.trim() ? "pointer" : "default",
                    background: canSend && draft.trim() ? "var(--accent)" : "var(--line)", color: "var(--on-accent, #fff)"
                  }}><Icon name="arrowUp" size={13} /></button>
              )}
            </div>
          </div>
        </div>
      </aside>
    );
  }
  window.SideChat = SideChat;
})();
