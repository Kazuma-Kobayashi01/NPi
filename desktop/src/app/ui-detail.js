/* app/ui-detail.js — Settings → Appearance → "Interface detail": five steps
   from the barest window to the fullest. A step is a bundle of existing
   tweaks (`patch`) plus a class on <html> (`detail-N`, layout/shell.css) that
   hides the prompt box's extra pills in the lower steps. Picking a step writes
   its patch in one go; the tweaks stay individually changeable afterwards. */
(function () {
  const LEVELS = Object.freeze([
    { level: 1, name: "Minimal", note: "No side panel; each turn's thinking and tool calls fold into one line; only the model under the prompt.",
      patch: { layout: "focus", foldWork: "box", foldThinking: "folded", foldTools: "folded", showSubagents: false, showChanges: false } },
    { level: 2, name: "Simple", note: "No side panel; thinking and tool calls fold into one line per turn; Plan mode stays.",
      patch: { layout: "focus", foldWork: "box", foldThinking: "folded", foldTools: "folded", showSubagents: false, showChanges: false } },
    { level: 3, name: "Standard", note: "Side panel on; each turn's thinking and tool calls fold into one line you can open.",
      patch: { layout: "rail", foldWork: "box", foldThinking: "folded", foldTools: "folded", showSubagents: true, showChanges: false } },
    { level: 4, name: "Detailed", note: "Thinking and tool calls as separate rows; thinking stays open, tool output folds by itself.",
      patch: { layout: "rail", foldWork: "rows", foldThinking: "open", foldTools: "auto", showSubagents: true, showChanges: false } },
    { level: 5, name: "Full", note: "Separate rows, everything open, with the Changes card.",
      patch: { layout: "rail", foldWork: "rows", foldThinking: "open", foldTools: "open", showSubagents: true, showChanges: true } },
  ]);
  const DEFAULT = 3;
  const clamp = (n) => { const v = Math.round(Number(n)); return Number.isFinite(v) ? Math.max(1, Math.min(5, v)) : DEFAULT; };
  const levelOf = (n) => LEVELS[clamp(n) - 1];
  window.OMP_UI_DETAIL = { LEVELS, DEFAULT, clamp, levelOf };
})();
