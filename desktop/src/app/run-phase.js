/* app/run-phase.js — what the agent is doing while a turn runs, as one word
   for the status line under the conversation: Loading (nothing arrived yet),
   Reasoning (a thinking block is streaming), Writing (the reply is streaming),
   or Running <tool>. Pure: `phase(messages, streaming)` → `{ key, label }` or
   null when idle. Regression: test-run-phase.mjs. */
(function () {
  function phase(messages, streaming) {
    if (!streaming) return null;
    const list = Array.isArray(messages) ? messages : [];
    const last = list[list.length - 1];
    if (!last || last.kind === "user") return { key: "loading", label: "Loading" };
    if (last.kind === "tool" && last.status === "running") {
      return { key: "tool", label: `Running ${last.tool || "tool"}` };
    }
    if (last.kind === "assistant" && last.streaming) {
      const hasText = Array.isArray(last.blocks) && last.blocks.some(b => b && typeof b.text === "string" && b.text.trim() !== "");
      if (hasText) return { key: "writing", label: "Writing" };
      if (last.thought) return { key: "reasoning", label: "Reasoning" };
      return { key: "loading", label: "Loading" };
    }
    return { key: "working", label: "Working" };
  }
  window.OMP_RUN_PHASE = { phase };
})();
