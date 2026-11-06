#!/usr/bin/env node
// Regression script for src/app/ui-store.js — what a page does on load with the
// saved interface settings (ui-state.json) and what localStorage already holds:
// put back keys WebKit lost, send back keys the file lacks or holds differently,
// and never let the launch-time copy overwrite a newer localStorage value.
// Run: node tests/test-ui-store.mjs

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const win = {};
// eslint-disable-next-line no-new-func
new Function("window", readFileSync(join(root, "src/app/ui-store.js"), "utf8"))(win);
const { plan } = win.OMP_UI_STORE;

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

check("a wiped WebKit store gets every saved key back and sends nothing", () => {
  assert.deepEqual(plan({ tweaks: "{\"theme\":\"ivory\"}" }, {}), {
    restore: { tweaks: "{\"theme\":\"ivory\"}" },
    upload: {},
  });
});

check("the first run after the move sends all of localStorage to the file", () => {
  assert.deepEqual(plan({}, { tweaks: "a", "npi.librarian.sort": "b" }), {
    restore: {},
    upload: { tweaks: "a", "npi.librarian.sort": "b" },
  });
  assert.deepEqual(plan(undefined, { tweaks: "a" }).upload, { tweaks: "a" });
});

check("a window opened later keeps localStorage's newer value and sends it back", () => {
  assert.deepEqual(plan({ tweaks: "old", gone: "x" }, { tweaks: "new" }), {
    restore: { gone: "x" },
    upload: { tweaks: "new" },
  });
});

check("matching copies need nothing", () => {
  assert.deepEqual(plan({ tweaks: "same" }, { tweaks: "same" }), { restore: {}, upload: {} });
});

console.log(`ok - ${passed} ui-store checks`);
