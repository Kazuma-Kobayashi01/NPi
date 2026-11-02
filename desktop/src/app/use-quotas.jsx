/* app/use-quotas.jsx — subscription limits scoped to each profile.
   Read the cache on entry and invalidate after a session settles or manual refresh. */

function useQuotas({ bridge, profileId, enabled }) {
  const [, render] = React.useState(0);
  const entries = React.useRef(new Map());
  const mounted = React.useRef(false);
  const selected = React.useRef(profileId);
  selected.current = profileId;

  const request = React.useCallback((profile, force) => {
    if (!bridge?.usageLimits) return;
    let entry = entries.current.get(profile);
    if (!entry) {
      entry = { rows: null, loading: false, error: "", running: false, queuedForce: false, generation: 0 };
      entries.current.set(profile, entry);
    }
    const update = () => {
      if (mounted.current && selected.current === profile) render(n => n + 1);
    };
    if (entry.running) {
      if (force) {
        entry.queuedForce = true;
        entry.generation++;
      }
      return;
    }
    entry.running = true;
    entry.loading = true;
    const generation = ++entry.generation;
    update();
    Promise.resolve().then(() => bridge.usageLimits(profile, force)).then(r => {
      if (generation !== entry.generation) return;
      if (r?.ok && Array.isArray(r.value)) {
        entry.rows = r.value;
        entry.error = "";
      } else {
        entry.error = String(r?.error ?? "unavailable");
      }
    }, error => {
      if (generation === entry.generation) entry.error = String(error?.message ?? error);
    }).finally(() => {
      entry.running = false;
      if (entry.queuedForce) {
        entry.queuedForce = false;
        request(profile, true);
      } else {
        entry.loading = false;
        update();
      }
    });
  }, [bridge]);

  React.useEffect(() => {
    mounted.current = true;
    const unsubscribe = bridge?.onSessionSettled?.(profile => request(profile, true));
    return () => { mounted.current = false; unsubscribe?.(); };
  }, [bridge, request]);

  React.useEffect(() => {
    if (enabled) request(profileId, false);
  }, [enabled, profileId, request]);

  const refresh = React.useCallback(() => request(profileId, true), [profileId, request]);
  const entry = entries.current.get(profileId);
  return { rows: entry?.rows ?? null, loading: entry?.loading ?? false, error: entry?.error ?? "", refresh };
}

Object.assign(window, { useQuotas });
