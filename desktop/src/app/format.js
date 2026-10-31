// Small display formatters shared by the chat views. Exposes `window.OMP_FORMAT`;
// an IIFE (AGENTS.md "IIFE rule"), dependency-free.
(function () {
  /** `1536` → `2 KB`, `3 * 1024 * 1024` → `3.0 MB`; `—` for an unknown size. */
  function formatBytes(n) {
    if (!Number.isFinite(n) || n < 0) return "—";
    if (n < 1024) return `${n} B`;
    if (n < 1024 * 1024) return `${(n / 1024).toFixed(0)} KB`;
    return `${(n / (1024 * 1024)).toFixed(1)} MB`;
  }

  window.OMP_FORMAT = { formatBytes };
})();
