/* librarian/librarian-window.jsx — the Librarian of one Academic project:
   a floating window with groups and tags on the left, the searchable list in
   the middle and one reference's fields on the right (docs/PLAN_LIBRARIAN.md,
   L1). The data is Rust's (library_* commands through `bridge.library`);
   what is shown and how it is filtered is app/librarian.js. */

function LibrarianWindow({ bridge, project, onClose }) {
  const T = window.OMP_I18N.t;
  const L = window.OMP_LIBRARIAN;
  const { Icon } = window;
  const [entries, setEntries] = React.useState([]);
  const [groups, setGroups] = React.useState([]);
  const [loaded, setLoaded] = React.useState(false);
  const [scope, setScope] = React.useState({ group: null, tag: null });
  const [query, setQuery] = React.useState("");
  const [sort, setSort] = React.useState({ key: "added", dir: "desc" });
  const [selectedId, setSelectedId] = React.useState(null);
  const [notice, setNotice] = React.useState("");
  const rootRef = React.useRef(null);
  window.useModalFocus(rootRef);
  const wc = window.useWindowChrome(onClose, rootRef);

  const call = React.useCallback((name, args) => bridge.library(name, project, args), [bridge, project]);
  const say = text => setNotice(text);

  const reload = React.useCallback(async () => {
    const [list, grp] = await Promise.all([call("list"), call("groups")]);
    if (list.ok) setEntries(list.value); else say(list.error);
    if (grp.ok) setGroups(grp.value);
    setLoaded(true);
  }, [call]);
  React.useEffect(() => { reload(); }, [reload]);

  React.useEffect(() => {
    const onKey = e => { if (e.key === "Escape") { e.preventDefault(); onClose(); } };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [onClose]);

  const replace = updated => setEntries(list => list.map(e => (e.id === updated.id ? updated : e)));
  const shown = React.useMemo(() => L.sorted(L.visible(entries, { ...scope, query }), sort.key, sort.dir), [entries, scope, query, sort]);
  const tags = React.useMemo(() => L.tagCounts(entries), [entries]);
  const selected = entries.find(e => e.id === selectedId) ?? null;

  const addReference = async () => {
    const res = await call("add", { csl: { type: "article-journal", title: T("New reference") } });
    if (!res.ok) return say(res.error);
    setEntries(list => [...list, res.value]);
    setScope({ group: null, tag: null }); setQuery(""); setSelectedId(res.value.id);
  };
  const importFile = async () => {
    const res = await call("import_pick");
    if (!res.ok) return say(res.error);
    if (res.value === null) return;
    say(T("Added {n}; {m} already in the library", { n: res.value.added.length, m: res.value.duplicates.length }));
    await reload();
  };
  const exportAs = async format => {
    const ids = shown.map(e => e.id);
    const res = await call("export_save", { format, ids });
    if (!res.ok) say(res.error); else if (res.value) say(T("Saved {n} references", { n: ids.length }));
  };
  const save = async (entry, csl, tagList, notes) => {
    let current = entry;
    if (L.formChanged(entry.csl, L.toForm(csl))) {
      const r = await call("update", { id: entry.id, csl });
      if (!r.ok) return r;
      current = r.value;
    }
    const m = await call("meta", { id: entry.id, tags: tagList, notes });
    if (!m.ok) return m;
    replace(m.value);
    return { ok: true };
  };
  const member = async (entry, group, on) => {
    const r = await call("member", { id: entry.id, group, member: on });
    if (!r.ok) return say(r.error);
    replace(r.value);
    setGroups((await call("groups")).value ?? groups);
  };
  const remove = async entry => {
    const r = await call("remove", { id: entry.id });
    if (!r.ok) return say(r.error);
    setEntries(list => list.filter(e => e.id !== entry.id));
    setSelectedId(null);
    setGroups((await call("groups")).value ?? groups);
  };
  const groupOp = async (name, args) => {
    const r = await call(name, args);
    if (r.ok) { const g = await call("groups"); if (g.ok) setGroups(g.value); }
    return r;
  };

  return (
    <div className="st-overlay" onMouseDown={e => { if (e.target === e.currentTarget) onClose(); }}>
      <div className="st-modal lb-modal" role="dialog" aria-label={T("Librarian")} ref={rootRef} tabIndex={-1}>
        <div className="st-modal-head" {...wc.headProps}>
          {wc.lights}
          <span className="st-modal-title">{T("Librarian")} · {project}</span>
          <span className="lb-head-tools">
            <button type="button" className="btn ghost" onClick={addReference}><Icon name="plus" size={11} /> {T("New reference")}</button>
            <button type="button" className="btn ghost" onClick={importFile}>{T("Import…")}</button>
            <select className="lb-select" aria-label={T("Export")} value="" disabled={shown.length === 0}
              onChange={e => { if (e.target.value) exportAs(e.target.value); }}>
              <option value="">{T("Export")}</option>
              <option value="bibtex">BibTeX (.bib)</option>
              <option value="ris">RIS (.ris)</option>
              <option value="csl-json">CSL-JSON (.json)</option>
            </select>
          </span>
        </div>
        {notice && <div className="lb-notice" role="status" onClick={() => setNotice("")}>{notice}</div>}
        <div className="lb-body">
          <LibSidebar total={entries.length} groups={groups} tags={tags} scope={scope} onScope={setScope}
            onGroupAdd={name => groupOp("group_add", { name })}
            onGroupRename={(id, name) => groupOp("group_rename", { id, name })}
            onGroupRemove={async id => { await groupOp("group_remove", { id }); if (scope.group === id) setScope({ group: null, tag: null }); await reload(); }} />
          <LibList entries={entries} shown={loaded ? shown : []} selectedId={selectedId} onSelect={setSelectedId}
            query={query} onQuery={setQuery} sort={sort} onSort={setSort} />
          {selected
            ? <LibDetail key={`${selected.id}:${selected.updatedAt}`} entry={selected} groups={groups}
                onSave={save} onMember={member} onDelete={remove} />
            : <section className="lb-detail lb-detail-empty">{T("Select a reference.")}</section>}
        </div>
      </div>
    </div>
  );
}

Object.assign(window, { LibrarianWindow });
