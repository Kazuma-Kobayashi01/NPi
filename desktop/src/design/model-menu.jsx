/* ═════════════════════════════════════════════════════════════════════
   model-menu.jsx — the small floating model picker next to the prompt box

   One button ("Sonnet 5.5  Ultra ⌄") that opens a card (after LabBot's):
   - the level card: the level in large type (the model's deepest is "Ultra",
     in a gradient), the model's name under it (a click lists the models),
     and a stepped slider with one stop per level the model supports;
   - Model ›: the favourite models (Settings → Models), each with its vendor
     in a small bordered tag, the current one ticked, the others numbered
     1-9 (those keys work on the card too), then More models › vendors, then
     that vendor's models, and Manage models….
   Used by the composer (a live tab), the start page (no session yet) and
   SideChat, which differ only in where models and levels come from, so both
   are props.
   ═════════════════════════════════════════════════════════════════════ */

const _mmLevel = (l) => window.OMP_MODELS.levelLabel(l);

function _MMChevron({ dir = "right", size = 14 }) {
  const d = { left: "M10 3.25 5.25 8 10 12.75", right: "M6 3.25 10.75 8 6 12.75", down: "M3.25 6 8 10.75 12.75 6" }[dir];
  return (
    <svg width={size} height={size} viewBox="0 0 16 16" fill="none" aria-hidden="true">
      <path d={d} stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

// Sparks drifting through the Ultra fill: fixed positions, so a render never moves them.
const _MM_SPARKS = Array.from({ length: 10 }, (_, k) => ({
  left: `${(k * 37 + 11) % 97}%`, top: `${20 + ((k * 53) % 60)}%`, delay: `${-(k * 0.37).toFixed(2)}s`, dur: `${(2.2 + (k % 4) * 0.4).toFixed(1)}s`,
}));

/** The level card. A drag shows its level and sends it once, on release
 *  (pointer, key or blur); a click on the stop in use sends it too, which
 *  pins a level `auto` resolved to. The thumb stays where it was dropped
 *  until omp reports the level (it may clamp it), or briefly when the level
 *  did not change. A model with fewer than two levels has no slider. */
function _MMCard({ levels, level, current, onSet, onModels }) {
  const M = window.OMP_MODELS;
  const T = window.OMP_I18N.t;
  const [drag, setDrag] = React.useState(null);
  const [sent, setSent] = React.useState(null);
  React.useEffect(() => { setSent(null); }, [level]);
  React.useEffect(() => {
    if (sent === null) return undefined;
    const t = setTimeout(() => setSent(null), 1500);
    return () => clearTimeout(t);
  }, [sent]);
  const list = levels ?? [];
  const shown = drag ?? sent ?? level;
  const { index, last, fraction, usable, ultra } = M.effortStops(list, shown);
  const commit = (l) => { setDrag(null); setSent(l); onSet?.(l); };
  const title = ultra ? "Ultra" : _mmLevel(shown);
  return (
    <div className="mm-card" data-lv={shown || undefined}>
      <button type="button" role="menuitem" className="mm-card-model" onClick={onModels} title={T("Model")}>
        {title && <span className={`mm-card-level${ultra ? " ultra" : ""}`}>{title}</span>}
        <span className="mm-card-name">
          <span className="mm-card-name-text">{current?.name ?? current?.id ?? T("Choose a model")}</span>
          <_MMChevron size={12} />
        </span>
      </button>
      {usable && (
        <div className={`mm-effort${index < 0 ? " unset" : ""}${ultra ? " ultra" : ""}`} style={{ "--mm-at": fraction }}>
          <div className="mm-effort-stops" aria-hidden="true">{list.map(l => <i key={l} />)}</div>
          {index >= 0 && (
            <div className="mm-effort-fill" aria-hidden="true">
              {ultra && _MM_SPARKS.map((s, k) => <i key={k} style={{ left: s.left, top: s.top, animationDelay: s.delay, animationDuration: s.dur }} />)}
            </div>
          )}
          <input type="range" min={0} max={last} step={1} value={Math.max(0, index)}
            aria-label={T("Effort")} aria-valuetext={title}
            onChange={e => setDrag(list[Number(e.target.value)])}
            onPointerUp={e => commit(list[Number(e.currentTarget.value)])}
            onKeyUp={() => { if (drag !== null) commit(drag); }}
            onBlur={() => { if (drag !== null) commit(drag); }} />
        </div>
      )}
    </div>
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
  const T = window.OMP_I18N.t;
  const [open, setOpen] = React.useState(false);
  const [view, setView] = React.useState("main"); // main | models | vendors | {vendor}
  const [levels, setLevels] = React.useState(null);
  const [recent, setRecent] = React.useState([]);
  const rootRef = React.useRef(null);
  const popRef = React.useRef(null);
  const levelsOf = React.useRef("");

  React.useLayoutEffect(() => {
    if (!open) return undefined;
    const fit = () => {
      const pop = popRef.current;
      pop.style.transform = "";
      const rect = pop.getBoundingClientRect();
      const shift = Math.max(12 - rect.left, Math.min(0, window.innerWidth - 12 - rect.right));
      pop.style.transform = `translateX(${shift}px)`;
    };
    fit();
    window.addEventListener("resize", fit);
    return () => window.removeEventListener("resize", fit);
  }, [open]);

  const close = React.useCallback(() => { setOpen(false); setView("main"); }, []);

  const currentKey = current ? M.keyOf(current) : "";

  // The levels follow the model: fetched when the model changes (the button says "Ultra" without
  // the menu open) and on every open. The recents change as models are used elsewhere.
  React.useEffect(() => {
    let live = true;
    if (levelsOf.current !== currentKey) { levelsOf.current = currentKey; setLevels(null); }
    Promise.resolve(loadLevels?.()).then(l => { if (live) setLevels(Array.isArray(l) ? l : []); }, () => { if (live) setLevels([]); });
    if (open) Promise.resolve(loadRecent?.()).then(r => { if (live && Array.isArray(r)) setRecent(r); }, () => {});
    return () => { live = false; };
  }, [open, currentKey]); // eslint-disable-line react-hooks/exhaustive-deps

  const picks = M.pickerModels(models ?? [], favorites, currentKey, recent);

  React.useEffect(() => {
    if (!open) return undefined;
    const away = e => { if (!rootRef.current?.contains(e.target)) close(); };
    const key = e => {
      if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); view === "main" ? close() : setView("main"); return; }
      if ((view === "main" || view === "models") && /^[1-9]$/.test(e.key) && !e.metaKey && !e.ctrlKey && !e.altKey) {
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
  if (view === "vendors") {
    const groups = M.groupByVendor(models ?? []);
    body = (<>
      {back("More models", "models")}
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
  } else if (view === "models") {
    body = (<>
      {back(T("Model"))}
      {picks.map((m, i) => row(m, M.keyOf(m) === currentKey ? tick : <span className="mm-num">{i + 1}</span>))}
      {picks.length === 0 && <div className="mm-empty">No models available. Add a vendor in Settings → Models.</div>}
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
  } else {
    body = <_MMCard levels={levels} level={level} current={current} onSet={onSetLevel} onModels={() => setView("models")} />;
  }

  const ultra = !!level && M.effortStops(levels, level).ultra;
  return (
    <div className="mm" ref={rootRef}>
      <button type="button" className={`mm-btn${open ? " on" : ""}`} aria-haspopup="menu" aria-expanded={open}
        title="Model and effort" onClick={() => { setOpen(v => !v); setView("main"); }}>
        <span className="mm-btn-name">{current?.name ?? current?.id ?? T("Choose a model")}</span>
        {level && <span className={`mm-btn-level${ultra ? " ultra" : ""}`}>{ultra ? "Ultra" : _mmLevel(level)}</span>}
        <span className="mm-btn-chev"><_MMChevron dir="down" size={10} /></span>
      </button>
      {open && (
        <div ref={popRef} className={`mm-pop mm-model-pop ${up ? "up" : "down"}`} role="menu">
          <div className="mm-view" key={typeof view === "object" ? `v:${view.vendor}` : view}>{body}</div>
        </div>
      )}
    </div>
  );
}

Object.assign(window, { ModelMenu });
