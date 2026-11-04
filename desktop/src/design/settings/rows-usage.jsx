/* design/settings/rows-usage.jsx — Settings → Usage (window.OMP_SETTINGS_ROWS): what is left of each
   subscription, as horizontal bars (`usageLimits`, model_config.rs), and the statistics the usage window
   shows (`usageStats`, design/usage-stats-panel.jsx). */

function _SRU_Bar({ bar }) {
  const T = window.OMP_I18N.t;
  return (
    <div className="st-quota-row">
      <div className="st-quota-name">
        <span>{bar.label}</span>
        {bar.reset && <small>{T(bar.reset.key, { time: bar.reset.time })}</small>}
      </div>
      <span className={`st-quota-bar ${bar.status}`} role="progressbar" aria-label={bar.label} aria-valuenow={bar.percent} aria-valuemin={0} aria-valuemax={100}>
        <span style={{ width: `${bar.percent}%` }} />
      </span>
      <span className="st-quota-used">{T("{n}% used", { n: bar.percent })}</span>
    </div>
  );
}

function _SRU_Limits({ row, ctx, query }) {
  const { SettingsRow: Row, SettingsButton: Button } = window;
  const T = window.OMP_I18N.t;
  const U = window.OMP_USAGE_LIMITS;
  const [state, setState] = React.useState(null);
  const [busy, setBusy] = React.useState(false);
  const load = React.useCallback(async force => {
    setBusy(true);
    setState(await ctx.bridge.usageLimits(undefined, force));
    setBusy(false);
  }, [ctx.bridge]);
  React.useEffect(() => { load(false); }, [load]);
  const accounts = (state?.ok ? state.value : []).filter(a => U.bars(a, 0).length > 0);
  return (
    <Row stack label={row.label} desc={row.desc} query={query}>
      <div className="st-quota">
        {!state && <div className="st-fav-empty">{T("Asking the providers…")}</div>}
        {state && !state.ok && <div className="lb-error" role="alert">{state.error}</div>}
        {state?.ok && accounts.length === 0 && <div className="st-fav-empty">{T("No subscription is signed in. Sign in under Models.")}</div>}
        {accounts.map((a, index) => (
          <section key={`${a.provider}:${index}`} className="st-quota-account">
            <h4>{U.heading(a)}</h4>
            {U.bars(a, Date.now()).map(b => <_SRU_Bar key={b.key} bar={b} />)}
          </section>
        ))}
        <div className="st-chattext-foot"><Button onClick={() => load(true)} disabled={busy}>{T("Refresh")}</Button></div>
      </div>
    </Row>
  );
}

function _SRU_Stats({ row, ctx, query }) {
  const { SettingsRow: Row } = window;
  return <Row stack label={row.label} desc={row.desc} query={query}><window.UsageStatsBody embedded /></Row>;
}

Object.assign(window.OMP_SETTINGS_ROWS = window.OMP_SETTINGS_ROWS || {}, { usageLimits: _SRU_Limits, usageStats: _SRU_Stats });
