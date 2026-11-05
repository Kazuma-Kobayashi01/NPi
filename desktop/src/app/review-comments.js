// Review comments on the git browser's diff (design/git/file-view.jsx): notes on
// lines that go to the Agent together. Kept per project in memory, so they
// outlive the overlay but not the app. Pure helpers, `window.OMP_REVIEW`.
(function () {
  const store = new Map();

  function load(project) { return store.get(project) ?? []; }
  function save(project, comments) { store.set(project, comments); }

  /** `comments` with a note on `line` (a `Line` of a FileView) of file `path`. */
  function add(comments, path, line, text) {
    const id = comments.reduce((n, c) => Math.max(n, c.id), 0) + 1;
    return [...comments, { id, path, old: line.old ?? null, new: line.new ?? null, kind: line.kind, code: line.text, text }];
  }

  function remove(comments, id) { return comments.filter(c => c.id !== id); }

  /** Whether `comment` sits on `line` of `path` as it is now shown. */
  function isOn(comment, path, line) {
    return comment.path === path && comment.old === (line.old ?? null)
      && comment.new === (line.new ?? null) && comment.code === line.text;
  }

  /** The prompt that carries `comments`, in file and line order. */
  function format(comments) {
    const number = c => c.new ?? c.old;
    const sorted = [...comments].sort((a, b) => a.path.localeCompare(b.path) || number(a) - number(b));
    const items = sorted.map((c, i) => {
      const where = `${c.path}:${number(c)}${c.kind === "rem" ? " (removed line)" : ""}`;
      return `${i + 1}. \`${where}\`\n   > ${c.code.trim()}\n   ${c.text.replace(/\n/g, "\n   ")}`;
    });
    return `Review comments on the current diff. Address each one:\n\n${items.join("\n\n")}`;
  }

  window.OMP_REVIEW = { load, save, add, remove, isOn, format };
})();
