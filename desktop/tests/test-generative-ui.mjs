#!/usr/bin/env node
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import { test } from "node:test";
const ctx = vm.createContext({ console, URL });
ctx.window = ctx;
for (const file of ["app/generative-ui.js", "app/turn-status.js", "app/transcript-merge.js", "adapter.js"]) {
  vm.runInContext(readFileSync(new URL(`../src/${file}`, import.meta.url), "utf8"), ctx, { filename: file });
}
const H = ctx.OMP_GENERATIVE_UI;
const own = v => JSON.parse(JSON.stringify(v));
const schema = () => ({ version: 1, title: "Agent options", components: [
  { type: "text", id: "intro", text: "<script>doNotExecute()</script>" },
  { type: "slider", id: "parallel", label: "Concurrency", min: 1, max: 32, default: 4 },
  { type: "select", id: "thinking", label: "Thinking", options: ["off", "low", "medium", "high"].map(value => ({ label: value, value })) },
  { type: "input", id: "note", label: "Note", minLength: 3 },
  { type: "button", id: "submit", label: "Send values" },
] });
const present = (s = schema()) => ({ role: "custom", customType: "ui.present", timestamp: 1000, details: { ui_id: "form-1", session_id: "session-1", task_id: "tool-1", message_id: "assistant-1", status: "waiting_input", rendered: false, schema: s } });
const submit = values => ({ role: "custom", customType: "ui.submit", timestamp: 2000, content: JSON.stringify({ ui_id: "form-1", values }), details: { ui_id: "form-1", session_id: "session-1", values } });
test("normalization supplies bounded defaults without interpreting text", () => {
  const s = H.normalizeSchema(schema());
  assert.equal(s.layout, "stack");
  assert.equal(s.components[0].text, "<script>doNotExecute()</script>");
  assert.equal(s.components[1].step, 1);
  assert.equal(s.components[3].maxLength, 1024);
  assert.deepEqual(own(H.initialValues(s)), { parallel: 4, thinking: "off", note: "" });
  assert.deepEqual(own(H.validateValues(s, H.initialValues(s))), {});
});
test("unsupported versions, components, keys and executable actions fail closed", () => {
  for (const mutate of [
    s => s.version = 2, s => s.layout = "masonry", s => s.layout = null,
    s => s.components[0].type = "html", s => s.components[0].html = "<b>bad</b>",
    s => s.components[4].action = "send_command", s => s.components[0].id = "__proto__",
    s => s.components[0].id = "constructor", s => s.components[0].id = "prototype",
    s => s.components[0].id = "submit", s => s.components.pop(),
    s => s.components[0].text = "x".repeat(4097), s => s.title = "x".repeat(161),
    s => s.components[1].default = 4.5, s => s.components[1].step = 0,
    s => s.components[1].max = Infinity, s => s.components[1].step = null,
    s => s.components[3].maxLength = 4097, s => s.components[3].default = null,
    s => s.components[2].options = [], s => s.components[2].options.push(s.components[2].options[0]),
  ]) {
    const s = schema(); mutate(s); assert.throws(() => H.normalizeSchema(s));
  }
});
test("field errors cover required, limits, membership, numeric steps and missing values", () => {
  const s = schema(); s.components[3].required = true;
  const n = H.normalizeSchema(s);
  assert.equal(Object.keys(H.validateValues(n, { parallel: 4.5, thinking: "arbitrary", note: "" })).length, 3);
  assert.equal(Object.keys(H.validateValues(n, { parallel: 4, thinking: "off", note: "ok" })).length, 1);
  assert.equal(Object.keys(H.validateValues(n, {})).length, 3);
  assert.throws(() => H.validateValues(n, { parallel: 4, thinking: "off", note: "okay", action: "run" }));
  const optional = schema(); optional.components[2].default = "";
  assert.equal(H.normalizeSchema(optional).components[2].default, "");
});
test("history folds submissions at the UI position and never shows a JSON bubble", () => {
  const values = { parallel: 8, thinking: "high", note: "okay" };
  const messages = ctx.adaptAgentMessages([present(), { role: "assistant", timestamp: 1500, content: "Choose above." }, submit(values)]);
  assert.equal(messages.length, 2);
  assert.equal(messages[0].kind, "ui");
  assert.equal(messages[0].status, "submitted");
  assert.deepEqual(own(messages[0].values), values);
  assert.equal(messages[1].kind, "assistant");
});
test("live duplicate presents and history refresh retain one submitted card and stable identity", () => {
  const values = { parallel: 8, thinking: "high", note: "okay" };
  let live = H.upsertMessage([], present()); live[0]._id = "stable";
  live = H.upsertMessage(live, present());
  live = H.upsertMessage(live, submit(values));
  live = H.upsertMessage(live, present());
  assert.equal(live.length, 1); assert.equal(live[0].status, "submitted");
  const merged = ctx.OMP_TRANSCRIPT.mergeTranscript(live, ctx.adaptAgentMessages([present(), submit(values)]));
  assert.equal(merged.length, 1); assert.equal(merged[0]._id, "stable");
  assert.deepEqual(own(merged[0].values), values);
  const mismatch = H.upsertMessage(live, { ...submit(values), details: { ...submit(values).details, session_id: "other" } });
  assert.equal(mismatch[0], live[0]);
  assert.match(mismatch[1].error, /session mismatch/);
});
test("invalid schema is a visible isolated UI error and unrelated custom messages stay ignored", () => {
  const bad = schema(); bad.components[0].type = "script";
  const cards = ctx.adaptAgentMessages([present(bad), { role: "user", timestamp: 3000, content: "Still here" }]);
  assert.equal(cards[0].kind, "ui"); assert.match(cards[0].error, /Unsupported/);
  assert.equal(cards[1].kind, "user");
  assert.equal(H.messageToUi({ role: "custom", customType: "async-result" }), null);
});
test("malformed external identities and cross-session submissions cannot break later history", () => {
  const malformedPresent = { ...present(), details: { schema: schema() }, timestamp: 1100 };
  const malformedSubmit = { ...submit({}), details: { ui_id: "form-1" }, timestamp: 1200 };
  const wrongSession = { ...submit({ parallel: 31 }), details: { ui_id: "form-1", session_id: "another", values: { parallel: 31 } } };
  const later = { role: "assistant", timestamp: 3000, content: "Later assistant remains visible." };
  const cards = ctx.adaptAgentMessages([present(), malformedPresent, malformedSubmit, wrongSession, later]);
  assert.equal(cards[0].status, "waiting_input");
  assert.equal(cards[0].values, undefined);
  assert.equal(cards.filter(m => m.kind === "ui" && m.error).length, 3);
  assert.equal(cards.at(-1).kind, "assistant");
  assert.equal(cards.at(-1).blocks[0].text, later.content);
  assert.equal(H.messageToUi(malformedPresent).ui_id, H.messageToUi(malformedPresent).ui_id);
  assert.equal(H.upsertMessage(cards, malformedPresent).length, cards.length);
});

// Exercise the real JSX event handlers with a tiny hook host. Browser fixture
// below tests actual React/DOM separately, without introducing a test runtime.
function formHarness() {
  const babelContext = vm.createContext({});
  vm.runInContext(readFileSync(new URL("../src/babel.min.js", import.meta.url), "utf8"), babelContext);
  const hooks = [];
  let index = 0;
  const React = {
    Component: class {},
    createElement: (type, props, ...children) => ({ type, props: { ...props, children } }),
    useMemo: fn => fn(),
    useEffect(fn) { fn(); },
    useState(initial) {
      const slot = index++;
      if (!(slot in hooks)) hooks[slot] = typeof initial === "function" ? initial() : initial;
      return [hooks[slot], value => { hooks[slot] = typeof value === "function" ? value(hooks[slot]) : value; }];
    },
    useRef(initial) {
      const slot = index++;
      if (!(slot in hooks)) hooks[slot] = { current: initial };
      return hooks[slot];
    },
  };
  const scope = vm.createContext({ React, window: { OMP_GENERATIVE_UI: H, SettingsSeg: "SettingsSeg", SettingsSlider: "SettingsSlider", SettingsButton: "SettingsButton", SettingsToggle: "SettingsToggle" } });
  const source = readFileSync(new URL("../src/design/chat/generative-ui.jsx", import.meta.url), "utf8");
  vm.runInContext(babelContext.Babel.transform(source, { presets: ["react"], filename: "generative-ui.jsx" }).code, scope);
  return Object.assign(props => { index = 0; return scope.GenerativeUiForm(props); }, { remount() { hooks.length = 0; } });
}
function nodes(tree) {
  if (Array.isArray(tree)) return tree.flatMap(nodes);
  if (!tree || typeof tree !== "object") return [];
  return [tree, ...nodes(tree.props.children)];
}
test("draft edits send nothing, duplicate clicks are gated, and confirmed values become readonly", async () => {
  const render = formHarness();
  const msg = H.messageToUi(present());
  let calls = 0, confirm;
  const props = { msg, disabled: false, onSubmit: (_msg, values) => { calls++; assert.equal(values.parallel, 12); return new Promise(resolve => { confirm = resolve; }); } };
  let tree = render(props);
  nodes(tree).find(n => n.type === "SettingsSlider").props.onChange(12);
  assert.equal(calls, 0);
  tree = render(props);
  const button = nodes(tree).find(n => n.type === "SettingsButton");
  const pending = button.props.onClick();
  await button.props.onClick();
  assert.equal(calls, 1);
  assert.equal(nodes(render(props)).find(n => n.type === "fieldset").props.disabled, true);
  confirm({ ui_id: msg.ui_id, status: "submitted" });
  await pending;
  tree = render(props);
  assert.equal(nodes(tree).filter(n => n.type === "SettingsButton").length, 0);
  assert.equal(nodes(tree).filter(n => n.type === "output").length, 3);
});
test("backend failure stays visible and retryable; disabled and invalid fields do not send", async () => {
  const render = formHarness();
  const s = schema(); s.components[3].required = true;
  const msg = H.messageToUi(present(s));
  let calls = 0;
  const props = { msg, disabled: false, onSubmit: async () => { calls++; throw new Error("Backend rejected values"); } };
  let tree = render(props);
  await nodes(tree).find(n => n.type === "SettingsButton").props.onClick();
  assert.equal(calls, 0);
  tree = render(props);
  assert.ok(nodes(tree).some(n => n.props.role === "alert"));
  nodes(tree).find(n => n.type === "input").props.onChange({ target: { value: "valid" } });
  tree = render({ ...props, disabled: true });
  await nodes(tree).find(n => n.type === "SettingsButton").props.onClick();
  assert.equal(calls, 0);
  tree = render(props);
  await nodes(tree).find(n => n.type === "SettingsButton").props.onClick();
  assert.equal(calls, 1);
  tree = render(props);
  assert.ok(nodes(tree).some(n => n.props.role === "alert" && n.props.children.includes("Backend rejected values")));
  assert.equal(nodes(tree).find(n => n.type === "SettingsButton").props.disabled, false);
});

const extendedSchema = () => ({ version: 1, layout: "grid", components: [
  { type: "textarea", id: "text", label: "Text", minLength: 2, maxLength: 8, rows: 3 },
  { type: "date", id: "date", label: "Date", min: "2026-01-01", max: "2026-12-31", default: "2026-11-02" },
  { type: "number", id: "number", label: "Number", min: -1, max: 1, step: 0.25, default: 0 },
  { type: "checkbox", id: "check", label: "Check" },
  { type: "switch", id: "switch", label: "Switch", default: true },
  { type: "multiselect", id: "multi", label: "Multi", options: [{ value: "a", label: "A" }, { value: "b", label: "B" }, { value: "c", label: "C" }], minItems: 1, maxItems: 2, default: ["a"] },
  { type: "button", id: "send", label: "Send" },
] });
test("additive controls normalize limits and clone array defaults", () => {
  const raw = extendedSchema(), s = H.normalizeSchema(raw);
  assert.equal(s.layout, "grid");
  assert.equal(s.components[0].rows, 3);
  assert.equal(s.components[1].default, "2026-11-02");
  assert.equal(s.components[3].default, false);
  const a = H.initialValues(s), b = H.initialValues(s);
  a.multi.push("b");
  assert.deepEqual(own(b.multi), ["a"]);
  assert.deepEqual(raw.components[5].default, ["a"]);
  assert.deepEqual(own(s.components[5].default), ["a"]);
  assert.equal(H.normalizeSchema(schema()).components[3].minLength, 3);
  const zero = schema(); delete zero.components[3].minLength;
  assert.equal(H.normalizeSchema(zero).components[3].minLength, 0);
});
test("new control validation enforces finite numeric steps, strict booleans and distinct known cardinality", () => {
  const s = H.normalizeSchema(extendedSchema());
  const good = { text: "okay", date: "2026-06-15", number: 0.25, check: true, switch: false, multi: ["a"] };
  for (const changes of [{}, { text: "", date: "" }, { text: "12345678", date: "2026-12-31", number: -1, multi: ["b", "c"] }]) {
    assert.deepEqual(own(H.validateValues(s, { ...good, ...changes })), {});
  }
  for (const changes of [
    { text: "x" }, { text: "123456789" }, { text: null },
    { date: "2026-02-30" }, { date: "2025-12-31" }, { date: "2027-01-01" }, { date: "not-a-date" },
    { number: NaN }, { number: Infinity }, { number: -Infinity }, { number: 1000001 }, { number: 0.1 }, { number: 1.25 },
    { check: "false" }, { check: 0 }, { switch: undefined },
    { multi: [] }, { multi: ["a", "a"] }, { multi: ["a", "b", "c"] }, { multi: ["unknown"] }, { multi: [1] }, { multi: "a" },
  ]) assert.ok(Object.keys(H.validateValues(s, { ...good, ...changes })).length > 0);
  for (const control of [
    { type: "textarea", id: "x", label: "X", rows: 0 },
    { type: "textarea", id: "x", label: "X", rows: 21 },
    { type: "textarea", id: "x", label: "X", maxLength: 4097 },
    { type: "checkbox", id: "x", label: "X", default: "true" },
    { type: "switch", id: "x", label: "X", default: 1 },
    { type: "number", id: "x", label: "X", min: 0, max: 1, step: Number.MIN_VALUE, default: 0 },
    { type: "select", id: "x", label: "X", options: [{ label: "Empty", value: "" }] },
    { type: "date", id: "x", label: "X", default: "2026-02-30" },
    { type: "date", id: "x", label: "X", min: "2026-12-01", max: "2026-01-01" },
    { ...extendedSchema().components[5], minItems: 2, maxItems: 1 },
    { ...extendedSchema().components[5], default: ["a", "a"] },
  ]) assert.throws(() => H.normalizeSchema({ version: 1, components: [control, { type: "button", id: "send", label: "Send" }] }));
});
test("rendered acknowledgement is once per identity, terminal receipts fold and never reactivate", () => {
  const render = formHarness(), msg = H.messageToUi(present());
  let rendered = 0, cancelled = 0;
  const props = { msg, onRendered: () => { rendered++; }, onCancel: async () => { cancelled++; return { ui_id: msg.ui_id, status: "cancelled" }; } };
  let tree = render(props); render(props);
  assert.equal(rendered, 1);
  const cancel = nodes(tree).find(n => n.type === "SettingsButton" && n.props.children.includes("Cancel"));
  const a = cancel.props.onClick(), b = cancel.props.onClick();
  return Promise.all([a, b]).then(() => {
    assert.equal(cancelled, 1);
    tree = render(props);
    assert.equal(nodes(tree).filter(n => n.type === "SettingsButton").length, 0);
    assert.equal(nodes(tree).find(n => n.type === "fieldset").props.disabled, true);
    let messages = H.upsertMessage([], present());
    const receipt = { role: "custom", customType: "ui.state", details: { ...present().details, schema: undefined, status: "cancelled", rendered: true } };
    messages = H.upsertMessage(messages, receipt);
    messages = H.upsertMessage(messages, present());
    assert.equal(messages[0].status, "cancelled");
    assert.equal(messages[0].rendered, true);
    messages = H.upsertMessage(messages, { ...receipt, details: { ...receipt.details, session_id: "copied" } });
    assert.equal(messages[0].status, "cancelled");
    assert.match(messages[1].error, /session mismatch/);
    const legacy = present(); delete legacy.details.task_id; delete legacy.details.message_id;
    assert.equal(H.messageToUi(legacy).status, "expired");
  });
});
test("submitted receipt survives admission failure and Resume task only reuses recorded values", async () => {
  const values = { parallel: 8, thinking: "high", note: "recorded" };
  let messages = H.upsertMessage([], present());
  messages = H.upsertMessage(messages, { ...submit(values), details: { ...submit(values).details, continuation: "pending" } });
  const render = formHarness();
  let calls = 0;
  const props = { msg: messages[0], onSubmit: async (_msg, sent) => { calls++; assert.deepEqual(own(sent), values); throw new Error("Admission interrupted"); } };
  let tree = render(props);
  assert.equal(nodes(tree).filter(n => n.type === "output").length, 3);
  let resume = nodes(tree).find(n => n.type === "SettingsButton");
  assert.deepEqual(resume.props.children, ["Resume task"]);
  await resume.props.onClick();
  tree = render(props);
  assert.equal(calls, 1);
  assert.equal(nodes(tree).filter(n => n.type === "output").length, 3);
  assert.ok(nodes(tree).some(n => n.props.role === "alert" && n.props.children.includes("Admission interrupted")));
  const success = { ...props, onSubmit: async (_msg, sent) => ({ ui_id: props.msg.ui_id, status: "submitted", continuation: "admitted" }) };
  await nodes(render(success)).find(n => n.type === "SettingsButton").props.onClick();
  assert.equal(nodes(render(success)).filter(n => n.type === "SettingsButton").length, 0);
});
test("new controls edit locally, expose accessible inputs, and render readable readonly output", async () => {
  const render = formHarness(), msg = H.messageToUi(present(extendedSchema()));
  let sent;
  const props = { msg, onSubmit: async (_msg, values) => { sent = own(values); return { ui_id: msg.ui_id, status: "submitted" }; } };
  let tree = render(props);
  nodes(tree).find(n => n.type === "textarea").props.onChange({ target: { value: "hello" } });
  nodes(tree).find(n => n.type === "input" && n.props.type === "date").props.onChange({ target: { value: "2026-12-01" } });
  nodes(tree).find(n => n.type === "input" && n.props.type === "number").props.onChange({ target: { valueAsNumber: 0.75 } });
  nodes(tree).find(n => n.type === "input" && n.props.id === "gui-form-1-check").props.onChange({ target: { checked: true } });
  nodes(tree).find(n => n.type === "SettingsToggle").props.onChange(false);
  nodes(tree).find(n => n.type === "input" && n.props.id === "gui-form-1-multi-1").props.onChange({ target: { checked: true } });
  assert.equal(sent, undefined);
  tree = render(props);
  await nodes(tree).find(n => n.type === "SettingsButton").props.onClick();
  assert.deepEqual(sent, { text: "hello", date: "2026-12-01", number: 0.75, check: true, switch: false, multi: ["a", "b"] });
  const outputs = nodes(render(props)).filter(n => n.type === "output").map(n => n.props.children[0]);
  assert.deepEqual(outputs, ["hello", "2026-12-01", 0.75, "On", "Off", "A, B"]);
});
test("drafts and rendered milestone survive tab remount, while expired history cannot send", async () => {
  const render = formHarness(), msg = H.messageToUi(present());
  let acknowledged = 0, submitted = 0;
  const props = { msg, onRendered: () => { acknowledged++; }, onSubmit: async () => { submitted++; } };
  let tree = render(props);
  nodes(tree).find(n => n.type === "SettingsSlider").props.onChange(16);
  render.remount();
  tree = render(props);
  assert.equal(nodes(tree).find(n => n.type === "SettingsSlider").props.value, 16);
  assert.equal(acknowledged, 1);
  const expired = { ...msg, status: "expired" };
  tree = render({ ...props, msg: expired });
  assert.equal(nodes(tree).filter(n => n.type === "SettingsButton").length, 0);
  await nodes(tree).find(n => n.type === "form").props.onSubmit({ preventDefault() {} });
  assert.equal(submitted, 0);
  assert.equal(acknowledged, 1);
});
test("stale lifecycle states cannot demote submitted admission or change recorded values", () => {
  const values = { parallel: 8, thinking: "high", note: "recorded" };
  let cards = H.upsertMessage([], present());
  cards = H.upsertMessage(cards, { ...submit(values), details: { ...submit(values).details, continuation: "admitted" } });
  cards = H.upsertMessage(cards, { role: "custom", customType: "ui.state", details: { ...present().details, status: "waiting_input", rendered: true, continuation: "pending" } });
  cards = H.upsertMessage(cards, submit({ ...values, parallel: 12 }));
  assert.equal(cards.length, 1);
  assert.equal(cards[0].status, "submitted");
  assert.equal(cards[0].continuation, "admitted");
  assert.deepEqual(own(cards[0].values), values);
  cards = H.upsertMessage(cards, { ...submit(values), details: { ...submit(values).details, values: null } });
  assert.equal(cards[0].status, "submitted");
  assert.match(cards[1].error, /object/);
});
test("pending submissions can expire or cancel, while admitted and genuine terminal receipts stay monotonic", () => {
  const values = { parallel: 8, thinking: "high", note: "recorded" };
  const state = status => ({ role: "custom", customType: "ui.state", details: { ...present().details, status, rendered: true } });
  for (const terminal of ["expired", "cancelled"]) {
    let cards = H.upsertMessage([], present());
    cards = H.upsertMessage(cards, { ...submit(values), details: { ...submit(values).details, continuation: "pending" } });
    cards = H.upsertMessage(cards, state(terminal));
    assert.equal(cards[0].status, terminal);
    cards = H.upsertMessage(cards, submit(values));
    cards = H.upsertMessage(cards, state("waiting_input"));
    assert.equal(cards[0].status, terminal);
    const render = formHarness();
    assert.equal(nodes(render({ msg: cards[0], onSubmit: async () => {} })).filter(n => n.type === "SettingsButton").length, 0);
    let admitted = H.upsertMessage([], present());
    admitted = H.upsertMessage(admitted, { ...submit(values), details: { ...submit(values).details, continuation: "admitted" } });
    admitted = H.upsertMessage(admitted, state(terminal));
    assert.equal(admitted[0].status, "submitted");
    assert.equal(admitted[0].continuation, "admitted");
  }
  const legacy = present(); delete legacy.details.task_id; delete legacy.details.message_id;
  let historical = H.upsertMessage([], legacy);
  assert.equal(historical[0].status, "expired");
  historical = H.upsertMessage(historical, submit(values));
  assert.equal(historical[0].status, "submitted");
});
test("failed rendered acknowledgement stays isolated and retries only after remount", async () => {
  const render = formHarness(), msg = H.messageToUi(present());
  let attempts = 0;
  const props = { msg, onRendered: async () => { attempts++; throw new Error("Render acknowledgement unavailable"); } };
  render(props);
  await new Promise(resolve => setImmediate(resolve));
  let tree = render(props);
  assert.equal(attempts, 1);
  assert.ok(nodes(tree).some(n => n.props.role === "alert" && n.props.children.includes("Render acknowledgement unavailable")));
  render({ ...props, onRendered: async () => { attempts++; } });
  assert.equal(attempts, 1);
  render.remount();
  tree = render({ ...props, onRendered: async () => { attempts++; } });
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(attempts, 2);
  assert.ok(nodes(tree).some(n => n.type === "SettingsSlider"));
});
test("full history cards advance admission without regressing terminal state or live identity", () => {
  const draft = H.messageToUi(present());
  const values = { parallel: 8, thinking: "high", note: "recorded" };
  const pending = { ...draft, _id: "live-id", status: "submitted", values, continuation: "pending" };
  const admitted = { ...pending, _id: "history-id", continuation: "admitted", rendered: true };
  let cards = H.upsertMessage([pending], admitted);
  assert.equal(cards[0].continuation, "admitted");
  assert.equal(cards[0]._id, "live-id");
  assert.equal(cards[0].rendered, true);
  cards = H.upsertMessage(cards, pending);
  cards = H.upsertMessage(cards, draft);
  assert.equal(cards[0].status, "submitted");
  assert.equal(cards[0].continuation, "admitted");
  assert.deepEqual(own(cards[0].values), values);
  for (const terminal of ["expired", "cancelled"]) {
    const expired = { ...draft, status: terminal, reason: "Task is no longer active." };
    let stopped = H.upsertMessage([pending], expired);
    assert.equal(stopped[0].status, terminal);
    assert.equal(stopped[0].reason, expired.reason);
    stopped = H.upsertMessage(stopped, admitted);
    assert.equal(stopped[0].status, terminal);
    assert.equal(H.upsertMessage([admitted], expired)[0].status, "submitted");
  }
  const restored = H.upsertMessage([draft], admitted);
  assert.equal(restored[0].status, "submitted");
  assert.equal(restored[0].continuation, "admitted");
  assert.deepEqual(own(restored[0].values), values);
  const legacy = present(); delete legacy.details.task_id; delete legacy.details.message_id;
  const projected = H.messageToUi(legacy);
  assert.equal(H.upsertMessage([pending], projected)[0].status, "submitted");
});
test("visibleWhen hides conditional controls dynamically and bypasses required validation while hidden", async () => {
  const condSchema = H.normalizeSchema({
    version: 1, layout: "grid", components: [
      { type: "checkbox", id: "advanced", label: "Advanced", default: false },
      { type: "input", id: "detail", label: "Detail", required: true, minLength: 3, visibleWhen: { id: "advanced", equals: true } },
      { type: "button", id: "send", label: "Send" },
    ],
  });
  assert.deepEqual(own(H.validateValues(condSchema, { advanced: false, detail: "" })), {});
  assert.ok(Object.keys(H.validateValues(condSchema, { advanced: true, detail: "" })).length > 0);
  const render = formHarness(), msg = H.messageToUi(present(condSchema));
  let tree = render({ msg, onSubmit: async () => ({ ui_id: msg.ui_id, status: "submitted" }) });
  assert.equal(nodes(tree).some(n => n.type === "input" && n.props.id === "gui-form-1-detail"), false);
  nodes(tree).find(n => n.type === "input" && n.props.id === "gui-form-1-advanced").props.onChange({ target: { checked: true } });
  tree = render({ msg, onSubmit: async () => ({ ui_id: msg.ui_id, status: "submitted" }) });
  assert.equal(nodes(tree).some(n => n.type === "input" && n.props.id === "gui-form-1-detail"), true);
});

test("display mode normalizes components, requires no submit button, and renders without form", () => {
  const displaySchema = {
    version: 1,
    mode: "display",
    title: "Project Progress",
    layout: "stack",
    components: [
      { type: "stat", id: "progress_stat", label: "Progress", value: "85%", trend: "up", diff: "+5%" },
      { type: "progress", id: "bar", label: "Completion", value: 85, max: 100 },
      { type: "badge", id: "b1", text: "On Track", variant: "success" },
      { type: "alert", id: "a1", message: "Review scheduled", level: "info" },
      { type: "table", id: "t1", columns: [{ key: "task", label: "Task" }, { key: "status", label: "Status" }], rows: [{ task: "Core", status: "Done" }] },
      { type: "key-value", id: "kv1", items: [{ key: "lead", label: "Lead", value: "Agent" }] },
      { type: "timeline", id: "tl1", items: [{ time: "10:00", title: "Started", status: "done" }] },
      { type: "collapsible", id: "c1", title: "Details", content: "All tests green" },
      { type: "tabs", id: "tab1", items: [{ id: "tab_a", label: "Overview", content: "All good" }] },
      { type: "bar-chart", id: "chart1", data: [{ label: "A", value: 10 }, { label: "B", value: 20 }] },
      { type: "divider", id: "d1", label: "Section" },
      { type: "image", id: "img1", src: "images/diagram.png", alt: "Diagram", title: "Diagram title" },
    ],
  };
  const s = H.normalizeSchema(displaySchema);
  assert.equal(s.mode, "display");
  assert.equal(s.components.length, 12);

  // Display mode rejects submit buttons or input controls
  assert.throws(() => H.normalizeSchema({ ...displaySchema, components: [...displaySchema.components, { type: "button", id: "btn", label: "Submit" }] }));
  assert.throws(() => H.normalizeSchema({ ...displaySchema, components: [...displaySchema.components, { type: "input", id: "inp", label: "Input" }] }));

  // Message mapping immediately yields rendered status
  const uiMsg = H.messageToUi({ role: "custom", customType: "ui.present", timestamp: 1000, details: { ui_id: "disp-1", session_id: "s1", task_id: "t1", message_id: "m1", status: "rendered", rendered: true, schema: s } });
  assert.equal(uiMsg.status, "rendered");
  assert.equal(uiMsg.rendered, true);
});

test("interactive mode supports option-cards and time-picker", () => {
  const interactiveSchema = H.normalizeSchema({
    version: 1,
    mode: "interactive",
    title: "Plan Selection",
    components: [
      {
        type: "option-cards",
        id: "plan",
        label: "Choose a plan",
        cards: [
          { id: "opt1", title: "Option A", description: "Fast", recommended: true },
          { id: "opt2", title: "Option B", description: "Safe" },
        ],
        default: "opt1",
      },
      { type: "time-picker", id: "alarm", label: "Time", default: "09:30" },
      { type: "button", id: "submit", label: "Confirm Plan" },
    ],
  });
  assert.equal(interactiveSchema.mode, "interactive");
  assert.deepEqual(own(H.initialValues(interactiveSchema)), { plan: "opt1", alarm: "09:30" });
  assert.deepEqual(own(H.validateValues(interactiveSchema, { plan: "opt2", alarm: "14:00" })), {});
  assert.ok(Object.keys(H.validateValues(interactiveSchema, { plan: "invalid", alarm: "14:00" })).length > 0);
  assert.throws(() => H.validateValues(interactiveSchema, { plan: "opt1", alarm: "14:00", extra: "illegal" }));
});

test("image components mirror kernel references and render through a component boundary", () => {
  const fixtureImage = {
    type: "image",
    id: "prismatic",
    src: "https://d9-wret.s3.us-west-2.amazonaws.com/assets/palladium/production/s3fs-public/media/images/Midway.png",
    thumbnail: "https://d9-wret.s3.us-west-2.amazonaws.com/assets/palladium/production/s3fs-public/styles/thumbnail/public/media/images/Midway.png?itok=MTMzyGco",
    alt: "Grand Prismatic Spring",
    title: "Grand Prismatic Spring",
    sourceUrl: "https://www.usgs.gov/media/images/grand-prismatic-spring-midway-geyser-basin-yellowstone-national-park",
    attribution: "Robert Fournier / USGS public domain",
  };
  const s = H.normalizeSchema({ version: 1, mode: "display", components: [fixtureImage, { type: "image", id: "local", src: "images/map.png" }] });
  assert.equal(s.components[0].src, fixtureImage.src);
  assert.equal(s.components[0].thumbnail, fixtureImage.thumbnail);
  assert.equal(s.components[0].sourceUrl, fixtureImage.sourceUrl);
  assert.equal(s.components[0].attribution, fixtureImage.attribution);
  assert.equal(s.components[1].src, "images/map.png");

  const normalizedRefs = H.normalizeSchema({
    version: 1, mode: "display",
    components: [
      { type: "image", id: "spaces", src: "assets/my image.png" },
      { type: "image", id: "slashes", src: "assets\\nested\\my image.png" },
      { type: "image", id: "canonical", src: "HTTPS://EXAMPLE.COM/Photo.PNG", sourceUrl: "HTTPS://WWW.USGS.GOV/photo" },
    ],
  });
  assert.equal(normalizedRefs.components[0].src, "assets/my image.png");
  assert.equal(normalizedRefs.components[1].src, "assets/nested/my image.png");
  assert.equal(normalizedRefs.components[2].src, "https://example.com/Photo.PNG");
  assert.equal(normalizedRefs.components[2].sourceUrl, "https://www.usgs.gov/photo");

  // References share the same 32 KiB budget as the rest of the card.
  const longRefs = Array.from({ length: 20 }, (_, i) => ({ type: "image", id: `long${i}`, src: "a".repeat(2000) }));
  assert.throws(() => H.normalizeSchema({ version: 1, mode: "display", components: longRefs }), /text exceeds limit/);

  const componentSchema = H.normalizeSchema({
    version: 1, mode: "display",
    components: [{ type: "text", id: "copy", text: "Field notes stay readable while the image loads." }, fixtureImage],
  });
  const render = formHarness();
  const msg = H.messageToUi(present(componentSchema));
  const tree = render({ msg, disabled: false });
  const text = nodes(tree).find(node => node.type === "div" && node.props.className === "gui-text");
  assert.ok(text, "text renders independently of the pending image");
  const image = nodes(tree).find(node => typeof node.type === "function" && node.type.name === "GenerativeUiImage");
  assert.ok(image, "registry entry must create a component element, not call the loader hook directly");
  assert.equal(image.props.component.src, fixtureImage.src);

  const reject = (patch) => assert.throws(() => H.normalizeSchema({ version: 1, mode: "display", components: [{ ...fixtureImage, ...patch }] }));
  reject({ src: "http://example.com/a.png" });
  reject({ src: "data:image/png;base64,AAAA" });
  reject({ src: "/absolute.png" });
  reject({ src: "../escape.png" });
  reject({ src: "images//map.png" });
  reject({ src: "https://localhost/a.png" });
  reject({ src: "https://127.0.0.1/a.png" });
  reject({ src: "https://example.com/my image.png" });
  reject({ src: `https://example.com/${"图".repeat(230)}` });
  reject({ src: "\\absolute.png" });
  reject({ src: "~home.png" });
  reject({ src: "x".repeat(2049) });
  reject({ thumbnail: "http://example.com/a.png" });
  reject({ sourceUrl: "https://127.0.0.1/page" });
  reject({ alt: "x".repeat(301) });
  reject({ title: "x".repeat(161) });
  reject({ attribution: "x".repeat(201) });
});
