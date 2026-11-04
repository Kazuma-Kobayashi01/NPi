/* librarian/copy-menu.jsx — "Copy as…": the references (the selected one, or what the list shows) copied in
   the form a journal or a regulation asks for, in one click. Type a journal ("Phys Rev Lett"), a publisher
   ("Elsevier") or a rule ("GB/T 7714", "ICMJE") and a click on the result sets that citation style and
   copies. What to copy is the reference list, the in-text citation, or BibTeX / RIS / CSL-JSON text.
   Which style a journal asks for is the table in app/librarian-journals.js; a row says whether the style
   is the journal's own or only the closest bundled one, and a journal with no bundled style says how to
   add its .csl. The journal's own guide for authors decides. */

function CopyMenu({ project, styles, styleId, selectedCount, shownCount, onCopy, onPickStyle, onAddStyle }) {
  const hasSelection = selectedCount > 0;
  const T = window.OMP_I18N.t;
  const J = window.OMP_JOURNALS;
  const [open, setOpen] = React.useState(false);
  const [query, setQuery] = React.useState("");
  const [kind, setKind] = React.useState("reference");
  const [scope, setScope] = React.useState(hasSelection ? "selected" : "shown");
  const [note, setNote] = React.useState(null);
  const rootRef = React.useRef(null);
  const recentKey = `npi.librarian.recent-styles.${project}`;
  const [recent, setRecent] = React.useState(() => { try { return JSON.parse(localStorage.getItem(recentKey) || "[]"); } catch { return []; } });

  React.useEffect(() => { setScope(hasSelection ? "selected" : "shown"); }, [hasSelection]);
  React.useEffect(() => {
    if (!open) return undefined;
    const away = e => { if (!rootRef.current?.contains(e.target)) setOpen(false); };
    const esc = e => { if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); setOpen(false); } };
    document.addEventListener("mousedown", away);
    document.addEventListener("keydown", esc, true);
    return () => { document.removeEventListener("mousedown", away); document.removeEventListener("keydown", esc, true); };
  }, [open]);

  const styleName = id => styles.find(s => s.id === id)?.name ?? id;
  const found = query.trim() ? J.find(query, 12) : [];
  const isText = !["reference", "citation"].includes(kind);
  const KINDS = [["reference", T("Reference list")], ["citation", T("In-text citation")], ["citekey", "[@key]"], ["bibtex", "BibTeX"], ["ris", "RIS"], ["csl-json", "CSL-JSON"]];

  const remember = (label, style) => {
    const next = [{ label, style }, ...recent.filter(r => r.style !== style)].slice(0, 5);
    setRecent(next);
    try { localStorage.setItem(recentKey, JSON.stringify(next)); } catch { /* not kept */ }
  };

  /** Copies in `style` (null: the current one). */
  const copy = async (style, label) => {
    const useStyle = style ?? styleId;
    if (style && style !== styleId) onPickStyle(style);
    const ok = await onCopy({ kind, scope, style: useStyle, label: label ?? styleName(useStyle) });
    if (ok && !isText) remember(label ?? styleName(useStyle), useStyle);
    if (ok) setOpen(false);
  };

  const choose = preset => {
    if (!preset.style) { setNote(preset); return; }
    setNote(preset.exact && !preset.note ? null : preset);
    copy(preset.style, preset.label);
  };

  const groups = [["common", T("Common styles")], ["archive", T("Publishers, journals, societies, standards")], ["user", T("Added by you")]];

  return (
    <span className="cm-root" ref={rootRef}>
      <button type="button" className={`btn ghost${open ? " on" : ""}`} disabled={shownCount === 0} aria-expanded={open}
        title={T("Copy references in the form a journal or a regulation asks for")} onClick={() => setOpen(v => !v)}>
        {T("Copy as…")}<span className="cm-current"> · {styleName(styleId)}</span>
      </button>
      {open && (
        <div className="cm-pop" role="dialog" aria-label={T("Copy as…")}>
          <div className="cm-row">
            <div className="mode-seg" role="radiogroup" aria-label={T("What to copy")}>
              {KINDS.map(([id, label]) => (
                <button key={id} type="button" role="radio" aria-checked={kind === id} className={kind === id ? "on" : ""} onClick={() => setKind(id)}>{label}</button>
              ))}
            </div>
          </div>
          <div className="cm-row cm-scope">
            <label><input type="radio" name="cm-scope" checked={scope === "selected"} disabled={!hasSelection} onChange={() => setScope("selected")} /> {selectedCount > 1 ? T("The {n} selected", { n: selectedCount }) : T("The selected reference")}</label>
            <label><input type="radio" name="cm-scope" checked={scope === "shown"} onChange={() => setScope("shown")} /> {T("All {n} in the list", { n: shownCount })}</label>
          </div>
          {isText ? (
            <div className="cm-row"><button type="button" className="btn primary" onClick={() => copy(null)}>{T("Copy")}</button></div>
          ) : (
            <>
              <div className="cm-row">
                <input className="lb-input cm-search" autoFocus value={query} onChange={e => { setQuery(e.target.value); setNote(null); }}
                  placeholder={T("Journal, publisher or rule: Phys Rev Lett, Elsevier, GB/T 7714, ICMJE…")} aria-label={T("Journal or regulation")} />
              </div>
              {query.trim() ? (
                <div className="cm-list" role="listbox">
                  {found.length === 0 && <div className="rd-none">{T("No journal or rule by that name. Pick a style below.")}</div>}
                  {found.map(pr => (
                    <button key={pr.label} type="button" role="option" className="cm-item" onClick={() => choose(pr)}>
                      <span className="cm-item-name">{pr.label}</span>
                      <span className="cm-item-style">{pr.style ? styleName(pr.style) : T("no bundled style")}</span>
                      {pr.style && !pr.exact && <span className="rd-badge scanned">{T("closest")}</span>}
                    </button>
                  ))}
                </div>
              ) : (
                recent.length > 0 && (
                  <div className="cm-row cm-recent">
                    <span className="cm-label">{T("Recent")}</span>
                    {recent.map(r => <button key={r.style} type="button" className="btn ghost" onClick={() => copy(r.style, r.label)} title={styleName(r.style)}>{r.label}</button>)}
                  </div>
                )
              )}
              {note && (
                <div className="cm-note" role="status">
                  {note.note || (note.style ? T("The closest bundled style.") : "")}
                  {!note.style && <button type="button" className="btn ghost" onClick={() => { setOpen(false); onAddStyle(); }}>{T("Add a .csl style…")}</button>}
                </div>
              )}
              <div className="cm-row cm-style">
                <span className="cm-label">{T("Or pick a style")}</span>
                <select className="lb-select" value={styleId} aria-label={T("Citation style")} onChange={e => onPickStyle(e.target.value)}>
                  {groups.map(([g, label]) => {
                    const list = styles.filter(s => s.group === g);
                    return list.length ? <optgroup key={g} label={label}>{list.map(s => <option key={s.id} value={s.id}>{s.name}</option>)}</optgroup> : null;
                  })}
                </select>
                <button type="button" className="btn primary" onClick={() => copy(null)}>{T("Copy in this style")}</button>
              </div>
              <p className="cm-hint">{T("A journal's own guide for authors decides; the list only says which style it is named after.")}</p>
            </>
          )}
        </div>
      )}
    </span>
  );
}
window.CopyMenu = CopyMenu;
