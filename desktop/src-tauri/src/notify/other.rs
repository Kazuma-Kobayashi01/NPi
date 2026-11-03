//! Windows and Linux: the desktop's own notification service. Neither has a
//! per-app permission to ask for through this path, and a click on the toast
//! is not wired, so the status is always granted and `sid` is unused.

use super::Status;

pub fn status() -> Status {
    Status::Granted
}

pub fn request() -> Status {
    Status::Granted
}

pub fn last_error() -> Option<String> {
    None
}

pub const fn signing() -> &'static str {
    "unknown"
}

pub fn post(title: &str, body: &str, _sid: &str) {
    #[cfg(target_os = "linux")]
    {
        let mut cmd = std::process::Command::new("notify-send");
        cmd.arg("--app-name=NPi Desktop")
            .arg("--")
            .arg(title)
            .arg(body);
        let _ = crate::spawn_gate::status(&mut cmd);
    }
    #[cfg(target_os = "windows")]
    {
        // The text goes through the environment, never into the script.
        let script = "[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null; \
            $t = [Windows.UI.Notifications.ToastNotificationManager]::GetTemplateContent([Windows.UI.Notifications.ToastTemplateType]::ToastText02); \
            $n = $t.GetElementsByTagName('text'); \
            $n.Item(0).AppendChild($t.CreateTextNode($env:NPI_NOTIFY_TITLE)) | Out-Null; \
            $n.Item(1).AppendChild($t.CreateTextNode($env:NPI_NOTIFY_BODY)) | Out-Null; \
            [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('NPi Desktop').Show([Windows.UI.Notifications.ToastNotification]::new($t));";
        let mut cmd = std::process::Command::new("powershell");
        cmd.args(["-NoProfile", "-Command", script])
            .env("NPI_NOTIFY_TITLE", title)
            .env("NPI_NOTIFY_BODY", body);
        let _ = crate::spawn_gate::status(&mut cmd);
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    let _ = (title, body);
}
