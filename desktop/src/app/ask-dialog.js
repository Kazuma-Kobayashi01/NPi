/* ═════════════════════════════════════════════════════════════════════
   app/ask-dialog.js — omp's ask dialog. Pure functions only; live.js
   keeps the request on its ask message, design/chat/ask-dialog.jsx
   renders it and holds the picks not sent yet.

   Once `set_ask_dialog` is enabled, every question of one ask tool call
   arrives as a single `extension_ui_request` with `method: "ask"`, and
   the reply is one `extension_ui_response` carrying `answers`. omp checks
   that reply and fails the ask tool on any mismatch instead of guessing
   (rpc-mode.ts `parseAskDialogResponse`):
     - one answer per question, in request order, `id` equal to the
       question's;
     - `selectedOptions` holds exact option labels, none twice;
     - a pick-one question (`multi` absent or false) carries at most one
       option, and never both an option and `customInput`;
     - `customInput` is trimmed, and ignored when empty.
   A pick-any question may be answered with nothing picked. A lone pick-one
   question answered with nothing counts as a cancel and stops the turn,
   so the dialog never sends one.
   ═════════════════════════════════════════════════════════════════════ */
(function () {
  function nonBlank(value) {
    return typeof value === "string" && value.trim() ? value : null;
  }

  /** The questions of an `ask` request, or null when they do not have the
   *  shape omp documents — live.js then cancels the request, so omp never
   *  waits on a dialog nothing can answer. `recommended` is kept only when
   *  it indexes an option. */
  function parseQuestions(raw) {
    if (!Array.isArray(raw) || raw.length === 0) return null;
    const questions = [];
    for (const q of raw) {
      if (!q || typeof q.id !== "string" || typeof q.question !== "string" || !Array.isArray(q.options)) return null;
      const options = [];
      for (const o of q.options) {
        if (!o || typeof o.label !== "string") return null;
        options.push({ label: o.label, description: nonBlank(o.description), preview: nonBlank(o.preview) });
      }
      const rec = q.recommended;
      questions.push({
        id: q.id,
        question: q.question,
        header: nonBlank(q.header)?.trim() ?? null,
        options,
        multi: q.multi === true,
        recommended: Number.isInteger(rec) && rec >= 0 && rec < options.length ? rec : null,
      });
    }
    return questions;
  }

  /** Nothing answered yet: per question, the picked option indexes
   *  (ascending) and the typed text. */
  function emptyDraft(questions) {
    return questions.map(() => ({ picks: [], text: "" }));
  }

  function replaceAt(draft, index, entry) {
    return draft.map((e, i) => (i === index ? entry : e));
  }

  /** A click on option `oi` of question `qi`. Pick-one: the option replaces
   *  the earlier pick and any typed text, since omp takes one or the other.
   *  Pick-any: toggles it. */
  function pick(draft, questions, qi, oi) {
    const entry = draft[qi];
    if (!questions[qi].multi) return replaceAt(draft, qi, { picks: [oi], text: "" });
    const picks = entry.picks.includes(oi)
      ? entry.picks.filter(i => i !== oi)
      : [...entry.picks, oi].sort((a, b) => a - b);
    return replaceAt(draft, qi, { ...entry, picks });
  }

  /** Typed text for question `qi`. On a pick-one question, text that is not
   *  blank replaces the picked option. */
  function type(draft, questions, qi, text) {
    const picks = !questions[qi].multi && text.trim() ? [] : draft[qi].picks;
    return replaceAt(draft, qi, { picks, text });
  }

  /** A pick-one question with neither a pick nor typed text. */
  function unanswered(q, entry) {
    return !q.multi && entry.picks.length === 0 && !entry.text.trim();
  }

  /** How many questions are `unanswered`. */
  function missing(questions, draft) {
    return questions.filter((q, i) => unanswered(q, draft[i])).length;
  }

  /** A lone pick-one question with options answers on click; everything
   *  else waits for Submit. */
  function answersOnClick(questions) {
    return questions.length === 1 && !questions[0].multi && questions[0].options.length > 0;
  }

  /** The `answers` of the reply to omp. */
  function answers(questions, draft) {
    return questions.map((q, i) => {
      const e = draft[i];
      const custom = e.text.trim();
      const picks = q.multi ? e.picks : custom ? [] : e.picks.slice(0, 1);
      const selectedOptions = picks.map(oi => q.options[oi].label);
      return custom ? { id: q.id, selectedOptions, customInput: custom } : { id: q.id, selectedOptions };
    });
  }

  /** omp's ask timeout (ms) as "45 s", "2 min" or "1 min 30 s"; null when
   *  there is none. */
  function timeoutLabel(ms) {
    if (!Number.isFinite(ms) || ms <= 0) return null;
    const s = Math.max(1, Math.round(ms / 1000));
    if (s < 60) return `${s} s`;
    const min = Math.floor(s / 60);
    const rest = s % 60;
    return rest ? `${min} min ${rest} s` : `${min} min`;
  }

  /** What omp does once its ask timeout (ms) runs out, or null when there
   *  is none: it picks the recommended options when every question has one,
   *  and otherwise answers on its own. */
  function timeoutNote(questions, ms) {
    const after = timeoutLabel(ms);
    if (!after) return null;
    if (!questions.every(q => q.recommended !== null)) return `omp answers for you after ${after}`;
    return `omp picks the recommended option${questions.length > 1 ? "s" : ""} after ${after}`;
  }

  /** `text` split into plain runs and `code` runs (a backtick pair). Option
   *  labels and descriptions render as text, not markdown: they sit inside
   *  the option's button. An unpaired backtick stays literal. */
  function codeRuns(text) {
    const runs = [];
    const re = /`([^`\n]+)`/g;
    let last = 0;
    for (let m = re.exec(text); m; m = re.exec(text)) {
      if (m.index > last) runs.push({ code: false, text: text.slice(last, m.index) });
      runs.push({ code: true, text: m[1] });
      last = re.lastIndex;
    }
    if (last < text.length) runs.push({ code: false, text: text.slice(last) });
    return runs;
  }

  /* ── The subagent preferences ask ────────────────────────────────────
     omp's Auto subagent mode asks which model and thinking level a batch
     uses: an ask call with exactly two pick-one questions, `model` (every
     configured model, the current one `recommended`) and `thinking` (omp's
     levels). The frames carry no tool call id, so it is recognised by those
     two ids and the level labels. The dialog shows it as one form (a
     searchable model list, a pill row of levels) but answers through the same
     draft and `answers()` as the generic dialog, so omp sees the same reply. */
  const THINKING_LEVELS = ["off", "minimal", "low", "medium", "high", "xhigh", "max"];

  /** `{ model, thinking }` question indexes when `questions` is the subagent
   *  preferences ask, else null. */
  function subagentForm(questions) {
    if (!Array.isArray(questions) || questions.length !== 2) return null;
    const model = questions.findIndex(q => q.id === "model");
    const thinking = questions.findIndex(q => q.id === "thinking");
    if (model < 0 || thinking < 0) return null;
    const m = questions[model];
    const t = questions[thinking];
    if (m.multi || t.multi || m.options.length === 0 || t.options.length === 0) return null;
    if (!t.options.every(o => THINKING_LEVELS.includes(o.label))) return null;
    return { model, thinking };
  }

  /** Model option indexes in display order: the recommended (current) model
   *  first, then the one picked last time, then the rest as omp sent them. */
  function modelOrder(q, last) {
    const all = q.options.map((_, i) => i);
    const lastIndex = last ? q.options.findIndex(o => o.label === last) : -1;
    const head = [q.recommended, lastIndex].filter((v, i, a) => Number.isInteger(v) && v >= 0 && a.indexOf(v) === i);
    return [...head, ...all.filter(i => !head.includes(i))];
  }

  /** The indexes of `order` whose label or description holds every word of `query`. */
  function filterModels(q, order, query) {
    const words = String(query ?? "").toLowerCase().split(/\s+/).filter(Boolean);
    if (words.length === 0) return order;
    return order.filter(i => {
      const o = q.options[i];
      const hay = `${o.label} ${o.description ?? ""}`.toLowerCase();
      return words.every(w => hay.includes(w));
    });
  }

  /** The form's first draft: the recommended option of each question, else
   *  what was picked last time (`last` = `{ model, thinking }` labels), else
   *  the first model and the `medium` level (or the first level). */
  function subagentDraft(questions, form, last) {
    const draft = emptyDraft(questions);
    for (const key of ["model", "thinking"]) {
      const qi = form[key];
      const q = questions[qi];
      const lastIndex = last?.[key] ? q.options.findIndex(o => o.label === last[key]) : -1;
      const fallback = key === "thinking" ? Math.max(0, q.options.findIndex(o => o.label === "medium")) : 0;
      const oi = q.recommended ?? (lastIndex >= 0 ? lastIndex : fallback);
      draft[qi] = { picks: [oi], text: "" };
    }
    return draft;
  }

  /** What to remember for next time: the labels picked (typed models are not). */
  function subagentMemory(questions, form, draft) {
    const label = key => {
      const e = draft[form[key]];
      return e.text.trim() || e.picks.length === 0 ? null : questions[form[key]].options[e.picks[0]].label;
    };
    return { model: label("model"), thinking: label("thinking") };
  }

  const MEMORY_KEY = "omp.askSubagent.v1";

  /** The model and level picked last time as `{ model, thinking }` labels, or
   *  null. A preselection only; storage can be missing or blocked. */
  function readSubagentMemory(storage) {
    try {
      const raw = (storage ?? globalThis.localStorage)?.getItem(MEMORY_KEY);
      const v = raw ? JSON.parse(raw) : null;
      return v && typeof v === "object" ? { model: typeof v.model === "string" ? v.model : null, thinking: typeof v.thinking === "string" ? v.thinking : null } : null;
    } catch { return null; }
  }

  function saveSubagentMemory(memory, storage) {
    try { (storage ?? globalThis.localStorage)?.setItem(MEMORY_KEY, JSON.stringify(memory)); } catch { /* a preselection only */ }
  }

  /** The draft with what is typed in the model box applied: a name that is
   *  (or, alone, matches) one model picks it, anything else is the typed
   *  answer omp already accepts. `query` null or blank changes nothing. */
  function applyModelQuery(questions, form, draft, query) {
    const text = String(query ?? "").trim();
    if (!text) return draft;
    const q = questions[form.model];
    const exact = q.options.findIndex(o => o.label.toLowerCase() === text.toLowerCase());
    if (exact >= 0) return pick(draft, questions, form.model, exact);
    const matches = filterModels(q, q.options.map((_, i) => i), text);
    return matches.length === 1 ? pick(draft, questions, form.model, matches[0]) : type(draft, questions, form.model, text);
  }

  window.OMP_ASK_DIALOG = {
    readSubagentMemory, saveSubagentMemory, applyModelQuery,
    subagentForm, modelOrder, filterModels, subagentDraft, subagentMemory, THINKING_LEVELS,
    parseQuestions, emptyDraft, pick, type, unanswered, missing, answersOnClick, answers, timeoutLabel, timeoutNote,
    codeRuns,
  };
})();
