'use strict';
const vscode = require('vscode');
const fs = require('node:fs/promises');
const path = require('node:path');
const { randomBytes, createHash } = require('node:crypto');
const { RpcClient } = require('./rpc');
const { resolveBackend } = require('./backend');

class CodeView {
  constructor(context) {
    this.context = context;
    this.output = vscode.window.createOutputChannel('NPi Code');
    this.messages = new Map();
    this.tools = new Map();
    this.attachments = [];
    this.dialogs = new Map();
    this.status = 'Disconnected';
    this.busy = false;
    this.connected = false;
    this.error = '';
    this.disposed = false;
    this.workspace = null;
    this.state = {};
    this.approvalMode = vscode.workspace.getConfiguration('npi').get('approvalMode');
  }

  resolveWebviewView(view) {
    this.view = view;
    const webview = view.webview;
    webview.options = { enableScripts: true, localResourceRoots: [this.context.extensionUri] };
    const resource = file => webview.asWebviewUri(vscode.Uri.joinPath(this.context.extensionUri, file));
    const nonce = randomBytes(24).toString('hex');
    webview.html = '<!DOCTYPE html><html lang="en"><head><meta charset="UTF-8">' +
      '<meta name="viewport" content="width=device-width,initial-scale=1">' +
      '<meta http-equiv="Content-Security-Policy" content="default-src \'none\'; style-src ' + webview.cspSource +
      '; script-src \'nonce-' + nonce + '\'; font-src ' + webview.cspSource + ';">' +
      '<link rel="stylesheet" href="' + resource('media/main.css') + '"><title>NPi Code</title></head>' +
      '<body><main id="app"></main><script nonce="' + nonce + '" src="' + resource('node_modules/markdown-it/dist/markdown-it.min.js') + '"></script>' +
      '<script nonce="' + nonce + '" src="' + resource('media/main.js') + '"></script></body></html>';
    view.onDidDispose(() => { if (this.view === view) this.view = undefined; });
    webview.onDidReceiveMessage(message => {
      this.action(message).catch(error => this.report(error));
    });
  }

  snapshot() {
    this.view?.webview.postMessage({ type: 'snapshot', connected: this.connected, busy: this.busy,
      status: this.status, error: this.error, workspace: this.workspace?.name || '',
      model: this.state.model ? this.state.model.provider + '/' + this.state.model.id : '',
      thinking: this.state.thinkingLevel || '', sessionName: this.state.sessionName || 'Code session',
      approvalMode: this.approvalMode,
      tools: [...this.tools.values()],
      attachments: this.attachments.map(({ id, label }) => ({ id, label })), messages: [...this.messages.values()] });
  }

  report(error) {
    this.error = error.message;
    this.output.appendLine(error.stack || error.message);
    this.snapshot();
  }

  async show() {
    await vscode.commands.executeCommand('npi.code.focus');
  }

  async chooseWorkspace() {
    if (!vscode.workspace.isTrusted) throw new Error('Trust this workspace before starting NPi.');
    const folders = (vscode.workspace.workspaceFolders || []).filter(folder => folder.uri.scheme === 'file');
    if (!folders.length) throw new Error('Open a local folder in VS Code before starting NPi.');
    const folder = folders.length === 1 ? folders[0] : await vscode.window.showWorkspaceFolderPick({ placeHolder: 'Choose the workspace NPi can edit' });
    if (!folder) throw new Error('No workspace selected.');
    return folder;
  }

  async connect() {
    if (this.connected) return;
    if (this.connecting) return this.connecting;
    this.connecting = this.start().finally(() => { this.connecting = null; });
    return this.connecting;
  }

  async start() {
    this.workspace = await this.chooseWorkspace();
    const config = vscode.workspace.getConfiguration('npi');
    const profile = config.get('profile');
    const key = createHash('sha256').update(this.workspace.uri.toString() + ':' + profile).digest('hex').slice(0, 20);
    this.sessionKey = 'sessions.' + key;
    const sessionDir = path.join(this.context.globalStorageUri.fsPath, 'sessions', key);
    await fs.mkdir(sessionDir, { recursive: true });
    this.approvalMode = config.get('approvalMode');
    const args = ['--mode', 'rpc-ui', '--cwd', this.workspace.uri.fsPath,
      '--approval-mode=' + this.approvalMode, '--session-dir', sessionDir, '--continue'];
    if (profile) args.push('--profile=' + profile);
    this.error = '';
    this.status = 'Connecting…';
    this.snapshot();
    const executable = await resolveBackend(this.context.extensionPath, config.get('executablePath'));
    this.output.appendLine('Starting NPi backend: ' + executable);
    const client = new RpcClient(executable, args, { cwd: this.workspace.uri.fsPath });
    this.client = client;
    client.on('stderr', text => { this.output.append(text); });
    client.on('fault', error => { if (this.client === client) this.report(error); });
    client.on('frame', frame => { if (this.client === client) this.frame(frame); });
    client.on('close', event => {
      if (this.client !== client) return;
      this.connected = false;
      this.busy = false;
      this.status = event.expected ? 'Disconnected' : 'Agent stopped';
      this.cancelDialogs();
      this.snapshot();
    });
    try {
      await client.ready;
      if (this.disposed) { client.dispose(); return; }
      await client.request('set_ask_dialog', { enabled: true });
      await this.refresh();
      this.connected = true;
      this.status = 'Ready';
      this.snapshot();
    } catch (error) {
      client.dispose();
      await client.closed;
      this.status = 'Connection failed';
      throw error;
    }
  }

  async remember() {
    if (!this.state.sessionFile) return;
    const sessions = this.context.workspaceState.get(this.sessionKey, []);
    const entry = { file: this.state.sessionFile, name: this.state.sessionName || 'Code session', updated: Date.now() };
    await this.context.workspaceState.update(this.sessionKey, [entry, ...sessions.filter(item => item.file !== entry.file)]);
  }

  async refresh() {
    this.state = await this.client.request('get_state');
    const history = await this.client.request('get_messages', { fullHistory: true });
    this.messages.clear();
    this.tools.clear();
    history.messages.forEach((message, index) => this.messages.set('history-' + index, { id: 'history-' + index, message }));
    this.busy = !this.state.isSettled;
    await this.remember();
    this.snapshot();
  }

  frame(frame) {
    switch (frame.type) {
      case 'message_start':
      case 'message_update':
      case 'message_end':
        this.messages.set(frame.messageId, { id: frame.messageId, message: frame.message });
        this.view?.webview.postMessage({ type: 'message', id: frame.messageId, message: frame.message });
        break;
      case 'agent_start': this.busy = true; this.status = 'Working'; this.snapshot(); break;
      case 'agent_end':
        this.status = frame.awaitingAsyncWork ? 'Waiting for background work' : 'Finishing';
        this.snapshot();
        break;
      case 'session_settled':
        this.busy = false;
        this.status = 'Ready';
        this.client.request('get_state').then(state => { this.state = state; return this.remember(); })
          .then(() => this.snapshot()).catch(error => this.report(error));
        break;
      case 'prompt_result':
        if (frame.status === 'error') this.report(new Error(frame.error || 'The model request failed.'));
        if (frame.sessionSettled) { this.busy = false; this.status = 'Ready'; this.snapshot(); }
        break;
      case 'tool_execution_start':
      case 'tool_execution_update':
      case 'tool_execution_end':
        this.tools.set(frame.toolCallId, frame);
        this.view?.webview.postMessage({ type: 'tool', frame });
        break;
      case 'notice': if (frame.level === 'error') this.report(new Error(frame.message)); break;
      case 'extension_error': this.report(new Error(frame.error)); break;
      case 'command_output': {
        const id = 'command-' + randomBytes(8).toString('hex');
        const message = { role: 'custom', customType: 'Command', content: [{ type: 'text', text: frame.text }] };
        this.messages.set(id, { id, message });
        this.snapshot();
        break;
      }
      case 'session_info_update':
      case 'config_update':
      case 'model_changed':
      case 'thinking_level_changed':
        this.client.request('get_state').then(state => { this.state = state; this.snapshot(); }).catch(error => this.report(error));
        break;
      case 'extension_ui_request': this.dialog(frame).catch(error => this.report(error)); break;
    }
  }

  async dialog(frame) {
    if (frame.method === 'cancel') { this.dialogs.get(frame.targetId)?.cancel(); return; }
    if (frame.method === 'notify') {
      this.output.appendLine(frame.message || '');
      if (frame.notifyType === 'error') this.report(new Error(frame.message));
      return;
    }
    if (frame.method === 'set_editor_text') { this.view?.webview.postMessage({ type: 'draft', text: frame.text }); return; }
    if (['setStatus', 'setTitle', 'setWidget'].includes(frame.method)) return;
    if (frame.method === 'open_url') {
      const uri = vscode.Uri.parse(frame.launchUrl || frame.url);
      if (!['https', 'http'].includes(uri.scheme)) throw new Error('NPi requested an unsupported sign-in URL scheme.');
      const answer = await vscode.window.showInformationMessage(frame.instructions || 'Open NPi sign-in page in your browser?', 'Open sign-in page');
      if (answer) await vscode.env.openExternal(uri);
      return;
    }
    const client = this.client;
    const token = new vscode.CancellationTokenSource();
    this.dialogs.set(frame.id, token);
    let timedOut = false;
    const timer = frame.timeout ? setTimeout(() => { timedOut = true; token.cancel(); }, frame.timeout) : undefined;
    let response;
    try {
      if (frame.method === 'confirm') {
        const value = await vscode.window.showQuickPick([
          { label: 'Deny', allow: false }, { label: 'Allow', allow: true }
        ], { title: frame.title, placeHolder: frame.message, ignoreFocusOut: true }, token.token);
        response = value ? { confirmed: value.allow } : { cancelled: true, timedOut };
      } else if (frame.method === 'select') {
        const options = frame.options.map((label, index) => ({ label, index, description: frame.optionDetails?.[index]?.description }));
        const value = await vscode.window.showQuickPick(options, { title: frame.title, placeHolder: frame.message, ignoreFocusOut: true }, token.token);
        response = value ? { value: value.label } : { cancelled: true, timedOut };
      } else if (frame.method === 'input' || frame.method === 'editor') {
        const value = await vscode.window.showInputBox({ title: frame.title, prompt: frame.message,
          value: frame.prefill || '', placeHolder: frame.placeholder, password: frame.secret, ignoreFocusOut: true }, token.token);
        response = value === undefined ? { cancelled: true, timedOut } : { value };
      } else if (frame.method === 'ask') {
        const answers = [];
        for (const question of frame.questions) {
          if (token.token.isCancellationRequested) break;
          const custom = { label: 'Other…', custom: true };
          const items = question.options.map(option => ({ label: option.label, description: option.description, detail: option.preview }));
          const picked = await vscode.window.showQuickPick([...items, custom], {
            title: question.header || frame.title || 'NPi', placeHolder: question.question,
            canPickMany: Boolean(question.multi), ignoreFocusOut: true
          }, token.token);
          if (!picked) break;
          const selected = Array.isArray(picked) ? picked : [picked];
          let customInput;
          if (selected.some(item => item.custom)) {
            customInput = await vscode.window.showInputBox({ prompt: question.question, ignoreFocusOut: true }, token.token);
            if (customInput === undefined) break;
          }
          answers.push({ id: question.id, selectedOptions: selected.filter(item => !item.custom).map(item => item.label), ...(customInput !== undefined ? { customInput } : {}) });
        }
        response = answers.length === frame.questions.length ? { answers } : { cancelled: true, timedOut };
      } else {
        response = { cancelled: true };
        this.report(new Error('Unsupported NPi dialog: ' + frame.method));
      }
      client.send({ type: 'extension_ui_response', id: frame.id, ...response });
    } finally {
      clearTimeout(timer);
      this.dialogs.delete(frame.id);
      token.dispose();
    }
  }

  cancelDialogs() { for (const token of this.dialogs.values()) token.cancel(); }

  async attach(selectionOnly) {
    if (!vscode.workspace.isTrusted) throw new Error('Trust this workspace before attaching code.');
    const editor = vscode.window.activeTextEditor;
    if (!editor) throw new Error('Open a text file to attach it.');
    const range = selectionOnly ? editor.selection : undefined;
    if (range?.isEmpty) throw new Error('Select code to attach.');
    const text = editor.document.getText(range);
    if (Buffer.byteLength(text) > 262144) throw new Error('Attachment is larger than 256 KiB. Attach a smaller selection.');
    const name = vscode.workspace.asRelativePath(editor.document.uri);
    const location = range ? ':' + (range.start.line + 1) + '-' + (range.end.line + 1) : '';
    this.attachments.push({ id: randomBytes(8).toString('hex'), label: name + location,
      text: '\n\nAttached editor content (' + name + location + '; language=' + editor.document.languageId + '):\n' + text });
    await this.show();
    this.snapshot();
  }

  async action(message) {
    switch (message.type) {
      case 'ready': this.snapshot(); return;
      case 'connect': await this.connect(); return;
      case 'attach': {
        const choice = await vscode.window.showQuickPick([
          { label: 'Current file', description: 'Include unsaved editor content', selection: false },
          { label: 'Selected code', description: 'Attach the current selection', selection: true }
        ], { title: 'Attach context' });
        if (choice) await this.attach(choice.selection);
        return;
      }
      case 'attachFile': await this.attach(false); return;
      case 'attachSelection': await this.attach(true); return;
      case 'removeAttachment': this.attachments = this.attachments.filter(item => item.id !== message.id); this.snapshot(); return;
      case 'logs': this.output.show(true); return;
      case 'settings': await vscode.commands.executeCommand('workbench.action.openSettings', '@ext:npi.npi-vscode'); return;
      case 'commands': await this.pickCommand(); return;
      case 'login': await this.login(); return;
      case 'terminal': await this.openTerminal(); return;
      case 'newSession': await this.newSession(); return;
      case 'sessions': await this.sessions(); return;
      case 'restart': await this.restart(); return;
      case 'stop': await this.client?.request('abort'); return;
      case 'model': await this.pickModel(); return;
      case 'thinking': await this.pickThinking(); return;
      case 'send': {
        if (typeof message.text !== 'string' || !message.text.trim()) return;
        const localCommand = new Map([
          ['/login', () => this.login()], ['/model', () => this.pickModel()], ['/models', () => this.pickModel()],
          ['/new', () => this.newSession()], ['/resume', () => this.sessions()], ['/settings', () => this.openTerminal()]
        ]).get(message.text.trim());
        if (localCommand) {
          try { await localCommand(); }
          finally { this.view?.webview.postMessage({ type: 'sendComplete' }); }
          return;
        }
        await this.connect();
        const submittedAttachments = new Set(this.attachments.map(item => item.id));
        const prompt = message.text + this.attachments.map(item => item.text).join('');
        this.error = '';
        this.busy = true;
        this.status = 'Sending';
        this.snapshot();
        try {
          if (!this.state.sessionName) {
            await this.client.request('set_session_name', { name: message.text.trim().slice(0, 80) });
            this.state.sessionName = message.text.trim().slice(0, 80);
          }
          const result = await this.client.request('prompt', { message: prompt, streamingBehavior: 'followUp' });
          this.attachments = this.attachments.filter(item => !submittedAttachments.has(item.id));
          this.view?.webview.postMessage({ type: 'sent', text: message.text });
          if (result?.agentInvoked === false) {
            this.busy = false;
            this.status = 'Ready';
            this.state = await this.client.request('get_state');
            await this.remember();
          }
          this.snapshot();
        } catch (error) {
          if (this.connected) {
            this.state = await this.client.request('get_state');
            this.busy = !this.state.isSettled;
            this.status = this.busy ? 'Working' : 'Ready';
          }
          throw error;
        }
        return;
      }
    }
  }

  async newSession() {
    await this.connect();
    if (this.busy) throw new Error('Stop the current task before creating a session.');
    const result = await this.client.request('new_session');
    if (result?.cancelled) return;
    this.error = '';
    await this.refresh();
  }

  async sessions() {
    await this.connect();
    if (this.busy) throw new Error('Stop the current task before switching sessions.');
    const records = this.context.workspaceState.get(this.sessionKey, []);
    const selected = await vscode.window.showQuickPick(records.map(item => ({
      label: item.name, description: new Date(item.updated).toLocaleString(), detail: path.basename(item.file), file: item.file
    })), { title: 'NPi Code sessions — ' + this.workspace.name });
    if (!selected) return;
    const result = await this.client.request('switch_session', { sessionPath: selected.file });
    if (!result?.cancelled) { this.error = ''; await this.refresh(); }
  }

  async pickModel() {
    await this.connect();
    if (this.busy) throw new Error('Stop the current task before changing models.');
    const { models } = await this.client.request('get_available_models');
    const selected = await vscode.window.showQuickPick(models.map(model => ({ label: model.name || model.id,
      description: model.provider, detail: model.id, model })), { title: 'NPi model' });
    if (!selected) return;
    await this.client.request('set_model', { provider: selected.model.provider, modelId: selected.model.id });
    this.state = await this.client.request('get_state');
    this.snapshot();
  }

  async pickThinking() {
    await this.connect();
    if (this.busy) throw new Error('Stop the current task before changing reasoning effort.');
    const { levels } = await this.client.request('get_available_thinking_levels');
    const level = await vscode.window.showQuickPick(levels, { title: 'Reasoning effort' });
    if (level) { await this.client.request('set_thinking_level', { level }); this.state = await this.client.request('get_state'); this.snapshot(); }
  }
  async pickCommand() {
    await this.connect();
    const { commands } = await this.client.request('get_available_commands');
    const local = [
      { label: '/login', description: 'Sign in to a shared NPi subscription', action: () => this.login() },
      { label: '/model', description: 'Choose a configured model', action: () => this.pickModel() },
      { label: '/new', description: 'Start a new Code session', action: () => this.newSession() },
      { label: '/resume', description: 'Switch Code sessions', action: () => this.sessions() },
      { label: '/settings', description: 'Open the full NPi terminal interface', action: () => this.openTerminal() }
    ];
    const labels = new Set(local.map(item => item.label));
    const selected = await vscode.window.showQuickPick([...local, ...commands
      .filter(command => !labels.has('/' + command.name))
      .map(command => ({ label: '/' + command.name, description: command.description,
        detail: command.input?.hint, command }))], { title: 'NPi commands' });
    if (!selected) return;
    if (selected.action) await selected.action();
    else this.view?.webview.postMessage({ type: 'draft', text: selected.label + ' ' });
  }

  async login() {
    await this.connect();
    if (this.busy) throw new Error('Stop the current task before signing in.');
    const { providers } = await this.client.request('get_login_providers');
    const selected = await vscode.window.showQuickPick(providers.filter(provider => provider.available !== false)
      .map(provider => ({ label: provider.name, description: provider.authenticated ? 'Signed in — shared with NPi' : 'Sign in',
        provider })), { title: 'NPi subscriptions — shared system sign-in' });
    if (!selected) return;
    await this.client.request('login', { providerId: selected.provider.id });
    this.state = await this.client.request('get_state');
    this.snapshot();
    vscode.window.showInformationMessage('Signed in to ' + selected.provider.name + '. This sign-in is shared with NPi CLI and Desktop.');
  }

  async openTerminal() {
    const workspace = this.workspace || await this.chooseWorkspace();
    const config = vscode.workspace.getConfiguration('npi');
    const executable = await resolveBackend(this.context.extensionPath, config.get('executablePath'));
    const args = ['--cwd', workspace.uri.fsPath, '--approval-mode=' + config.get('approvalMode')];
    if (config.get('profile')) args.push('--profile=' + config.get('profile'));
    const terminal = vscode.window.createTerminal({ name: 'NPi', shellPath: executable,
      shellArgs: args, cwd: workspace.uri.fsPath });
    terminal.show();
    this.context.subscriptions.push(terminal);
  }


  async restart() {
    if (this.connecting) await this.connecting;
    if (this.busy) {
      const answer = await vscode.window.showWarningMessage('Restarting stops the running NPi task.', { modal: true }, 'Restart');
      if (answer !== 'Restart') return;
    }
    const previous = this.client;
    this.client = undefined;
    this.connected = false;
    this.cancelDialogs();
    previous?.dispose();
    // Wait for the child to release its session lease before reconnecting.
    if (previous) await previous.closed;
    await this.connect();
  }

  dispose() {
    this.disposed = true;
    this.cancelDialogs();
    this.client?.dispose();
    this.output.dispose();
  }
}

function activate(context) {
  const provider = new CodeView(context);
  context.subscriptions.push(provider, vscode.window.registerWebviewViewProvider('npi.code', provider));
  const commands = {
    open: () => provider.show(), newSession: () => provider.newSession(), sessions: () => provider.sessions(),
    attachSelection: () => provider.attach(true), attachFile: () => provider.attach(false),
    restart: () => provider.restart(), logs: () => provider.output.show(true),
    commands: () => provider.pickCommand(), login: () => provider.login(), terminal: () => provider.openTerminal()
  };
  for (const [name, action] of Object.entries(commands)) {
    context.subscriptions.push(vscode.commands.registerCommand('npi.' + name, () => action().catch(error => {
      provider.report(error); vscode.window.showErrorMessage(error.message);
    })));
  }
  return { provider };
}
module.exports = { activate };
