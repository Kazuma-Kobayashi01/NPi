// app/usage-limits.js: the subscription bars in Settings → Usage.
import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";
import { fileURLToPath } from "node:url";

const win = {};
vm.runInNewContext(fs.readFileSync(fileURLToPath(new URL("../src/app/usage-limits.js", import.meta.url)), "utf8"), { window: win });
const U = win.OMP_USAGE_LIMITS;
let n = 0;
const check = (name, fn) => { try { fn(); n++; } catch (e) { console.error(`FAILED: ${name}`); throw e; } };
const plain = v => JSON.parse(JSON.stringify(v));

check("a label loses the provider and says its window", () => {
  assert.equal(U.limitLabel({ label: "Claude 5 Hour", windowId: "5h" }, "anthropic"), "5 Hour");
  assert.equal(U.limitLabel({ label: "Gemini", windowId: "weekly" }, "google-antigravity"), "Gemini · weekly");
  assert.equal(U.limitLabel({ label: "7 days", windowId: "7d" }, "openai-codex"), "7 days");
  assert.equal(U.limitLabel({ label: "Credits" }, "cursor"), "Credits");
});
check("a reset says the time today, the weekday this week, the date beyond", () => {
  const now = new Date(2026, 10, 4, 12, 0).getTime();
  const today = plain(U.resetText(new Date(2026, 10, 4, 19, 0).getTime(), now, "en-US"));
  assert.equal(today.key, "Resets at {time}");
  assert.match(today.time, /7:00/);
  assert.match(U.resetText(new Date(2026, 10, 8, 15, 0).getTime(), now, "en-US").time, /^Sunday 3:00/);
  assert.match(U.resetText(new Date(2026, 10, 20, 15, 0).getTime(), now, "en-US").time, /^Nov 20/);
  assert.equal(U.resetText(null, now, "en-US"), null);
});
check("bars run from the shortest window, a shared quota shows once, and no fraction means no bar", () => {
  const a = { provider: "anthropic", limits: [
    { label: "Claude 7 Day", durationMs: 604800000, usedFraction: 0.59, status: "ok" },
    { label: "Claude 5 Hour", durationMs: 18000000, usedFraction: 0.734, status: "ok" },
    { label: "Shared", windowId: "weekly", resetsAt: 5, remainingFraction: 0.25, status: "warning" },
    { label: "Shared", windowId: "weekly", resetsAt: 5, remainingFraction: 0.25, status: "warning" },
    { label: "Credits", usedFraction: null, remainingFraction: null },
  ] };
  assert.deepEqual(plain(U.bars(a, 0, "en-US")).map(b => [b.label, b.percent]), [["5 Hour", 73], ["7 Day", 59], ["Shared · weekly", 75]]);
  assert.deepEqual(plain(U.bars({ provider: "x" }, 0, "en-US")), []);
});
check("how much is used stays in range", () => {
  assert.equal(U.usedOf({ usedFraction: 1.4 }), 1);
  assert.equal(U.usedOf({ remainingFraction: 1 }), 0);
  assert.equal(U.usedOf({}), null);
});
check("an account is named by its service and plan", () => {
  assert.equal(U.heading({ provider: "anthropic", plan: "max" }), "Claude Max");
  assert.equal(U.heading({ provider: "somebody" }), "somebody");
});
console.log(`test-usage-limits: ${n} checks passed`);
