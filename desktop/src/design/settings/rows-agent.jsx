/* ═════════════════════════════════════════════════════════════════════
   design/settings/rows-agent.jsx — custom Settings rows of Agent
   (window.OMP_SETTINGS_ROWS):
     startupProfile  the profile of the tab opened at launch and of new tabs
                     with none to inherit from (Rust's `profiles.json`,
                     ctx.onSetStartupProfile → OMP_BRIDGE.setStartupProfile);
                     the reason a pick was refused shows under the menu
     goalContinue    omp's goal auto-continue (`goal.continuationModes`
                     holds "rpc") in omp's config of ctx.goal.profileId,
                     read and written through app/use-goal-mode.jsx's
                     useGoalContinuation (goal_continuation_get/set)
   ═════════════════════════════════════════════════════════════════════ */

function _SRA_profileName(ctx, id) {
  return ctx.profiles.find(p => p.id === id)?.name ?? id;
}

function _SRA_StartupProfile({ row, ctx, query }) {
  const { SettingsRow: Row, PathPicker, DEFAULT_PROFILE_ID } = window;
  const [error, setError] = React.useState(null);
  const current = ctx.startupProfileId;
  const rows = ctx.profiles.map(p => ({
    name: p.name,
    hint: p.id === DEFAULT_PROFILE_ID ? "built-in" : "",
    path: "",
    value: p.id,
    checked: p.id === current,
  }));
  const save = async (id) => {
    setError(null);
    const res = await ctx.onSetStartupProfile(id);
    if (!res?.ok) setError(res?.error ?? "the profile could not be saved");
  };
  return (
    <Row label={row.label} desc={row.desc} query={query}
      resettable={current !== DEFAULT_PROFILE_ID} onReset={() => save(DEFAULT_PROFILE_ID)}>
      <PathPicker name="startup profile" label={_SRA_profileName(ctx, current)} title={current}
        rows={rows} disabled={rows.length === 0} error={error} save={save} />
    </Row>
  );
}

function _SRA_GoalContinue({ row, ctx, query }) {
  const { SettingsRow: Row, SettingsToggle: Toggle } = window;
  const profileId = ctx.goal?.profileId ?? null;
  const cont = window.useGoalContinuation(ctx.bridge, profileId, true);
  const name = profileId ? _SRA_profileName(ctx, profileId) : null;
  const scope = name ? ` Applies to profile “${name}” (omp's config); its running tabs pick it up without a restart.` : "";
  let control;
  if (cont.value !== null) {
    control = <Toggle value={cont.value} label={row.label} onChange={v => { if (!cont.busy) cont.set(v); }} />;
  } else {
    control = <span className="st-ro">{cont.error ? "unavailable" : "reading…"}</span>;
  }
  return (
    <>
      <Row label={row.label} query={query} desc={`${row.desc}${scope}`}>
        {control}
      </Row>
      {cont.error && <div className="st-row-error">{cont.error}</div>}
    </>
  );
}

function _SRA_SubagentModel({ row, ctx, query, modified, onReset }) {
  const { SettingsRow: Row, SettingsSeg: Seg, OMP_MODELS: M } = window;
  const { t, setTweak, bridge, profileId } = ctx;
  const mode = t.subagentModelMode;
  const customModel = t.subagentCustomModel;
  const thinking = t.subagentCustomThinking;
  const [catalogState, setCatalogState] = React.useState({ profileId: null, models: [], loading: true, error: "" });
  React.useEffect(() => {
    let current = true;
    setCatalogState({ profileId, models: [], loading: true, error: "" });
    bridge.modelsCatalog(profileId).then(result => {
      if (current) setCatalogState({ profileId, models: result.ok ? result.value : [],
        loading: false, error: result.ok ? "" : result.error });
    }, err => {
      if (current) setCatalogState({ profileId, models: [], loading: false, error: String(err) });
    });
    return () => { current = false; };
  }, [bridge, profileId]);
  const catalog = catalogState.profileId === profileId ? catalogState.models : [];
  const loading = catalogState.profileId !== profileId || catalogState.loading;
  const groups = React.useMemo(() => M.groupByVendor(catalog), [catalog, M]);
  const model = catalog.find(m => M.keyOf(m) === customModel);
  const levels = model?.thinking.length ? model.thinking.filter(l => l !== "auto") : ["off"];
  const invalidThinking = model && !levels.includes(thinking);
  const error = catalogState.error || (!loading && (
    !catalog.length ? "No models available for this profile." :
    !customModel ? "Select a Custom model before delegating." :
    !model ? `Custom model unavailable: ${customModel}. Select a model from this profile.` :
    invalidThinking ? `Thinking “${thinking}” is unavailable for this model. Select a supported level.` : ""));
  const selectModel = key => {
    const selected = catalog.find(m => M.keyOf(m) === key);
    const supported = selected.thinking.length ? selected.thinking.filter(l => l !== "auto") : ["off"];
    setTweak("subagentCustomModel", key);
    if (!supported.includes(thinking)) {
      setTweak("subagentCustomThinking", supported.includes("medium") ? "medium" : supported[0]);
    }
  };
  const descriptions = {
    auto: "Ask before spawning to choose the subagent model and thinking level.",
    sync: "Follow the conversation’s current model and thinking level.",
    custom: "Use the model and explicit thinking level selected below.",
  };
  return (
    <Row stack label={row.label} desc={row.desc} query={query} modified={modified} onReset={onReset}>
      <div className="st-subagent-settings">
        <Seg label="Subagent model mode" options={[
          { value: "auto", label: "Auto" }, { value: "sync", label: "Sync" }, { value: "custom", label: "Custom" },
        ]} value={mode} onChange={v => setTweak("subagentModelMode", v)} />
        <div className="st-rd">{descriptions[mode]}</div>
        {mode === "custom" && <>
          <div className="st-subagent-pickers">
            <label className="st-modelsel">Model
              <select value={customModel} aria-label="Subagent Custom model"
                disabled={loading || !catalog.length} onChange={e => selectModel(e.target.value)}>
                <option value="" disabled>{loading ? "Loading models…" : catalog.length ? "Select a model…" : "No models available"}</option>
                {customModel && !model && <option value={customModel} disabled>{customModel} (unavailable)</option>}
                {groups.map(g => <optgroup key={g.vendor} label={g.vendor}>
                  {g.models.map(m => <option key={M.keyOf(m)} value={M.keyOf(m)}>{m.name || M.keyOf(m)}</option>)}
                </optgroup>)}
              </select>
            </label>
            <label className="st-modelsel">Thinking
              <select value={model ? thinking : ""} aria-label="Subagent Custom thinking"
                disabled={loading || !model} onChange={e => setTweak("subagentCustomThinking", e.target.value)}>
                {!model && <option value="">Select a model first</option>}
                {invalidThinking && <option value={thinking} disabled>{thinking} (unavailable)</option>}
                {model && levels.map(level => <option key={level} value={level}>{level}</option>)}
              </select>
            </label>
          </div>
          {error && <div className="st-subagent-error" role="alert">{error}</div>}
        </>}
      </div>
    </Row>
  );
}

function _SRA_SubagentDelegation({ row, ctx, query, modified, onReset }) {
  const { SettingsRow: Row, SettingsSeg: Seg } = window;
  return (
    <Row stack label={row.label} desc={row.desc} query={query} modified={modified} onReset={onReset}>
      <div className="st-subagent-settings st-subagent-appetite">
        <Seg label={row.label} value={ctx.t.subagentDelegation}
          onChange={value => ctx.setTweak("subagentDelegation", value)}
          options={[
            { value: "unlimited", label: <span className="st-subagent-unlimited">Unlimited ≤32</span> },
            { value: "aggressive", label: "Aggressive ≤8" },
            { value: "moderate", label: "Moderate ≤4" },
            { value: "conservative", label: "Conservative ≤2" },
            { value: "off", label: "Off 0" },
          ]} />
      </div>
    </Row>
  );
}

function _SRA_SidechatModel({ row, ctx, query, modified, onReset }) {
  const { SettingsRow: Row } = window;
  const M = window.OMP_MODELS;
  const [catalog, setCatalog] = React.useState([]);
  const [error, setError] = React.useState("");
  React.useEffect(() => {
    let current = true;
    ctx.bridge.modelsCatalog(ctx.profileId).then(result => {
      if (!current) return;
      if (result.ok) setCatalog(result.value);
      else setError(result.error);
    }, err => { if (current) setError(String(err)); });
    return () => { current = false; };
  }, [ctx.bridge, ctx.profileId]);
  const groups = React.useMemo(() => M.groupByVendor(catalog), [catalog]);
  const selected = ctx.t.sidechatModel;
  return (
    <Row stack label={row.label} desc={row.desc} query={query} modified={modified} onReset={onReset}>
      <div className="st-modelsel">
        <select aria-label="SideChat model" value={selected}
          onChange={e => ctx.setTweak("sidechatModel", e.target.value)}>
          <option value="">Worker default model</option>
          {selected && !catalog.some(m => M.keyOf(m) === selected) &&
            <option value={selected}>{selected}</option>}
          {groups.map(g => <optgroup key={g.vendor} label={g.vendor}>
            {g.models.map(m => <option key={M.keyOf(m)} value={M.keyOf(m)}>{m.name || M.keyOf(m)}</option>)}
          </optgroup>)}
        </select>
        {error && <div role="alert">{error}</div>}
      </div>
    </Row>
  );
}

Object.assign(window.OMP_SETTINGS_ROWS = window.OMP_SETTINGS_ROWS || {}, {
  startupProfile: _SRA_StartupProfile,
  goalContinue: _SRA_GoalContinue,
  subagentModel: _SRA_SubagentModel,
  subagentDelegation: _SRA_SubagentDelegation,
  sidechatModel: _SRA_SidechatModel,
});
