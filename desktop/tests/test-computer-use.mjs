#!/usr/bin/env node
// Regression script for src/app/computer-use.js: which state a profile reads
// as, and in what order intent, host permissions and worker readiness win.
// Run: node tests/test-computer-use.mjs

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const win = {};
// eslint-disable-next-line no-new-func
new Function("window", readFileSync(join(root, "src/app/computer-use.js"), "utf8"))(win);
const { status, missingRequired } = win.OMP_COMPUTER_USE;

const config = over => ({
  supported: true,
  enabled: true,
  permissions: { screenRecording: true, accessibility: true, inputMonitoring: false },
  responsibleApp: "NPi Desktop",
  ...over,
});
const CAPS = {
  backend: "quartz", capture: true, input: true, ax: true, backgroundWindowInput: true,
  takeover: true, applications: true, menus: true, heldInput: true, spaces: true, globalEscape: true,
  capturePermission: "granted", inputPermission: "granted", axPermission: "granted",
  displayCount: 1, screenLocked: false, displayAsleep: false,
};
const runtime = over => ({ enabled: true, active: true, running: false, capabilities: CAPS, ...over });

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
async function acheck(label, fn) {
  try {
    await fn();
  } catch (err) {
    console.error(`FAILED: ${label}`);
    throw err;
  }
  passed++;
}

check("a failure from the host wins over every cached state", () => {
  assert.equal(status(config(), runtime(), "boom", "s1").kind, "error");
});

check("a process death is shown as one bounded line, not a stderr dump", () => {
  const death = `1 | foo\n2 | bar\n^ SessionModelRestoreError: Could not restore model x\n    at createAgentSessionScoped (cli.js:1:1)`;
  const detail = status(config(), runtime(), death, "s1").detail;
  assert.equal(detail.includes("\n"), false);
  assert.ok(detail.startsWith("1 | foo"), detail);
  const huge = "x".repeat(400);
  assert.equal(status(config(), runtime(), huge, "s1").detail.length, 241);
});

check("a request that has not been answered yet is not reported as missing", () => {
  const pending = config({ permissions: { screenRecording: true, accessibility: false, inputMonitoring: false } });
  assert.equal(status(pending, null, null, "s1").label, "Waiting for permissions");
  assert.equal(status(pending, null, null, "s1", "accessibility").label, "Waiting for macOS");
});

check("a profile with no permission grant cannot claim readiness", () => {
  const caps = runtime({ capabilities: { ...CAPS, input: false, inputPermission: "denied" } });
  assert.equal(status(config(), caps, null, "s1").kind, "waiting");
});

check("an enabled profile with no conversation keeps a waiting state, not ready", () => {
  assert.equal(status(config(), null, null, null).kind, "waiting");
});

check("a session without eval tools is not reported as ready", () => {
  assert.equal(status(config(), runtime({ enabled: false, capabilities: undefined }), null, "s1").kind, "waiting");
});

check("grants plus a capable worker is the only ready state", () => {
  assert.equal(status(config(), runtime(), null, "s1").kind, "ready");
  assert.equal(status(config(), runtime({ running: true }), null, "s1").label, "Operating");
});

check("turning the profile off never hides a conversation still operating", () => {
  const off = config({ enabled: false });
  assert.equal(status(off, runtime({ running: true }), null, "s1").kind, "error");
  assert.equal(status(off, runtime(), null, "s1").kind, "error");
  assert.equal(status(off, null, null, null).kind, "disabled");
});

check("unsupported hosts stay disabled regardless of grants", () => {
  assert.equal(status(config({ supported: false }), null, null, null).kind, "unsupported");
});

check("required permissions are reported in screen-then-input order", () => {
  assert.deepEqual(missingRequired({ screenRecording: false, accessibility: false }), ["screenRecording", "accessibility"]);
  assert.deepEqual(missingRequired({ screenRecording: true, accessibility: false }), ["accessibility"]);
  assert.deepEqual(missingRequired({ screenRecording: true, accessibility: true }), []);
});

// ── app/computer-rpc.js: a tab without a live process is not a failure ──────
const rpcWin = {};
// eslint-disable-next-line no-new-func
new Function("window", readFileSync(join(root, "src/app/computer-rpc.js"), "utf8"))(rpcWin);
const { create } = rpcWin.OMP_COMPUTER_RPC;

/** A Tauri stub: `onWrite` decides what happens after `send_command`. */
function stubTauri(onWrite) {
  const listeners = new Map();
  const off = [];
  return {
    off,
    tauri: {
      event: {
        listen: async (channel, handler) => { listeners.set(channel, handler); return () => off.push(channel); },
      },
      core: {
        invoke: async (command, args) => {
          if (command !== "send_command") throw new Error(`unexpected ${command}`);
          await onWrite(listeners, args);
        },
      },
    },
  };
}
const answer = (listeners, sid, body, id) =>
  listeners.get(`agent://line/${sid}`)({ payload: { text: JSON.stringify({ type: "response", id, ...body }) } });

await acheck("a refused write reports the tab as absent, not as a feature error", async () => {
  const { tauri, off } = stubTauri(() => { throw new Error("3 | x\n^ SessionModelRestoreError: nope"); });
  await assert.rejects(create(tauri).request("s1", { type: "set_computer_use", enabled: true }), error => {
    assert.equal(error.absent, true);
    assert.equal(error.message, "3 | x");
    return true;
  });
  assert.equal(off.length, 2, "both listeners are released");
});

await acheck("a process death during a request is absent and bounded to one line", async () => {
  const huge = `no frame\n${"y".repeat(5000)}`;
  const { tauri } = stubTauri(listeners => listeners.get("agent://exit/s1")({ payload: huge }));
  await assert.rejects(create(tauri).request("s1", { type: "get_computer_use" }), error => {
    assert.equal(error.absent, true);
    assert.equal(error.message.includes("\n"), false);
    assert.ok(error.message.length <= 200, String(error.message.length));
    return true;
  });
});

await acheck("a real response resolves with its data and unlistens", async () => {
  const { tauri, off } = stubTauri(async (listeners, args) => {
    answer(listeners, "s1", { success: true, data: { enabled: true } }, JSON.parse(args.json).id);
  });
  const data = await create(tauri).request("s1", { type: "get_computer_use" });
  assert.deepEqual(data, { enabled: true });
  assert.equal(off.length, 2);
});

await acheck("an RPC refusal keeps its reason and is not marked absent", async () => {
  const { tauri } = stubTauri(async (listeners, args) => {
    answer(listeners, "s1", { success: false, error: "Computer use is unavailable" }, JSON.parse(args.json).id);
  });
  await assert.rejects(create(tauri).request("s1", { type: "set_computer_use", enabled: true }), error => {
    assert.equal(error.absent, undefined);
    assert.equal(error.message, "Computer use is unavailable");
    return true;
  });
});

console.log(`test-computer-use: ${passed} checks passed`);
