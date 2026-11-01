#!/usr/bin/env node
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
const win = {};
new Function("window", readFileSync(fileURLToPath(new URL("../src/app/tool-groups.js", import.meta.url)), "utf8"))(win);
const G = win.OMP_TOOL_GROUPS;
const tool = (id, extra = {}) => ({ _id: id, kind: "tool", tool: "read", status: "ok", ...extra });
let passed = 0;
function check(name, run) {
  try { run(); passed++; } catch (error) { console.error(`FAILED: ${name}`); throw error; }
}

check("grouping retains every record and original execution order without mutating messages", () => {
  const messages = [tool(1), tool(2), { _id: 3, kind: "assistant", blocks: [{ type: "text", text: "Result" }] }, tool(4)];
  const before = JSON.stringify(messages);
  const rows = G.groupMessages(messages);
  assert.deepEqual(rows.flatMap(row => row.type === "tool_group" ? row.tools : [row.msg]), messages);
  assert.deepEqual(rows.flatMap(row => row.type === "tool_group" ? row.indices : [row.idx]), [0, 1, 2, 3]);
  assert.equal(rows[0].tools.length, 2);
  assert.equal(rows[2].tools.length, 1);
  assert.equal(JSON.stringify(messages), before);
});

check("running, failure and confirmation boundaries are never hidden inside completed groups", () => {
  for (const exceptional of [tool(3, { status: "running" }), tool(3, { isError: true }),
    tool(3, { status: "pending" }), tool(3, { status: "waiting" }),
    { _id: 3, kind: "ask" }, { _id: 3, kind: "approval" }]) {
    const rows = G.groupMessages([tool(1), tool(2), exceptional, tool(4), tool(5)]);
    assert.equal(rows.length, 3);
    assert.equal(rows[1].msg, exceptional);
    assert.deepEqual(rows.filter(row => row.type === "tool_group").map(row => row.tools.length), [2, 2]);
  }
});

check("settling and appending tools preserve the established group identity", () => {
  const one = G.groupMessages([tool(1)])[0];
  const active = G.groupMessages([tool(1), tool(2), tool(3, { status: "running" })]);
  const settled = G.groupMessages([tool(1), tool(2), tool(3)])[0];
  assert.equal(one.key, active[0].key);
  assert.equal(active[0].key, settled.key);
  assert.equal(settled.tools.length, 3);
  assert.notEqual(settled.key, String(settled.tools[0]._id));
});

check("group and first-child manual states are independent and the latest group choice wins", () => {
  const group = G.groupMessages([tool(1), tool(2)])[0];
  assert.equal(G.isGroupCollapsed(group), true);
  assert.equal(G.isGroupCollapsed(group, { foldModes: { tool: "open" } }), false);
  assert.equal(G.isGroupCollapsed(group, { collapseOverrides: { 1: true, 2: false } }), false);
  assert.equal(G.isGroupCollapsed(group, { collapseOverrides: { [group.key]: true, 1: false } }), true);
  assert.equal(G.isGroupCollapsed(group, { collapseOverrides: { [group.key]: false, 1: true, 2: true } }), false);
});

check("a manually expanded single call stays accessible when later calls form and extend its group", () => {
  const overrides = { 1: false };
  for (const calls of [[tool(1)], [tool(1), tool(2)], [tool(1), tool(2), tool(3)]]) {
    assert.equal(G.isGroupCollapsed(G.groupMessages(calls)[0], { collapseOverrides: overrides }), false);
  }
});

check("hidden retry rows do not split visually adjacent records or alter their source indices", () => {
  const messages = [tool(1), { _id: 2, kind: "retry", hidden: true }, tool(3)];
  const group = G.groupMessages(messages, { hidden: msg => !!msg.hidden })[0];
  assert.deepEqual(group.indices, [0, 2]);
  assert.deepEqual(group.tools, [messages[0], messages[2]]);
  assert.equal(messages.length, 3);
});
console.log(`tool-groups: ${passed} checks passed`);
