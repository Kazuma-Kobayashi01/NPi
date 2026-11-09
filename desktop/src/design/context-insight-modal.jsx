/* ═════════════════════════════════════════════════════════════════════
   context-insight-modal.jsx — "Context details", opened from the rail's
   Context card: KPI tiles, the whole context as a donut (the card's fixed
   parts plus the Messages share split by role), the context each model
   request carried, and the tools, commands and files behind it.
   Data: `breakdown` with its `insight` (app/context-insight.js), computed
   by live.js from the same get_messages context as the card. Charts live
   in context-insight-charts.jsx.
   ═════════════════════════════════════════════════════════════════════ */

const { CIDonut: _CI_Donut, CILegend: _CI_Legend, CITrend: _CI_Trend, CIChips: _CI_Chips } = window;

function _CI_Card({ title, extra, className = "", children }) {
  return (
    <section className={`ci-card ${className}`}>
      <div className="ci-card-head"><span className="ci-card-title">{title}</span>{extra && <span className="ci-card-extra">{extra}</span>}</div>
      {children}
    </section>
  );
}

function _CI_Tile({ value, label, tone }) {
  return (
    <div className="ci-tile">
      <span className="ci-tile-value" style={tone ? { color: tone } : undefined}>{value}</span>
      <span className="ci-tile-label">{label}</span>
    </div>
  );
}

// The size the user last dragged the panel to (localStorage, written through
// to ui-state.json by app/ui-store.js); the CSS default and limits apply when
// none is stored or it no longer fits the window.
const _CI_SIZE_KEY = "npi.contextDetails.size";

function _ciStoredSize() {
  try {
    const v = JSON.parse(localStorage.getItem(_CI_SIZE_KEY) || "null");
    if (v && v.w > 0 && v.h > 0) return { width: `min(${Math.round(v.w)}px, calc(100vw - 16px))`, height: `min(${Math.round(v.h)}px, calc(100vh - 16px))` };
  } catch { /* unavailable or malformed: use the default size */ }
  return undefined;
}

// Remember the panel's size after a resize, but not while it is zoomed or shaded.
function _useRememberSize(panelRef) {
  React.useEffect(() => {
    const el = panelRef.current;
    if (!el || typeof ResizeObserver === "undefined") return undefined;
    let timer = 0, first = true;
    const ro = new ResizeObserver(() => {
      if (first) { first = false; return; }
      clearTimeout(timer);
      timer = setTimeout(() => {
        if (el.classList.contains("wc-zoomed") || el.classList.contains("wc-shaded")) return;
        try { localStorage.setItem(_CI_SIZE_KEY, JSON.stringify({ w: el.offsetWidth, h: el.offsetHeight })); } catch { /* not stored */ }
      }, 300);
    });
    ro.observe(el);
    return () => { clearTimeout(timer); ro.disconnect(); };
  }, [panelRef]);
}

/** The card's fixed parts and the Messages split, in one list for the donut. */
function _ciSegments(breakdown) {
  const fixed = breakdown.parts.filter(p => p.key !== "messages" && p.key !== "free" && p.tokens > 0);
  return [...breakdown.insight.parts, ...fixed];
}

function ContextInsightModal({ breakdown, onClose }) {
  const panelRef = React.useRef(null);
  const wc = window.useWindowChrome(onClose, panelRef);
  const [initialSize] = React.useState(_ciStoredSize);
  _useRememberSize(panelRef);
  // A resize drag that ends outside the panel clicks the backdrop: close only
  // on a click that also started on it.
  const downOnScrim = React.useRef(false);
  React.useEffect(() => {
    const onKey = e => { if (e.key === "Escape") { e.preventDefault(); onClose(); } };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [onClose]);
  const fmt = window.formatTokens;
  const insight = breakdown?.insight;

  let body = <div className="ci-empty big">Nothing sent yet.</div>;
  if (insight) {
    const segments = _ciSegments(breakdown);
    const toolCalls = insight.tools.reduce((n, t) => n + t.calls, 0);
    const toolErrors = insight.tools.reduce((n, t) => n + t.errors, 0);
    const edited = insight.files.filter(f => f.writes > 0).length;
    const pct = breakdown.pct < 1 && breakdown.used > 0 ? "<1" : Math.round(breakdown.pct);
    const { trend, requests, cacheHit } = insight;
    body = (
      <>
        <div className="ci-tiles">
          <_CI_Tile value={insight.turns} label="Turns" tone="var(--accent)" />
          <_CI_Tile value={requests} label="Requests" tone="var(--cyan)" />
          <_CI_Tile value={toolCalls} label={toolErrors ? `Tool calls · ${toolErrors} failed` : "Tool calls"} tone="var(--amber)" />
          <_CI_Tile value={cacheHit === null ? "—" : `${(cacheHit * 100).toFixed(1)}%`} label="Cache hit" tone="var(--lime)" />
          <_CI_Tile value={`${pct}%`} label={`of ${fmt(breakdown.total)} window`} tone="var(--lilac)" />
        </div>

        <_CI_Card title="Current context" extra={`${fmt(breakdown.used)} / ${fmt(breakdown.total)} tokens`}>
          <div className="ci-composition">
            <_CI_Donut segments={segments} center={fmt(breakdown.used)} sub={`${pct}% used`} />
            <_CI_Legend rows={segments} total={breakdown.used} fmt={fmt} />
          </div>
        </_CI_Card>

        <_CI_Card title="Context trend"
          extra={trend.length < requests ? `last ${trend.length} of ${requests} requests` : `${requests} request${requests === 1 ? "" : "s"}`}>
          <div className="ci-key">
            <span><i style={{ background: "var(--accent)" }} />cached</span>
            <span><i style={{ background: "var(--amber)" }} />new input</span>
            {trend.length > 1 && <span className="ci-key-range">{fmt(trend[0].context)} → {fmt(trend[trend.length - 1].context)}</span>}
          </div>
          <_CI_Trend points={trend} fmt={fmt} />
        </_CI_Card>

        <div className="ci-row">
          <_CI_Card title="Tool calls" extra={toolCalls || null}>
            <_CI_Chips empty="No tool calls in context."
              items={insight.tools.map(t => ({ key: t.name, label: t.name, count: t.calls, errors: t.errors, title: `~${fmt(t.tokens)} tokens in context` }))} />
          </_CI_Card>
          <_CI_Card title="Command executions" extra={insight.commands.reduce((n, c) => n + c.count, 0) || null}>
            <_CI_Chips empty="No shell commands in context."
              items={insight.commands.map(c => ({ key: c.program, label: c.program, count: c.count }))} />
          </_CI_Card>
        </div>

        <_CI_Card title="File activity" extra={insight.files.length ? `${insight.files.length} files · ${edited} edited` : null}>
          {insight.files.length === 0 ? <div className="ci-empty">No files read or edited in context.</div> : (
            <div className="ci-files">
              {insight.files.map(f => (
                <div key={f.path} className="ci-file" title={f.path}>
                  <span className="ci-file-path mono">{f.path}</span>
                  {f.reads > 0 && <span className="ci-badge read">{f.reads} read</span>}
                  {f.writes > 0 && <span className="ci-badge write">{f.writes} edit</span>}
                </div>
              ))}
            </div>
          )}
        </_CI_Card>

        <div className="sp-note">
          Text estimates scaled together to the reported total{insight.images > 0 ? `; ${insight.images} image${insight.images === 1 ? "" : "s"} not sized` : ""}.
          Cached is the share of request context read from the provider's prompt cache. Only what follows the latest compaction is counted.
        </div>
      </>
    );
  }

  return (
    <div className="bridge-scrim ci-scrim"
      onMouseDown={e => { downOnScrim.current = e.target === e.currentTarget; }}
      onClick={e => { if (downOnScrim.current && e.target === e.currentTarget) onClose(); }}>
      <div className="stats-panel ci-panel" ref={panelRef} style={initialSize} onClick={e => e.stopPropagation()}>
        <div className="stats-head" {...wc.headProps}>
          {wc.lights}
          <span className="mono" style={{ color: "var(--fg-2)" }}>context details</span>
        </div>
        <div className="stats-body ci-body">{body}</div>
      </div>
    </div>
  );
}

Object.assign(window, { ContextInsightModal });
