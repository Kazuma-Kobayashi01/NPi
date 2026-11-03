// app/notify.js: tweaks → Rust notification config, permission wording.
import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";
import { fileURLToPath } from "node:url";

const ctx = { window: {} };
vm.runInNewContext(fs.readFileSync(fileURLToPath(new URL("../src/app/notify.js", import.meta.url)), "utf8"), ctx);
const N = ctx.window.OMP_NOTIFY;
let n = 0;
const check = (name, fn) => { fn(); n++; };

check("the language follows the choice when Rust speaks it, else the system", () => {
  assert.equal(N.resolveLang("ja-JP", "en-GB"), "ja-JP");
  assert.equal(N.resolveLang("auto", "zh-TW"), "zh-TW");
  assert.equal(N.resolveLang("auto", "zh-Hant-HK"), "zh-TW");
  assert.equal(N.resolveLang("auto", "zh-CN"), "zh-CN");
  assert.equal(N.resolveLang("auto", "ja"), "ja-JP");
  assert.equal(N.resolveLang("fr-FR", "fr-FR"), "en-US");
  assert.equal(N.resolveLang(undefined, undefined), "en-US");
});
check("the master switch is off and the categories on until said otherwise", () => {
  assert.deepEqual(JSON.parse(JSON.stringify(N.configFrom({}, "en"))), { enabled: false, done: true, ask: true, failed: true, lang: "en-US" });
  const c = N.configFrom({ notifications: true, notifyDone: false, notifyAsk: "yes", language: "zh-CN" }, "en");
  assert.equal(c.enabled, true);
  assert.equal(c.done, false);
  assert.equal(c.ask, true, "a non-boolean keeps the default");
  assert.equal(c.lang, "zh-CN");
});
check("each permission state says what to do and what can be tried", () => {
  assert.equal(N.describe("granted").canTest, true);
  assert.equal(N.describe("notDetermined").canAsk, true);
  assert.match(N.describe("denied").text, /System Settings/);
  assert.equal(N.describe("denied").canTest, false);
  assert.equal(N.describe("unsupported").canAsk, false);
  assert.equal(N.describe(null).tone, "idle");
});
console.log(`test-notify: ${n} checks passed`);
