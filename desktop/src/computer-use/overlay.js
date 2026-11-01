(function () {
  "use strict";
  const api = window.__TAURI__;
  const label = new URLSearchParams(location.search).get("label") || "";
  const stop = document.getElementById("stop");
  const error = document.getElementById("error");
  let current = null;

  async function failed(reason) {
    const message = reason instanceof Error ? reason.message : String(reason);
    error.textContent = message;
    await api?.event.emit("computer-use://error", { sessionId: current?.sessionId || "", error: message }).catch(() => {});
  }

  // Earliest possible probe: says whether this page runs at all, and with what.
  api?.event?.emit("computer-use://boot", { label, href: location.href })?.catch(() => {});

  if (!api?.event) {
    document.body.textContent = "Computer Use indicator unavailable";
    return;
  }

  stop.addEventListener("click", async () => {
    stop.disabled = true;
    try {
      await api.core.invoke("computer_use_stop", { sessionId: current.sessionId });
    } catch (reason) {
      await failed(reason);
    } finally {
      stop.disabled = false;
    }
  });

  api.event.listen("computer-use://display", async ({ payload }) => {
    current = payload;
    try {
      // Nothing is drawn before a real operation arrives, so a page that loads
      // early never flashes an empty prompt at the user.
      document.body.dataset.role = label.replace("computer-use-", "");
      document.documentElement.style.setProperty("--agent-accent", payload.accent);
      const target = payload.activity.target;
      const detail = [target?.app || target?.title, payload.activity.action].filter(Boolean).join(" · ");
      document.getElementById("detail").textContent = detail;
      error.textContent = "";
      stop.disabled = false;
      const revision = payload.revision;
      // A loaded page is not yet a painted indicator. Two compositor turns
      // ensure the geometry and real-operation content have been presented.
      requestAnimationFrame(() => requestAnimationFrame(() => {
        if (current.revision !== revision) return;
        api.event.emit("computer-use://painted", {
          label, sessionId: payload.sessionId, activityId: payload.activity.id, revision,
        }).catch(failed);
      }));
    } catch (reason) {
      await failed(reason);
    }
  }).then(() => api.event.emit("computer-use://ready", label)).catch(failed);
})();
