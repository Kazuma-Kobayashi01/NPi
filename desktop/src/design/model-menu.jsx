/* ═════════════════════════════════════════════════════════════════════
   model-menu.jsx — the small floating model picker next to the prompt box

   One button ("Sonnet 5.5  Medium") that opens a card with
   - the favourite models (Settings → Models), each with its vendor in a
     small bordered tag, the current one ticked, the others numbered 1-9;
   - Effort › the thinking level of the current model;
   - More models › vendors, then that vendor's models.
   Used by the composer (a live tab) and by the start page (no session yet),
   which differ only in where models and levels come from, so both are props.
   ═════════════════════════════════════════════════════════════════════ */

const { Icon: _MM_Icon } = window;

const _mmCap = (t) => (t ? String(t).charAt(0).toUpperCase() + String(t).slice(1) : "");

function _MMChevron({ dir = "right" }) {
  return (
    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path d={dir === "left" ? "M10 3.25 5.25 8 10 12.75" : "M6 3.25 10.75 8 6 12.75"}
        stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

/**
 * props
 *   models         [{ provider, id, name }]  everything that can be picked
 *   current        the model in use (or the one a new tab will start on)
 *   favorites      ["provider/id", …]  from Settings → Models
 *   level          the thinking level in use ("medium"), "" when unknown
 *   loadLevels()   → Promise<string[]>  the levels of the current model
 *   loadRecent()   → Promise<string[]>  recently used model keys (optional)
 *   onPick(model)  onSetLevel(level)  onManage()  (Settings → Models)
 *   up             open above the button (default) or below it
 */
function ModelMenu({ models, current, favorites, level, loadLevels, loadRecent, onPick, onSetLevel, onManage, up = true }) {
  const M = window.OMP_MODELS;
  const [open, setOpen] = React.useState(false);
  const [view, setView] = React.useState("main"); // main | effort | vendors | {vendor}
  const [levels, setLevels] = React.useState(null);
  const [recent, setRecent] = React.useState([]);
  const rootRef = React.useRef(null);

  const close = React.useCallback(() => { setOpen(false); setView("main"); }, []);

  // Fetched on every open: the levels follow the model, the recents change as
  // models are used elsewhere.
  React.useEffect(() => {
    if (!open) return undefined;
    let live = true;
    setLevels(null);
    Promise.resolve(loadLevels?.()).then(l => { if (live) setLevels(Array.isArray(l) ? l : []); }, () => { if (live) setLevels([]); });
    Promise.resolve(loadRecent?.()).then(r => { if (live && Array.isArray(r)) setRecent(r); }, () => {});
    return () => { live = false; };
  }, [open, current && M.keyOf(current)]); // eslint-disable-line react-hooks/exhaustive-deps

  const currentKey = current ? M.keyOf(current) : "";
  const picks = M.pickerModels(models ?? [], favorites, currentKey, recent);

  React.useEffect(() => {
    if (!open) return undefined;
    const away = e => { if (!rootRef.current?.contains(e.target)) close(); };
    const key = e => {
      if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); view === "main" ? close() : setView("main"); return; }
      if (view === "main" && /^[1-9]$/.test(e.key) && !e.metaKey && !e.ctrlKey && !e.altKey) {
        const m = picks[Number(e.key) - 1];
        if (m) { e.preventDefault(); onPick(m); close(); }
      }
    };
    document.addEventListener("mousedown", away);
    document.addEventListener("keydown", key, true);
    return () => { document.removeEventListener("mousedown", away); document.removeEventListener("keydown", key, true); };
  }, [open, view, picks.map(M.keyOf).join("|")]); // eslint-disable-line react-hooks/exhaustive-deps

  const row = (m, right) => (
    <button key={M.keyOf(m)} type="button" role="menuitem" className={`mm-row${M.keyOf(m) === currentKey ? " on" : ""}`}
      onClick={() => { onPick(m); close(); }}>
      <span className="mm-name">{m.name ?? m.id}</span>
      <span className="mm-vendor">{M.vendorLabel(m.provider)}</span>
      <span className="mm-right">{right}</span>
    </button>
  );
  const tick = <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true"><path d="m3.5 8.5 3 3 6-7" stroke="var(--accent)" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" /></svg>;
  const back = (label, to = "main") => (
    <button type="button" className="mm-back" onClick={() => setView(to)}>
      <_MMChevron dir="left" /><span>{label}</span>
    </button>
  );

  let body;
  if (view === "effort") {
    body = (<>
      {back("Effort")}
      {(levels ?? []).map(l => (
        <button key={l} type="button" role="menuitemradio" aria-checked={l === level} className={`mm-row${l === level ? " on" : ""}`}
          onClick={() => { onSetLevel?.(l); close(); }}>
          <span className="mm-name">{_mmCap(l)}</span>
          <span className="mm-right">{l === level ? tick : null}</span>
        </button>
      ))}
      {levels && levels.length === 0 && <div className="mm-empty">This model has no effort levels.</div>}
    </>);
  } else if (view === "vendors") {
    const groups = M.groupByVendor(models ?? []);
    body = (<>
      {back("More models")}
      {groups.map(g => (
        <button key={g.vendor} type="button" role="menuitem" className="mm-row nav" onClick={() => setView({ vendor: g.vendor })}>
          <span className="mm-name">{g.vendor}</span>
          <span className="mm-val">{g.models.length}</span>
          <_MMChevron />
        </button>
      ))}
      {groups.length === 0 && <div className="mm-empty">No models available. Add a vendor in Settings → Models.</div>}
    </>);
  } else if (typeof view === "object") {
    const group = M.groupByVendor(models ?? []).find(g => g.vendor === view.vendor);
    body = (<>
      {back(view.vendor, "vendors")}
      {(group?.models ?? []).map(m => row(m, M.keyOf(m) === currentKey ? tick : null))}
    </>);
  } else {
    body = (<>
      {picks.map((m, i) => row(m, M.keyOf(m) === currentKey ? tick : <span className="mm-num">{i + 1}</span>))}
      {picks.length === 0 && <div className="mm-empty">No models available. Add a vendor in Settings → Models.</div>}
      {level && levels && levels.length > 0 && (<>
        <div className="mm-sep" />
        <button type="button" role="menuitem" className="mm-row nav" onClick={() => setView("effort")}>
          <span className="mm-name">Effort</span>
          <span className="mm-val">{_mmCap(level)}</span>
          <_MMChevron />
        </button>
      </>)}
      <div className="mm-sep" />
      <button type="button" role="menuitem" className="mm-row nav" onClick={() => setView("vendors")}>
        <span className="mm-name">More models</span>
        <_MMChevron />
      </button>
      {onManage && (
        <button type="button" role="menuitem" className="mm-row nav" onClick={() => { close(); onManage(); }}>
          <span className="mm-name">Manage models…</span>
        </button>
      )}
    </>);
  }

  return (
    <div className="mm" ref={rootRef}>
      <button type="button" className={`mm-btn${open ? " on" : ""}`} aria-haspopup="menu" aria-expanded={open}
        title="Model and effort" onClick={() => { setOpen(v => !v); setView("main"); }}>
        <span className="mm-btn-name">{current?.name ?? current?.id ?? "Choose a model"}</span>
        {level && <span className="mm-btn-level">{_mmCap(level)}</span>}
      </button>
      {open && <div className={`mm-pop ${up ? "up" : "down"}`} role="menu">{body}</div>}
    </div>
  );
}

Object.assign(window, { ModelMenu });
