/* ═════════════════════════════════════════════════════════════════════
   design/settings/rows-models.jsx — Settings → Models (window.OMP_SETTINGS_ROWS)
     defaultModel    the model new tabs start on (omp's `modelRoles.default`)
     favoriteModels  which models the small picker lists (an app setting)
     vendors         the vendors omp can use now, and signing in to more

   Models and the default come from omp's CLI (`omp models --json`,
   `omp config`; src-tauri/src/model_config.rs), so no session is needed for
   them. Signing in is omp's own OAuth flow over RPC (`login`), which needs a
   running tab: the app forwards the browser step to the system browser.
   ═════════════════════════════════════════════════════════════════════ */

/** The catalog and the default, loaded together; `reload` re-reads both. */
function _SRM_useCatalog(ctx) {
  const [state, setState] = React.useState({ loading: true, models: [], defaultSel: "", error: "" });
  const bridge = ctx.bridge;
  const reload = React.useCallback(() => {
    let live = true;
    setState(s => ({ ...s, loading: true }));
    Promise.all([bridge?.modelsCatalog?.(), bridge?.defaultModel?.()]).then(([cat, def]) => {
      if (!live) return;
      setState({
        loading: false,
        models: cat?.ok && Array.isArray(cat.value) ? cat.value : [],
        defaultSel: def?.ok ? (def.value ?? "") : "",
        error: cat && !cat.ok ? String(cat.error ?? "could not list the models") : "",
      });
    });
    return () => { live = false; };
  }, [bridge, ctx.profileId]);
  React.useEffect(() => reload(), [reload]);
  return { ...state, reload };
}

function _SRM_Default({ row, ctx, query }) {
  const { SettingsRow: Row } = window;
  const M = window.OMP_MODELS;
  const cat = _SRM_useCatalog(ctx);
  const [error, setError] = React.useState("");
  const [key, level = ""] = (cat.defaultSel || "").split(":");
  const groups = React.useMemo(() => M.groupByVendor(cat.models), [cat.models]);
  const model = cat.models.find(m => M.keyOf(m) === key);
  const levels = model?.thinking ?? [];
  const save = async (k, l) => {
    setError("");
    const r = await ctx.bridge?.setDefaultModel?.(l ? `${k}:${l}` : k);
    if (r && !r.ok) setError(String(r.error ?? "could not save"));
    cat.reload();
  };
  return (
    <>
      <Row label={row.label} desc={row.desc} query={query}>
        <div className="st-modelsel">
          <select value={model ? key : ""} aria-label="Default model" onChange={e => save(e.target.value, "")} disabled={cat.models.length === 0}>
            {!model && <option value="">{cat.loading ? "Loading…" : (cat.models.length ? "Not set" : "No models")}</option>}
            {groups.map(g => (
              <optgroup key={g.vendor} label={g.vendor}>
                {g.models.map(m => <option key={M.keyOf(m)} value={M.keyOf(m)}>{m.name}</option>)}
              </optgroup>
            ))}
          </select>
          {levels.length > 0 && (
            <select value={levels.includes(level) ? level : ""} aria-label="Default effort" onChange={e => save(key, e.target.value)}>
              <option value="">Effort: model default</option>
              {levels.map(l => <option key={l} value={l}>{l.charAt(0).toUpperCase() + l.slice(1)}</option>)}
            </select>
          )}
        </div>
      </Row>
      {(error || cat.error) && <div className="st-row-error">{error || cat.error}</div>}
    </>
  );
}

function _SRM_Favorites({ row, ctx, query }) {
  const { SettingsRow: Row } = window;
  const M = window.OMP_MODELS;
  const cat = _SRM_useCatalog(ctx);
  const favorites = Array.isArray(ctx.t.favoriteModels) ? ctx.t.favoriteModels : [];
  const groups = React.useMemo(() => M.groupByVendor(cat.models), [cat.models]);
  return (
    <Row stack label={row.label} desc={row.desc} query={query}>
      <div className="st-favs">
        {groups.map(g => (
          <div key={g.vendor} className="st-fav-group">
            <div className="st-fav-vendor">{g.vendor}</div>
            <div className="st-fav-list">
              {g.models.map(m => {
                const k = M.keyOf(m);
                const on = favorites.includes(k);
                return (
                  <button key={k} type="button" className={`st-fav${on ? " on" : ""}`} aria-pressed={on}
                    onClick={() => ctx.setTweak("favoriteModels", M.toggleFavorite(favorites, k))}>
                    {on && <span className="st-fav-tick" aria-hidden="true">✓</span>}{m.name}
                  </button>
                );
              })}
            </div>
          </div>
        ))}
        {groups.length === 0 && <div className="st-fav-empty">{cat.loading ? "Loading models…" : "No models yet. Add a vendor below."}</div>}
        {favorites.length > 0 && <div className="st-fav-empty">The picker lists every ticked model, sorted by vendor.</div>}
      </div>
    </Row>
  );
}

function _SRM_Vendors({ row, ctx, query }) {
  const { SettingsRow: Row, SettingsButton: Button } = window;
  const M = window.OMP_MODELS;
  const cat = _SRM_useCatalog(ctx);
  const [providers, setProviders] = React.useState(null); // omp's OAuth providers; null: not loaded
  const [busy, setBusy] = React.useState("");
  const connected = ctx.bridge?.isConnected;
  const loadProviders = React.useCallback(() => {
    if (!ctx.bridge?.isConnected) { setProviders(null); return; }
    ctx.bridge.getLoginProviders().then(d => setProviders(d?.providers ?? []), () => setProviders([]));
  }, [ctx.bridge, ctx.activeProject?.id]);
  React.useEffect(loadProviders, [loadProviders]);
  const groups = React.useMemo(() => M.groupByVendor(cat.models), [cat.models]);

  const login = async (p) => {
    if (ctx.close) ctx.close();
    if (p) {
      await ctx.onLogin?.(p);
    } else if (ctx.openLogin) {
      ctx.openLogin();
    }
  };

  return (
    <Row stack label={row.label} desc={row.desc} query={query}>
      <div className="st-vendors">
        <div className="st-vendor-head">Connected</div>
        {groups.map(g => (
          <div key={g.vendor} className="st-vendor">
            <span className="st-vendor-name">{g.vendor}</span>
            <span className="st-vendor-meta">{g.models.length} model{g.models.length === 1 ? "" : "s"}</span>
          </div>
        ))}
        {groups.length === 0 && <div className="st-fav-empty">{cat.loading ? "Loading…" : "No vendor is signed in yet."}</div>}

        <div className="st-vendor-head">Add a vendor</div>
        {!connected && (
          <div className="st-fav-empty">Signing in runs inside a conversation: open a project first, then come back here.</div>
        )}
        {connected && providers === null && <div className="st-fav-empty">Checking what can be signed in…</div>}
        {connected && providers !== null && providers.length === 0 && <div className="st-fav-empty">omp offers no vendors to sign in to.</div>}
        {connected && providers !== null && providers.map(p => (
          <div key={p.id} className="st-vendor">
            <span className="st-vendor-name">{p.name}</span>
            <span className="st-vendor-meta">{p.authenticated ? "signed in" : p.available ? "" : "unavailable"}</span>
            <Button onClick={() => login(p)} disabled={!!busy || (!p.available && !p.authenticated)}>
              {busy === p.id ? "Waiting for the browser…" : p.authenticated ? "Sign in again" : "Sign in"}
            </Button>
          </div>
        ))}
        {connected && <div className="st-fav-empty">A vendor that uses an API key signs in from a terminal: run <code>npi login</code>.</div>}
        {connected && (
          <div style={{ marginTop: "10px" }}>
            <Button onClick={() => { if (ctx.close) ctx.close(); ctx.openLogin?.(); }}>
              Open /login menu…
            </Button>
          </div>
        )}
      </div>
    </Row>
  );
}

Object.assign(window.OMP_SETTINGS_ROWS = window.OMP_SETTINGS_ROWS || {}, {
  defaultModel: _SRM_Default,
  favoriteModels: _SRM_Favorites,
  vendors: _SRM_Vendors,
});
