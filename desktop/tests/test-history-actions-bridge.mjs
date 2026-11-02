#!/usr/bin/env node
// Consumer-visible scenario regressions for historical session resume and deletion in live.js.
// Run: node tests/test-history-actions-bridge.mjs

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const source = (path) =>
  readFileSync(
    fileURLToPath(new URL(`../src/${path}`, import.meta.url)),
    "utf8",
  );

function harness(opts = {}) {
  const listeners = new Map();
  const invokes = [];

  const w = {
    OMP_SLASH: { LOCAL_COMMANDS: [] },
    OMP_SUBAGENTS: {
      emptySubagents: () => ({}),
      subscriptionLevelFor: () => "progress",
    },
    OMP_EXT_WIDGETS: { EMPTY: {} },
    OMP_SESSION_PREFS: {
      fromTweaks: () => ({ autoCompaction: true, autoRetry: true, confirmBusyClose: true }),
      closeGuardArmed: () => true,
      tabCloseConfirm: () => ({ title: "Close busy tab?" }),
    },
    OMP_TWEAKS_STORE: { read: () => ({}) },
    OMP_APPROVAL_MODE: { isMode: () => true },
    DEFAULT_PROFILE_ID: "default",
    INTENT_FRAMING: () => "",
    timeNow: () => "00:00",
  };

  for (const path of [
    "app/constants.js",
    "app/slash-commands.js",
    "app/project-nav.js",
    "app/message-queue.js",
    "app/goal.js",
    "app/prompt-history.js",
    "app/session-title.js",
    "app/turn-status.js",
    "app/transcript-merge.js",
    "app/usage-view.js",
    "app/generative-ui.js",
    "adapter.js",
    "app/pr-automation.js",
  ]) {
    new Function("window", source(path))(w);
  }

  const timeout = (fn, ms) => {
    const timer = setTimeout(fn, ms);
    timer.unref();
    return timer;
  };

  new Function("window", "setTimeout", "clearTimeout", source("live.js"))(
    w,
    timeout,
    clearTimeout,
  );

  w.__TAURI__ = {
    core: {
      async invoke(command, args) {
        invokes.push({ command, args });
        if (command === "start_session") {
          if (opts.failSpawn) throw new Error("backend spawn rejected: invalid profile");
          return "prompt";
        }
        if (command === "start_git_watch") return "main";
        if (command === "stop_session") return null;
        if (command === "close_tab") return null;
        if (command === "open_tabs_save") return null;
        if (command === "replay_events") return { dropped: false, events: [] };
        if (command === "delete_saved_session") {
          if (opts.failDelete) throw new Error("could not delete: permission denied");
          return null;
        }
        if (command === "batch_delete_saved_sessions") return args.paths || [];
        if (command === "list_profiles") {
          return {
            profiles: [
              { id: "default", name: "Default" },
              { id: "work", name: "Work" },
            ],
            startupId: "default",
          };
        }
        if (command === "list_saved_sessions") {
          return [
            { id: "sess-1", path: "/path/to/default-sess.jsonl", title: "Default Chat" },
          ];
        }
        return null;
      },
    },
    event: {
      async listen(event, cb) {
        listeners.set(event, cb);
        return async () => { listeners.delete(event); };
      },
    },
  };
  let seq = 0;

  return {
    bridge: w.OMP_BRIDGE,
    invokes,
    w,
    emit(id, frame) {
      listeners.get(`agent://line/${id}`)({ payload: { seq: ++seq, text: JSON.stringify(frame) } });
    },
  };
}

let passed = 0;
async function test(name, fn) {
  try {
    await fn();
    passed++;
  } catch (err) {
    console.error(`FAILED: ${name}`);
    throw err;
  }
}

async function run() {
  await test("list default then active profile switches work then resume listed default retains correct profile", async () => {
    const { bridge, invokes } = harness();
    // 1. List sessions for default profile
    const list = await bridge.listSavedSessions(null, "default");
    assert.equal(list.length, 1);
    assert.equal(list[0].profile, "default", "listed row must be stamped with default profile");

    // 2. Active profile switches to work by opening a session in work profile
    await bridge.openSession("/work-project", "work");
    const activeTabProfile = invokes.find(i => i.command === "start_session" && i.args?.args?.profile === "work");
    assert.ok(activeTabProfile, "opened session in work profile");

    // 3. User resumes the listed default session while active tab is in work profile
    invokes.length = 0;
    const resumedId = await bridge.resumeSession(list[0]);
    assert.ok(resumedId);

    // 4. Must spawn under "default" profile (the stamped profile), NOT "work"
    const resumeCall = invokes.find(i => i.command === "start_session");
    assert.ok(resumeCall);
    assert.equal(resumeCall.args?.args?.profile, "default", "must retain listed default profile");
    assert.equal(resumeCall.args?.args?.resume, "/path/to/default-sess.jsonl");
  });

  await test("resume older branch opens exact requested file despite resume_path metadata", async () => {
    const { bridge, invokes } = harness();
    // Historical branch item with resume_path pointing to newer family head
    const olderBranch = {
      path: "/sessions/older-branch.jsonl",
      resume_path: "/sessions/family-newest.jsonl",
      family_path: "/sessions/family-root.jsonl",
      profile: "default",
      title: "Older Branch",
    };

    const id = await bridge.resumeSession(olderBranch);
    assert.ok(id);
    const spawnCall = invokes.find(i => i.command === "start_session");
    assert.ok(spawnCall);
    // Must open exact requested file, NOT redirected to resume_path
    assert.equal(spawnCall.args?.args?.resume, "/sessions/older-branch.jsonl", "must honor exact session.path");
  });

  await test("resume same session twice focuses existing tab (only one spawn)", async () => {
    const { bridge, invokes } = harness();
    const session = {
      path: "/sessions/conversation-a.jsonl",
      profile: "default",
      title: "Conv A",
    };

    const id1 = await bridge.resumeSession(session);
    assert.ok(id1);
    const spawnCalls1 = invokes.filter(i => i.command === "start_session").length;
    assert.equal(spawnCalls1, 1);

    // Resume same session a second time
    const id2 = await bridge.resumeSession(session);
    assert.equal(id2, id1, "must return existing tab id");
    const spawnCalls2 = invokes.filter(i => i.command === "start_session").length;
    assert.equal(spawnCalls2, 1, "must not spawn a second tab");
  });

  await test("failed spawn clean rollback", async () => {
    const { bridge } = harness({ failSpawn: true });
    const session = {
      path: "/sessions/failing.jsonl",
      profile: "work",
      title: "Will Fail",
    };

    await assert.rejects(
      async () => { await bridge.resumeSession(session); },
      /backend spawn rejected/,
    );
  });

  await test("close cancellation prevents delete", async () => {
    const { bridge, invokes } = harness();
    // Open a session running a file
    const session = {
      path: "/sessions/active-busy.jsonl",
      profile: "default",
      title: "Active Busy",
    };
    const tabId = await bridge.resumeSession(session);
    assert.ok(tabId);

    // Cancel close confirm when asked
    setTimeout(() => {
      bridge.resolveCloseConfirm(false);
    }, 10);

    invokes.length = 0;
    const res = await bridge.deleteSavedSession("/sessions/active-busy.jsonl", "default");
    assert.equal(res.ok, false);
    assert.match(res.error, /cancelled/i);

    // delete_saved_session must NOT have been called on backend
    const delCall = invokes.find(i => i.command === "delete_saved_session");
    assert.equal(delCall, undefined, "backend delete must not be called when close was cancelled");
  });

  await test("backend delete error returns error object", async () => {
    const { bridge } = harness({ failDelete: true });
    const res = await bridge.deleteSavedSession("/sessions/locked.jsonl", "default");
    assert.deepEqual(res, { ok: false, error: "Error: could not delete: permission denied" });
  });

  await test("batch deletion preserves open files while deleting closed siblings", async () => {
    const { bridge, invokes } = harness();
    const openPath = "/sessions/open-branch.jsonl";
    const closedPath = "/sessions/closed-parent.jsonl";
    await bridge.resumeSession({ path: openPath, profile: "default", title: "Open branch" });
    const result = await bridge.batchDeleteSavedSessions([openPath, closedPath], "default");
    assert.deepEqual(result, { ok: true, value: [closedPath] });
    const deletion = invokes.find(call => call.command === "batch_delete_saved_sessions");
    assert.deepEqual(deletion.args.paths, [closedPath]);
  });

  for (const mode of ["auto", "manual"]) {
    await test(`${mode} compaction blocks prompts and releases on failure or completion`, async () => {
      const { bridge, invokes, emit } = harness();
      const id = await bridge.openSession("/context-project", "default");
      let snapshot;
      bridge.onUpdate(value => { snapshot = value; });
      emit(id, { type: `${mode}_compaction_start` });
      assert.equal(snapshot.isCompacting, true);
      invokes.length = 0;
      assert.equal(await bridge.send("keep this draft"), false);
      assert.equal(await bridge.steer("not yet"), false);
      assert.equal(await bridge.followUp("not yet either"), false);
      assert.equal(invokes.some(call => call.command === "send_command"), false);
      assert.equal(snapshot.messages.some(message => message.text === "keep this draft"), false);

      emit(id, { type: `${mode}_compaction_end`, aborted: true, errorMessage: "cancelled" });
      assert.equal(snapshot.isCompacting, false);
      assert.equal(await bridge.send("now allowed"), true);
      assert.equal(snapshot.messages.some(message => message.text === "now allowed"), true);
    });
  }

  await test("manual compact locks immediately, then current context refresh remains separate from full history", async () => {
    const { bridge, invokes, emit } = harness();
    const id = await bridge.openSession("/context-project", "default");
    let snapshot;
    bridge.onUpdate(value => { snapshot = value; });
    bridge.compact();
    assert.equal(snapshot.isCompacting, true);
    assert.equal(await bridge.send("blocked before start event"), false);
    emit(id, { type: "manual_compaction_end", aborted: false, result: { summary: "earlier work" } });
    emit(id, { type: "response", command: "get_state", success: true, data: {
      isCompacting: false, contextUsage: { tokens: 100, contextWindow: 1000 },
      systemPrompt: ["system"], dumpTools: [],
    } });
    const currentRequest = invokes.filter(call => call.command === "send_command")
      .map(call => JSON.parse(call.args.json)).findLast(frame => frame.id?.startsWith("context-"));
    emit(id, { type: "response", command: "get_messages", id: currentRequest.id, success: true, data: {
      messages: [{ role: "compactionSummary", summary: "earlier work" }],
    } });
    assert.equal(snapshot.isCompacting, false);
    assert.ok(snapshot.ctxBreakdown.parts.find(part => part.key === "compacted").tokens > 0);
    assert.equal(snapshot.messages.some(message => message.text === "earlier work"), false);
  });

  await test("UI history and live events deduplicate, and a submitted form cannot be sent twice or to another conversation", async () => {
    const { bridge, invokes, emit } = harness();
    const id = await bridge.openSession("/ui-project", "default");
    let snapshot;
    bridge.onUpdate(value => { snapshot = value; });
    const rpcState = { sessionId: "ui-conversation", isCompacting: false, systemPrompt: [], dumpTools: [] };
    emit(id, { type: "response", command: "get_state", success: true, data: rpcState });
    const presentation = {
      role: "custom", customType: "ui.present", timestamp: 100,
      details: { ui_id: "ui-example", session_id: "ui-conversation", task_id: "ui-tool", message_id: "assistant-entry", status: "waiting_input", rendered: false, schema: {
        version: 1, title: "Subagent settings", layout: "stack", components: [
          { id: "concurrency", type: "slider", label: "Concurrency", min: 1, max: 32, step: 1, default: 4 },
          { id: "reasoning", type: "select", label: "Reasoning", options: [{ label: "Low", value: "low" }, { label: "High", value: "high" }], default: "low" },
          { id: "submit", type: "button", label: "Submit" },
        ],
      } },
    };
    emit(id, { type: "tool_execution_start", toolName: "ui.present", toolCallId: "ui-tool" });
    emit(id, { type: "message_end", message: presentation });
    emit(id, { type: "message_end", message: presentation });
    emit(id, { type: "response", command: "get_messages", success: true, data: { messages: [presentation] } });
    assert.equal(snapshot.messages.filter(message => message.kind === "ui").length, 1);
    assert.equal(snapshot.messages.some(message => message.kind === "tool"), false);
    const card = snapshot.messages.find(message => message.kind === "ui");
    const values = { concurrency: 7, reasoning: "high" };
    const submitted = bridge.submitUi(card, values);
    const request = invokes.filter(call => call.command === "send_command")
      .map(call => JSON.parse(call.args.json)).findLast(frame => frame.type === "submit_ui");
    emit(id, { type: "response", command: "submit_ui", id: request.id, success: true, data: {
      ui_id: card.ui_id, session_id: card.session_id, task_id: card.task_id, message_id: card.message_id,
      status: "submitted", rendered: true, continuation: "admitted",
    } });
    await submitted;
    assert.equal(snapshot.messages.find(message => message.kind === "ui").status, "submitted");
    invokes.length = 0;
    await assert.rejects(bridge.submitUi(card, values));
    emit(id, { type: "response", command: "get_state", success: true, data: { ...rpcState, sessionId: "another-conversation" } });
    await assert.rejects(bridge.submitUi(card, values));
    assert.equal(invokes.some(call => call.command === "send_command" && JSON.parse(call.args.json).type === "submit_ui"), false);
  });

  await test("rendering is not an answer, and cancelling a form releases the waiting task across history replay", async () => {
    const { bridge, invokes, emit } = harness();
    const id = await bridge.openSession("/ui-cancel", "default");
    let snapshot;
    bridge.onUpdate(value => { snapshot = value; });
    emit(id, { type: "response", command: "get_state", success: true, data: {
      sessionId: "cancel-conversation", isStreaming: false, systemPrompt: [], dumpTools: [],
    } });
    const details = {
      ui_id: "cancel-card", session_id: "cancel-conversation", task_id: "cancel-tool", message_id: "cancel-assistant",
      status: "waiting_input", rendered: false,
      schema: { version: 1, components: [
        { type: "number", id: "workers", label: "Workers", min: 1, max: 32, default: 4 },
        { type: "button", id: "send", label: "Apply" },
      ] },
    };
    const presentation = { role: "custom", customType: "ui.present", details };
    emit(id, { type: "message_end", message: presentation });
    const card = snapshot.messages.find(message => message.kind === "ui");
    assert.equal(snapshot.sessions.find(session => session.id === id).runState, "waiting-user");
    const rendered = bridge.renderedUi(card);
    const renderRequest = invokes.filter(call => call.command === "send_command")
      .map(call => JSON.parse(call.args.json)).findLast(frame => frame.type === "render_ui");
    const renderedDetails = { ...details, rendered: true };
    emit(id, { type: "response", id: renderRequest.id, command: "render_ui", success: true, data: renderedDetails });
    await rendered;
    assert.equal(snapshot.messages.find(message => message.kind === "ui").rendered, true);
    assert.equal(snapshot.sessions.find(session => session.id === id).runState, "waiting-user");
    const cancelled = bridge.cancelUi(card);
    const cancelRequest = invokes.filter(call => call.command === "send_command")
      .map(call => JSON.parse(call.args.json)).findLast(frame => frame.type === "cancel_ui");
    const cancellation = { role: "custom", customType: "ui.state", details: { ...renderedDetails, status: "cancelled" } };
    emit(id, { type: "message_end", message: cancellation });
    emit(id, { type: "response", id: cancelRequest.id, command: "cancel_ui", success: true, data: cancellation.details });
    await cancelled;
    emit(id, { type: "response", command: "get_messages", success: true, data: { messages: [presentation, cancellation] } });
    assert.equal(snapshot.messages.filter(message => message.kind === "ui").length, 1);
    assert.equal(snapshot.messages.find(message => message.kind === "ui").status, "cancelled");
    assert.equal(snapshot.sessions.find(session => session.id === id).runState, "idle");
    await assert.rejects(bridge.submitUi(card, { workers: 8 }));
  });

  console.log(`PASS test-history-actions-bridge.mjs (${passed} consumer scenarios)`);
}

run();
