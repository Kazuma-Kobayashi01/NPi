/* app/use-quotas.jsx — what the signed-in subscriptions still allow, for the
   side panel's Usage card. useQuotas({ bridge, profileId, enabled }) →
   { rows, loading, error, refresh }. `rows` is OMP_BRIDGE.usageLimits' value
   (null until the first answer; the last good rows stay through a failed
   refresh). Asked while `enabled` (the panel is shown) and every five
   minutes after: omp asks the providers over the network, a few seconds. */

function useQuotas({ bridge, profileId, enabled }) {
  const [state, setState] = React.useState({ rows: null, loading: false, error: "" });
  const alive = React.useRef(true);
  React.useEffect(() => () => { alive.current = false; }, []);

  const refresh = React.useCallback(() => {
    if (!bridge?.usageLimits) return;
    setState(s => ({ ...s, loading: true }));
    Promise.resolve(bridge.usageLimits(profileId)).then(r => {
      if (!alive.current) return;
      setState(s => ({
        rows: r?.ok && Array.isArray(r.value) ? r.value : s.rows,
        loading: false,
        error: r?.ok ? "" : String(r?.error ?? "unavailable"),
      }));
    });
  }, [bridge, profileId]);

  React.useEffect(() => {
    if (!enabled) return undefined;
    refresh();
    const id = setInterval(refresh, 5 * 60 * 1000);
    return () => clearInterval(id);
  }, [enabled, refresh]);

  return { ...state, refresh };
}

Object.assign(window, { useQuotas });
