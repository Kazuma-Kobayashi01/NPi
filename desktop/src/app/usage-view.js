// What the side panel shows about a conversation's size and cost, pure.
// Exposes `window.OMP_USAGE`; an IIFE (AGENTS.md "IIFE rule"), dependency-free.
//
//  - contextBreakdown(rpcState): the context window split into messages /
//    system prompt / tools / free. omp reports the total in use
//    (`contextUsage`); the system prompt and the tool definitions it also
//    sends (`systemPrompt`, `dumpTools`) are sized by an estimate, and the
//    rest is the conversation. The parts are estimates and say so.
//  - quotaFor(rows, provider): the subscription limits of the provider in
//    use, and the other signed-in subscriptions (rows: `usage_limits`).
//  - tokenRows(stats): input / output / cache read / cache write from
//    `get_session_stats`.
(function () {
  const CJK_FROM = 0x2e80;

  /** About 4 characters per token for Latin text, about 1 for CJK. */
  function estimateTokens(text) {
    let n = 0;
    for (const ch of String(text ?? "")) n += ch.codePointAt(0) >= CJK_FROM ? 1 : 0.25;
    return Math.ceil(n);
  }

  function contextBreakdown(rpcState) {
    const cu = rpcState?.contextUsage;
    if (!cu || !(cu.contextWindow > 0)) return null;
    const total = cu.contextWindow;
    const used = Math.max(0, Math.min(cu.tokens ?? 0, total));
    const system = Math.min(used, estimateTokens((rpcState.systemPrompt ?? []).join("\n")));
    const tools = Math.min(used - system, estimateTokens(JSON.stringify(rpcState.dumpTools ?? [])));
    const messages = used - system - tools;
    return {
      total, used, pct: (used / total) * 100,
      parts: [
        { key: "messages", label: "Messages", tokens: messages },
        { key: "system", label: "System prompt", tokens: system },
        { key: "tools", label: "Tools", tokens: tools },
        { key: "free", label: "Free space", tokens: total - used },
      ],
    };
  }

  /** `{ mine, others }`: the report of `provider`, and the other providers'. */
  function quotaFor(rows, provider) {
    const list = Array.isArray(rows) ? rows : [];
    return {
      mine: list.find(r => r.provider === provider) ?? null,
      others: list.filter(r => r.provider !== provider),
    };
  }

  /** `3h 20m`, `2d 4h`, `45m`; "now" at or past zero. */
  function untilReset(resetsAtMs, nowMs) {
    const ms = (resetsAtMs ?? 0) - nowMs;
    if (!(ms > 60_000)) return "now";
    const min = Math.round(ms / 60_000);
    const d = Math.floor(min / 1440), h = Math.floor((min % 1440) / 60), m = min % 60;
    if (d > 0) return `${d}d ${h}h`;
    if (h > 0) return `${h}h ${m}m`;
    return `${m}m`;
  }

  /** Percent of a limit still free, 0–100, rounded; null when unknown. */
  function leftPct(limit) {
    const f = limit?.remainingFraction;
    return typeof f === "number" ? Math.max(0, Math.min(100, Math.round(f * 100))) : null;
  }

  /** The rows of the token table; Reasoning only when there is some. */
  function tokenRows(stats) {
    const t = stats?.tokens;
    if (!t) return [];
    const rows = [
      { key: "input", label: "Input", tokens: t.input ?? 0 },
      { key: "output", label: "Output", tokens: t.output ?? 0 },
      { key: "cacheRead", label: "Cache read", tokens: t.cacheRead ?? 0 },
      { key: "cacheWrite", label: "Cache write", tokens: t.cacheWrite ?? 0 },
    ];
    if ((t.reasoning ?? 0) > 0) rows.push({ key: "reasoning", label: "Reasoning", tokens: t.reasoning });
    return rows;
  }

  /** `$0.24`; `<$0.01` for a trace; `$0.00` when unknown. */
  function formatCost(cost) {
    if (!(cost > 0)) return "$0.00";
    return cost < 0.01 ? "<$0.01" : `$${cost.toFixed(2)}`;
  }

  window.OMP_USAGE = { estimateTokens, contextBreakdown, quotaFor, untilReset, leftPct, tokenRows, formatCost };
})();
