'use strict';
(() => {
  const vscode = acquireVsCodeApi();
  const md = window.markdownit({ html: false, linkify: true, breaks: true });
  md.disable('image');
  const app = document.getElementById('app');
  app.innerHTML = '<header><div class="brand"><span class="mark">Π</span><strong>NPi</strong><span class="mode">CODE</span></div>' +
    '<div class="actions"><button data-action="sessions" title="Session history" aria-label="Session history">◷</button><button data-action="newSession" title="New session" aria-label="New session">＋</button><button data-action="settings" title="Settings" aria-label="Settings">⋯</button></div></header>' +
    '<div class="workspace"><span id="workspace">Open a project to get started</span><span id="status" role="status">Disconnected</span></div>' +
    '<div id="error" role="alert" hidden></div>' +
    '<section id="timeline" aria-label="Code conversation"><div id="welcome"><span class="welcome-mark">Π</span><h1>Start with the code.</h1>' +
    '<p>Explore your project, make changes,<br>and verify the result with NPi.</p><button class="primary" data-action="connect">Connect to NPi</button>' +
    '<p class="hint">Includes the NPi backend.<br>Uses the same settings and sign-in as NPi CLI and Desktop.</p><button data-action="login">Sign in</button></div><div id="messages"></div></section>' +
    '<footer><form id="composer"><div id="attachments" aria-label="Attached code"></div><textarea id="prompt" rows="3" placeholder="Describe what to build…" aria-label="Task input" title="Enter to send · Shift+Enter for newline"></textarea>' +
    '<div class="composer-tools"><div class="composer-options"><button type="button" class="icon-button" data-action="attach" title="Attach context" aria-label="Attach context">＋</button><button type="button" class="icon-button" data-action="commands" title="NPi commands" aria-label="NPi commands">/</button><span class="code-label">Code</span>' +
    '<button type="button" id="model" data-action="model" title="Choose model" aria-label="Choose model"><span id="model-label">Models</span><span class="chevron" aria-hidden="true">⌄</span></button>' +
    '<button type="button" id="thinking" class="icon-button" data-action="thinking" title="Reasoning effort" aria-label="Reasoning effort"><svg viewBox="0 0 16 16" aria-hidden="true"><path d="M2 4h12M2 8h12M2 12h12M5 2v4M11 6v4M6 10v4"/></svg></button></div>' +
    '<div class="composer-submit"><button id="stop" type="button" class="icon-button" data-action="stop" title="Stop task" aria-label="Stop task" hidden>■</button><button id="send" type="submit" class="icon-button primary" title="Send message" aria-label="Send message">↑</button></div></div></form>' +
    '<div class="composer-meta"><div class="runtime-info"><span class="local-label"><svg viewBox="0 0 16 16" aria-hidden="true"><rect x="2" y="2" width="12" height="9" rx="1"/><path d="M5 14h6M8 11v3"/></svg>Local</span><button id="approval" data-action="settings" title="Approval policy">Write policy</button></div>' +
    '<div class="utility-actions"><button data-action="terminal" title="Full NPi terminal interface">Terminal</button><button data-action="logs" title="Show agent logs">Logs</button><button data-action="restart" title="Restart agent">Reconnect</button></div></div></footer>';
  const timeline = document.getElementById('timeline');
  const messages = document.getElementById('messages');
  const input = document.getElementById('prompt');
  const welcome = document.getElementById('welcome');
  const send = document.getElementById('send');
  const stop = document.getElementById('stop');
  const state = vscode.getState() || {};
  input.value = state.draft || '';
  let connected = false;
  let sending = false;
  const nodes = new Map();
  const tools = new Map();
  const escape = text => String(text).replace(/[&<>"']/g, character => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[character]);
  const post = (type, fields = {}) => vscode.postMessage({ type, ...fields });
  const nearBottom = () => timeline.scrollHeight - timeline.scrollTop - timeline.clientHeight < 90;
  const scroll = () => { timeline.scrollTop = timeline.scrollHeight; };
  const plain = content => typeof content === 'string' ? content : (content || []).filter(block => block.type === 'text').map(block => block.text).join('\n');

  function renderMessage(id, message) {
    if (!['user', 'assistant', 'toolResult', 'custom'].includes(message.role)) return;
    const sticky = nearBottom();
    let node = nodes.get(id);
    if (!node) {
      node = document.createElement('article');
      nodes.set(id, node);
      messages.append(node);
    }
    node.className = 'message ' + message.role;
    if (message.role === 'toolResult') {
      let card = tools.get(message.toolCallId);
      if (!card) {
        card = document.createElement('details');
        card.className = 'tool-card';
        tools.set(message.toolCallId, card);
      }
      card.innerHTML = '<summary><span class="tool-indicator ' + (message.isError ? 'failed' : 'done') + '"></span>' + escape(message.toolName || 'Tool') +
        '<span class="tool-state">' + (message.isError ? 'Failed' : 'Done') + '</span></summary><pre>' + escape(plain(message.content)) + '</pre>';
      node.replaceChildren(card);
    } else {
      const content = typeof message.content === 'string' ? [{ type: 'text', text: message.content }] : message.content || [];
      const body = content.map(block => {
        if (block.type === 'text') return '<div class="prose">' + md.render(block.text) + '</div>';
        if (block.type === 'thinking' && block.thinking) return '<details class="thinking"><summary>Thinking</summary><div class="prose">' + md.render(block.thinking) + '</div></details>';
        if (block.type === 'toolCall') return '<div class="call"><span class="call-icon">↳</span><code>' + escape(block.name) + '</code><details><summary>Arguments</summary><pre>' + escape(JSON.stringify(block.arguments, null, 2)) + '</pre></details></div>';
        if (block.type === 'image') return '<p class="hint">[Image attachment]</p>';
        return '';
      }).join('');
      node.innerHTML = '<div class="speaker">' + (message.role === 'user' ? 'YOU' : message.role === 'assistant' ? 'NPi' : escape(message.customType || 'NPi')) + '</div>' + body +
        (message.errorMessage ? '<p class="inline-error">' + escape(message.errorMessage) + '</p>' : '');
    }
    welcome.hidden = true;
    if (sticky) scroll();
  }

  function toolEvent(frame) {
    const sticky = nearBottom();
    let card = tools.get(frame.toolCallId);
    if (!card) {
      card = document.createElement('details');
      card.className = 'tool-card';
      tools.set(frame.toolCallId, card);
      messages.append(card);
    }
    const complete = frame.type === 'tool_execution_end';
    const result = frame.result || frame.partialResult;
    const output = result ? plain(result.content) : JSON.stringify(frame.args, null, 2);
    card.innerHTML = '<summary><span class="tool-indicator ' + (complete ? frame.isError ? 'failed' : 'done' : '') + '"></span>' +
      escape(frame.toolName) + '<span class="tool-state">' + (complete ? frame.isError ? 'Failed' : 'Done' : 'Running') +
      '</span></summary><pre>' + escape(output || '') + '</pre>';
    if (sticky) scroll();
  }

  window.addEventListener('message', event => {
    const data = event.data;
    if (data.type === 'snapshot') {
      connected = data.connected;
      document.getElementById('workspace').textContent = data.workspace || 'Open a project to get started';
      document.getElementById('status').textContent = data.status;
      document.getElementById('status').classList.toggle('active', data.busy);
      const error = document.getElementById('error');
      error.textContent = data.error;
      error.hidden = !data.error;
      document.getElementById('model-label').textContent = data.model ? data.model.slice(data.model.indexOf('/') + 1) : 'Models';
      document.getElementById('model').title = data.model || 'Choose model';
      document.getElementById('thinking').title = data.thinking ? 'Reasoning effort: ' + data.thinking : 'Reasoning effort';
      document.getElementById('approval').textContent = ({ write: 'Write policy', 'always-ask': 'Ask every time', 'always-ask-cached': 'Ask with cache' })[data.approvalMode];
      document.getElementById('approval').title = 'Startup approval policy: ' + data.approvalMode + '. Additional NPi tool rules still apply. Open settings to configure.';
      document.getElementById('model').disabled = data.busy;
      document.getElementById('thinking').disabled = data.busy;
      stop.hidden = !data.busy;
      send.title = data.busy ? 'Queue message' : 'Send message';
      send.setAttribute('aria-label', send.title);
      send.disabled = sending;
      const ids = new Set(data.messages.map(item => item.id));
      for (const [id, node] of nodes) if (!ids.has(id)) { node.remove(); nodes.delete(id); }
      if (!data.messages.length) { messages.replaceChildren(); nodes.clear(); tools.clear(); }
      data.messages.forEach(item => renderMessage(item.id, item.message));
      data.tools.forEach(toolEvent);
      welcome.hidden = data.messages.length > 0;
      welcome.querySelector('[data-action="connect"]').hidden = connected;
      document.getElementById('attachments').innerHTML = data.attachments.map(item => '<button type="button" class="attachment" data-remove="' + escape(item.id) + '" title="Click to remove">' + escape(item.label) + ' ×</button>').join('');
      if (data.error) { sending = false; send.disabled = false; }
    } else if (data.type === 'message') renderMessage(data.id, data.message);
    else if (data.type === 'tool') toolEvent(data.frame);
    else if (data.type === 'draft') { input.value = data.text; input.focus(); }
    else if (data.type === 'sendComplete') { sending = false; send.disabled = false; }
    else if (data.type === 'sent') {
      sending = false;
      send.disabled = false;
      if (input.value === data.text) input.value = '';
      vscode.setState({ draft: input.value });
      input.focus();
    }
  });

  app.addEventListener('click', event => {
    const button = event.target.closest('button');
    if (!button) return;
    if (button.dataset.action) post(button.dataset.action);
    if (button.dataset.remove) post('removeAttachment', { id: button.dataset.remove });
  });
  document.getElementById('composer').addEventListener('submit', event => {
    event.preventDefault();
    if (!input.value.trim() || sending) return;
    sending = true;
    send.disabled = true;
    post('send', { text: input.value });
  });
  input.addEventListener('input', () => vscode.setState({ draft: input.value }));
  input.addEventListener('input', () => {
    if (input.value === '/') post('commands');
  });
  input.addEventListener('keydown', event => {
    if (event.key === 'Enter' && !event.shiftKey && !event.isComposing) {
      event.preventDefault();
      document.getElementById('composer').requestSubmit();
    }
  });
  post('ready');
})();
