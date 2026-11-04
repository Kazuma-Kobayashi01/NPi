//! The quick-ask bar's blur on macOS: an `NSVisualEffectView` behind the
//! transparent webview, which blurs what is behind the window (a page cannot).
//! The look of the glass itself is drawn by the page (`quickask.css`); this is
//! only the frosted substrate. The class is looked up by name at run time, so
//! this builds against any SDK and does nothing where it is missing.
//! Everything runs on the main thread.

use objc2::runtime::{AnyClass, AnyObject};
use objc2::msg_send;
use objc2_foundation::{NSPoint, NSRect, NSSize};

/// What backs the bar: a blur view, or nothing (the page paints its own background).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backdrop {
    Vibrancy,
    None,
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
                let Some(class) = AnyClass::get(c"NSVisualEffectView") else { return };
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
        // Stays pinned to the top (flexible bottom margin) when the window grows for the menu.
        let _: () = msg_send![view, setAutoresizingMask: 32_usize];
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
