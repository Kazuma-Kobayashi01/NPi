//! Public TCC APIs, invoked only on the application's main runloop.

use super::{Permission, Permissions};
use std::ffi::c_void;

type CfRef = *const c_void;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGPreflightScreenCaptureAccess() -> bool;
    fn CGRequestScreenCaptureAccess() -> bool;
    fn CGPreflightListenEventAccess() -> bool;
    fn CGRequestListenEventAccess() -> bool;
}

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrustedWithOptions(options: CfRef) -> u8;
    static kAXTrustedCheckOptionPrompt: CfRef;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    static kCFBooleanTrue: CfRef;
    fn CFDictionaryCreate(
        allocator: CfRef,
        keys: *const CfRef,
        values: *const CfRef,
        count: isize,
        key_callbacks: CfRef,
        value_callbacks: CfRef,
    ) -> CfRef;
    fn CFRelease(value: CfRef);
}

pub(super) fn permissions(request: Option<Permission>) -> Result<Permissions, String> {
    // SAFETY: these public APIs accept no owned buffers. AX's prompt dictionary
    // contains process-lifetime CF constants and is released after the call.
    unsafe {
        match request {
            Some(Permission::ScreenRecording) => {
                CGRequestScreenCaptureAccess();
            }
            Some(Permission::InputMonitoring) => {
                CGRequestListenEventAccess();
            }
            Some(Permission::Accessibility) => {
                let key = kAXTrustedCheckOptionPrompt;
                let value = kCFBooleanTrue;
                let options = CFDictionaryCreate(
                    std::ptr::null(),
                    &key,
                    &value,
                    1,
                    std::ptr::null(),
                    std::ptr::null(),
                );
                if options.is_null() {
                    return Err("could not allocate macOS accessibility permission options".to_owned());
                }
                AXIsProcessTrustedWithOptions(options);
                CFRelease(options);
            }
            None => {}
        }
        // Request return values are not an onboarding state: permission can be
        // changed in Settings asynchronously and may require restarting the host.
        // Re-query flags without prompting; never distinguish denied/unrequested.
        Ok(Permissions {
            screen_recording: CGPreflightScreenCaptureAccess(),
            accessibility: AXIsProcessTrustedWithOptions(std::ptr::null()) != 0,
            input_monitoring: CGPreflightListenEventAccess(),
        })
    }
}
