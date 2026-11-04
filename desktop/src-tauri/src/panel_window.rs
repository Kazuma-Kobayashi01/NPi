//! Secondary windows: Librarian and usage statistics open as their own OS
//! windows (resizable, zoomable, closed with Cmd+W) instead of floating inside
//! the main window.
//!
//! A panel window loads `index.html?panel=<kind>&…` (`src/app/panel-mode.js`):
//! the page then shows that one panel and keeps `live.js` inert, so a panel
//! never touches the tabs the main window owns. Only the kinds in [`KINDS`]
//! open, and the navigation guard lets only those windows load that URL.

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

/// The panels that run without the main window's state: (kind, width, height).
const KINDS: [(&str, f64, f64); 3] = [("librarian", 1120.0, 740.0), ("reader", 1180.0, 780.0), ("stats", 760.0, 660.0)];
pub const LABEL_PREFIX: &str = "panel-";

/// Percent-encodes `s` for a query value.
fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn hex(s: &str) -> String {
    s.bytes().map(|b| format!("{b:02x}")).collect()
}

/// The window label of a panel: one window per kind and project.
fn label_for(kind: &str, project: &str) -> String {
    if project.is_empty() {
        format!("{LABEL_PREFIX}{kind}")
    } else {
        format!("{LABEL_PREFIX}{kind}-{}", hex(project))
    }
}

/// Opens the panel window of `kind`, or brings the one already open forward.
#[tauri::command]
pub fn open_panel_window(
    app: AppHandle,
    kind: String,
    project: Option<String>,
    profile: Option<String>,
    title: Option<String>,
    anchor: Option<i64>,
) -> Result<(), String> {
    let &(_, width, height) = KINDS
        .iter()
        .find(|(k, ..)| *k == kind)
        .ok_or_else(|| format!("unknown panel: {kind}"))?;
    let project = project.unwrap_or_default();
    let profile = profile.unwrap_or_default();
    if project.len() > 200 || profile.len() > 200 {
        return Err("panel arguments are too long".to_string());
    }
    let label = label_for(&kind, &project);
    if let Some(window) = app.get_webview_window(&label) {
        if anchor.is_some() {
            // A citation's evidence: the open reader goes to it.
            let _ = tauri::Emitter::emit_to(&app, label.as_str(), "reader://goto", anchor);
        }
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        return Ok(());
    }
    let mut url = format!(
        "index.html?panel={kind}&project={}&profile={}",
        encode(&project),
        encode(&profile)
    );
    if let Some(anchor) = anchor.filter(|a| *a > 0) {
        url.push_str(&format!("&anchor={anchor}"));
    }
    let title = title.unwrap_or_else(|| match project.is_empty() {
        true => kind.clone(),
        false => format!("{kind} · {project}"),
    });
    let builder = WebviewWindowBuilder::new(&app, &label, WebviewUrl::App(url.into()))
        .title(title)
        .inner_size(width, height)
        .min_inner_size(520.0, 360.0)
        .resizable(true)
        .maximizable(true)
        .center();
    #[cfg(target_os = "macos")]
    let builder = builder
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true)
        .traffic_light_position(tauri::LogicalPosition::new(18.0, 20.5));
    builder.build().map(|_| ()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_values_are_percent_encoded() {
        assert_eq!(encode("a b&c=中"), "a%20b%26c%3D%E4%B8%AD");
        assert_eq!(encode("Proj-1_x.y"), "Proj-1_x.y");
    }

    #[test]
    fn a_label_has_only_label_characters() {
        let l = label_for("librarian", "论文 A");
        assert!(l.starts_with("panel-librarian-"));
        assert!(l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'));
        assert_eq!(label_for("stats", ""), "panel-stats");
    }
}
