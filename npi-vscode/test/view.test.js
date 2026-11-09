'use strict';
const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const { createRequire } = require('node:module');

function view() {
  const filename = path.resolve(__dirname, '../src/extension.js');
  const localRequire = createRequire(filename);
  const vscode = {
    workspace: { getConfiguration: () => ({ get: () => 'write' }) },
    window: { createOutputChannel: () => ({ appendLine() {}, dispose() {} }), registerWebviewViewProvider() {} },
    commands: { registerCommand() {} }
  };
  const module = { exports: {} };
  vm.runInNewContext(fs.readFileSync(filename, 'utf8'), {
    require: name => name === 'vscode' ? vscode : localRequire(name), module, Buffer, setTimeout, clearTimeout
  }, { filename });
  const { provider } = module.exports.activate({ subscriptions: [] });
  return provider;
}

test('slash command output survives subsequent snapshots and sidebar recreation', () => {
  const provider = view();
  provider.frame({ type: 'command_output', text: 'Configured tools: read, write' });
  let snapshot;
  provider.view = { webview: { postMessage: value => { snapshot = value; } } };
  provider.snapshot();
  assert.equal(snapshot.messages.length, 1);
  assert.equal(snapshot.messages[0].message.content[0].text, 'Configured tools: read, write');
  provider.frame({ type: 'agent_start' });
  assert.equal(snapshot.messages[0].message.content[0].text, 'Configured tools: read, write');
});

test('running tool output survives sidebar recreation until authoritative history reload', () => {
  const provider = view();
  provider.frame({ type: 'tool_execution_start', toolCallId: 'build', toolName: 'bash', args: { command: 'build' } });
  provider.frame({ type: 'tool_execution_update', toolCallId: 'build', toolName: 'bash', partialResult: { content: [{ type: 'text', text: 'Compiling' }] } });
  let snapshot;
  provider.view = { webview: { postMessage: value => { snapshot = value; } } };
  provider.snapshot();
  assert.equal(snapshot.tools.length, 1);
  assert.equal(snapshot.tools[0].partialResult.content[0].text, 'Compiling');
});
