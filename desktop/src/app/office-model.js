// app/office-model.js — the Office window's view of a company.
//
// Pure: turns omp's `OfficeState` (get_office_state / office_state frames)
// into what the window draws: company line, per-office cards with their
// members, the pull-request funnel and who spent what. Who is at a desk and
// what was spent both come from the state itself (omp books usage as it
// happens and pushes the state), so nothing here guesses from other frames.
(function () {
  const MANAGER = "general-manager";
  const ZERO = Object.freeze({ tokens: 0, cost: 0, requests: 0 });
  const PHASES = ["working", "reviewing", "awaiting_owner", "awaiting_manager", "idle"];
  const STANDARD_EFFORTS = Object.freeze(["off", "minimal", "low", "medium", "high", "xhigh", "max"]);

  const amount = a => ({ tokens: a?.tokens ?? 0, cost: a?.cost ?? 0, requests: a?.requests ?? 0 });
  const plus = (a, b) => ({ tokens: a.tokens + b.tokens, cost: a.cost + b.cost, requests: a.requests + b.requests });
  const usageOf = (state, name) => {
    const u = state.usage?.members?.[name];
    return { today: amount(u?.today), total: amount(u?.total) };
  };
  const unfinished = p => p.status !== "merged" && p.status !== "closed";

  /** Last path segment of the checkout: the name shown for the company. */
  function repoName(checkout) {
    const parts = String(checkout || "").split(/[\\/]/).filter(Boolean);
    return parts[parts.length - 1] || "";
  }

  /** A member's runs, oldest first. */
  function memberRuns(state, member) {
    return (state?.runs ?? []).filter(r => r.member === member);
  }

  function memberView(state, office, member, working) {
    const runs = memberRuns(state, member.id);
    const last = runs[runs.length - 1];
    const busy = working.has(member.id);
    return {
      id: member.id,
      role: member.role,
      capability: member.capability,
      model: member.model || "",
      thinking: member.thinking || "",
      status: busy ? "working" : last?.failed ? "failed" : "idle",
      usage: usageOf(state, member.id),
      // The run to show first: the one in progress, else the latest.
      run: last ?? null,
      runs: runs.length,
    };
  }

  function officeView(state, office) {
    const activity = (state.activity ?? []).find(a => a.office === office.number);
    const working = new Set(activity?.working ?? []);
    const members = office.members
      .map(m => memberView(state, office, m, working))
      .sort((a, b) => (a.role === "leader" ? 0 : 1) - (b.role === "leader" ? 0 : 1));
    const pulls = (state.pulls ?? []).filter(p => p.office === office.number);
    const memory = (state.memory ?? []).find(m => m.office === office.number);
    const usage = members.reduce((sum, m) => ({ today: plus(sum.today, m.usage.today), total: plus(sum.total, m.usage.total) }),
      { today: ZERO, total: ZERO });
    return {
      number: office.number,
      phase: PHASES.includes(activity?.phase) ? activity.phase : "idle",
      task: activity?.task ?? "",
      members,
      usage,
      pulls,
      openPulls: pulls.filter(unfinished).length,
      // The office pull request only the owner can settle, when there is one.
      waiting: pulls.find(p => p.kind === "office" && p.status === "approved") ?? null,
      lessons: memory?.notes?.length ?? 0,
      memory: memory ?? { office: office.number, notes: [], log: [] },
    };
  }

  /** Everything the window draws, from one `OfficeState`. `null` state → `null`. */
  function view(state) {
    if (!state) return null;
    const offices = (state.offices ?? []).map(o => officeView(state, o));
    const pulls = state.pulls ?? [];
    const manager = usageOf(state, MANAGER);
    const usage = {
      day: state.usage?.day ?? "",
      today: amount(state.usage?.today),
      total: amount(state.usage?.total),
    };
    return {
      enabled: !!state.enabled,
      company: state.company
        ? { name: repoName(state.company.checkout), checkout: state.company.checkout, branch: state.company.baseBranch }
        : null,
      reason: state.reason ?? "",
      scale: state.scale,
      memberCount: offices.reduce((n, o) => n + o.members.length, 0),
      offices,
      manager,
      usage,
      working: offices.some(o => o.members.some(m => m.status === "working")),
      funnel: {
        assigned: offices.reduce((n, o) => n + o.members.filter(m => m.status === "working").length, 0),
        workerReview: pulls.filter(p => p.kind === "worker" && p.status === "open").length,
        officeReview: pulls.filter(p => p.kind === "office" && p.status === "open").length,
        awaitingOwner: pulls.filter(p => p.kind === "office" && p.status === "approved").length,
        merged: pulls.filter(p => p.status === "merged").length,
      },
      waiting: offices.map(o => o.waiting).filter(Boolean),
    };
  }

  /** Who spent what in `range` ("today" | "total"): the manager, then each office; zero shares dropped. */
  function shares(v, range) {
    if (!v) return [];
    const rows = [{ key: "manager", office: null, ...v.manager[range] }, ...v.offices.map(o => ({ key: `office-${o.number}`, office: o.number, ...o.usage[range] }))];
    const total = rows.reduce((n, r) => n + r.tokens, 0);
    return rows.filter(r => r.tokens > 0).map(r => ({ ...r, share: total > 0 ? r.tokens / total : 0 }));
  }

  /** Find the catalog entry matching `model` by selector or provider/id; null if missing or unset. */
  function findCatalogModel(catalog, model) {
    return model ? catalog.find(m => m.selector === model || `${m.provider}/${m.id}` === model) ?? null : null;
  }

  /**
   * Effort options for `model`:
   * - catalog match: ["off", ...entry.thinking without "auto"/"off"] (just ["off"] if no thinking)
   * - capability tier, unknown model, or empty catalog: the standard set.
   */
  function effortChoices(catalog, model) {
    const entry = findCatalogModel(catalog, model);
    if (!entry) return [...STANDARD_EFFORTS];
    const raw = Array.isArray(entry.thinking) ? entry.thinking : [];
    const filtered = raw.filter(l => l && l !== "auto" && l !== "off");
    return ["off", ...filtered];
  }

  window.OMP_OFFICE_MODEL = { MANAGER, PHASES, STANDARD_EFFORTS, view, shares, memberRuns, repoName, findCatalogModel, effortChoices };
})();
