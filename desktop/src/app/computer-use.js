/* Computer Use keeps profile intent, host permissions and worker readiness
   separate. Nothing here asks macOS for a permission. */
(function () {
  /** One bounded line: a process death arrives as a joined stderr tail. */
  function summarize(error) {
    const line = String(error ?? "").split("\n").map(part => part.trim()).find(Boolean) ?? "";
    return line.length > 240 ? `${line.slice(0, 240)}…` : line;
  }

  function status(config, runtime, error, sessionId, awaiting) {
    if (error) return { kind: "error", label: "Needs attention", detail: summarize(error) };
    if (!config) return { kind: "loading", label: "Reading status…", detail: "Checking this profile and macOS permissions." };
    if (!config.supported) return { kind: "unsupported", label: "Not supported", detail: "Computer Use is currently available on macOS only." };
    // A running conversation can still operate after the profile was turned
    // off: its process applied the change on its own, and a partial sync must
    // never read as safely disabled.
    if (runtime && (runtime.enabled || runtime.running) && !config.enabled) {
      return {
        kind: "error",
        label: "Still active in a conversation",
        detail: runtime.running
          ? "Computer Use is turned off for this profile, but a conversation is still operating your computer. Use Stop, then restart that conversation."
          : "Computer Use is turned off for this profile, but a running conversation still has it enabled. Restart that conversation.",
      };
    }
    if (!config.enabled) return { kind: "disabled", label: "Disabled", detail: "The agent cannot use your computer through this profile." };
    const missing = ["screenRecording", "accessibility"].filter(key => !config.permissions[key]);
    if (missing.length > 0) {
      // The request returns before macOS records an answer, so the status must
      // not claim the grant is still missing while the prompt is on screen.
      return awaiting
        ? { kind: "waiting", label: "Waiting for macOS", detail: `Answer the macOS prompt, or grant access in System Settings, then Recheck.` }
        : { kind: "waiting", label: "Waiting for permissions", detail: "Enabled for this profile. Grant Screen Recording and Accessibility in macOS to continue." };
    }
    if (!sessionId) {
      return { kind: "waiting", label: "Enabled · no conversation open", detail: "Open a conversation with this profile to check the agent process." };
    }
    if (!runtime?.enabled) {
      return { kind: "waiting", label: "Enabled · awaiting agent process", detail: "This conversation allows no eval tools, so it cannot use Computer Use. Open a coding conversation with this profile." };
    }
    if (!runtime.capabilities) {
      return { kind: "waiting", label: "Enabled · awaiting agent process", detail: "Waiting for the agent process to report its screen access." };
    }
    const cap = runtime.capabilities;
    if (!cap.capture || !cap.input || !cap.ax || cap.capturePermission !== "granted" || cap.inputPermission !== "granted" || cap.axPermission !== "granted") {
      return { kind: "waiting", label: "Agent permissions not ready", detail: "The agent process cannot yet capture the screen, send input or read Accessibility. Recheck after granting access. macOS may require you to quit and reopen NPi Desktop." };
    }
    return { kind: "ready", label: runtime.running ? "Operating" : "Ready", detail: runtime.running ? "The agent is using your computer. Stop ends only the current computer operation." : "The agent process has confirmed screen capture, input and Accessibility access." };
  }

  const missingRequired = permissions => ["screenRecording", "accessibility"].filter(key => !permissions[key]);
  window.OMP_COMPUTER_USE = { status, missingRequired };
})();
