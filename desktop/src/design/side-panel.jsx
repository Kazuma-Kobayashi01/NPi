/* ═════════════════════════════════════════════════════════════════════
   side-panel.jsx — the right panel's cards, always shown for a tab:
     Environment  where the conversation runs: computer, folder, branch
     Context      how full the model's window is, split into its parts
     Usage        the subscription's remaining limits (a signed-in
                  subscription) and this conversation's spend and tokens
                  (always: with an API key those are all there is)
   The numbers are prepared in app/usage-view.js; this only draws them.
   ═════════════════════════════════════════════════════════════════════ */

const { Icon: _SP2_Icon } = window;

function SideCard({ title, extra, children, startOpen = true }) {
  const [open, setOpen] = React.useState(startOpen);
  return (
    <section className="sp-card">
      <button type="button" className="sp-head" aria-expanded={open} onClick={() => setOpen(v => !v)}>
        <span className="sp-title">{title}</span>
        {extra}
        <svg className={`sp-chev${open ? " open" : ""}`} width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
          <path d="M2 3.5 5 6.5 8 3.5" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
      </button>
      {open && <div className="sp-body">{children}</div>}
    </section>
  );
}

const _sp2Home = (p) => String(p ?? "").replace(/^\/Users\/[^/]+(?=\/|$)/, "~");

function EnvironmentCard({ project, profileLabel }) {
  const path = _sp2Home(project?.path);
  const parent = path.includes("/") ? path.slice(0, path.lastIndexOf("/")) || "/" : "";
  return (
    <SideCard title="Environment">
      <div className="sp-row">
        <svg width="15" height="15" viewBox="0 0 16 16" fill="none" aria-hidden="true">
          <rect x="2.5" y="3" width="11" height="7.5" rx="1.2" stroke="currentColor" strokeWidth="1.3" />
          <path d="M1.5 12.5h13" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" />
        </svg>
        <span className="sp-label">This computer</span>
      </div>
      {project?.path && (
        <div className="sp-row" title={project.path}>
          <_SP2_Icon name="folder" size={15} />
          <span className="sp-label">{window.OMP_PROJECT_NAV.basename(project.path)}</span>
          <span className="sp-dim sp-ellip">{parent}</span>
        </div>
      )}
      {project?.branch && (
        <div className="sp-row">
          <_SP2_Icon name="branch" size={15} />
          <span className="sp-label">{project.branch}</span>
        </div>
      )}
      {profileLabel && (
        <div className="sp-row">
          <_SP2_Icon name="agent" size={15} />
          <span className="sp-label">Profile</span>
          <span className="sp-dim">{profileLabel}</span>
        </div>
      )}
    </SideCard>
  );
}

const _SP2_COLORS = { messages: "var(--accent)", compacted: "var(--cyan)", system: "var(--lilac)", tools: "var(--amber)", free: "var(--line-bright)" };

function ContextCard({ breakdown, onCompact, compacting, disabled }) {
  const fmt = window.formatTokens;
  if (!breakdown) {
    return <SideCard title="Context"><div className="sp-dim">Nothing sent yet.</div></SideCard>;
  }
  const { total, used, pct, parts } = breakdown;
  return (
    <SideCard title="Context">
      <div className="sp-big">
        <span>{pct < 1 && used > 0 ? "<1" : Math.round(pct)}% used</span>
        <span className="sp-dim">{fmt(used)} / {fmt(total)}</span>
      </div>
      <div className="sp-bar" role="img" aria-label={`${Math.round(pct)} percent of the context window used`}>
        {parts.filter(p => p.key !== "free" && p.tokens > 0).map(p => (
          <span key={p.key} style={{ width: `${(p.tokens / total) * 100}%`, background: _SP2_COLORS[p.key] }} title={`${p.label}: ${fmt(p.tokens)}`} />
        ))}
      </div>
      <div className="sp-table">
        {parts.map(p => (
          <div key={p.key} className="sp-trow">
            <span className="sp-swatch" style={{ background: _SP2_COLORS[p.key] }} />
            <span className="sp-label">{p.label}</span>
            <span className="sp-num">{fmt(p.tokens)}</span>
            <span className="sp-num sp-dim">{((p.tokens / total) * 100).toFixed(1)}%</span>
          </div>
        ))}
      </div>
      <div className="sp-note">Parts are text estimates reconciled to the reported total. Native payloads and images are not separately sized.</div>
      {onCompact && (
        <div className="sp-foot-action">
          <button
            type="button"
            className="btn ghost is-floating sp-compact-btn"
            disabled={disabled || compacting || used === 0}
            onClick={onCompact}
            title={compacting ? "Compacting context..." : "Compact context now"}
          >
            {compacting ? (
              <span className="sp-compact-status">
                <span className="pulse-dot" /> Compacting…
              </span>
            ) : (
              <span>Compact Context</span>
            )}
          </button>
        </div>
      )}
    </SideCard>
  );
}

function _SP2_Limits({ row }) {
  const U = window.OMP_USAGE;
  return (
    <div className="sp-limits">
      {row.limits.map((l, i) => {
        const used = U.usedPct(l);
        return (
          <div key={`${l.windowId ?? l.label}:${i}`} className="sp-limit">
            <div className="sp-trow">
              <span className="sp-label">{l.label || l.windowId || "Limit"}</span>
              <span className="sp-num">{used === null ? "—" : `${used}% used`}</span>
            </div>
            <div className="sp-meter"><span style={{ width: `${used ?? 0}%`, background: used !== null && used > 85 ? "var(--rose)" : used !== null && used > 65 ? "var(--amber)" : "var(--accent)" }} /></div>
            {l.resetsAt && <div className="sp-note">Resets in {U.untilReset(l.resetsAt, Date.now())}</div>}
          </div>
        );
      })}
    </div>
  );
}

function UsageCard({ provider, quotas, stats }) {
  const U = window.OMP_USAGE;
  const M = window.OMP_MODELS;
  const fmt = window.formatTokens;
  const { mine, others } = U.quotaFor(quotas?.rows, provider);
  const rows = U.tokenRows(stats);
  const title = mine ? "Plan usage" : "Usage";
  const [othersOpen, setOthersOpen] = React.useState(false);
  return (
    <SideCard title={title}
      extra={quotas?.refresh && (
        <span className="sp-refresh" role="button" tabIndex={0} title="Refresh limits"
          onClick={e => { e.stopPropagation(); quotas.refresh(); }}
          onKeyDown={e => { if (e.key === "Enter") { e.stopPropagation(); quotas.refresh(); } }}>
          {quotas.loading ? "…" : "↻"}
        </span>
      )}>
      {mine && (<>
        <div className="sp-sub">{M.vendorLabel(mine.provider)}{mine.plan ? ` · ${mine.plan}` : ""}</div>
        <_SP2_Limits row={mine} />
      </>)}
      {!mine && quotas?.loading && !quotas.rows && <div className="sp-dim">Checking limits…</div>}
      {quotas?.error && <div className="sp-dim" role="alert">Limits unavailable: {quotas.error}{quotas.rows ? " (showing last available)" : ""}</div>}
      {others.length > 0 && (<>
        <button type="button" className="sp-sub sp-fold" aria-expanded={othersOpen} onClick={() => setOthersOpen(v => !v)}>
          <span>Other accounts</span>
          <svg className={`sp-chev${othersOpen ? " open" : ""}`} width="10" height="10" viewBox="0 0 10 10" aria-hidden="true">
            <path d="M2 3.5 5 6.5 8 3.5" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
        </button>
        {othersOpen && others.map(r => (
          <div key={r.provider} className="sp-other">
            <div className="sp-trow"><span className="sp-label">{M.vendorLabel(r.provider)}{r.plan ? ` · ${r.plan}` : ""}</span></div>
            {r.limits.map((l, i) => (
              <div key={i} className="sp-trow sp-small">
                <span className="sp-dim">{l.label || l.windowId}</span>
                <span className="sp-num">{U.usedPct(l) === null ? "—" : `${U.usedPct(l)}% used`}</span>
              </div>
            ))}
          </div>
        ))}
      </>)}

      <div className="sp-sub">This conversation</div>
      <div className="sp-trow">
        <span className="sp-label">Spend</span>
        <span className="sp-num">{U.formatCost(stats?.cost)}</span>
      </div>
      {rows.map(r => (
        <div key={r.key} className="sp-trow">
          <span className="sp-label sp-dim">{r.label}</span>
          <span className="sp-num">{fmt(r.tokens)}</span>
        </div>
      ))}
      {rows.length === 0 && <div className="sp-dim">No tokens used yet.</div>}
    </SideCard>
  );
}

function TodoPlanCard({ counts, kanban }) {
  const hasTasks = counts.total > 0;
  const inProgress = kanban.some(col => col.tasks.some(task => task.status === "in_progress"));
  const title = (
    <span className="sp-todo-label">
      <span className="sp-todo-icon">
        <_SP2_Icon name="plan" size={13} color="var(--accent)" />
        {inProgress && <span className="sp-todo-live" />}
      </span>
      <span>Todo and plan</span>
    </span>
  );
  return (
    <div className="sp-todo-card">
      <SideCard title={title} startOpen={false}
        extra={<span className="chip muted mono">{hasTasks ? `${counts.done}/${counts.total}` : "idle"}</span>}>
        {hasTasks ? kanban.map(col => (
          <section key={col.id} className="sp-todo-phase">
            <h3>{col.title}</h3>
            <ul className="sp-todo-list">
              {col.tasks.map(task => (
                <li key={task.id} className={`sp-todo-task ${task.status}`}>
                  <span className="sp-todo-mark" aria-label={task.status === "done" ? "Done" : task.status === "in_progress" ? "In progress" : "Pending"}>
                    {task.status === "done" ? "✓" : task.status === "in_progress" ? "●" : "○"}
                  </span>
                  <span className="sp-todo-text">
                    {task.text}
                    {task.reason && <span className="sp-todo-reason">{task.reason}</span>}
                  </span>
                </li>
              ))}
            </ul>
          </section>
        )) : <div className="sp-dim">No tasks in this session.</div>}
      </SideCard>
    </div>
  );
}

Object.assign(window, { SideCard, EnvironmentCard, ContextCard, UsageCard, TodoPlanCard });
