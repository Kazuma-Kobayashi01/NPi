/* Host permissions are read on mount/focus; requests only follow a click.
   Runtime status comes from the real worker, never the saved preference.
   macOS answers a request asynchronously, so a request leaves the row in an
   explicit "waiting for macOS" state and re-reads a couple of times after. */
function useComputerUse(bridge, profileId, sessionId) {
  const [view, setView] = React.useState({ config: null, runtime: null, busy: false, error: null, awaiting: null });
  const scope = React.useMemo(() => ({ profileId, sessionId }), [bridge, profileId, sessionId]);
  const current = React.useRef(scope);
  current.current = scope;
  const busy = React.useMemo(() => ({ current: false }), [scope]);
  const alive = React.useRef(false);
  const timers = React.useRef([]);
  const valid = () => alive.current && current.current === scope;
  const update = patch => { if (valid()) setView(v => ({ ...v, ...patch })); };
  const settle = fn => { if (valid()) setView(v => fn(v)); };
  const value = result => {
    if (!result.ok) throw new Error(result.error);
    return result.value;
  };
  const clearTimers = () => { for (const id of timers.current) clearTimeout(id); timers.current = []; };
  const read = async () => {
    const config = value(await bridge.computerUseGet(profileId));
    const missing = window.OMP_COMPUTER_USE.missingRequired(config.permissions);
    settle(v => ({ ...v, config, awaiting: missing.length === 0 ? null : v.awaiting }));
    if (!valid()) return;
    const probe = config.supported && sessionId
      ? await bridge.computerUseRuntime(sessionId)
      : null;
    // A conversation whose process is not running has nothing to report; its
    // next process applies the profile setting and reports on its own.
    settle(v => ({ ...v, runtime: probe?.ok ? probe.value : null }));
  };
  const act = async work => {
    if (busy.current) return;
    busy.current = true;
    update({ busy: true, error: null });
    try { await work(); }
    catch (error) { update({ error: String(error.message ?? error) }); }
    finally { busy.current = false; update({ busy: false }); }
  };
  const recheck = () => act(async () => {
    update({ awaiting: null });
    await read();
  });
  /** TCC records an answer after the request returns; re-read briefly instead of guessing. */
  const afterRequest = () => {
    clearTimers();
    for (const delay of [1_500, 5_000]) {
      timers.current.push(setTimeout(() => { if (alive.current) void read().catch(() => {}); }, delay));
    }
  };

  React.useEffect(() => {
    alive.current = true;
    setView({ config: null, runtime: null, busy: true, error: null, awaiting: null });
    // A previous profile's in-flight request may finish, but cannot update
    // this scope or request further permissions for it.
    busy.current = false;
    void act(read);
    const focus = () => { if (document.visibilityState !== "hidden") void act(read); };
    window.addEventListener("focus", focus);
    document.addEventListener("visibilitychange", focus);
    let last;
    const unsub = bridge.onUpdate(snap => {
      if (!valid() || snap.activeSessionId !== sessionId || snap.computerUse === last) return;
      last = snap.computerUse;
      if (last) update({ runtime: last.state, error: last.error });
    });
    return () => {
      alive.current = false;
      clearTimers();
      window.removeEventListener("focus", focus);
      document.removeEventListener("visibilitychange", focus);
      unsub();
    };
  }, [bridge, scope]);

  const setEnabled = enabled => act(async () => {
    const saved = await bridge.computerUseSet(profileId, enabled);
    // A running conversation can refuse; the saved intent still decides what
    // the next process runs with, so show it before surfacing the refusal.
    const config = value(await bridge.computerUseGet(profileId));
    update({ config });
    if (enabled && valid()) {
      let permissions = config.permissions;
      for (const permission of window.OMP_COMPUTER_USE.missingRequired(permissions)) {
        update({ awaiting: permission });
        permissions = value(await bridge.computerUseRequestPermission(permission));
        update({ config: { ...config, permissions } });
      }
      afterRequest();
    }
    if (valid()) await read();
    value(saved);
  });
  const request = permission => act(async () => {
    update({ awaiting: permission });
    const permissions = value(await bridge.computerUseRequestPermission(permission));
    settle(v => ({ ...v, config: v.config ? { ...v.config, permissions } : v.config }));
    afterRequest();
  });
  /** For a rebuilt app: the old grant no longer matches this binary, so clear it and ask again. */
  const resetRequest = permission => act(async () => {
    update({ awaiting: permission });
    value(await bridge.computerUseResetPermission(permission));
    const permissions = value(await bridge.computerUseRequestPermission(permission));
    settle(v => ({ ...v, config: v.config ? { ...v.config, permissions } : v.config }));
    afterRequest();
    if (valid()) await read();
  });
  const openSettings = permission => act(async () => {
    value(await bridge.computerUseOpenSettings(permission));
  });
  const stop = () => act(async () => {
    value(await bridge.computerUseStop(sessionId));
    if (valid()) await read();
  });
  return { ...view, recheck, setEnabled, request, resetRequest, openSettings, stop };
}

function useComputerUseAccent(bridge, theme, accent) {
  const [error, setError] = React.useState(null);
  React.useEffect(() => {
    if (!window.__TAURI__ || !bridge?.computerUseSetAccent) return undefined;
    let current = true;
    const color = accent || window.OMP_SETTINGS.themeAccent(theme);
    bridge.computerUseSetAccent(color).then(result => {
      if (current) setError(result.ok ? null : result.error);
    }, error => { if (current) setError(String(error)); });
    return () => { current = false; };
  }, [bridge, theme, accent]);
  return error;
}

Object.assign(window, { useComputerUse, useComputerUseAccent });
