// app/compaction.js: how a compaction's end settles its transcript row.
import assert from "node:assert/strict";
import fs from "node:fs";
import { fileURLToPath } from "node:url";

const win = {};
new Function("window", fs.readFileSync(fileURLToPath(new URL("../src/app/compaction.js", import.meta.url)), "utf8"))(win);
const C = win.OMP_COMPACTION;
let n = 0;
const check = (name, fn) => { try { fn(); n++; } catch (e) { console.error(`FAILED: ${name}`); throw e; } };
const pending = id => ({ kind: "compact", status: "pending", id });

check("a committed pass is done, with its summary and size", () => {
  const out = C.outcome({ result: { summary: "S", shortSummary: "short", tokensBefore: 9000 }, aborted: false });
  assert.deepEqual(out, { status: "done", summary: "S", shortSummary: "short", tokensBefore: 9000, reason: null });
});

check("nothing to compact is skipped with omp's reason, not a failure", () => {
  assert.deepEqual(C.outcome({ skipped: true, aborted: false, errorMessage: "Nothing to compact" }),
    { status: "skipped", reason: "Nothing to compact" });
});

check("a skipped automatic pass changed nothing, so its row goes", () => {
  const msgs = [{ kind: "user" }, pending("a")];
  assert.deepEqual(C.settle(msgs, C.outcome({ skipped: true, aborted: false, auto: true })), [{ kind: "user" }]);
});

check("a cancelled pass is cancelled; anything else is an error carrying the message", () => {
  assert.equal(C.outcome({ aborted: true }).status, "cancelled");
  assert.deepEqual(C.outcome({ aborted: false, errorMessage: "  No model selected " }), { status: "error", reason: "No model selected" });
  assert.deepEqual(C.outcome({ errorMessage: "" }), { status: "error", reason: null });
  assert.equal(C.outcome({ errorMessage: "x".repeat(900) }).reason.length, 500);
});

check("settle touches only the last pending row, and nothing once it is settled", () => {
  const msgs = [{ kind: "compact", status: "done", id: "old" }, pending("a")];
  const once = C.settle(msgs, C.outcome({ aborted: true }));
  assert.deepEqual(once.map(m => m.status), ["done", "cancelled"]);
  // The `compact` response after `manual_compaction_end`: no pending row left.
  assert.equal(C.settle(once, C.outcome({ errorMessage: "late" })), once);
});

check("a dead process leaves no row running forever", () => {
  const msgs = [pending("a"), { kind: "user" }];
  assert.deepEqual(C.interruptPending(msgs).map(m => m.status), ["cancelled", undefined]);
  const none = [{ kind: "user" }];
  assert.equal(C.interruptPending(none), none);
});

console.log(`test-compaction: ${n} checks passed`);
