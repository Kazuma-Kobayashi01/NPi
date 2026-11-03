// Independent, ephemeral chat worker. Visibility never owns its lifetime: the
// mounted SideChat owns this controller and disposes it only on actual unmount.
(function () {
  class CommandError extends Error {}
  // `startCommand`/`startArgs`: the Rust command that starts the worker and its extra arguments
  // (the Academic Copilot starts through `academic_copilot_start` with its project).
  function create({ sessionId, profile = null, model = "", invoke, listen, onChange,
    startCommand = "sidechat_start", startArgs = {}, timeoutMs = 30000, schedule = setTimeout, cancel = clearTimeout }) {
    const D = window.OMP_STREAM_DELTA;
    // `level`: the thinking level in use ("off" when unset), "" while unknown or when the model has none.
    let state = { phase: "idle", messages: [], liveText: null, error: null, model: "", level: "" };
    let rawLevel = null;          // omp's effective level; null until it reports one
    let levels = [];              // the levels the current model lists, `off` first
    let desiredLevel = "";        // the pane's own pick, kept for the worker's lifetime
    let desiredModel = model;
    let defaultModel = "";
    let appliedModel = null;
    let disposed = false;
    let started = false;
    let startTask = null;
    let disposeTask = null;
    let ready = null;
    let serial = 0;
    let lastSeq = 0;
    let activePrompt = null;
    let accumulator = [];
    const pending = new Map();
    const unlisteners = [];

    function update(patch) {
      state = { ...state, ...patch };
      if (!disposed) onChange(state);
    }
    /** What the pane shows: the level in use, but only for a model with thinking. */
    function publishLevel() {
      const level = levels.some(l => l !== "off") ? (rawLevel ?? "off") : "";
      if (level !== state.level) update({ level });
    }
    function detail(error) { return error instanceof Error ? error.message : String(error); }
    function deferred(label) {
      let resolve, reject;
      const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
      const timer = schedule(() => reject(new Error(`${label} timed out`)), timeoutMs);
      return { promise, resolve: value => { cancel(timer); resolve(value); },
        reject: error => { cancel(timer); reject(error); } };
    }
    function rejectPending(error) {
      ready?.reject(error);
      for (const request of pending.values()) request.reject(error);
      pending.clear();
    }
    function fail(error, fatal = false) {
      const text = textOf(accumulator);
      accumulator = [];
      if (text) update({ messages: [...state.messages, { id: `partial-${++serial}`, role: "assistant", text }] });
      activePrompt = null;
      update({ phase: fatal ? "failed" : "error", liveText: null, error: detail(error) });
    }
    function request(type, params = {}, id = `${sessionId}-${++serial}`) {
      if (disposed) return Promise.reject(new Error("SideChat has been closed"));
      const completion = deferred(`${type} response`);
      pending.set(id, { ...completion, type });
      Promise.resolve().then(() => invoke("send_command", {
        sessionId, json: JSON.stringify({ id, type, ...params }),
      })).catch(error => {
        const waiting = pending.get(id);
        if (waiting) { pending.delete(id); waiting.reject(error); }
      });
      return completion.promise.finally(() => pending.delete(id));
    }
    function textOf(blocks) {
      return D.view(blocks).blocks.filter(block => block.type === "text").map(block => block.text).join("\n\n");
    }
    function finishPrompt(frame) {
      if (frame.id !== activePrompt) return;
      activePrompt = null;
      if (frame.status === "error" && state.phase !== "stopping") fail(new Error(frame.error.message));
      else {
        const text = textOf(accumulator);
        accumulator = [];
        update({ phase: state.phase === "stopping" ? "stopping" : "ready", liveText: null,
          error: frame.status === "error" ? frame.error.message : state.error,
          messages: text ? [...state.messages, { id: `partial-${++serial}`, role: "assistant", text }] : state.messages });
      }
    }
    function frameReceived(frame) {
      if (frame.type === "ready") { ready?.resolve(); return; }
      if (frame.type === "response") {
        const waiting = pending.get(frame.id);
        if (waiting) {
          if (frame.success) waiting.resolve(frame.data);
          else waiting.reject(new CommandError(frame.error));
        } else if (frame.id === activePrompt && !frame.success) {
          fail(new Error(frame.error));
        }
        return;
      }
      if (frame.type === "prompt_result") { finishPrompt(frame); return; }
      // Every change of the applied level (a pick, a model switch re-clamping it): the effective one.
      if (frame.type === "thinking_level_changed") { rawLevel = frame.thinkingLevel ?? "off"; publishLevel(); return; }
      if (!activePrompt) return;
      if (frame.type === "message_start" && frame.message.role === "assistant") {
        accumulator = D.seed(frame.message);
        update({ liveText: textOf(accumulator) });
      } else if (frame.type === "message_update" && frame.message.role === "assistant") {
        if (D.applyEvent(accumulator, frame.assistantMessageEvent)) update({ liveText: textOf(accumulator) });
      } else if (frame.type === "message_end" && frame.message.role === "assistant") {
        const text = textOf(D.seed(frame.message));
        update({ messages: text ? [...state.messages, { id: `assistant-${++serial}`, role: "assistant", text }] : state.messages,
          liveText: null });
        accumulator = [];
        // A message is not a prompt: only its correlated prompt_result ends work.
      }
    }
    function lineReceived(event) {
      if (disposed) return;
      const { seq, text } = event.payload;
      if (seq <= lastSeq) return;
      lastSeq = seq;
      try { frameReceived(JSON.parse(text)); }
      catch (error) { rejectPending(error); fail(new Error(`SideChat protocol error: ${detail(error)}`), true); }
    }
    async function attach(name, callback) {
      const unlisten = await listen(name, callback);
      if (disposed) unlisten();
      else unlisteners.push(unlisten);
    }
    function detach() { for (const unlisten of unlisteners.splice(0)) unlisten(); }
    /** The levels omp lists for the current model; `[]` when it cannot say. */
    async function loadLevels() {
      if (disposed || !started) return [];
      try {
        const data = await request("get_available_thinking_levels");
        levels = Array.isArray(data?.levels) ? data.levels.filter(l => typeof l === "string") : [];
      } catch { levels = []; }
      publishLevel();
      return levels;
    }
    /** Sends the pane's pick when omp lists it for this model; never a level omp did not list. */
    async function applyLevel() {
      if (!desiredLevel || desiredLevel === rawLevel || !levels.includes(desiredLevel)) return;
      await request("set_thinking_level", { level: desiredLevel });
    }
    /** After a model switch: the list follows the model, the pick is re-applied where it still exists. */
    function refreshLevels() {
      loadLevels().then(applyLevel).catch(() => {});
    }
    async function applyModel() {
      const key = desiredModel || defaultModel;
      if (key === appliedModel) return;
      const slash = key.indexOf("/");
      if (slash < 1 || slash === key.length - 1) throw new CommandError(`Invalid SideChat model: ${key}`);
      await request("set_model", { provider: key.slice(0, slash), modelId: key.slice(slash + 1) });
      appliedModel = key;
      update({ model: key });
      refreshLevels();
      // A setting changed during the acknowledgment: do not send using the old model.
      if ((desiredModel || defaultModel) !== key) await applyModel();
    }
    function start() {
      if (startTask) return startTask;
      update({ phase: "starting", error: null });
      startTask = (async () => {
        try {
          await attach(`agent://line/${sessionId}`, lineReceived);
          if (disposed) return;
          await attach(`agent://exit/${sessionId}`, event => {
            if (disposed) return;
            const error = new Error(`SideChat process exited${event.payload ? `: ${event.payload}` : ""}`);
            rejectPending(error);
            fail(error, true);
          });
          if (disposed) return;
          ready = deferred("SideChat startup handshake");
          const readyOutcome = ready.promise.then(() => null, error => error);
          started = true;
          await invoke(startCommand, { sessionId, profile, ...startArgs });
          const startupError = await readyOutcome;
          if (startupError) throw startupError;
          if (disposed) return;
          const snapshot = await request("get_state");
          defaultModel = `${snapshot.model.provider}/${snapshot.model.id}`;
          rawLevel = snapshot.thinkingLevel ?? "off";
          await applyModel();
          if (!disposed && state.phase !== "failed") update({ phase: "ready" });
        } catch (error) {
          if (!disposed) {
            rejectPending(error);
            fail(error, true);
            detach();
            if (started) {
              try { await invoke("close_tab", { sessionId }); started = false; }
              catch (cleanupError) {
                update({ error: `${detail(error)}; cleanup failed: ${detail(cleanupError)}` });
              }
            }
          }
        }
      })();
      return startTask;
    }
    async function send(text) {
      if (disposed || !["ready", "error"].includes(state.phase) || !started) return false;
      const id = `${sessionId}-prompt-${++serial}`;
      update({ phase: "sending", error: null });
      try {
        await applyModel();
        if (disposed) return false;
        await applyLevel().catch(() => {});
        if (disposed) return false;
        activePrompt = id;
        accumulator = [];
        update({ phase: "generating", liveText: "", messages: [...state.messages, { id, role: "user", text }] });
        const ack = await request("prompt", { message: text }, id);
        if (ack?.agentInvoked === false && activePrompt === id) finishPrompt({ id, status: "success" });
        return true;
      } catch (error) {
        if (!disposed && (activePrompt === id || state.phase === "sending")) fail(error, !(error instanceof CommandError));
        return false;
      }
    }
    async function stop() {
      if (disposed || !activePrompt || state.phase === "stopping") return;
      update({ phase: "stopping" });
      try {
        await request("abort");
        if (!disposed && state.phase !== "failed") {
          activePrompt = null;
          const text = textOf(accumulator);
          update({ phase: "ready", liveText: null,
            messages: text ? [...state.messages, { id: `stopped-${++serial}`, role: "assistant", text }] : state.messages });
          accumulator = [];
        }
      } catch (error) { if (!disposed) fail(error, true); }
    }
    async function clear() {
      if (disposed || !started || !["ready", "error"].includes(state.phase)) return;
      update({ phase: "clearing", error: null });
      try {
        const result = await request("new_session");
        if (result.cancelled) throw new CommandError("SideChat conversation reset was cancelled");
        appliedModel = null;
        accumulator = [];
        if (!disposed && state.phase !== "failed") update({ phase: "ready", messages: [], liveText: null });
      } catch (error) { if (!disposed) fail(error, !(error instanceof CommandError)); }
    }
    function dispose() {
      if (disposeTask) return disposeTask;
      disposed = true;
      rejectPending(new Error("SideChat has been closed"));
      detach();
      disposeTask = (async () => {
        // Await an in-flight spawn before closing; otherwise it can outlive close_tab.
        await startTask;
        if (started) { await invoke("close_tab", { sessionId }); started = false; }
      })();
      return disposeTask;
    }
    /** The levels of the current model, `off` first; `[]` for a model without thinking or before the worker is ready. */
    function thinkingLevels() { return loadLevels(); }
    /** The pane's own level pick. It is sent now when the worker is idle, else with the next message;
     *  resolves false for a level the model does not list (nothing is sent). */
    async function setThinkingLevel(level) {
      if (disposed || !started) return false;
      if (!levels.includes(level)) await loadLevels();
      if (!levels.includes(level)) return false;
      desiredLevel = level;
      if (["ready", "error"].includes(state.phase)) {
        try { await applyLevel(); } catch { return false; }
      }
      return true;
    }
    return { start, send, stop, clear, dispose, thinkingLevels, setThinkingLevel,
      setModel(value) { desiredModel = value; }, getState() { return state; } };
  }
  /** The model key a pane asks its worker for: the pane's own pick, else the Settings default, else
   *  "" (the worker keeps the profile's model). */
  const wantedModel = (picked, setting) => picked || setting || "";
  /** A menu entry for `provider/id` when the catalog lacks it (or has not loaded); null for a bad key. */
  function stubModel(key) {
    const slash = String(key || "").indexOf("/");
    return slash > 0 && slash < key.length - 1
      ? { provider: key.slice(0, slash), id: key.slice(slash + 1), name: key.slice(slash + 1) }
      : null;
  }
  window.OMP_SIDECHAT = { create, wantedModel, stubModel };
})();
