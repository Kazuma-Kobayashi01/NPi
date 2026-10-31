// app/tooltip.js: how a title splits into a name and shortcut keys.
import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";
import { fileURLToPath } from "node:url";

const src = fs.readFileSync(fileURLToPath(new URL("../src/app/tooltip.js", import.meta.url)), "utf8");
const ctx = { window: {}, document: { readyState: "complete" } };
vm.runInNewContext(src, ctx);
const { parse } = ctx.window.OMP_TIP;
const plain = (v) => JSON.parse(JSON.stringify(v));
let n = 0;
const check = (name, fn) => { fn(); n++; };

check("a plain title has no keys", () => assert.deepEqual(plain(parse("Changes")), { name: "Changes", keys: [] }));
check("a trailing shortcut becomes keys", () => {
  assert.deepEqual(plain(parse("Browser (Shift+Cmd+B)")), { name: "Browser", keys: ["⇧", "⌘", "B"] });
  assert.deepEqual(plain(parse("Terminal (Ctrl+`)")), { name: "Terminal", keys: ["⌃", "`"] });
});
check("a parenthesis that is not a shortcut stays in the name", () => {
  assert.deepEqual(plain(parse("Close (esc)")), { name: "Close", keys: ["Esc"] });
  assert.deepEqual(plain(parse("Refresh limits (every minute)")), { name: "Refresh limits (every minute)", keys: [] });
});
console.log(`test-tooltip: ${n} checks passed`);
