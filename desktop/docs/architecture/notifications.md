# System notifications

Code: `src-tauri/src/notify/` (`mod.rs` state, frame classification and commands; `policy.rs` pure rules and banner text; `macos.rs`; `other.rs`), `agent/reader.rs` (the hooks), `app/notify.js` (`OMP_NOTIFY`), `design/settings/rows-notifications.jsx`, `live.js` (`notify*` bridge methods, `notify://open`). Tests: `cargo test --lib notify`, `test-notify.mjs`.

## What was wrong

The old `system_notice.rs` ran `osascript -e 'display notification'` (macOS), `notify-send` or a PowerShell toast built by string formatting. On macOS that banner belongs to Script Editor, not to NPi Desktop: the app never had a permission of its own to ask for or to be refused, a failure was swallowed, and a click did nothing. The webview fired it only for the *active* tab (a background tab's frames are not processed there), only when `document.hasFocus()` was false, with no setting to turn it off, and the "turn finished" banner carried the first 80 characters of the reply. The PowerShell text was spliced into the script, so a quote in it broke (or injected into) the command.

## Policy lives in Rust

Only the stdout reader sees every tab's frames, so it hands each parsed frame to `NotifyState::observe` (one string compare for most frames). Three events become notifications:

|Kind|Frame|Rule|
|---|---|---|
|Done|a terminal `agent_end` (`yielded`, else `isTerminal !== false`: omp's `isYield`)|the run, timed from `agent_start`, lasted 8 s or more|
|NeedsInput|`extension_ui_request` with method `select`, `ask` or `confirm` that no stored approval rule answered (the reader only gets here for prompts the human will see)|always|
|Failed|a terminal `agent_end` whose last assistant message has `stopReason: "error"`, or an `agent://exit` with a reason (the process died)|always|

`policy::should_send` then applies: the master switch and the kind's own switch; no banner while the window has focus **and** the event's tab is the one on screen (a background tab of a focused window still notifies); one banner per tab and kind per 20 s. The Settings test button ignores all of it.

The webview sends only the switches and the language (`notify_configure`, from `OMP_NOTIFY.configFrom`: four booleans and one of `zh-CN`, `zh-TW`, `en-US`, `ja-JP`; anything else keeps the old language) and which tab is on screen (`notify_set_active`: `null` on the start page and the Settings page). A banner's title is the project folder name (cut to 60 characters) or "NPi Desktop"; its body is a fixed sentence per kind and language. No prompt, reply, tool or file text ever reaches it.

## macOS

`macos.rs` talks to `UNUserNotificationCenter` through `objc2-user-notifications`: `requestAuthorizationWithOptions` (alert and sound; the system shows its prompt once, the first time), `getNotificationSettings` for the state, `addNotificationRequest` to post. The Settings row asks when the master switch goes on, shows `granted`, `denied` (with "enable in System Settings → Notifications") or `notDetermined`, rechecks when the window regains focus, and has a **Send test notification** button (shown even while the app is in front, through `willPresentNotification`).

A request identifier is `npi.<n>|<tab id>`. The delegate's `didReceiveNotificationResponse` parses the tab id back, shows and focuses the main window and emits `notify://open`; `live.js` activates that tab. The delegate is installed once at startup and leaked on purpose (the centre holds it weakly).

The centre only works for a launched `.app`: an unbundled binary (`tauri dev`) has no bundle identifier and `currentNotificationCenter` would raise, so every entry checks it first and the state is `unsupported`. `notify/macos.rs` is the eighth production module with `unsafe` (the delegate's `init`, the settings reference handed to a completion block), each with a `SAFETY` comment.

## Windows and Linux

`other.rs` keeps the previous mechanism, repaired: PowerShell reads the title and body from environment variables instead of having them spliced into the script, and `notify-send` takes them after `--`. There is no per-app permission to ask for on this path (status is always `granted`) and a click on the toast is not wired. The `tauri-plugin-notification` desktop implementation was considered and not used: it answers `Granted` without asking, on macOS it goes through the deprecated `NSUserNotification` and cannot report a denial or a click.

## Not the same as phone push

Web Push to a paired phone (`remote-access.md`, `remote/push*.rs`) is a different feature with its own settings; nothing here touches it.

## Unverified

The OS prompt, the banner and the click-to-focus have only been checked by reading the API and by the unit tests of the rules; they need a packaged build. An ad-hoc signed bundle may be asked or shown differently by macOS than a notarized one: if the prompt does not appear, enable NPi Desktop in System Settings → Notifications by hand.
