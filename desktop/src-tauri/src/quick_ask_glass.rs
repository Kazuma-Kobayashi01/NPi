//! The quick-ask window on macOS: a non-activating panel, and the blur behind it.
//!
//! [`make_panel`] turns the window into an `NSPanel` that takes the keyboard
//! without activating the app, which an `NSWindow` cannot do. [`apply`] puts an
//! `NSVisualEffectView` behind the transparent webview, which blurs what is behind
//! the window (a page cannot); the glass itself is drawn by the page (`quick.css`). The class is looked up by name at run time, so
//! this builds against any SDK and does nothing where it is missing.
//! Everything runs on the main thread.

use objc2::ffi::object_setClass;
use objc2::runtime::{AnyClass, AnyObject, Bool, ClassBuilder, Sel};
use objc2::{msg_send, sel};
use objc2_foundation::{NSPoint, NSRect, NSSize};

/// What backs the bar: a blur view, or nothing (the page paints its own background).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backdrop {
    Vibrancy,
    None,
}

/// `NSWindowStyleMaskNonactivatingPanel`.
const NONACTIVATING_PANEL: usize = 1 << 7;
/// `canJoinAllSpaces | fullScreenAuxiliary`: shows over the space and full-screen app in use.
const COLLECTION_BEHAVIOR: usize = (1 << 0) | (1 << 8);

extern "C-unwind" fn yes(_: *mut AnyObject, _: Sel) -> Bool {
    Bool::YES
}

extern "C-unwind" fn no(_: *mut AnyObject, _: Sel) -> Bool {
    Bool::NO
}

/// An `NSPanel` that may become key though the window has no title bar, and is never main.
fn panel_class() -> Option<&'static AnyClass> {
    if let Some(class) = AnyClass::get(c"NPiQuickPanel") {
        return Some(class);
    }
    let mut builder = ClassBuilder::new(c"NPiQuickPanel", AnyClass::get(c"NSPanel")?)?;
    // SAFETY: both functions have the signature of the `-(BOOL)` selectors they implement.
    unsafe {
        builder.add_method(
            sel!(canBecomeKeyWindow),
            yes as extern "C-unwind" fn(_, _) -> _,
        );
        builder.add_method(
            sel!(canBecomeMainWindow),
            no as extern "C-unwind" fn(_, _) -> _,
        );
    }
    Some(builder.register())
}

/// Turns `ns_window` into a non-activating panel. tao keeps its state in the window's delegate and
/// its content view, so changing the class leaves focus and size events working. Main thread only.
pub fn make_panel(ns_window: *mut std::ffi::c_void) {
    let Some(class) = panel_class() else { return };
    if ns_window.is_null() {
        return;
    }
    let ns_window = ns_window.cast::<AnyObject>();
    // SAFETY: `ns_window` is the live window of a window we just built, used on the main thread.
    // `NPiQuickPanel` descends from `NSWindow` and adds no instance variables, so the object's
    // layout still fits, and the messages are `NSWindow`'s and `NSPanel`'s own.
    unsafe {
        object_setClass(ns_window, std::ptr::from_ref(class));
        let mask: usize = msg_send![ns_window, styleMask];
        let _: () = msg_send![ns_window, setStyleMask: mask | NONACTIVATING_PANEL];
        let _: () = msg_send![ns_window, setCollectionBehavior: COLLECTION_BEHAVIOR];
        let _: () = msg_send![ns_window, setHidesOnDeactivate: false];
        let _: () = msg_send![ns_window, setFloatingPanel: true];
    }
}

/// Gives `view` the keyboard in `ns_window`, without activating the app. Main thread only.
pub fn focus_view(ns_window: *mut std::ffi::c_void, view: *mut std::ffi::c_void) {
    if ns_window.is_null() || view.is_null() {
        return;
    }
    let (ns_window, view) = (ns_window.cast::<AnyObject>(), view.cast::<AnyObject>());
    // SAFETY: both are live objects of a window on screen, used on the main thread.
    unsafe {
        let _: () = msg_send![ns_window, makeKeyAndOrderFront: std::ptr::null_mut::<AnyObject>()];
        let _: Bool = msg_send![ns_window, makeFirstResponder: view];
    }
}

/// Which backdrop this system can give.
pub fn available() -> Backdrop {
    if AnyClass::get(c"NSVisualEffectView").is_some() {
        Backdrop::Vibrancy
    } else {
        Backdrop::None
    }
}

/// Puts the backdrop behind the webview, inset by `inset` points on every side and rounded by `radius`.
/// `ns_window` is the window's `NSWindow`.
pub fn apply(ns_window: *mut std::ffi::c_void, width: f64, height: f64, inset: f64, radius: f64) {
    let kind = available();
    if kind == Backdrop::None || ns_window.is_null() {
        return;
    }
    let frame = NSRect::new(
        NSPoint::new(inset, inset),
        NSSize::new(width - 2.0 * inset, height - 2.0 * inset),
    );
    let ns_window = ns_window.cast::<AnyObject>();
    // SAFETY: `ns_window` is the live NSWindow of a window we just built, used on the main thread.
    // Every message below is one the named class documents (`contentView`, `initWithFrame:`,
    // `setCornerRadius:`, `setMaterial:`/`setBlendingMode:`/`setState:`, `addSubview:positioned:relativeTo:`),
    // and the view is retained by its superview once added.
    unsafe {
        let content: *mut AnyObject = msg_send![ns_window, contentView];
        if content.is_null() {
            return;
        }
        let view: *mut AnyObject = match kind {
            Backdrop::Vibrancy | Backdrop::None => {
                let Some(class) = AnyClass::get(c"NSVisualEffectView") else {
                    return;
                };
                let view: *mut AnyObject = msg_send![class, alloc];
                let view: *mut AnyObject = msg_send![view, initWithFrame: frame];
                // Popover material (follows the system appearance), behind the window, always active; rounded through its layer.
                let _: () = msg_send![view, setMaterial: 6_isize];
                let _: () = msg_send![view, setBlendingMode: 0_isize];
                let _: () = msg_send![view, setState: 1_isize];
                let _: () = msg_send![view, setWantsLayer: true];
                let layer: *mut AnyObject = msg_send![view, layer];
                if !layer.is_null() {
                    let _: () = msg_send![layer, setCornerRadius: radius];
                    let _: () = msg_send![layer, setMasksToBounds: true];
                }
                view
            }
        };
        if view.is_null() {
            return;
        }
        // Grows with the window (NSViewWidthSizable | NSViewHeightSizable): the page sets the height.
        let _: () = msg_send![view, setAutoresizingMask: 18_usize];
        // NSWindowBelow = -1: behind the webview, which stays transparent above it.
        let none: *mut AnyObject = std::ptr::null_mut();
        let _: () = msg_send![content, addSubview: view, positioned: -1_isize, relativeTo: none];
        let _: () = msg_send![view, release];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_window_is_left_alone() {
        apply(std::ptr::null_mut(), 720.0, 92.0, 14.0, 22.0);
    }

    #[test]
    fn this_system_names_a_backdrop_it_can_give() {
        // A visual-effect view where there is one; never panics either way.
        let _ = available();
    }
}
