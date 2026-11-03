/* librarian/lib-detail.jsx — the right column: one reference's fields, its
   groups, tags and notes. Edits stay in the form until Save; a group tick
   applies at once. The parent re-keys it by id and update time, so a saved or
   switched reference starts from what is stored. */

function LibDetail({ entry, groups, draft, onSave, onMember, onDelete, onCite, onLookup, onAttach, onOpenAttachment, onRemoveAttachment, onAttachDuplicate, onDiscardDraft }) {
  const T = window.OMP_I18N.t;
  const L = window.OMP_LIBRARIAN;
  const isDraft = Boolean(draft);
  const [form, setForm] = React.useState(() => L.toForm(entry.csl));
  const [tags, setTags] = React.useState(() => entry.tags.join(", "));
  const [notes, setNotes] = React.useState(entry.notes);
  const [armed, setArmed] = React.useState(false);
  const [state, setState] = React.useState("");   // "" | "saving" | an error | "copied"
  const [lookup, setLookup] = React.useState(null); // null | { busy } | { error } | { candidates, picked, keys }
  const guess = React.useMemo(() => L.guessedFields(draft), [draft]);
  const set = key => e => setForm(f => ({ ...f, [key]: e.target.value }));

  const dirty = isDraft || L.formChanged(entry.csl, form) || notes !== entry.notes
    || JSON.stringify(L.parseTags(tags)) !== JSON.stringify(entry.tags);
  const save = async () => {
    setState("saving");
    const res = await onSave(entry, L.fromForm(entry.csl, form), L.parseTags(tags), notes);
    if (res?.ok === false) setState(res.error ?? T("could not save")); else setState("");
  };
  const copy = async kind => { if (await onCite([entry.id], kind)) setState("copied"); };
  const field = (key, label, props = {}) => (
    <label className={`lb-field${guess.has(key) ? " lb-guess" : ""}`}><span>{T(label)}{guess.has(key) && <em> · {T("check")}</em>}</span>
      <input value={form[key]} onChange={set(key)} {...props} /></label>
  );

  const runLookup = async () => {
    setLookup({ busy: true });
    const first = form.authors.split("\n")[0] ?? "";
    const author = L.parsePerson(first).family ?? "";
    const res = await onLookup({ doi: form.doi.trim(), title: form.title.trim(), author });
    if (!res.ok) return setLookup({ error: res.error });
    const candidates = res.value.map(c => c.csl);
    const picked = 0;
    setLookup({ candidates, picked, keys: new Set(L.lookupDiff(form, candidates[picked]).filter(r => r.differs).map(r => r.key)) });
  };
  const pickCandidate = index => setLookup(l => ({ ...l, picked: index,
    keys: new Set(L.lookupDiff(form, l.candidates[index]).filter(r => r.differs).map(r => r.key)) }));
  const toggleKey = key => setLookup(l => { const keys = new Set(l.keys); if (keys.has(key)) keys.delete(key); else keys.add(key); return { ...l, keys }; });
  const applyLookup = () => { setForm(f => L.applyLookup(f, lookup.candidates[lookup.picked], [...lookup.keys])); setLookup(null); };

  return (
    <section className="lb-detail" aria-label={T("Reference")}>
      <div className="lb-detail-scroll">
        {isDraft && (
          <div className="lb-draft" role="status">
            <strong>{T("New from a PDF")}</strong>
            <span>{draft.scanned
              ? T("This PDF has no text layer, so nothing could be read from it. Fill the fields in by hand.")
              : T("Read from page 1 of {name}. Check the marked fields before adding.", { name: draft.name })}</span>
            {draft.duplicateOf && (
              <span className="lb-draft-dup">{T("Already in the library as {key}.", { key: draft.duplicateOf.citekey })}{" "}
                <button type="button" className="btn ghost" onClick={() => onAttachDuplicate(draft.duplicateOf)}>{T("Attach the PDF to it")}</button></span>
            )}
          </div>
        )}
        <div className="lb-lookup-bar">
          <button type="button" className="btn ghost" disabled={lookup?.busy} onClick={runLookup}
            title={T("Sends the DOI, or the title and first author's family name, to Crossref. Nothing else leaves the app.")}>
            {lookup?.busy ? T("Looking up…") : T("Look up online…")}
          </button>
          <span className="lb-state">{T("Only when you click · Crossref")}</span>
        </div>
        {lookup?.error && <div className="lb-lookup lb-lookup-error" role="alert">{lookup.error}<button type="button" className="btn ghost" onClick={() => setLookup(null)}>{T("Close")}</button></div>}
        {lookup?.candidates && (
          <div className="lb-lookup" role="group" aria-label={T("Online result")}>
            {lookup.candidates.length > 1 && (
              <div className="lb-lookup-cands">
                {lookup.candidates.map((c, i) => (
                  <button key={i} type="button" className={`lb-lookup-cand${i === lookup.picked ? " on" : ""}`} onClick={() => pickCandidate(i)}>
                    {(c.title ?? "").slice(0, 80)}{c.issued?.["date-parts"]?.[0]?.[0] ? ` (${c.issued["date-parts"][0][0]})` : ""}
                  </button>
                ))}
              </div>
            )}
            <table className="lb-lookup-table">
              <tbody>
                {L.lookupDiff(form, lookup.candidates[lookup.picked]).map(r => (
                  <tr key={r.key} className={r.differs ? "differs" : "same"}>
                    <td><input type="checkbox" checked={lookup.keys.has(r.key)} disabled={!r.differs} onChange={() => toggleKey(r.key)} aria-label={T(r.label)} /></td>
                    <th>{T(r.label)}</th>
                    <td>{r.differs && r.current && <del>{r.current}</del>}{" "}{r.found}</td>
                  </tr>
                ))}
              </tbody>
            </table>
            <div className="lb-lookup-foot">
              <button type="button" className="btn outlined accent" disabled={lookup.keys.size === 0} onClick={applyLookup}>{T("Take the ticked fields")}</button>
              <button type="button" className="btn ghost" onClick={() => setLookup(null)}>{T("Close")}</button>
              <span className="lb-state">{T("The form changes only; Save keeps it.")}</span>
            </div>
          </div>
        )}
        <label className="lb-field"><span>{T("Type")}</span>
          <select value={form.type} onChange={set("type")}>
            {L.TYPES.map(([id, label]) => <option key={id} value={id}>{T(label)}</option>)}
            {!L.TYPES.some(([id]) => id === form.type) && <option value={form.type}>{form.type}</option>}
          </select>
        </label>
        <label className={`lb-field${guess.has("title") ? " lb-guess" : ""}`}><span>{T("Title")}{guess.has("title") && <em> · {T("check")}</em>}</span>
          <textarea rows={2} value={form.title} onChange={set("title")} /></label>
        <label className={`lb-field${guess.has("authors") ? " lb-guess" : ""}`}><span>{T("Authors (one per line, Family, Given)")}{guess.has("authors") && <em> · {T("check")}</em>}</span>
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

        {!isDraft && (
          <>
            <div className="lb-field"><span>{T("Citation key")}</span>
              <code className="lb-key">{entry.citekey}</code></div>
            <div className="lb-field"><span>{T("Files")}</span>
              <ul className="lb-files">
                {entry.attachments.map(a => (
                  <li key={a.id} className={a.missing ? "missing" : ""}>
                    <button type="button" className="btn ghost lb-file-open" disabled={a.missing} onClick={() => onOpenAttachment(a)} title={a.path}>
                      {a.path.split("/").pop()}
                    </button>
                    {a.missing && <em>{T("missing")}</em>}
                    <button type="button" className="btn ghost" title={T("Removes the link; the file stays in the project")} onClick={() => onRemoveAttachment(a)}>{T("Unlink")}</button>
                  </li>
                ))}
              </ul>
              <div><button type="button" className="btn ghost" onClick={() => onAttach(entry)}>{T("Attach a PDF…")}</button></div>
            </div>
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
          </>
        )}
      </div>
      <div className="lb-detail-foot">
        <button type="button" className="btn outlined accent" disabled={!dirty || state === "saving"} onClick={save}>{isDraft ? T("Add to library") : T("Save")}</button>
        {isDraft
          ? <button type="button" className="btn ghost" onClick={onDiscardDraft}>{T("Discard")}</button>
          : <>
              <button type="button" className="btn ghost" disabled={!dirty} onClick={() => { setForm(L.toForm(entry.csl)); setTags(entry.tags.join(", ")); setNotes(entry.notes); }}>{T("Revert")}</button>
              <button type="button" className="btn ghost" title={T("The in-text citation, in the chosen style")} onClick={() => copy("citation")}>{T("Copy citation")}</button>
              <button type="button" className="btn ghost" title={T("The reference-list entry, in the chosen style")} onClick={() => copy("reference")}>{T("Copy reference")}</button>
            </>}
        <span className="lb-state" role="status">{state === "copied" ? T("Copied") : state === "saving" ? "" : state}</span>
        {!isDraft && (
          <button type="button" className={`btn ghost lb-del${armed ? " armed" : ""}`}
            title={T("Moves it out of the library")}
            onClick={() => { if (armed) onDelete(entry); else setArmed(true); }} onBlur={() => setArmed(false)}>
            {armed ? T("Delete?") : T("Delete")}
          </button>
        )}
      </div>
    </section>
  );
}

Object.assign(window, { LibDetail });
