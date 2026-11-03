/* librarian/lib-detail.jsx — the right column: one reference's fields, its
   groups, tags and notes. Edits stay in the form until Save; a group tick
   applies at once. The parent re-keys it by id and update time, so a saved or
   switched reference starts from what is stored. */

function LibDetail({ entry, groups, onSave, onMember, onDelete }) {
  const T = window.OMP_I18N.t;
  const L = window.OMP_LIBRARIAN;
  const [form, setForm] = React.useState(() => L.toForm(entry.csl));
  const [tags, setTags] = React.useState(() => entry.tags.join(", "));
  const [notes, setNotes] = React.useState(entry.notes);
  const [armed, setArmed] = React.useState(false);
  const [state, setState] = React.useState("");   // "" | "saving" | an error | "copied"
  const set = key => e => setForm(f => ({ ...f, [key]: e.target.value }));

  const dirty = L.formChanged(entry.csl, form) || notes !== entry.notes
    || JSON.stringify(L.parseTags(tags)) !== JSON.stringify(entry.tags);
  const save = async () => {
    setState("saving");
    const res = await onSave(entry, L.fromForm(entry.csl, form), L.parseTags(tags), notes);
    if (res?.ok === false) setState(res.error ?? T("could not save")); else setState("");
  };
  const copy = async () => {
    try { await navigator.clipboard.writeText(L.plainCitation(entry)); setState("copied"); } catch { setState(T("could not copy")); }
  };
  const field = (key, label, props = {}) => (
    <label className="lb-field"><span>{T(label)}</span><input value={form[key]} onChange={set(key)} {...props} /></label>
  );

  return (
    <section className="lb-detail" aria-label={T("Reference")}>
      <div className="lb-detail-scroll">
        <label className="lb-field"><span>{T("Type")}</span>
          <select value={form.type} onChange={set("type")}>
            {L.TYPES.map(([id, label]) => <option key={id} value={id}>{T(label)}</option>)}
            {!L.TYPES.some(([id]) => id === form.type) && <option value={form.type}>{form.type}</option>}
          </select>
        </label>
        <label className="lb-field"><span>{T("Title")}</span><textarea rows={2} value={form.title} onChange={set("title")} /></label>
        <label className="lb-field"><span>{T("Authors (one per line, Family, Given)")}</span>
          <textarea rows={3} value={form.authors} onChange={set("authors")} /></label>
        <div className="lb-field-row">
          {field("year", "Year", { inputMode: "numeric", maxLength: 4 })}
          {field("volume", "Volume")}
          {field("issue", "Issue")}
          {field("page", "Pages")}
        </div>
        {field("container", "Journal / book / conference")}
        {field("publisher", "Publisher")}
        {field("doi", "DOI")}
        {field("url", "URL")}
        <label className="lb-field"><span>{T("Abstract")}</span><textarea rows={4} value={form.abstract} onChange={set("abstract")} /></label>

        <div className="lb-field"><span>{T("Citation key")}</span>
          <code className="lb-key">{entry.citekey}</code></div>
        {groups.length > 0 && (
          <div className="lb-field"><span>{T("Groups")}</span>
            <div className="lb-checks">
              {groups.map(g => (
                <label key={g.id} className="lb-check">
                  <input type="checkbox" checked={entry.groups.includes(g.id)} onChange={e => onMember(entry, g.id, e.target.checked)} />
                  {g.name}
                </label>
              ))}
            </div>
          </div>
        )}
        <label className="lb-field"><span>{T("Tags (comma separated)")}</span><input value={tags} onChange={e => setTags(e.target.value)} /></label>
        <label className="lb-field"><span>{T("Notes")}</span><textarea rows={5} value={notes} onChange={e => setNotes(e.target.value)} /></label>
      </div>
      <div className="lb-detail-foot">
        <button type="button" className="btn outlined accent" disabled={!dirty || state === "saving"} onClick={save}>{T("Save")}</button>
        <button type="button" className="btn ghost" disabled={!dirty} onClick={() => { setForm(L.toForm(entry.csl)); setTags(entry.tags.join(", ")); setNotes(entry.notes); }}>{T("Revert")}</button>
        <button type="button" className="btn ghost" onClick={copy}>{T("Copy citation")}</button>
        <span className="lb-state" role="status">{state === "copied" ? T("Copied") : state === "saving" ? "" : state}</span>
        <button type="button" className={`btn ghost lb-del${armed ? " armed" : ""}`}
          title={T("Moves it out of the library")}
          onClick={() => { if (armed) onDelete(entry); else setArmed(true); }} onBlur={() => setArmed(false)}>
          {armed ? T("Delete?") : T("Delete")}
        </button>
      </div>
    </section>
  );
}

Object.assign(window, { LibDetail });
