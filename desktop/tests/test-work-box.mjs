// app/work-box.js: a turn's thinking and tool calls fold into one box.
import assert from "node:assert/strict";
import fs from "node:fs";
import { fileURLToPath } from "node:url";

const win = {};
const load = f => new Function("window", fs.readFileSync(fileURLToPath(new URL(`../src/app/${f}`, import.meta.url)), "utf8"))(win);
load("i18n.js"); load("i18n/ui.js"); win.OMP_I18N.setLocale("zh-CN");
load("ui-detail.js"); load("tool-groups.js"); load("work-box.js"); load("chat-collapse.js");
const W = win.OMP_WORK_BOX, C = win.OMP_COLLAPSE;
let n = 0;
const check = (name, fn) => { try { fn(); n++; } catch (e) { console.error(`FAILED: ${name}`); throw e; } };
const tool = (id, extra = {}) => ({ _id: id, kind: "tool", tool: "read", title: `T${id}`, status: "ok", ...extra });
const asst = (id, extra = {}) => ({ _id: id, kind: "assistant", blocks: [], ...extra });
const text = t => [{ type: "text", text: t }];
const shape = rows => rows.map(r => r.type === "work" ? `work[${r.items.map(i => i.kind[0] + i.msg._id).join(",")}]` : `${r.msg.kind}${r.msg._id}${r.thoughtInBox ? "*" : ""}`);

check("thinking-only replies and tool calls between two texts become one box", () => {
  const msgs = [
    { _id: 1, kind: "user" },
    asst(2, { thought: "plan" }), tool(3), tool(4), asst(5, { thought: "next" }), tool(6),
    asst(7, { thought: "done", blocks: text("Answer") }),
  ];
  assert.deepEqual(shape(W.groupWork(msgs)), ["user1", "work[t2,t3,t4,t5,t6,t7]", "assistant7*"]);
});

check("a reply that is only punctuation between tool calls is filler and joins the box", () => {
  for (const filler of [".", "…", "。", " . ", "..."]) {
    const msgs = [{ _id: 1, kind: "user" }, tool(2), asst(3, { blocks: text(filler) }), tool(4), asst(5, { blocks: text("Answer") })];
    assert.deepEqual(shape(W.groupWork(msgs)), ["user1", "work[t2,t4]", "assistant5"], JSON.stringify(filler));
  }
  for (const real of ["OK", "好", "1", "😀", "```js\nx\n```", "- item"]) assert.ok(W.hasSubstance(real), real);
  assert.ok(!W.hasSubstance(".") && !W.hasSubstance("") && !W.hasSubstance(undefined));
});
check("every record is kept in order and the messages are untouched", () => {
  const msgs = [asst(1, { thought: "a" }), tool(2), asst(3, { blocks: text("x") }), tool(4)];
  const before = JSON.stringify(msgs);
  const rows = W.groupWork(msgs);
  assert.equal(JSON.stringify(msgs), before);
  assert.deepEqual(rows.flatMap(r => r.type === "work" ? r.indices : [r.idx]), [0, 1, 2, 3]);
  assert.equal(rows[0].key, "work:1");
  assert.equal(rows[2].key, "work:4");
});

check("a failed tool, approval, ask, UI and a failed reply stay outside the box", () => {
  for (const out of [tool(3, { isError: true }), { _id: 3, kind: "approval" }, { _id: 3, kind: "ask" }, { _id: 3, kind: "ui" },
    { _id: 3, kind: "retry" }, asst(3, { failure: { headline: "x" } })]) {
    const rows = W.groupWork([tool(1), tool(2), out, tool(4)]);
    assert.equal(rows.length, 3, out.kind);
    assert.equal(rows[0].type, "work");
    assert.equal(rows[1].msg._id, 3);
    assert.equal(rows[2].type, "work");
  }
});

check("a reply with nothing to show is left out; one with only an image counts as visible", () => {
  assert.deepEqual(shape(W.groupWork([asst(1), tool(2)])), ["work[t2]"]);
  assert.deepEqual(shape(W.groupWork([asst(1, { blocks: [{ type: "text", text: "  " }] })])), []);
  assert.deepEqual(shape(W.groupWork([asst(1, { blocks: [{ type: "image" }] })])), ["assistant1"]);
});

check("hidden messages are skipped", () => {
  const rows = W.groupWork([tool(1), tool(2, { superseded: true }), tool(3)], { hidden: m => !!m.superseded });
  assert.deepEqual(rows[0].indices, [0, 2]);
});

check("the finished line counts thinking and tools; the live line says what is happening", () => {
  const done = W.summarize(W.groupWork([asst(1, { thought: "a" }), tool(2), tool(3)])[0].items);
  assert.deepEqual([done.label, done.live, done.tools], ["已思考 · 用了 2 个工具", false, 2]);
  assert.equal(W.summarize(W.groupWork([tool(1), tool(2)])[0].items).label, "用了 2 个工具");
  assert.equal(W.summarize(W.groupWork([asst(1, { thought: "a" })])[0].items).label, "已思考");
  const thinking = W.summarize(W.groupWork([tool(1), asst(2, { thought: "a", streaming: true })])[0].items);
  assert.deepEqual([thinking.live, thinking.liveLabel], [true, "思考中"]);
  const running = W.summarize(W.groupWork([asst(1, { thought: "a" }), tool(2, { status: "running", title: "Reading x" })])[0].items);
  assert.deepEqual([running.live, running.liveLabel, running.current], [true, "Reading x", "Reading x"]);
});

check("a box is folded until the user opens it", () => {
  assert.equal(W.isCollapsed("work:1", {}), true);
  assert.equal(W.isCollapsed("work:1", { "work:1": false }), false);
  assert.deepEqual(W.groupOverrides([tool(1), asst(2, { blocks: text("x") }), tool(3)], false), { "work:1": false, "work:3": false });
});

check("the work mode follows the detail step, and an explicit choice wins", () => {
  assert.equal(C.foldModes({}).work, "box");
  assert.equal(C.foldModes({ uiDetail: 4 }).work, "rows");
  assert.equal(C.foldModes({ uiDetail: 5 }).work, "rows");
  assert.equal(C.foldModes({ uiDetail: 5, foldWork: "box" }).work, "box");
  assert.equal(C.foldModes({ uiDetail: 2, foldWork: "junk" }).work, "box");
});

check("Collapse all / Expand all toggles boxes in box mode and tool groups in rows mode", () => {
  const msgs = [tool(1), tool(2), asst(3, { blocks: text("x") })];
  assert.ok(Object.hasOwn(C.overridesForAll(msgs, false, { work: "box" }), "work:1"));
  assert.ok(Object.hasOwn(C.overridesForAll(msgs, false, { work: "rows" }), "tool-group:1"));
});
check("the box lines follow the UI language", () => {
  const items = W.groupWork([asst(1, { thought: "a" }), tool(2), tool(3)])[0].items;
  win.OMP_I18N.setLocale("en-US");
  assert.equal(W.summarize(items).label, "Thought · Tools used: 2");
  win.OMP_I18N.setLocale("ja-JP");
  assert.equal(W.summarize(items).label, "思考済み · ツールを 2 個使用");
});
console.log(`test-work-box: ${n} checks passed`);
