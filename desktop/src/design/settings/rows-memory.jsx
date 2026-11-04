/* design/settings/rows-memory.jsx — Settings → Chat's custom row `memory` (window.OMP_SETTINGS_ROWS): the
   switch, and every note the model keeps, by kind, to edit or delete. All of it is Rust's
   (src-tauri/src/memory/). A change reaches a session when it next starts. */

function _SRMem_Note({ note, onSave, onDelete }) {
  const T = key => window.OMP_I18N.t(key);
  const M = window.OMP_MEMORY;
  const [draft, setDraft] = React.useState(null);
  const [armed, setArmed] = React.useState(false);
  const age = M.age(note.updatedAt, Date.now() / 1000);
  const when = window.OMP_I18N.t(age.key, { n: age.n });
  const save = async () => { if (await onSave(note.id, draft)) setDraft(null); };
  return (
    <li className="st-mem-note">
      <span className="st-mem-title">{note.title}</span>
      {draft === null
        ? <button type="button" className="st-mem-text selectable" title={T("Edit")} onClick={() => setDraft(note.text)}>{note.text}</button>
        : <textarea className="st-textarea selectable" rows={2} autoFocus maxLength={300} value={draft} aria-label={note.title} onChange={e => setDraft(e.target.value)}
            onKeyDown={e => { if (e.key === "Escape") { e.stopPropagation(); setDraft(null); } if (window.isSubmitEnter(e)) { e.preventDefault(); save(); } }} onBlur={save} />}
      <span className="st-mem-when">{when}</span>
      <button type="button" className={`btn ghost${armed ? " armed" : ""}`} onClick={() => { if (armed) onDelete(note.id); else setArmed(true); }} onBlur={() => setArmed(false)}>{armed ? T("Delete?") : T("Delete")}</button>
    </li>
  );
}

function _SRMem_Memory({ row, ctx, query }) {
  const { SettingsRow: Row, SettingsToggle: Toggle, SettingsButton: Button } = window;
  const T = key => window.OMP_I18N.t(key);
  const [state, setState] = React.useState(null);
  const [error, setError] = React.useState("");
  const [armed, setArmed] = React.useState(false);
  const call = React.useCallback(async (name, args) => {
    const res = await ctx.bridge.memory(name, args);
    if (res.ok) { setState(res.value); setError(""); } else setError(res.error);
    return res.ok;
  }, [ctx.bridge]);
  React.useEffect(() => { call("state"); }, [call]);
  const groups = window.OMP_MEMORY.grouped(state?.notes);
  return (
    <Row stack label={row.label} desc={row.desc} query={query}>
      <div className="st-mem">
        <Toggle value={state?.enabled ?? true} label={row.label} onChange={on => call("set_enabled", { on })} />
        {error && <div className="lb-error" role="alert">{error}</div>}
        {state && groups.length === 0 && <div className="st-fav-empty">{T("Nothing yet. Notes appear here as you talk.")}</div>}
        {groups.map(g => (
          <section key={g.kind}>
            <h4 className="st-mem-kind">{T(g.label)}</h4>
            <ul className="st-mem-list">
              {g.notes.map(n => <_SRMem_Note key={`${n.id}:${n.updatedAt}`} note={n} onSave={(id, text) => call("update", { id, text })} onDelete={id => call("delete", { id })} />)}
            </ul>
          </section>
        ))}
        <div className="st-chattext-foot">
          {state?.notes.length > 0 && <Button onClick={() => { if (armed) { setArmed(false); call("clear"); } else setArmed(true); }}>{armed ? T("Delete every note?") : T("Delete all")}</Button>}
          {state && <span className="st-fav-empty selectable">{state.path}</span>}
        </div>
      </div>
    </Row>
  );
}

Object.assign(window.OMP_SETTINGS_ROWS = window.OMP_SETTINGS_ROWS || {}, { memory: _SRMem_Memory });
