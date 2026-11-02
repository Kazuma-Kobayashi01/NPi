#!/usr/bin/env node
// Shared progressive image loader (src/app/image-loader.js):
// - a source without a distinct preview is one full request, never a
//   thumbnail request followed by a duplicate full request;
// - queued reads are gated so a cancel before admission never reaches IPC;
// - explicit previews upgrade in place, aborts are per consumer, failures
//   are explicit-retry only, and the cache stays bounded.
// Run: node tests/test-chat-image-loader.mjs

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import vm from "node:vm";

const source = readFileSync(new URL("../src/app/image-loader.js", import.meta.url), "utf8");
const ctx = vm.createContext({
  console,
  performance,
  AbortController,
  setTimeout,
  clearTimeout,
  URL,
  Promise,
  Error,
  Object,
  Array,
  String,
  Number,
  Math,
  Date,
  Set,
  Map,
});
ctx.window = ctx;
ctx.globalThis = ctx;
vm.runInContext(source, ctx, { filename: "app/image-loader.js" });
const { createStore, conversationKey, deriveImageStatus, redactSource } = ctx.OMP_CHAT_IMAGE;

const tick = () => new Promise(resolve => setTimeout(resolve, 0));

function deferred() {
  let resolve, reject;
  const promise = new Promise((res, rej) => { resolve = res; reject = rej; });
  return { promise, resolve, reject };
}

function harness(overrides = {}) {
  const calls = [];
  const cancels = [];
  const read = (source, { requestId, priority }) => {
    const pending = deferred();
    calls.push({ source, requestId, priority, ...pending });
    return pending.promise;
  };
  const store = createStore({
    key: "test-session",
    read,
    cancel: requestId => {
      cancels.push(requestId);
      const call = calls.find(c => c.requestId === requestId);
      call?.reject(new Error("image request cancelled"));
      return true;
    },
    ...overrides,
  });
  return { store, calls, cancels };
}

const THUMB = { data: "dGh1bWI=", mimeType: "image/png" };
const FULL = { data: "ZnVsbA==", mimeType: "image/png" };

function resolveCall(call, result = FULL) {
  call.resolve({ ...result, requestId: call.requestId, cacheHit: false, timing: { totalMs: 1 } });
}

test("a pending image reports local loading while the rest of the message renders", async () => {
  const h = harness();
  const seen = [];
  const off = h.store.subscribeImage("https://example.com/a.png", snapshot => seen.push(snapshot));
  assert.equal(seen[0].phase, "loading");
  assert.equal(seen[0].url, null);
  await tick();
  assert.equal(h.calls.length, 1);
  assert.equal(h.calls[0].priority, "full");
  off();
});

test("no distinct preview means exactly one full request and cache reuse", async () => {
  const h = harness();
  const off = h.store.subscribeImage("https://example.com/no-thumb.png", () => {});
  await tick();
  assert.equal(h.calls.length, 1);
  assert.equal(h.calls[0].priority, "full");
  assert.equal(h.calls[0].source, "https://example.com/no-thumb.png");
  resolveCall(h.calls[0], FULL);
  await tick();
  off();

  const cached = [];
  const offCached = h.store.subscribeImage("https://example.com/no-thumb.png", snapshot => cached.push(snapshot));
  await tick();
  assert.equal(h.calls.length, 1, "a resolved image is shared, not downloaded again");
  assert.equal(cached[0].phase, "ready");
  assert.equal(cached[0].fullUrl, "data:image/png;base64,ZnVsbA==");
  offCached();
});

test("an explicit preview lands first, then upgrades to the full source", async () => {
  const h = harness();
  const seen = [];
  const source = "https://example.com/full.png";
  const preview = "https://example.com/preview.png";
  const off = h.store.subscribeImage(source, snapshot => seen.push(snapshot), { thumbnailSource: preview });
  await tick();
  assert.equal(h.calls.length, 1);
  assert.equal(h.calls[0].source, preview);
  assert.equal(h.calls[0].priority, "thumbnail");
  resolveCall(h.calls[0], THUMB);
  await tick();
  const previewState = seen.find(s => s.phase === "full-loading");
  assert.ok(previewState);
  assert.equal(previewState.thumbnailUrl, "data:image/png;base64,dGh1bWI=");
  assert.equal(h.calls.length, 2);
  assert.equal(h.calls[1].source, source);
  assert.equal(h.calls[1].priority, "full");
  resolveCall(h.calls[1], FULL);
  await tick();
  assert.equal(seen[seen.length - 1].phase, "ready");
  assert.equal(seen[seen.length - 1].fullUrl, "data:image/png;base64,ZnVsbA==");
  off();
});

test("a preview identical to the source is one full request", async () => {
  const h = harness();
  const source = "https://example.com/same.png";
  const seen = [];
  const off = h.store.subscribeImage(source, snapshot => seen.push(snapshot), { thumbnailSource: source });
  await tick();
  assert.equal(h.calls.length, 1);
  assert.equal(h.calls[0].source, source);
  assert.equal(h.calls[0].priority, "full");
  resolveCall(h.calls[0], FULL);
  await tick();
  assert.equal(seen[seen.length - 1].phase, "ready");
  off();
});

test("two cards on one source share one download and one subscriber's abort does not cancel it", async () => {
  const h = harness();
  const first = new AbortController();
  const second = new AbortController();
  h.store.subscribeImage("https://example.com/shared.png", () => {}, { signal: first.signal });
  h.store.subscribeImage("https://example.com/shared.png", () => {}, { signal: second.signal });
  await tick();
  assert.equal(h.calls.length, 1);
  first.abort();
  await tick();
  assert.equal(h.cancels.length, 0);
  assert.equal(h.calls.length, 1);
  second.abort();
  await tick();
  assert.equal(h.cancels.length, 1);
  assert.equal(h.cancels[0], h.calls[0].requestId);
  assert.equal(h.store.size, 0);
});

test("cancelling before read admission never invokes the host read", async () => {
  const h = harness();
  const controller = new AbortController();
  h.store.subscribeImage("https://example.com/queued.png", () => {}, { signal: controller.signal });
  controller.abort(); // the read microtask has not run yet
  await tick();
  assert.equal(h.calls.length, 0, "the cancelled queued request must not reach IPC");
  assert.equal(h.cancels.length, 1);
  assert.equal(h.store.size, 0);
});

test("a failed full image keeps the validated preview, never auto-retries, and retries explicitly", async () => {
  const h = harness();
  const source = "https://example.com/full-fails.png";
  const preview = "https://example.com/full-fails-preview.png";
  const seen = [];
  const off = h.store.subscribeImage(source, snapshot => seen.push(snapshot), { thumbnailSource: preview });
  await tick();
  resolveCall(h.calls[0], THUMB);
  await tick();
  h.calls[1].reject(new Error("image exceeds maximum size"));
  await tick();
  const failed = seen[seen.length - 1];
  assert.equal(failed.phase, "thumbnail");
  assert.equal(failed.fullError, "image exceeds maximum size");
  assert.equal(failed.thumbnailUrl, "data:image/png;base64,dGh1bWI=");
  assert.equal(failed.url, "data:image/png;base64,dGh1bWI=");

  // Another card on the same source sees the failure, not a new download.
  const off2 = h.store.subscribeImage(source, () => {});
  await tick();
  assert.equal(h.calls.length, 2);
  off2();

  // Explicit retry starts exactly one fresh full request.
  h.store.retryImage(source);
  await tick();
  assert.equal(h.calls.length, 3);
  assert.equal(h.calls[2].source, source);
  assert.equal(h.calls[2].priority, "full");
  resolveCall(h.calls[2], FULL);
  await tick();
  assert.equal(seen[seen.length - 1].phase, "ready");
  off();
});

test("a failed preview falls back to one full request, then reports a visible error", async () => {
  const h = harness();
  const source = "https://example.com/broken.png";
  const preview = "https://example.com/broken-preview.png";
  const seen = [];
  const off = h.store.subscribeImage(source, snapshot => seen.push(snapshot), { thumbnailSource: preview });
  await tick();
  h.calls[0].reject(new Error("preview unavailable"));
  await tick();
  assert.equal(h.calls.length, 2);
  assert.equal(h.calls[1].source, source);
  assert.equal(h.calls[1].priority, "full");
  h.calls[1].reject(new Error("image file not found"));
  await tick();
  const error = seen[seen.length - 1];
  assert.equal(error.phase, "error");
  assert.equal(error.error, "image file not found");
  assert.equal(error.url, null);
  off();
});

test("independent sources resolve out of order without blocking each other", async () => {
  const h = harness();
  const order = [];
  const offA = h.store.subscribeImage("https://example.com/a.png", s => {
    if (s.phase === "ready") order.push("a");
  });
  const offB = h.store.subscribeImage("https://example.com/b.png", s => {
    if (s.phase === "ready") order.push("b");
  });
  await tick();
  assert.equal(h.calls.length, 2);
  const a = h.calls.find(c => c.source.endsWith("/a.png"));
  const b = h.calls.find(c => c.source.endsWith("/b.png"));
  resolveCall(b, FULL);
  await tick();
  assert.deepEqual(order, ["b"]);
  resolveCall(a, FULL);
  await tick();
  assert.deepEqual(order, ["b", "a"]);
  offA();
  offB();
});

test("the cache evicts the least recently used unpinned image", async () => {
  const h = harness({ maxEntries: 2 });
  async function load(source) {
    const off = h.store.subscribeImage(source, () => {});
    await tick();
    const call = h.calls.find(c => c.source === source);
    resolveCall(call, FULL);
    await tick();
    off();
  }
  await load("https://example.com/1.png");
  await load("https://example.com/2.png");
  await load("https://example.com/3.png");
  assert.equal(h.store.size, 2);
  const before = h.calls.length;
  const off = h.store.subscribeImage("https://example.com/1.png", () => {});
  await tick();
  assert.equal(h.calls.length, before + 1);
  off();
});

test("another conversation downloads the same source separately", async () => {
  const calls = [];
  const cancels = [];
  const makeRead = () => (source, options) => {
    const pending = deferred();
    calls.push({ source, requestId: options.requestId, priority: options.priority, ...pending });
    return pending.promise;
  };
  const a = createStore({ key: "session-a", read: makeRead(), cancel: id => { cancels.push(id); return true; } });
  const b = createStore({ key: "session-b", read: makeRead(), cancel: id => { cancels.push(id); return true; } });
  a.subscribeImage("https://example.com/same.png", () => {});
  b.subscribeImage("https://example.com/same.png", () => {});
  await tick();
  assert.equal(calls.length, 2);
  a.dispose();
  assert.deepEqual(cancels, [calls[0].requestId]);
  assert.equal(b.size, 1);
  b.dispose();
});

test("resolveImage keeps the legacy {data, mimeType} contract with one request", async () => {
  const h = harness();
  const pending = h.store.resolveImage("https://example.com/legacy.png");
  await tick();
  assert.equal(h.calls.length, 1);
  assert.equal(h.calls[0].priority, "full");
  resolveCall(h.calls[0], FULL);
  const result = await pending;
  assert.deepEqual({ ...result }, { data: "ZnVsbA==", mimeType: "image/png" });
});

test("reopening a tab reuses the same backend conversation key", () => {
  const reopened = conversationKey("profile-1", "/sessions/abc.jsonl", "tab-second-mount");
  assert.equal(conversationKey("profile-1", "/sessions/abc.jsonl", "tab-first-mount"), reopened);
  assert.notEqual(conversationKey("profile-2", "/sessions/abc.jsonl", "tab-first-mount"), reopened);
  assert.notEqual(conversationKey("profile-1", null, "tab-first-mount"), conversationKey("profile-1", null, "tab-second-mount"));
});

test("a full image that fails to decode becomes an error with retry, not endless loading", () => {
  const ready = {
    source: "https://example.com/truncated.png",
    phase: "ready",
    url: "data:image/png;base64,iVBORw0KGgo=",
    thumbnailUrl: null,
    fullUrl: "data:image/png;base64,iVBORw0KGgo=",
    error: null,
    fullError: null,
    thumbAt: 10,
    fullAt: 20,
  };
  const failed = deriveImageStatus(ready, { thumbBroken: false, fullBroken: true });
  assert.equal(failed.status, "error");
  assert.equal(failed.displayUrl, null);
  assert.match(failed.error, /decode/);

  const withPreview = deriveImageStatus(
    { ...ready, thumbnailUrl: "data:image/png;base64,dGh1bWI=" },
    { thumbBroken: false, fullBroken: true },
  );
  assert.equal(withPreview.status, "full-error");
  assert.equal(withPreview.displayKind, "thumbnail");
  assert.equal(withPreview.displayUrl, "data:image/png;base64,dGh1bWI=");

  const loading = deriveImageStatus(
    { ...ready, phase: "full-loading", fullUrl: null, thumbnailUrl: "data:image/png;base64,dGh1bWI=" },
    { thumbBroken: true, fullBroken: false },
  );
  assert.equal(loading.status, "loading");
});

test("debug labels never include the query string", () => {
  assert.equal(redactSource("https://example.com/a.png?token=secret#frag"), "https://example.com/a.png");
  assert.equal(redactSource("data:image/png;base64,AAAA"), "data:image/png;base64,…");
  assert.equal(redactSource("images/diagram.png?x=1"), "images/diagram.png");
});
