# Quick ask and the tray icon

Code: `src-tauri/src/quick_ask.rs`, `quick_ask_glass.rs`, `src/design/quick/quick-window.jsx`, `design/layout/quick.css`, `live.js` (`_QUICK_PANEL`, `quickNewSession`, `quickContinue`, `quickask://continue`), `app-live.jsx` (`npi:show-mode`), `icons/tray.png`.

## Option+Space

`tauri-plugin-global-shortcut` registers Option+Space at startup (`quick_ask::setup`), which also builds the window, hidden, so the first press does not wait for a page load. The window (label `panel-quick`, so it has the main window's capabilities and navigation rule) goes on the screen the pointer is on, centred both ways on its usable area, and stays centred as it grows and shrinks, on the screen it was opened on. The shortcut again, Esc, or a click elsewhere (`WindowEvent::Focused(false)`) hides it.

On macOS the window's class is changed to a runtime subclass of `NSPanel` with `NSWindowStyleMaskNonactivatingPanel` (`quick_ask_glass::make_panel`): it becomes key and takes the keyboard while the app the user was in stays active. `set_focus` must not be used on it, because that activates the app; `focus_view` makes it key and gives the webview first responder.

## The conversation

The page is `index.html?panel=quick`. Unlike other panels, `live.js` runs here (`_QUICK_PANEL`): it owns one tab of its own, and stays out of what belongs to the main window (`open-tabs.json`, the recent projects, the close guard, `close_all_tabs`). A panel showing the main window's tab was not an option: `live.js` streams only the active tab, so that would switch the tab under the user.

- A process starts the first time the panel is shown, in Chat, and provides the model list. Hiding the panel keeps the conversation and any turn in progress.
- `start_session` takes `appMode` here, so the panel's mode does not touch the app's. The folder is the mode's space (`quickask_folder`): `NPi/Chat`, `NPi/Code`, `NPi/Academic`.
- ⌘1–3 or the capsule picks the mode. The conversation on screen stays until the next message, which opens a new one.
- It is an ordinary session, saved under the mode's folder. **Continue in window** closes the panel's process first, so the file has one writer, then `quickask_continue` brings the app forward and emits `quickask://continue`, on which the main window switches mode and resumes the file. It waits for the first reply, since omp writes the file then, and for a turn to end.
- A message cannot be sent during a turn; the text stays in the input.

## Window size and glass

The window is exactly as tall as `.quick`: a `ResizeObserver` calls `quickask_resize`, clamped to 80–700 pt, which also recentres it. Messages stop at 380 pt and scroll in `ChatView`, which sticks to the bottom. The transparent margin (`MARGIN` in Rust, `--quick-margin` in CSS) holds the glow. The `NSVisualEffectView` from `quick_ask_glass::apply` sizes with the window, and the page draws the rest.

## Tray icon and Cmd+W

A menu-bar icon (the mark, as a template image) with "Open NPi Desktop", "Quick Ask ⌥Space" and "Quit NPi Desktop"; a left click shows the main window. On macOS Cmd+W and the red button only hide the main window (`hides_on_close`); the Dock icon is dropped while it is hidden (`ActivationPolicy::Accessory`) and comes back when it is shown, so the tray icon is how the app is reached. Quitting is the tray menu or Cmd+Q (`CloseGuard` still asks first while a tab is busy).

## Not verified

The panel's page was run in a browser with a fake bridge: input growth, send, stop, new conversation, mode change and continue; a reply from a real session was not rendered. Nothing native has run: the non-activating panel, keyboard focus, placement, blur-to-hide, a real session with `appMode`, and the hand-off to the main window are checked only by compiling. Liquid Glass is unavailable on the development Mac (macOS 15).
