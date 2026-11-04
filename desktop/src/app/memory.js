// app/memory.js — Settings' view of Memory (src-tauri/src/memory/): the notes by kind, and how long ago.
(function () {
  const KINDS = [["you", "You"], ["topic", "Topics"], ["area", "Areas"], ["person", "People"]];

  /** `[{ kind, label, notes }]` in the fixed order, leaving out kinds with no note. */
  function grouped(notes) {
    return KINDS.map(([kind, label]) => ({ kind, label, notes: (notes ?? []).filter(n => n.kind === kind) })).filter(g => g.notes.length > 0);
  }

  /** `{ key, n }`, the i18n key and count for "Updated …" from two times in seconds. */
  function age(then, now) {
    const days = Math.floor(Math.max(0, now - then) / 86400);
    if (days === 0) return { key: "Updated today", n: 0 };
    if (days === 1) return { key: "Updated yesterday", n: 1 };
    return { key: "Updated {n} days ago", n: days };
  }

  window.OMP_MEMORY = { grouped, age };
})();
