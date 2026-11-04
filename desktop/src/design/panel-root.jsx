/* panel-root.jsx — the page of a secondary OS window (app/panel-mode.js,
   src-tauri/src/panel_window.rs): one panel filling the whole window. The
   panels are the ones the main window used to float; they only call commands,
   so they run without the main window's state. */

function PanelRoot() {
  const { kind, project, profile, anchor } = window.OMP_PANEL;
  const close = React.useCallback(() => { window.__TAURI__.window.getCurrentWindow().close(); }, []);
  // The same theme as the main window: the tweaks live in the shared localStorage.
  const [t] = window.useTweaks(window.TWEAK_DEFAULTS);
  window.useThemeEffect(t);
  if (kind === "librarian") return <LibrarianWindow bridge={window.OMP_BRIDGE} project={project} onClose={close} />;
  if (kind === "reader") return <ReaderWindow bridge={window.OMP_BRIDGE} project={project} anchor={anchor} onClose={close} />;
  if (kind === "stats") return <UsageStatsPanel onClose={close} profileLabel={profile ?? "default"} />;
  return null;
}
window.PanelRoot = PanelRoot;
