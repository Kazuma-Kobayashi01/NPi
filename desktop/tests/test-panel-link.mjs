#!/usr/bin/env node
// Regression script for src/app/panel-link.js — the main window and the
// secondary windows that show its tab. Two vm contexts, one per window, share
// a fake Tauri event bus: snapshots reach a panel that says hello, calls run
// in the main window only when listed for the panel's kind.
// Run: node tests/test-panel-link.mjs

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import vm from "node:vm";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const source = readFileSync(join(root, "src/app/panel-link.js"), "utf8");

const listeners = new Map();   // event → Set(handler)
const emitted = [];
const bus = {
  async listen(name, fn) {
    if (!listeners.has(name)) listeners.set(name, new Set());
    listeners.get(name).add(fn);
    return () => listeners.get(name)?.delete(fn);
  },
  async emit(name, payload) {
    emitted.push(name);
    // Serialised like an IPC hop: functions and identity do not cross.
    const copy = payload === undefined ? undefined : JSON.parse(JSON.stringify(payload));
    for (const fn of [...(listeners.get(name) ?? [])]) queueMicrotask(() => fn({ event: name, payload: copy }));
  },
};

function windowContext(panel, bridge) {
  const ctx = vm.createContext({ console, setTimeout, queueMicrotask, Promise, JSON, Object, Date, Map, Set, Error, String, Array });
  ctx.window = ctx;
  ctx.__TAURI__ = { event: bus };
  ctx.OMP_PANEL = panel;
  ctx.OMP_BRIDGE = bridge;
  ctx.addEventListener = () => {};
  ctx.removeEventListener = () => {};
  vm.runInContext(source, ctx, { filename: "panel-link.js" });
  return ctx;
}
const tick = () => new Promise(r => setTimeout(r, 5));

let passed = 0;
async function check(name, fn) {
  try { await fn(); passed += 1; } catch (e) { console.error(`FAILED: ${name}`); throw e; }
}

const revoked = [];
const main = windowContext(null, {
  listApprovalRules: async () => [{ tool: "bash", scope: "session" }],
  revokeApprovalRule: async (tool, scope) => { revoked.push(`${tool}/${scope}`); },
  closeSession: async () => { throw new Error("must never run from a panel"); },
});
const rules = windowContext({ kind: "rules" }, null);
const plan = windowContext({ kind: "plan" }, null);

await check("a panel gets the latest snapshot when it says hello, and later ones while open", async () => {
  const link = main.OMP_PANEL_LINK;
  let shown = null;
  link.onPresence(() => {});
  link.publish("plan", { kanban: [1] });
  const stop = plan.OMP_PANEL_LINK.subscribe("plan", s => { shown = s; });
  await tick();
  assert.deepEqual(shown, { kanban: [1] });
  link.publish("plan", { kanban: [1, 2] });
  await tick();
  assert.deepEqual(shown, { kanban: [1, 2] });
  const before = emitted.filter(n => n === "panel-state:plan").length;
  link.publish("plan", { kanban: [1, 2] });
  await tick();
  assert.equal(emitted.filter(n => n === "panel-state:plan").length, before, "an equal snapshot is not sent again");
  stop();
});

await check("presence follows hello and bye", async () => {
  let seen = null;
  main.OMP_PANEL_LINK.onPresence(open => { seen = [...open]; });
  const stop = rules.OMP_PANEL_LINK.subscribe("rules", () => {});
  await tick();
  assert.ok(seen.includes("rules"));
  stop();
  await tick();
  assert.ok(!seen.includes("rules"));
});

await check("a listed call runs on the main window's bridge and returns its value", async () => {
  const bridge = rules.OMP_PANEL_LINK.proxy("rules");
  assert.deepEqual(await bridge.listApprovalRules(), [{ tool: "bash", scope: "session" }]);
  await bridge.revokeApprovalRule("bash", "session");
  assert.deepEqual(revoked, ["bash/session"]);
  assert.equal(bridge.closeSession, undefined, "the proxy has only the kind's names");
});

await check("an action served by the main window runs; an unlisted name is refused", async () => {
  let aborted = 0;
  main.OMP_PANEL_LINK.serve("plan", { abort: () => { aborted += 1; return "ok"; } });
  assert.equal(await plan.OMP_PANEL_LINK.proxy("plan").abort(), "ok");
  assert.equal(aborted, 1);
  await assert.rejects(plan.OMP_PANEL_LINK.call("plan", "closeSession", []), /not allowed/);
  await assert.rejects(plan.OMP_PANEL_LINK.call("settings", "closeSession", []), /not allowed/);
});

console.log(`test-panel-link: ${passed} checks passed`);
