#!/usr/bin/env node
// Regression script for src/app/office-model.js and src/app/office-meter.js —
// the Office window's view of omp's OfficeState and its fuel-pump meter.
// Run: node tests/test-office-model.mjs

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const win = {};
for (const file of ["src/app/office-model.js", "src/app/office-meter.js"]) {
  // eslint-disable-next-line no-new-func
  new Function("window", readFileSync(join(root, file), "utf8"))(win);
}
const M = win.OMP_OFFICE_MODEL;
const P = win.OMP_OFFICE_METER;

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

const member = (id, role, capability = "common") => ({ id, role, capability, model: "" });
const amount = (tokens, cost, requests = 1) => ({ tokens, cost, requests });
const pull = (number, kind, office, status, author = "Worker-b") => ({ number, kind, office, author, status, title: `PR ${number}`, body: "", source: "s", target: "main", reviews: [] });

function state() {
  return {
    enabled: true,
    scale: "studio",
    company: { checkout: "/Users/x/Code/NPi Desktop", baseBranch: "main" },
    offices: [
      { number: 1, members: [member("Worker-b", "worker"), member("Worker-a", "leader", "expert"), member("Worker-c", "worker")] },
      { number: 2, members: [member("Worker-d", "leader"), member("Worker-e", "worker"), member("Worker-f", "worker")] },
    ],
    pulls: [pull(1, "worker", 1, "merged"), pull(2, "office", 1, "approved", "Worker-a"), pull(3, "worker", 2, "open", "Worker-e"), pull(4, "office", 2, "open", "Worker-d")],
    memory: [{ office: 1, notes: [{ by: "owner", text: "x" }], log: [] }, { office: 2, notes: [], log: [] }],
    activity: [
      { office: 1, phase: "awaiting_owner", working: [] },
      { office: 2, phase: "working", working: ["Worker-d", "Worker-e"], task: "Refactor login" },
    ],
    usage: {
      day: "2026-11-10",
      today: amount(700, 0.7, 7),
      total: amount(1700, 1.7, 17),
      members: {
        "general-manager": { today: amount(100, 0.1), total: amount(100, 0.1) },
        "Worker-a": { today: amount(200, 0.2), total: amount(1000, 1.0) },
        "Worker-e": { today: amount(400, 0.4), total: amount(600, 0.6) },
        "Worker-gone": { today: amount(0, 0), total: amount(0, 0) },
      },
    },
    runs: [
      { id: "r1", office: 1, member: "Worker-b", by: "Worker-a", task: "t", startedAt: "a", endedAt: "b", failed: true },
      { id: "r2", office: 2, member: "Worker-e", by: "Worker-d", task: "t2", startedAt: "c" },
    ],
  };
}

check("no state, no view", () => assert.equal(M.view(null), null));

check("the company line names the repository folder and branch", () => {
  const v = M.view(state());
  assert.deepEqual(v.company, { name: "NPi Desktop", checkout: "/Users/x/Code/NPi Desktop", branch: "main" });
  assert.equal(v.memberCount, 6);
  assert.equal(v.working, true);
});

check("the leader comes first and members carry their own status", () => {
  const [one, two] = M.view(state()).offices;
  assert.deepEqual(one.members.map(m => m.id), ["Worker-a", "Worker-b", "Worker-c"]);
  assert.equal(one.members[1].status, "failed");
  assert.equal(one.members[2].status, "idle");
  assert.deepEqual(two.members.map(m => m.status), ["working", "working", "idle"]);
  assert.equal(two.task, "Refactor login");
});

check("a member's thinking effort passes through and defaults to empty", () => {
  const s = state();
  s.offices[0].members[0].thinking = "high";
  const byId = Object.fromEntries(M.view(s).offices[0].members.map(m => [m.id, m]));
  assert.equal(byId["Worker-b"].thinking, "high");
  assert.equal(byId["Worker-a"].thinking, "");
});

check("an office adds up its members and knows what waits for the owner", () => {
  const [one, two] = M.view(state()).offices;
  assert.equal(one.usage.today.tokens, 200);
  assert.equal(one.usage.total.tokens, 1000);
  assert.equal(one.waiting.number, 2);
  assert.equal(one.openPulls, 1);
  assert.equal(one.lessons, 1);
  assert.equal(two.waiting, null);
  assert.equal(two.openPulls, 2);
});

check("the funnel counts each stage once", () => {
  assert.deepEqual(M.view(state()).funnel, { assigned: 2, workerReview: 1, officeReview: 1, awaitingOwner: 1, merged: 1 });
});

check("shares split the range between the manager and the offices", () => {
  const v = M.view(state());
  const today = M.shares(v, "today");
  assert.deepEqual(today.map(s => [s.key, s.tokens]), [["manager", 100], ["office-1", 200], ["office-2", 400]]);
  assert.ok(Math.abs(today.reduce((n, s) => n + s.share, 0) - 1) < 1e-9);
  assert.deepEqual(M.shares(v, "total").map(s => s.tokens), [100, 1000, 600]);
});

check("a state without a company still draws", () => {
  const v = M.view({ enabled: true, scale: "team", offices: [], company: null, reason: "not a git repository", pulls: [], memory: [], activity: [], usage: null, runs: [] });
  assert.equal(v.company, null);
  assert.equal(v.reason, "not a git repository");
  assert.deepEqual(v.usage.today, { tokens: 0, cost: 0, requests: 0 });
});

// ── Catalog & Thinking Effort ─────────────────────────────────────────────
const sampleCatalog = [
  { provider: "anthropic", id: "claude-3-7-sonnet", selector: "anthropic/claude-3-7-sonnet", name: "Claude 3.7 Sonnet", thinking: ["auto", "low", "medium", "high", "max"] },
  { provider: "anthropic", id: "claude-3-5-haiku", selector: "claude-3-5-haiku-alias", name: "Claude 3.5 Haiku", thinking: ["off"] },
  { provider: "openai", id: "gpt-4o", selector: "openai/gpt-4o", name: "GPT-4o", thinking: [] },
  { provider: "google", id: "gemini-2.5-pro", selector: "google/gemini-2.5-pro", name: "Gemini 2.5 Pro", thinking: ["off", "low", "high"] },
  { provider: "meta", id: "llama-3-8b", selector: "meta/llama-3-8b", name: "Llama 3 8B", thinking: null },
];

check("findCatalogModel matches by selector, provider/id, and handles missing/empty values", () => {
  assert.equal(M.findCatalogModel([], "anthropic/claude-3-7-sonnet"), null);
  assert.equal(M.findCatalogModel(sampleCatalog, null), null);
  assert.equal(M.findCatalogModel(sampleCatalog, ""), null);
  assert.equal(M.findCatalogModel(sampleCatalog, "unknown"), null);
  // match by selector
  assert.equal(M.findCatalogModel(sampleCatalog, "anthropic/claude-3-7-sonnet")?.id, "claude-3-7-sonnet");
  assert.equal(M.findCatalogModel(sampleCatalog, "claude-3-5-haiku-alias")?.id, "claude-3-5-haiku");
  // match by provider/id when selector is different
  assert.equal(M.findCatalogModel(sampleCatalog, "anthropic/claude-3-5-haiku")?.name, "Claude 3.5 Haiku");
});

check("effortChoices returns standard set for capability tier, unknown model, or empty catalog", () => {
  const standard = ["off", "minimal", "low", "medium", "high", "xhigh", "max"];
  assert.deepEqual(M.effortChoices(sampleCatalog, ""), standard);
  assert.deepEqual(M.effortChoices(sampleCatalog, null), standard);
  assert.deepEqual(M.effortChoices(sampleCatalog, "nonexistent-model"), standard);
  assert.deepEqual(M.effortChoices([], "anthropic/claude-3-7-sonnet"), standard);
});

check("effortChoices formats thinking levels for catalog models", () => {
  // model with auto and thinking levels: filters auto and off, prepends off
  assert.deepEqual(M.effortChoices(sampleCatalog, "anthropic/claude-3-7-sonnet"), ["off", "low", "medium", "high", "max"]);
  // model with only off in thinking: returns ["off"]
  assert.deepEqual(M.effortChoices(sampleCatalog, "claude-3-5-haiku-alias"), ["off"]);
  // model with empty thinking: returns ["off"]
  assert.deepEqual(M.effortChoices(sampleCatalog, "openai/gpt-4o"), ["off"]);
  // model with thinking: null: returns ["off"]
  assert.deepEqual(M.effortChoices(sampleCatalog, "meta/llama-3-8b"), ["off"]);
  // model with off, low, high: returns ["off", "low", "high"]
  assert.deepEqual(M.effortChoices(sampleCatalog, "google/gemini-2.5-pro"), ["off", "low", "high"]);
  // match by provider/id
  assert.deepEqual(M.effortChoices(sampleCatalog, "anthropic/claude-3-5-haiku"), ["off"]);
});

// ── Meter ────────────────────────────────────────────────────────────────
check("the meter rolls toward a higher reading and lands on it", () => {
  let m = P.rest({ tokens: 100, cost: 1 });
  m = P.aim(m, { tokens: 300, cost: 3 }, 1000);
  assert.deepEqual(P.shown(m, 1000), { tokens: 100, cost: 1 });
  const mid = P.shown(m, 1000 + P.ROLL_MS / 2);
  assert.ok(mid.tokens > 100 && mid.tokens < 300);
  assert.equal(P.rolling(m, 1000 + P.ROLL_MS / 2), true);
  assert.deepEqual(P.shown(m, 1000 + P.ROLL_MS), { tokens: 300, cost: 3 });
  assert.equal(P.rolling(m, 1000 + P.ROLL_MS), false);
});

check("a new reading mid-roll continues from what is on the dial", () => {
  let m = P.aim(P.rest({ tokens: 0, cost: 0 }), { tokens: 100, cost: 1 }, 0);
  const at = P.shown(m, 900);
  m = P.aim(m, { tokens: 200, cost: 2 }, 900);
  assert.deepEqual(P.shown(m, 900), at);
});

check("a lower reading (midnight, another range) shows at once", () => {
  let m = P.rest({ tokens: 500, cost: 5 });
  m = P.aim(m, { tokens: 0, cost: 0 }, 50);
  assert.deepEqual(P.shown(m, 50), { tokens: 0, cost: 0 });
  assert.equal(P.rolling(m, 51), false);
});

check("the same reading does not restart the roll", () => {
  const m = P.aim(P.rest({ tokens: 1, cost: 1 }), { tokens: 9, cost: 9 }, 10);
  assert.equal(P.aim(m, { tokens: 9, cost: 9 }, 500), m);
});

check("unit price is per million tokens", () => {
  assert.equal(P.unitPrice({ tokens: 2_000_000, cost: 13.56 }), 6.78);
  assert.equal(P.unitPrice({ tokens: 0, cost: 0 }), null);
});

check("yen follows the rate and needs one", () => {
  assert.equal(P.toJpy(12.47, 150.2), 12.47 * 150.2);
  assert.equal(P.toJpy(1, null), null);
  assert.equal(P.fmtJpy(P.toJpy(12.47, 150.2)), "¥1,873");
  assert.equal(P.fmtJpy(null), "¥—");
  assert.equal(P.fmtJpy(0.4), "¥0.4");
});

check("figures are grouped and small costs keep their digits", () => {
  assert.equal(P.fmtTokens(1832406.9), "1,832,406");
  assert.equal(P.fmtUsd(1234.5), "$1,234.50");
  assert.equal(P.fmtUsd(0.001365), "$0.0014");
  assert.equal(P.fmtUsd(0), "$0.00");
  assert.equal(P.fmtTokensShort(412_300), "412k");
  assert.equal(P.fmtTokensShort(1_832_406), "1.8M");
  assert.equal(P.fmtTokensShort(950), "950");
});

check("today starts over at local midnight", () => {
  assert.equal(P.msToMidnight(new Date(2026, 10, 10, 23, 59, 30)), 30_000);
  assert.equal(P.msToMidnight(new Date(2026, 10, 10, 0, 0, 0)), 86_400_000);
});

check("usage deltas are billed locally", () => {
  const amt = (tokens, cost) => ({ tokens, cost, requests: 1 });
  const usage = {
    day: "2026-11-10",
    members: { "1-a": { today: amt(100, 0.1), total: amt(500, 0.5) } },
    today: amt(100, 0.1),
    total: amt(500, 0.5),
  };
  const a = P.book(usage, { member: "1-a", day: "2026-11-10", amount: amt(50, 0.05) });
  assert.equal(a.members["1-a"].today.tokens, 150);
  assert.equal(a.members["1-a"].total.tokens, 550);
  assert.equal(a.today.requests, 2);
  assert.equal(usage.today.tokens, 100, "the old state is not mutated");
  const b = P.book(a, { member: "1-b", day: "2026-11-10", amount: amt(10, 0.01) });
  assert.equal(b.members["1-b"].today.tokens, 10);
  assert.equal(b.total.tokens, 560);
  // A booking on the next day starts "today" over.
  const c = P.book(b, { member: "1-a", day: "2026-11-11", amount: amt(7, 0.007) });
  assert.equal(c.day, "2026-11-11");
  assert.equal(c.today.tokens, 7);
  assert.equal(c.members["1-b"].today.tokens, 0);
  assert.equal(c.members["1-b"].total.tokens, 10);
  assert.equal(c.total.tokens, 567);
  // A late booking for yesterday counts only toward the total.
  const d = P.book(c, { member: "1-a", day: "2026-11-10", amount: amt(3, 0.003) });
  assert.equal(d.today.tokens, 7);
  assert.equal(d.total.tokens, 570);
  // Midnight with nobody working.
  const e = P.rollDay(d, "2026-11-12");
  assert.equal(e.today.tokens, 0);
  assert.equal(e.total.tokens, 570);
  assert.equal(P.rollDay(e, "2026-11-12"), e);
  assert.equal(P.localDay(new Date(2026, 0, 5, 23, 59)), "2026-01-05");
});

console.log(`office-model: ${passed} checks passed`);
