// app/review.js — the card a multi-expert review shows before it starts (src-tauri expert_review):
// what to say about the models, the providers the material goes to and the cost.
// Exposes `window.OMP_REVIEW`.
(function () {
  /** "≈ 186K tokens" from Rust's estimate. */
  function tokensLabel(n) {
    const v = Number(n) || 0;
    if (v >= 1e6) return `≈ ${(v / 1e6).toFixed(1)}M tokens`;
    if (v >= 1e3) return `≈ ${Math.round(v / 1e3)}K tokens`;
    return `≈ ${v} tokens`;
  }

  /** `{ title, lines, providers, cost, heavy, missing }` for the card Rust sent (`review://confirm`). */
  function cardView(card) {
    const c = card ?? {};
    const lines = (c.experts ?? []).map(e => ({ who: `${e.letter} · ${e.angle}`, model: e.model }));
    lines.push({ who: "Arbiter", model: c.arbiter ?? "" });
    return {
      title: `${c.kind ?? "Review"}: ${c.subject ?? ""}`,
      lines,
      providers: (c.providers ?? []).join(", "),
      cost: tokensLabel(c.tokens),
      heavy: (Number(c.tokens) || 0) >= 100000,
      missing: (c.missing ?? []).join(", "),
    };
  }

  window.OMP_REVIEW = { tokensLabel, cardView };
})();
