/* librarian/lib-import.jsx — adding references that exist elsewhere: a file (RIS, BibTeX, CSL-JSON, PubMed
   .nbib, Web of Science, EndNote XML), or pasted text. Pasted DOIs and arXiv ids are looked up one by one;
   any other text is read as a file's. What is already in the library is reported, never added twice. */

function LibImport({ call, onFile, onDone, onClose }) {
  const T = window.OMP_I18N.t;
  const L = window.OMP_LIBRARIAN;
  const [text, setText] = React.useState("");
  const [busy, setBusy] = React.useState(false);
  const [status, setStatus] = React.useState("");

  /** Asks for each DOI's record and adds it; what could not be found stays in the box to be retried. */
  const addIdentifiers = async ids => {
    const report = { added: [], duplicates: [] };
    const failed = [];
    for (const [i, doi] of ids.entries()) {
      setStatus(T("Looking up {i} of {n}…", { i: i + 1, n: ids.length }));
      const found = await call("lookup", { doi });
      const res = found.ok ? await call("import", { format: "csl-json", text: JSON.stringify([found.value[0].csl]) }) : found;
      if (res.ok) {
        report.added.push(...res.value.added);
        report.duplicates.push(...res.value.duplicates);
      } else failed.push(doi);
    }
    return { report, failed };
  };

  const add = async () => {
    const ids = L.parseIdentifiers(text);
    setBusy(true);
    setStatus("");
    if (ids) {
      const { report, failed } = await addIdentifiers(ids);
      setBusy(false);
      if (failed.length === 0) { onClose(); return onDone(report); }
      setText(failed.join("\n"));
      setStatus(T("Added {n}; not found: {m}", { n: report.added.length, m: failed.length }));
      return onDone(report);
    }
    const res = await call("import_text", { text });
    setBusy(false);
    if (!res.ok) return setStatus(res.error);
    onClose();
    return onDone(res.value);
  };

  return (
    <div className="lb-import" role="group" aria-label={T("Import")}>
      <div className="lb-import-head">
        <strong>{T("Import references")}</strong>
        <button type="button" className="btn ghost" onClick={onFile}>{T("Choose a file…")}</button>
        <button type="button" className="btn ghost" onClick={onClose}>{T("Close")}</button>
      </div>
      <textarea autoFocus value={text} spellCheck={false} aria-label={T("Pasted references")} onChange={e => setText(e.target.value)}
        placeholder={T("Paste DOIs or arXiv ids, one per line, or BibTeX, RIS, CSL-JSON, PubMed or Web of Science text")} />
      <div className="lb-import-foot">
        <button type="button" className="btn outlined accent" disabled={busy || !text.trim()} onClick={add}>{T("Add to library")}</button>
        <span className="lb-state" role="status">{status}</span>
      </div>
    </div>
  );
}

Object.assign(window, { LibImport });
