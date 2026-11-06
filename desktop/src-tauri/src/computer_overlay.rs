//! Real-operation indicators. The RPC reader schedules work here independently
//! of whichever tab is selected; native input waits for the painted ACK.

mod state;
mod windows;

use std::sync::Mutex;

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::agent::AgentBridge;
use state::{Activity, Machine};

pub struct OverlayState(Mutex<Machine>);

impl Default for OverlayState {
    fn default() -> Self {
        Self(Mutex::new(Machine::with_accent("#4f8cff")))
    }
}


fn report(app: &AppHandle, sid: &str, error: &str) {
    eprintln!("Computer Use indicator ({sid}): {error}");
    if let Err(emit_error) = app.emit_to("main", "computer-use://error", json!({ "sessionId": sid, "error": error })) {
        eprintln!("Could not report Computer Use indicator error: {emit_error}");
    }
}

fn fail(app: &AppHandle, sid: &str, error: &str) {
    report(app, sid, error);
    if let Err(hide_error) = windows::hide(app) { report(app, sid, &hide_error); }
}
/// Bounded breadcrumb trail in the app cache: a failed indicator is otherwise
/// invisible, since nothing of it reaches the transcript.
fn trace(line: &str) {
    let Ok(dir) = crate::npi_dirs::cache_dir() else { return };
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("computer-use-overlay.log");
    if std::fs::metadata(&path).is_ok_and(|meta| meta.len() > 256 * 1024) {
        let _ = std::fs::remove_file(&path);
    }
    use std::io::Write;
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(file, "{:?} {line}", std::time::SystemTime::now());
    }
}

fn render(app: &AppHandle) {
    let display = {
        let state = app.state::<OverlayState>();
        let machine = state.0.lock().expect("Computer Use overlay state poisoned");
        machine.display()
    };
    let Some(display) = display else {
        if let Err(error) = windows::hide(app) { report(app, "", &error); }
        return;
    };
    if let Err(error) = windows::render(app, &display) {
        fail(app, &display.session_id, &error);
    }
}

/// The exit button ends the takeover: stop only computer work, then drop the share.
pub fn stop_from_share(app: &AppHandle) {
    if let Some(session) = windows::current_session() {
        if let Err(error) = app.state::<AgentBridge>().send(&session, r#"{"type":"stop_computer_use"}"#) {
            report(app, &session, &error);
        }
        clear_session(app, &session);
    } else {
        let _ = windows::hide(app);
    }
}

fn dispatch(app: &AppHandle, action: impl FnOnce(&AppHandle) + Send + 'static) {
    let handle = app.clone();
    if let Err(error) = app.run_on_main_thread(move || action(&handle)) {
        report(app, "", &format!("Could not schedule Computer Use indicator: {error}"));
    }
}

pub fn setup(app: &AppHandle) -> Result<(), String> {
    trace("setup native indicator");
    windows::remember_exit_hook(app);
    // The share outlives individual operations; a slow tick retires it once the
    // hold window lapses, so the border never stays on screen forever.
    let handle = app.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_millis(700));
        let expired = handle.try_state::<OverlayState>().is_some_and(|state| {
            state
                .0
                .lock()
                .map(|machine| machine.expired_without_activity())
                .unwrap_or(false)
        });
        if expired {
            dispatch(&handle, |app| {
                if let Err(error) = windows::hide(app) {
                    report(app, "", &error);
                }
            });
        }
    });
    Ok(())
}

/// Call only on live (not replayed) stdout frames. No pixels enter the overlay.
pub fn observe(app: &AppHandle, session_id: &str, frame: &Value) {
    if frame.get("type").and_then(Value::as_str) != Some("computer_activity") { return; }
    let sid = session_id.to_owned();
    let activity: Activity = match serde_json::from_value(frame["activity"].clone()) {
        Ok(activity) => activity,
        Err(_) => return,
    };
    dispatch(app, move |app| {
        {
            let state = app.state::<OverlayState>();
            let mut machine = state.0.lock().expect("Computer Use overlay state poisoned");
            machine.observe(&sid, activity);
        }
        render(app);
    });
}
pub fn clear_session(app: &AppHandle, session_id: &str) {
    let sid = session_id.to_owned();
    dispatch(app, move |app| {
        {
            let state = app.state::<OverlayState>();
            let mut machine = state.0.lock().expect("Computer Use overlay state poisoned");
            machine.clear(&sid);
        }
        render(app);
    });
}

pub fn shutdown(app: &AppHandle) {
    dispatch(app, |app| {
        *app.state::<OverlayState>().0.lock().expect("Computer Use overlay state poisoned") = Machine::default();
        let _ = windows::hide(app);
    });
}
pub fn set_accent(app: &AppHandle, accent: &str) -> Result<(), String> {
    if accent.len() != 7 || !accent.starts_with('#') || !accent.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit) {
        return Err("Computer Use accent must be a #RRGGBB color".into());
    }
    let accent = accent.to_owned();
    dispatch(app, move |app| {
        app.state::<OverlayState>().0.lock().expect("Computer Use overlay state poisoned").accent = accent;
        render(app);
    });
    Ok(())
}

#[tauri::command]
pub fn computer_use_set_accent(app: AppHandle, accent: String) -> Result<(), String> { set_accent(&app, &accent) }

#[tauri::command]
pub fn computer_use_stop(app: AppHandle, bridge: State<'_, AgentBridge>, session_id: String) -> Result<(), String> {
    bridge.send(&session_id, r#"{"type":"stop_computer_use"}"#)?;
    clear_session(&app, &session_id);
    Ok(())
}
