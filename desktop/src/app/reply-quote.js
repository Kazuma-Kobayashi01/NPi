// app/reply-quote.js — text helpers for the assistant reply bubble's
// selection toolbar (Quote / Ask / Steer) and its feedback follow-up
// (design/chat/selection-toolbar.jsx, design/chat/reply-feedback.jsx).
// Pure and dependency-free (IIFE, `window.OMP_REPLY_QUOTE`).
//
//   MAX_QUOTE_CHARS            longest quote that is kept
//   clip(text, max?)           the selection normalised and bounded, "…" when cut
//   blockquote(text)           `clip`ped text as a Markdown blockquote
//   withQuote(quote, note)     `> quote` + blank line + the user's own words
//   appendBlock(draft, block)  `block` after the composer draft, as its own paragraph
//   feedbackMessage({reason, excerpt?})
//                              the "Tell assistant" message of a 👎

(function () {
  const MAX_QUOTE_CHARS = 2000;
  const EXCERPT_CHARS = 200;

  /** `text` as the DOM selection gave it: LF line ends, no trailing blanks,
   *  at most one empty line in a row, at most `max` characters (the last one
   *  an ellipsis when the text was cut). */
  function clip(text, max = MAX_QUOTE_CHARS) {
    const clean = String(text ?? "")
      .replace(/\r\n?/g, "\n")
      .replace(/\u00a0/g, " ")
      .replace(/[ \t]+$/gm, "")
      .replace(/\n{3,}/g, "\n\n")
      .trim();
    if (clean.length <= max) return clean;
    let end = max - 1;
    // Never cut between the halves of a surrogate pair.
    const last = clean.charCodeAt(end - 1);
    if (last >= 0xd800 && last <= 0xdbff) end -= 1;
    return clean.slice(0, end).trimEnd() + "…";
  }

  /** Every line of the clipped `text` behind `> ` (empty lines as `>`);
   *  "" when nothing is left of it. */
  function blockquote(text, max = MAX_QUOTE_CHARS) {
    const clean = clip(text, max);
    if (!clean) return "";
    return clean.split("\n").map(line => (line ? `> ${line}` : ">")).join("\n");
  }

  /** The message Ask and Steer send: the quote, then what the user typed. */
  function withQuote(quote, note) {
    return `${blockquote(quote)}\n\n${String(note ?? "").trim()}`;
  }

  /** `block` after the draft as a paragraph of its own, with a blank line
   *  after it so typing continues below the quote. */
  function appendBlock(draft, block) {
    const base = String(draft ?? "").replace(/\s+$/, "");
    return base ? `${base}\n\n${block}\n\n` : `${block}\n\n`;
  }

  /** What "Tell assistant" sends for a 👎. `excerpt` (the rated reply's
   *  opening) is given when that reply is not the latest one, so "previous"
   *  cannot be read as the wrong reply. */
  function feedbackMessage({ reason, excerpt = "" }) {
    const why = String(reason ?? "").trim();
    const quote = blockquote(excerpt, EXCERPT_CHARS);
    if (!quote) return `Feedback on your previous reply: ${why}`;
    return `Feedback on your earlier reply:\n${quote}\n\n${why}`;
  }

  window.OMP_REPLY_QUOTE = { MAX_QUOTE_CHARS, EXCERPT_CHARS, clip, blockquote, withQuote, appendBlock, feedbackMessage };
})();
