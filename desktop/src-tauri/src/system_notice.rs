//! System notification dispatcher leveraging OS-native notification centers.
//! All Do Not Disturb, Focus modes, and notification sound policies are
//! handled globally by the host operating system.

#[tauri::command]
pub fn system_notification_send(title: String, body: String) -> Result<(), String> {
    let clean_title = title.replace('"', "\\\"").replace('\n', " ");
    let clean_body = body.replace('"', "\\\"").replace('\n', " ");

    #[cfg(target_os = "macos")]
    {
        let script = format!(
            "display notification \"{}\" with title \"{}\"",
            clean_body, clean_title
        );
        let mut cmd = std::process::Command::new("osascript");
        cmd.arg("-e").arg(script);
        let _ = crate::spawn_gate::status(&mut cmd);
    }

    #[cfg(target_os = "linux")]
    {
        let mut cmd = std::process::Command::new("notify-send");
        cmd.arg(&clean_title).arg(&clean_body);
        let _ = crate::spawn_gate::status(&mut cmd);
    }

    #[cfg(target_os = "windows")]
    {
        let ps_cmd = format!(
            "[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null; \
             $template = [Windows.UI.Notifications.ToastNotificationManager]::GetTemplateContent([Windows.UI.Notifications.ToastTemplateType]::ToastText02); \
             $textNodes = $template.GetElementsByTagName('text'); \
             $textNodes.Item(0).AppendChild($template.CreateTextNode('{}')) | Out-Null; \
             $textNodes.Item(1).AppendChild($template.CreateTextNode('{}')) | Out-Null; \
             $toast = [Windows.UI.Notifications.ToastNotification]::new($template); \
             [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('NPi Desktop').Show($toast);",
            clean_title, clean_body
        );
        let mut cmd = std::process::Command::new("powershell");
        cmd.arg("-Command").arg(ps_cmd);
        let _ = crate::spawn_gate::status(&mut cmd);
    }

    Ok(())
}
