// app/memory.js: Settings' view of Memory.
import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";
import { fileURLToPath } from "node:url";

const win = {};
vm.runInNewContext(fs.readFileSync(fileURLToPath(new URL("../src/app/memory.js", import.meta.url)), "utf8"), { window: win });
const M = win.OMP_MEMORY;
let n = 0;
const check = (name, fn) => { try { fn(); n++; } catch (e) { console.error(`FAILED: ${name}`); throw e; } };
const plain = v => JSON.parse(JSON.stringify(v));

check("notes come in the fixed order and empty kinds go", () => {
  const g = plain(M.grouped([{ kind: "person", title: "a" }, { kind: "you", title: "b" }, { kind: "person", title: "c" }]));
  assert.deepEqual(g.map(x => [x.kind, x.notes.length]), [["you", 1], ["person", 2]]);
  assert.equal(g[0].label, "You");
  assert.deepEqual(plain(M.grouped(null)), []);
});
check("how long ago", () => {
  const day = 86400;
  assert.equal(M.age(1000, 1000 + 60).key, "Updated today");
  assert.equal(M.age(1000, 1000 + day + 5).key, "Updated yesterday");
  assert.deepEqual(plain(M.age(1000, 1000 + 3 * day)), { key: "Updated {n} days ago", n: 3 });
  assert.equal(M.age(2000, 1000).key, "Updated today", "a clock that moved back");
});
console.log(`test-memory: ${n} checks passed`);
