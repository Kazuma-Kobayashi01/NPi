// app/panel-link.js — the main window and the secondary windows that show its
// state (history, approval rules, plan, git browser: panel_window.rs).
//
// Those panels need the main window's tabs, which only its live.js holds, so
// they talk to it through app events:
// - state: the main window `publish(kind, state)`es a JSON snapshot as
//   `panel-state:<kind>`; a panel `subscribe`s and says `panel-hello` when it
//   starts, which the main window answers with the latest snapshot.
// - calls: a panel's `proxy(kind, names)` is a bridge whose methods run in the
//   main window (`panel-call` → `panel-reply:<id>`), on the main window's
//   OMP_BRIDGE or an action of `serve(kind, actions)`. Only names listed for
//   that kind in `CALLS` run; anything else is refused.
// - presence: `panel-hello` / `panel-bye` keep the set of open kinds, so the
//   main window computes a snapshot only while its panel is open.
(function () {
  const ev = () => window.__TAURI__?.event;

  /** The bridge methods (and actions) each panel kind may run in the main window. */
  const CALLS = {
    history: ["listSavedSessions", "resume"],
    rules: ["listApprovalRules", "revokeApprovalRule", "approvalModes", "setApprovalModeDefault", "setApprovalModeProject"],
    plan: ["abort"],
    git: [
      "gitBrowserStatus", "gitBrowserLog", "gitBrowserFile", "gitBrowserCommit", "gitBrowserCommitFile",
      "workspaceAccept", "workspaceReject", "workspaceDiscardHunk", "worktreeInfo", "worktreeMerge", "worktreeRemove",
      "sendToAgent",
    ],
  };

  // ── Main window ───────────────────────────────────────────────────────────
  const last = new Map();      // kind → latest snapshot
  const sent = new Map();      // kind → its JSON, to skip repeats
  const actions = new Map();   // kind → { name: fn }
  const open = new Set();      // kinds whose panel said hello and not bye
  const presence = new Set();  // callbacks on `open` changes
  let wired = false;

  function wireMain() {
    if (wired || !ev() || window.OMP_PANEL) return;
    wired = true;
    const changed = () => presence.forEach(fn => fn(new Set(open)));
    ev().listen("panel-hello", e => {
      const kind = e.payload?.kind;
      if (!Object.hasOwn(CALLS, kind)) return;
      open.add(kind);
      changed();
      if (last.has(kind)) ev().emit(`panel-state:${kind}`, last.get(kind)).catch(() => {});
    }).catch(error => console.error("[panel-link] hello listener failed:", error));
    ev().listen("panel-bye", e => {
      if (open.delete(e.payload?.kind)) changed();
    }).catch(error => console.error("[panel-link] bye listener failed:", error));
    ev().listen("panel-call", async e => {
      const { id, kind, name, args } = e.payload ?? {};
      if (typeof id !== "string") return;
      const reply = body => ev().emit(`panel-reply:${id}`, body).catch(() => {});
      if (!CALLS[kind]?.includes(name)) return reply({ ok: false, error: `not allowed: ${kind}.${name}` });
      const own = actions.get(kind)?.[name];
      const fn = own ?? window.OMP_BRIDGE?.[name]?.bind(window.OMP_BRIDGE);
      if (typeof fn !== "function") return reply({ ok: false, error: `unavailable: ${name}` });
      try {
        reply({ ok: true, value: (await fn(...(Array.isArray(args) ? args : []))) ?? null });
      } catch (err) {
        reply({ ok: false, error: String(err?.message ?? err) });
      }
    }).catch(error => console.error("[panel-link] call listener failed:", error));
  }

  /** Sends `state` (plain JSON) to the panel of `kind`, now and to one that opens later;
   *  a snapshot equal to the last one is not sent again. */
  function publish(kind, state) {
    wireMain();
    const json = JSON.stringify(state);
    if (sent.get(kind) === json) return;
    sent.set(kind, json);
    last.set(kind, state);
    if (open.has(kind)) ev()?.emit(`panel-state:${kind}`, state).catch(() => {});
  }

  /** Main-window handlers for the calls of `kind` that are not bridge methods. */
  function serve(kind, handlers) {
    wireMain();
    actions.set(kind, handlers);
  }

  /** `fn(openKinds)` whenever a panel opens or closes; returns the unsubscribe. */
  function onPresence(fn) {
    wireMain();
    presence.add(fn);
    fn(new Set(open));
    return () => presence.delete(fn);
  }

  // ── Panel window ──────────────────────────────────────────────────────────
  let seq = 0;

  /** Runs `name(...args)` in the main window; resolves to its value, rejects with its error. */
  async function call(kind, name, args) {
    const events = ev();
    if (!events) throw new Error("no desktop shell");
    const id = `${kind}-${Date.now().toString(36)}-${(seq += 1)}`;
    let settle = null;
    const answer = new Promise((resolve, reject) => { settle = { resolve, reject }; });
    // Listening before the call goes out, so a quick reply is not missed.
    const unlisten = await events.listen(`panel-reply:${id}`, e => {
      if (e.payload?.ok) settle.resolve(e.payload.value);
      else settle.reject(new Error(e.payload?.error ?? "failed"));
    });
    try {
      await events.emit("panel-call", { id, kind, name, args });
      return await answer;
    } finally {
      unlisten();
    }
  }

  /** A bridge for a panel of `kind`: each method of CALLS[kind] runs in the main window. */
  function proxy(kind) {
    return Object.fromEntries((CALLS[kind] ?? []).map(name => [name, (...args) => call(kind, name, args)]));
  }

  /** `fn(state)` on every snapshot the main window sends for `kind`; returns the unsubscribe. */
  function subscribe(kind, fn) {
    const events = ev();
    if (!events) return () => {};
    let unlisten = null;
    let gone = false;
    events.listen(`panel-state:${kind}`, e => fn(e.payload)).then(u => {
      if (gone) u();
      else unlisten = u;
      events.emit("panel-hello", { kind }).catch(() => {});
    });
    const bye = () => events.emit("panel-bye", { kind }).catch(() => {});
    window.addEventListener("beforeunload", bye);
    return () => {
      gone = true;
      unlisten?.();
      window.removeEventListener("beforeunload", bye);
      bye();
    };
  }

  window.OMP_PANEL_LINK = { CALLS, publish, serve, onPresence, call, proxy, subscribe };
})();
