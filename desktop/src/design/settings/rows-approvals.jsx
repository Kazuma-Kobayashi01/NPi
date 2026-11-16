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
     execBackend    omp's `exec.backend` in the config of ctx.profileId: where the agent's
                    commands run (OMP_BRIDGE.execBackend / setExecBackend)
     vmPermissions  the Sandbox VM plugin's permissions (`sandbox-vm/permissions`, `sandbox-vm/set-permissions`)
     vmBase         the Sandbox VM plugin's system, Alpine or Ubuntu (`sandbox-vm/*` plugin commands of
                    the active tab through OMP_BRIDGE.extensionCommand)
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

const _SRAp_EXEC_OPTIONS = [
  { value: "", label: "This machine" },
  { value: "sandbox-vm/default", label: "Sandbox VM" },
];

function _SRAp_ExecBackend({ row, ctx, query }) {
  const { SettingsRow: Row, SettingsSeg: Seg } = window;
  const { bridge, profileId } = ctx;
  const [state, setState] = React.useState({ profileId, value: null, error: "", busy: false });
  React.useEffect(() => {
    let current = true;
    setState({ profileId, value: null, error: "", busy: false });
    bridge.execBackend(profileId).then(res => {
      if (current) setState({ profileId, value: res.ok ? res.value : null, error: res.ok ? "" : res.error, busy: false });
    });
    return () => { current = false; };
  }, [bridge, profileId]);
  const save = backend => {
    if (state.busy || backend === state.value) return;
    setState(s => ({ ...s, busy: true, error: "" }));
    bridge.setExecBackend(profileId, backend).then(res => {
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
          ? <Seg label={row.label} options={_SRAp_EXEC_OPTIONS} value={state.value} onChange={save} />
          : <span className="st-ro">{state.error ? "unavailable" : "reading…"}</span>}
      </Row>
      {state.error && <div className="st-row-error">{state.error}</div>}
    </>
  );
}

const _SRAp_VM_BASES = [
  { value: "alpine", label: "Alpine" },
  { value: "ubuntu", label: "Ubuntu" },
];

/** The Sandbox VM plugin's base (`sandbox-vm/status`, `sandbox-vm/set-base` on the active tab). */
function _SRAp_VmBase({ row, ctx, query }) {
  const { SettingsRow: Row, SettingsSeg: Seg } = window;
  const { bridge } = ctx;
  const [state, setState] = React.useState({ info: null, error: "", busy: false });
  const reply = (call) => call.then(
    info => setState({ info, error: "", busy: false }),
    err => setState(s => ({ ...s, busy: false, error: String(err?.message || err) })));
  React.useEffect(() => {
    let current = true;
    bridge.extensionCommand("sandbox-vm/status").then(
      info => { if (current) setState({ info, error: "", busy: false }); },
      err => { if (current) setState({ info: null, error: String(err?.message || err), busy: false }); });
    return () => { current = false; };
  }, [bridge]);
  const save = base => {
    if (state.busy || base === state.info?.base) return;
    setState(s => ({ ...s, busy: true, error: "" }));
    reply(bridge.extensionCommand("sandbox-vm/set-base", { base }));
  };
  const info = state.info;
  const missing = info && (info.bases || []).find(b => b.name === info.base && !b.built);
  return (
    <>
      <Row label={row.label} desc={row.desc} query={query}>
        {info
          ? <Seg label={row.label} options={_SRAp_VM_BASES} value={info.base} onChange={save} />
          : <span className="st-ro">{state.error ? "unavailable" : "reading…"}</span>}
      </Row>
      {missing && <div className="st-row-error">The {info.base} image is not built yet: run scripts/build-image.sh {info.base}</div>}
      {state.error && <div className="st-row-error">{state.error}</div>}
    </>
  );
}

const _SRAp_VM_NET = [
  { value: "off", label: "Off" },
  { value: "nat", label: "On" },
];
const _SRAp_VM_PROJECT = [
  { value: "read-write", label: "Read-write" },
  { value: "read-only", label: "Read-only" },
];

/** The Sandbox VM's permissions (`sandbox-vm/permissions`, `sandbox-vm/set-permissions` on the active tab): network,
 *  project access, extra read-only folders, cores, memory ceiling, idle stop. They are saved at once and apply
 *  when the VM restarts; the row offers that restart. */
function _SRAp_VmPermissions({ row, ctx, query }) {
  const { SettingsRow: Row, SettingsSeg: Seg, SettingsSlider: Slider, SettingsButton: Btn } = window;
  const { bridge } = ctx;
  const T = (key, vars) => window.OMP_I18N.t(key, vars);
  const [state, setState] = React.useState({ data: null, error: "", busy: false, pending: false, folder: "" });
  React.useEffect(() => {
    let current = true;
    bridge.extensionCommand("sandbox-vm/permissions").then(
      data => { if (current) setState(s => ({ ...s, data, error: "" })); },
      err => { if (current) setState(s => ({ ...s, data: null, error: String(err?.message || err) })); });
    return () => { current = false; };
  }, [bridge]);
  const apply = (changes, restart = false) => {
    if (state.busy) return;
    setState(s => ({ ...s, busy: true, error: "" }));
    bridge.extensionCommand("sandbox-vm/set-permissions", { changes, restart }).then(
      data => setState(s => ({ ...s, data, busy: false, folder: changes.readOnlyMounts ? "" : s.folder,
        pending: restart ? false : (data.restartNeeded || (s.pending && data.running)) })),
      err => setState(s => ({ ...s, busy: false, error: String(err?.message || err) })));
  };
  const data = state.data;
  if (!data) {
    return (
      <>
        <Row label={row.label} desc={row.desc} query={query}><span className="st-ro">{state.error ? "unavailable" : "reading…"}</span></Row>
        {state.error && <div className="st-row-error">{state.error}</div>}
      </>
    );
  }
  const p = data.permissions;
  const lim = data.limits;
  const maxGiB = Math.max(1, Math.floor(lim.hostMemoryMiB / 1024));
  const addFolder = () => { const folder = state.folder.trim(); if (folder) apply({ readOnlyMounts: [...p.readOnlyMounts, folder] }); };
  return (
    <>
      <Row label={row.label} desc={row.desc} query={query}>
        <Seg label={row.label} options={_SRAp_VM_NET} value={p.network} onChange={network => apply({ network })} />
      </Row>
      <Row label="Project folder" desc="Read-only: commands in the VM can read the project but not change it. The file tools on this machine are not affected." query={query}>
        <Seg label="Project folder" options={_SRAp_VM_PROJECT} value={p.projectAccess} onChange={projectAccess => apply({ projectAccess })} />
      </Row>
      <Row label="CPU cores" desc="Most cores the VM may use." query={query}>
        <Slider label="CPU cores" min={1} max={lim.maxCpus} value={Math.min(p.cpus, lim.maxCpus)} onChange={cpus => apply({ cpus })} />
      </Row>
      <Row label="Memory ceiling" desc="Most memory the VM may use. It takes memory from this machine only as commands use it, and gives it back when the VM stops." query={query}>
        <Slider label="Memory ceiling" min={1} max={maxGiB} value={Math.min(maxGiB, Math.max(1, Math.round(p.memoryMiB / 1024)))} unit=" GiB"
          onChange={gib => apply({ memoryMiB: gib * 1024 })} />
      </Row>
      <Row label="Stop when idle" desc="Stops the VM after this many minutes without a command; the next command starts it again. 0 keeps it running." query={query}>
        <Slider label="Stop when idle" min={0} max={120} step={5} value={p.idleStopMinutes} unit=" min" onChange={idleStopMinutes => apply({ idleStopMinutes })} />
      </Row>
      <Row label="Read-only folders" desc="Extra folders of this machine the VM can read (never write), at the same path." stack query={query}>
        <div className="st-vm-folders">
          {p.readOnlyMounts.map(folder => (
            <div className="st-vm-folder" key={folder}>
              <code>{folder}</code>
              <Btn onClick={() => apply({ readOnlyMounts: p.readOnlyMounts.filter(m => m !== folder) })} disabled={state.busy}>{T("Remove")}</Btn>
            </div>
          ))}
          <div className="st-vm-folder">
            <input className="st-input" value={state.folder} placeholder="/absolute/path" aria-label={T("Read-only folders")}
              onChange={e => setState(s => ({ ...s, folder: e.target.value }))}
              onKeyDown={e => { if (e.key === "Enter") addFolder(); }} />
            <Btn onClick={addFolder} disabled={state.busy || !state.folder.trim()}>{T("Add")}</Btn>
          </div>
        </div>
      </Row>
      {(state.pending || data.running) && (
        <Row label="Restart the VM" desc={state.pending ? "The changes above apply when the VM restarts. Commands running in it are stopped." : "Restarts the VM; the next command boots it again."} query={query}>
          <Btn accent={state.pending} onClick={() => apply({}, true)} disabled={state.busy}>{T("Restart VM")}</Btn>
        </Row>
      )}
      {state.error && <div className="st-row-error">{state.error}</div>}
    </>
  );
}

Object.assign(window.OMP_SETTINGS_ROWS = window.OMP_SETTINGS_ROWS || {}, {
  approvalMode: _SRAp_Mode,
  approvalRules: _SRAp_Rules,
  sandboxMode: _SRAp_Sandbox,
  execBackend: _SRAp_ExecBackend,
  vmBase: _SRAp_VmBase,
  vmPermissions: _SRAp_VmPermissions,
});
