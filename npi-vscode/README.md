# NPi for VScode

The Code-only VS Code client for NPi. The macOS Apple Silicon VSIX includes a standalone NPi backend (`npi/2.0.10`), started as a private `--mode rpc-ui` subprocess. No separate CLI installation or PATH setup is required. It uses the same system configuration and provider/subscription authentication as NPi CLI and Desktop, not a second credential store. Desktop's Chat and Academic modes are not included. The extension interface is English.

## Getting started

1. Install VS Code 1.96 or later on macOS Apple Silicon.
2. Choose **Install from VSIX…** in the VS Code Extensions menu and install `npi-vscode-0.1.1-darwin-arm64.vsix` from this directory.
3. Open and trust a local project folder. Select **NPi** in the Activity Bar, then **Connect to NPi**. Multi-root workspaces prompt you to choose a working directory.
4. Existing NPi settings and sign-in are used automatically. To add a subscription, run **NPi: Sign In to Subscription**, or choose `/login` from the **/** command menu.
5. Describe your task. Press Enter to send or Shift+Enter for a newline. While NPi is working, queue another task or select **Stop**.

If an older installation explicitly set `npi.executablePath`, clear that setting to use the bundled backend. Otherwise an explicit custom executable remains authoritative.

## Code features

- Streaming Markdown responses, collapsible thinking, tool arguments, execution output, and errors.
- Native model and reasoning-effort pickers populated from your NPi configuration.
- A compact composer keeps attachment, model, and reasoning controls inside the input card, with send/queue and stop controls aligned on the right. **Local**, the startup approval policy, logs, and reconnect remain below the card. Long model names truncate without displacing the action buttons.
- New sessions, session history, and automatic restoration of the latest session when reconnecting. Sessions are named after the first 80 characters of their first task.
- Use the **+** context menu to attach the current file or selected code, including unsaved editor content. Attachments appear inside the composer and can be removed without submitting the draft. Each attachment is limited to 256 KiB and captures the content at attachment time; other editor files are not attached automatically.
- **NPi: Attach Selection** in the editor context menu. Shortcut: `Cmd+Alt+N` on macOS, `Ctrl+Alt+N` elsewhere.
- Tool approvals and `ask` requests use native VS Code pickers, with multiple selections and custom answers. Text answers use VS Code's single-line input box rather than a separate multiline editor.
- **NPi: Show Agent Logs** displays subprocess errors. Hiding the sidebar does not stop the agent. Closing the window or restarting the agent cancels the running task and disposes its subprocess.
- **/** opens commands discovered from the running kernel, plus native sign-in, model, new-session, and history actions. Selecting a kernel command inserts it into the draft so you can supply arguments before sending.
- `/login`, `/model`, `/models`, `/new`, and `/resume` invoke native extension actions instead of being sent to the model. Cancelling a picker keeps your draft and attachments.
- Command output survives later sidebar snapshots; running tool output is restored when the sidebar is recreated. Command output is transient UI state, not part of NPi's persisted conversation.
- **Terminal** / **NPi: Open Full NPi Terminal** launches the bundled backend's complete terminal UI for settings, dashboards, and terminal-only commands. `/settings` opens that interface, not a webview settings clone. This is a separate NPi terminal session, not the sidebar conversation.

## Backend and shared configuration

The extension resolves `runtime/darwin-arm64/npi` inside the installed extension unless `npi.executablePath` is explicitly set. It does not discover or launch Desktop, attach to another NPi process, copy credentials into VS Code, or install a second global CLI.

Without environment overrides, the backend reads `~/.local/share/npi/agent/`, including model/provider configuration, subscription credentials, skills, MCP configuration, and model roles. `npi.profile` selects the same named NPi profile as the CLI. Existing NPi environment overrides (`PI_CONFIG_DIR`, `PI_CODING_AGENT_DIR`, profile variables) are inherited unchanged. Restart the agent after changing a sign-in or configuration in another client.

Authentication, OAuth refresh, and subscription routing remain owned by NPi. Signing in from this extension changes the shared NPi account state, not an extension-only account. No additional subscription is needed; usage still counts against the selected provider's subscription/API allowance.

## Settings

| Setting | Default | Description |
| --- | --- | --- |
| `npi.executablePath` | Empty | Bundled backend by default. Optional executable name on PATH or absolute path for a custom NPi kernel, not a shell command. |
| `npi.profile` | Empty | An existing NPi profile. Empty uses the default model, authentication, and tool configuration. |
| `npi.approvalMode` | `write` | NPi allows reads/writes and asks before execution. `always-ask` also asks before writes; `always-ask-cached` can remember file grants. No yolo option. |

Run **NPi: Restart Agent** after changing settings. These are user/machine settings; project configuration cannot override the executable or approval policy.

## Data and permissions

The extension runs only in trusted filesystem workspaces. Once you choose a workspace and connect, the Code agent can read and edit files and run commands according to NPi's tool policies. Workspace Trust is not an operating-system sandbox.

Sessions are stored in this extension's VS Code global storage under `sessions/<workspace-and-profile-hash>/`, with history indexed in workspace extension state. They do not resume or take over Desktop sessions. NPi continues to manage model credentials; the extension does not store a separate API key. Tasks, attached code, and files read by the agent are processed according to your selected model provider's policies.

The webview uses local resources, a strict CSP, and Markdown with raw HTML disabled. The extension includes no remote scripts or telemetry. English interface labels do not change the language of your prompts, model responses, or provider-supplied messages.

## Development and packaging

```bash
cd npi-vscode
npm ci --ignore-scripts
npm run check
npm test
npm run package
```

`npm run package` stages the existing signed Desktop sidecar from `../desktop/src-tauri/binaries/npi-aarch64-apple-darwin` and builds a platform-targeted VSIX. An optional executable path may be passed with `npm run package -- /path/to/npi`. The script does not rebuild or install Desktop or the kernel, and fails rather than producing a package without a backend. Staged executables under `runtime/` are ignored by Git but included in the VSIX.

After staging the runtime, open `npi-vscode/` as a folder in VS Code and press F5 to launch an Extension Development Host. Open a project folder in that host. Runtime resolution is shared by the sidebar and full terminal command.

## Verification scope

Verified on macOS arm64 with VS Code 1.141.0 and the real `npi/2.0.10` kernel. An isolated HOME, credential directory, and VS Code user-data directory, together with a local deterministic OpenAI-compatible endpoint, exercised sidebar prompts, selection attachments, real tool writes and native approvals, model/reasoning discovery, new sessions, session switching, and reconnect restoration. No real cloud model requests or other operating systems/Remote SSH were tested. Transport regressions cover large UTF-8 histories, out-of-order response correlation, invalid chunks, and process exit.

The English interface was checked in the actual VS Code webview, including its empty view, accessible labels, queued/busy controls, attachment removal, thinking, arguments, and running/success/failure tool states. Localization state checks used temporary display fixtures without model requests.

Version 0.1.1 was exercised in a real VS Code 1.141.0 Development Host with a PATH containing no `npi`, temporary HOME and VS Code storage, and configuration/authentication only in the standard NPi system directory within that HOME. The bundled backend loaded the configured model and sent the configured synthetic bearer key to a local deterministic endpoint. Sidebar model requests, actual file writes, native exec approval, kernel command discovery/output retention, subscription provider discovery (cancelled without sign-in), reconnect/history restoration, and the full NPi terminal UI were exercised. Five targeted regressions pass, including two retention bugs reproduced before their fix. No real subscription credentials or paid cloud requests were used; real OAuth completion and other platforms/remote hosts remain unverified.
