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
  assert.ok(by.system > 0 && by.system < 50);
  assert.ok(by.tools > 0 && by.tools < 50);
  assert.equal(by.messages, 50 - by.system - by.tools);
  assert.equal(by.free, 950);
});

check("active summaries are separate from current messages, never sized by tokensAfter", () => {
  const state = {
    contextUsage: { tokens: 1000, contextWindow: 10000 },
    systemPrompt: ["a".repeat(400)],
    dumpTools: [],
  };
  const active = [
    { role: "compactionSummary", summary: "b".repeat(800), tokensBefore: 200000, tokensAfter: 9000 },
    { role: "user", content: "c".repeat(400) },
  ];
  const b = plain(U.contextBreakdown(state, active));
  const by = Object.fromEntries(b.parts.map(p => [p.key, p.tokens]));
  assert.equal(by.compacted, 200);
  assert.equal(by.messages, 1000 - by.system - by.tools - by.compacted);
  assert.equal(b.parts.reduce((n, p) => n + p.tokens, 0), 10000);
});

check("fixed estimates do not starve known active messages after compaction", () => {
  const b = plain(U.contextBreakdown({
    contextUsage: { tokens: 1000, contextWindow: 10000 },
    systemPrompt: ["a".repeat(8000)],
    dumpTools: [{ description: "t".repeat(8000) }],
  }, [
    { role: "compactionSummary", summary: "s".repeat(800) },
    { role: "user", content: "m".repeat(800) },
  ]));
  const by = Object.fromEntries(b.parts.map(p => [p.key, p.tokens]));
  assert.ok(by.messages > 0);
  assert.ok(by.compacted > 0);
  assert.equal(b.parts.reduce((n, p) => n + p.tokens, 0), 10000);
});

check("native payload bytes are not invented summary tokens", () => {
  const b = plain(U.contextBreakdown({
    contextUsage: { tokens: 1000, contextWindow: 10000 },
  }, [{
    role: "compactionSummary", summary: "abcd",
    providerPayload: { encrypted: "x".repeat(80000) },
    images: [{ data: "x".repeat(80000) }],
  }]));
  assert.equal(b.parts.find(p => p.key === "compacted").tokens, 1);
  assert.ok(!U.contextBreakdown({ contextUsage: { tokens: 1, contextWindow: 10 } }, null)
    .parts.some(p => p.key === "compacted"));
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
console.log("test-usage-view: 11 checks passed");

// Exercise the real hook with deferred reads and live completion notifications.
{
  const slots = [];
  let index = 0, pending = [], settled;
  const changed = (a, b) => !a || a.length !== b.length || a.some((v, i) => !Object.is(v, b[i]));
  const React = {
    useState(initial) {
      const slot = slots[index] ??= { value: initial };
      index++;
      return [slot.value, value => { slot.value = typeof value === "function" ? value(slot.value) : value; }];
    },
    useRef(initial) { return slots[index++] ??= { current: initial }; },
    useCallback(fn, deps) {
      const i = index++;
      if (!slots[i] || changed(slots[i].deps, deps)) slots[i] = { deps, value: fn };
      return slots[i].value;
    },
    useEffect(fn, deps) {
      const i = index++, prev = slots[i];
      if (!prev || changed(prev.deps, deps)) pending.push(() => {
        prev?.cleanup?.();
        slots[i] = { deps, cleanup: fn() };
      });
    },
  };
  const window = {};
  vm.runInNewContext(fs.readFileSync(new URL("../src/app/use-quotas.jsx", import.meta.url), "utf8"), { window, React });
  const calls = [];
  const bridge = {
    usageLimits(profile, force) {
      return new Promise((resolve, reject) => calls.push({ profile, force, resolve, reject }));
    },
    onSessionSettled(fn) { settled = fn; return () => { settled = null; }; },
  };
  let profileId = "a";
  const render = () => {
    index = 0; pending = [];
    const result = window.useQuotas({ bridge, profileId, enabled: true });
    for (const effect of pending) effect();
    return result;
  };
  const flush = async () => { for (let i = 0; i < 12; i++) await Promise.resolve(); };
  render();
  await flush();
  assert.equal(calls[0].force, false);
  render().refresh();
  await flush();
  assert.equal(calls.length, 1, "manual refresh waits for the automatic read");
  calls[0].resolve({ ok: true, value: [{ provider: "old" }] });
  await flush();
  assert.equal(calls[1].force, true);
  assert.equal(render().rows, null, "superseded automatic response is not shown");
  calls[1].resolve({ ok: true, value: [{ provider: "fresh" }] });
  await flush();
  assert.equal(render().rows[0].provider, "fresh");
  settled("a");
  await flush();
  assert.equal(calls[2].force, true, "completion invalidates the originating profile");
  settled("a");
  profileId = "b";
  assert.equal(render().rows, null, "profile switch clears previous rows immediately");
  await flush();
  assert.equal(calls[3].profile, "b");
  calls[2].resolve({ ok: true, value: [{ provider: "late" }] });
  calls[3].resolve({ ok: true, value: [{ provider: "b" }] });
  await flush();
  assert.equal(calls[4].profile, "a");
  assert.equal(calls[4].force, true, "later completion is not dropped while a refresh runs");
  calls[4].resolve({ ok: true, value: [{ provider: "a2" }] });
  await flush();
  assert.equal(render().rows[0].provider, "b", "background completion cannot replace active profile");
  settled("b");
  await flush();
  calls[5].reject(new Error("offline"));
  await flush();
  assert.equal(render().rows[0].provider, "b");
  assert.equal(render().error, "offline");
  for (const slot of slots) slot?.cleanup?.();
}
