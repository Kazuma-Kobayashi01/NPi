// app/ui-store.js — keeps what pages put in localStorage (the tweaks, small
// per-window choices) also in ~/.local/share/npi/desktop/ui-state.json, beside
// the command line's data (src-tauri/src/ui_state.rs). localStorage stays the
// live copy all windows share; the file is the lasting one.
//
// Every page starts with the file's contents in window.__NPI_UI_STATE__ (an
// init script built at launch). On load a key localStorage lacks is put back
// from it (WebKit's store was wiped, or this is the first run after the move),
// and a key localStorage has that the file lacks or holds differently is sent
// to Rust: localStorage wins, so a window opened later with the launch-time
// copy only fills gaps. After that every setItem/removeItem on localStorage is
// written through, batched for a moment so a dragged slider is one write.
//
// Loaded before every module that reads localStorage (src/index.html).

(function () {
  const FLUSH_MS = 300;

  /**
   * What to do on load: `restore` (into localStorage) and `upload` (to Rust).
   * `snapshot` and `stored` are plain { key: value } objects.
   */
  function plan(snapshot, stored) {
    const restore = {};
    const upload = {};
    for (const [key, value] of Object.entries(snapshot || {})) {
      if (!(key in stored)) restore[key] = value;
    }
    for (const [key, value] of Object.entries(stored)) {
      if (!snapshot || snapshot[key] !== value) upload[key] = value;
    }
    return { restore, upload };
  }

  /** localStorage as a plain object. */
  function readAll(storage) {
    const out = {};
    for (let i = 0; i < storage.length; i++) {
      const key = storage.key(i);
      if (key !== null) out[key] = storage.getItem(key);
    }
    return out;
  }

  function install(win) {
    let storage;
    try { storage = win.localStorage; } catch { return; }
    const invoke = win.__TAURI__ && win.__TAURI__.core && win.__TAURI__.core.invoke;
    if (!storage || !invoke || !win.Storage) return;

    let pending = null;
    const flush = () => {
      const entries = pending;
      pending = null;
      invoke("ui_state_set", { entries }).catch(err => console.warn("[ui-store] not saved:", err));
    };
    const queue = (key, value) => {
      if (!pending) {
        pending = {};
        setTimeout(flush, FLUSH_MS);
      }
      pending[key] = value;
    };

    const { restore, upload } = plan(win.__NPI_UI_STATE__, readAll(storage));
    for (const [key, value] of Object.entries(restore)) {
      try { storage.setItem(key, value); } catch { /* full or blocked: stays in the file */ }
    }
    for (const [key, value] of Object.entries(upload)) queue(key, value);

    const proto = win.Storage.prototype;
    const setItem = proto.setItem;
    const removeItem = proto.removeItem;
    const clear = proto.clear;
    proto.setItem = function (key, value) {
      setItem.call(this, key, value);
      if (this === storage) queue(String(key), String(value));
    };
    proto.removeItem = function (key) {
      removeItem.call(this, key);
      if (this === storage) queue(String(key), null);
    };
    proto.clear = function () {
      const keys = this === storage ? Object.keys(readAll(storage)) : [];
      clear.call(this);
      for (const key of keys) queue(key, null);
    };
  }

  install(window);
  window.OMP_UI_STORE = { plan };
})();
