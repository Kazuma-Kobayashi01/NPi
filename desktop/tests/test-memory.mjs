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
check("a notice offers Undo only for something new", () => {
  assert.deepEqual(plain(M.noticeOf({ note: { id: 4, title: "Home", text: "Osaka" }, replaced: false })),
    { what: "Remembered", title: "Home", text: "Osaka", undo: { note: 4 } });
  assert.equal(M.noticeOf({ note: { id: 4, title: "Home", text: "Kyoto" }, replaced: true }).undo, null);
  assert.deepEqual(plain(M.noticeOf({ skill: "plan-build", replaced: false }).undo), { skill: "plan-build" });
  assert.equal(M.noticeOf({ skill: "plan-build", replaced: true }).what, "Skill updated");
  const p = M.noticeOf({ profile: "academic", text: "Pages for every claim." });
  assert.equal(p.title, "Academic"); assert.equal(p.undo, null);
  assert.equal(M.noticeOf({ profile: "elsewhere" }), null);
  assert.equal(M.noticeOf({}), null);
});
check("usage reads as a count of a limit", () => {
  assert.equal(M.share(1474, 3000), "1,474 / 3,000");
  assert.equal(M.share(undefined, 300), "0 / 300");
});
console.log(`test-memory: ${n} checks passed`);
