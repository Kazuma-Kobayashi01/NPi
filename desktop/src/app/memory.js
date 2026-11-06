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

  const MODE_LABELS = { chat: "Chat", code: "Code", academic: "Academic" };

  /**
   * What the bottom notice says about one change the agent made (`memory://noted` from Rust, or a
   * `manage_skill` result seen in live.js): `{ what, title, text, undo }`, where `undo` is
   * `{ note: id }` or `{ skill: name }` for something new, else null. Null for a payload it does
   * not know.
   */
  function noticeOf(d) {
    if (d?.note?.title) {
      return { what: d.replaced ? "Memory updated" : "Remembered", title: d.note.title, text: d.note.text ?? "", undo: d.replaced ? null : { note: d.note.id } };
    }
    if (typeof d?.profile === "string" && MODE_LABELS[d.profile]) {
      return { what: "Profile updated", title: MODE_LABELS[d.profile], text: d.text ?? "", undo: null };
    }
    if (typeof d?.skill === "string" && d.skill) {
      return { what: d.replaced ? "Skill updated" : "New skill", title: d.skill, text: "", undo: d.replaced ? null : { skill: d.skill } };
    }
    return null;
  }

  /** "1,474 / 3,000": a count of a limit, as the usage line shows it. */
  function share(used, limit) {
    const n = v => Number(v ?? 0).toLocaleString("en-US");
    return `${n(used)} / ${n(limit)}`;
  }

  window.OMP_MEMORY = { grouped, age, noticeOf, share };
})();
