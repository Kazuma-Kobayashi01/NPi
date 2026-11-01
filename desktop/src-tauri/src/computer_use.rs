//! Profile-owned opt-in and host-owned macOS privacy permissions.
//! Queries never request permission; only the explicit request command prompts.

use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex, PoisonError};
use tauri::{AppHandle, State};

#[cfg(target_os = "macos")]
#[path = "computer_use/macos.rs"]
mod macos;

const SETTING_KEY: &str = "computer.enabled";
static CONFIG_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Permissions {
    pub screen_recording: bool,
    pub accessibility: bool,
    pub input_monitoring: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputerUseConfig {
    supported: bool,
    enabled: bool,
    permissions: Permissions,
    responsible_app: String,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Permission {
    ScreenRecording,
    Accessibility,
    InputMonitoring,
}

impl Permission {
    #[cfg(target_os = "macos")]
    fn settings_url(self) -> &'static str {
        match self {
            Self::ScreenRecording => "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture",
            Self::Accessibility => "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
            Self::InputMonitoring => "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent",
        }
    }

    /// The `tccutil reset` service name.
    #[cfg(target_os = "macos")]
    fn tcc_service(self) -> &'static str {
        match self {
            Self::ScreenRecording => "ScreenCapture",
            Self::Accessibility => "Accessibility",
            Self::InputMonitoring => "ListenEvent",
        }
    }
}

fn config_args(profile: Option<&str>, enabled: Option<bool>) -> Vec<String> {
    let mut args = Vec::with_capacity(5);
    args.extend(crate::agent::spawn::profile_flag(profile));
    args.extend(["config", if enabled.is_some() { "set" } else { "get" }, SETTING_KEY].map(String::from));
    args.push(enabled.map_or_else(|| "--json".to_owned(), |value| value.to_string()));
    args
}

fn parse_enabled(stdout: &[u8]) -> Result<bool, String> {
    #[derive(Deserialize)]
    struct ConfigValue {
        value: bool,
    }
    serde_json::from_slice::<ConfigValue>(stdout)
        .map(|config| config.value)
        .map_err(|error| format!("unexpected {SETTING_KEY} from omp: {error}"))
}

fn config(profile: Option<&str>, enabled: Option<bool>) -> Result<bool, String> {
    let _guard = CONFIG_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(value) = enabled {
        crate::omp_cli::run(&config_args(profile, Some(value)), "omp config")?;
    }
    parse_enabled(&crate::omp_cli::run(&config_args(profile, None), "omp config")?)
}

fn responsible_app(app: &AppHandle) -> Result<String, String> {
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    if executable.ancestors().any(|path| path.extension().is_some_and(|ext| ext == "app")) {
        Ok(app.package_info().name.clone())
    } else {
        Ok(format!(
            "{} (development host: {}; macOS may attribute permission to the launching terminal or IDE)",
            app.package_info().name,
            executable.display()
        ))
    }
}

/// Run native privacy APIs on the Cocoa runloop; waiting happens off the UI thread.
#[cfg(target_os = "macos")]
async fn permissions(app: &AppHandle, request: Option<Permission>) -> Result<Permissions, String> {
    let (send, receive) = std::sync::mpsc::sync_channel(1);
    app.run_on_main_thread(move || {
        let result = macos::permissions(request);
        // A closed receiver means the invoking webview/command has gone away.
        let _ = send.send(result);
    }).map_err(|error| error.to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        receive.recv().map_err(|error| format!("permission query interrupted: {error}"))?
    }).await.map_err(|error| format!("join error: {error}"))?
}

#[cfg(not(target_os = "macos"))]
async fn permissions(_app: &AppHandle, request: Option<Permission>) -> Result<Permissions, String> {
    if request.is_some() {
        return Err("Computer Use permissions are supported only on macOS".to_owned());
    }
    Ok(Permissions { screen_recording: false, accessibility: false, input_monitoring: false })
}

async fn snapshot(app: AppHandle, profile: Option<String>, enabled: Option<bool>) -> Result<ComputerUseConfig, String> {
    let responsible_app = responsible_app(&app)?;
    let enabled = tauri::async_runtime::spawn_blocking(move || config(profile.as_deref(), enabled))
        .await.map_err(|error| format!("join error: {error}"))??;
    Ok(ComputerUseConfig {
        supported: cfg!(target_os = "macos"),
        enabled,
        permissions: permissions(&app, None).await?,
        responsible_app,
    })
}

#[tauri::command]
pub async fn computer_use_get(
    app: AppHandle,
    profile: Option<String>,
    store: State<'_, Arc<crate::profiles::ProfileStore>>,
) -> Result<ComputerUseConfig, String> {
    snapshot(app, store.resolve_owned(profile)?, None).await
}

#[tauri::command]
pub async fn computer_use_set(
    app: AppHandle,
    profile: Option<String>,
    enabled: bool,
    store: State<'_, Arc<crate::profiles::ProfileStore>>,
) -> Result<ComputerUseConfig, String> {
    let profile = store.resolve_owned(profile)?;
    if enabled && !cfg!(target_os = "macos") {
        return Err("Computer Use is supported only on macOS".to_owned());
    }
    snapshot(app, profile, Some(enabled)).await
}

#[tauri::command]
pub async fn computer_use_request_permission(app: AppHandle, permission: Permission) -> Result<Permissions, String> {
    permissions(&app, Some(permission)).await
}

/// Clears this app's TCC record for `permission` and reports the grants left.
///
/// An ad-hoc signed build is identified by its CDHash, so every rebuild makes
/// macOS treat the new binary as a different application while System Settings
/// still lists the old grant as enabled. Resetting is what lets the app ask
/// again instead of silently staying denied.
#[tauri::command]
pub async fn computer_use_reset_permission(
    app: AppHandle,
    permission: Permission,
) -> Result<Permissions, String> {
    #[cfg(target_os = "macos")]
    {
        let bundle = app.config().identifier.clone();
        let service = permission.tcc_service();
        tauri::async_runtime::spawn_blocking(move || {
            let mut cmd = std::process::Command::new("/usr/bin/tccutil");
            cmd.args(["reset", service, &bundle]);
            let status = crate::spawn_gate::status(&mut cmd)
                .map_err(|error| format!("could not run tccutil: {error}"))?;
            if !status.success() {
                return Err(format!("tccutil reset {service} {bundle} failed ({status})"));
            }
            Ok(())
        })
        .await
        .map_err(|error| format!("join error: {error}"))??;
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (app, permission);
        return Err("Computer Use permissions are supported only on macOS".to_owned());
    }
    permissions(&app, None).await
}

#[tauri::command]
pub async fn computer_use_open_settings(permission: Permission) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        tauri::async_runtime::spawn_blocking(move || {
            let _gate = crate::spawn_gate::shared();
            #[allow(clippy::disallowed_methods)] // launch is protected by the spawn gate
            open::that_detached(permission.settings_url()).map_err(|error| error.to_string())
        }).await.map_err(|error| format!("join error: {error}"))?
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = permission;
        Err("Computer Use permissions are supported only on macOS".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_read_requires_boolean_value() {
        assert!(parse_enabled(br#"{"value":true}"#).unwrap());
        assert!(!parse_enabled(br#"{"value":false}"#).unwrap());
        for invalid in [br#"{}"#.as_slice(), br#"{"value":null}"#, br#"{"value":"true"}"#] {
            assert!(parse_enabled(invalid).is_err());
        }
    }
}
