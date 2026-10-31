// The side panel's pure logic: context breakdown, quota rows, token rows
// (app/usage-view.js).
import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";

const win = {};
vm.runInNewContext(fs.readFileSync(new URL("../src/app/usage-view.js", import.meta.url), "utf8"), { window: win, JSON });
const U = win.OMP_USAGE;
// Values made inside the vm context have another realm's prototypes; compare plain copies.
const plain = (v) => JSON.parse(JSON.stringify(v));

let failed = false;
function check(name, fn) {
  try { fn(); } catch (e) { failed = true; console.error("FAILED:", name); console.error(e); }
}

check("tokens are estimated at 4 Latin characters, or 1 CJK character, each", () => {
  assert.equal(U.estimateTokens(""), 0);
  assert.equal(U.estimateTokens("abcdefgh"), 2);
  assert.equal(U.estimateTokens("你好"), 2);
  assert.equal(U.estimateTokens(undefined), 0);
});

check("the context splits into messages, system prompt, tools and free space, summing to the window", () => {
  const b = plain(U.contextBreakdown({
    contextUsage: { tokens: 1000, contextWindow: 10000 },
    systemPrompt: ["a".repeat(400)],            // ~100 tokens
    dumpTools: [{ name: "read", description: "x".repeat(200) }],
  }));
  const by = Object.fromEntries(b.parts.map(p => [p.key, p.tokens]));
  assert.equal(by.system, 100);
  assert.ok(by.tools > 0 && by.tools < 200, `tools ${by.tools}`);
  assert.equal(by.messages, 1000 - by.system - by.tools);
  assert.equal(by.free, 9000);
  assert.equal(b.parts.reduce((n, p) => n + p.tokens, 0), 10000);
  assert.equal(b.pct, 10);
});

check("estimates never exceed what omp says is in use", () => {
  const b = plain(U.contextBreakdown({ contextUsage: { tokens: 50, contextWindow: 1000 }, systemPrompt: ["a".repeat(4000)], dumpTools: [{ d: "b".repeat(4000) }] }));
  const by = Object.fromEntries(b.parts.map(p => [p.key, p.tokens]));
  assert.equal(by.system, 50);
  assert.equal(by.tools, 0);
  assert.equal(by.messages, 0);
  assert.equal(by.free, 950);
});

check("without context usage there is no breakdown", () => {
  assert.equal(U.contextBreakdown(null), null);
  assert.equal(U.contextBreakdown({}), null);
  assert.equal(U.contextBreakdown({ contextUsage: { tokens: 1, contextWindow: 0 } }), null);
});

check("quota rows split into the provider in use and the others", () => {
  const rows = [{ provider: "anthropic", limits: [] }, { provider: "openai-codex", limits: [] }];
  const q = plain(U.quotaFor(rows, "openai-codex"));
  assert.equal(q.mine.provider, "openai-codex");
  assert.deepEqual(q.others.map(r => r.provider), ["anthropic"]);
  assert.equal(U.quotaFor(rows, "deepseek").mine, null, "an API-key provider has no quota");
  assert.deepEqual(plain(U.quotaFor(undefined, "x")), { mine: null, others: [] });
});

check("time until a reset, and percent left", () => {
  const now = 1_000_000_000;
  assert.equal(U.untilReset(now + 45 * 60_000, now), "45m");
  assert.equal(U.untilReset(now + (3 * 60 + 20) * 60_000, now), "3h 20m");
  assert.equal(U.untilReset(now + (52 * 60) * 60_000, now), "2d 4h");
  assert.equal(U.untilReset(now - 5, now), "now");
  assert.equal(U.untilReset(null, now), "now");
  assert.equal(U.leftPct({ remainingFraction: 0.745 }), 75);
  assert.equal(U.leftPct({ remainingFraction: 2 }), 100);
  assert.equal(U.leftPct({}), null);
  assert.equal(U.usedPct({ remainingFraction: 0.745 }), 25, "used is what is not left");
  assert.equal(U.usedPct({ remainingFraction: 0 }), 100);
  assert.equal(U.usedPct({}), null);
});

check("token rows: input, output, cache read, cache write; reasoning only when used", () => {
  const rows = plain(U.tokenRows({ tokens: { input: 10, output: 20, cacheRead: 30, cacheWrite: 40, total: 100 } }));
  assert.deepEqual(rows.map(r => [r.label, r.tokens]), [["Input", 10], ["Output", 20], ["Cache read", 30], ["Cache write", 40]]);
  assert.equal(U.tokenRows({ tokens: { input: 1, output: 1, cacheRead: 0, cacheWrite: 0, reasoning: 5 } }).length, 5);
  assert.deepEqual(plain(U.tokenRows(null)), []);
});

check("cost formatting", () => {
  assert.equal(U.formatCost(0.236), "$0.24");
  assert.equal(U.formatCost(0.004), "<$0.01");
  assert.equal(U.formatCost(0), "$0.00");
  assert.equal(U.formatCost(undefined), "$0.00");
});

if (failed) process.exit(1);
console.log("test-usage-view: 8 checks passed");
