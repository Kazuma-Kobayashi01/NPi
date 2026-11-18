# System notifications

Code: `src-tauri/src/notify/` (`mod.rs` state, frame classification and commands; `policy.rs` pure rules and banner text; `summary.rs` the banner's one line; `macos.rs`; `other.rs`), `agent/reader.rs` (the hooks), `app/notify.js` (`OMP_NOTIFY`), `design/settings/rows-notifications.jsx`, `live.js` (`notify*` bridge methods, `notify://open`). Tests: `cargo test --lib notify`, `test-notify.mjs`.

## What was wrong

The old `system_notice.rs` ran `osascript -e 'display notification'` (macOS), `notify-send` or a PowerShell toast built by string formatting. On macOS that banner belongs to Script Editor, not to NPi Desktop: the app never had a permission of its own to ask for or to be refused, a failure was swallowed, and a click did nothing. The webview fired it only for the *active* tab (a background tab's frames are not processed there), only when `document.hasFocus()` was false, with no setting to turn it off, and the "turn finished" banner carried the first 80 characters of the reply, markup and all. The PowerShell text was spliced into the script, so a quote in it broke (or injected into) the command.

## Policy lives in Rust

Only the stdout reader sees every tab's frames, so it hands each parsed frame to `NotifyState::observe` (one string compare for most frames). Three events become notifications:

|Kind|Frame|Rule|
|---|---|---|
|Done|a terminal `agent_end` (`yielded`, else `isTerminal !== false`: omp's `isYield`)|the run, timed from `agent_start`, lasted 8 s or more|
|NeedsInput|`extension_ui_request` with method `select`, `ask` or `confirm` that no stored approval rule answered (the reader only gets here for prompts the human will see)|always|
|Failed|a terminal `agent_end` whose last assistant message has `stopReason: "error"`, or an `agent://exit` with a reason (the process died)|always|

`policy::should_send` then applies: the master switch and the kind's own switch; no banner while the window has focus **and** the event's tab is the one on screen (a background tab of a focused window still notifies); one banner per tab and kind per 20 s. The Settings test button ignores all of it.

The webview sends only the switches and the language (`notify_configure`, from `OMP_NOTIFY.configFrom`: four booleans and one of `zh-CN`, `zh-TW`, `en-US`, `ja-JP`; anything else keeps the old language) and which tab is on screen (`notify_set_active`: `null` on the start page and the Settings page). A banner's title is the project folder name (cut to 60 characters) or "NPi Desktop".

## The banner's line

The body is a short lead per kind and language ("已完成：", "等你处理：", "出错：") followed by one line taken from the frame that raised it, so the banner says what happened rather than only that something did:

|Kind|Line from|
|---|---|
|Done|the run's last assistant message (`agent_end` `messages`): its text blocks; thinking and tool calls are ignored|
|NeedsInput|an ask dialog's first `question`, else the prompt's `title` (a tool approval: "Allow tool: …"), else its `message`|
|Failed|that message's `errorMessage`, or the process exit reason|

`summary::line` turns the text into the line: the first line that is prose (headings, code fences, tables, rules and HTML are passed over), its quote/list/task marker and inline markup removed (links and images become their text), cut after the first sentence (`。！？`, or `.!?` before a space) and to 90 characters with "…". Replies are written conclusion first, so this is the reply's own summary; nothing asks a model for one. With no such line the body is the kind's generic sentence ("任务已完成。"). The text appears on the lock screen as macOS shows previews (System Settings → Notifications → Show previews).

## macOS

`macos.rs` talks to `UNUserNotificationCenter` through `objc2-user-notifications`: `requestAuthorizationWithOptions` (alert and sound; the system shows its prompt once, the first time), `getNotificationSettings` for the state, `addNotificationRequest` to post. The Settings row asks when the master switch goes on, shows `granted`, `denied` (with "enable in System Settings → Notifications") or `notDetermined`, rechecks when the window regains focus, and has a **Send test notification** button (shown even while the app is in front, through `willPresentNotification`).

A request identifier is `npi.<n>|<tab id>`. The delegate's `didReceiveNotificationResponse` parses the tab id back, shows and focuses the main window and emits `notify://open`; `live.js` activates that tab. The delegate is installed once at startup and leaked on purpose (the centre holds it weakly).

The centre only works for a launched `.app`: an unbundled binary (`tauri dev`) has no bundle identifier and `currentNotificationCenter` would raise, so every entry checks it first and the state is `unsupported`. `notify/macos.rs` is the eighth production module with `unsafe` (the delegate's `init`, the settings reference handed to a completion block), each with a `SAFETY` comment.

## Windows and Linux

`other.rs` keeps the previous mechanism, repaired: PowerShell reads the title and body from environment variables instead of having them spliced into the script, and `notify-send` takes them after `--`. There is no per-app permission to ask for on this path (status is always `granted`) and a click on the toast is not wired. The `tauri-plugin-notification` desktop implementation was considered and not used: it answers `Granted` without asking, on macOS it goes through the deprecated `NSUserNotification` and cannot report a denial or a click.

## Why a macOS build may never be allowed (ad-hoc signing)

Checked on the developer's machine: `~/Library/Preferences/com.apple.ncprefs.plist` listed `dev.npi.desktop` with **eight** separate sources, one per build, each with a different code requirement and `auth: 0`. The shipped bundle is signed ad hoc (`signingIdentity: "-"`, `codesign` says `Signature=adhoc`), and an ad-hoc signature's identity is the build's own hash, so macOS sees every build as a new, never-asked app: the permission does not carry over and the prompt may not return. The Settings row therefore shows what the system said when it refused (`notify_diagnose`: the last `NSError` of the authorization request or of a post) and, for an ad-hoc build that is not allowed, a note pointing here.

The fix is a stable signing identity, not code. `scripts/macos-local-signing.sh` creates a self-signed code-signing certificate ("NPi Desktop Local") in the login keychain once (it asks macOS to trust it for code signing, which needs your password, so it is run by hand), and `npm run build -- --config src-tauri/tauri.localsign.conf.json` signs with it, so the code identity stays the same across builds and the permission is kept. Remove the stale "NPi Desktop" entries from System Settings → Notifications first if the old ones are still listed. Not verified: the script and the signed build have not been run, and whether macOS keeps the permission for a self-signed identity is the expected behaviour, not a measured one.

## Not the same as phone push

Web Push to a paired phone (`remote-access.md`, `remote/push*.rs`) is a different feature with its own settings; nothing here touches it.

## Unverified

The OS prompt, the banner and the click-to-focus have only been checked by reading the API and by the unit tests of the rules; they need a packaged build. An ad-hoc signed bundle may be asked or shown differently by macOS than a notarized one: if the prompt does not appear, enable NPi Desktop in System Settings → Notifications by hand.

## Signing a local build

`tauri build` under `devstore run` cannot see the login keychain (its `HOME` is the SDK volume), so a build with `tauri.localsign.conf.json` fails with "The specified item could not be found in the keychain". Build as usual (ad-hoc), then sign outside `devstore run`: `codesign --force --deep --options runtime --sign "NPi Desktop Local" "<the .app>"`. The designated requirement becomes `identifier "dev.npi.desktop" and certificate leaf = H"…"`, the same for every build, so the notification permission is kept. The script must use `/usr/bin/openssl`: a conda or Homebrew openssl 3 writes a PKCS#12 that `security import` rejects.
