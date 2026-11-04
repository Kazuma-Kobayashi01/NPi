// app/write-pipeline.js — what the Write window shows of a paper's pipeline: from `write_status`
// (src-tauri/src/writing/commands.rs) to the three stages of the progress bar and the next step.
(function () {
  /** The three stages: `{ id, label, percent, state, detail }`; `state` is idle, active, done or attention. */
  function stages(status) {
    const sections = status?.sections ?? [];
    const run = status?.run ?? null;
    const total = sections.length;
    const words = sections.reduce((n, s) => n + s.words, 0);
    const fresh = sections.filter(s => s.state === "fresh").length;
    const warned = sections.some(s => s.warnings.length > 0) || (status?.missing ?? []).length > 0;
    const compile = run
      ? { percent: run.total > 0 ? Math.round((run.done / run.total) * 100) : 0, state: "active", done: run.done, total: run.total, current: run.current }
      : { percent: total > 0 ? Math.round((fresh / total) * 100) : 0, state: fresh === total && total > 0 ? (warned ? "attention" : "done") : fresh > 0 ? "attention" : "idle", done: fresh, total, current: "" };
    const link = status?.link ?? "none";
    return [
      { id: "draft", label: "Draft", percent: words > 0 ? 100 : 0, state: words > 0 ? "done" : "idle", total, words },
      { id: "compile", label: "Compile", ...compile },
      { id: "link", label: "Link", percent: link === "fresh" ? 100 : link === "stale" ? 50 : 0, state: link === "fresh" ? "done" : link === "stale" ? "attention" : "idle", link },
    ];
  }

  /** What the user should do next: `init`, `write`, `compile`, `link` or `done`. */
  function nextStep(status) {
    if (!status?.ready) return "init";
    const sections = status.sections ?? [];
    if (sections.every(s => s.words <= 1)) return "write";
    if (sections.some(s => s.state !== "fresh")) return "compile";
    return status.link === "fresh" ? "done" : "link";
  }

  window.OMP_WRITE = { stages, nextStep };
})();
