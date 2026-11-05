// app/panel-mode.js — tells a page whether it is a secondary OS window.
//
// A secondary window (src-tauri/src/panel_window.rs) loads index.html as
// `index.html?panel=<kind>&project=…&profile=…` and shows one panel filling
// the whole window. `window.OMP_PANEL` is `{ kind, project, profile }` there
// and null in the main window. live.js stays inert in a panel (it would reset
// the tabs the main window owns); the window's own title bar carries the
// traffic lights, the panel's head only drags (ui/window-chrome.js).
(function () {
  const q = new URLSearchParams(location.search);
  const kind = q.get("panel");
  window.OMP_PANEL = kind ? { kind, project: q.get("project") || "", profile: q.get("profile") || null, anchor: Number(q.get("anchor")) || null, path: q.get("path") || null } : null;
  if (window.OMP_PANEL) document.documentElement.classList.add("panel-mode", `panel-${kind}`);

  /** Opens (or focuses) the secondary window of `kind`; resolves to whether it opened. */
  window.OMP_OPEN_PANEL = async function (kind, opts) {
    const invoke = window.__TAURI__?.core?.invoke;
    if (!invoke) return false;
    try {
      await invoke("open_panel_window", { kind, project: opts?.project ?? null, profile: opts?.profile ?? null, title: opts?.title ?? null, anchor: opts?.anchor ?? null, path: opts?.path ?? null });
      return true;
    } catch (e) {
      console.error("[panel] open failed:", e);
      return false;
    }
  };
})();
