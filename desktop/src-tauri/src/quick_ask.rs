//! The quick-ask window and the menu-bar (tray) icon.
//!
//! Option+Space shows a floating panel that answers without bringing the app
//! forward, like Spotlight: on macOS it is a non-activating panel
//! (`quick_ask_glass.rs`), so whatever the user was in stays the active app. The
//! page is `index.html?panel=quick` (`design/quick/quick-window.jsx`), which runs
//! `live.js` for one ordinary conversation of its own. That conversation is saved
//! like any other, and [`quickask_continue`] hands it to the main window.
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

/// A `panel-…` label: it takes the main window's capabilities and navigation rule (`panel_window.rs`).
pub const QUICK_LABEL: &str = "panel-quick";
const WIDTH: f64 = 760.0;
/// The height before the page reports its own.
const HEIGHT: f64 = 140.0;
const MIN_HEIGHT: f64 = 80.0;
/// Input, 380 pt of messages, footer and capsule, with the page's margins.
const MAX_HEIGHT: f64 = 700.0;

/// A piece of glass the page draws on: a rectangle of the window, in points from its top left,
/// with its corner radius (`quick-window.jsx` `glassShapes`).
#[derive(Clone, Copy, Debug, serde::Deserialize)]
pub struct Shape {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub radius: f64,
}

/// The page, told which backdrop the window gives it (`quick.css` `[data-backdrop]`).
fn page() -> String {
    #[cfg(target_os = "macos")]
    let backdrop = crate::quick_ask_glass::available().as_str();
    #[cfg(not(target_os = "macos"))]
    let backdrop = "none";
    format!("index.html?panel=quick&backdrop={backdrop}")
}

/// The screen the window was last opened on.
static SCREEN: std::sync::Mutex<Option<Area>> = std::sync::Mutex::new(None);

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
        let target = window.clone();
        let _ = window.run_on_main_thread(move || {
            if let Ok(ns_window) = target.ns_window() {
                crate::quick_ask_glass::make_panel(ns_window);
                crate::quick_ask_glass::install(ns_window);
            }
        });
    }
    Ok(window)
}

/// The usable part of a screen, in physical pixels.
#[derive(Clone, Copy)]
struct Area {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    scale: f64,
}

/// `(x, y)` of a window `width` by `height` points, centred both ways on `area`. One taller than
/// the area stays on it, top first.
fn position(area: Area, width: f64, height: f64) -> (i32, i32) {
    let x = f64::from(area.x) + (f64::from(area.width) - width * area.scale) / 2.0;
    let y = f64::from(area.y) + ((f64::from(area.height) - height * area.scale) / 2.0).max(0.0);
    (x.round() as i32, y.round() as i32)
}

/// The usable area of the screen the pointer is on, past the menu bar and the Dock.
fn pointer_area(window: &tauri::WebviewWindow) -> Option<Area> {
    let app = window.app_handle();
    let monitor = app
        .cursor_position()
        .ok()
        .and_then(|p| app.monitor_from_point(p.x, p.y).ok().flatten())
        .or_else(|| window.current_monitor().ok().flatten())?;
    let work = monitor.work_area();
    Some(Area {
        x: work.position.x,
        y: work.position.y,
        width: work.size.width,
        height: work.size.height,
        scale: monitor.scale_factor(),
    })
}

/// Puts the window on the screen the pointer is on.
fn place(window: &tauri::WebviewWindow) {
    let Some(area) = pointer_area(window) else {
        return;
    };
    let height = window
        .inner_size()
        .map_or(HEIGHT, |size| f64::from(size.height) / area.scale);
    let (x, y) = position(area, WIDTH, height);
    let _ = window.set_position(tauri::PhysicalPosition::new(x, y));
    SCREEN
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .replace(area);
}

/// Makes the window the key window, without activating the app, with the keyboard on its page.
fn show(window: &tauri::WebviewWindow) {
    let _ = window.show();
    #[cfg(target_os = "macos")]
    {
        let _ = window.with_webview(|webview| {
            crate::quick_ask_glass::focus_view(webview.ns_window(), webview.inner());
        });
    }
    #[cfg(not(target_os = "macos"))]
    let _ = window.set_focus();
    let _ = window.emit("quickask://focus", ());
}

/// Option+Space: show the quick-ask window, or hide it when it is up.
pub fn toggle(app: &AppHandle) {
    let window = match app.get_webview_window(QUICK_LABEL) {
        Some(w) => w,
        None => match build_window(app) {
            Ok(w) => w,
            Err(e) => return eprintln!("quick ask window: {e}"),
        },
    };
    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
        return;
    }
    place(&window);
    show(&window);
}

/// A click outside the quick-ask window puts it away.
pub fn on_blur(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(QUICK_LABEL) {
        let _ = window.hide();
    }
}

/// The page's height or its glass changed: the window follows, and the glass behind the page is
/// laid out again once the window has its new size.
#[tauri::command]
pub fn quickask_resize(app: AppHandle, height: f64, shapes: Vec<Shape>) {
    let Some(window) = app.get_webview_window(QUICK_LABEL) else {
        return;
    };
    let _ = window.set_size(tauri::LogicalSize::new(
        WIDTH,
        height.clamp(MIN_HEIGHT, MAX_HEIGHT),
    ));
    #[cfg(target_os = "macos")]
    let _ = window.run_on_main_thread(move || crate::quick_ask_glass::layout(&shapes));
    #[cfg(not(target_os = "macos"))]
    let _ = shapes;
}

#[tauri::command]
pub fn quickask_hide(app: AppHandle) {
    on_blur(&app);
}

/// The folder a quick conversation of `mode` runs in: the mode's own space.
#[tauri::command]
pub fn quickask_folder(app: AppHandle, mode: String) -> Result<String, String> {
    let dir = match crate::app_mode::AppMode::parse(&mode).ok_or("unknown mode")? {
        crate::app_mode::AppMode::Chat => app
            .state::<crate::chat_space::ChatSpace>()
            .ensure_dir()?
            .to_path_buf(),
        crate::app_mode::AppMode::Code => {
            crate::workspace_dir(&app, "Code").ok_or("no Code folder")?
        }
        crate::app_mode::AppMode::Academic => {
            crate::workspace_dir(&app, "Academic").ok_or("no Academic folder")?
        }
    };
    dir.to_str()
        .map(str::to_string)
        .ok_or_else(|| "not a UTF-8 path".to_string())
}

/// Opens the panel's conversation in the main window, which brings the app forward, and puts the
/// panel away. The caller has closed its own process on the file first.
#[tauri::command]
pub fn quickask_continue(
    app: AppHandle,
    session_file: String,
    profile: Option<String>,
    cwd: String,
    mode: String,
) -> Result<(), String> {
    if crate::app_mode::AppMode::parse(&mode).is_none() {
        return Err("unknown mode".to_string());
    }
    on_blur(&app);
    show_main(&app);
    app.emit_to(
        "main",
        "quickask://continue",
        json!({ "sessionFile": session_file, "profile": profile, "cwd": cwd, "mode": mode }),
    )
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
    // Built ahead: the first Option+Space must not wait for the page to load.
    if let Err(e) = build_window(app) {
        eprintln!("quick ask window: {e}");
    }
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open NPi Desktop", true, None::<&str>)?;
    let ask = MenuItem::with_id(app, "ask", "Quick Ask      ⌥Space", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit NPi Desktop", true, None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[&open, &ask, &PredefinedMenuItem::separator(app)?, &quit],
    )?;
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
    fn the_window_is_centred_both_ways() {
        let area = Area {
            x: 0,
            y: 0,
            width: 2880,
            height: 1800,
            scale: 2.0,
        };
        assert_eq!(position(area, 760.0, 140.0), (680, 760));
        let left = Area {
            x: -1920,
            y: 100,
            width: 1920,
            height: 1000,
            scale: 1.0,
        };
        assert_eq!(position(left, 760.0, 140.0), (-1340, 530));
    }

    #[test]
    fn a_window_stays_centred_as_it_grows_and_never_leaves_the_top() {
        let area = Area {
            x: 0,
            y: 0,
            width: 1440,
            height: 900,
            scale: 1.0,
        };
        assert_eq!(position(area, 760.0, 600.0).1, 150);
        assert_eq!(position(area, 760.0, 2000.0).1, 0);
    }

    #[test]
    fn the_tray_icon_is_a_png() {
        let bytes = include_bytes!("../icons/tray.png");
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    }
}
