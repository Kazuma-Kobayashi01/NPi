/* rewind-dialog.jsx — Rewind with files changed since that message: the user
   picks whether the files go back with the conversation (omp's file
   checkpoints). Props: { changes: [{ path, removes }], onAnswer }; onAnswer
   gets "files", "conversation" or "cancel". Focus starts on "conversation",
   the choice that changes no file. */

function RewindDialog({ changes, onAnswer }) {
  const T = window.OMP_I18N.t;
  const safeRef = React.useRef(null);
  window.useModalFocus(safeRef);
  React.useEffect(() => {
    const onKey = e => {
      if (e.key !== "Escape") return;
      e.preventDefault();
      e.stopPropagation();
      onAnswer("cancel");
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [onAnswer]);

  const SHOWN = 8;
  return (
    <div className="bridge-scrim" onClick={() => onAnswer("cancel")} style={{ paddingTop: "18vh" }}>
      <div className="close-confirm-panel" role="alertdialog" aria-modal="true" aria-label={T("Rewind to here")} onClick={e => e.stopPropagation()}>
        <div className="close-confirm-head">
          <span className="mono" style={{ color: "var(--fg-2)" }}>{T("Rewind to here")}</span>
        </div>
        <div className="close-confirm-body">
          <div>{T("{n} files changed since this message.", { n: changes.length })}</div>
          <ul className="close-confirm-tabs">
            {changes.slice(0, SHOWN).map(c => (
              <li key={c.path} className="close-confirm-tab mono">
                <span className="close-confirm-tab-name">{c.path}</span>
                <span className="close-confirm-tab-state">{T(c.removes ? "removed" : "restored")}</span>
              </li>
            ))}
            {changes.length > SHOWN && <li className="close-confirm-tab mono">{T("…and {n} more", { n: changes.length - SHOWN })}</li>}
          </ul>
          <div>{T("Files ignored by the project and changes outside it are not touched.")}</div>
        </div>
        <div className="close-confirm-foot">
          <button className="btn ghost" onClick={() => onAnswer("cancel")}>{T("Cancel")}</button>
          <button className="btn ghost" ref={safeRef} onClick={() => onAnswer("conversation")}>{T("Conversation only")}</button>
          <button className="btn outlined danger" onClick={() => onAnswer("files")}>{T("Files and conversation")}</button>
        </div>
      </div>
    </div>
  );
}

window.RewindDialog = RewindDialog;
