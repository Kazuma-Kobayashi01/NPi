#!/usr/bin/env node
// Regression script for src/app/plan-edit.js — the staged edits of the inline
// plan block and the one message that sends them to the agent.
// Run: node tests/test-plan-edit.mjs

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const src  = readFileSync(join(root, "src/app/plan-edit.js"), "utf8");
const win  = {};
// eslint-disable-next-line no-new-func
new Function("window", src)(win);
const P = win.OMP_PLAN_EDIT;

let passed = 0;
function check(label, fn) {
  try {
    fn();
  } catch (err) {
    console.error(`FAILED: ${label}`);
    throw err;
  }
  passed++;
}

const plan = () => ({
  type: "plan", title: "Plan",
  phases: [
    { id: "Scout", label: "Scout", tasks: [
      { id: "Scout-0", text: "Read the router", status: "done" },
      { id: "Scout-1", text: 'Find "legacy" callers', status: "in_progress" },
    ] },
    { id: "Build", label: "Build", tasks: [
      { id: "Build-0", text: "Write the parser", status: "pending" },
      { id: "Build-1", text: "Add tests", status: "pending" },
      { id: "Build-2", text: "Update docs", status: "pending" },
    ] },
  ],
});
const texts = (draft, phase) => draft.phases.find(p => p.id === phase).tasks.map(t => t.text);

check("an untouched draft is a copy of the plan and changes nothing", () => {
  const p = plan();
  const d = P.startDraft(p);
  assert.equal(P.isDirty(p, d), false);
  assert.equal(P.buildMessage(p, d), null);
  assert.deepEqual(texts(d, "Build"), ["Write the parser", "Add tests", "Update docs"]);
  assert.equal(d.sig, P.planSignature(p));
});

check("the signature follows the plan's content, not its identity", () => {
  const p = plan();
  assert.equal(P.planSignature(p), P.planSignature(plan()));
  const q = plan();
  q.phases[1].tasks[0].status = "done";
  assert.notEqual(P.planSignature(p), P.planSignature(q));
  const r = plan();
  r.phases[1].tasks.pop();
  assert.notEqual(P.planSignature(p), P.planSignature(r));
});

check("toggling marks a pending task done and a done task open again", () => {
  const p = plan();
  let d = P.toggleTask(p, P.startDraft(p), "Build-0");
  assert.equal(d.phases[1].tasks[0].status, "done");
  d = P.toggleTask(p, d, "Scout-0");
  assert.equal(d.phases[0].tasks[0].status, "pending");
  const c = P.changes(p, d);
  assert.deepEqual(c.done, [{ phase: "Build", text: "Write the parser" }]);
  assert.deepEqual(c.reopened, [{ phase: "Scout", text: "Read the router" }]);
});

check("toggling twice restores the plan's own status, in-progress included", () => {
  const p = plan();
  let d = P.toggleTask(p, P.startDraft(p), "Scout-1");
  assert.equal(d.phases[0].tasks[1].status, "done");
  d = P.toggleTask(p, d, "Scout-1");
  assert.equal(d.phases[0].tasks[1].status, "in_progress");
  d = P.toggleTask(p, d, "Scout-0");
  d = P.toggleTask(p, d, "Scout-0");
  assert.equal(P.isDirty(p, d), false);
});

check("a dropped task is listed once as removed, whatever else was done to it", () => {
  const p = plan();
  let d = P.toggleTask(p, P.startDraft(p), "Build-1");
  d = P.setRemoved(d, "Build-1", true);
  assert.deepEqual(P.changes(p, d).removed, [{ phase: "Build", text: "Add tests" }]);
  assert.deepEqual(P.changes(p, d).done, []);
  const same = P.toggleTask(p, d, "Build-1");
  assert.deepEqual(same, d, "toggle of a dropped task is ignored");
  assert.equal(same.phases[1].tasks[1].status, "done", "the dropped task keeps its earlier staged status");
  d = P.setRemoved(d, "Build-1", false);
  assert.deepEqual(P.changes(p, d).removed, []);
});

check("restoring every edit discards cleanly", () => {
  const p = plan();
  let d = P.setRemoved(P.startDraft(p), "Build-2", true);
  d = P.setRemoved(d, "Build-2", false);
  d = P.moveTask(d, "Build-0", 1);
  d = P.moveTask(d, "Build-0", -1);
  assert.equal(P.isDirty(p, d), false);
  assert.equal(P.buildMessage(p, d), null);
});

check("moveTask swaps within the phase and stops at its ends", () => {
  const p = plan();
  let d = P.moveTask(P.startDraft(p), "Build-0", 1);
  assert.deepEqual(texts(d, "Build"), ["Add tests", "Write the parser", "Update docs"]);
  assert.deepEqual(texts(P.moveTask(d, "Build-1", -1), "Build"), ["Add tests", "Write the parser", "Update docs"], "the first task does not move up");
  d = P.moveTask(P.startDraft(p), "Build-0", -1);
  assert.deepEqual(texts(d, "Build"), ["Write the parser", "Add tests", "Update docs"]);
  d = P.moveTask(P.startDraft(p), "Build-2", 1);
  assert.deepEqual(texts(d, "Build"), ["Write the parser", "Add tests", "Update docs"]);
  assert.deepEqual(texts(d, "Scout"), ["Read the router", 'Find "legacy" callers'], "other phases stay");
});

check("dropTask puts a task before or after a neighbour of its phase", () => {
  const p = plan();
  const base = P.startDraft(p);
  assert.deepEqual(texts(P.dropTask(base, "Build-0", "Build-2", true), "Build"), ["Add tests", "Update docs", "Write the parser"]);
  assert.deepEqual(texts(P.dropTask(base, "Build-2", "Build-0", false), "Build"), ["Update docs", "Write the parser", "Add tests"]);
  assert.deepEqual(texts(P.dropTask(base, "Build-1", "Build-0", false), "Build"), ["Add tests", "Write the parser", "Update docs"]);
  assert.equal(P.dropTask(base, "Build-0", "Scout-0", false).phases[1], base.phases[1], "a target of another phase is no slot");
  assert.equal(P.dropTask(base, "Build-0", "Build-0", false).phases[1], base.phases[1]);
});

check("a reorder reports the phase's whole new order, without the dropped tasks", () => {
  const p = plan();
  let d = P.dropTask(P.startDraft(p), "Build-2", "Build-0", false);
  const c = P.changes(p, d);
  assert.deepEqual(c.reordered, [{ phase: "Build", order: ["Update docs", "Write the parser", "Add tests"] }]);
  d = P.setRemoved(d, "Build-1", true);
  assert.deepEqual(P.changes(p, d).reordered, [{ phase: "Build", order: ["Update docs", "Write the parser"] }]);
});

check("dropping a task alone is not a reorder", () => {
  const p = plan();
  const d = P.setRemoved(P.startDraft(p), "Build-0", true);
  const c = P.changes(p, d);
  assert.deepEqual(c.reordered, []);
  assert.deepEqual(c.removed, [{ phase: "Build", text: "Write the parser" }]);
});

check("the message names every change by exact text and phase, then asks for a todo update", () => {
  const p = plan();
  let d = P.toggleTask(p, P.startDraft(p), "Build-0");
  d = P.toggleTask(p, d, "Scout-0");
  d = P.setRemoved(d, "Build-2", true);
  d = P.dropTask(d, "Build-1", "Build-0", false);
  d = P.toggleTask(p, d, "Scout-1");
  const msg = P.buildMessage(p, d);
  assert.equal(msg, [
    "I adjusted the plan:",
    '- Marked done (phase "Scout"): "Find \\"legacy\\" callers"',
    '- Marked done (phase "Build"): "Write the parser"',
    '- Reopened as pending (phase "Scout"): "Read the router"',
    '- Removed from the plan (phase "Build"): "Update docs"',
    '- Reordered phase "Build"; its remaining tasks are now in this order:',
    '  1. "Add tests"',
    '  2. "Write the parser"',
    "",
    "Please update the todo list with the todo tool so it matches exactly these changes (leave every other task as it is), then continue with the plan.",
  ].join("\n"));
});

check("a task text with quotes or a newline stays one unambiguous token", () => {
  const p = plan();
  p.phases[1].tasks[0].text = 'Say "hi"\nthen stop';
  const d = P.setRemoved(P.startDraft(p), "Build-0", true);
  const line = P.buildMessage(p, d).split("\n").find(l => l.startsWith("- Removed"));
  assert.equal(line, '- Removed from the plan (phase "Build"): "Say \\"hi\\"\\nthen stop"');
});

check("operations leave the draft they were given untouched", () => {
  const p = plan();
  const d = P.startDraft(p);
  const snapshot = JSON.stringify(d);
  P.toggleTask(p, d, "Build-0");
  P.setRemoved(d, "Build-0", true);
  P.moveTask(d, "Build-0", 1);
  P.dropTask(d, "Build-0", "Build-2", true);
  assert.equal(JSON.stringify(d), snapshot);
});

console.log(`plan-edit: ${passed} checks passed`);
