/* librarian/lib-panels.jsx — the strips that open above the Librarian's columns: the trash, and the
   citation check of a draft. */

function LibTrash({ items, onRestore, onClose }) {
  const T = window.OMP_I18N.t;
  return (
    <div className="lb-report lb-trash" role="region" aria-label={T("Trash")}>
      <div className="lb-report-head">
        <strong>{T("Trash")}</strong>
        <span>{items.length === 0 ? T("Nothing in the trash.") : T("{n} removed", { n: items.length })}</span>
        <button type="button" className="btn ghost" onClick={onClose}>{T("Close")}</button>
      </div>
      <ul>
        {items.map(item => (
          <li key={item.id}>
            <button type="button" className="btn ghost" onClick={() => onRestore(item)}>{T("Restore")}</button>
            <code>{item.citekey}</code> <span>{window.OMP_LIBRARIAN.plainText(item.csl?.title).slice(0, 100)}</span>
          </li>
        ))}
      </ul>
      <small>{T("Removed references are kept, and their citekeys are never reused.")}</small>
    </div>
  );
}

function LibReport({ report, onClose }) {
  const T = window.OMP_I18N.t;
  return (
    <div className="lb-report" role="region" aria-label={T("Citation check")}>
      <div className="lb-report-head">
        <strong>{T("Citation check")}</strong>
        <span>{report.findings.length === 0
          ? T("No checkable citations found.")
          : T("{matched} matched, {problems} need attention", { matched: report.matched, problems: report.problems })}</span>
        <button type="button" className="btn ghost" onClick={onClose}>{T("Close")}</button>
      </div>
      <ul>
        {report.findings.map((f, i) => (
          <li key={i} className={f.status === "matched" ? "ok" : "check"}>
            <span className="lb-report-status">{f.status === "matched" ? T("ok") : T("check")}</span>
            <code>{f.text}</code> <span>{f.detail}</span>
          </li>
        ))}
      </ul>
      <small>{T("Only [@citekey], Author (Year) and DOIs are checked; numbered citations like [1] are not.")}</small>
    </div>
  );
}

Object.assign(window, { LibTrash, LibReport });
