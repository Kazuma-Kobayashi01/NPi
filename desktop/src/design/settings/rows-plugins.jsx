/* design/settings/rows-plugins.jsx — Settings → Agent → Plugins (window.OMP_SETTINGS_ROWS): the installed
   plugins of the active tab's profile (npix.rs → `omp plugin list --json`, each with its source and the
   developer from its `developer.json`), uninstall (`omp plugin uninstall`), and installing a `.npix`
   package (`omp plugin inspect|install <file> --json`): the package is inspected first and installed only
   after the user has seen what it declares and who made it. Tabs started afterwards load the change. */

function _SPL_size(bytes) {
  if (bytes < 1024 * 1024) return `${Math.max(1, Math.round(bytes / 1024))} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/** "By Ada Lovelace · Analytical Engines · ada@example.com · https://…". The homepage stays text: it is the
 *  plugin's own claim, and a link would open inside the app. */
function _SPL_Developer({ developer }) {
  const T = window.OMP_I18N.t;
  if (!developer) return <small>{T("Developer not stated")}</small>;
  const who = [developer.name, developer.organization, developer.email, developer.url].filter(Boolean).join(" · ");
  return <small>{T("By {who}", { who })}</small>;
}

function _SPL_Details({ info }) {
  const T = window.OMP_I18N.t;
  const kinds = [
    ["extensions", T("Extensions")],
    ["tools", T("Tools")],
    ["hooks", T("Hooks")],
    ["commands", T("Commands")],
    ["features", T("Features")],
  ];
  return (
    <section className="st-quota-account st-acct-group">
      <h4>
        {info.name} <small>{info.version}</small>
      </h4>
      <div className="st-acct-who"><_SPL_Developer developer={info.developer} /></div>
      {info.description && <div className="st-acct-who"><small>{info.description}</small></div>}
      {kinds.filter(([key]) => info.capabilities?.[key]?.length).map(([key, label]) => (
        <div key={key} className="st-acct-who">
          <small>{label}: {info.capabilities[key].join(", ")}</small>
        </div>
      ))}
      {info.bundles?.skills && <div className="st-acct-who"><small>{T("Bundles skills")}</small></div>}
      {info.bundles?.mcp && <div className="st-acct-who"><small>{T("Bundles MCP servers")}</small></div>}
      <div className="st-acct-who">
        <small>{T("{n} files, {size} unpacked", { n: info.files, size: _SPL_size(info.bytes) })}</small>
      </div>
    </section>
  );
}

function _SPL_Installed({ plugin, busy, onUninstall }) {
  const T = window.OMP_I18N.t;
  const { SettingsButton: Button } = window;
  const [asking, setAsking] = React.useState(false);
  const description = plugin.manifest?.description;
  return (
    <div className={`st-acct-row ${plugin.enabled ? "is-active" : "is-idle"}`}>
      <div className="st-acct-who">
        <span className="st-acct-name">{plugin.name} <small>{plugin.version}</small></span>
        <_SPL_Developer developer={plugin.developer} />
        {description && <small>{description}</small>}
      </div>
      <span className={`st-acct-state ${plugin.enabled ? "is-active" : "is-idle"}`}>
        {plugin.npix ? ".npix" : T("Other source")}{plugin.enabled ? "" : ` · ${T("Disabled")}`}
      </span>
      <div className="st-acct-actions">
        {asking ? (
          <>
            <Button accent onClick={() => onUninstall(plugin)} disabled={busy}>{T("Confirm uninstall")}</Button>
            <Button onClick={() => setAsking(false)} disabled={busy}>{T("Cancel")}</Button>
          </>
        ) : (
          <Button onClick={() => setAsking(true)} disabled={busy}>{T("Uninstall")}</Button>
        )}
      </div>
    </div>
  );
}

function _SPL_Plugins({ row, ctx, query }) {
  const { SettingsRow: Row, SettingsButton: Button } = window;
  const T = window.OMP_I18N.t;
  // `installed`: the last list read (`{ok, value|error}`); `pending`: an inspected package waiting for the
  // user's go-ahead; `note`: the last outcome.
  const [installed, setInstalled] = React.useState(null);
  const [pending, setPending] = React.useState(null);
  const [note, setNote] = React.useState(null);
  const [busy, setBusy] = React.useState(false);
  const load = React.useCallback(async () => setInstalled(await ctx.bridge.pluginsList()), [ctx.bridge]);
  React.useEffect(() => { load(); }, [load]);
  const plugins = installed?.ok ? (installed.value?.npm ?? []) : [];

  const pick = async () => {
    setNote(null);
    const path = await ctx.bridge.npixPick();
    if (!path) return;
    setBusy(true);
    const result = await ctx.bridge.npixInspect(path);
    setBusy(false);
    if (result.ok) setPending({ path, info: result.value });
    else setNote({ ok: false, text: result.error });
  };
  const install = async () => {
    setBusy(true);
    const result = await ctx.bridge.npixInstall(pending.path);
    setNote(result.ok
      ? { ok: true, text: T("Installed {name} {version}. Tabs opened from now on load it.", { name: pending.info.name, version: pending.info.version }) }
      : { ok: false, text: result.error });
    if (result.ok) {
      setPending(null);
      await load();
    }
    setBusy(false);
  };
  const uninstall = async plugin => {
    setBusy(true);
    const result = await ctx.bridge.pluginUninstall(plugin.name);
    setNote(result.ok
      ? { ok: true, text: T("Uninstalled {name}. Tabs opened from now on no longer load it.", { name: plugin.name }) }
      : { ok: false, text: result.error });
    await load();
    setBusy(false);
  };

  return (
    <Row stack label={row.label} desc={row.desc} query={query}>
      <div className="st-quota">
        {!installed && <div className="st-fav-empty">{T("Loading plugins…")}</div>}
        {installed && !installed.ok && <div className="lb-error" role="alert">{installed.error}</div>}
        {installed?.ok && plugins.length === 0 && <div className="st-fav-empty">{T("No plugins installed.")}</div>}
        {plugins.length > 0 && (
          <section className="st-quota-account st-acct-group">
            {plugins.map(plugin => (
              <_SPL_Installed key={plugin.name} plugin={plugin} busy={busy} onUninstall={uninstall} />
            ))}
          </section>
        )}
        {pending && (
          <>
            <_SPL_Details info={pending.info} />
            <div className="st-fav-empty">{T("A plugin runs with the same access as NPi itself. Install only plugins you trust.")}</div>
          </>
        )}
        {note && (note.ok
          ? <div className="st-fav-empty" role="status">{note.text}</div>
          : <div className="lb-error" role="alert">{note.text}</div>)}
        <div className="st-chattext-foot">
          {pending ? (
            <>
              <Button accent onClick={install} disabled={busy}>{T("Install")}</Button>
              <Button onClick={() => setPending(null)} disabled={busy}>{T("Cancel")}</Button>
            </>
          ) : (
            <>
              <Button onClick={pick} disabled={busy}>{busy ? T("Reading package…") : T("Choose a .npix file…")}</Button>
              <Button onClick={load} disabled={busy}>{T("Refresh")}</Button>
            </>
          )}
        </div>
      </div>
    </Row>
  );
}

Object.assign(window.OMP_SETTINGS_ROWS = window.OMP_SETTINGS_ROWS || {}, { npixInstall: _SPL_Plugins });
