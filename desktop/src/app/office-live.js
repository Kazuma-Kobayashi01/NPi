// app/office-live.js — the Office window's own line to one tab's omp.
//
// An Office window (`index.html?panel=office&project=<session id>`) does not
// go through the main window: it listens to the session's RPC lines itself
// and sends its own commands, so it keeps working while the main window is
// hidden or shows another tab. It only reads what omp pushes about the
// company (`office_state`, and `office_usage`: spend as a delta the window
// adds up itself) and answers to its own requests, whose ids start
// with `office:` — the main window resolves responses by its own ids, so the
// two never take each other's replies.
(function () {
  const PREFIX = "office:";
  const TIMEOUT_MS = 20_000;

  /** Parses one `agent://line` payload (`{seq, text}`); null when it is not ours to read. */
  function readLine(payload) {
    if (!payload || typeof payload.text !== "string") return null;
    // Cheap reject before parsing: streaming deltas are most of the traffic.
    if (!payload.text.includes('"office_') && !payload.text.includes(`"id":"${PREFIX}`)) return null;
    let frame;
    try { frame = JSON.parse(payload.text); } catch (_) { return null; }
    if (frame?.type === "office_state" && frame.state) return { kind: "state", state: frame.state };
    if (frame?.type === "office_usage" && frame.amount) return { kind: "usage", frame };
    if (frame?.type === "response" && typeof frame.id === "string" && frame.id.startsWith(PREFIX)) return { kind: "response", frame };
    return null;
  }

  /**
   * Connects to tab `sessionId`. `onState(state)` gets every company state,
   * `onUsage(frame)` every `office_usage` delta, `onOnline(bool, reason)`
   * whether its omp answers. Returns
   * `{ request(cmd), command(args), refresh(), transcript(file, fromByte), report(runId), close() }`.
   */
  function connect(sessionId, { onState, onUsage, onOnline }) {
    const tauri = window.__TAURI__;
    const pending = new Map();
    const unlisten = [];
    let seq = 0;
    let closed = false;

    function settle(frame) {
      const waiter = pending.get(frame.id);
      if (!waiter) return;
      pending.delete(frame.id);
      clearTimeout(waiter.timer);
      if (frame.success) waiter.resolve(frame.data ?? null);
      else waiter.reject(new Error(frame.error ?? `${frame.command} failed`));
    }

    function request(cmd) {
      if (closed || !tauri) return Promise.reject(new Error("not connected"));
      const id = `${PREFIX}${++seq}`;
      return new Promise((resolve, reject) => {
        const timer = setTimeout(() => { pending.delete(id); reject(new Error("omp did not answer")); }, TIMEOUT_MS);
        pending.set(id, { resolve, reject, timer });
        tauri.core.invoke("send_command", { sessionId, json: JSON.stringify({ ...cmd, id }) }).catch(e => {
          pending.delete(id);
          clearTimeout(timer);
          reject(new Error(String(e?.message ?? e)));
        });
      });
    }

    async function refresh() {
      try {
        const state = await request({ type: "get_office_state" });
        if (closed) return;
        onOnline(true, "");
        onState(state);
      } catch (e) {
        if (!closed) onOnline(false, String(e?.message ?? e));
      }
    }

    /** Runs an `/office` command; resolves to `{ text, failed }` (the state it returns is applied). */
    async function command(args) {
      const data = await request({ type: "office_command", args });
      if (data?.state && !closed) onState(data.state);
      return { text: data?.text ?? "", failed: !!data?.failed };
    }

    /** The Markdown report of run `runId`, or null when none is kept. */
    function report(runId) {
      return request({ type: "get_office_report", runId });
    }

    function transcript(sessionFile, fromByte) {
      return request({ type: "get_subagent_messages", sessionFile, fromByte: fromByte ?? 0 });
    }

    if (tauri) {
      tauri.event.listen(`agent://line/${sessionId}`, ev => {
        const line = readLine(ev.payload);
        if (!line || closed) return;
        if (line.kind === "state") { onOnline(true, ""); onState(line.state); }
        else if (line.kind === "usage") onUsage?.(line.frame);
        else settle(line.frame);
      }).then(un => (closed ? un() : unlisten.push(un)));
      tauri.event.listen(`agent://exit/${sessionId}`, () => { if (!closed) onOnline(false, "exit"); })
        .then(un => (closed ? un() : unlisten.push(un)));
      refresh();
    } else {
      onOnline(false, "no desktop shell");
    }

    return {
      request, command, refresh, transcript, report,
      close() {
        closed = true;
        for (const un of unlisten) { try { un(); } catch (_) {} }
        for (const w of pending.values()) clearTimeout(w.timer);
        pending.clear();
      },
    };
  }

  window.OMP_OFFICE_LIVE = { connect, readLine, PREFIX };
})();
