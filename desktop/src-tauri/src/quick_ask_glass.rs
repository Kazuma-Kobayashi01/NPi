//! The quick-ask window on macOS: a non-activating panel, and the glass behind its page.
//!
//! [`make_panel`] turns the window into an `NSPanel` that takes the keyboard
//! without activating the app, which an `NSWindow` cannot do. [`install`] puts an
//! `AppKit` backdrop behind the transparent webview and [`layout`] gives it one piece
//! per shape the page reports (the card and the capsule, `quick-window.jsx`): on
//! macOS 26 and later these are Liquid Glass (`NSGlassEffectView`, grouped in an
//! `NSGlassEffectContainerView` so neighbouring pieces blend like the system's),
//! before that `NSVisualEffectView` blur. The page paints no glass of its own then
//! (`quick.css` `[data-backdrop]`). `AppKit` classes are looked up at run time, so this
//! runs on any macOS the app supports. Everything runs on the main thread.

use std::cell::RefCell;

use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, Bool, ClassBuilder, Sel};
use objc2::{available, sel, MainThreadMarker, MainThreadOnly as _};
use objc2_app_kit::{
    NSAutoresizingMaskOptions, NSGlassEffectContainerView, NSGlassEffectView,
    NSGlassEffectViewStyle, NSPanel, NSResponder, NSView, NSVisualEffectBlendingMode,
    NSVisualEffectMaterial, NSVisualEffectState, NSVisualEffectView, NSWindow,
    NSWindowCollectionBehavior, NSWindowOrderingMode, NSWindowStyleMask,
};
use objc2_foundation::{NSPoint, NSRect, NSSize};

use crate::quick_ask::Shape;

/// What backs the page: Liquid Glass, blur, or nothing (the page paints its own background).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backdrop {
    Glass,
    Vibrancy,
    None,
}

impl Backdrop {
    /// The page's `backdrop` query value.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Glass => "glass",
            Self::Vibrancy => "vibrancy",
            Self::None => "none",
        }
    }
}

/// The backdrop views of the quick-ask window: `root` fills the window behind the webview,
/// `pieces` are its shapes, in the page's order.
struct Glass {
    kind: Backdrop,
    root: Retained<NSView>,
    pieces: Vec<Retained<NSView>>,
}

thread_local! {
    /// Main thread only, like the views it holds.
    static GLASS: RefCell<Option<Glass>> = const { RefCell::new(None) };
}

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

/// The `NSWindow` behind tao's raw pointer, on the main thread.
///
/// # Safety
/// `ns_window` is null or the live `NSWindow` of one of the app's windows, and stays alive for `'a`.
unsafe fn window<'a>(ns_window: *mut std::ffi::c_void) -> Option<&'a NSWindow> {
    MainThreadMarker::new()?;
    // SAFETY: the caller's contract; a null pointer gives `None`.
    unsafe { ns_window.cast::<NSWindow>().as_ref() }
}

/// Turns `ns_window` into a non-activating panel. tao keeps its state in the window's delegate and
/// its content view, so changing the class leaves focus and size events working. Main thread only.
pub fn make_panel(ns_window: *mut std::ffi::c_void) {
    let Some(class) = panel_class() else { return };
    // SAFETY: `ns_window` is the live window of a window we just built.
    let Some(window) = (unsafe { window(ns_window) }) else {
        return;
    };
    // SAFETY: `NPiQuickPanel` descends from `NSWindow` and adds no instance variables, so the
    // object's layout still fits.
    unsafe { objc2::ffi::object_setClass(ns_window.cast(), std::ptr::from_ref(class)) };
    window.setStyleMask(window.styleMask() | NSWindowStyleMask::NonactivatingPanel);
    window.setCollectionBehavior(
        NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::FullScreenAuxiliary,
    );
    window.setHidesOnDeactivate(false);
    if let Some(panel) = window.downcast_ref::<NSPanel>() {
        panel.setFloatingPanel(true);
    }
}

/// Gives `view` the keyboard in `ns_window`, without activating the app. Main thread only.
pub fn focus_view(ns_window: *mut std::ffi::c_void, view: *mut std::ffi::c_void) {
    // SAFETY: both are live objects of a window on screen; `view` is an `NSView`, so a responder.
    let (Some(window), Some(view)) = (unsafe { window(ns_window) }, unsafe {
        view.cast::<NSResponder>().as_ref()
    }) else {
        return;
    };
    window.makeKeyAndOrderFront(None);
    window.makeFirstResponder(Some(view));
}

/// Which backdrop this system can give.
pub fn available() -> Backdrop {
    if available!(macos = 26.0) && AnyClass::get(c"NSGlassEffectView").is_some() {
        Backdrop::Glass
    } else if AnyClass::get(c"NSVisualEffectView").is_some() {
        Backdrop::Vibrancy
    } else {
        Backdrop::None
    }
}

/// A view that fills its superview and follows its size.
fn filling(view: &NSView, bounds: NSRect) {
    view.setFrame(bounds);
    view.setAutoresizingMask(
        NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable,
    );
}

/// Puts the backdrop behind the webview of `ns_window`, empty until [`layout`]. Main thread only.
pub fn install(ns_window: *mut std::ffi::c_void) {
    let kind = available();
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    // SAFETY: `ns_window` is the live window of a window we just built.
    let Some(content) = (unsafe { window(ns_window) }).and_then(NSWindow::contentView) else {
        return;
    };
    if kind == Backdrop::None {
        return;
    }
    let bounds = content.bounds();
    let root: Retained<NSView> = if kind == Backdrop::Glass {
        let container = NSGlassEffectContainerView::initWithFrame(
            NSGlassEffectContainerView::alloc(mtm),
            bounds,
        );
        let inner = NSView::initWithFrame(NSView::alloc(mtm), bounds);
        filling(&inner, bounds);
        container.setContentView(Some(&inner));
        Retained::into_super(container)
    } else {
        NSView::initWithFrame(NSView::alloc(mtm), bounds)
    };
    filling(&root, bounds);
    // Behind the webview, which stays transparent above it.
    content.addSubview_positioned_relativeTo(&root, NSWindowOrderingMode::Below, None);
    GLASS.with_borrow_mut(|glass| {
        if let Some(old) = glass.replace(Glass {
            kind,
            root,
            pieces: Vec::new(),
        }) {
            old.root.removeFromSuperview();
        }
    });
}

/// A new piece of glass (or blur) of `kind`.
fn piece(kind: Backdrop, mtm: MainThreadMarker) -> Retained<NSView> {
    let zero = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0));
    let view: Retained<NSView> = if kind == Backdrop::Glass {
        let glass = NSGlassEffectView::initWithFrame(NSGlassEffectView::alloc(mtm), zero);
        glass.setStyle(NSGlassEffectViewStyle::Regular);
        Retained::into_super(glass)
    } else {
        let blur = NSVisualEffectView::initWithFrame(NSVisualEffectView::alloc(mtm), zero);
        // Popover material (follows the system appearance), behind the window, always active.
        blur.setMaterial(NSVisualEffectMaterial::Popover);
        blur.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
        blur.setState(NSVisualEffectState::Active);
        blur.setWantsLayer(true);
        Retained::into_super(blur)
    };
    // Keeps its distance from the top as the window grows downwards.
    view.setAutoresizingMask(NSAutoresizingMaskOptions::ViewMinYMargin);
    view
}

/// `shape` (page coordinates, from the top left) in a view `height` points tall (from the bottom left).
fn frame(shape: &Shape, height: f64) -> NSRect {
    NSRect::new(
        NSPoint::new(shape.x, height - shape.y - shape.height),
        NSSize::new(shape.width, shape.height),
    )
}

/// The corner radius of `shape`: what the page asks for, at most a capsule's.
fn radius(shape: &Shape) -> f64 {
    shape.radius.clamp(0.0, shape.width.min(shape.height) / 2.0)
}

/// Gives the backdrop one piece per `shape`, after the window took its new size. Main thread only.
pub fn layout(shapes: &[Shape]) {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    GLASS.with_borrow_mut(|glass| {
        let Some(glass) = glass.as_mut() else { return };
        let parent = match glass.kind {
            Backdrop::Glass => glass
                .root
                .downcast_ref::<NSGlassEffectContainerView>()
                .and_then(NSGlassEffectContainerView::contentView),
            Backdrop::Vibrancy | Backdrop::None => Some(glass.root.clone()),
        };
        let Some(parent) = parent else { return };
        let height = parent.bounds().size.height;
        for extra in glass.pieces.drain(shapes.len().min(glass.pieces.len())..) {
            extra.removeFromSuperview();
        }
        while glass.pieces.len() < shapes.len() {
            let view = piece(glass.kind, mtm);
            parent.addSubview(&view);
            glass.pieces.push(view);
        }
        for (view, shape) in glass.pieces.iter().zip(shapes) {
            view.setFrame(frame(shape, height));
            let radius = radius(shape);
            if let Some(glass) = view.downcast_ref::<NSGlassEffectView>() {
                glass.setCornerRadius(radius);
            } else if let Some(layer) = view.layer() {
                layer.setCornerRadius(radius);
                layer.setMasksToBounds(true);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shape(x: f64, y: f64, width: f64, height: f64, radius: f64) -> Shape {
        Shape {
            x,
            y,
            width,
            height,
            radius,
        }
    }

    #[test]
    fn a_missing_window_is_left_alone() {
        install(std::ptr::null_mut());
        layout(&[shape(20.0, 20.0, 720.0, 66.0, 22.0)]);
    }

    #[test]
    fn this_system_names_a_backdrop_it_can_give() {
        // Glass from macOS 26, blur before; never panics either way.
        assert_ne!(available(), Backdrop::None);
    }

    #[test]
    fn page_coordinates_flip_to_appkit() {
        let f = frame(&shape(20.0, 20.0, 720.0, 66.0, 22.0), 160.0);
        assert_eq!((f.origin.x, f.origin.y), (20.0, 74.0));
        assert_eq!((f.size.width, f.size.height), (720.0, 66.0));
    }

    #[test]
    fn a_capsule_is_rounded_by_half_its_height_at_most() {
        assert_eq!(radius(&shape(0.0, 0.0, 300.0, 32.0, 999.0)), 16.0);
        assert_eq!(radius(&shape(0.0, 0.0, 720.0, 66.0, 22.0)), 22.0);
    }
}
