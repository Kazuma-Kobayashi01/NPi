/* ═════════════════════════════════════════════════════════════════════
   context-menu.jsx — the right-click card of the sidebar rows, at the
   pointer. `items`: `{ label, key?, danger?, confirm?, onClick }` or "-".
   `key` is the one-letter shortcut shown at the right and pressed while the
   menu is open; `confirm` makes the first press only ask (the label turns
   into that text). Closes on a press outside, Escape and after a pick.
   `useContextMenu()` → `{ menu, open(e, items), close }` for the owner.
   ═════════════════════════════════════════════════════════════════════ */

function useContextMenu() {
  const [menu, setMenu] = React.useState(null);
  const open = React.useCallback((e, items) => {
    if (!items.length) return;
    e.preventDefault();
    e.stopPropagation();
    setMenu({ x: e.clientX, y: e.clientY, items });
  }, []);
  const close = React.useCallback(() => setMenu(null), []);
  return { menu, open, close };
}

function ContextMenu({ menu, onClose }) {
  const [asking, setAsking] = React.useState(null);
  const ref = React.useRef(null);
  const [pos, setPos] = React.useState({ left: menu.x, top: menu.y });
  React.useLayoutEffect(() => {
    const r = ref.current?.getBoundingClientRect();
    if (!r) return;
    setPos({
      left: Math.max(8, Math.min(menu.x, window.innerWidth - r.width - 8)),
      top: Math.max(8, Math.min(menu.y, window.innerHeight - r.height - 8)),
    });
  }, [menu]);
  const pick = (it) => {
    if (it.confirm && asking !== it) { setAsking(it); return; }
    onClose();
    it.onClick?.();
  };
  React.useEffect(() => {
    const away = e => { if (!ref.current?.contains(e.target)) onClose(); };
    const keys = e => {
      if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); onClose(); return; }
      if (e.metaKey || e.ctrlKey || e.altKey) return;
      const it = menu.items.find(i => i !== "-" && i.key && i.key.toLowerCase() === e.key.toLowerCase());
      if (it) { e.preventDefault(); pick(it); }
    };
    document.addEventListener("mousedown", away, true);
    document.addEventListener("keydown", keys, true);
    window.addEventListener("blur", onClose);
    return () => {
      document.removeEventListener("mousedown", away, true);
      document.removeEventListener("keydown", keys, true);
      window.removeEventListener("blur", onClose);
    };
  }); // eslint-disable-line react-hooks/exhaustive-deps
  return (
    <div className="ctx-menu" role="menu" ref={ref} style={pos} onContextMenu={e => e.preventDefault()}>
      {menu.items.map((it, i) => it === "-"
        ? <div key={i} className="ctx-sep" role="separator" />
        : (
          <button key={i} type="button" role="menuitem" className={`ctx-item${it.danger ? " danger" : ""}`} onClick={() => pick(it)}>
            <span>{asking === it ? it.confirm : it.label}</span>
            {it.key && asking !== it && <span className="ctx-key">{it.key}</span>}
          </button>
        ))}
    </div>
  );
}

Object.assign(window, { ContextMenu, useContextMenu });
