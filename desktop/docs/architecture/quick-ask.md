# Quick ask and the tray icon

Code: `src-tauri/src/quick_ask.rs`, `src/quickask.{html,css,js}`, `capabilities/quickask.json`, `live.js` (`quickask://submit`), `app-live.jsx` (`npi:show-chat`), `icons/tray.png`.

## Option+Space

`tauri-plugin-global-shortcut` registers Option+Space at startup (`quick_ask::setup`). It opens a small always-on-top, borderless, transparent window (label `quickask`, created on the first press and then reused) in the upper third of the screen the pointer is on; pressing the shortcut again, Esc, or a click elsewhere (`WindowEvent::Focused(false)`) puts it away. If another app owns the shortcut, registration fails and is logged, nothing else changes. There is no setting to change the key yet.

The page is plain HTML (`quickask.html`, its own CSS and JS; no bundler, and `build-frontend.mjs` copies it as it is). It takes the theme, accent and language from the main window's tweaks in `localStorage`, shows the project's mark, one input, a target menu ("New Chat" / "Current chat") and a send button. It may call only two commands: `quickask_submit(text, target)` (trimmed text of 1 to 20,000 bytes; `target` is `new` or `current`) and `quickask_hide`.

`quickask_submit` hides the window, shows the main window and emits `quickask://submit` to it. `live.js` handles that: for `new` (or when no tab is open) it opens a tab in the Chat folder (`chatDir`, after `npi:show-chat` has switched the app to Chat mode) and sends the text there; for `current` it sends the text to the active tab. A prompt is sent like a typed one, so approvals and every other rule apply.

## Glass and blur

The glass look is drawn by the page (`quickask.css`): a tinted frosted fill, specular rims, an inner glow, a light that flows around the rim (an animated conic gradient masked to the border) and a slow sweep. A page cannot blur what is behind its own window, so on macOS `quick_ask_glass.rs` (the ninth `unsafe` module; the class is found by name at run time) puts an `NSVisualEffectView` (popover material, rounded through its layer, pinned to the top when the window grows) behind the transparent webview, and the page, told through `quickask.html?backdrop=vibrancy`, lowers its tint so the blur shows. The system's own Liquid Glass (`NSGlassEffectView`) is not used. Without the class (Windows, Linux) the tint stays higher so the bar is legible. The blur follows the system appearance, not the app's theme tweak.

The target menu ("New Chat / Current chat") opens below the bar: the window grows by `MENU_HEIGHT` (`quickask_resize`) and shrinks again when it closes or the window is shown.

## Tray icon and Cmd+W

A menu-bar icon (the mark, as a template image) with "Open NPi Desktop", "Quick Ask ⌥Space" and "Quit NPi Desktop"; a left click shows the main window. On macOS Cmd+W and the red button only hide the main window (`hides_on_close`); the Dock icon is dropped while it is hidden (`ActivationPolicy::Accessory`) and comes back when it is shown, so the tray icon is how the app is reached. Quitting is the tray menu or Cmd+Q (`CloseGuard` still asks first while a tab is busy).

## Not verified

None of this was run in a real app, and the Liquid Glass path could not even be exercised on the development Mac (macOS 15, which has no `NSGlassEffectView`; only the older HUD-material path and the "no backdrop" page were reachable by compiling and in a browser): the shortcut, the window placement, the tray icon, the Dock icon toggling and the hand-off to a new chat are checked only by compiling, the Rust unit tests (shortcut value, the icon is a PNG) and the page in a browser pane. Option+Space may also collide with a system or input-method shortcut on some machines.
