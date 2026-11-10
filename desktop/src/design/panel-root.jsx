/* panel-root.jsx — the page of a secondary OS window (app/panel-mode.js,
   src-tauri/src/panel_window.rs): one panel filling the whole window. The
   panels are the ones the main window used to float. Most only call commands;
   history, approval rules, plan and the git browser show the main window's tab
   through app/panel-link.js. */

// A panel that shows the main window's tab (app/panel-link.js): its snapshot,
// null until the main window answers, and a bridge whose calls run there.
function LinkedPanel({ kind, onClose }) {
  const link = window.OMP_PANEL_LINK;
  const [state, setState] = React.useState(null);
  React.useEffect(() => link.subscribe(kind, setState), [kind, link]);
  const bridge = React.useMemo(() => link.proxy(kind), [kind, link]);
  // A call that ends the panel (resume, send to the agent) is answered before the window closes.
  const pending = React.useRef(null);
  const closeAfter = () => { Promise.resolve(pending.current).finally(onClose); };
  const handOver = run => { pending.current = run().catch(e => console.error(`[panel] ${kind}:`, e)); };
  if (!state) return null;
  if (kind === "history") {
    return <HistoryModal open bridge={bridge} activeCwd={state.activeCwd} onResume={s => handOver(() => bridge.resume(s))} onClose={closeAfter} />;
  }
  if (kind === "rules") {
    return <ApprovalRulesPanel bridge={bridge} activeProject={state.activeProject} sessions={state.sessions} approvalModes={state.approvalModes} onClose={onClose} />;
  }
  if (kind === "plan") {
    return <PlanKanban kanban={state.kanban} planMeta={state.planMeta} onAbort={() => bridge.abort()} onClose={onClose} />;
  }
  if (kind === "git") {
    // Remounted per project, as in the main window, and per open, so an edit card's path is selected.
    return (
      <GitBrowser key={`${state.project}\0${state.seq}`} bridge={bridge} project={state.project} branch={state.branch} edits={state.edits}
        initialPath={state.path} onClose={onClose} onSendToAgent={text => { handOver(() => bridge.sendToAgent(text)); closeAfter(); }} />
    );
  }
  return null;
}

function PanelRoot() {
  const { kind, project, profile, anchor, path } = window.OMP_PANEL;
  const close = React.useCallback(() => { window.__TAURI__.window.getCurrentWindow().close(); }, []);
  // The same theme as the main window: the tweaks live in the shared localStorage.
  const [t] = window.useTweaks(window.TWEAK_DEFAULTS);
  window.useThemeEffect(t);
  if (kind === "quick") return <QuickWindow bridge={window.OMP_BRIDGE} />;
  if (kind === "librarian") return <LibrarianWindow bridge={window.OMP_BRIDGE} project={project} profile={profile} onClose={close} />;
  if (kind === "reader") return <ReaderWindow bridge={window.OMP_BRIDGE} project={project} anchor={anchor} path={path} onClose={close} />;
  if (kind === "write") return <WriteWindow bridge={window.OMP_BRIDGE} project={project} profile={profile} onClose={close} />;
  // The Office window of one tab: `project` is that tab's session id (app/office-live.js).
  if (kind === "office") return <OfficeWindow sessionId={project} profile={profile} onClose={close} />;
  if (kind === "stats") return <UsageStatsPanel onClose={close} profileLabel={profile ?? "default"} />;
  if (Object.hasOwn(window.OMP_PANEL_LINK.CALLS, kind)) return <LinkedPanel kind={kind} onClose={close} />;
  return null;
}
window.PanelRoot = PanelRoot;
