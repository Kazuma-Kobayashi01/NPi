/* office-window.jsx — the Office window (index.html?panel=office&project=<session id>).
   One tab's company: the meter and the pull-request funnel on top, a card per
   office below; a card opens that office's member conversations
   (office-detail.jsx). Data comes over the window's own line to the tab's omp
   (app/office-live.js), shaped by app/office-model.js and app/office-meter.js.
   Members are read-only here: the owner's actions are merging or sending back
   an approved office pull request, the roster, and clearing an office's memory,
   all run as `/office` commands. */

const _OF_T = (key, vars) => window.OMP_I18N.t(key, vars);
const { Icon: _OF_Icon, OMP_OFFICE_MODEL: _OF_M, OMP_OFFICE_METER: _OF_P } = window;
const _OF_SCALES = [["team", "Team", 1], ["studio", "Studio", 3], ["enterprise", "Enterprise", 6]];
const _OF_CAPS = ["common", "professional", "expert"];
const _OF_EFFORTS = ["off", "minimal", "low", "medium", "high", "xhigh", "max"];
const _OF_RANGE_KEY = "npi.office.range";
const _OF_FLOW = ["Assigned", "Worker PRs in review", "Office PRs in review", "Waiting for you", "Merged"];
const _OF_PENDING_MS = 10000;

/** The company of tab `sessionId`: `{ state, online, reason, link }`; `link` runs commands. */
function useOffice(sessionId) {
  const [state, setState] = React.useState(null);
  const [online, setOnline] = React.useState({ on: true, reason: "" });
  const link = React.useMemo(
    () => window.OMP_OFFICE_LIVE.connect(sessionId, { onState: setState, onOnline: (on, reason) => setOnline({ on, reason }) }),
    [sessionId],
  );
  React.useEffect(() => () => link.close(), [link]);
  // "Today" starts over at local midnight; with nobody working no frame says so.
  React.useEffect(() => {
    const timer = setTimeout(() => link.refresh(), _OF_P.msToMidnight(new Date()) + 1500);
    return () => clearTimeout(timer);
  }, [link, state?.usage?.day]);
  // Another process (a shell's `npi office merge`) is not pushed: catch up when the window is looked at.
  React.useEffect(() => {
    const onFocus = () => link.refresh();
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [link]);
  return { state, online, link };
}

/** USD→JPY: `{ rate, date, stale }`, `rate` null until (or unless) one is known. */
function useFxRate() {
  const [fx, setFx] = React.useState({ rate: null, date: null, stale: false });
  React.useEffect(() => {
    let live = true;
    const load = () => window.__TAURI__?.core.invoke("fx_rate_usd_jpy").then(r => { if (live && r) setFx(r); }, () => {});
    load();
    const timer = setInterval(load, 6 * 60 * 60 * 1000);
    return () => { live = false; clearInterval(timer); };
  }, []);
  return fx;
}

/** The figures on the dial for `reading`, rolling toward it frame by frame. */
function useOfMeter(reading) {
  const meter = React.useRef(_OF_P.rest(reading));
  const [, tick] = React.useReducer(n => n + 1, 0);
  const now = performance.now();
  meter.current = _OF_P.aim(meter.current, reading, now);
  const moving = _OF_P.rolling(meter.current, now);
  React.useEffect(() => {
    if (!moving) return undefined;
    const frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
  });
  return _OF_P.shown(meter.current, now);
}

function OfMeter({ view, range, onRange, fx, compact }) {
  const reading = compact ?? view.usage[range];
  const shown = useOfMeter(reading);
  const price = _OF_P.unitPrice(reading);
  const yen = v => _OF_P.fmtJpy(_OF_P.toJpy(v, fx.rate));
  const shares = compact ? [] : _OF_M.shares(view, range);
  return (
    <div className={`of-meter${compact ? " compact" : ""}${view.working ? " pumping" : ""}`}>
      <div className="of-dials">
        <div className="of-dial">
          <div className="of-dial-k"><span className="of-lamp" />{_OF_T("Cost")}</div>
          <div className="of-dial-v mono">
            <span>{_OF_P.fmtUsd(shown.cost)}</span>
            <span className="of-dial-yen">{yen(shown.cost)}</span>
          </div>
        </div>
        <div className="of-dial">
          <div className="of-dial-k">{_OF_T("Tokens")}</div>
          <div className="of-dial-v mono"><span>{_OF_P.fmtTokens(shown.tokens)}</span></div>
        </div>
        {!compact && (
          <div className="of-dial small">
            <div className="of-dial-k">{_OF_T("Unit price")}</div>
            <div className="of-dial-v mono">
              <span>{price == null ? "—" : _OF_P.fmtUsd(price)}</span>
              <span className="of-dial-yen">{price == null ? "" : yen(price)}</span>
            </div>
            <div className="of-dial-note">{_OF_T("per million tokens")}</div>
          </div>
        )}
        {!compact && (
          <div className="of-meter-side">
            <div className="of-seg" role="tablist">
              {[["today", "Today"], ["total", "All time"]].map(([id, label]) => (
                <button key={id} role="tab" aria-selected={range === id} className={range === id ? "on" : ""} onClick={() => onRange(id)}>{_OF_T(label)}</button>
              ))}
            </div>
            <div className={`of-rate${fx.stale && fx.rate ? " stale" : ""}`} title={fx.stale && fx.rate ? _OF_T("This rate is more than a day old and could not be refreshed.") : undefined}>
              {fx.rate ? _OF_T("1 USD = {rate} JPY ({date})", { rate: fx.rate.toFixed(2), date: fx.date ?? "" }) : _OF_T("Exchange rate unavailable")}
            </div>
          </div>
        )}
      </div>
      {!compact && (
        <>
          <div className="of-shares" aria-hidden={shares.length === 0}>
            {shares.map(s => (
              <div key={s.key} className={`of-share ${s.office == null ? "manager" : `o${s.office}`}`} style={{ flexGrow: Math.max(s.share, 0.04) }}
                title={`${s.office == null ? _OF_T("General manager") : _OF_T("Office {n}", { n: s.office })} · ${_OF_P.fmtTokens(s.tokens)} · ${_OF_P.fmtUsd(s.cost)}`}>
                <span>{s.office == null ? _OF_T("General manager") : _OF_T("Office {n}", { n: s.office })}</span>
              </div>
            ))}
          </div>
          <div className="of-meter-note">{_OF_T("Estimated at catalog prices; a subscription login is billed differently.")}</div>
        </>
      )}
    </div>
  );
}

function OfFunnel({ funnel }) {
  const steps = [["Assigned", funnel.assigned], ["Worker PRs in review", funnel.workerReview], ["Office PRs in review", funnel.officeReview],
    ["Waiting for you", funnel.awaitingOwner, "hot"], ["Merged", funnel.merged]];
  return (
    <div className="of-funnel">
      {steps.map(([label, n, hot], i) => (
        <React.Fragment key={label}>
          {i > 0 && <span className="of-funnel-arrow">→</span>}
          <span className={`of-funnel-step${hot && n > 0 ? " hot" : ""}${n === 0 ? " zero" : ""}`}>{_OF_T(label)} <b className="mono">{n}</b></span>
        </React.Fragment>
      ))}
    </div>
  );
}

function OfMemberRow({ member, range, fx }) {
  const u = member.usage[range];
  return (
    <div className={`of-member ${member.status}`}>
      <span className={`of-dot ${member.status}`} title={member.status === "failed" ? _OF_T("failed") : undefined} />
      <span className="of-member-name mono">{member.role === "leader" ? "★ " : ""}{member.id}</span>
      <span className={`of-cap ${member.capability}`}>{_OF_T(`cap.${member.capability}`)}</span>
      <span className="of-member-model" title={member.model || undefined}>{member.model ? window.saModelName(member.model) : _OF_T("default model")}{member.thinking ? ` · ${member.thinking}` : ""}</span>
      <span className="of-member-use mono">{_OF_P.fmtTokensShort(u.tokens)} · {_OF_P.fmtUsd(u.cost)} · {_OF_P.fmtJpy(_OF_P.toJpy(u.cost, fx.rate))}</span>
    </div>
  );
}

function OfCard({ office, range, fx, branch, onOpen, onMerge, onReject, onMemory }) {
  const u = office.usage[range];
  const act = fn => e => { e.stopPropagation(); fn(); };
  return (
    <div className={`of-card ${office.phase}`} role="button" tabIndex={0} onClick={onOpen}
      onKeyDown={e => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); onOpen(); } }}>
      <div className="of-card-head">
        <span className="of-card-title">{_OF_T("Office {n}", { n: office.number })}</span>
        <span className={`of-phase ${office.phase}`}>{_OF_T(`phase.${office.phase}`)}</span>
      </div>
      <div className="of-members">
        {office.members.map(m => <OfMemberRow key={m.id} member={m} range={range} fx={fx} />)}
      </div>
      <div className="of-card-total mono">{_OF_P.fmtTokensShort(u.tokens)} Token · {_OF_P.fmtUsd(u.cost)} · {_OF_P.fmtJpy(_OF_P.toJpy(u.cost, fx.rate))}</div>
      {office.task && <div className="of-card-task" title={office.task}>{_OF_T("Now: {task}", { task: office.task })}</div>}
      {office.waiting && (
        <div className="of-card-waiting">
          <span className="of-card-pr">#{office.waiting.number} {office.waiting.title}</span>
          <button className="btn primary" onClick={act(() => onMerge(office.waiting))}>{_OF_T("Merge into {branch}", { branch: office.waiting.target || branch })}</button>
          <button className="btn outlined" onClick={act(() => onReject(office.waiting))}>{_OF_T("Send back")}</button>
        </div>
      )}
      <div className="of-card-foot">
        <span>{_OF_T("Open PRs: {n}", { n: office.openPulls })}</span>
        <button className="of-link" onClick={act(onMemory)}>{_OF_T("Lessons kept: {n}", { n: office.lessons })}</button>
      </div>
    </div>
  );
}

/** A small dialog over the window: `children`, then Cancel and the action. */
function OfDialog({ title, note, action, danger, busy, error, onAction, onClose, children }) {
  React.useEffect(() => {
    const onKey = e => { if (e.key === "Escape") { e.stopPropagation(); onClose(); } };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [onClose]);
  return (
    <div className="of-scrim" onClick={onClose}>
      <div className="of-dialog" role="dialog" aria-modal="true" onClick={e => e.stopPropagation()}>
        <div className="of-dialog-title">{title}</div>
        {note && <div className="of-dialog-note">{note}</div>}
        {children}
        {error && <div className="of-error selectable">{error}</div>}
        <div className="of-dialog-actions">
          <button className="btn outlined" onClick={onClose}>{_OF_T(action ? "Cancel" : "Close")}</button>
          {action && <button className={`btn ${danger ? "danger outlined" : "primary"}`} disabled={busy} onClick={onAction}>{action}</button>}
        </div>
      </div>
    </div>
  );
}

/** Runs one `/office` command for a dialog: `{ busy, error, run(args) → ok }`. omp's refusal is shown as it is. */
function useOfCommand(link) {
  const [busy, setBusy] = React.useState(false);
  const [error, setError] = React.useState("");
  const run = React.useCallback(async args => {
    setBusy(true);
    setError("");
    try {
      const result = await link.command(args);
      if (result.failed) setError(result.text);
      return !result.failed;
    } catch (e) {
      setError(String(e?.message ?? e));
      return false;
    } finally {
      setBusy(false);
    }
  }, [link]);
  return { busy, error, run };
}

function OfMergeDialog({ pull, link, onClose }) {
  const cmd = useOfCommand(link);
  return (
    <OfDialog title={_OF_T("Merge #{n} “{title}” into {branch}?", { n: pull.number, title: pull.title, branch: pull.target })}
      note={_OF_T("The change lands on your checked-out branch as one commit.")} action={_OF_T("Merge")} busy={cmd.busy} error={cmd.error}
      onAction={async () => { if (await cmd.run(["merge", String(pull.number)])) onClose(); }} onClose={onClose}>
      {pull.body && <div className="of-dialog-body selectable">{pull.body}</div>}
    </OfDialog>
  );
}

function OfRejectDialog({ pull, link, onClose }) {
  const cmd = useOfCommand(link);
  const [reason, setReason] = React.useState("");
  return (
    <OfDialog title={_OF_T("Why does #{n} go back?", { n: pull.number })} note={_OF_T("The office reads this as the review comment.")}
      action={_OF_T("Send back")} busy={cmd.busy || !reason.trim()} error={cmd.error}
      onAction={async () => { if (await cmd.run(["reject", String(pull.number), reason.trim()])) onClose(); }} onClose={onClose}>
      <textarea className="of-input" rows={4} autoFocus value={reason} onChange={e => setReason(e.target.value)} />
    </OfDialog>
  );
}

function OfMemoryDialog({ office, link, onClose }) {
  const cmd = useOfCommand(link);
  const [confirming, setConfirming] = React.useState(false);
  const { notes, log } = office.memory;
  const empty = notes.length + log.length === 0;
  if (confirming) {
    return (
      <OfDialog title={_OF_T("Clear what office {n} remembers about this project?", { n: office.number })} action={_OF_T("Forget everything")} danger
        busy={cmd.busy} error={cmd.error} onAction={async () => { if (await cmd.run(["forget", String(office.number)])) onClose(); }} onClose={() => setConfirming(false)} />
    );
  }
  return (
    <OfDialog title={`${_OF_T("Office {n}", { n: office.number })} · ${_OF_T("Memory of this project")}`} onClose={onClose}>
      <div className="of-memory selectable">
        {empty && <div className="of-empty">{_OF_T("Nothing remembered yet.")}</div>}
        {notes.length > 0 && <div className="of-block-k">{_OF_T("Lessons")}</div>}
        {notes.map((n, i) => <div key={i} className="of-memory-row">{n.text}<span className="of-memory-by mono">{n.by}</span></div>)}
        {log.length > 0 && <div className="of-block-k">{_OF_T("Merged work")}</div>}
        {log.map((e, i) => <div key={i} className="of-memory-row">#{e.pr} {e.title}<span className="of-memory-by mono">{e.author} → {e.target}</span></div>)}
      </div>
      {!empty && <button className="btn danger outlined of-forget" onClick={() => setConfirming(true)}>{_OF_T("Forget everything")}</button>}
    </OfDialog>
  );
}

function OfRoster({ view, link, onClose }) {
  const cmd = useOfCommand(link);
  const [removing, setRemoving] = React.useState(null);
  const [models, setModels] = React.useState([]);
  React.useEffect(() => {
    window.__TAURI__?.core.invoke("model_usage_list", { profile: null }).then(list => setModels(Array.isArray(list) ? list : []), () => {});
  }, []);
  const setModel = (id, value) => { const v = value.trim(); cmd.run(["set", id, "model", v || "default"]); };
  const setEffort = (id, value) => cmd.run(["set", id, "thinking", value || "default"]);
  if (removing) {
    return (
      <OfDialog title={_OF_T("Remove {name}? The name is not reused.", { name: removing })} action={_OF_T("Remove")} danger busy={cmd.busy} error={cmd.error}
        onAction={async () => { if (await cmd.run(["remove", removing])) setRemoving(null); }} onClose={() => setRemoving(null)} />
    );
  }
  return (
    <div className="of-scrim side" onClick={onClose}>
      <div className="of-drawer" role="dialog" aria-modal="true" onClick={e => e.stopPropagation()}>
        <div className="of-drawer-head">
          <span className="of-dialog-title">{_OF_T("Roster")}</span>
          <button className="btn icon ghost" aria-label={_OF_T("Close")} onClick={onClose}><_OF_Icon name="close" size={12} /></button>
        </div>
        <div className="of-drawer-note">{_OF_T("The roster is shared by every tab.")}</div>
        <div className="of-drawer-note">{_OF_T("An empty model uses the capability's tier from Settings → Subagents; an empty effort uses that model's default.")}</div>
        {cmd.error && <div className="of-error selectable">{cmd.error}</div>}
        <div className="of-roster-row">
          <span className="of-block-k">{_OF_T("Company size")}</span>
          <div className="of-seg">
            {_OF_SCALES.map(([id, label, n]) => (
              <button key={id} className={view.scale === id ? "on" : ""} disabled={cmd.busy} onClick={() => cmd.run(["scale", id])}>{_OF_T(label)} · {n}</button>
            ))}
          </div>
        </div>
        <datalist id="of-models">{models.map(m => <option key={m} value={m} />)}</datalist>
        {view.offices.map(office => (
          <div key={office.number} className="of-roster-office">
            <div className="of-roster-row">
              <span className="of-card-title">{_OF_T("Office {n}", { n: office.number })}</span>
              <span className="of-block-k">{_OF_T("Members")} {office.members.length}</span>
              <button className="btn outlined" disabled={cmd.busy || office.members.length >= 6} onClick={() => cmd.run(["add", String(office.number)])}>{_OF_T("Add a worker")}</button>
            </div>
            {office.members.map(m => (
              <div key={m.id} className="of-roster-member">
                <span className="of-member-name mono">{m.role === "leader" ? "★ " : ""}{m.id}</span>
                <select aria-label={_OF_T("Capability")} value={m.capability} disabled={cmd.busy} onChange={e => cmd.run(["set", m.id, "capability", e.target.value])}>
                  {_OF_CAPS.map(c => <option key={c} value={c}>{_OF_T(`cap.${c}`)}</option>)}
                </select>
                <input key={m.model} className="of-input mono" list="of-models" aria-label={_OF_T("Model")} defaultValue={m.model} placeholder={_OF_T("default model")} disabled={cmd.busy}
                  onKeyDown={e => { if (e.key === "Enter") e.currentTarget.blur(); }}
                  onBlur={e => { if (e.target.value.trim() !== m.model) setModel(m.id, e.target.value); }} />
                <select aria-label={_OF_T("Effort")} value={m.thinking} disabled={cmd.busy} onChange={e => setEffort(m.id, e.target.value)}>
                  <option value="">{_OF_T("default effort")}</option>
                  {_OF_EFFORTS.map(l => <option key={l} value={l}>{l}</option>)}
                </select>
                {m.role !== "leader" && (
                  <>
                    <button className="btn ghost" disabled={cmd.busy} onClick={() => cmd.run(["set", m.id, "role", "leader"])}>{_OF_T("Make leader")}</button>
                    <button className="btn danger" disabled={cmd.busy || office.members.length <= 3} onClick={() => setRemoving(m.id)}>{_OF_T("Remove")}</button>
                  </>
                )}
              </div>
            ))}
          </div>
        ))}
      </div>
    </div>
  );
}

/** The tab's Office switch. The main window owns the tab's flag (it stores it and
   sends omp `set_office_enabled`), so this window only asks it to. `pending`
   holds the asked value until omp's state shows it; the main window may also
   refuse silently, so the ask lapses after a while. */
function useOfEnabled(sessionId, enabled) {
  const [pending, setPending] = React.useState(null);
  React.useEffect(() => { if (pending !== null && enabled === pending) setPending(null); }, [enabled, pending]);
  React.useEffect(() => {
    if (pending === null) return undefined;
    const timer = setTimeout(() => setPending(null), _OF_PENDING_MS);
    return () => clearTimeout(timer);
  }, [pending]);
  const set = on => {
    setPending(on);
    window.__TAURI__.event.emitTo("main", "office://set-enabled", { sessionId, enabled: on }).catch(() => setPending(null));
  };
  return { pending, set };
}

function OfSwitch({ on, pending, disabled, onChange }) {
  const busy = pending !== null;
  const shown = busy ? pending : on;
  const label = busy ? _OF_T(pending ? "Turning on…" : "Turning off…") : _OF_T("Office");
  return (
    <button type="button" className={`btn ghost of-switch${busy ? " pending" : ""}`} role="switch" aria-checked={shown} aria-busy={busy}
      disabled={disabled || busy} title={_OF_T(on ? "Turn Virtual Office off for this tab" : "Turn Virtual Office on for this tab")}
      onClick={() => onChange(!on)}>
      <span className="of-switch-track" aria-hidden="true" />
      <span className="of-switch-label">{label}</span>
    </button>
  );
}

/** Office is off for this tab: what it is, how work flows, and the way to turn it on. */
function OfOff({ busy, disabled, onEnable }) {
  return (
    <div className="of-off">
      <div className="of-off-panel">
        <span className="of-off-icon" aria-hidden="true"><_OF_Icon name="grid" size={18} /></span>
        <h1 className="of-off-title">{_OF_T("Virtual Office is off for this tab")}</h1>
        <p className="of-off-text">{_OF_T("Turn it on and the general manager splits your requests among offices of agents. Each office reviews its own pull requests; only work that passes reaches you to merge.")}</p>
        <ol className="of-flow" aria-label={_OF_T("How work reaches you")}>
          {_OF_FLOW.map(label => <li key={label} className={label === "Waiting for you" ? "hot" : undefined}>{_OF_T(label)}</li>)}
        </ol>
        <button type="button" className="btn primary of-off-action" disabled={disabled || busy} onClick={onEnable}>
          {busy ? _OF_T("Turning on…") : _OF_T("Enable Office")}
        </button>
        <span className="of-off-note">{_OF_T("Applies to this tab only. The roster is shared by every tab.")}</span>
      </div>
    </div>
  );
}

/** No state yet: the outline of the meter, the funnel and the cards. */
function OfLoading() {
  return (
    <div className="of-body of-loading" role="status" aria-label={_OF_T("Loading…")}>
      <div className="of-skel of-skel-meter" />
      <div className="of-skel-row">{_OF_FLOW.map(label => <span key={label} className="of-skel of-skel-pill" />)}</div>
      <div className="of-grid">{[1, 2, 3].map(n => <div key={n} className="of-skel of-skel-card" />)}</div>
    </div>
  );
}

function OfficeWindow({ sessionId, onClose }) {
  const rootRef = React.useRef(null);
  const wc = window.useWindowChrome(onClose, rootRef);
  const { state, online, link } = useOffice(sessionId);
  const fx = useFxRate();
  const [range, setRangeState] = React.useState(() => { try { return localStorage.getItem(_OF_RANGE_KEY) === "total" ? "total" : "today"; } catch (_) { return "today"; } });
  const setRange = r => { setRangeState(r); try { localStorage.setItem(_OF_RANGE_KEY, r); } catch (_) {} };
  const [detail, setDetail] = React.useState(null);
  const [dialog, setDialog] = React.useState(null);
  const view = React.useMemo(() => _OF_M.view(state), [state]);
  const enabled = useOfEnabled(sessionId, view?.enabled);
  const openOffice = view?.enabled ? view.offices.find(o => o.number === detail) ?? null : null;
  const scaleLabel = _OF_SCALES.find(s => s[0] === view?.scale)?.[1] ?? "";
  const live = online.on && view?.enabled;

  return (
    <div className="of-window" ref={rootRef}>
      <div className="of-head" {...wc.headProps}>
        {wc.lights}
        <span className="of-head-title">NPi Office</span>
        {view?.company && <span className="mono of-head-repo">{view.company.name} @ {view.company.branch}</span>}
        {view && <span className="of-head-meta">{_OF_T(scaleLabel)} · {_OF_T("{offices} offices · {members} members", { offices: view.offices.length, members: view.memberCount })}</span>}
        <span className="of-head-fill" />
        <span className={`of-online${live ? " on" : ""}`} title={live ? _OF_T("General manager online") : _OF_T("Offline")}>
          <span className="of-dot" /><span className="of-online-label">{live ? _OF_T("General manager online") : _OF_T("Offline")}</span>
        </span>
        {view && <OfSwitch on={view.enabled} pending={enabled.pending} disabled={!online.on} onChange={enabled.set} />}
        {view && <button className="btn outlined" onClick={() => setDialog({ kind: "roster" })}>{_OF_T("Roster")}</button>}
      </div>
      {!online.on && (
        <div className="of-banner warn" role="alert">
          <_OF_Icon name="warn" size={13} />
          <span className="of-banner-text">{_OF_T("The general manager's tab is closed or its agent stopped. The figures below are the last ones received.")}</span>
          <button className="btn outlined" onClick={() => link.refresh()}><_OF_Icon name="refresh" size={11} /> {_OF_T("Retry")}</button>
        </div>
      )}
      {view?.enabled && !view.company && (
        <div className="of-banner">
          <_OF_Icon name="info" size={13} />
          <span className="of-banner-text">{_OF_T("No company here: {reason}", { reason: view.reason })}</span>
        </div>
      )}
      {!view ? (online.on && <OfLoading />) : !view.enabled ? (
        <OfOff busy={enabled.pending !== null} disabled={!online.on} onEnable={() => enabled.set(true)} />
      ) : (
        <div className="of-body">
          <OfMeter view={view} range={range} onRange={setRange} fx={fx} />
          <OfFunnel funnel={view.funnel} />
          <div className={`of-grid n${Math.min(view.offices.length, 6)}`}>
            {view.offices.map(office => (
              <OfCard key={office.number} office={office} range={range} fx={fx} branch={view.company?.branch ?? ""}
                onOpen={() => setDetail(office.number)}
                onMerge={pull => setDialog({ kind: "merge", pull })} onReject={pull => setDialog({ kind: "reject", pull })}
                onMemory={() => setDialog({ kind: "memory", office: office.number })} />
            ))}
          </div>
        </div>
      )}
      {openOffice && (
        <OfficeDetail office={openOffice} state={state} range={range} fx={fx} link={link}
          onMerge={pull => setDialog({ kind: "merge", pull })} onReject={pull => setDialog({ kind: "reject", pull })} onClose={() => setDetail(null)} />
      )}
      {dialog?.kind === "merge" && <OfMergeDialog pull={dialog.pull} link={link} onClose={() => setDialog(null)} />}
      {dialog?.kind === "reject" && <OfRejectDialog pull={dialog.pull} link={link} onClose={() => setDialog(null)} />}
      {dialog?.kind === "memory" && view?.offices.find(o => o.number === dialog.office) && (
        <OfMemoryDialog office={view.offices.find(o => o.number === dialog.office)} link={link} onClose={() => setDialog(null)} />
      )}
      {dialog?.kind === "roster" && view && <OfRoster view={view} link={link} onClose={() => setDialog(null)} />}
    </div>
  );
}

Object.assign(window, { OfficeWindow, OfMeter, OfDialog, useOfMeter });
