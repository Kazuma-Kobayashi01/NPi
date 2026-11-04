/* librarian/lib-sidebar.jsx — the left column of the Librarian window: the
   whole library, the manual groups (add / rename / remove) and the tags. */

function LibSidebar({ dropGroup, total, smart, groups, tags, scope, onScope, onGroupAdd, onGroupRename, onGroupRemove }) {
  const T = window.OMP_I18N.t;
  const { Icon } = window;
  const [edit, setEdit] = React.useState(null);   // { id | null (new), draft }
  const [armed, setArmed] = React.useState(null);
  const [error, setError] = React.useState("");

  const commit = async () => {
    const name = edit.draft.trim();
    if (!name) { setEdit(null); return; }
    const res = edit.id === null ? await onGroupAdd(name) : await onGroupRename(edit.id, name);
    if (res?.ok === false) setError(res.error); else { setEdit(null); setError(""); }
  };
  const isAll = scope.group === null && scope.tag === null && !scope.smart;
  const SMART = [["incomplete", T("Missing metadata")], ["nofile", T("No PDF")], ["duplicates", T("Possible duplicates")], ["suggestions", T("Suggestions waiting")]];
  const row = (on, onClick, label, count, extra, groupId) => (
    <div className={`lb-nav${on ? " on" : ""}${groupId && dropGroup === groupId ? " drop" : ""}`} data-group={groupId || undefined}>
      <button type="button" className="lb-nav-main" onClick={onClick}>
        <span className="lb-nav-label">{label}</span><span className="lb-nav-count">{count}</span>
      </button>
      {extra}
    </div>
  );

  return (
    <nav className="lb-side" aria-label={T("Librarian")}>
      {row(isAll, () => onScope({ group: null, tag: null }), T("All references"), total)}
      {SMART.filter(([id]) => smart[id] > 0 || scope.smart === id).map(([id, label]) => (
        <React.Fragment key={id}>{row(scope.smart === id, () => onScope({ group: null, tag: null, smart: id }), label, smart[id])}</React.Fragment>
      ))}
      <div className="lb-side-head">
        <span>{T("Groups")}</span>
        <button type="button" className="btn icon ghost" title={T("New group")} onClick={() => { setEdit({ id: null, draft: "" }); setError(""); }}>
          <Icon name="plus" size={11} />
        </button>
      </div>
      {groups.map(g => edit?.id === g.id ? (
        <input key={g.id} className="lb-input" autoFocus maxLength={60} value={edit.draft}
          onChange={e => setEdit({ id: g.id, draft: e.target.value })} onBlur={commit}
          onKeyDown={e => { if (window.isSubmitEnter(e)) { e.preventDefault(); commit(); } if (e.key === "Escape") { e.stopPropagation(); setEdit(null); } }} />
      ) : row(scope.group === g.id, () => onScope({ group: g.id, tag: null }), g.name, g.count,
        <span className="lb-nav-tools">
          <button type="button" className="btn icon ghost" title={T("Rename")} onClick={() => { setEdit({ id: g.id, draft: g.name }); setError(""); }}><Icon name="edit" size={10} /></button>
          <button type="button" className={`btn icon ghost${armed === g.id ? " armed" : ""}`}
            title={armed === g.id ? T("Click again: the references stay, the group goes") : T("Delete")}
            onClick={() => { if (armed === g.id) { setArmed(null); onGroupRemove(g.id); } else setArmed(g.id); }}>
            {armed === g.id ? <span className="mono">{T("Delete?")}</span> : <Icon name="trash" size={10} />}
          </button>
        </span>, g.id))}
      {edit?.id === null && (
        <input className="lb-input" autoFocus maxLength={60} placeholder={T("Group name")} value={edit.draft}
          onChange={e => setEdit({ id: null, draft: e.target.value })} onBlur={commit}
          onKeyDown={e => { if (window.isSubmitEnter(e)) { e.preventDefault(); commit(); } if (e.key === "Escape") { e.stopPropagation(); setEdit(null); } }} />
      )}
      {error && <div className="lb-error" role="alert">{error}</div>}
      {tags.length > 0 && <div className="lb-side-head"><span>{T("Tags")}</span></div>}
      {tags.map(t => row(scope.tag !== null && scope.tag.toLowerCase() === t.tag.toLowerCase(), () => onScope({ group: null, tag: t.tag }), t.tag, t.count))}
    </nav>
  );
}

Object.assign(window, { LibSidebar });
