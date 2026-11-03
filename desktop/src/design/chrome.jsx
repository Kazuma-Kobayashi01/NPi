/* ═════════════════════════════════════════════════════════════════════
   chrome.jsx — the window's top bar and the floating details cards
   - WindowChrome: sidebar toggle, back / forward, title (the Chat | Code | Academic switch lives atop the sidebar: mode-switch.jsx)
   - ChromeToolbar: terminal, git, and the "More" menu
   - AmbientRail: the right panel (environment, context, usage, then widgets)
   ═════════════════════════════════════════════════════════════════════ */

// The UI language (app/i18n.js).
const _CH_T = (key, vars) => window.OMP_I18N.t(key, vars);
const { Icon, SubagentRailCard } = window;

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
// ── Window chrome ─────────────────────────────────────────────────────
// One bar: the left segment (traffic lights, sidebar toggle) is as wide as
// the sidebar and shares its background, so the sidebar reads as running the
// full window height; the rest holds the session title and the tool buttons.
function WindowChrome({
  project, onCmd,
  sidebarOpen, onToggleSidebar, toolbar,
  canBack, canForward, onBack, onForward, home = false,
}) {
  return (
    <div className={`chrome ${sidebarOpen ? "with-sidebar" : ""}`} data-tauri-drag-region>
      <div className="chrome-left" data-tauri-drag-region>
        <button className={`chrome-btn ${sidebarOpen ? "on" : ""}`}
          title={_CH_T("project sidebar ({hint})", { hint: hintFor("desktop.sidebar.toggle", "Ctrl+B") })}
          aria-pressed={!!sidebarOpen}
          onClick={onToggleSidebar}>
          <SidebarGlyph />
        </button>
        <button className="chrome-btn chrome-arrow" title={_CH_T("Back to the start page")} disabled={!canBack} onClick={onBack}>
          <ChevronGlyph dir="left" />
        </button>
        <button className="chrome-btn chrome-arrow" title={_CH_T("Back to the conversation")} disabled={!canForward} onClick={onForward}>
          <ChevronGlyph dir="right" />
        </button>
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

        <div className="chrome-right">
          {toolbar}
          {/* Windows controls — right side, hidden on macOS/Linux */}
          {IS_WIN && (
            <div className="win-controls">
              <button className="win-ctrl win-min" title={_CH_T("Minimize")}>
                <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><path d="M1 5.5h8" stroke="currentColor" strokeWidth="1" /></svg>
              </button>
              <button className="win-ctrl win-max" title={_CH_T("Maximize / Restore")}>
                <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><rect x="1.5" y="1.5" width="7" height="7" fill="none" stroke="currentColor" strokeWidth="1" /></svg>
              </button>
              <button className="win-ctrl win-close" title={_CH_T("Close")}>
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
function ChromeToolbar({ onTerminal, terminalOpen, onChanges, moreItems, chat = false, academic = false, onPanel, panelOpen, onBrowser, browserOpen, browserHint, onSubagents, subagentsOpen, onTree, treeOpen, onSideChat, sideChatOpen, copilot = false }) {
  return (
    <div className="chrome-toolbar">
      {!chat && !academic && <button className={`chrome-btn ${terminalOpen ? "on" : ""}`} aria-pressed={!!terminalOpen}
        title={_CH_T("Terminal ({hint})", { hint: hintFor("desktop.terminal.toggle", "Ctrl+`") })} onClick={onTerminal}>
        <Icon name="terminal" size={15} />
      </button>}
      {!chat && <button className="chrome-btn" title={_CH_T("Changes")} onClick={onChanges}>
        <Icon name="diff2" size={15} />
      </button>}
      {!chat && onBrowser && (
        <button className={`chrome-btn ${browserOpen ? "on" : ""}`} aria-pressed={!!browserOpen}
          title={`${_CH_T("Browser panel")}${browserHint ? ` (${browserHint})` : ""}`} onClick={onBrowser}>
          <Icon name="web" size={15} />
        </button>
      )}
      {!chat && !academic && onSubagents && (
        <button className={`chrome-btn ${subagentsOpen ? "on" : ""}`} aria-pressed={!!subagentsOpen} title={_CH_T("Subagents")} onClick={onSubagents}>
          <Icon name="agent" size={15} />
        </button>
      )}
      {!chat && onTree && (
        <button className={`chrome-btn ${treeOpen ? "on" : ""}`} aria-pressed={!!treeOpen} title={_CH_T("Conversation tree")} onClick={onTree}>
          <Icon name="branch" size={15} />
        </button>
      )}
      {onSideChat && (
        <button className={`chrome-btn ${sideChatOpen ? "on" : ""}`} aria-pressed={!!sideChatOpen} title={_CH_T(copilot ? "Academic Copilot (this project's files)" : "SideChat (temporary scratchpad)")} onClick={onSideChat}>
          <Icon name="thinking" size={15} />
        </button>
      )}
      <MoreMenu items={moreItems} />
      {onPanel && (
        <button className={`chrome-btn ${panelOpen ? "on" : ""}`} aria-pressed={!!panelOpen} title={_CH_T("Side panel")} onClick={onPanel}>
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
      <button className={`chrome-btn ${open ? "on" : ""}`} title={_CH_T("More")} aria-haspopup="menu" aria-expanded={open}
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
  const isLive = window.OMP_SUBAGENTS?.isLive ?? (s => s === "pending" || s === "running");
  const hasActiveAgents = Array.isArray(subagents) && subagents.some(a => isLive(a.status));
  const { TodoPlanCard } = window;
  return (
    <aside className="rail">
      <EnvironmentCard project={project} profileLabel={profileLabel} />
      <ContextCard breakdown={ctxBreakdown} onCompact={onCompact} compacting={compacting} />
      <UsageCard provider={provider} quotas={quotas} stats={sessionStats} />
      {(show.subagents ?? true) && hasActiveAgents && (
        <SubagentRailCard agents={subagents} paneOpen={subagentPaneOpen}
          onOpen={onOpenSubagent} onTogglePane={onToggleSubagentPane} />
      )}
      {children}
      <TodoPlanCard counts={todoCounts} kanban={kanban} />
    </aside>
  );
}

Object.assign(window, { WindowChrome, ChromeToolbar, MoreMenu, AmbientRail });
