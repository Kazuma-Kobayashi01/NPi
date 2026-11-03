/* design/settings/rows-notifications.jsx — Settings → Session's custom row
   `notifications` (window.OMP_SETTINGS_ROWS): the master switch, one switch per
   kind of banner, the system's permission state and a test button. The system
   permission is asked when the master switch goes on (Rust: notify/macos.rs);
   what is sent and when is Rust's (docs/architecture/notifications.md). */

function _SRN_Notifications({ row, ctx, query }) {
  const { SettingsRow: Row, SettingsToggle: Toggle, SettingsButton: Button } = window;
  const N = window.OMP_NOTIFY;
  const t = ctx.t;
  const T = (key) => window.OMP_I18N.t(key);
  const on = t.notifications === true;
  const [status, setStatus] = React.useState(null);
  const [busy, setBusy] = React.useState(false);
  const [sent, setSent] = React.useState(false);

  const refresh = React.useCallback(() => {
    Promise.resolve(ctx.bridge?.notifyStatus?.()).then(s => setStatus(s ?? "unsupported"));
  }, [ctx.bridge]);
  React.useEffect(() => {
    refresh();
    // The user may flip it in System Settings and come back.
    window.addEventListener("focus", refresh);
    return () => window.removeEventListener("focus", refresh);
  }, [refresh]);

  const turnOn = async (next) => {
    ctx.setTweak({ notifications: next });
    if (!next) return;
    setBusy(true);
    try { setStatus((await ctx.bridge?.notifyRequest?.()) ?? "unsupported"); }
    finally { setBusy(false); }
  };
  const test = async () => {
    setBusy(true);
    try {
      const s = await ctx.bridge?.notifyTest?.();
      setStatus(s ?? "unsupported");
      setSent(s === "granted");
      window.setTimeout(() => setSent(false), 3000);
    } finally { setBusy(false); }
  };

  const raw = N.describe(status);
  const info = { ...raw, text: T(raw.text) };
  const kinds = [
    ["notifyDone", "A long run finished"],
    ["notifyAsk", "A question or approval is waiting"],
    ["notifyFailed", "A request failed or the session ended"],
  ];
  return (
    <Row stack label={row.label} desc={row.desc} query={query}>
      <div className="st-notify">
        <label className="st-notify-line">
          <span>{T("Send system notifications")}</span>
          <Toggle value={on} onChange={turnOn} label={T("Send system notifications")} />
        </label>
        <p className={`st-notify-state is-${info.tone}`} role="status">{info.text}</p>
        {on && kinds.map(([key, label]) => (
          <label key={key} className="st-notify-line">
            <span>{T(label)}</span>
            <Toggle value={t[key] !== false} onChange={v => ctx.setTweak({ [key]: v })} label={T(label)} />
          </label>
        ))}
        <div className="st-btns">
          {info.canAsk && <Button accent disabled={busy} onClick={() => turnOn(true)}>{T("Allow notifications")}</Button>}
          <Button disabled={busy || !info.canTest} onClick={test}>{T(sent ? "Sent" : "Send test notification")}</Button>
        </div>
      </div>
    </Row>
  );
}

Object.assign(window.OMP_SETTINGS_ROWS = window.OMP_SETTINGS_ROWS || {}, { notifications: _SRN_Notifications });
