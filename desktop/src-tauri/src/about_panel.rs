//! The omp core and kernel versions in the About panel.
//!
//! The panel comes from the About item of Tauri's default menu and shows the
//! app's own name and version. The versions of the omp it runs are asked of
//! that omp (`agent::spawn::about_versions`) and go into the copyright line,
//! the panel's small text under the version. The credits field is no use for
//! this: it takes text without a colour, which is black on a dark panel.

use tauri::menu::{AboutMetadata, MenuItemKind, PredefinedMenuItem};
use tauri::AppHandle;

/// Add the versions once omp has answered. The menu is up before that, so
/// asking never delays the launch; an omp that cannot answer leaves the
/// panel as it was.
pub fn setup(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        let Some(versions) = crate::agent::spawn::about_versions() else {
            return;
        };
        if let Err(e) = install(&app, &versions) {
            eprintln!("[omp-desktop] about panel versions: {e}");
        }
    });
}

/// Replace the About item with one that also carries `versions`. Tauri's
/// default menu puts the app submenu first and About first in it; any other
/// shape is left alone.
fn install(app: &AppHandle, versions: &str) -> tauri::Result<()> {
    let Some(menu) = app.menu() else {
        return Ok(());
    };
    let Some(MenuItemKind::Submenu(app_menu)) = menu.items()?.into_iter().next() else {
        return Ok(());
    };
    if !matches!(app_menu.items()?.first(), Some(MenuItemKind::Predefined(_))) {
        return Ok(());
    }
    let package = app.package_info();
    let bundle = &app.config().bundle;
    let copyright = bundle.copyright.as_ref().map_or_else(
        || versions.to_string(),
        |copyright| format!("{versions}\n{copyright}"),
    );
    let about = PredefinedMenuItem::about(
        app,
        None,
        Some(AboutMetadata {
            name: Some(package.name.clone()),
            version: Some(package.version.to_string()),
            copyright: Some(copyright),
            authors: bundle.publisher.clone().map(|publisher| vec![publisher]),
            ..Default::default()
        }),
    )?;
    app_menu.remove_at(0)?;
    app_menu.insert(&about, 0)
}
