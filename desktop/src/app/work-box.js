/* app/work-box.js — display-only grouping of a turn's "work": the thinking
   and the tool calls between two visible replies fold into ONE disclosure
   (the "box"), like Claude's "Thought · used 3 tools" line. The transcript
   and the original indices stay unchanged; `chat-collapse.js` `workMode`
   decides whether the chat uses boxes or today's separate rows.

   What stays outside a box, so it is never hidden: a reply with text, a
   failed tool call, approvals, asks, rendered UI, retries, goals, jobs,
   compaction and the user's own messages. A box is open only when the user
   opened it (`overrides[key] === false`). Regression: test-work-box.mjs. */
(function () {
  /** Text that says something: a letter, digit or emoji, a code fence, a table or a
   *  list. A lone "." or "…" some models put between tool calls is filler, not a reply. */
  const hasSubstance = text => /[\p{L}\p{N}\p{Extended_Pictographic}]|```|^\s*(?:[|#>*+-]|\d+[.)])\s/mu.test(text ?? "");
  const nonBlank = b => (b.type === "text" ? hasSubstance(b.text) : true);
  /** A reply that shows something of its own besides thinking. */
  const hasVisible = m => !!m.failure || (m.blocks ?? []).some(nonBlank);
  const hasThought = m => !!m.thought?.trim();
  /** A tool call that folds into the box: any but a failed one. */
  const isWork = m => m.kind === "tool" && !m.isError;
  const thinkingActive = m => (window.OMP_COLLAPSE?.thinkingActive(m) ?? !!m.streaming);

  /** Rows for the chat: `{ type: "work", key, items, firstIdx, lastIdx, indices }`
   *  for a box (`items`: `{ kind: "thought" | "tool", msg, idx }`) and
   *  `{ type: "message", msg, idx, thoughtInBox }` for everything else. A reply
   *  with thinking and text puts its thinking in the box just above it
   *  (`thoughtInBox`: the bubble then leaves it out); a reply with neither
   *  shows nothing and is left out. */
  function groupWork(messages, { hidden = () => false } = {}) {
    const rows = [];
    let items = [];
    const flush = () => {
      if (items.length === 0) return;
      rows.push({
        type: "work",
        key: `work:${items[0].msg._id}`,
        firstIdx: items[0].idx,
        lastIdx: items[items.length - 1].idx,
        indices: items.map(item => item.idx),
        items,
      });
      items = [];
    };
    for (let idx = 0; idx < messages.length; idx++) {
      const msg = messages[idx];
      if (hidden(msg)) continue;
      if (isWork(msg)) { items.push({ kind: "tool", msg, idx }); continue; }
      if (msg.kind === "assistant") {
        const thought = hasThought(msg);
        if (thought) items.push({ kind: "thought", msg, idx });
        if (hasVisible(msg)) { flush(); rows.push({ type: "message", msg, idx, thoughtInBox: thought }); }
        continue;
      }
      flush();
      rows.push({ type: "message", msg, idx, thoughtInBox: false });
    }
    flush();
    return rows;
  }

  /** What a box's one line says. `tools`/`thoughts`: counts; `live`: something
   *  in it is still running; `current`: the running tool's one-line title, or
   *  "" while the model thinks. `label` is the finished line, `liveLabel` the
   *  running one (a quiet verb or the current tool), shown with a spinner. */
  // Strings come from the UI language (app/i18n.js); a bare context (tests) gets the ids.
  const T = (key, vars) => (window.OMP_I18N ? window.OMP_I18N.t(key, vars) : key);

  function summarize(items) {
    const tools = items.filter(item => item.kind === "tool");
    const thoughts = items.filter(item => item.kind === "thought");
    const running = tools.findLast(item => item.msg.status === "running");
    const thinking = thoughts.some(item => thinkingActive(item.msg));
    const live = thinking || !!running || items.some(item => item.msg.streaming && item.kind === "thought");
    const parts = [];
    if (thoughts.length) parts.push(T("work.thought"));
    if (tools.length) parts.push(T("work.tools", { n: tools.length }));
    const current = running ? (running.msg.title || running.msg.tool || "") : "";
    return {
      tools: tools.length, thoughts: thoughts.length, live, current,
      label: parts.join(" · "),
      liveLabel: thinking && !running ? T("work.thinking") : (current || T("work.working")),
    };
  }

  /** Whether a box is folded: it is, unless the user opened it. */
  const isCollapsed = (key, overrides = {}) => (Object.hasOwn(overrides, key) ? overrides[key] : true);

  /** `key → collapsed` for every box (Collapse all / Expand all). */
  function groupOverrides(messages, collapsed, options) {
    const out = {};
    for (const row of groupWork(messages, options)) if (row.type === "work") out[row.key] = collapsed;
    return out;
  }

  window.OMP_WORK_BOX = { groupWork, summarize, isCollapsed, groupOverrides, hasVisible, hasThought, hasSubstance };
})();
