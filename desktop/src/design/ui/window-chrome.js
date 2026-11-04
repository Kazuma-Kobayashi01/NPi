// ui/window-chrome.js — what a secondary window (Settings, Librarian, Git,
// file preview, Approvals, Usage, Plan) shares: traffic lights at the left of
// its head and a head that drags the window about.
//
//   const wc = window.useWindowChrome(onClose, panelRef);
//   <div className="panel" ref={panelRef}>              // the window's frame
//     <div className="head" {...wc.headProps}>{wc.lights} …</div>   // a direct child
//     …
//
// Red closes. Yellow folds the window down to its head, green fills the
// screen (both again to undo; a double click on the head is green). The head
// drags by its empty parts, never by a button, field or link, and cannot be
// dragged off the screen. Position and state live as long as the component
// that calls the hook, i.e. while the window is open. Styles: layout/window-chrome.css.
//
// Wrapped in an IIFE so nothing leaks into document scope.
(function () {
  const INTERACTIVE = "button, input, select, textarea, a, label, [contenteditable='true'], [data-wc-nodrag]";
  const KEEP_VISIBLE = 48; // px of the window that must stay on screen

  /** Where a window moved by (dx, dy) may be: kept at least KEEP_VISIBLE px inside the screen, its head never above the top. */
  function clampMove(rect, dx, dy, width, height) {
    let x = dx, y = dy;
    if (rect.right + x < KEEP_VISIBLE) x = KEEP_VISIBLE - rect.right;
    if (rect.left + x > width - KEEP_VISIBLE) x = width - KEEP_VISIBLE - rect.left;
    if (rect.top + y < 0) y = -rect.top;
    if (rect.top + y > height - KEEP_VISIBLE) y = height - KEEP_VISIBLE - rect.top;
    return { x, y };
  }

  function Lights({ onClose, shaded, zoomed, onShade, onZoom }) {
    const T = window.OMP_I18N.t;
    const light = (cls, label, onClick, glyph, pressed) =>
      React.createElement("button", {
        type: "button", className: `wc-light ${cls}`, title: label, "aria-label": label,
        ...(pressed === undefined ? {} : { "aria-pressed": pressed }), onClick,
      }, React.createElement("span", { "aria-hidden": "true" }, glyph));
    return React.createElement("span", { className: "wc-lights", "data-wc-nodrag": "" },
      light("wc-close", T("Close"), onClose, "×"),
      light("wc-min", shaded ? T("Expand") : T("Collapse"), onShade, "−", shaded),
      light("wc-zoom", zoomed ? T("Restore") : T("Enlarge"), onZoom, "+", zoomed));
  }

  function useWindowChrome(onClose, panelRef) {
    const [shaded, setShaded] = React.useState(false);
    const [zoomed, setZoomed] = React.useState(false);
    const offset = React.useRef({ x: 0, y: 0 });
    const zoomedNow = React.useRef(false);
    zoomedNow.current = zoomed;

    React.useEffect(() => {
      const el = panelRef.current;
      if (!el) return;
      el.classList.toggle("wc-shaded", shaded);
      el.classList.toggle("wc-zoomed", zoomed);
    }, [shaded, zoomed, panelRef]);

    const place = React.useCallback(() => {
      const el = panelRef.current;
      const { x, y } = offset.current;
      if (el) el.style.transform = x || y ? `translate(${x}px, ${y}px)` : "";
    }, [panelRef]);

    const onPointerDown = React.useCallback(e => {
      const el = panelRef.current;
      if (e.button !== 0 || !el || zoomedNow.current || e.target.closest(INTERACTIVE)) return;
      const startX = e.clientX, startY = e.clientY, base = { ...offset.current };
      const rect = el.getBoundingClientRect();
      document.body.classList.add("wc-dragging");
      const move = ev => {
        const d = clampMove(rect, ev.clientX - startX, ev.clientY - startY, window.innerWidth, window.innerHeight);
        offset.current = { x: base.x + d.x, y: base.y + d.y };
        place();
      };
      const end = () => {
        document.body.classList.remove("wc-dragging");
        window.removeEventListener("pointermove", move);
        window.removeEventListener("pointerup", end);
        window.removeEventListener("pointercancel", end);
      };
      window.addEventListener("pointermove", move);
      window.addEventListener("pointerup", end);
      window.addEventListener("pointercancel", end);
    }, [panelRef, place]);

    const zoom = React.useCallback(() => { setShaded(false); setZoomed(z => !z); }, []);
    // In a secondary OS window the title bar's own lights and the OS drag do all of this.
    if (window.OMP_PANEL) {
      return { lights: null, headProps: { "data-wc-head": "", "data-tauri-drag-region": "" }, shaded: false, zoomed: false };
    }
    const lights = React.createElement(Lights, {
      onClose, shaded, zoomed, onShade: () => { setZoomed(false); setShaded(s => !s); }, onZoom: zoom,
    });
    const onDoubleClick = React.useCallback(e => { if (!e.target.closest(INTERACTIVE)) zoom(); }, [zoom]);
    return { lights, headProps: { "data-wc-head": "", onPointerDown, onDoubleClick }, shaded, zoomed };
  }

  window.useWindowChrome = useWindowChrome;
  window.OMP_WINDOW_CHROME = { clampMove };
})();
