// app/review-comments.js: the review comments of the git browser.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

globalThis.window = {};
new Function(readFileSync(new URL("../src/app/review-comments.js", import.meta.url), "utf8"))();
const R = window.OMP_REVIEW;

const add = { kind: "add", old: null, new: 12, text: "let x = 1;" };
const rem = { kind: "rem", old: 3, new: null, text: "  gone()" };

let list = R.add([], "b.rs", add, "use a const");
list = R.add(list, "a.rs", rem, "why remove this?\nIt is used.");
assert.deepEqual(list.map(c => c.id), [1, 2]);

assert.equal(R.isOn(list[0], "b.rs", add), true);
assert.equal(R.isOn(list[0], "a.rs", add), false);
assert.equal(R.isOn(list[0], "b.rs", { ...add, text: "let x = 2;" }), false, "a changed line loses its comment");

assert.equal(
  R.format(list),
  "Review comments on the current diff. Address each one:\n\n"
    + "1. `a.rs:3 (removed line)`\n   > gone()\n   why remove this?\n   It is used.\n\n"
    + "2. `b.rs:12`\n   > let x = 1;\n   use a const",
);

assert.deepEqual(R.remove(list, 1).map(c => c.id), [2]);
assert.equal(R.add(R.remove(list, 1), "c", add, "x").at(-1).id, 3);

R.save("/p", list);
assert.equal(R.load("/p"), list);
assert.deepEqual(R.load("/other"), []);
console.log("review comments: ok");
