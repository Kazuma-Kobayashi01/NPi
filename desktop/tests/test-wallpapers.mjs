// The chat wallpaper's artwork list and its choice (app/wallpapers.js).
import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../", import.meta.url));
const win = {};
vm.runInNewContext(fs.readFileSync(path.join(root, "src/app/wallpapers.js"), "utf8"), { window: win, Math });
const W = win.OMP_WALLPAPERS;

let failed = false;
function check(name, fn) {
  try { fn(); } catch (e) { failed = true; console.error("FAILED:", name); console.error(e); }
}

check("there are at least five artworks, each with its file on disk and a credit", () => {
  assert.ok(W.LIST.length >= 5);
  const credits = fs.readFileSync(path.join(root, "src/assets/wallpapers/CREDITS.md"), "utf8");
  for (const a of W.LIST) {
    assert.ok(fs.existsSync(path.join(root, "src", a.file)), `${a.file} is missing`);
    assert.ok(credits.includes(`${a.id}.png`), `${a.id} has no credit line`);
    assert.ok(a.title && a.artist && a.year, a.id);
  }
  assert.equal(new Set(W.LIST.map(a => a.id)).size, W.LIST.length, "ids are unique");
});

check("a named artwork is used as is", () => {
  assert.equal(W.pick("melencolia", "any").id, "melencolia");
});

check("auto picks per conversation, stably, and spreads over the list", () => {
  assert.equal(W.pick("auto", "tab-1").id, W.pick("auto", "tab-1").id);
  const seen = new Set();
  for (let i = 0; i < 60; i++) seen.add(W.pick("auto", `tab-${i}`).id);
  assert.ok(seen.size >= 5, `only ${seen.size} different artworks over 60 tabs`);
});

check("an unknown id or no key still gives an artwork", () => {
  assert.ok(W.pick("gone", "k").id);
  assert.ok(W.pick(undefined, undefined).id);
});

if (failed) process.exit(1);
console.log("test-wallpapers: 4 checks passed");
