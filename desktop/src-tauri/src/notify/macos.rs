//! macOS: the real notification flow through `UNUserNotificationCenter`.
//!
//! Authorization is asked from the user by the system (`request`), its state
//! read back (`status`), a notification posted (`post`), and a click on one
//! reaches `NotifyState::clicked` through the delegate. The centre only works
//! for a launched `.app` bundle (an unbundled `tauri dev` binary has no bundle
//! identifier and `currentNotificationCenter` would raise), so every entry
//! checks `supported` first.

use std::ptr::NonNull;
use std::sync::mpsc;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use block2::{DynBlock, RcBlock};
use objc2::rc::Retained;
use objc2::runtime::{Bool, ProtocolObject};
use objc2::{define_class, AnyThread};
use objc2_foundation::{NSBundle, NSError, NSObject, NSObjectProtocol, NSString};
use objc2_user_notifications::{
    UNAuthorizationOptions, UNMutableNotificationContent, UNNotification,
    UNNotificationPresentationOptions, UNNotificationRequest, UNNotificationResponse,
    UNNotificationSettings, UNNotificationSound, UNUserNotificationCenter,
    UNUserNotificationCenterDelegate,
};
use tauri::AppHandle;

use super::Status;

/// The system answers through a completion block on its own queue.
const ANSWER_TIMEOUT: Duration = Duration::from_secs(10);
/// Separates our counter from the tab id inside a request identifier.
const SEP: char = '|';

static APP: OnceLock<AppHandle> = OnceLock::new();
/// What the system last said when it refused a request or a post, for the Settings row.
static LAST_ERROR: Mutex<Option<String>> = Mutex::new(None);

fn remember(error: *mut NSError) {
    // SAFETY: the system passes either null or a live NSError for the duration of its completion block.
    let text = unsafe { error.as_ref() }.map(|e| format!("{} ({}, {})", e.localizedDescription(), e.domain(), e.code()));
    if let Ok(mut last) = LAST_ERROR.lock() {
        *last = text;
    }
}

/// The system's last refusal, if any.
pub fn last_error() -> Option<String> {
    LAST_ERROR.lock().ok().and_then(|e| e.clone())
}

/// `adhoc` when the running bundle carries an ad-hoc signature: such a build has a new code
/// identity on every build, so the system treats each as a different app and may not keep its
/// notification permission. `signed` otherwise, `unknown` when `codesign` could not be run.
pub fn signing() -> &'static str {
    let bundle = NSBundle::mainBundle().bundlePath().to_string();
    let mut cmd = std::process::Command::new("/usr/bin/codesign");
    cmd.args(["-dv", "--verbose=2", &bundle]);
    match crate::spawn_gate::output(&mut cmd) {
        Ok(out) => {
            let text = String::from_utf8_lossy(&out.stderr);
            if text.contains("Signature=adhoc") { "adhoc" } else { "signed" }
        }
        Err(_) => "unknown",
    }
}

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "NpiNotificationDelegate"]
    struct Delegate;

    unsafe impl NSObjectProtocol for Delegate {}

    unsafe impl UNUserNotificationCenterDelegate for Delegate {
        // Shown even while the app is frontmost: only the Settings test does that.
        #[unsafe(method(userNotificationCenter:willPresentNotification:withCompletionHandler:))]
        fn will_present(
            &self,
            _center: &UNUserNotificationCenter,
            _notification: &UNNotification,
            handler: &DynBlock<dyn Fn(UNNotificationPresentationOptions)>,
        ) {
            handler.call((UNNotificationPresentationOptions::Banner
                | UNNotificationPresentationOptions::Sound,));
        }

        #[unsafe(method(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:))]
        fn did_receive(
            &self,
            _center: &UNUserNotificationCenter,
            response: &UNNotificationResponse,
            handler: &DynBlock<dyn Fn()>,
        ) {
            let id = response.notification().request().identifier().to_string();
            if let (Some(app), Some((_, sid))) = (APP.get(), id.split_once(SEP)) {
                super::clicked(app, sid);
            }
            handler.call(());
        }
    }
);

/// Whether this process is a bundled app the system will talk to.
fn supported() -> bool {
    let bundle = NSBundle::mainBundle();
    bundle.bundleIdentifier().is_some()
        && std::path::Path::new(&bundle.bundlePath().to_string())
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("app"))
}

/// Becomes the centre's delegate; called once at startup.
pub fn install(app: &AppHandle) {
    if !supported() || APP.set(app.clone()).is_err() {
        return;
    }
    // SAFETY: `init` on a freshly allocated instance of a class with no ivars.
    let delegate: Retained<Delegate> = unsafe { objc2::msg_send![Delegate::alloc(), init] };
    UNUserNotificationCenter::currentNotificationCenter()
        .setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    // The centre holds its delegate weakly: this one lives as long as the app.
    std::mem::forget(delegate);
}

const fn map(raw: isize) -> Status {
    match raw {
        0 => Status::NotDetermined,
        2..=4 => Status::Granted,
        _ => Status::Denied,
    }
}

pub fn status() -> Status {
    if !supported() {
        return Status::Unsupported;
    }
    let (tx, rx) = mpsc::channel();
    let block = RcBlock::new(move |settings: NonNull<UNNotificationSettings>| {
        // SAFETY: the system hands over a live settings object for the call.
        let _ = tx.send(unsafe { settings.as_ref() }.authorizationStatus().0);
    });
    UNUserNotificationCenter::currentNotificationCenter()
        .getNotificationSettingsWithCompletionHandler(&block);
    rx.recv_timeout(ANSWER_TIMEOUT).map_or(Status::Denied, map)
}

/// Asks the system (it shows its prompt the first time only), then reads the answer.
pub fn request() -> Status {
    if !supported() {
        return Status::Unsupported;
    }
    let (tx, rx) = mpsc::channel();
    let block = RcBlock::new(move |_granted: Bool, error: *mut NSError| {
        remember(error);
        let _ = tx.send(());
    });
    UNUserNotificationCenter::currentNotificationCenter()
        .requestAuthorizationWithOptions_completionHandler(
            UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound,
            &block,
        );
    let _ = rx.recv_timeout(ANSWER_TIMEOUT);
    status()
}

/// Posts one notification; `sid` rides in the identifier so a click finds its tab.
pub fn post(title: &str, body: &str, sid: &str) {
    if !supported() {
        return;
    }
    let content = UNMutableNotificationContent::new();
    content.setTitle(&NSString::from_str(title));
    content.setBody(&NSString::from_str(body));
    content.setSound(Some(&UNNotificationSound::defaultSound()));
    let identifier = NSString::from_str(&format!("npi.{}{SEP}{sid}", super::next_id()));
    let request =
        UNNotificationRequest::requestWithIdentifier_content_trigger(&identifier, &content, None);
    let done = RcBlock::new(|error: *mut NSError| remember(error));
    UNUserNotificationCenter::currentNotificationCenter()
        .addNotificationRequest_withCompletionHandler(&request, Some(&done));
}
