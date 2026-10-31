// app/ui-detail.js: the five interface-detail steps.
import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";
import { fileURLToPath } from "node:url";

const ctx = { window: {} };
vm.runInNewContext(fs.readFileSync(fileURLToPath(new URL("../src/app/ui-detail.js", import.meta.url)), "utf8"), ctx);
const D = ctx.window.OMP_UI_DETAIL;
let n = 0;
const check = (name, fn) => { fn(); n++; };

check("there are five steps, numbered 1 to 5, with the default in the middle", () => {
  assert.equal(D.LEVELS.length, 5);
  assert.deepEqual(JSON.parse(JSON.stringify(D.LEVELS.map(l => l.level))), [1, 2, 3, 4, 5]);
  assert.equal(D.DEFAULT, 3);
});
check("a step is clamped; junk falls back to the default", () => {
  assert.equal(D.clamp(0), 1);
  assert.equal(D.clamp(9), 5);
  assert.equal(D.clamp("4"), 4);
  assert.equal(D.clamp("x"), 3);
  assert.equal(D.levelOf(undefined).name, "Standard");
});
check("every step sets the same tweak keys, and the fullest shows more than the barest", () => {
  const keys = [...Object.keys(D.LEVELS[0].patch)].sort();
  for (const l of D.LEVELS) assert.deepEqual([...Object.keys(l.patch)].sort(), keys);
  assert.equal(D.LEVELS[0].patch.layout, "focus");
  assert.equal(D.LEVELS[4].patch.layout, "rail");
  assert.equal(D.LEVELS[4].patch.showChanges, true);
});
console.log(`test-ui-detail: ${n} checks passed`);
