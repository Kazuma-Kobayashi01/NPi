// Inline plan editing: the staged edits a user makes to the plan block in the
// chat (tick a task done or open again, reorder tasks inside a phase, drop a
// task) and the one message that sends them to the agent. omp has no RPC to
// edit its todo list — the agent owns it through its `todo` tool — so an edit is
// a request in words, and the plan only changes when the agent's next
// `todo` result comes back (`state.kanban`, the inline block and the Todo and
// plan panel then all show it). Pure; the UI is design/chat/inline-plan.jsx,
// the send is `OMP_BRIDGE.sendPlanEdit`.
//
// A plan is the inline block live.js `_injectInlinePlan` builds:
//   { phases: [{ id, label, tasks: [{ id, text, status: "pending" | "in_progress" | "done" }] }] }
// A draft is a copy of it that the user edits: `{ sig, phases: [{ id, tasks:
// [{ id, text, status, removed }] }] }`. `sig` ties it to the plan it was
// started on (`planSignature`); once the plan changes the draft is stale.
//
// Exposes `window.OMP_PLAN_EDIT`; wrapped as an IIFE per the project rule for
// plain <script> tags (see AGENTS.md "IIFE rule"). Regression: test-plan-edit.mjs.
(function () {
  /** What identifies the plan content a draft was staged on. */
  function planSignature(plan) {
    return JSON.stringify(plan.phases.map(p => [p.id, p.tasks.map(t => [t.text, t.status])]));
  }

  /** An untouched draft of `plan`. */
  function startDraft(plan) {
    return {
      sig: planSignature(plan),
      phases: plan.phases.map(p => ({
        id: p.id,
        tasks: p.tasks.map(t => ({ id: t.id, text: t.text, status: t.status, removed: false })),
      })),
    };
  }

  /** `draft` with the task `taskId` replaced by `fn(task)`. */
  function _mapTask(draft, taskId, fn) {
    return {
      ...draft,
      phases: draft.phases.map(p => (p.tasks.some(t => t.id === taskId)
        ? { ...p, tasks: p.tasks.map(t => (t.id === taskId ? fn(t) : t)) }
        : p)),
    };
  }

  function _originalTask(plan, taskId) {
    for (const p of plan.phases) {
      const t = p.tasks.find(x => x.id === taskId);
      if (t) return t;
    }
    return null;
  }

  /** The status a click on a task's mark leads to: a done task opens again
   *  (`pending`) and any other becomes done; the click that undoes the edit
   *  lands on the plan's own status again, so an in-progress task is not
   *  turned into a pending one by a double toggle. */
  function nextStatus(original, current) {
    if (current !== "done") return "done";
    return original === "done" ? "pending" : original;
  }

  /** Toggle task `taskId` done / not done. A dropped task does not toggle. */
  function toggleTask(plan, draft, taskId) {
    const original = _originalTask(plan, taskId);
    return _mapTask(draft, taskId, t => (t.removed ? t : { ...t, status: nextStatus(original.status, t.status) }));
  }

  /** Drop task `taskId`, or take it back (`removed` false). */
  function setRemoved(draft, taskId, removed) {
    return _mapTask(draft, taskId, t => (t.removed === removed ? t : { ...t, removed }));
  }

  /** Move task `taskId` one place up (`delta` -1) or down (+1) inside its
   *  phase; at either end it stays. */
  function moveTask(draft, taskId, delta) {
    return {
      ...draft,
      phases: draft.phases.map(p => {
        const from = p.tasks.findIndex(t => t.id === taskId);
        const to = from + delta;
        if (from < 0 || to < 0 || to >= p.tasks.length) return p;
        const tasks = [...p.tasks];
        [tasks[from], tasks[to]] = [tasks[to], tasks[from]];
        return { ...p, tasks };
      }),
    };
  }

  /** Move task `taskId` next to `targetId` of the same phase: before it, or
   *  `after` it (the drop slot of OMP_DRAG_REORDER). */
  function dropTask(draft, taskId, targetId, after) {
    return {
      ...draft,
      phases: draft.phases.map(p => {
        const moving = p.tasks.find(t => t.id === taskId);
        if (!moving || taskId === targetId || !p.tasks.some(t => t.id === targetId)) return p;
        const rest = p.tasks.filter(t => t.id !== taskId);
        const at = rest.findIndex(t => t.id === targetId) + (after ? 1 : 0);
        return { ...p, tasks: [...rest.slice(0, at), moving, ...rest.slice(at)] };
      }),
    };
  }

  /** What the draft changes against `plan`:
   *  `done` / `reopened` / `removed`: `{ phase, text }` per task (phase label,
   *  exact task text; a dropped task is listed only as removed);
   *  `reordered`: `{ phase, order }` per phase whose remaining tasks changed
   *  places, `order` their texts in the new order. */
  function changes(plan, draft) {
    const out = { done: [], reopened: [], removed: [], reordered: [] };
    for (const phase of plan.phases) {
      const dp = draft.phases.find(p => p.id === phase.id);
      const original = new Map(phase.tasks.map(t => [t.id, t]));
      for (const t of dp.tasks) {
        const entry = { phase: phase.label, text: t.text };
        if (t.removed) out.removed.push(entry);
        else if (t.status !== original.get(t.id).status) (t.status === "done" ? out.done : out.reopened).push(entry);
      }
      const before = phase.tasks.filter(t => !dp.tasks.find(d => d.id === t.id).removed).map(t => t.id);
      const after = dp.tasks.filter(t => !t.removed);
      if (after.some((t, i) => t.id !== before[i])) {
        out.reordered.push({ phase: phase.label, order: after.map(t => t.text) });
      }
    }
    return out;
  }

  function isDirty(plan, draft) {
    const c = changes(plan, draft);
    return c.done.length + c.reopened.length + c.removed.length + c.reordered.length > 0;
  }

  /** The text sent to the agent for the draft, or null with no change. One
   *  ordinary user message: every task is named by its exact text and its
   *  phase, and a reorder gives the whole new order of the phase, so nothing
   *  depends on positions. It asks for the `todo` call that makes the
   *  agent's list match, then for the work to go on. */
  function buildMessage(plan, draft) {
    const c = changes(plan, draft);
    const lines = [];
    const task = (verb, e) => `- ${verb} (phase ${JSON.stringify(e.phase)}): ${JSON.stringify(e.text)}`;
    for (const e of c.done) lines.push(task("Marked done", e));
    for (const e of c.reopened) lines.push(task("Reopened as pending", e));
    for (const e of c.removed) lines.push(task("Removed from the plan", e));
    for (const r of c.reordered) {
      lines.push(`- Reordered phase ${JSON.stringify(r.phase)}; its remaining tasks are now in this order:`);
      r.order.forEach((text, i) => lines.push(`  ${i + 1}. ${JSON.stringify(text)}`));
    }
    if (lines.length === 0) return null;
    return [
      "I adjusted the plan:",
      ...lines,
      "",
      "Please update the todo list with the todo tool so it matches exactly these changes (leave every other task as it is), then continue with the plan.",
    ].join("\n");
  }

  window.OMP_PLAN_EDIT = Object.freeze({
    planSignature, startDraft, toggleTask, setRemoved, moveTask, dropTask, changes, isDirty, buildMessage,
  });
})();
