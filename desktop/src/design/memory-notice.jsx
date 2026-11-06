/* memory-notice.jsx — something the agent just kept for later, shown for a moment at the bottom of
   the window so nothing is kept unseen: a note about the user or the mode's profile (`memory://noted`,
   src-tauri/src/memory/tools.rs) or a skill it wrote (`manage_skill`, seen in live.js). Something new
   can be undone here; a change to what was there is edited in Settings → Memory or Settings → Skills.
   Listens to `npi:memory-noted` itself; what to say is OMP_MEMORY.noticeOf. */

function MemoryNotice({ bridge }) {
  const T = window.OMP_I18N.t;
  const [shown, setShown] = React.useState(null);
  const timer = React.useRef(null);
  React.useEffect(() => {
    const onNoted = e => {
      const notice = window.OMP_MEMORY.noticeOf(e.detail);
      if (!notice) return;
      setShown(notice);
      clearTimeout(timer.current);
      timer.current = setTimeout(() => setShown(null), 6000);
    };
    window.addEventListener("npi:memory-noted", onNoted);
    return () => { window.removeEventListener("npi:memory-noted", onNoted); clearTimeout(timer.current); };
  }, []);
  if (!shown) return null;
  const undo = async () => {
    const { undo: what } = shown;
    setShown(null);
    if (what?.note !== undefined) await bridge.memory("delete", { id: what.note });
    else if (what?.skill) await bridge.deleteManagedSkill(what.skill);
  };
  return (
    <div className="memory-notice" role="status">
      <span className="memory-notice-what">{T(shown.what)}</span>
      <span className="memory-notice-text selectable" title={shown.text}><b>{shown.title}</b> {shown.text}</span>
      {shown.undo && <button type="button" className="btn ghost" onClick={undo}>{T("Undo")}</button>}
      <button type="button" className="btn ghost" aria-label={T("Close")} onClick={() => setShown(null)}>×</button>
    </div>
  );
}
window.MemoryNotice = MemoryNotice;
