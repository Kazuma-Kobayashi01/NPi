/* ═════════════════════════════════════════════════════════════════════
   context-insight-charts.jsx — the Context details panel's pictures:
   the composition donut with its legend, the per-request trend and the
   tool/command chips. Pure presentation over `breakdown.insight`
   (app/context-insight.js); the panel itself is context-insight-modal.jsx.
   Layout after dsh-context's Context tab (Apache-2.0); no code shared.
   ═════════════════════════════════════════════════════════════════════ */

// One colour per category across the donut, legend and bars.
const CI_PALETTE = {
  system: "var(--lilac)", tools: "var(--amber)", mcp: "var(--magenta)", skills: "var(--lime)", rules: "var(--fg-4)",
  compacted: "var(--cyan)", user: "var(--accent)", answers: "color-mix(in oklab, var(--accent) 55%, var(--cyan))",
  reasoning: "color-mix(in oklab, var(--lilac) 60%, var(--rose))", toolCalls: "color-mix(in oklab, var(--amber) 70%, var(--rose))",
  toolResults: "var(--rose)", other: "var(--fg-3)", overhead: "var(--line-bright)",
};
const CI_CHIP_COLORS = ["var(--accent)", "var(--cyan)", "var(--amber)", "var(--lilac)", "var(--rose)", "var(--lime)", "var(--magenta)"];

// Ring of `segments` ({key, tokens}) over their sum, with a centred label.
function CIDonut({ segments, center, sub }) {
  const sum = segments.reduce((n, s) => n + s.tokens, 0) || 1;
  const r = 42, c = 2 * Math.PI * r;
  let offset = 0;
  return (
    <div className="ci-donut">
      <svg viewBox="0 0 100 100" aria-hidden="true">
        <circle cx="50" cy="50" r={r} fill="none" stroke="var(--line)" strokeWidth="10" />
        {segments.map(s => {
          const len = (s.tokens / sum) * c;
          const el = (
            <circle key={s.key} cx="50" cy="50" r={r} fill="none" stroke={CI_PALETTE[s.key] ?? "var(--fg-4)"} strokeWidth="10"
              strokeDasharray={`${Math.max(0, len - 0.6)} ${c}`} strokeDashoffset={-offset} transform="rotate(-90 50 50)" />
          );
          offset += len;
          return el;
        })}
      </svg>
      <div className="ci-donut-label"><strong>{center}</strong><span>{sub}</span></div>
    </div>
  );
}

// Legend rows: swatch, label, a bar against the largest row, ≈tokens, share.
function CILegend({ rows, total, fmt }) {
  const max = Math.max(...rows.map(r => r.tokens), 1);
  return (
    <div className="ci-legend">
      {rows.map(r => (
        <div key={r.key} className="ci-legend-row" title={r.detail?.join(", ")}>
          <span className="sp-swatch" style={{ background: CI_PALETTE[r.key] ?? "var(--fg-4)" }} />
          <span className="ci-legend-label">{r.label}{r.count > 0 && <span className="sp-count">{r.count}</span>}</span>
          <span className="ci-mini"><span style={{ width: `${(r.tokens / max) * 100}%`, background: CI_PALETTE[r.key] ?? "var(--fg-4)" }} /></span>
          <span className="ci-legend-num">≈{fmt(r.tokens)}</span>
          <span className="ci-legend-pct">{total > 0 ? `${Math.round((r.tokens / total) * 100)}%` : "—"}</span>
        </div>
      ))}
    </div>
  );
}

// Nice round axis maximum and its four ticks.
function _ciTicks(max) {
  const step = Math.pow(10, Math.floor(Math.log10(max / 4 || 1)));
  const nice = [1, 1.5, 2, 2.5, 3, 4, 5, 6, 8, 10].map(m => m * step).find(s => s * 4 >= max) ?? step * 10;
  return [4, 3, 2, 1, 0].map(i => i * nice);
}

// Stacked bars per request (cached / fresh input), turn bands under them,
// and the hovered (or last) request's numbers in a strip below.
function CITrend({ points, fmt }) {
  const [hover, setHover] = React.useState(null);
  if (points.length === 0) return <div className="ci-empty">No model request in this context yet.</div>;
  const ticks = _ciTicks(Math.max(...points.map(p => p.context), 1));
  const top = ticks[0] || 1;
  const sel = points[hover ?? points.length - 1];
  const turns = [];
  for (const p of points) {
    const last = turns[turns.length - 1];
    if (last && last.turn === p.turn) last.count++;
    else turns.push({ turn: p.turn, count: 1 });
  }
  const selIndex = hover ?? points.length - 1;
  return (
    <div className="ci-trend-wrap">
      <div className="ci-trend-plot">
        <div className="ci-axis">{ticks.map(t => <span key={t}>{fmt(t)}</span>)}</div>
        <div className="ci-bars" onMouseLeave={() => setHover(null)}>
          {ticks.slice(0, -1).map(t => <span key={t} className="ci-grid-line" style={{ bottom: `${(t / top) * 100}%` }} />)}
          {points.map((p, i) => (
            <div key={i} className={`ci-bar${i === selIndex ? " sel" : ""}`} onMouseEnter={() => setHover(i)}
              style={{ height: `${(p.context / top) * 100}%` }}>
              <span className="ci-bar-fresh" style={{ flexGrow: Math.max(0, p.context - p.cacheRead) }} />
              <span className="ci-bar-cached" style={{ flexGrow: p.cacheRead }} />
            </div>
          ))}
        </div>
      </div>
      <div className="ci-turns">
        {turns.map((t, i) => <span key={i} style={{ flexGrow: t.count }} title={`Turn ${t.turn}`}>{t.count >= 3 || turns.length < 12 ? t.turn : ""}</span>)}
      </div>
      <div className="ci-detail">
        <span className="ci-detail-title">Request {selIndex + 1} of {points.length} · turn {sel.turn}</span>
        {sel.model && <span className="ci-pill dim">{sel.model}</span>}
        <span className="ci-pill">Context {fmt(Math.round(sel.context))}</span>
        <span className="ci-pill">Output {fmt(Math.round(sel.output))}</span>
        <span className="ci-pill">Cache {sel.context > 0 ? `${((sel.cacheRead / sel.context) * 100).toFixed(1)}%` : "—"}</span>
      </div>
    </div>
  );
}

// `name count` chips, the first `limit` shown and the rest behind "+N more".
function CIChips({ items, limit = 8, empty }) {
  const [all, setAll] = React.useState(false);
  if (items.length === 0) return <div className="ci-empty">{empty}</div>;
  const shown = all ? items : items.slice(0, limit);
  return (
    <div className="ci-chips">
      {shown.map((it, i) => (
        <span key={it.key} className="ci-chip" title={it.title}>
          <span className="ci-chip-dot" style={{ background: CI_CHIP_COLORS[i % CI_CHIP_COLORS.length] }} />
          <span className="ci-chip-name">{it.label}</span>
          <strong>{it.count}</strong>
          {it.errors > 0 && <span className="ci-chip-err">{it.errors} err</span>}
        </span>
      ))}
      {items.length > limit && (
        <button type="button" className="ci-chip more" onClick={() => setAll(v => !v)}>{all ? "less" : `+${items.length - limit} more`}</button>
      )}
    </div>
  );
}

Object.assign(window, { CIDonut, CILegend, CITrend, CIChips });
