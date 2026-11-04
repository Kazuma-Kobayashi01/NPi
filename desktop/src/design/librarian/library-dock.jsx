/* librarian/library-dock.jsx — the literature column at the far right of an Academic project:
   the project's references with a search box, a group filter and, per reference, what is needed while
   writing: put its citation key into the prompt, open it in the Librarian, or open its PDF in the Reader. It only reads
   (`bridge.library`, the Librarian's own commands); everything that changes the library stays in the
   Librarian window. The search is the project's full-text index when there is one (the Librarian
   window's `search` command), else a substring filter (app/librarian.js). */

function LibraryDock({ bridge, project, onInsert, onOpenLibrarian, onOpenReader, onClose }) {
  const T = window.OMP_I18N.t;
  const L = window.OMP_LIBRARIAN;
  const { Icon } = window;
  const [entries, setEntries] = React.useState([]);
  const [groups, setGroups] = React.useState([]);
  const [group, setGroup] = React.useState("");
  const [query, setQuery] = React.useState("");
  const [ranked, setRanked] = React.useState(null);
  const [open, setOpen] = React.useState(null);
  const [error, setError] = React.useState("");
  const [loaded, setLoaded] = React.useState(false);

  const reload = React.useCallback(async () => {
    const [list, grp] = await Promise.all([bridge.library("list", project), bridge.library("groups", project)]);
    if (list.ok) { setEntries(list.value); setError(""); } else setError(list.error);
    if (grp.ok) setGroups(grp.value);
    setLoaded(true);
  }, [bridge, project]);

  React.useEffect(() => {
    setEntries([]); setLoaded(false); setOpen(null);
    reload();
    // The Librarian window changes the library from another window: look again when this one is back in front.
    const again = () => reload();
    window.addEventListener("focus", again);
    return () => window.removeEventListener("focus", again);
  }, [reload]);

  React.useEffect(() => {
    const q = query.trim();
    if (!q) { setRanked(null); return undefined; }
    let live = true;
    const timer = setTimeout(async () => {
      const res = await bridge.library("search", project, { query: q });
      if (live) setRanked(res.ok && Array.isArray(res.value) ? res.value : null);
    }, 160);
    return () => { live = false; clearTimeout(timer); };
  }, [query, bridge, project, entries]);

  const shown = React.useMemo(() => {
    const scope = { group: group ? Number(group) : null, tag: null };
    const plain = L.sorted(L.visible(entries, { ...scope, query }), "added", "desc");
    if (!ranked || !query.trim()) return plain;
    const inScope = new Map(L.visible(entries, { ...scope, query: "" }).map(e => [e.id, e]));
    const byRank = ranked.map(id => inScope.get(id)).filter(Boolean);
    const seen = new Set(byRank.map(e => e.id));
    return [...byRank, ...plain.filter(e => !seen.has(e.id))];
  }, [entries, group, query, ranked, L]);

  return (
    <aside className="libdock" aria-label={T("Library")}>
      <div className="libdock-head">
        <span className="libdock-title">{T("Library")}</span>
        <span className="libdock-count">{entries.length}</span>
        <span className="libdock-gap" />
        <button type="button" className="sc-ibtn" title={T("Open the Librarian")} onClick={onOpenLibrarian}><Icon name="layers" size={13} /></button>
        <button type="button" className="sc-ibtn" title={T("Open the reader")} onClick={() => onOpenReader()}><Icon name="file" size={13} /></button>
        <button type="button" className="sc-ibtn" title={T("Close")} aria-label={T("Close")} onClick={onClose}><Icon name="close" size={12} /></button>
      </div>
      <div className="libdock-tools">
        <input className="lb-input" value={query} onChange={e => setQuery(e.target.value)} placeholder={T("Search references")} aria-label={T("Search references")} />
        {groups.length > 0 && (
          <select className="lb-select" value={group} onChange={e => setGroup(e.target.value)} aria-label={T("Group")}>
            <option value="">{T("All references")}</option>
            {groups.map(g => <option key={g.id} value={g.id}>{g.name}</option>)}
          </select>
        )}
      </div>
      {error && <div className="lb-notice" onClick={() => setError("")}>{error}</div>}
      <div className="libdock-list">
        {loaded && entries.length === 0 && !error && <div className="rd-none">{T("No references yet. Add some in the Librarian.")}</div>}
        {loaded && entries.length > 0 && shown.length === 0 && <div className="rd-none">{T("No matches")}</div>}
        {shown.map(e => {
          const d = L.display(e);
          const on = open === e.id;
          const pdf = (e.attachments ?? []).find(a => !a.missing);
          return (
            <div key={e.id} className={`libdock-row${on ? " on" : ""}`}>
              <button type="button" className="libdock-main" onClick={() => setOpen(on ? null : e.id)} aria-expanded={on}>
                <span className="libdock-ref">{d.title}</span>
                <span className="libdock-meta">{[d.authors, d.year].filter(Boolean).join(" · ")}</span>
              </button>
              {on && (
                <div className="libdock-more">
                  {d.container && <div className="libdock-meta">{d.container}</div>}
                  {e.csl?.DOI && <div className="libdock-meta">DOI {e.csl.DOI}</div>}
                  {(e.tags ?? []).length > 0 && <div className="libdock-tags">{e.tags.map(t => <span key={t} className="rd-badge">{t}</span>)}</div>}
                  <div className="libdock-actions">
                    <button type="button" className="btn ghost" onClick={() => onInsert(`[@${e.citekey}]`)}>{T("Cite in prompt")}</button>
                    <button type="button" className="btn ghost" onClick={onOpenLibrarian}>{T("Open in Librarian")}</button>
                    {pdf && <button type="button" className="btn ghost" onClick={() => onOpenReader(pdf.path)}>{T("Open in Reader")}</button>}
                  </div>
                  <code className="libdock-key">{e.citekey}</code>
                </div>
              )}
            </div>
          );
        })}
      </div>
    </aside>
  );
}
window.LibraryDock = LibraryDock;
