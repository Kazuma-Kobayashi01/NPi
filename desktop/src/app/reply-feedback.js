// app/reply-feedback.js — 👍/👎 on assistant replies, kept locally so they
// survive a reload (design/chat/reply-feedback.jsx). Dependency-free IIFE,
// `window.OMP_REPLY_FEEDBACK`.
//
// One localStorage key, `npi.replyFeedback`, which app/ui-store.js writes
// through to ui-state.json (a key count and size limited file, so one key
// for every conversation, not one per conversation):
//
//   { "<conversation>": { "<reply ts>": { r: "up" | "down", t?: "<reason>" } } }
//
// <conversation> is the session file's name (`conversationKey`), <reply ts>
// the omp message timestamp the transcript gives every reply (`ts`, stable
// across reloads, unlike the UI's `_id`). The value is kept under MAX_BYTES:
// past that the conversation written longest ago goes first (a lone
// conversation loses its oldest ratings instead). A rating is local; it
// reaches the model only when the user presses "Tell assistant", as an
// ordinary message — the model's own memory tools decide what to keep
// (docs/architecture/memory.md), nothing here writes memory.
//
//   conversationKey(sessionFile)   the key for a tab's conversation, or null
//   createStore(storage)           { get, set, clear, subscribe } over `storage`
//   shared                         the store over window.localStorage (null without one)
//   put / drop / trim              pure edits of the parsed value

(function () {
  const KEY = "npi.replyFeedback";
  const MAX_BYTES = 48 * 1024;
  const MAX_REASON_CHARS = 500;

  const sizeOf = (store) => new TextEncoder().encode(JSON.stringify(store)).length;

  /** The session file's name: its directory differs per profile and machine,
   *  the name (`<time>_<id>.jsonl`) is what identifies the conversation. */
  function conversationKey(sessionFile) {
    if (typeof sessionFile !== "string" || !sessionFile) return null;
    return sessionFile.split(/[\\/]/).pop() || null;
  }

  /** Drops the least recently written conversation (the first key) until
   *  `store` fits `max` bytes; the last conversation sheds its oldest
   *  ratings instead. Returns `store` itself when it already fits. */
  function trim(store, max = MAX_BYTES) {
    if (sizeOf(store) <= max) return store;
    const next = { ...store };
    while (sizeOf(next) > max) {
      const convs = Object.keys(next);
      if (convs.length > 1) { delete next[convs[0]]; continue; }
      const conv = convs[0];
      const replies = { ...next[conv] };
      delete replies[Object.keys(replies)[0]];
      if (Object.keys(replies).length === 0) { delete next[conv]; break; }
      next[conv] = replies;
    }
    return next;
  }

  /** `store` with `entry` ({ r, t? }) on reply `ts` of `conv`; that conversation
   *  and reply become the most recently written. The reason is bounded. */
  function put(store, conv, ts, entry, max = MAX_BYTES) {
    const { [conv]: current = {}, ...others } = store;
    const { [ts]: _old, ...replies } = current;
    const reason = typeof entry.t === "string" ? entry.t.trim().slice(0, MAX_REASON_CHARS) : "";
    const next = { r: entry.r };
    if (reason) next.t = reason;
    return trim({ ...others, [conv]: { ...replies, [ts]: next } }, max);
  }

  /** `store` without the rating of reply `ts` of `conv`. */
  function drop(store, conv, ts) {
    if (!store[conv]?.[ts]) return store;
    const { [ts]: _gone, ...replies } = store[conv];
    const { [conv]: _conv, ...others } = store;
    return Object.keys(replies).length ? { ...others, [conv]: replies } : others;
  }

  /** The stored value of a raw string: an object of objects, else empty. */
  function parse(raw) {
    try {
      const v = JSON.parse(raw);
      return v && typeof v === "object" && !Array.isArray(v) ? v : {};
    } catch { return {}; }
  }

  /** A store over `storage` (a localStorage). `get` returns the same entry
   *  object until it changes, so it is a stable snapshot for
   *  useSyncExternalStore; a change from another window (the shared
   *  localStorage) is seen by the next `get`, and announced to `subscribe`rs
   *  through `notifyExternal`. */
  function createStore(storage) {
    let raw = null;
    let value = {};
    const listeners = new Set();
    const read = () => {
      const now = storage.getItem(KEY);
      if (now !== raw) { raw = now; value = parse(now); }
      return value;
    };
    const write = (next) => {
      if (next === value) return;
      const text = JSON.stringify(next);
      try {
        storage.setItem(KEY, text);
      } catch (err) {
        console.warn("[reply-feedback] not saved:", err);
        return;
      }
      raw = text;
      value = next;
      listeners.forEach(fn => fn());
    };
    return {
      /** `{ r, t? }` for a reply, or undefined. */
      get(conv, ts) { return conv && ts != null ? read()[conv]?.[ts] : undefined; },
      set(conv, ts, entry) { write(put(read(), conv, ts, entry)); },
      clear(conv, ts) { write(drop(read(), conv, ts)); },
      subscribe(fn) { listeners.add(fn); return () => listeners.delete(fn); },
      notifyExternal() { listeners.forEach(fn => fn()); },
    };
  }

  const shared = typeof window.localStorage === "undefined" ? null : createStore(window.localStorage);
  // Another window (the Office, a panel) rating a reply changes the shared localStorage.
  if (shared && window.addEventListener) {
    window.addEventListener("storage", (e) => { if (e.key === KEY || e.key === null) shared.notifyExternal(); });
  }

  window.OMP_REPLY_FEEDBACK = { KEY, MAX_BYTES, MAX_REASON_CHARS, conversationKey, trim, put, drop, parse, createStore, shared };
})();
