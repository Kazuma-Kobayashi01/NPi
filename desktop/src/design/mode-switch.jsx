/* ═════════════════════════════════════════════════════════════════
   mode-switch.jsx — the Chat | Code | Academic switch at the top of the
   left sidebar. A header showing the current mode with a chevron that opens a
   menu of the three (title + one-line hint, a check on the current), like a
   workspace picker; arrow keys / Home / End move, Enter picks, Escape closes.
   `mode` is the app mode ("chat" | "academic"; anything else is Code),
   `onMode(id)` the same callback the old top-bar buttons used.
   ═════════════════════════════════════════════════════════════════ */

// The UI language (app/i18n.js).
const _MS_T = (key, vars) => window.OMP_I18N.t(key, vars);
function ChatGlyph() {
  return (
    <svg width="15" height="15" viewBox="0 0 18 18" fill="none" aria-hidden="true">
      <path d="M9 2.75c-3.45 0-6.25 2.4-6.25 5.4 0 1.5.7 2.85 1.85 3.85L4 14.75l2.7-1.1c.7.2 1.5.3 2.3.3 3.45 0 6.25-2.4 6.25-5.4S12.45 2.75 9 2.75Z"
        stroke="currentColor" strokeWidth="1.4" strokeLinejoin="round" />
    </svg>
  );
}
function CodeGlyph() {
  return (
    <svg width="15" height="15" viewBox="0 0 18 18" fill="none" aria-hidden="true">
      <path d="M6.5 5.5 3 9l3.5 3.5M11.5 5.5 15 9l-3.5 3.5M10.2 4 7.8 14"
        stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}
function AcademicGlyph() {
  return (
    <svg width="15" height="15" viewBox="0 0 18 18" fill="none" aria-hidden="true">
      <path d="M9 3.5 1.75 7 9 10.5 16.25 7 9 3.5Z" stroke="currentColor" strokeWidth="1.4" strokeLinejoin="round" />
      <path d="M5 9v3c0 1.1 1.8 2 4 2s4-.9 4-2V9M16.25 7v4.25"
        stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

const _MS_MODES = [
  { id: "chat", label: "Chat", hint: "Ask, write, and explore", Glyph: ChatGlyph },
  { id: "code", label: "Code", hint: "Build, debug, and ship", Glyph: CodeGlyph },
  { id: "academic", label: "Academic", hint: "Read, search, and organize research", Glyph: AcademicGlyph },
];

function ModeSwitch({ mode, onMode }) {
  const current = mode === "chat" || mode === "academic" ? mode : "code";
  const cur = _MS_MODES.find(m => m.id === current);
  const [open, setOpen] = React.useState(false);
  const rootRef = React.useRef(null);
  const items = React.useRef([]);
  const close = (refocus) => { setOpen(false); if (refocus) rootRef.current?.querySelector(".mode-switch-head")?.focus(); };

  React.useEffect(() => {
    if (!open) return undefined;
    items.current[_MS_MODES.findIndex(m => m.id === current)]?.focus();
    const away = (e) => { if (!rootRef.current?.contains(e.target)) setOpen(false); };
    document.addEventListener("mousedown", away);
    return () => document.removeEventListener("mousedown", away);
  }, [open, current]);

  const choose = (id) => { close(true); if (id !== current) onMode?.(id); };
  const onMenuKey = (e) => {
    const at = items.current.indexOf(document.activeElement);
    const go = (i) => { e.preventDefault(); items.current[(i + _MS_MODES.length) % _MS_MODES.length]?.focus(); };
    if (e.key === "ArrowDown") go(at + 1);
    else if (e.key === "ArrowUp") go(at - 1);
    else if (e.key === "Home") go(0);
    else if (e.key === "End") go(_MS_MODES.length - 1);
    else if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); close(true); }
    else if (e.key === "Tab") setOpen(false);
  };
  return (
    <div className={`mode-switch${open ? " is-open" : ""}`} ref={rootRef}>
      <button type="button" className="mode-switch-head" aria-haspopup="menu" aria-expanded={open}
        title={_MS_T("Mode")} onClick={() => setOpen(v => !v)}
        onKeyDown={e => { if (e.key === "ArrowDown" && !open) { e.preventDefault(); setOpen(true); } }}>
        <cur.Glyph /><span className="mode-switch-name">{_MS_T(cur.label)}</span>
        <svg className="mode-switch-chev" width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
          <path d="M2 3.5 5 6.5 8 3.5" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
      </button>
      {open && (
        <div className="mode-switch-menu" role="menu" aria-label={_MS_T("Mode")} onKeyDown={onMenuKey}>
          {_MS_MODES.map(({ id, label, hint, Glyph }, i) => (
            <button key={id} type="button" role="menuitemradio" aria-checked={id === current}
              ref={el => { items.current[i] = el; }} tabIndex={id === current ? 0 : -1}
              className={`mode-switch-item${id === current ? " on" : ""}`} onClick={() => choose(id)}>
              <span className="mode-switch-text">
                <span className="mode-switch-title">{_MS_T(label)}</span>
                <span className="mode-switch-hint">{_MS_T(hint)}</span>
              </span>
              {id === current && (
                <svg className="mode-switch-check" width="14" height="14" viewBox="0 0 14 14" aria-hidden="true">
                  <path d="M2.5 7.5 5.5 10.5 11.5 3.5" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" />
                </svg>
              )}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

Object.assign(window, { ModeSwitch });
