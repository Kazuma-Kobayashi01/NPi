// What the side panel shows about a conversation's size and cost, pure.
// Exposes `window.OMP_USAGE`; an IIFE (AGENTS.md "IIFE rule"), dependency-free.
//
//  - contextBreakdown(rpcState, contextMessages): the context window split
//    into current messages / previous compaction / system prompt / tools /
//    free. The reported total includes all of them. Text estimates are
//    reconciled to that total; active messages must come from get_messages
//    without fullHistory, not the transcript displayed in chat.
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

  function messageTextTokens(message) {
    if (message.excludeFromContext) return 0;
    let tokens = estimateTokens(message.summary);
    if (message.role === "bashExecution") {
      tokens += estimateTokens(message.command) + estimateTokens(message.output);
    } else if (message.role === "pythonExecution") {
      tokens += estimateTokens(message.code) + estimateTokens(message.output);
    } else if (message.role === "fileMention") {
      for (const file of message.files) tokens += estimateTokens(file.content);
    }
    const content = message.blocks ?? message.content;
    if (typeof content === "string") return tokens + estimateTokens(content);
    for (const block of content ?? []) {
      if (block.type === "text") tokens += estimateTokens(block.text);
      else if (block.type === "thinking") tokens += estimateTokens(block.thinking);
      else if (block.type === "toolCall") {
        tokens += estimateTokens(block.name) + estimateTokens(JSON.stringify(block.arguments));
      }
    }
    // Images and native provider payloads have no token size in this RPC.
    // Never price their base64/ciphertext as if it were summary text.
    return tokens;
  }

  function contextBreakdown(rpcState, contextMessages) {
    const cu = rpcState?.contextUsage;
    if (!cu || !(cu.contextWindow > 0)) return null;
    const total = cu.contextWindow;
    const used = Math.max(0, Math.min(cu.tokens ?? 0, total));
    // The prompt's own sections: skills and rules are cut out of "System prompt".
    const promptText = (rpcState.systemPrompt ?? []).join("\n");
    const take = re => promptText.match(re)?.[0] ?? "";
    const skillsText = take(/<skills>[\s\S]*?<\/skills>/);
    const rulesText = `${take(/<generic-rules>[\s\S]*?<\/generic-rules>/)}\n${take(/<domain-rules>[\s\S]*?<\/domain-rules>/)}`.trim();
    const names = text => [...text.matchAll(/^- ([^:\n(]+)/gm)].map(m => m[1].trim());
    let skills = estimateTokens(skillsText);
    let rules = estimateTokens(rulesText);
    let system = Math.max(0, estimateTokens(promptText) - skills - rules);
    // omp names an MCP server's tool `mcp__<server>_<tool>`.
    const allTools = rpcState.dumpTools ?? [];
    const isMcp = t => typeof t?.name === "string" && t.name.startsWith("mcp__");
    const mcpList = allTools.filter(isMcp);
    const servers = new Map();
    for (const t of mcpList) {
      const server = t.name.slice(5).split("_")[0] || "mcp";
      servers.set(server, (servers.get(server) ?? 0) + 1);
    }
    let tools = estimateTokens(JSON.stringify(allTools.filter(t => !isMcp(t))));
    let mcp = mcpList.length ? estimateTokens(JSON.stringify(mcpList)) : 0;
    let summary = 0, current = 0, hasSummary = false;
    for (const message of contextMessages ?? []) {
      if (message.role === "compactionSummary") {
        hasSummary = true;
        summary += messageTextTokens(message);
      } else {
        current += messageTextTokens(message);
      }
    }
    // Fixed prompt estimates must not consume the whole budget before known
    // messages are considered. Scale all text estimates down together, then
    // leave unclassified provider overhead in the residual Messages category.
    const estimated = system + tools + mcp + skills + rules + summary + current;
    const scale = estimated > used ? used / estimated : 1;
    system = Math.floor(system * scale);
    tools = Math.floor(tools * scale);
    mcp = Math.floor(mcp * scale);
    skills = Math.floor(skills * scale);
    rules = Math.floor(rules * scale);
    summary = Math.floor(summary * scale);
    const messages = used - system - tools - mcp - skills - rules - summary;
    return {
      total, used, pct: (used / total) * 100,
      parts: [
        { key: "messages", label: "Messages", tokens: messages },
        ...(hasSummary ? [{ key: "compacted", label: "Previous compaction", tokens: summary }] : []),
        { key: "system", label: "System prompt", tokens: system },
        { key: "tools", label: "Tools", tokens: tools, count: allTools.length - mcpList.length },
        ...(mcpList.length ? [{ key: "mcp", label: "MCP tools", tokens: mcp, count: mcpList.length, detail: [...servers].map(([n, c]) => `${n} · ${c}`) }] : []),
        ...(skillsText ? [{ key: "skills", label: "Skills", tokens: skills, count: names(skillsText).length, detail: names(skillsText) }] : []),
        ...(rulesText ? [{ key: "rules", label: "Rules", tokens: rules, count: names(rulesText).length, detail: names(rulesText) }] : []),
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

  /** Adds streamed output `delta` to the running estimate `prev` ({ tokens, cost }), priced at the
   *  model's output rate (`price.output`, USD per million tokens; no price, no cost). */
  function liveSpend(prev, delta, price) {
    const tokens = (prev?.tokens ?? 0) + estimateTokens(delta);
    const rate = Number(price?.output);
    return { tokens, cost: rate > 0 ? (tokens * rate) / 1e6 : null };
  }

  /** Running spend at a glance: four decimals under $1 ($0.0123), two above; $0.0000 when unknown. */
  function formatCostPrecise(cost) {
    if (!(cost > 0)) return "$0.0000";
    if (cost < 0.0001) return "<$0.0001";
    return "$" + (cost < 1 ? cost.toFixed(4) : cost.toFixed(2));
  }

  /** Percent of a limit already used (0-100), or null when unknown. */
  function usedPct(limit) {
    const left = leftPct(limit);
    return left === null ? null : 100 - left;
  }

  window.OMP_USAGE = { estimateTokens, contextBreakdown, quotaFor, untilReset, leftPct, usedPct, tokenRows, formatCost, formatCostPrecise, liveSpend };
})();
