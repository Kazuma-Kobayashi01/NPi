// app/compaction.js — the transcript's compaction row (`kind: "compact"`).
//
// A row goes in as `pending` when a compaction starts (the `compact()` call,
// or omp's `auto_compaction_start` / `manual_compaction_start`) and is
// settled by whichever end arrives first: the `*_compaction_end` event or
// the `compact` response. omp ends a pass four ways, and only one of them is
// a failure:
//   - committed (`result`)               → done, with the summary
//   - nothing to compact (`skipped`)     → skipped, with omp's reason; an
//                                          automatic pass skipped this way
//                                          made no change, so its row goes
//   - cancelled (`aborted`, or a hook)   → cancelled
//   - anything else                      → error, with omp's message
(function () {
  const REASON_MAX = 500;

  function reasonOf(text) {
    return typeof text === "string" && text.trim() ? text.trim().slice(0, REASON_MAX) : null;
  }

  /** The fields a finished compaction gives its pending row, or `null` when
   *  the row should be removed. `auto` marks an `auto_compaction_end`. */
  function outcome({ result, aborted, skipped, errorMessage, auto } = {}) {
    if (result) {
      return {
        status: "done",
        summary: result.summary || null,
        shortSummary: result.shortSummary || null,
        tokensBefore: result.tokensBefore,
        reason: null,
      };
    }
    if (skipped && auto) return null;
    const reason = reasonOf(errorMessage);
    if (skipped) return { status: "skipped", reason };
    if (aborted) return { status: "cancelled", reason };
    return { status: "error", reason };
  }

  /** `messages` with its last pending compaction row settled by `fields`
   *  (from `outcome`); unchanged when no row is pending. */
  function settle(messages, fields) {
    let idx = -1;
    for (let i = messages.length - 1; i >= 0; i--) {
      if (messages[i].kind === "compact" && messages[i].status === "pending") { idx = i; break; }
    }
    if (idx === -1) return messages;
    if (fields === null) return messages.filter((_, i) => i !== idx);
    return messages.map((m, i) => (i === idx ? { ...m, ...fields } : m));
  }

  /** Every pending compaction row of `messages` marked as interrupted: the
   *  process that ran it is gone, so no end will ever arrive. */
  function interruptPending(messages) {
    if (!messages.some(m => m.kind === "compact" && m.status === "pending")) return messages;
    return messages.map(m => (m.kind === "compact" && m.status === "pending" ? { ...m, status: "cancelled", reason: null } : m));
  }

  window.OMP_COMPACTION = { outcome, settle, interruptPending };
})();
