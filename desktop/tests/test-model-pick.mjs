// The model picker's pure logic: vendor names, grouping, and which models the
// small picker shows (app/model-pick.js).
import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";

const win = {};
vm.runInNewContext(fs.readFileSync(new URL("../src/app/model-pick.js", import.meta.url), "utf8"), { window: win });
// Values made inside the vm context have another realm's prototypes; compare plain copies.
const plain = (v) => JSON.parse(JSON.stringify(v));
const M = win.OMP_MODELS;

let failed = false;
function check(name, fn) {
  try { fn(); } catch (e) { failed = true; console.error("FAILED:", name); console.error(e); }
}

const m = (provider, id, name = id) => ({ provider, id, name });
const all = [
  m("anthropic", "opus", "Opus 5.5"), m("anthropic", "sonnet", "Sonnet 5.5"),
  m("openai-codex", "gpt", "GPT 5"), m("google-antigravity", "flash", "Gemini Flash"),
  m("my-gateway", "x", "X"),
];

check("provider ids become vendor names; vendors of one company share a name", () => {
  assert.equal(M.vendorLabel("anthropic"), "Anthropic");
  assert.equal(M.vendorLabel("openai-codex"), "OpenAI");
  assert.equal(M.vendorLabel("openai"), "OpenAI");
  assert.equal(M.vendorLabel("google-antigravity"), "Google");
  assert.equal(M.vendorLabel("my-gateway"), "My Gateway", "an unknown id is title-cased");
  assert.equal(M.vendorLabel(""), "Other");
  assert.equal(M.vendorLabel(undefined), "Other");
});

check("the picker lists every available favourite, sorted by vendor then name", () => {
  const out = M.pickerModels(all, ["openai-codex/gpt", "anthropic/opus", "gone/model"], "anthropic/sonnet", []);
  assert.deepEqual(plain(out.map(x => x.id)), ["opus", "sonnet", "gpt"], "unavailable favourites are skipped; the current model is kept");
});

check("with no favourites it falls back to the current model, then recent ones", () => {
  const out = M.pickerModels(all, [], "anthropic/sonnet", ["openai-codex/gpt", "anthropic/sonnet", "nope/x"]);
  assert.deepEqual(plain(out.map(x => x.id)), ["sonnet", "gpt"]);
  assert.deepEqual(plain(M.pickerModels([], [], "a/b", [])), []);
});

check("there is no limit on favourites, and the current model is always shown", () => {
  const favs = all.map(M.keyOf).filter(k => k !== "my-gateway/x");
  const out = M.pickerModels(all, favs, "my-gateway/x", []);
  assert.equal(out.length, all.length);
  assert.ok(out.some(x => x.id === "x"));
});

check("models group by vendor, alphabetically, shared vendors together", () => {
  const groups = M.groupByVendor([...all, m("openai", "o3", "o3")]);
  assert.deepEqual(plain(groups.map(g => g.vendor)), ["Anthropic", "Google", "My Gateway", "OpenAI"]);
  assert.deepEqual(plain(groups[3].models.map(x => x.id)), ["gpt", "o3"].sort((a, b) => a.localeCompare(b)));
  assert.deepEqual(plain(groups[0].models.map(x => x.name)), ["Opus 5.5", "Sonnet 5.5"]);
});

check("toggling a favourite adds it at the end or removes it", () => {
  assert.deepEqual(plain(M.toggleFavorite(["a/b"], "c/d")), ["a/b", "c/d"]);
  assert.deepEqual(plain(M.toggleFavorite(["a/b", "c/d"], "a/b")), ["c/d"]);
  assert.deepEqual(plain(M.toggleFavorite(undefined, "a/b")), ["a/b"]);
});

if (failed) process.exit(1);
console.log("test-model-pick: 5 checks passed");
