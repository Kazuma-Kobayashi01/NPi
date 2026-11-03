/* Usage/cost dashboard scoped to the active profile and selected time range.
   app-live.jsx keys the panel by profile, so changing profiles remounts it. */

const { Icon: _StatsIcon } = window;

function formatCost(n) {
  return n === null || n === undefined ? "—" : `$${n.toFixed(2)}`;
}

function formatPct(n) {
  return n === null || n === undefined ? "—" : `${(n * 100).toFixed(1)}%`;
}

function formatMs(n) {
  if (n === null || n === undefined) return "—";
  return n >= 1000 ? `${(n / 1000).toFixed(1)}s` : `${Math.round(n)}ms`;
}

function formatCount(n) {
  // `total_premium_requests` (and nothing else this panel renders) is a
  // SQL SUM of a fractional REAL column (provider premium multipliers
  // include values like 0.33/0.25), so it needs its own formatter rather
  // than the integer-assuming `.toLocaleString()` used elsewhere —
  // rounds to 2 places, matching upstream's own `normalizePremiumRequests`.
  return n === null || n === undefined
    ? "—"
    : n.toLocaleString(undefined, { maximumFractionDigits: 2 });
}

// `omp stats`'s own `folder` field observed in practice: a cwd under
// $HOME is stored home-relative with `/` replaced by `-` (e.g.
// `/home/user/my-project` → `-my-project`, NOT the full absolute path
// with every segment flattened), and a cwd equal to `$HOME` itself
// encodes to the bare string `-`. A cwd outside $HOME/tmp instead keeps
// a real absolute-path shape (`/opt-foo/` style). Stripping the leading
// `-` therefore isn't guaranteed to leave a non-empty, meaningful name —
// a home-directory session strips down to "", which without the
// fallback below would render as a blank row (the real key is still
// available via the `title` tooltip either way).
function displayFolder(folder) {
  return folder.replace(/^-/, "") || "~";
}

const EMPTY = { overall: null, byModel: [], byFolder: [], byAgentType: [] };
const STATS_RANGES = [
  { value: "24h", label: "24h", window: "last 24 hours" },
  { value: "7d", label: "7d", window: "last 7 days" },
  { value: "30d", label: "1mo", window: "last 30 days" },
  { value: "all", label: "Total", window: "all local history" },
];

// Stats records normalize input to uncached input, with cache counts separate.
function statsTotalTokens(stats) {
  return stats.totalInputTokens + stats.totalOutputTokens +
    stats.totalCacheReadTokens + stats.totalCacheWriteTokens;
}

// Mounted only while open (app-live.jsx gates on `statsOpen`), so there
// is no `open` prop and no early return for it — see the matching note
// in approval-rules-panel.jsx.
function UsageStatsPanel({ onClose, profileLabel }) {
  const bridge = window.OMP_BRIDGE;
  const panelRef = React.useRef(null);
  const wc = window.useWindowChrome(onClose, panelRef);
  const [range, setRange]     = React.useState("all");
  const [result, setResult]   = React.useState(null);
  const [loading, setLoading] = React.useState(true);
  const request = React.useRef(0);

  const refresh = React.useCallback(async () => {
    const id = ++request.current;
    setLoading(true);
    setResult(null);
    const res = await bridge.usageStats(range);
    if (id !== request.current) return;
    setResult({ range, res });
    setLoading(false);
  }, [bridge, range]);

  React.useEffect(() => {
    refresh();
    return () => { request.current++; };
  }, [refresh]);

  const selectRange = value => {
    if (value === range) return;
    request.current++;
    setRange(value);
  };
  const selected = STATS_RANGES.find(item => item.value === range);
  const current = result?.range === range ? result.res : null;
  const overall = current?.ok && current.value.overall.totalRequests > 0 ? current.value.overall : null;
  const data = overall ? current.value : EMPTY;

  return (
    <div className="bridge-scrim" onClick={onClose} style={{ paddingTop: "6vh" }}>
      <div className="stats-panel" ref={panelRef} onClick={e => e.stopPropagation()}>
        <div className="stats-head" {...wc.headProps}>
          {wc.lights}
          <_StatsIcon name="cost" size={13} color="var(--accent)" />
          <span className="mono" style={{ color: "var(--fg-2)" }}>usage · {profileLabel} · {selected.window}</span>
          <button className="btn icon ghost" style={{ marginLeft: "auto" }} onClick={refresh} title="Refresh">
            <_StatsIcon name="refresh" size={11} />
          </button>
        </div>
        <div className="stats-body">
          <div className="stats-ranges mode-seg" role="radiogroup" aria-label="Usage time range">
            {STATS_RANGES.map(item => (
              <button key={item.value} type="button" role="radio" aria-checked={range === item.value}
                onClick={() => selectRange(item.value)}>{item.label}</button>
            ))}
          </div>
          {(loading || !current) && <div className="panel-empty mono">syncing session logs…</div>}
          {!loading && current && !current.ok && <div className="panel-empty mono">{current.error}</div>}
          {!loading && current?.ok && !overall && <div className="panel-empty mono">no usage in {selected.window}</div>}
          {!loading && overall && (
            <>
              <div className="stats-cards">
                <div className="stats-card">
                  <span className="stats-card-label">requests</span>
                  <span className="stats-card-value">{overall.totalRequests.toLocaleString()}</span>
                  <span className="stats-card-sub">{formatPct(overall.errorRate)} error rate</span>
                </div>
                <div className="stats-card">
                  <span className="stats-card-label">cost</span>
                  <span className="stats-card-value">{formatCost(overall.totalCost)}</span>
                  <span className="stats-card-sub">{formatCount(overall.totalPremiumRequests)} premium{overall.unpricedRequests > 0 ? ` · ${overall.unpricedRequests} unpriced` : ""}</span>
                </div>
                <div className="stats-card">
                  <span className="stats-card-label">total tokens</span>
                  <span className="stats-card-value">{window.formatTokens(statsTotalTokens(overall))}</span>
                  <span className="stats-card-sub">uncached input + output + cache read + cache write</span>
                </div>
                <div className="stats-card">
                  <span className="stats-card-label">performance</span>
                  <span className="stats-card-value">
                    {overall.avgTokensPerSecond == null ? "—" : `${Math.round(overall.avgTokensPerSecond)} t/s`}
                  </span>
                  <span className="stats-card-sub">
                    {formatMs(overall.avgTtft)} ttft · {formatMs(overall.avgDuration)} avg
                  </span>
                </div>
              </div>

              <div className="stats-section-label mono">by provider / model</div>
              <div className="stats-table">
                {data.byModel.length === 0 && <div className="panel-empty mono">no data</div>}
                {data.byModel.map(m => (
                  <div key={JSON.stringify([m.provider, m.model])} className="stats-model-row">
                    <div className="stats-row">
                      <span className="stats-row-name mono" title={`${m.provider}/${m.model}`}>{m.provider} / {m.model}</span>
                      <span className="stats-row-metric mono">{window.formatTokens(statsTotalTokens(m))} tokens</span>
                      <span className="stats-row-metric mono">{m.totalRequests.toLocaleString()} req</span>
                      <span className="stats-row-metric mono">{formatCost(m.totalCost)}</span>
                    </div>
                    <div className="stats-model-tokens mono">
                      <span>uncached input {window.formatTokens(m.totalInputTokens)}</span>
                      <span>output {window.formatTokens(m.totalOutputTokens)}</span>
                      <span>cache read {window.formatTokens(m.totalCacheReadTokens)}</span>
                      <span>cache write {window.formatTokens(m.totalCacheWriteTokens)}</span>
                      {m.unpricedRequests > 0 && <span>{m.unpricedRequests} unpriced</span>}
                    </div>
                  </div>
                ))}
              </div>

              <div className="stats-section-label mono">by folder</div>
              <div className="stats-table">
                {data.byFolder.length === 0 && <div className="panel-empty mono">no data</div>}
                {data.byFolder.map(f => (
                  <div key={f.folder} className="stats-row">
                    <span className="stats-row-name mono" title={f.folder}>{displayFolder(f.folder)}</span>
                    <span className="stats-row-metric mono">{f.totalRequests.toLocaleString()} req</span>
                    <span className="stats-row-metric mono">{formatCost(f.totalCost)}</span>
                  </div>
                ))}
              </div>

              {data.byAgentType.length > 0 && (
                <>
                  <div className="stats-section-label mono">by agent type</div>
                  <div className="stats-chips">
                    {data.byAgentType.map(a => (
                      <span key={a.agentType} className="chip muted mono">
                        {a.agentType} · {a.totalRequests.toLocaleString()} req · {formatCost(a.totalCost)}
                      </span>
                    ))}
                  </div>
                </>
              )}
            </>
          )}
        </div>
      </div>
    </div>
  );
}

window.UsageStatsPanel = UsageStatsPanel;
