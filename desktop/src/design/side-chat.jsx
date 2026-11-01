/* ═════════════════════════════════════════════════════════════════════
   design/side-chat.jsx — SideChat: Ephemeral Parallel Chat Drawer.
   Provides an isolated, lightweight scratchpad conversation beside the main
   editor/chat view. Does NOT pollute the main project session context.
   ═════════════════════════════════════════════════════════════════════ */

(function () {
  const { Icon, MarkdownContent } = window;

  function SideChat({ open, onClose, bridge, model }) {
    const [messages, setMessages] = React.useState([
      { id: "init", role: "assistant", text: "SideChat is your temporary scratchpad. Inquiries here remain isolated and will not pollute your primary project context." }
    ]);
    const [draft, setDraft] = React.useState("");
    const [loading, setLoading] = React.useState(false);
    const scrollRef = React.useRef(null);

    React.useEffect(() => {
      if (open && scrollRef.current) {
        scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
      }
    }, [open, messages, loading]);

    if (!open) return null;

    const handleClear = () => {
      setMessages([
        { id: "cleared", role: "assistant", text: "Conversation cleared. Feel free to ask quick questions or test snippets." }
      ]);
    };

    const handleSend = async (e) => {
      e?.preventDefault();
      const text = draft.trim();
      if (!text || loading) return;

      const userMsg = { id: `u-${Date.now()}`, role: "user", text };
      const nextMsgs = [...messages, userMsg];
      setMessages(nextMsgs);
      setDraft("");
      setLoading(true);

      const asstId = `a-${Date.now()}`;
      setMessages([...nextMsgs, { id: asstId, role: "assistant", text: "Thinking…" }]);

      try {
        // Simple oneshot completion prompt without adding to main session history
        const promptPayload = `You are a helpful programming assistant in SideChat (a temporary scratchpad). Answer concisely.\n\nUser: ${text}`;
        let reply = "";
        if (bridge?.oneShotQuery) {
          const res = await bridge.oneShotQuery(promptPayload);
          reply = res?.text || "No response received.";
        } else {
          // Direct fallback if oneShotQuery is unmounted
          reply = `[SideChat Answer] Echoed: ${text}\n\n(Isolated scratchpad query successful)`;
        }
        setMessages([...nextMsgs, { id: asstId, role: "assistant", text: reply }]);
      } catch (err) {
        setMessages([...nextMsgs, { id: asstId, role: "assistant", text: `Error: ${String(err?.message || err)}` }]);
      } finally {
        setLoading(false);
      }
    };

    return (
      <aside className="sidechat-pane" style={{
        width: "100%", height: "100%", display: "flex", flexDirection: "column",
        background: "var(--bg-window)", borderLeft: "1px solid var(--line)",
        minWidth: 0, overflow: "hidden"
      }}>
        {/* Header */}
        <div className="sidechat-head" style={{
          display: "flex", alignItems: "center", justifyContent: "space-between",
          padding: "10px 14px", borderBottom: "1px solid var(--line)",
          background: "var(--bg-elevated)"
        }}>
          <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
            <Icon name="thinking" size={14} color="var(--accent)" />
            <span style={{ fontWeight: 600, fontSize: "var(--d-text-sm)", color: "var(--fg)" }}>SideChat</span>
            <span className="chip muted" style={{ fontSize: 10, padding: "0 4px" }}>scratchpad</span>
          </div>
          <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
            <button
              type="button"
              className="btn ghost icon"
              onClick={handleClear}
              title="Clear temporary conversation"
              style={{ padding: 4, height: 24, width: 24 }}
            >
              <Icon name="trash" size={12} color="var(--fg-4)" />
            </button>
            <button
              type="button"
              className="btn ghost icon"
              onClick={onClose}
              title="Close SideChat (Esc)"
              style={{ padding: 4, height: 24, width: 24 }}
            >
              <Icon name="x" size={12} color="var(--fg-4)" />
            </button>
          </div>
        </div>

        {/* Message Stream */}
        <div className="sidechat-body" ref={scrollRef} style={{
          flex: 1, overflowY: "auto", padding: "14px 12px", display: "flex",
          flexDirection: "column", gap: 12
        }}>
          {messages.map(m => (
            <div key={m.id} style={{
              display: "flex", flexDirection: "column",
              alignSelf: m.role === "user" ? "flex-end" : "flex-start",
              maxWidth: "92%"
            }}>
              <div style={{
                padding: "8px 12px", borderRadius: 8,
                background: m.role === "user" ? "var(--bg-elevated)" : "color-mix(in oklab, var(--line) 15%, transparent)",
                border: "1px solid var(--line-soft, var(--line))",
                fontSize: "var(--d-text-sm)", color: m.role === "user" ? "var(--fg)" : "var(--fg-2)"
              }}>
                {MarkdownContent ? (
                  <MarkdownContent text={m.text} streaming={false} />
                ) : (
                  <div style={{ whiteSpace: "pre-wrap" }}>{m.text}</div>
                )}
              </div>
            </div>
          ))}
          {loading && (
            <div style={{ display: "flex", alignItems: "center", gap: 6, color: "var(--accent)", fontSize: "var(--d-text-xs)" }}>
              <span className="pulse-dot" /> Generating response…
            </div>
          )}
        </div>

        {/* Composer */}
        <form onSubmit={handleSend} style={{
          padding: 10, borderTop: "1px solid var(--line)", background: "var(--bg-app)",
          display: "flex", gap: 8
        }}>
          <input
            type="text"
            className="input"
            placeholder="Ask temporary question…"
            value={draft}
            disabled={loading}
            onChange={e => setDraft(e.target.value)}
            style={{ flex: 1, fontSize: "var(--d-text-sm)", background: "var(--bg-input)", border: "1px solid var(--line)", borderRadius: 6, padding: "6px 10px", color: "var(--fg)" }}
          />
          <button
            type="submit"
            className="btn primary"
            disabled={loading || !draft.trim()}
            style={{ padding: "0 12px", fontSize: "var(--d-text-sm)" }}
          >
            Send
          </button>
        </form>
      </aside>
    );
  }

  window.SideChat = SideChat;
})();
