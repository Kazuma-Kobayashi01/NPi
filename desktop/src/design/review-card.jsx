/* review-card.jsx — the go-ahead for a multi-expert review (src-tauri/src/expert_review), above the
   composer of the tab that asked. Rust waits for this answer, not the model's: nothing is sent to
   any model until Start. It names the models by angle, the providers the material goes to and the
   estimated tokens (OMP_REVIEW.cardView). Listens to `npi:review-confirm` (live.js) itself. */

function ReviewCard({ sessionId, bridge }) {
  const T = window.OMP_I18N.t;
  const [cards, setCards] = React.useState({});
  React.useEffect(() => {
    const onAsk = e => {
      const d = e.detail;
      if (d?.sessionId && d.id) setCards(m => ({ ...m, [d.sessionId]: d }));
    };
    window.addEventListener("npi:review-confirm", onAsk);
    return () => window.removeEventListener("npi:review-confirm", onAsk);
  }, []);
  const ask = cards[sessionId];
  if (!ask) return null;
  const v = window.OMP_REVIEW.cardView(ask.card);
  const answer = accept => {
    setCards(m => { const { [sessionId]: _gone, ...rest } = m; return rest; });
    bridge.answerReview(ask.id, accept);
  };
  return (
    <div className="handoff-card review-card" role="group" aria-label={T("Multi-expert review")}>
      <div className="handoff-main">
        <div className="handoff-title">{T("Multi-expert review (simulated)")} · {v.title}</div>
        <ul className="review-card-models">
          {v.lines.map(l => <li key={l.who}><span>{T(l.who)}</span><code>{l.model}</code></li>)}
        </ul>
        <div className={`review-card-cost${v.heavy ? " heavy" : ""}`}>
          {T("The material goes to every model: {cost}, to {providers}. This uses a large share of your plan.").replace("{cost}", v.cost).replace("{providers}", v.providers)}
        </div>
        {v.missing && <div className="review-card-note">{T("No model set for: {tiers}.").replace("{tiers}", v.missing)}</div>}
      </div>
      <button type="button" className="btn ghost" onClick={() => answer(false)}>{T("Not now")}</button>
      <button type="button" className="btn primary" onClick={() => answer(true)}>{T("Start review")}</button>
    </div>
  );
}
window.ReviewCard = ReviewCard;
