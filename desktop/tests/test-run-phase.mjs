// app/run-phase.js: the one-word status of a running turn.
import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";
import { fileURLToPath } from "node:url";

const ctx = { window: {} };
vm.runInNewContext(fs.readFileSync(fileURLToPath(new URL("../src/app/run-phase.js", import.meta.url)), "utf8"), ctx);
const { phase } = ctx.window.OMP_RUN_PHASE;
const plain = (v) => JSON.parse(JSON.stringify(v));
let n = 0;
const check = (name, fn) => { fn(); n++; };

check("idle shows nothing", () => {
  assert.equal(phase([{ kind: "user" }], false), null);
});
check("just sent: Loading", () => {
  assert.equal(plain(phase([], true)).label, "Loading");
  assert.equal(plain(phase([{ kind: "user", text: "hi" }], true)).label, "Loading");
});
check("a streaming reply: Reasoning while only thinking, Writing once text arrives", () => {
  const streaming = (extra) => [{ kind: "user" }, { kind: "assistant", streaming: true, blocks: [{ type: "text", text: "" }], ...extra }];
  assert.equal(plain(phase(streaming({}), true)).label, "Loading");
  assert.equal(plain(phase(streaming({ thought: "hmm" }), true)).label, "Reasoning");
  assert.equal(plain(phase(streaming({ thought: "hmm", blocks: [{ type: "text", text: "Hello" }] }), true)).label, "Writing");
});
check("a running tool names itself", () => {
  assert.deepEqual(plain(phase([{ kind: "tool", tool: "bash", status: "running" }], true)), { key: "tool", label: "Running bash" });
});
check("anything else mid-turn is Working", () => {
  assert.equal(plain(phase([{ kind: "tool", tool: "bash", status: "ok" }], true)).label, "Working");
});
console.log(`test-run-phase: ${n} checks passed`);
