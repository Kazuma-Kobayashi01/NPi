//! Writes text (and, for a paste into Word or a mail, HTML) to the system clipboard from Rust.
//!
//! The webview's own `navigator.clipboard.write` is refused by WebKit once an `await` (here: asking Rust
//! to format the references) has used up the click's user gesture, which is what made "Copy" fail with
//! "could not copy". `NSPasteboard` has no such rule, and the app is macOS-only. The class is looked up
//! by name at run time; elsewhere the command reports it is unsupported and the page falls back to the
//! browser API.

/// Puts `text` on the clipboard; with `html` also as rich text.
#[tauri::command]
pub fn clipboard_write(text: String, html: Option<String>) -> Result<(), String> {
    imp::write(&text, html.as_deref())
}

#[cfg(not(target_os = "macos"))]
mod imp {
    pub fn write(_: &str, _: Option<&str>) -> Result<(), String> {
        Err("the system clipboard is not available here".to_string())
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use objc2::msg_send;
    use objc2::runtime::{AnyClass, AnyObject};
    use objc2_foundation::NSString;

    pub fn write(text: &str, html: Option<&str>) -> Result<(), String> {
        let class = AnyClass::get(c"NSPasteboard").ok_or("the system clipboard is not available")?;
        // SAFETY: `NSPasteboard` and the messages below are AppKit's documented API (`generalPasteboard`,
        // `clearContents`, `setString:forType:`); the strings outlive the calls, and the pasteboard is
        // the process-wide general one, which AppKit allows from any thread.
        unsafe {
            let board: *mut AnyObject = msg_send![class, generalPasteboard];
            if board.is_null() {
                return Err("the system clipboard is not available".into());
            }
            let _: isize = msg_send![board, clearContents];
            let put = |value: &str, kind: &str| -> bool {
                let (value, kind) = (NSString::from_str(value), NSString::from_str(kind));
                msg_send![board, setString: &*value, forType: &*kind]
            };
            let mut ok = put(text, "public.utf8-plain-text");
            if let Some(html) = html {
                ok &= put(html, "public.html");
            }
            if ok { Ok(()) } else { Err("the clipboard refused the text".into()) }
        }
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    #[test]
    fn text_and_html_reach_the_pasteboard() {
        // Not asserted on the real clipboard of whoever runs the tests: only that the call is accepted.
        let saved = std::process::Command::new("/usr/bin/pbpaste").output().ok().map(|o| o.stdout);
        super::imp::write("npi clipboard test", Some("<p>npi clipboard test</p>")).expect("write");
        let got = std::process::Command::new("/usr/bin/pbpaste").output().expect("pbpaste").stdout;
        assert_eq!(String::from_utf8_lossy(&got), "npi clipboard test");
        if let Some(old) = saved {
            use std::io::Write as _;
            if let Ok(mut child) = std::process::Command::new("/usr/bin/pbcopy").stdin(std::process::Stdio::piped()).spawn() {
                let _ = child.stdin.take().map(|mut s| s.write_all(&old));
                let _ = child.wait();
            }
        }
    }
}
