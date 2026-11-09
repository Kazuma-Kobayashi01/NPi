// The Context details view's pure logic (app/context-insight.js).
import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";

const win = {};
const ctx = { window: win, JSON };
vm.runInNewContext(fs.readFileSync(new URL("../src/app/usage-view.js", import.meta.url), "utf8"), ctx);
vm.runInNewContext(fs.readFileSync(new URL("../src/app/context-insight.js", import.meta.url), "utf8"), ctx);
const I = win.OMP_CONTEXT_INSIGHT;
const plain = (v) => JSON.parse(JSON.stringify(v));

let failed = false;
function check(name, fn) {
  try { fn(); } catch (e) { failed = true; console.error("FAILED:", name); console.error(e); }
}

const usage = (input, cacheRead, output) => ({ input, cacheRead, cacheWrite: 0, output, totalTokens: input + cacheRead + output });
const messages = [
  { role: "compactionSummary", summary: "x".repeat(4000) },
  { role: "user", content: "a".repeat(400), timestamp: 1 },
  {
    role: "assistant", model: "m1", timestamp: 2, usage: usage(1000, 0, 50),
    content: [
      { type: "thinking", thinking: "t".repeat(200) },
      { type: "toolCall", id: "1", name: "read", arguments: { path: "src/a.ts:10-20" } },
      { type: "toolCall", id: "2", name: "edit", arguments: { path: "src/a.ts", edits: [] } },
      { type: "toolCall", id: "3", name: "bash", arguments: { command: "cd x && FOO=1 npm test | tail -5" } },
    ],
  },
  { role: "toolResult", toolCallId: "1", toolName: "read", content: [{ type: "text", text: "r".repeat(800) }], isError: false },
  { role: "toolResult", toolCallId: "2", toolName: "edit", content: [{ type: "text", text: "failed" }], isError: true },
  { role: "toolResult", toolCallId: "3", toolName: "bash", content: [{ type: "text", text: "ok" }], isError: false },
  { role: "assistant", model: "m1", timestamp: 3, usage: usage(200, 1000, 20), content: [{ type: "text", text: "w".repeat(80) }] },
  { role: "bashExecution", command: "git status", output: "clean" },
];

check("splits the Messages total by role, scaling estimates up or down to fill it exactly", () => {
  for (const budget of [10_000, 100]) {
    const r = plain(I.analyze(messages, budget));
    const by = Object.fromEntries(r.parts.map((p) => [p.key, p.tokens]));
    assert.equal(r.parts.reduce((n, p) => n + p.tokens, 0), budget);
    assert.ok(!("overhead" in by), "text is attributed, not left as a residual");
    assert.ok(!("other" in by), "the compaction summary is the card's own row, not Other");
    assert.ok(by.user > by.reasoning && by.reasoning > by.answers, "proportions survive the scaling");
  }
  const r = plain(I.analyze(messages, 10_000));
  assert.equal(r.turns, 1);
  assert.deepEqual(plain(I.analyze([], 500)).parts, [{ key: "overhead", label: "Unattributed", tokens: 500 }]);
});

check("images are counted, not sized", () => {
  const withImage = [...messages, { role: "toolResult", toolName: "read", content: [{ type: "image", data: "AAAA" }], isError: false }];
  assert.equal(I.analyze(withImage, 1000).images, 1);
});

check("tools count calls and errors; files merge reads with their range selector stripped", () => {
  const r = plain(I.analyze(messages, 10_000));
  const edit = r.tools.find((t) => t.name === "edit");
  assert.deepEqual([edit.calls, edit.errors], [1, 1]);
  assert.deepEqual(r.files, [{ path: "src/a.ts", reads: 1, writes: 1 }]);
});

check("commands count each program of a pipeline, skipping env assignments", () => {
  const r = plain(I.analyze(messages, 10_000));
  assert.deepEqual(r.commands.map((c) => c.program).sort(), ["cd", "git", "npm", "tail"]);
  assert.deepEqual(plain(I.programsOf("sudo /usr/bin/make -j4; ls")), ["make", "ls"]);
  // Quotes, escaped spaces and heredoc bodies stay out of the program list.
  assert.deepEqual(plain(I.programsOf('"/a/NPi\\ Desktop/.tools/bun" run build')), ["bun"]);
  assert.deepEqual(plain(I.programsOf('export PATH="/a/NPi Desktop/.bin:$PATH"; bun test')), ["bun"]);
  assert.deepEqual(plain(I.programsOf("python3 - <<EOF\nimport os\nos.write(fd, b)\nEOF\nls")), ["python3", "ls"]);
  assert.deepEqual(plain(I.programsOf('node -e "\nconst a = 1;\n" | tail -3')), ["node", "tail"]);
  assert.deepEqual(plain(I.programsOf("FOO=1 sudo -E make 2>&1 & wait")), ["make", "wait"]);
});

check("trend has one point per request and the cache hit rate spans them", () => {
  const r = plain(I.analyze(messages, 10_000));
  assert.deepEqual(r.trend.map((p) => p.context), [1000, 1200]);
  assert.equal(r.requests, 2);
  assert.deepEqual(r.trend.map((p) => p.turn), [1, 1]);
  assert.equal(r.cacheHit, 1000 / 2200);
  assert.equal(I.analyze([], 0).cacheHit, null);
});

if (failed) process.exit(1);
console.log("context-insight: all checks passed");
