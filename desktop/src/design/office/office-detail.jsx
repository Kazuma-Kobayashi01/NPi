/* office-detail.jsx — one office, opened from its card in the Office window:
   a column per member (leader first) with that member's assignments and the
   conversation of each. Read-only: nothing here sends to a member or stops
   one. A run's conversation is read from its session file in steps
   (`get_subagent_messages` from the last byte), about once a second while the
   run is in progress, once when it has ended. */

const _OD_T = (key, vars) => window.OMP_I18N.t(key, vars);
const { Icon: _OD_Icon, OMP_SUBAGENTS: _OD_S, OMP_OFFICE_MODEL: _OD_M, OMP_OFFICE_METER: _OD_P } = window;
const _OD_POLL_MS = 1200;

/** When a run started: the time, with the date when that was not today. */
function odWhen(iso) {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "";
  const time = d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  return d.toDateString() === new Date().toDateString() ? time : `${d.toLocaleDateString([], { month: "numeric", day: "numeric" })} ${time}`;
}

const odBlocks = content => (typeof content === "string" ? [{ type: "text", text: content }] : content ?? []);

/** The conversation of `run`: `{ messages, status }` with status loading | ready | missing. Polls while the run is open. */
function useOdTranscript(run, link, enabled) {
  const [data, setData] = React.useState({ messages: [], status: "loading" });
  const file = run.sessionFile;
  const ended = !!run.endedAt;
  React.useEffect(() => {
    if (!enabled) return undefined;
    if (!file) { setData({ messages: [], status: ended ? "missing" : "loading" }); return undefined; }
    let live = true;
    let timer = null;
    let transcript = null;
    const step = async () => {
      try {
        const next = await link.transcript(file, transcript?.nextByte ?? 0);
        if (!live) return;
        // No new bytes: keep the same object so the column does not re-render.
        if (!transcript || next.reset || (next.messages ?? []).length > 0) {
          transcript = _OD_S.mergeTranscript(transcript, next);
          setData({ messages: transcript.messages, status: "ready" });
        } else {
          transcript = { ...transcript, nextByte: next.nextByte ?? transcript.nextByte };
        }
      } catch (_) {
        if (!live) return;
        if (!transcript) setData({ messages: [], status: "missing" });
      }
      if (live && !ended) timer = setTimeout(step, _OD_POLL_MS);
    };
    step();
    return () => { live = false; clearTimeout(timer); };
  }, [file, ended, enabled, link]);
  return data;
}

const OdMessages = React.memo(function OdMessages({ messages }) {
  const rows = [];
  messages.forEach((m, i) => {
    if (m.role === "assistant") {
      odBlocks(m.content).forEach((b, j) => {
        if (b.type === "text" && b.text?.trim()) rows.push(<div key={`${i}.${j}`} className="od-text selectable">{b.text}</div>);
        else if (b.type === "toolCall") rows.push(<div key={`${i}.${j}`} className="od-call"><span className="od-tool mono">{b.name}</span><span className="od-args">{_OD_S.formatArgs(b.arguments)}</span></div>);
      });
    } else if (m.role === "toolResult" && m.isError) {
      rows.push(<div key={i} className="od-result err">{_OD_S.blockText(m.content).split("\n").slice(0, 3).join("\n")}</div>);
    }
  });
  return <>{rows}</>;
});

function OdRun({ run, link, open, onToggle, onReport }) {
  const ended = !!run.endedAt;
  const transcript = useOdTranscript(run, link, open);
  const outcome = !ended ? "In progress" : run.failed ? "Did not finish" : "Finished";
  return (
    <div className={`od-run${ended ? "" : " live"}${run.failed ? " failed" : ""}`}>
      <button className="od-run-head" onClick={onToggle} aria-expanded={open} title={_OD_T(open ? "Hide the conversation" : "Show the conversation")}>
        <span className="od-run-by">{_OD_T("Assigned by {by}", { by: run.by === _OD_M.MANAGER ? _OD_T("General manager") : run.by })} · {odWhen(run.startedAt)}</span>
        <span className="od-run-task">{run.task}</span>
      </button>
      {open && (
        <div className="od-run-body">
          {transcript.status === "loading" && transcript.messages.length === 0 && <div className="of-empty"><span className="shimmer-text">{_OD_T("Loading…")}</span></div>}
          {transcript.status === "missing" && <div className="of-empty">{_OD_T("The conversation of this run is no longer available.")}</div>}
          <OdMessages messages={transcript.messages} />
        </div>
      )}
      <div className="od-run-foot">
        <span className={`od-outcome${ended ? "" : " live"}${run.failed ? " failed" : ""}`}>{_OD_T(outcome)}</span>
        {run.report && <button className="of-link od-report" title={run.report} onClick={() => onReport(run)}>{_OD_T("Report: {title}", { title: run.report })}</button>}
      </div>
    </div>
  );
}

function OdColumn({ member, state, range, fx, link, onReport }) {
  const runs = React.useMemo(() => _OD_M.memberRuns(state, member.id), [state, member.id]);
  const last = runs[runs.length - 1];
  // The latest run is open; earlier ones open on demand. A new run takes over.
  const [opened, setOpened] = React.useState(() => new Set());
  const [closedLast, setClosedLast] = React.useState(null);
  const [showEarlier, setShowEarlier] = React.useState(false);
  const isOpen = run => (run === last ? closedLast !== run.id : opened.has(run.id));
  const toggle = run => {
    if (run === last) setClosedLast(c => (c === run.id ? null : run.id));
    else setOpened(set => { const next = new Set(set); if (next.has(run.id)) next.delete(run.id); else next.add(run.id); return next; });
  };
  // Follow the bottom while the reader is there; once they scroll up, stop and offer the way back.
  const bodyRef = React.useRef(null);
  const [following, setFollowing] = React.useState(true);
  const onScroll = e => { const el = e.currentTarget; setFollowing(el.scrollHeight - el.scrollTop - el.clientHeight < 48); };
  React.useEffect(() => {
    const el = bodyRef.current;
    if (!el || !following) return undefined;
    const pin = () => { el.scrollTop = el.scrollHeight; };
    pin();
    const observer = new MutationObserver(pin);
    observer.observe(el, { childList: true, subtree: true, characterData: true });
    return () => observer.disconnect();
  }, [following, last?.id]);
  const u = member.usage[range];
  const shown = showEarlier ? runs : runs.slice(-1);
  return (
    <div className={`od-col ${member.status}`}>
      <div className="od-col-head">
        <div className="od-col-name">
          <span className={`of-dot ${member.status}`} />
          <span className="mono">{member.role === "leader" ? "★ " : ""}{member.id}</span>
          <span className={`of-cap ${member.capability}`}>{_OD_T(`cap.${member.capability}`)}</span>
        </div>
        <div className="od-col-meta">
          <span>{_OD_T(member.role === "leader" ? "Leader" : "Worker")}</span>
          <span title={member.model || undefined}>{member.model ? window.saModelName(member.model) : _OD_T("capability tier")}{member.thinking ? ` · ${member.thinking}` : ""}</span>
        </div>
        <div className="od-col-use mono">{_OD_P.fmtTokensShort(u.tokens)} · {_OD_P.fmtUsd(u.cost)} · {_OD_P.fmtJpy(_OD_P.toJpy(u.cost, fx.rate))}</div>
      </div>
      <div className="od-col-body" ref={bodyRef} onScroll={onScroll}>
        {runs.length === 0 && <div className="of-empty">{_OD_T("No assignment yet.")}</div>}
        {runs.length > 1 && !showEarlier && (
          <button className="of-link od-earlier" onClick={() => setShowEarlier(true)}>{_OD_T("Earlier assignments: {n}", { n: runs.length - 1 })}</button>
        )}
        {shown.map(run => <OdRun key={run.id} run={run} link={link} open={isOpen(run)} onToggle={() => toggle(run)} onReport={onReport} />)}
      </div>
      {!following && (
        <button className="btn outlined od-latest" onClick={() => { setFollowing(true); }}>{_OD_T("Back to latest")}</button>
      )}
    </div>
  );
}

function OfficeDetail({ office, state, range, fx, link, onReport, onClose }) {
  React.useEffect(() => {
    const onKey = e => { if (e.key === "Escape" && !document.querySelector(".of-scrim")) onClose(); };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);
  const view = { working: office.members.some(m => m.status === "working") };
  return (
    <div className="od-page">
      <div className="od-head" data-wc-head="" data-tauri-drag-region="">
        <button className="btn outlined" onClick={onClose}><_OD_Icon name="back" size={11} /> {_OD_T("Back")}</button>
        <span className="of-card-title">{_OD_T("Office {n}", { n: office.number })}</span>
        <span className={`of-phase ${office.phase}`}>{_OD_T(`phase.${office.phase}`)}</span>
        <span className="chip muted">{_OD_T("Read-only")}</span>
        <span className="of-head-fill" />
        <window.OfMeter view={view} compact={office.usage[range]} fx={fx} />
      </div>
      {office.task && <div className="od-task" title={office.task}>{_OD_T("Now: {task}", { task: office.task })}</div>}
      <div className="od-cols" style={{ "--od-cols": office.members.length }}>
        {office.members.map(m => <OdColumn key={m.id} member={m} state={state} range={range} fx={fx} link={link} onReport={onReport} />)}
      </div>
    </div>
  );
}

Object.assign(window, { OfficeDetail });
