/* librarian/lib-batch.jsx — the right column while several references are selected: what can be done to
   them together. Two selected can be merged, which keeps the one the user picks. */

/** What the window does to the selected references together. */
function useLibBatch({ call, chosen, replace, setEntries, setSelection, refreshGroups, say }) {
  const T = window.OMP_I18N.t;
  const group = async id => {
    for (const e of chosen) {
      const r = await call("member", { id: e.id, group: id, member: true });
      if (!r.ok) return say(r.error);
      replace(r.value);
    }
    await refreshGroups();
    return say(T("Added {n} to the group", { n: chosen.length }));
  };
  const tag = async text => {
    for (const e of chosen) {
      const r = await call("meta", { id: e.id, tags: [...e.tags, text], notes: e.notes });
      if (!r.ok) return say(r.error);
      replace(r.value);
    }
    return say(T("Tagged {n} with {tag}", { n: chosen.length, tag: text }));
  };
  const remove = async () => {
    for (const e of chosen) {
      const r = await call("remove", { id: e.id });
      if (!r.ok) return say(r.error);
    }
    const gone = new Set(chosen.map(e => e.id));
    setEntries(list => list.filter(e => !gone.has(e.id)));
    setSelection({ ids: [], anchor: null });
    say(T("Moved {n} to the trash", { n: gone.size }));
    return refreshGroups();
  };
  return { group, tag, remove };
}

function LibBatch({ entries, groups, onGroup, onTag, onAlign, onMerge, onRemove }) {
  const T = window.OMP_I18N.t;
  const L = window.OMP_LIBRARIAN;
  const [tag, setTag] = React.useState("");
  const [armed, setArmed] = React.useState(false);
  const [keep, setKeep] = React.useState(entries[0]?.id ?? null);
  const pair = entries.length === 2;
  const submitTag = e => { e.preventDefault(); const t = tag.trim(); if (t) { onTag(t); setTag(""); } };
  return (
    <section className="lb-detail lb-batch" aria-label={T("{n} selected", { n: entries.length })}>
      <div className="lb-detail-scroll">
        <h3 className="lb-batch-title">{T("{n} selected", { n: entries.length })}</h3>
        <ul className="lb-batch-list">
          {entries.slice(0, 12).map(e => <li key={e.id}>{L.display(e).title}</li>)}
          {entries.length > 12 && <li>…</li>}
        </ul>
        <div className="lb-field"><span>{T("Metadata")}</span>
          <div><button type="button" className="btn ghost" onClick={onAlign}>{T("Align metadata…")}</button></div>
        </div>
        {groups.length > 0 && (
          <label className="lb-field"><span>{T("Add to a group")}</span>
            <select className="lb-select" value="" onChange={e => { if (e.target.value) onGroup(Number(e.target.value)); }}>
              <option value="">{T("Choose a group…")}</option>
              {groups.map(g => <option key={g.id} value={g.id}>{g.name}</option>)}
            </select>
          </label>
        )}
        <form className="lb-field" onSubmit={submitTag}><span>{T("Add a tag")}</span>
          <input value={tag} onChange={e => setTag(e.target.value)} placeholder={T("Tag, then Enter")} />
        </form>
        {pair && (
          <div className="lb-field"><span>{T("Merge these two")}</span>
            {entries.map(e => (
              <label key={e.id} className="lb-check lb-merge-row">
                <input type="radio" name="lb-keep" checked={keep === e.id} onChange={() => setKeep(e.id)} />
                {T("Keep {key}", { key: e.citekey })}
              </label>
            ))}
            <div><button type="button" className="btn ghost" onClick={() => onMerge(keep, entries.find(e => e.id !== keep).id)}>{T("Merge")}</button></div>
            <small className="lb-state">{T("Missing fields, tags, groups, files and notes move to the one kept; the other goes to the trash.")}</small>
          </div>
        )}
      </div>
      <div className="lb-detail-foot">
        <span className="lb-state" />
        <button type="button" className={`btn ghost lb-del${armed ? " armed" : ""}`} onBlur={() => setArmed(false)}
          onClick={() => { if (armed) onRemove(); else setArmed(true); }}>
          {armed ? T("Delete?") : T("Delete")}
        </button>
      </div>
    </section>
  );
}

Object.assign(window, { LibBatch, useLibBatch });
