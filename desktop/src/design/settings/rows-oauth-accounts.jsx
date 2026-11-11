/* design/settings/rows-oauth-accounts.jsx — Settings → Accounts (window.OMP_SETTINGS_ROWS): the
   signed-in OAuth accounts grouped by provider (accounts.rs → `omp account --json`). One account per
   provider can be active: omp then uses only it and the provider's other accounts are idle. A
   provider with none active rotates its accounts by remaining quota. Each account can be renamed. */

function _SOA_groups(rows) {
  const groups = [];
  for (const row of rows) {
    const last = groups[groups.length - 1];
    if (last && last.provider === row.provider) last.rows.push(row);
    else groups.push({ provider: row.provider, name: row.providerName, rows: [row] });
  }
  return groups;
}

function _SOA_Account({ row, busy, onActivate, onRename }) {
  const T = window.OMP_I18N.t;
  const { SettingsButton: Button } = window;
  const [editing, setEditing] = React.useState(false);
  const [draft, setDraft] = React.useState(row.label ?? "");
  const save = async () => {
    if (await onRename(row, draft)) setEditing(false);
  };
  const stateLabel = row.state === "active" ? T("Active") : row.state === "idle" ? T("Idle") : T("Rotating");
  return (
    <div className={`st-acct-row is-${row.state}`}>
      <div className="st-acct-who">
        {editing ? (
          <input className="st-input" value={draft} autoFocus maxLength={64}
            aria-label={T("Account name")} placeholder={row.email ?? ""}
            onChange={e => setDraft(e.target.value)}
            onKeyDown={e => {
              if (e.key === "Enter") save();
              if (e.key === "Escape") { setDraft(row.label ?? ""); setEditing(false); }
            }} />
        ) : (
          <span className="st-acct-name">{row.name}</span>
        )}
        {row.detail && <small>{row.detail}</small>}
      </div>
      <span className={`st-acct-state is-${row.state}`}>{stateLabel}</span>
      <div className="st-acct-actions">
        {editing ? (
          <>
            <Button accent onClick={save} disabled={busy}>{T("Save")}</Button>
            <Button onClick={() => { setDraft(row.label ?? ""); setEditing(false); }} disabled={busy}>{T("Cancel")}</Button>
          </>
        ) : (
          <>
            <Button accent onClick={() => onActivate(row)} disabled={busy || row.state === "active"}
              title={T("Use only this account for {provider}; its other accounts go idle", { provider: row.providerName })}>
              {T("Activate")}
            </Button>
            <Button onClick={() => { setDraft(row.label ?? ""); setEditing(true); }} disabled={busy}>{T("Rename")}</Button>
          </>
        )}
      </div>
    </div>
  );
}

function _SOA_Accounts({ row, ctx, query }) {
  const { SettingsRow: Row, SettingsButton: Button } = window;
  const T = window.OMP_I18N.t;
  const [state, setState] = React.useState(null);
  const [busy, setBusy] = React.useState(false);
  const apply = React.useCallback(async (call) => {
    setBusy(true);
    const result = await call();
    // A refused change keeps the list on screen and shows why.
    setState(prev => (result.ok || !prev?.ok ? result : { ...prev, error: result.error }));
    setBusy(false);
    return result.ok;
  }, []);
  const load = React.useCallback(() => apply(() => ctx.bridge.accountsList()), [apply, ctx.bridge]);
  React.useEffect(() => { load(); }, [load]);
  const rows = state?.ok ? state.value : [];
  const groups = _SOA_groups(rows);
  return (
    <Row stack label={row.label} desc={row.desc} query={query}>
      <div className="st-quota">
        {!state && <div className="st-fav-empty">{T("Loading accounts…")}</div>}
        {state && !state.ok && <div className="lb-error" role="alert">{state.error}</div>}
        {state?.ok && state.error && <div className="lb-error" role="alert">{state.error}</div>}
        {state?.ok && rows.length === 0 && <div className="st-fav-empty">{T("No OAuth account is signed in. Sign in under Models.")}</div>}
        {groups.map(group => {
          const rotating = group.rows.every(r => r.state === "rotating");
          return (
            <section key={group.provider} className="st-quota-account st-acct-group">
              <h4>
                {group.name}
                <small>{rotating ? T("No active account · accounts rotate") : T("One active account · the others are idle")}</small>
              </h4>
              {group.rows.map(r => (
                <_SOA_Account key={r.credentialId} row={r} busy={busy}
                  onActivate={a => apply(() => ctx.bridge.accountActivate(a.provider, a.credentialId))}
                  onRename={(a, name) => apply(() => ctx.bridge.accountRename(a.provider, a.credentialId, name))} />
              ))}
              {!rotating && group.rows.length > 1 && (
                <div className="st-acct-foot">
                  <Button onClick={() => apply(() => ctx.bridge.accountRotate(group.provider))} disabled={busy}>
                    {T("Rotate all accounts")}
                  </Button>
                </div>
              )}
            </section>
          );
        })}
        <div className="st-chattext-foot"><Button onClick={load} disabled={busy}>{T("Refresh")}</Button></div>
      </div>
    </Row>
  );
}

Object.assign(window.OMP_SETTINGS_ROWS = window.OMP_SETTINGS_ROWS || {}, { oauthAccounts: _SOA_Accounts });
