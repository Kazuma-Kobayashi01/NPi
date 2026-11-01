function _SCU_Permission({ name, permission, granted, description, computer }) {
  const { SettingsButton: Button } = window;
  const enabled = computer.config.enabled;
  return (
    <div className="st-cu-permission">
      <div className="st-cu-permission-copy">
        <div className="st-cu-permission-title">{name}<span className={granted ? "st-cu-granted" : ""}>{granted ? "Granted" : "Not granted"}</span></div>
        <p>{description}</p>
      </div>
      <div className="st-btns">
        {!granted && <Button disabled={!enabled || computer.busy} onClick={() => computer.request(permission)}>Request access</Button>}
        {!granted && <Button disabled={!enabled || computer.busy} onClick={() => computer.resetRequest(permission)}>Reset &amp; request</Button>}
        <Button disabled={computer.busy || !computer.config.supported} onClick={() => computer.openSettings(permission)}>Open settings</Button>
      </div>
    </div>
  );
}

function _SCU_Settings({ row, ctx, query }) {
  const { SettingsRow: Row, SettingsButton: Button } = window;
  const sessionId = ctx.computerUseSessionId;
  const [selectedProfile, setSelectedProfile] = React.useState(null);
  const profileId = sessionId ? ctx.goal.profileId : selectedProfile ?? ctx.startupProfileId ?? window.DEFAULT_PROFILE_ID;
  const computer = window.useComputerUse(ctx.bridge, profileId, sessionId);
  const profile = ctx.profiles.find(p => p.id === profileId);
  const name = profile?.name ?? profileId;
  const config = computer.config;
  const state = window.OMP_COMPUTER_USE.status(config, computer.runtime, computer.error, sessionId, computer.awaiting);
  const escapeUnavailable = computer.runtime?.capabilities?.globalEscape === false;
  return (
    <Row stack label={row.label} desc={row.desc} query={query} className="st-cu">
      <div className="st-cu-scope">
        <span>Profile</span>
        {sessionId ? <strong>{name}</strong> : <select aria-label="Computer Use profile" value={profileId} disabled={computer.busy}
          onChange={e => setSelectedProfile(e.target.value)}>
          {ctx.profiles.map(p => <option key={p.id} value={p.id}>{p.name}{p.id === ctx.startupProfileId ? " · startup default" : ""}</option>)}
        </select>}
        <small>{sessionId ? "Applies to every open conversation using this profile." : "No conversation is open. Choose the profile to configure; the startup default is selected initially."}</small>
      </div>
      <div className="st-cu-master">
        <div><strong>Allow Computer Use</strong><p>Let this profile see the screen and operate apps. New operations are blocked immediately when you turn this off.</p></div>
        <button type="button" className="st-tgl" role="switch" aria-label="Allow Computer Use"
          aria-checked={config?.enabled === true} disabled={!config?.supported || computer.busy}
          onClick={() => computer.setEnabled(!config.enabled)} />
      </div>
      <div className={`st-cu-status ${state.kind}`} role={computer.error ? "alert" : "status"} aria-live="polite">
        <div><strong>{state.label}</strong><p>{state.detail}</p></div>
        <Button disabled={computer.busy} onClick={computer.recheck}>{computer.busy ? "Checking…" : "Recheck"}</Button>
      </div>
      {config?.supported && <>
        <div className="st-cu-permissions">
          <_SCU_Permission name="Screen Recording" permission="screenRecording" granted={config.permissions.screenRecording}
            description="Allows the agent to capture the screen and understand what is visible." computer={computer} />
          <_SCU_Permission name="Accessibility" permission="accessibility" granted={config.permissions.accessibility}
            description="Allows the agent to inspect controls and send clicks, typing and shortcuts." computer={computer} />
          {config.enabled && escapeUnavailable && <_SCU_Permission name="Input Monitoring · optional" permission="inputMonitoring" granted={config.permissions.inputMonitoring}
            description="The agent process cannot currently listen for physical Escape. Input Monitoring can enable Escape to stop takeover input; it is not needed for ordinary Computer Use. The Stop button remains available." computer={computer} />}
        </div>
        <p className="st-cu-note">In macOS Privacy &amp; Security, grant access to <strong>{config.responsibleApp}</strong>. Returning here rechecks access without asking again. macOS may require you to quit and reopen the app after a permission change; NPi will not restart itself.</p>
        <p className="st-cu-note">A rebuild counts as a different app to macOS: the switch in System Settings can stay on while this build is still denied. Use <strong>Reset &amp; request</strong> to clear the old record and grant again.</p>
        <div className="st-cu-operation">
          <div><strong>Current computer operation</strong><p>{computer.runtime?.running ? "Stop releases computer control without ending the agent conversation or disabling this profile." : "No computer operation is running in this conversation."}</p></div>
          <Button disabled={!sessionId || !computer.runtime?.running || computer.busy} onClick={computer.stop}>Stop current operation</Button>
        </div>
        <p className="st-cu-note">While operating, a theme-coloured border follows the target window, with a stop prompt and an agent cursor at the actual operation point. Desktop-level actions mark the display. Turning Computer Use off stops computer work but does not revoke macOS permissions; manage those in System Settings.</p>
      </>}
      {ctx.computerUseAccentError && <p className="st-cu-error" role="alert">Operation indicator: {ctx.computerUseAccentError}</p>}
    </Row>
  );
}

window.OMP_SETTINGS_ROWS = Object.assign(window.OMP_SETTINGS_ROWS ?? {}, { computerUse: _SCU_Settings });
