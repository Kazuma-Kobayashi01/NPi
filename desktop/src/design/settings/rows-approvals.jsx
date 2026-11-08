/* ═════════════════════════════════════════════════════════════════════
   design/settings/rows-approvals.jsx — custom Settings rows of Approvals
   (window.OMP_SETTINGS_ROWS):
     approvalMode   the tool approval mode editor (default + per-project
                    overrides, approval-mode-section.jsx) — the same
                    component the approvals panel shows
     approvalRules  how many standing rules the active tab's session and
                    project have (OMP_BRIDGE.listApprovalRules) and
                    "Manage rules", which opens the approvals panel
     sandboxMode    omp's `sandbox.mode` in the config of ctx.profileId
                    (OMP_BRIDGE.sandboxMode / setSandboxMode)
   ═════════════════════════════════════════════════════════════════════ */

/** The rendered tab's entry, null with no tab (EMPTY_PROJECT has no id). */
function _SRAp_project(ctx) {
  return ctx.activeProject?.id ? ctx.activeProject : null;
}

function _SRAp_Mode({ row, ctx, query }) {
  const { SettingsRow: Row, ApprovalModeSection } = window;
  return (
    <Row stack label={row.label} desc={row.desc} query={query}>
      <ApprovalModeSection view={ctx.approvalModes} activeProject={_SRAp_project(ctx)}
        sessions={ctx.sessions} query={query} />
    </Row>
  );
}

function _SRAp_Rules({ row, ctx, query }) {
  const { SettingsRow: Row, SettingsButton: Button, Icon } = window;
  const project = _SRAp_project(ctx);
  const [count, setCount] = React.useState(null);
  React.useEffect(() => {
    if (!ctx.bridge || !project) { setCount(null); return undefined; }
    let live = true;
    ctx.bridge.listApprovalRules().then(rules => { if (live) setCount(Array.isArray(rules) ? rules.length : null); });
    return () => { live = false; };
  }, [ctx.bridge, project?.id]);
  let lead = "Open a tab to see its session and project rules.";
  if (project && count !== null) {
    lead = `${count === 0 ? "No" : count} standing rule${count === 1 ? "" : "s"} for this tab's session and project.`;
  }
  return (
    <Row label={row.label} query={query} className="st-linkrow" desc={`${lead} ${row.desc}`}>
      <Button accent onClick={ctx.openApprovals}>
        Manage rules <Icon name="external" size={11} />
      </Button>
    </Row>
  );
}

const _SRAp_SANDBOX_OPTIONS = [
  { value: "off", label: "Off" },
  { value: "workspace-write", label: "Workspace write" },
  { value: "read-only", label: "Read only" },
];

function _SRAp_Sandbox({ row, ctx, query }) {
  const { SettingsRow: Row, SettingsSeg: Seg } = window;
  const { bridge, profileId } = ctx;
  const [state, setState] = React.useState({ profileId, value: null, error: "", busy: false });
  React.useEffect(() => {
    let current = true;
    setState({ profileId, value: null, error: "", busy: false });
    bridge.sandboxMode(profileId).then(res => {
      if (current) setState({ profileId, value: res.ok ? res.value : null, error: res.ok ? "" : res.error, busy: false });
    });
    return () => { current = false; };
  }, [bridge, profileId]);
  const save = mode => {
    if (state.busy || mode === state.value) return;
    setState(s => ({ ...s, busy: true, error: "" }));
    bridge.setSandboxMode(profileId, mode).then(res => {
      // A reply for a profile the row no longer shows is dropped.
      setState(s => s.profileId !== profileId ? s
        : { ...s, busy: false, value: res.ok ? res.value : s.value, error: res.ok ? "" : res.error });
    });
  };
  const ready = state.profileId === profileId && state.value !== null;
  return (
    <>
      <Row label={row.label} desc={row.desc} query={query}>
        {ready
          ? <Seg label={row.label} options={_SRAp_SANDBOX_OPTIONS} value={state.value} onChange={save} />
          : <span className="st-ro">{state.error ? "unavailable" : "reading…"}</span>}
      </Row>
      {state.error && <div className="st-row-error">{state.error}</div>}
    </>
  );
}

Object.assign(window.OMP_SETTINGS_ROWS = window.OMP_SETTINGS_ROWS || {}, {
  approvalMode: _SRAp_Mode,
  approvalRules: _SRAp_Rules,
  sandboxMode: _SRAp_Sandbox,
});
