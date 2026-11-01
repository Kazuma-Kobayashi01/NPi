/* Display-only grouping; the transcript and original indices stay unchanged. */
(function () {
  function groupMessages(messages, { hidden = () => false } = {}) {
    const rows = [];
    let items = [];
    function flush() {
      if (items.length === 0) return;
      rows.push({
        type: "tool_group",
        key: `tool-group:${items[0].msg._id}`,
        firstIdx: items[0].idx,
        lastIdx: items[items.length - 1].idx,
        indices: items.map(item => item.idx),
        tools: items.map(item => item.msg),
        items,
      });
      items = [];
    }
    for (let idx = 0; idx < messages.length; idx++) {
      const msg = messages[idx];
      if (hidden(msg)) continue;
      if (msg.kind === "tool" && msg.status === "ok" && !msg.isError) {
        items.push({ msg, idx });
      } else {
        flush();
        rows.push({ type: "message", msg, idx });
      }
    }
    flush();
    return rows;
  }

  function isGroupCollapsed(group, { foldModes = { tool: "folded" }, collapseOverrides = {} } = {}) {
    if (group.tools.length === 1) return false;
    if (Object.hasOwn(collapseOverrides, group.key)) return collapseOverrides[group.key];
    if (group.tools.some(tool => collapseOverrides[tool._id] === false)) return false;
    return foldModes.tool !== "open";
  }

  function groupOverrides(messages, collapsed, options) {
    const overrides = {};
    for (const row of groupMessages(messages, options)) {
      if (row.type === "tool_group" && row.tools.length > 1) overrides[row.key] = collapsed;
    }
    return overrides;
  }

  function groupSummary(tools) {
    const summary = tools.slice(0, 3).map(tool => tool.title || tool.tool).join(" · ");
    return tools.length > 3 ? `${summary} · +${tools.length - 3}` : summary;
  }

  window.OMP_TOOL_GROUPS = { groupMessages, isGroupCollapsed, groupOverrides, groupSummary };
})();
