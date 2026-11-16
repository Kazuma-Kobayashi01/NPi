/* ═════════════════════════════════════════════════════════════════════
   Settings custom row: removing the Sandbox VM (Settings → Approvals).
     vmCleanup   asks first, then the active tab's plugin cleans up
                 (`sandbox-vm/clean`: stops the VM, resets a persisted
                 exec.backend, deletes local images, sandbox-vm.json and
                 stale terminal routes) and, when ticked, the plugin is
                 uninstalled from ctx.profileId (OMP_BRIDGE.pluginUninstall),
                 which takes its bundled images with it.
   ═════════════════════════════════════════════════════════════════════ */

const _SRVm_PLUGIN = "@npi/plugin-sandbox-vm";

function _SRVm_size(bytes) {
  if (!bytes) return "0 MB";
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

function _SRVm_Cleanup({ row, ctx, query }) {
  const { SettingsRow: Row, SettingsButton: Btn } = window;
  const T = (key, vars) => window.OMP_I18N.t(key, vars);
  const { bridge, profileId } = ctx;
  const [asking, setAsking] = React.useState(false);
  const [uninstall, setUninstall] = React.useState(true);
  const [busy, setBusy] = React.useState(false);
  const [note, setNote] = React.useState(null);

  const run = async () => {
    setBusy(true);
    setNote(null);
    let done;
    try {
      done = await bridge.extensionCommand("sandbox-vm/clean");
    } catch (err) {
      setNote({ ok: false, text: String(err?.message || err) });
      setBusy(false);
      return;
    }
    const lines = [T("Cleaned up: {size} of images built here freed.", { size: _SRVm_size(done.freedBytes) })];
    if (done.backendReset) lines.push(T("Commands run on this machine again."));
    if (done.persistedBackend) lines.push(T("A project config still sets exec.backend to the VM; remove it there."));
    let ok = !done.persistedBackend;
    if (uninstall) {
      const res = await bridge.pluginUninstall(_SRVm_PLUGIN, profileId);
      if (res.ok) lines.push(T("Plugin uninstalled ({size} of bundled images). Open tabs drop it when they restart.", { size: _SRVm_size(done.bundledBytes) }));
      else { ok = false; lines.push(res.error); }
    }
    setNote({ ok, text: lines.join(" ") });
    setAsking(false);
    setBusy(false);
  };

  return (
    <>
      <Row label={row.label} desc={row.desc} query={query}>
        {asking ? (
          <span className="st-vm-folder">
            <label className="st-vm-check">
              <input type="checkbox" checked={uninstall} disabled={busy} onChange={e => setUninstall(e.target.checked)} />
              {T("Also uninstall the plugin")}
            </label>
            <Btn accent onClick={run} disabled={busy}>{busy ? T("Cleaning up…") : T("Confirm clean up")}</Btn>
            <Btn onClick={() => setAsking(false)} disabled={busy}>{T("Cancel")}</Btn>
          </span>
        ) : (
          <Btn onClick={() => { setNote(null); setAsking(true); }}>{T("Clean up…")}</Btn>
        )}
      </Row>
      {note && (note.ok
        ? <div className="st-fav-empty" role="status">{note.text}</div>
        : <div className="st-row-error" role="alert">{note.text}</div>)}
    </>
  );
}

Object.assign(window.OMP_SETTINGS_ROWS = window.OMP_SETTINGS_ROWS || {}, { vmCleanup: _SRVm_Cleanup });
