#!/usr/bin/env node
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const win = {};
for (const file of ["stream-delta", "sidechat"]) {
  new Function("window", readFileSync(new URL(`../src/app/${file}.js`, import.meta.url), "utf8"))(win);
}
const tick = () => new Promise(resolve => setImmediate(resolve));
function latch() {
  let resolve;
  const promise = new Promise(yes => { resolve = yes; });
  return { promise, resolve };
}
function harness(options = {}) {
  const listeners = new Map(), timers = new Map(), calls = [], commands = [], views = [];
  let sequence = 0, timerId = 0;
  const h = {
    calls, commands, views, listeners, timers,
    autoModel: true, autoPrompt: true, autoClear: true, autoAbort: true, ...options,
    emit(frame, seq = ++sequence) {
      listeners.get("agent://line/sidechat-test")?.({ payload: { seq, text: JSON.stringify(frame) } });
      return seq;
    },
    response(command, data, success = true) {
      h.emit({ type: "response", id: command.id, command: command.type, success, data, error: success ? undefined : data });
    },
    exit(reason) { listeners.get("agent://exit/sidechat-test")?.({ payload: reason }); },
    last(type) { return commands.filter(command => command.type === type).at(-1); },
    async dispose() {
      await h.controller.dispose();
      assert.equal(listeners.size, 0, "every subscription must be removed");
      assert.equal(timers.size, 0, "no response/handshake timer may survive teardown");
    },
  };
  h.controller = win.OMP_SIDECHAT.create({
    sessionId: "sidechat-test", profile: "test-profile", model: options.model || "",
    ...(options.copilot ? { startCommand: "academic_copilot_start", startArgs: { project: "/p/Thesis" } } : {}),
    onChange: state => views.push(state),
    schedule(fn) { const id = ++timerId; timers.set(id, fn); return id; },
    cancel(id) { timers.delete(id); },
    async listen(name, callback) {
      listeners.set(name, callback);
      if (h.listenerGate) await h.listenerGate.promise;
      return () => listeners.delete(name);
    },
    async invoke(name, args) {
      calls.push({ name, args });
      if (name === "sidechat_start" || name === "academic_copilot_start") {
        assert.equal(listeners.size, 2, "listen before starting the process");
        assert.deepEqual(args, options.copilot
          ? { sessionId: "sidechat-test", profile: "test-profile", project: "/p/Thesis" }
          : { sessionId: "sidechat-test", profile: "test-profile" });
        if (h.startError) throw new Error(h.startError);
        if (!h.noReady) h.emit({ type: "ready" });
        if (h.spawnGate) await h.spawnGate.promise;
      } else if (name === "send_command") {
        const command = JSON.parse(args.json);
        commands.push(command);
        if (command.type === "get_state") h.response(command, { model: { provider: "default", id: "chat" } });
        if (command.type === "set_model" && h.autoModel) h.response(command, {});
        if (command.type === "prompt" && h.autoPrompt) h.response(command, {});
        if (command.type === "get_available_thinking_levels" && h.levels) h.response(command, { levels: h.levels });
        if (command.type === "set_thinking_level" && h.autoLevel !== false) {
          h.emit({ type: "thinking_level_changed", thinkingLevel: command.level });
          h.response(command, {});
        }
        if (command.type === "new_session" && h.autoClear) h.response(command, { cancelled: false });
        if (command.type === "abort" && h.autoAbort) h.response(command, {});
      }
    },
  });
  h.state = () => h.controller.getState();
  return h;
}
let passed = 0;
async function check(label, run) {
  try { await run(); passed++; }
  catch (error) { console.error(`FAILED: ${label}`); throw error; }
}

await check("enveloped delta streaming, deduplication and assistant-only completion", async () => {
  const h = harness();
  await h.controller.start();
  assert.equal(h.state().phase, "ready");
  await h.controller.send("hello");
  const prompt = h.last("prompt");
  h.emit({ type: "message_start", message: { role: "assistant", content: [] } });
  h.emit({ type: "message_update", message: { role: "assistant" }, assistantMessageEvent: {
    type: "thinking_delta", contentIndex: 0, delta: "considering",
  } });
  const textDelta = { type: "message_update", message: { role: "assistant" }, assistantMessageEvent: {
    type: "text_delta", contentIndex: 1, delta: "Hello",
  } };
  const seq = h.emit(textDelta);
  h.emit(textDelta, seq);
  assert.equal(h.state().liveText, "Hello");
  h.emit({ type: "message_end", message: { role: "user", content: "hello" } });
  assert.equal(h.state().phase, "generating");
  assert.equal(h.state().liveText, "Hello");
  h.emit({ type: "message_end", message: { role: "assistant", content: [{ type: "text", text: "Hello!" }] } });
  assert.equal(h.state().messages.at(-1).text, "Hello!");
  assert.equal(h.state().phase, "generating", "message_end is not prompt completion");
  h.emit({ type: "prompt_result", id: "unrelated", status: "success" });
  assert.equal(h.state().phase, "generating");
  h.emit({ type: "prompt_result", id: prompt.id, status: "success" });
  assert.equal(h.state().phase, "ready");
  await h.dispose();
});

await check("model acknowledgment gates prompts, including a model changed during the ack", async () => {
  const h = harness();
  await h.controller.start();
  h.autoModel = false;
  h.controller.setModel("one/models/chat");
  const sending = h.controller.send("model test");
  await tick();
  const first = h.last("set_model");
  assert.equal(first.provider, "one");
  assert.equal(first.modelId, "models/chat");
  assert.equal(h.last("prompt"), undefined);
  h.response({ ...first, id: "wrong-id" }, {});
  await tick();
  assert.equal(h.state().phase, "sending");
  h.controller.setModel("two/chat");
  h.response(first, {});
  await tick();
  const second = h.last("set_model");
  assert.equal(second.provider, "two");
  assert.equal(h.last("prompt"), undefined);
  h.response(second, {});
  await sending;
  h.emit({ type: "prompt_result", id: h.last("prompt").id, status: "success" });
  h.controller.setModel("");
  const reverting = h.controller.send("default again");
  await tick();
  const restore = h.last("set_model");
  assert.equal(restore.provider, "default");
  assert.equal(restore.modelId, "chat");
  h.response(restore, {});
  await reverting;
  await h.dispose();
});

await check("startup waits for ready and the initial model acknowledgment", async () => {
  const h = harness({ noReady: true, autoModel: false, model: "chosen/chat" });
  const starting = h.controller.start();
  await tick();
  assert.equal(h.commands.length, 0);
  h.emit({ type: "ready" });
  await tick();
  assert.equal(h.last("set_model").provider, "chosen");
  assert.equal(h.state().phase, "starting");
  h.response(h.last("set_model"), {});
  await starting;
  assert.equal(h.state().phase, "ready");
  await h.dispose();
});

await check("prompt errors and worker exit are explicit and preserve partial text", async () => {
  const h = harness();
  await h.controller.start();
  await h.controller.send("fail");
  h.emit({ type: "prompt_result", id: h.last("prompt").id, status: "error", error: { message: "provider unavailable" } });
  assert.equal(h.state().phase, "error");
  assert.equal(h.state().error, "provider unavailable");
  await h.controller.send("try again");
  h.emit({ type: "message_update", message: { role: "assistant" }, assistantMessageEvent: {
    type: "text_delta", contentIndex: 0, delta: "partial answer",
  } });
  h.exit("worker crashed");
  assert.equal(h.state().phase, "failed");
  assert.match(h.state().error, /worker crashed/);
  assert.equal(h.state().messages.at(-1).text, "partial answer");
  assert.equal(await h.controller.send("must not send"), false);
  await h.dispose();
});

await check("reopening retains the worker and receives frames while hidden", async () => {
  const h = harness();
  await h.controller.start();
  await h.controller.send("background reply");
  // Visibility has no controller transition: closing only hides the mounted view.
  h.emit({ type: "message_end", message: { role: "assistant", content: [{ type: "text", text: "received while hidden" }] } });
  h.emit({ type: "prompt_result", id: h.last("prompt").id, status: "success" });
  await h.controller.start();
  assert.equal(h.calls.filter(call => call.name === "sidechat_start").length, 1);
  assert.equal(h.listeners.size, 2);
  assert.equal(h.state().messages.at(-1).text, "received while hidden");
  await h.dispose();
  assert.deepEqual(h.calls.at(-1), { name: "close_tab", args: { sessionId: "sidechat-test" } });
});

await check("clear waits for an acknowledged reset and rejects overlapping sends", async () => {
  const h = harness({ autoClear: false });
  await h.controller.start();
  await h.controller.send("keep until reset accepted");
  h.emit({ type: "prompt_result", id: h.last("prompt").id, status: "success" });
  const clearing = h.controller.clear();
  await tick();
  assert.equal(h.state().messages.length, 1);
  assert.equal(await h.controller.send("blocked"), false);
  h.response(h.last("new_session"), { cancelled: true });
  await clearing;
  assert.equal(h.state().phase, "error");
  assert.equal(h.state().messages.length, 1);
  const accepted = h.controller.clear();
  await tick();
  h.response(h.last("new_session"), { cancelled: false });
  await accepted;
  assert.equal(h.state().messages.length, 0);
  await h.dispose();
});

await check("stop remains serialized until abort acknowledgment even after prompt_result", async () => {
  const h = harness({ autoAbort: false });
  await h.controller.start();
  await h.controller.send("stop this");
  const stopping = h.controller.stop();
  await tick();
  h.emit({ type: "prompt_result", id: h.last("prompt").id, status: "aborted" });
  assert.equal(h.state().phase, "stopping");
  assert.equal(await h.controller.send("not yet"), false);
  h.response(h.last("abort"), {});
  await stopping;
  assert.equal(h.state().phase, "ready");
  await h.dispose();
});

await check("unmount during listener registration releases late listeners without spawning", async () => {
  const listenerGate = latch();
  const h = harness({ listenerGate });
  const starting = h.controller.start();
  const disposing = h.controller.dispose();
  listenerGate.resolve();
  await Promise.all([starting, disposing]);
  assert.equal(h.calls.length, 0);
  assert.equal(h.listeners.size, 0);
  assert.equal(h.timers.size, 0);
});

await check("unmount during spawn closes only after the spawn settles", async () => {
  const spawnGate = latch();
  const h = harness({ spawnGate, noReady: true });
  const starting = h.controller.start();
  await tick();
  const disposing = h.controller.dispose();
  await tick();
  assert.equal(h.calls.filter(call => call.name === "close_tab").length, 0);
  spawnGate.resolve();
  await Promise.all([starting, disposing]);
  assert.equal(h.calls.filter(call => call.name === "close_tab").length, 1);
  assert.equal(h.listeners.size, 0);
  assert.equal(h.timers.size, 0);
});

await check("startup failures and missing ready fail explicitly and clean up", async () => {
  const failed = harness({ startError: "executable not found" });
  await failed.controller.start();
  assert.equal(failed.state().phase, "failed");
  assert.match(failed.state().error, /executable not found/);
  await failed.dispose();
  const timed = harness({ noReady: true });
  const starting = timed.controller.start();
  await tick();
  for (const callback of timed.timers.values()) callback();
  await starting;
  assert.equal(timed.state().phase, "failed");
  assert.match(timed.state().error, /startup handshake timed out/);
  await timed.dispose();
});

await check("a rejected prompt response stops thinking without pretending it succeeded", async () => {
  const h = harness({ autoPrompt: false });
  await h.controller.start();
  const sending = h.controller.send("rejected");
  await tick();
  h.response(h.last("prompt"), "prompt admission failed", false);
  assert.equal(await sending, false);
  assert.equal(h.state().phase, "error");
  assert.equal(h.state().liveText, null);
  assert.equal(h.state().error, "prompt admission failed");
  await h.dispose();
});
await check("the Academic Copilot starts through its own command with its project", async () => {
  const h = harness({ copilot: true });
  await h.controller.start();
  assert.equal(h.state().phase, "ready");
  assert.equal(h.calls.filter(call => call.name === "academic_copilot_start").length, 1);
  assert.equal(h.calls.filter(call => call.name === "sidechat_start").length, 0);
  await h.dispose();
});
await check("the pane's pick overrides the Settings model; bad keys give no menu entry", async () => {
  const S = win.OMP_SIDECHAT;
  assert.equal(S.wantedModel(null, "a/one"), "a/one");
  assert.equal(S.wantedModel("b/two", "a/one"), "b/two");
  assert.equal(S.wantedModel(null, ""), "");
  assert.deepEqual(JSON.parse(JSON.stringify(S.stubModel("openai/gpt-5.5"))), { provider: "openai", id: "gpt-5.5", name: "gpt-5.5" });
  assert.equal(S.stubModel("nope"), null);
  assert.equal(S.stubModel("a/"), null);
  assert.equal(S.stubModel(""), null);
});
await check("a model picked after start is applied with the next message, through the switching phase", async () => {
  const h = harness();
  await h.controller.start();
  h.autoModel = false;
  h.controller.setModel("picked/model");
  const sent = h.controller.send("hi");
  await tick();
  assert.equal(h.state().phase, "sending");
  const set = h.last("set_model");
  assert.equal(set.provider, "picked");
  assert.equal(h.last("prompt"), undefined);
  h.response(set, {});
  await sent;
  assert.equal(h.state().phase, "generating");
  assert.equal(h.state().model, "picked/model");
  await h.dispose();
});
const settle = async () => { for (let i = 0; i < 6; i++) await tick(); };
const levelCommands = h => h.commands.filter(command => command.type === "set_thinking_level").map(command => command.level);

await check("the model's thinking levels are listed and the level in use is shown", async () => {
  const h = harness({ levels: ["off", "low", "high"] });
  assert.equal(h.state().level, "", "unknown before the worker is ready");
  await h.controller.start();
  await settle();
  assert.equal(h.state().level, "off", "an unset level reads as off");
  assert.deepEqual(await h.controller.thinkingLevels(), ["off", "low", "high"]);
  h.emit({ type: "thinking_level_changed", thinkingLevel: "low" });
  assert.equal(h.state().level, "low");
  await h.dispose();
});

await check("a pick is validated against the list, sent, and never a level omp did not list", async () => {
  const h = harness({ levels: ["off", "low", "high"] });
  await h.controller.start();
  await settle();
  assert.equal(await h.controller.setThinkingLevel("xhigh"), false);
  assert.deepEqual(levelCommands(h), []);
  assert.equal(await h.controller.setThinkingLevel("high"), true);
  assert.deepEqual(levelCommands(h), ["high"]);
  assert.equal(h.state().level, "high", "the level shown follows omp's thinking_level_changed");
  await h.dispose();
});

await check("a model without thinking shows no level, whatever omp reports", async () => {
  for (const levels of [[], ["off"]]) {
    const h = harness({ levels });
    await h.controller.start();
    await settle();
    assert.equal(h.state().level, "");
    h.emit({ type: "thinking_level_changed", thinkingLevel: "off" });
    assert.equal(h.state().level, "");
    assert.equal(await h.controller.setThinkingLevel("low"), false);
    await h.dispose();
  }
});

await check("a model change refreshes the list and the pick is applied again where it still exists", async () => {
  const h = harness({ levels: ["off", "low", "high"] });
  await h.controller.start();
  await settle();
  await h.controller.setThinkingLevel("high");
  h.emit({ type: "thinking_level_changed", thinkingLevel: "low" });   // the new model clamped it
  const listed = h.commands.filter(command => command.type === "get_available_thinking_levels").length;
  h.controller.setModel("picked/model");
  await h.controller.send("hi");
  await settle();
  assert.ok(h.commands.filter(command => command.type === "get_available_thinking_levels").length > listed, "the list was asked again");
  assert.deepEqual(levelCommands(h), ["high", "high"]);
  h.levels = [];
  h.controller.setModel("plain/model");
  h.emit({ type: "prompt_result", id: h.last("prompt").id, status: "success" });
  await settle();
  await h.controller.send("again");
  await settle();
  assert.equal(h.state().level, "", "the new model has no thinking: the level goes away");
  await h.dispose();
});

await check("a pick made while the worker is busy waits for the next message", async () => {
  const h = harness({ levels: ["off", "low", "high"], autoPrompt: false });
  await h.controller.start();
  await settle();
  const sent = h.controller.send("work");
  await settle();
  assert.equal(h.state().phase, "generating");
  assert.equal(await h.controller.setThinkingLevel("low"), true);
  assert.deepEqual(levelCommands(h), [], "nothing is sent mid-reply");
  h.response(h.last("prompt"), {});
  await sent;
  h.emit({ type: "prompt_result", id: h.last("prompt").id, status: "success" });
  await settle();
  h.autoPrompt = true;
  await h.controller.send("next");
  assert.deepEqual(levelCommands(h), ["low"]);
  await h.dispose();
});
console.log(`SideChat: ${passed} deterministic regressions passed`);
