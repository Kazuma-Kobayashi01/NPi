/* handoff-card.jsx — Chat's offer to continue as work, above the composer.
   The model's `handoff_to_work` call (src-tauri/src/chat_handoff.rs) becomes
   this card; nothing starts until the user clicks. The button opens a tab of
   the chosen mode on a folder the user picks, with the model's brief in the
   prompt box, unsent. */

function HandoffCard({ offer, onAccept, onDismiss }) {
  const T = window.OMP_I18N.t;
  const [busy, setBusy] = React.useState(false);
  const [expanded, setExpanded] = React.useState(false);
  const mode = offer.target === "academic" ? T("Academic") : T("Code");
  const accept = async () => {
    if (busy) return;
    setBusy(true);
    try { await onAccept(offer); } finally { setBusy(false); }
  };
  return (
    <div className="handoff-card" role="group" aria-label={T("Continue as work")}>
      <div className="handoff-main">
        <div className="handoff-title">{offer.title || T("Ready to start the work")}</div>
        <button type="button" className="handoff-peek" onClick={() => setExpanded(v => !v)} aria-expanded={expanded}>
          {expanded ? T("Hide brief") : T("Show brief")}
        </button>
        {expanded && <pre className="handoff-brief">{offer.prompt}</pre>}
      </div>
      <button type="button" className="btn ghost" onClick={onDismiss}>{T("Not now")}</button>
      <button type="button" className="btn primary" onClick={accept} disabled={busy}>
        {T("Continue in {mode}", { mode })}
      </button>
    </div>
  );
}
window.HandoffCard = HandoffCard;
