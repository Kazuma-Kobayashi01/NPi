#!/usr/bin/env node
// Regression script for src/app/reply-quote.js — the text helpers of the
// assistant reply's selection toolbar (Quote / Ask / Steer) and of the 👎
// "Tell assistant" message.
// Run: node tests/test-reply-quote.mjs

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const win = {};
// eslint-disable-next-line no-new-func
new Function("window", readFileSync(join(root, "src/app/reply-quote.js"), "utf8"))(win);
const Q = win.OMP_REPLY_QUOTE;

let passed = 0;
function check(label, fn) {
  try {
    fn();
  } catch (err) {
    console.error(`FAILED: ${label}`);
    throw err;
  }
  passed++;
}

check("clip normalises what a DOM selection hands over", () => {
  assert.equal(Q.clip("  one\r\ntwo  \r\n\r\n\r\n\r\nthree\u00a0 \n"), "one\ntwo\n\nthree");
  assert.equal(Q.clip("a\rb"), "a\nb");
  assert.equal(Q.clip(""), "");
  assert.equal(Q.clip(null), "");
  assert.equal(Q.clip(" \n\t "), "");
});

check("a quote within the bound is kept whole", () => {
  const exact = "x".repeat(Q.MAX_QUOTE_CHARS);
  assert.equal(Q.clip(exact), exact);
});

check("a longer selection is cut to the bound, ending in an ellipsis", () => {
  const long = "word ".repeat(1000);
  const out = Q.clip(long);
  assert.ok(out.length <= Q.MAX_QUOTE_CHARS);
  assert.ok(out.endsWith("…"));
  assert.ok(!out.endsWith(" …"), "no space before the ellipsis");
  assert.equal(Q.clip("x".repeat(Q.MAX_QUOTE_CHARS + 1)), "x".repeat(Q.MAX_QUOTE_CHARS - 1) + "…");
  assert.equal(Q.clip("abcdef", 4), "abc…");
});

check("the cut never splits a surrogate pair", () => {
  const out = Q.clip("😀".repeat(1500));
  assert.ok(out.endsWith("…"));
  assert.ok(out.length <= Q.MAX_QUOTE_CHARS);
  assert.ok(!/[\ud800-\udbff](?![\udc00-\udfff])/.test(out), "no lone high surrogate");
  assert.ok(!/(?<![\ud800-\udbff])[\udc00-\udfff]/.test(out), "no lone low surrogate");
});

check("blockquote prefixes every line, empty lines as a bare `>`", () => {
  assert.equal(Q.blockquote("a\n\nb"), "> a\n>\n> b");
  assert.equal(Q.blockquote("single"), "> single");
  assert.equal(Q.blockquote("  \n "), "");
  assert.equal(Q.blockquote("x".repeat(5000)).length, 2 + Q.MAX_QUOTE_CHARS);
});

check("blockquote of an already clipped quote does not change it", () => {
  const once = Q.blockquote("y".repeat(5000));
  assert.equal(Q.blockquote(once.slice(2)), once);
});

check("withQuote is the quote, a blank line, then the typed words", () => {
  assert.equal(Q.withQuote("line one\nline two", "  why?  "), "> line one\n> line two\n\nwhy?");
});

check("appendBlock puts the quote after the draft as its own paragraph", () => {
  assert.equal(Q.appendBlock("", "> q"), "> q\n\n");
  assert.equal(Q.appendBlock("  \n", "> q"), "> q\n\n");
  assert.equal(Q.appendBlock("typed so far", "> q"), "typed so far\n\n> q\n\n");
  assert.equal(Q.appendBlock("typed so far \n\n", "> q"), "typed so far\n\n> q\n\n");
  assert.equal(Q.appendBlock(undefined, "> q"), "> q\n\n");
});

check("feedbackMessage for the latest reply is the plain sentence", () => {
  assert.equal(Q.feedbackMessage({ reason: "  too long  " }), "Feedback on your previous reply: too long");
  assert.equal(Q.feedbackMessage({ reason: "too long", excerpt: "" }), "Feedback on your previous reply: too long");
});

check("feedbackMessage for an older reply quotes its opening, bounded", () => {
  assert.equal(
    Q.feedbackMessage({ reason: "wrong file", excerpt: "I edited a.js\nand b.js" }),
    "Feedback on your earlier reply:\n> I edited a.js\n> and b.js\n\nwrong file",
  );
  const msg = Q.feedbackMessage({ reason: "r", excerpt: "z".repeat(900) });
  const quoted = msg.split("\n")[1];
  assert.equal(quoted.length, 2 + Q.EXCERPT_CHARS);
  assert.ok(quoted.endsWith("…"));
});

console.log(`reply-quote: ${passed} checks passed`);
