// Computer Use requests also reach background tabs, whose ordinary listeners
// are detached. Subscribe before sending and correlate only this request's id.
//
// A tab without a live process is not a failure: its next process reads the
// profile setting itself. Those rejections carry `absent`, so callers can skip
// them instead of reporting a feature error.
(function () {
  let nextId = 0;

  /** One line of a child's exit payload; a startup death arrives as a stderr tail. */
  function firstLine(text) {
    const line = String(text ?? "").replace(/^Error:\s*/, "")
      .split("\n").map(part => part.trim()).find(Boolean);
    return (line ?? "Agent process exited").slice(0, 200);
  }

  function create(tauri) {
    async function request(sessionId, command) {
      const id = `computer-${++nextId}`;
      const off = [];
      let timer = null;
      let settle;
      let fail;
      const response = new Promise((resolve, reject) => { settle = resolve; fail = reject; });
      try {
        off.push(await tauri.event.listen(`agent://line/${sessionId}`, event => {
          let frame;
          try { frame = JSON.parse(event.payload.text); } catch { return; }
          if (frame?.type !== "response" || frame.id !== id) return;
          if (frame.success) settle(frame.data);
          else fail(new Error(frame.error ?? `Computer Use ${command.type} failed`));
        }));
        off.push(await tauri.event.listen(`agent://exit/${sessionId}`, event => {
          const error = new Error(firstLine(event.payload));
          error.absent = true;
          fail(error);
        }));
        timer = setTimeout(() => fail(new Error(`Computer Use ${command.type} timed out`)), 15000);
        try {
          await tauri.core.invoke("send_command", {
            sessionId,
            json: JSON.stringify({ ...command, id }),
          });
        } catch (reason) {
          const error = new Error(firstLine(String(reason)));
          error.absent = true;
          fail(error);
        }
        return await response;
      } finally {
        clearTimeout(timer);
        for (const stop of off) stop();
      }
    }
    return { request };
  }

  window.OMP_COMPUTER_RPC = { create };
})();
