/* ═════════════════════════════════════════════════════════════════════
   chrome.jsx — the window's top bar and the floating details cards
   - WindowChrome: sidebar toggle, back / forward, Chat | Code switch, title
   - ChromeToolbar: terminal, git, and the "More" menu
   - AmbientRail: the right panel (environment, context, usage, then widgets)
   ═════════════════════════════════════════════════════════════════════ */

const { Icon, ProfileMenu, SubagentRailCard } = window;

// Thin wrapper around the shared `OMP_KEYMAP.hintFor` — same guard and
// rationale as composer.jsx's copy (the registry may not be loaded yet).
function hintFor(actionId, fallback) {
  return window.OMP_KEYMAP ? window.OMP_KEYMAP.hintFor(actionId) : fallback;
}

// ── Platform detection ────────────────────────────────────────────────
const IS_WIN = typeof navigator !== "undefined" &&
  (navigator.userAgent.includes("Windows") || navigator.platform.startsWith("Win"));

// The macOS "sidebar.left" glyph: a rounded window with the left pane shaded.
function SidebarGlyph() {
  return (
    <svg width="18" height="18" viewBox="0 0 18 18" fill="none" aria-hidden="true">
      <path d="M5 3.25h2v11.5H5a3 3 0 0 1-3-3v-5.5a3 3 0 0 1 3-3z" fill="currentColor" opacity="0.28" />
      <rect x="2" y="3.25" width="14" height="11.5" rx="3" stroke="currentColor" strokeWidth="1.4" />
      <path d="M7 3.5v11" stroke="currentColor" strokeWidth="1.4" />
    </svg>
  );
}

function ChevronGlyph({ dir }) {
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path d={dir === "left" ? "M10 3.25 5.25 8 10 12.75" : "M6 3.25 10.75 8 6 12.75"}
        stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}
function ChatGlyph() {
  return (
    <svg width="18" height="18" viewBox="0 0 18 18" fill="none" aria-hidden="true">
      <path d="M9 2.75c-3.45 0-6.25 2.4-6.25 5.4 0 1.5.7 2.85 1.85 3.85L4 14.75l2.7-1.1c.7.2 1.5.3 2.3.3 3.45 0 6.25-2.4 6.25-5.4S12.45 2.75 9 2.75Z"
        stroke="currentColor" strokeWidth="1.4" strokeLinejoin="round" />
    </svg>
  );
}
function CodeGlyph() {
  return (
    <svg width="18" height="18" viewBox="0 0 18 18" fill="none" aria-hidden="true">
      <path d="M6.5 5.5 3 9l3.5 3.5M11.5 5.5 15 9l-3.5 3.5M10.2 4 7.8 14"
        stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

// ── Window chrome ─────────────────────────────────────────────────────
// One bar: the left segment (traffic lights, sidebar toggle) is as wide as
// the sidebar and shares its background, so the sidebar reads as running the
// full window height; the rest holds the session title and the tool buttons.
function WindowChrome({
  project, onCmd,
  profiles, activeProfileId, startupProfileId,
  onSelectProfile, onCreateProfile, onRenameProfile, onDeleteProfile, onSetStartupProfile,
  sidebarOpen, onToggleSidebar, toolbar,
  canBack, canForward, onBack, onForward, mode, onMode, home = false,
}) {
  return (
    <div className={`chrome ${sidebarOpen ? "with-sidebar" : ""}`} data-tauri-drag-region>
      <div className="chrome-left" data-tauri-drag-region>
        <button className={`chrome-btn ${sidebarOpen ? "on" : ""}`}
          title={`project sidebar (${hintFor("desktop.sidebar.toggle", "Ctrl+B")})`}
          aria-pressed={!!sidebarOpen}
          onClick={onToggleSidebar}>
          <SidebarGlyph />
        </button>
        <button className="chrome-btn chrome-arrow" title="Back to the start page" disabled={!canBack} onClick={onBack}>
          <ChevronGlyph dir="left" />
        </button>
        <button className="chrome-btn chrome-arrow" title="Back to the conversation" disabled={!canForward} onClick={onForward}>
          <ChevronGlyph dir="right" />
        </button>
        {sidebarOpen && <div className="chrome-seg" role="radiogroup" aria-label="Mode">
          <button type="button" role="radio" aria-checked={mode === "chat"} title="Chat" className={mode === "chat" ? "on" : ""} onClick={() => onMode?.("chat")}>
            <ChatGlyph />
          </button>
          <button type="button" role="radio" aria-checked={mode !== "chat"} title="Code" className={mode !== "chat" ? "on" : ""} onClick={() => onMode?.("code")}>
            <CodeGlyph />
          </button>
        </div>}
      </div>

      <div className="chrome-main" data-tauri-drag-region>
        <div className="chrome-title" data-tauri-drag-region>
          {!home && <span className="chrome-name">{project.name}</span>}
          {!home && project.branch && (
            <span className="chrome-branch">
              <Icon name="branch" size={11} color="var(--fg-4)" />
              {project.branch}
            </span>
          )}
        </div>

        {/* Per-tab omp profile — the selector acts on the active tab only. */}
        <ProfileMenu
          profiles={profiles}
          activeId={activeProfileId}
          startupId={startupProfileId}
          onSelect={onSelectProfile}
          onCreate={onCreateProfile}
          onRename={onRenameProfile}
          onDelete={onDeleteProfile}
          onSetStartup={onSetStartupProfile}
        />

        <div className="chrome-right">
          {toolbar}
          {/* Windows controls — right side, hidden on macOS/Linux */}
          {IS_WIN && (
            <div className="win-controls">
              <button className="win-ctrl win-min" title="Minimize">
                <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><path d="M1 5.5h8" stroke="currentColor" strokeWidth="1" /></svg>
              </button>
              <button className="win-ctrl win-max" title="Maximize / Restore">
                <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><rect x="1.5" y="1.5" width="7" height="7" fill="none" stroke="currentColor" strokeWidth="1" /></svg>
              </button>
              <button className="win-ctrl win-close" title="Close">
                <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><path d="M1.5 1.5l7 7M8.5 1.5l-7 7" stroke="currentColor" strokeWidth="1.1" /></svg>
              </button>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

// The tool buttons that used to sit in the footer status bar.

// ── Toolbar: the two code-mode tools, then everything else under "More" ──
function ChromeToolbar({ onTerminal, terminalOpen, onChanges, moreItems, chat = false, onPanel, panelOpen, onBrowser, browserOpen, browserHint, onSubagents, subagentsOpen, onTree, treeOpen, onSideChat, sideChatOpen }) {
  return (
    <div className="chrome-toolbar">
      {!chat && <button className={`chrome-btn ${terminalOpen ? "on" : ""}`} aria-pressed={!!terminalOpen}
        title={`Terminal (${hintFor("desktop.terminal.toggle", "Ctrl+`")})`} onClick={onTerminal}>
        <Icon name="terminal" size={15} />
      </button>}
      {!chat && <button className="chrome-btn" title="Changes" onClick={onChanges}>
        <Icon name="diff2" size={15} />
      </button>}
      {!chat && onBrowser && (
        <button className={`chrome-btn ${browserOpen ? "on" : ""}`} aria-pressed={!!browserOpen}
          title={`Browser panel${browserHint ? ` (${browserHint})` : ""}`} onClick={onBrowser}>
          <Icon name="web" size={15} />
        </button>
      )}
      {!chat && onSubagents && (
        <button className={`chrome-btn ${subagentsOpen ? "on" : ""}`} aria-pressed={!!subagentsOpen} title="Subagents" onClick={onSubagents}>
          <Icon name="agent" size={15} />
        </button>
      )}
      {!chat && onTree && (
        <button className={`chrome-btn ${treeOpen ? "on" : ""}`} aria-pressed={!!treeOpen} title="Conversation tree" onClick={onTree}>
          <Icon name="branch" size={15} />
        </button>
      )}
      {onSideChat && (
        <button className={`chrome-btn ${sideChatOpen ? "on" : ""}`} aria-pressed={!!sideChatOpen} title="SideChat (temporary scratchpad)" onClick={onSideChat}>
          <Icon name="thinking" size={15} />
        </button>
      )}
      <MoreMenu items={moreItems} />
      {onPanel && (
        <button className={`chrome-btn ${panelOpen ? "on" : ""}`} aria-pressed={!!panelOpen} title="Side panel" onClick={onPanel}>
          <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
            <rect x="1.75" y="2.75" width="12.5" height="10.5" rx="2.2" stroke="currentColor" strokeWidth="1.3" />
            <path d="M10 3v10" stroke="currentColor" strokeWidth="1.3" />
          </svg>
        </button>
      )}
    </div>
  );
}

/** The "⋯" popover. `items`: `{ label, icon?, hint?, on?, onClick }` or the
 *  string "-" for a divider. Closes on a press outside, on Escape and after a
 *  pick. */
function MoreMenu({ items }) {
  const [open, setOpen] = React.useState(false);
  const rootRef = React.useRef(null);
  React.useEffect(() => {
    if (!open) return undefined;
    const away = e => { if (!rootRef.current?.contains(e.target)) setOpen(false); };
    const esc = e => { if (e.key === "Escape") { e.preventDefault(); setOpen(false); } };
    document.addEventListener("mousedown", away);
    document.addEventListener("keydown", esc, true);
    return () => { document.removeEventListener("mousedown", away); document.removeEventListener("keydown", esc, true); };
  }, [open]);
  return (
    <div className="more-menu" ref={rootRef}>
      <button className={`chrome-btn ${open ? "on" : ""}`} title="More" aria-haspopup="menu" aria-expanded={open}
        onClick={() => setOpen(v => !v)}>
        <svg width="16" height="16" viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
          <circle cx="3.25" cy="8" r="1.35" /><circle cx="8" cy="8" r="1.35" /><circle cx="12.75" cy="8" r="1.35" />
        </svg>
      </button>
      {open && (
        <div className="more-pop" role="menu">
          {items.map((it, i) => it === "-"
            ? <div key={i} className="more-sep" role="separator" />
            : (
              <button key={i} type="button" role="menuitem" className={`more-item${it.on ? " on" : ""}`}
                onClick={() => { setOpen(false); it.onClick?.(); }}>
                {it.icon && <Icon name={it.icon} size={14} />}
                <span className="more-label">{it.label}</span>
                {it.hint && <span className="more-hint">{it.hint}</span>}
              </button>
            ))}
        </div>
      )}
    </div>
  );
}

// ── Right panel: environment, context, usage, then what is running ─────
// Always for a tab; below them the running subagents, the branch's PR and the
// user's widgets (passed as children).
function AmbientRail({ project, profileLabel, ctxBreakdown, sessionStats, provider, quotas,
  subagents, subagentPaneOpen, onOpenSubagent, onToggleSubagentPane, show = {}, todoCounts, kanban, onCompact, compacting, children }) {
  const hasAgents = Array.isArray(subagents) && subagents.length > 0;
  const { TodoPlanCard } = window;
  return (
    <aside className="rail">
      <EnvironmentCard project={project} profileLabel={profileLabel} />
      <ContextCard breakdown={ctxBreakdown} onCompact={onCompact} compacting={compacting} />
      <UsageCard provider={provider} quotas={quotas} stats={sessionStats} />
      {(show.subagents ?? true) && hasAgents && (
        <SubagentRailCard agents={subagents} paneOpen={subagentPaneOpen}
          onOpen={onOpenSubagent} onTogglePane={onToggleSubagentPane} />
      )}
      {children}
      <TodoPlanCard counts={todoCounts} kanban={kanban} />
    </aside>
  );
}

Object.assign(window, { WindowChrome, ChromeToolbar, MoreMenu, AmbientRail });
