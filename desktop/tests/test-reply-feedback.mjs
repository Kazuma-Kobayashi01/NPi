#!/usr/bin/env node
// Regression script for src/app/reply-feedback.js — the local 👍/👎 store of
// assistant replies, kept per conversation and reply in one localStorage key.
// Run: node tests/test-reply-feedback.mjs

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const win = {};
// eslint-disable-next-line no-new-func
new Function("window", readFileSync(join(root, "src/app/reply-feedback.js"), "utf8"))(win);
const FB = win.OMP_REPLY_FEEDBACK;

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

/** A localStorage that can be shared by two stores, like two windows or a reload. */
const memoryStorage = () => {
  const map = new Map();
  return {
    getItem: (k) => (map.has(k) ? map.get(k) : null),
    setItem: (k, v) => { map.set(k, String(v)); },
    removeItem: (k) => { map.delete(k); },
  };
};
const bytes = (v) => new TextEncoder().encode(JSON.stringify(v)).length;

check("no localStorage, no shared store", () => {
  assert.equal(FB.shared, null);
});

check("conversationKey is the session file's name", () => {
  assert.equal(FB.conversationKey("/Users/a/.omp/sessions/--proj--/2026-11-13T10-00-00_abc.jsonl"), "2026-11-13T10-00-00_abc.jsonl");
  assert.equal(FB.conversationKey("C:\\Users\\a\\s\\x.jsonl"), "x.jsonl");
  assert.equal(FB.conversationKey("plain.jsonl"), "plain.jsonl");
  assert.equal(FB.conversationKey(""), null);
  assert.equal(FB.conversationKey(null), null);
  assert.equal(FB.conversationKey(undefined), null);
});

check("a rating is stored per conversation and reply, and read back", () => {
  const store = FB.createStore(memoryStorage());
  assert.equal(store.get("c1", 100), undefined);
  store.set("c1", 100, { r: "up" });
  store.set("c1", 200, { r: "down", t: "too long" });
  store.set("c2", 100, { r: "down" });
  assert.deepEqual(store.get("c1", 100), { r: "up" });
  assert.deepEqual(store.get("c1", 200), { r: "down", t: "too long" });
  assert.deepEqual(store.get("c2", 100), { r: "down" });
  assert.equal(store.get("c2", 200), undefined);
  assert.equal(store.get(null, 100), undefined);
  assert.equal(store.get("c1", null), undefined);
});

check("ratings survive a reload (a second store over the same storage)", () => {
  const storage = memoryStorage();
  FB.createStore(storage).set("c1", 100, { r: "down", t: "wrong file" });
  assert.deepEqual(FB.createStore(storage).get("c1", 100), { r: "down", t: "wrong file" });
  assert.ok(JSON.parse(storage.getItem(FB.KEY)).c1, "stored under the one key");
});

check("rating again replaces; clear removes and drops an empty conversation", () => {
  const storage = memoryStorage();
  const store = FB.createStore(storage);
  store.set("c1", 100, { r: "up" });
  store.set("c1", 100, { r: "down", t: "no" });
  assert.deepEqual(store.get("c1", 100), { r: "down", t: "no" });
  store.set("c1", 200, { r: "up" });
  store.clear("c1", 100);
  assert.equal(store.get("c1", 100), undefined);
  assert.deepEqual(store.get("c1", 200), { r: "up" });
  store.clear("c1", 200);
  assert.deepEqual(JSON.parse(storage.getItem(FB.KEY)), {});
  store.clear("c1", 999); // nothing to clear: no write, no throw
});

check("the reason is trimmed, bounded, and dropped when blank", () => {
  const store = FB.createStore(memoryStorage());
  store.set("c", 1, { r: "down", t: "  spaced  " });
  assert.equal(store.get("c", 1).t, "spaced");
  store.set("c", 2, { r: "down", t: "x".repeat(2000) });
  assert.equal(store.get("c", 2).t.length, FB.MAX_REASON_CHARS);
  store.set("c", 3, { r: "down", t: "   " });
  assert.deepEqual(store.get("c", 3), { r: "down" });
  store.set("c", 4, { r: "up", t: undefined });
  assert.deepEqual(store.get("c", 4), { r: "up" });
});

check("get is a stable snapshot until the entry changes", () => {
  const store = FB.createStore(memoryStorage());
  store.set("c", 1, { r: "up" });
  store.set("c", 2, { r: "up" });
  const first = store.get("c", 1);
  assert.equal(store.get("c", 1), first);
  store.set("c", 2, { r: "down" });
  assert.equal(store.get("c", 1), first, "an unrelated change keeps the object");
  store.set("c", 1, { r: "down" });
  assert.notEqual(store.get("c", 1), first);
});

check("subscribers hear every change, once, until they unsubscribe", () => {
  const store = FB.createStore(memoryStorage());
  let calls = 0;
  const off = store.subscribe(() => { calls++; });
  store.set("c", 1, { r: "up" });
  store.clear("c", 1);
  store.clear("c", 1);
  assert.equal(calls, 2);
  off();
  store.set("c", 1, { r: "up" });
  assert.equal(calls, 2);
});

check("a change made by another window is seen by the next get", () => {
  const storage = memoryStorage();
  const mine = FB.createStore(storage);
  const theirs = FB.createStore(storage);
  mine.set("c", 1, { r: "up" });
  assert.deepEqual(theirs.get("c", 1), { r: "up" });
  theirs.set("c", 1, { r: "down" });
  assert.deepEqual(mine.get("c", 1), { r: "down" });
  let calls = 0;
  mine.subscribe(() => { calls++; });
  mine.notifyExternal();
  assert.equal(calls, 1);
});

check("a stored value that is not an object is ignored", () => {
  for (const junk of ["not json", "[1,2]", "null", "42", '"s"']) {
    const storage = memoryStorage();
    storage.setItem(FB.KEY, junk);
    const store = FB.createStore(storage);
    assert.equal(store.get("c", 1), undefined, junk);
    store.set("c", 1, { r: "up" });
    assert.deepEqual(store.get("c", 1), { r: "up" }, junk);
  }
});

check("a storage that refuses the write leaves the store as it was", () => {
  const storage = memoryStorage();
  const store = FB.createStore(storage);
  store.set("c", 1, { r: "up" });
  storage.setItem = () => { throw new Error("quota"); };
  const warn = console.warn;
  console.warn = () => {};
  try { store.set("c", 2, { r: "down" }); } finally { console.warn = warn; }
  assert.equal(store.get("c", 2), undefined);
  assert.deepEqual(store.get("c", 1), { r: "up" });
});

check("over the byte bound the conversation written longest ago goes first", () => {
  let store = {};
  for (let c = 0; c < 6; c++) {
    for (let ts = 0; ts < 10; ts++) store = FB.put(store, `conv-${c}`, 1700000000000 + ts, { r: "down", t: "reason ".repeat(10) });
  }
  const max = Math.floor(bytes(store) * 0.6);
  const trimmed = FB.trim(store, max);
  assert.ok(bytes(trimmed) <= max);
  assert.ok(!("conv-0" in trimmed), "the oldest conversation is gone");
  assert.ok("conv-5" in trimmed, "the newest stays");
  assert.equal(FB.trim(store, bytes(store)), store, "a store that fits is returned as it is");
});

check("writing a conversation again makes it the newest", () => {
  let store = {};
  store = FB.put(store, "a", 1, { r: "up" });
  store = FB.put(store, "b", 1, { r: "up" });
  store = FB.put(store, "a", 2, { r: "up" });
  assert.deepEqual(Object.keys(store), ["b", "a"]);
});

check("a single conversation over the bound loses its oldest ratings", () => {
  let store = {};
  for (let ts = 0; ts < 40; ts++) store = FB.put(store, "only", 1700000000000 + ts, { r: "down", t: "because of reasons" });
  const max = Math.floor(bytes(store) / 2);
  const trimmed = FB.trim(store, max);
  assert.ok(bytes(trimmed) <= max);
  assert.ok(!(1700000000000 in trimmed.only));
  assert.ok(1700000000039 in trimmed.only, "the newest rating stays");
});

check("the default bound keeps the value under ui-state.json's per-value limit", () => {
  assert.ok(FB.MAX_BYTES < 64 * 1024);
});

console.log(`reply-feedback: ${passed} checks passed`);
