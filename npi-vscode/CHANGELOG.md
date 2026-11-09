# Changelog

## 0.1.1 — 2026-11-08

- Bundle the signed macOS Apple Silicon NPi backend in the platform-targeted VSIX; remove the default dependency on a globally installed CLI or VS Code's PATH.
- Reuse NPi's system configuration, profiles, provider credentials, and subscription sign-in without copying credentials into extension storage. Preserve explicit executable overrides.
- Add a kernel-backed command picker, native subscription sign-in, and a full bundled NPi terminal for terminal-only settings and dashboards.
- Route local sign-in/model/session commands to native controls instead of sending them to the model.
- Retain command output through subsequent snapshots and running tool output through sidebar recreation. Add failing-before/passing-after retention regressions.
- Verify actual bundled-backend requests, synthetic shared authentication, file writes, native exec approval, reconnect/history, provider discovery, and the terminal UI in an isolated VS Code Development Host without `npi` on PATH.

## 0.1.0 — 2026-11-08

- Add a dedicated Code sidebar connected to the local NPi kernel over JSONL RPC v2.
- Support streaming conversations, tool execution, native approvals and questions, model/reasoning selection, and editor attachments.
- Persist extension sessions separately, with new-session, history, and reconnect support.
- Require a trusted filesystem workspace, default to write approvals, and never automatically approve tools.
- Add transport boundary regressions and actual macOS Extension Development Host verification.
- Use English throughout the interface, tool states, tooltips, accessible labels, and extension documentation; set the webview document language to English. User content and model response languages remain unchanged.
- Reorganize the composer into a Copilot-style input card: context menu, model and reasoning controls inside, right-aligned queue/stop actions, and a compact local-runtime/approval-policy row below. Verify idle/busy layouts at 260, 320, and 480 px and attachment removal without submitting a draft.
