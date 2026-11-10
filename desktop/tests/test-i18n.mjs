// app/i18n.js and its dictionaries (app/i18n/*.js): the UI language.
import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";
import { fileURLToPath } from "node:url";

const ctx = { window: { TWEAK_DEFAULTS: {} } };
const run = f => vm.runInNewContext(fs.readFileSync(fileURLToPath(new URL(`../src/app/${f}`, import.meta.url)), "utf8"), ctx);
for (const f of ["i18n.js", "i18n/ui.js", "i18n/office.js", "i18n/settings.js", "i18n/settings-desc.js", "greetings.js", "settings-schema.js"]) run(f);
const I = ctx.window.OMP_I18N;
const D = I.dictionaries();
let n = 0;
const check = (name, fn) => { try { fn(); n++; } catch (e) { console.error(`FAILED: ${name}`); throw e; } };
const placeholders = s => [...String(s).matchAll(/\{(\w+)\}/g)].map(m => m[1]).sort().join(",");
const DOTTED = /^[a-z]+(\.[\w-]+)+$/;

check("the four locales and their own names", () => {
  assert.deepEqual(Array.from(I.LOCALES).sort(), ["en-US", "ja-JP", "zh-CN", "zh-TW"]);
  assert.deepEqual({ ...I.NAMES }, { "zh-CN": "简体中文", "zh-TW": "繁體中文", "en-US": "English (US)", "ja-JP": "日本語" });
});
check("the three translated sets have identical keys and no empty value", () => {
  const keys = Object.keys(D["zh-CN"]).sort();
  assert.ok(keys.length > 250, `only ${keys.length} keys`);
  for (const loc of ["zh-TW", "ja-JP"]) assert.deepEqual(Object.keys(D[loc]).sort(), keys, loc);
  for (const loc of ["zh-CN", "zh-TW", "ja-JP"]) for (const k of keys) assert.ok(String(D[loc][k]).trim(), `${loc} ${k}`);
});
check("every translation keeps the {placeholders} of its key", () => {
  for (const [k, zh] of Object.entries(D["zh-CN"])) {
    const want = DOTTED.test(k) ? placeholders(D["en-US"][k]) : placeholders(k);
    for (const loc of ["zh-CN", "zh-TW", "ja-JP"]) assert.equal(placeholders(D[loc][k]), want, `${loc} ${k}`);
  }
});
check("a dotted id has its English text; English source keys need none", () => {
  for (const k of Object.keys(D["zh-CN"])) if (DOTTED.test(k)) assert.ok(D["en-US"][k], k);
  for (const k of Object.keys(D["en-US"])) assert.ok(D["zh-CN"][k], `en-only ${k}`);
});
check("t: selected locale, then en-US, then the key; {vars} filled", () => {
  I.setLocale("zh-CN");
  assert.equal(I.t("Settings"), "设置");
  assert.equal(I.t("{n} matching", { n: 3 }), "3 项匹配");
  assert.equal(I.t("Not in any dictionary"), "Not in any dictionary");
  assert.equal(I.t("Not in {x}", { y: 1 }), "Not in {x}", "an unknown variable stays");
  I.setLocale("en-US");
  assert.equal(I.t("Settings"), "Settings");
  assert.equal(I.t("work.thought"), "Thought");
  I.setLocale("ja-JP");
  assert.equal(I.t("Settings"), "設定");
  I.setLocale("xx");
  assert.equal(I.locale(), "en-US", "an unsupported locale is en-US");
});
check("detect maps a browser language to a supported locale", () => {
  const at = { "zh-CN": "zh-CN", "zh-Hans": "zh-CN", "zh": "zh-CN", "zh-SG": "zh-CN", "zh-TW": "zh-TW", "zh-HK": "zh-TW", "zh-Hant": "zh-TW", "zh-Hant-TW": "zh-TW",
    "ja": "ja-JP", "ja-JP": "ja-JP", "en-GB": "en-US", "en-US": "en-US", "fr-FR": "en-US", "": "en-US", "ZH_tw": "zh-TW" };
  for (const [tag, want] of Object.entries(at)) assert.equal(I.detect(tag), want, tag);
});
check("resolve: an explicit choice wins, auto follows the system", () => {
  assert.equal(I.resolve("ja-JP", "zh-CN"), "ja-JP");
  assert.equal(I.resolve("auto", "zh-HK"), "zh-TW");
  assert.equal(I.resolve(undefined, "de"), "en-US");
  assert.equal(I.resolve("klingon", "ja"), "ja-JP");
});
check("every schema text has a translation (label, desc, title, blurb, group)", () => {
  const S = ctx.window.OMP_SETTINGS;
  const missing = [];
  const need = s => { if (s && !(s in D["zh-CN"])) missing.push(s.slice(0, 60)); };
  for (const sec of S.SECTIONS) {
    need(sec.title); need(sec.blurb);
    for (const g of sec.groups) { need(g.name); for (const r of g.rows) { need(r.label); need(r.desc); for (const o of r.options ?? []) if (/^[a-zA-Z-]+$/.test(o.label) && !["rail", "split", "focus", "tree", "browser", "enter"].includes(o.label)) need(o.label); } }
  }
  assert.deepEqual(missing, []);
});
check("the Language row offers auto and the four languages in their own names", () => {
  const row = ctx.window.OMP_SETTINGS.rows().find(r => r.key === "language");
  assert.deepEqual(Array.from(row.options, o => o.value), ["auto", "zh-CN", "zh-TW", "en-US", "ja-JP"]);
  assert.deepEqual(Array.from(row.options.slice(1), o => o.label), ["简体中文", "繁體中文", "English (US)", "日本語"]);
});
check("settings search also matches the text of the UI language", () => {
  I.setLocale("zh-CN");
  const hit = ctx.window.OMP_SETTINGS.search("壁纸", {});
  assert.ok(hit.total >= 1);
  assert.equal(ctx.window.OMP_SETTINGS.search("wallpaper", {}).total >= 1, true, "English still matches");
  I.setLocale("en-US");
});
check("greetings exist in every language, with or without a name", () => {
  const G = ctx.window.OMP_GREETINGS;
  for (const locale of ["zh-CN", "zh-TW", "ja-JP", "en-US"]) for (const mode of ["chat", "code", "academic"]) for (let h = 0; h < 24; h += 3) for (let i = 0; i < 40; i++) for (const name of ["", "Kazuma"]) {
    const g = G.greeting({ mode, name, hour: h, seed: i / 40, locale });
    assert.ok(g.length > 0 && !g.includes("{") && !g.includes("  ") && !/^[，、,]|[，、,]$/.test(g), `${locale}/${mode}/${h}/${i}/${name}: ${g}`);
  }
  assert.equal(G.greeting({ name: "K", hour: 8, seed: 0, locale: "ja-JP" }), "おはようございます、Kさん");
  assert.equal(G.greeting({ name: "", hour: 8, seed: 0, locale: "ja-JP" }), "おはようございます");
  assert.equal(G.greeting({ name: "", hour: 8, seed: 0, locale: "zh-CN" }), "早上好");
});
console.log(`test-i18n: ${n} checks passed`);
