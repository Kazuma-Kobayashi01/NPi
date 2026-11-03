//! The quick-ask window and the menu-bar (tray) icon.
//!
//! Option+Space opens a small always-on-top window (`src/quickask.html`) with
//! one prompt field. Sending it hands the text to the main window (event
//! `quickask://submit`, handled in `live.js`), which opens a new chat or
//! continues the current tab. The quick-ask page is plain HTML: it holds no
//! secrets and calls only [`quickask_submit`] and [`quickask_hide`].
//!
//! The tray icon is how the app is reached once its window is closed with
//! Cmd+W or the red button (`lib.rs` `hides_on_close`): on macOS the Dock icon
//! goes while the window is hidden, and a click on the tray icon (or its menu)
//! brings the window back.

use serde_json::json;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

pub const QUICK_LABEL: &str = "quickask";
const WIDTH: f64 = 720.0;
const HEIGHT: f64 = 92.0;
/// The longest prompt the quick-ask window may hand over.
const MAX_PROMPT_BYTES: usize = 20_000;

fn shortcut() -> Shortcut {
    Shortcut::new(Some(Modifiers::ALT), Code::Space)
}

/// Brings the main window forward (and the Dock icon back on macOS).
pub fn show_main(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Regular);
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// The main window was closed to the background: only the tray icon is left (macOS).
pub fn main_hidden(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
    #[cfg(not(target_os = "macos"))]
    let _ = app;
}

/// The page, told which native backdrop (if any) is behind it: `?backdrop=glass` or `vibrancy`.
fn page() -> &'static str {
    #[cfg(target_os = "macos")]
    match crate::quick_ask_glass::available() {
        crate::quick_ask_glass::Backdrop::Glass => return "quickask.html?backdrop=glass",
        crate::quick_ask_glass::Backdrop::Vibrancy => return "quickask.html?backdrop=vibrancy",
        crate::quick_ask_glass::Backdrop::None => {}
    }
    "quickask.html"
}

fn build_window(app: &AppHandle) -> Result<tauri::WebviewWindow, String> {
    let window = WebviewWindowBuilder::new(app, QUICK_LABEL, WebviewUrl::App(page().into()))
        .title("Quick Ask")
        .inner_size(WIDTH, HEIGHT)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .visible(false)
        .focused(false)
        .build()
        .map_err(|e| e.to_string())?;
    #[cfg(target_os = "macos")]
    {
        // The bar is inset from the window edge (the page's body padding) and has 22 pt corners.
        let target = window.clone();
        let _ = window.run_on_main_thread(move || {
            if let Ok(ns_window) = target.ns_window() {
                crate::quick_ask_glass::apply(ns_window, WIDTH, HEIGHT, 14.0, 22.0);
            }
        });
    }
    Ok(window)
}

/// Puts the window in the upper third of the screen the pointer is on.
fn place(window: &tauri::WebviewWindow) {
    let monitor = window
        .app_handle()
        .cursor_position()
        .ok()
        .and_then(|p| window.app_handle().monitor_from_point(p.x, p.y).ok().flatten())
        .or_else(|| window.current_monitor().ok().flatten());
    let Some(monitor) = monitor else { return };
    let scale = monitor.scale_factor();
    let (size, origin) = (monitor.size(), monitor.position());
    let x = f64::from(origin.x) + (f64::from(size.width) - WIDTH * scale) / 2.0;
    let y = f64::from(origin.y) + f64::from(size.height) * 0.22;
    let _ = window.set_position(tauri::PhysicalPosition::new(x.round() as i32, y.round() as i32));
}

/// Option+Space: show the quick-ask window, or hide it when it is already up.
pub fn toggle(app: &AppHandle) {
    let window = match app.get_webview_window(QUICK_LABEL) {
        Some(w) => w,
        None => match build_window(app) {
            Ok(w) => w,
            Err(e) => return eprintln!("quick ask window: {e}"),
        },
    };
    if window.is_visible().unwrap_or(false) && window.is_focused().unwrap_or(false) {
        let _ = window.hide();
        return;
    }
    place(&window);
    let _ = window.show();
    let _ = window.set_focus();
    let _ = window.emit("quickask://focus", ());
}

/// A click outside the quick-ask window puts it away.
pub fn on_blur(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(QUICK_LABEL) {
        let _ = window.hide();
    }
}

#[tauri::command]
pub fn quickask_hide(app: AppHandle) {
    on_blur(&app);
}

/// Sends a prompt from the quick-ask window to the main window: `target` is
/// `new` (a new chat) or `current` (the tab on screen).
#[tauri::command]
pub fn quickask_submit(app: AppHandle, text: String, target: String) -> Result<(), String> {
    let text = text.trim();
    if text.is_empty() || text.len() > MAX_PROMPT_BYTES {
        return Err("the prompt is empty or too long".to_string());
    }
    if !["new", "current"].contains(&target.as_str()) {
        return Err("unknown target".to_string());
    }
    on_blur(&app);
    show_main(&app);
    app.emit_to("main", "quickask://submit", json!({ "text": text, "target": target }))
        .map_err(|e| e.to_string())
}

/// Registers Option+Space and builds the tray icon. Neither failing stops the app.
pub fn setup(app: &AppHandle) {
    let plugin = tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, pressed, event| {
            if event.state() == ShortcutState::Pressed && *pressed == shortcut() {
                toggle(app);
            }
        })
        .build();
    if let Err(e) = app.plugin(plugin) {
        eprintln!("quick ask shortcut plugin: {e}");
    } else if let Err(e) = app.global_shortcut().register(shortcut()) {
        eprintln!("quick ask shortcut (Option+Space) is not available: {e}");
    }
    if let Err(e) = build_tray(app) {
        eprintln!("tray icon: {e}");
    }
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open NPi Desktop", true, None::<&str>)?;
    let ask = MenuItem::with_id(app, "ask", "Quick Ask      ⌥Space", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit NPi Desktop", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &ask, &PredefinedMenuItem::separator(app)?, &quit])?;
    let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/tray.png"))?;
    TrayIconBuilder::with_id("main")
        .icon(icon)
        .icon_as_template(true)
        .tooltip("NPi Desktop")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main(app),
            "ask" => toggle(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shortcut_is_option_space() {
        assert_eq!(shortcut().to_string().to_lowercase(), "alt+space");
    }

    #[test]
    fn the_tray_icon_is_a_png() {
        let bytes = include_bytes!("../icons/tray.png");
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    }
}
