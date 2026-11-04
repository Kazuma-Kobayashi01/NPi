/* librarian/lib-head.jsx — the tools in the Librarian window's title bar. */

function LibHead({ project, styles, styleId, selectedCount, shownCount, onNew, onImport, onPdf, onAlign, onCopy, onPickStyle, onAddStyle, onCiteDraft, onCheckDraft, onExport, onTrash, onConnect }) {
  const T = window.OMP_I18N.t;
  const { Icon } = window;
  return (
    <span className="lb-head-tools">
      <button type="button" className="btn ghost" onClick={onNew}><Icon name="plus" size={11} /> {T("New reference")}</button>
      <button type="button" className="btn ghost" onClick={onImport}>{T("Import…")}</button>
      <button type="button" className="btn ghost" onClick={onPdf}>{T("Add PDF…")}</button>
      <button type="button" className="btn ghost" title={T("Compares the references with missing metadata in the list, or several selected ones, with Crossref and OpenAlex")} onClick={onAlign}>{T("Align metadata…")}</button>
      <span className="lb-head-gap" />
      <CopyMenu project={project} styles={styles} styleId={styleId} selectedCount={selectedCount} shownCount={shownCount}
        onCopy={onCopy} onPickStyle={onPickStyle} onAddStyle={onAddStyle} />
      <button type="button" className="btn ghost" title={T("Replaces [@citekey] in a Markdown draft with citations in the style chosen under “Copy as…” and appends the references")} onClick={onCiteDraft}>{T("Cite a draft…")}</button>
      <button type="button" className="btn ghost" title={T("Checks a draft's citations against the library")} onClick={onCheckDraft}>{T("Check a draft…")}</button>
      <select className="lb-select" aria-label={T("Export")} value="" disabled={shownCount === 0}
        onChange={e => { if (e.target.value) onExport(e.target.value); }}>
        <option value="">{T("Export to a file…")}</option>
        <option value="bibtex">BibTeX (.bib)</option>
        <option value="ris">RIS (.ris)</option>
        <option value="csl-json">CSL-JSON (.json)</option>
      </select>
      <window.MoreMenu items={[
        { label: T("Trash"), onClick: onTrash },
        { label: T("Add a .csl style…"), onClick: onAddStyle },
        "-",
        { label: T("Connect agents"), onClick: onConnect },
      ]} />
    </span>
  );
}

Object.assign(window, { LibHead });
