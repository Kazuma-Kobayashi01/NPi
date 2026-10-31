/* ═════════════════════════════════════════════════════════════════════
   usage-popover.jsx — what the ring in the composer opens: the plan's
   limits (a signed-in subscription) and this conversation's spend. With an
   API key there is no plan, so only the spend and tokens show. The numbers
   come from app/usage-view.js (window.OMP_USAGE); `quotas` is use-quotas'
   result, `stats` the tab's session stats.
   ═════════════════════════════════════════════════════════════════════ */

function UsagePopover({ provider, quotas, stats, context, onSeeAll, onClose }) {
  const U = window.OMP_USAGE;
  const M = window.OMP_MODELS;
  const fmt = window.formatTokens;
  const { mine } = U.quotaFor(quotas?.rows, provider);
  const rows = U.tokenRows(stats);
  const ref = React.useRef(null);
  React.useEffect(() => {
    const away = e => { if (!ref.current?.parentElement?.contains(e.target)) onClose(); };
    const esc = e => { if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); onClose(); } };
    document.addEventListener("mousedown", away);
    document.addEventListener("keydown", esc, true);
    return () => { document.removeEventListener("mousedown", away); document.removeEventListener("keydown", esc, true); };
  }, [onClose]);
  return (
    <div className="usage-pop" ref={ref} role="dialog" aria-label="Usage">
      {mine ? (<>
        <div className="usage-head">
          <span>Plan usage limits{mine.plan ? ` · ${mine.plan}` : ""}</span>
          {onSeeAll && <button type="button" className="usage-all" onClick={() => { onClose(); onSeeAll(); }}>See all →</button>}
        </div>
        {mine.limits.map((l, i) => {
          const left = U.leftPct(l);
          const used = left === null ? null : 100 - left;
          return (
            <div key={`${l.windowId ?? l.label}:${i}`} className="usage-limit">
              <div className="usage-line">
                <span className="usage-name">{l.label || l.windowId || "Limit"}</span>
                {l.resetsAt && <span className="usage-reset">Resets in {U.untilReset(l.resetsAt, Date.now())}</span>}
                <span className="usage-pct">{used === null ? "—" : `${used}%`}</span>
              </div>
              <div className="usage-meter"><span style={{ width: `${used ?? 0}%`, background: used !== null && used > 85 ? "var(--rose)" : used !== null && used > 65 ? "var(--amber)" : "var(--accent)" }} /></div>
            </div>
          );
        })}
      </>) : (
        <div className="usage-head"><span>{provider ? `${M.vendorLabel(provider)} · API usage` : "API usage"}</span></div>
      )}
      <div className="usage-sub">This conversation</div>
      <div className="usage-line"><span className="usage-name">Spend</span><span className="usage-pct">{U.formatCost(stats?.cost)}</span></div>
      {!mine && rows.map(r => (
        <div key={r.key} className="usage-line"><span className="usage-name usage-dim">{r.label}</span><span className="usage-pct">{fmt(r.tokens)}</span></div>
      ))}
      {context && <div className="usage-line"><span className="usage-name usage-dim">Context</span><span className="usage-pct">{(+context.pct).toFixed(0)}%</span></div>}
    </div>
  );
}

Object.assign(window, { UsagePopover });
