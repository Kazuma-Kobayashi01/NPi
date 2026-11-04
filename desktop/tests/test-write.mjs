// app/write-pipeline.js: the Write window's progress bar and next step.
import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";
import { fileURLToPath } from "node:url";

const win = {};
vm.runInNewContext(fs.readFileSync(fileURLToPath(new URL("../src/app/write-pipeline.js", import.meta.url)), "utf8"), { window: win });
const W = win.OMP_WRITE;
let n = 0;
const check = (name, fn) => { try { fn(); n++; } catch (e) { console.error(`FAILED: ${name}`); throw e; } };
const sec = (state, words = 100, warnings = []) => ({ file: "a.md", title: "A", words, state, warnings });
const plain = v => JSON.parse(JSON.stringify(v));

check("nothing yet is idle everywhere", () => {
  const [draft, compile, link] = plain(W.stages({ ready: true, sections: [sec("new", 0)], link: "none" }));
  assert.deepEqual([draft.state, compile.state, link.state], ["idle", "idle", "idle"]);
});
check("half compiled wants attention and counts", () => {
  const [, compile] = plain(W.stages({ ready: true, sections: [sec("fresh"), sec("stale")], link: "none" }));
  assert.deepEqual([compile.percent, compile.state, compile.done, compile.total], [50, "attention", 1, 2]);
});
check("a running compile shows its own progress", () => {
  const [, compile] = plain(W.stages({ ready: true, sections: [sec("stale")], link: "none", run: { done: 1, total: 4, current: "b.md" } }));
  assert.deepEqual([compile.percent, compile.state, compile.current], [25, "active", "b.md"]);
});
check("a run that has not counted yet is at zero, not NaN", () => {
  assert.equal(plain(W.stages({ sections: [], run: { done: 0, total: 0, current: "" } }))[1].percent, 0);
});
check("a warning or a citation the library lacks keeps Compile from looking finished", () => {
  assert.equal(plain(W.stages({ sections: [sec("fresh", 9, ["x"])], link: "none" }))[1].state, "attention");
  assert.equal(plain(W.stages({ sections: [sec("fresh")], missing: ["k"], link: "none" }))[1].state, "attention");
  assert.equal(plain(W.stages({ sections: [sec("fresh")], missing: [], link: "fresh" }))[1].state, "done");
});
check("a stale link is half way", () => {
  const link = plain(W.stages({ sections: [sec("fresh")], link: "stale" }))[2];
  assert.deepEqual([link.percent, link.state], [50, "attention"]);
});
check("the next step", () => {
  assert.equal(W.nextStep({ ready: false }), "init");
  assert.equal(W.nextStep({ ready: true, sections: [sec("new", 1)], link: "none" }), "write");
  assert.equal(W.nextStep({ ready: true, sections: [sec("stale")], link: "fresh" }), "compile");
  assert.equal(W.nextStep({ ready: true, sections: [sec("fresh")], link: "stale" }), "link");
  assert.equal(W.nextStep({ ready: true, sections: [sec("fresh")], link: "fresh" }), "done");
});
console.log(`test-write: ${n} checks passed`);
