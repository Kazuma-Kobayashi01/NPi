#!/usr/bin/env node
// Regression script for src/app/review.js — the card a multi-expert review shows before it
// starts: models by angle, the providers the material goes to, and the cost.
// Run: node tests/test-review.mjs
import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";

const win = {};
vm.runInNewContext(fs.readFileSync(new URL("../src/app/review.js", import.meta.url), "utf8"), { window: win });
const R = win.OMP_REVIEW;
const plain = v => JSON.parse(JSON.stringify(v));
let n = 0;
const check = (name, fn) => { try { fn(); n++; } catch (e) { console.error(`FAILED: ${name}`); throw e; } };

check("tokens read as a rough size", () => {
  assert.equal(R.tokensLabel(800), "≈ 800 tokens");
  assert.equal(R.tokensLabel(186400), "≈ 186K tokens");
  assert.equal(R.tokensLabel(2_400_000), "≈ 2.4M tokens");
  assert.equal(R.tokensLabel(undefined), "≈ 0 tokens");
});
check("the card lists each expert by angle, then the arbiter, and flags a heavy run", () => {
  const v = plain(R.cardView({ kind: "Manuscript", subject: "Ch. 2", experts: [{ letter: "A", angle: "Domain expert", model: "a/x" }], arbiter: "c/z", providers: ["anthropic", "google"], tokens: 150000, missing: ["standard"] }));
  assert.equal(v.title, "Manuscript: Ch. 2");
  assert.deepEqual(v.lines, [{ who: "A · Domain expert", model: "a/x" }, { who: "Arbiter", model: "c/z" }]);
  assert.equal(v.providers, "anthropic, google");
  assert.equal(v.heavy, true);
  assert.equal(v.missing, "standard");
  assert.equal(R.cardView({ tokens: 5000 }).heavy, false);
});
console.log(`test-review: ${n} checks passed`);
