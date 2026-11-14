/* chat/inline-plan.jsx — InlinePlan: the plan block of an assistant reply
   (built from the agent's `todo` tool call: live by live.js `_injectInlinePlan`,
   from history by adapter.js `adaptAgentMessages`; both through `buildInlinePlan`).

   The newest block (`plan.live`) can be edited in place: tick a task done or
   open again, move it inside its phase (the arrow buttons, Alt+↑/↓ on a
   focused row, or a drag, ui/use-drag-reorder.js), drop it. The edits are
   only staged: the block shows them as modified, and Apply sends them to the
   agent as one message (app/plan-edit.js `buildMessage`, delivered by
   `OMP_BRIDGE.sendPlanEdit`: a steer while a turn runs, else a prompt) —
   Discard throws them away. omp's todo list is the agent's own, so nothing
   changes until its next `todo` call rewrites the block (and `state.kanban`,
   which the Todo and plan panel and the rail card show); staged edits do not
   survive that rewrite, they were made against the old list. Only the newest
   block is live (adapter.js `markLatestPlanLive`); earlier plan blocks stay as
   the read-only snapshots they were. */

const { Icon: _IP_Icon } = window;
const _IP = window.OMP_PLAN_EDIT;
// The UI language (app/i18n.js): chrome goes through it at render; the agent's task text never does.
const _IP_T = (key, vars) => window.OMP_I18N.t(key, vars);

// "2 done · 1 reopened · 1 phase reordered · 1 removed" for the staged changes.
function _IP_summary(c) {
  return [
    c.done.length && _IP_T("plan.sumDone", { n: c.done.length }),
    c.reopened.length && _IP_T("plan.sumReopened", { n: c.reopened.length }),
    c.reordered.length && _IP_T(c.reordered.length === 1 ? "plan.sumReordered" : "plan.sumReorderedMany", { n: c.reordered.length }),
    c.removed.length && _IP_T("plan.sumRemoved", { n: c.removed.length }),
  ].filter(Boolean).join(" · ");
}

function InlinePlan({ plan, onOpenPlan = null }) {
  const bridge = window.OMP_BRIDGE;
  const editable = plan.live === true;
  const sig = _IP.planSignature(plan);
  const [draft, setDraft] = React.useState(null);
  const [sentFor, setSentFor] = React.useState(null);  // signature of the plan the sent edits were made on
  const [note, setNote] = React.useState(null);  // a `plan.note*` dictionary key, translated at render
  const [busy, setBusy] = React.useState(false);
  const bodyRef = React.useRef(null);
  const focusId = React.useRef(null);

  // The agent rewrote the list (or a newer plan took over) under the staged edits.
  React.useEffect(() => {
    if (draft && (!editable || draft.sig !== sig)) {
      setDraft(null);
      if (editable) setNote("plan.noteRewritten");
    }
  }, [draft, editable, sig]);

  // A moved row keeps the keyboard focus (React may re-insert its node).
  React.useLayoutEffect(() => {
    if (focusId.current === null) return;
    const id = focusId.current;
    focusId.current = null;
    [...(bodyRef.current?.querySelectorAll("[data-reorder-id]") ?? [])].find(el => el.dataset.reorderId === id)?.focus();
  });

  const current = editable && draft && draft.sig === sig ? draft : null;
  const changes = current ? _IP.changes(plan, current) : null;
  const dirty = current ? _IP.isDirty(plan, current) : false;

  const edit = (fn, focus = null) => {
    setSentFor(null);
    setNote(null);
    focusId.current = focus;
    setDraft(d => fn(d && d.sig === sig ? d : _IP.startDraft(plan)));
  };
  const reorder = window.useDragReorder((_list, id, targetId, after) => edit(d => _IP.dropTask(d, id, targetId, after), id));

  const discard = () => { setDraft(null); setNote(null); };
  const apply = async () => {
    const text = dirty ? _IP.buildMessage(plan, current) : null;
    if (!text || busy) return;
    setBusy(true);
    const ok = await bridge.sendPlanEdit(text);
    setBusy(false);
    if (ok) { setDraft(null); setNote(null); setSentFor(sig); }
    else setNote("plan.noteFailed");
  };

  const onRowKey = (e, id) => {
    if (!e.altKey || e.ctrlKey || e.metaKey || e.shiftKey || (e.key !== "ArrowUp" && e.key !== "ArrowDown")) return;
    e.preventDefault();
    e.stopPropagation();
    edit(d => _IP.moveTask(d, id, e.key === "ArrowUp" ? -1 : 1), id);
  };

  const phaseTasks = ph => (current ? current.phases.find(p => p.id === ph.id).tasks : ph.tasks);
  const reorderedPhases = new Set(changes ? changes.reordered.map(r => r.phase) : []);

  return (
    <div className={`inline-plan slide-in${editable ? " is-live" : ""}`}>
      <div className="inline-plan-head">
        <_IP_Icon name="plan" size={12} color="var(--accent)" />
        <span style={{ color: "var(--fg-2)", fontWeight: 600 }}>{_IP_T("plan.title")}</span>
        <span className="chip muted">{_IP_T("plan.tasks", { n: plan.phases.reduce((n, p) => n + p.tasks.length, 0) })}</span>
        {dirty && <span className="chip inline-plan-modified">{_IP_T("plan.modified")}</span>}
        <div style={{ flex: 1 }} />
        {onOpenPlan && <button type="button" className="btn ghost" style={{ height: 22 }} onClick={onOpenPlan}>{_IP_T("plan.openKanban")}</button>}
      </div>
      <div className="inline-plan-body" ref={bodyRef} onPointerDown={editable ? reorder.onPointerDown : undefined}>
        {plan.phases.map((ph) => {
          const tasks = phaseTasks(ph);
          return (
            <div key={ph.id} className="inline-phase">
              <div className="inline-phase-head">
                <span className="mono" style={{ color: "var(--fg-3)" }}>{ph.label.toUpperCase()}</span>
                {reorderedPhases.has(ph.label) && <span className="chip inline-plan-modified">{_IP_T("plan.reordered")}</span>}
                <span className="hr" />
              </div>
              {tasks.map((t, i) => {
                const original = ph.tasks.find(o => o.id === t.id);
                const edited = !!current && (t.removed || t.status !== original.status);
                const rowProps = editable ? {
                  ...reorder.itemProps(ph.id, t.id, "y"),
                  tabIndex: 0,
                  onKeyDown: e => onRowKey(e, t.id),
                } : {};
                return (
                  <div key={t.id}
                    className={`inline-task status-${t.status}${editable ? " is-editable" : ""}${edited ? " is-edited" : ""}${t.removed ? " is-removed" : ""}${editable ? reorder.dropClass(ph.id, t.id) : ""}`}
                    {...rowProps}>
                    {editable ? (
                      <button type="button" className="task-mark task-toggle" role="checkbox"
                        aria-checked={t.status === "done"} disabled={t.removed}
                        aria-label={_IP_T(t.status === "done" ? "plan.reopen" : "plan.markDone", { task: t.text })}
                        onClick={() => edit(d => _IP.toggleTask(plan, d, t.id))}>
                        {t.status === "done" && <_IP_Icon name="check" size={10} />}
                        {t.status === "in_progress" && <span className="pulse-dot" />}
                        {t.status === "pending" && <span className="mono" style={{ color: "var(--fg-4)" }}>○</span>}
                      </button>
                    ) : (
                      <span className="task-mark">
                        {t.status === "done" && <_IP_Icon name="check" size={10} />}
                        {t.status === "in_progress" && <span className="pulse-dot" />}
                        {t.status === "pending" && <span className="mono" style={{ color: "var(--fg-4)" }}>○</span>}
                      </span>
                    )}
                    <span className="task-text selectable">{t.text}</span>
                    {editable && (
                      <span className="task-ops">
                        <button type="button" className="btn icon ghost task-op" title={_IP_T("plan.moveUpTitle")} aria-label={_IP_T("plan.moveUp", { task: t.text })}
                          disabled={i === 0} onClick={() => edit(d => _IP.moveTask(d, t.id, -1), t.id)}>
                          <_IP_Icon name="arrowUp" size={10} />
                        </button>
                        <button type="button" className="btn icon ghost task-op flip" title={_IP_T("plan.moveDownTitle")} aria-label={_IP_T("plan.moveDown", { task: t.text })}
                          disabled={i === tasks.length - 1} onClick={() => edit(d => _IP.moveTask(d, t.id, 1), t.id)}>
                          <_IP_Icon name="arrowUp" size={10} />
                        </button>
                        {t.removed ? (
                          <button type="button" className="btn icon ghost task-op" title={_IP_T("plan.keepTitle")} aria-label={_IP_T("plan.keep", { task: t.text })}
                            onClick={() => edit(d => _IP.setRemoved(d, t.id, false), t.id)}>
                            <_IP_Icon name="back" size={10} />
                          </button>
                        ) : (
                          <button type="button" className="btn icon ghost task-op" title={_IP_T("plan.removeTitle")} aria-label={_IP_T("plan.remove", { task: t.text })}
                            onClick={() => edit(d => _IP.setRemoved(d, t.id, true), t.id)}>
                            <_IP_Icon name="close" size={10} />
                          </button>
                        )}
                      </span>
                    )}
                  </div>
                );
              })}
            </div>
          );
        })}
      </div>
      {(dirty || note || sentFor === sig) && (
        <div className="inline-plan-foot" role="status">
          <span className="inline-plan-foot-text">
            {dirty ? _IP_summary(changes) : _IP_T(note ?? "plan.sent")}
          </span>
          {dirty && (
            <>
              {note && <span className="inline-plan-foot-note">{_IP_T(note)}</span>}
              <button type="button" className="btn ghost" onClick={discard} disabled={busy}>{_IP_T("plan.discard")}</button>
              <button type="button" className="btn primary" onClick={apply} disabled={busy || !bridge?.isConnected}>{_IP_T("plan.apply")}</button>
            </>
          )}
        </div>
      )}
    </div>
  );
}

Object.assign(window, { InlinePlan });
