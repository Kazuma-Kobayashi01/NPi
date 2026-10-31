// app/greetings.js: the start page's greeting.
import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";
import { fileURLToPath } from "node:url";

const ctx = { window: {} };
vm.runInNewContext(fs.readFileSync(fileURLToPath(new URL("../src/app/greetings.js", import.meta.url)), "utf8"), ctx);
const G = ctx.window.OMP_GREETINGS;
const plain = (v) => JSON.parse(JSON.stringify(v));
let n = 0;
const check = (name, fn) => { fn(); n++; };

check("hours fall into the five periods", () => {
  const at = Object.fromEntries([[4, "midnight"], [5, "morning"], [10, "morning"], [11, "noon"], [13, "noon"], [14, "afternoon"], [17, "afternoon"], [18, "evening"], [22, "evening"], [23, "midnight"], [0, "midnight"]]);
  for (const [h, p] of Object.entries(at)) assert.equal(G.periodOf(Number(h)), p, `hour ${h}`);
});
check("the name is filled in, or dropped with its comma", () => {
  assert.equal(G.fill("Good morning, {name}", "Kazuma"), "Good morning, Kazuma");
  assert.equal(G.fill("Good morning, {name}", ""), "Good morning");
  assert.equal(G.fill("Night owl! {name}", "Kazuma"), "Night owl! Kazuma");
  assert.equal(G.fill("Night owl! {name}", " "), "Night owl!");
  assert.equal(G.fill("Lunchtime, {name}?", ""), "Lunchtime?");
  assert.equal(G.fill("Good to see you, {name}. The afternoon is young", ""), "Good to see you. The afternoon is young");
});
check("the first half of the seed is the hour's greeting, the same for both modes", () => {
  assert.equal(G.greeting({ mode: "chat", name: "K", hour: 2, seed: 0 }), G.greeting({ mode: "code", name: "K", hour: 2, seed: 0 }));
  assert.equal(G.greeting({ name: "K", hour: 2, seed: 0 }), "Night owl! K");
  assert.ok(G.PERIODS.morning.some(t => G.fill(t, "K") === G.greeting({ name: "K", hour: 8, seed: 0.3 })));
});
check("the second half is the mode's own line; each mode has a dozen", () => {
  assert.ok(G.BY_MODE.chat.length >= 10 && G.BY_MODE.code.length >= 10);
  assert.notEqual(G.greeting({ mode: "chat", name: "K", hour: 9, seed: 0.9 }), G.greeting({ mode: "code", name: "K", hour: 9, seed: 0.9 }));
  assert.ok(G.BY_MODE.code.some(t => G.fill(t, "K") === G.greeting({ mode: "code", name: "K", hour: 9, seed: 0.75 })));
});
check("every seed gives a line, with or without a name, and never a leftover token", () => {
  for (const mode of ["chat", "code"]) for (let h = 0; h < 24; h++) for (let i = 0; i < 100; i++) {
    for (const name of ["", "Kazuma"]) {
      const g = G.greeting({ mode, name, hour: h, seed: i / 100 });
      assert.ok(g.length > 0 && !g.includes("{") && !g.includes("  ") && !/,\s*[?.!]/.test(g), g);
    }
  }
  assert.equal(typeof G.greeting({ seed: 5 }), "string");
});
console.log(`test-greetings: ${n} checks passed`);
