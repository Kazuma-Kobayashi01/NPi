//! Native macOS indicators drawn directly with AppKit + Core Animation.
//!
//! Three borderless, transparent windows own the whole share: an
//! inward-gradient border around the operated target, an always-visible arrow
//! pointer at the real operation point, and one small exit button that ends the
//! takeover. Nothing here is a web page, so there is no page load and no script
//! handshake that native input could ever wait on.

use std::ffi::{c_char, c_void, CString};
use std::sync::{Mutex, OnceLock};

use tauri::AppHandle;

use super::state::{Bounds, Display};

#[repr(C)]
#[derive(Clone, Copy)]
struct Point { x: f64, y: f64 }

#[repr(C)]
#[derive(Clone, Copy)]
struct Size { width: f64, height: f64 }

#[repr(C)]
#[derive(Clone, Copy)]
struct Rect { origin: Point, size: Size }

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGMainDisplayID() -> u32;
    fn CGDisplayBounds(display: u32) -> Rect;
    fn CGColorCreateSRGB(red: f64, green: f64, blue: f64, alpha: f64) -> *mut c_void;
    fn CGColorRelease(color: *mut c_void);
    fn CGPathCreateWithRoundedRect(
        rect: Rect,
        corner_width: f64,
        corner_height: f64,
        transform: *const c_void,
    ) -> *mut c_void;
    fn CGPathRelease(path: *mut c_void);
}

#[link(name = "objc")]
unsafe extern "C" {
    fn sel_registerName(name: *const c_char) -> *mut c_void;
    fn objc_getClass(name: *const c_char) -> *mut c_void;
    fn objc_allocateClassPair(superclass: *mut c_void, name: *const c_char, extra: usize) -> *mut c_void;
    fn objc_registerClassPair(class: *mut c_void);
    fn class_addMethod(class: *mut c_void, selector: *mut c_void, imp: *const c_void, types: *const c_char) -> bool;
    fn objc_msgSend();
}

/// Inward gradient: rings inset from the frame edge, fading towards the middle.
const RINGS: [(f64, f64, f64); 4] = [(0.0, 0.90, 3.0), (3.5, 0.42, 3.0), (7.5, 0.18, 4.0), (12.5, 0.07, 5.0)];

fn hex_to_rgb(hex: &str) -> (f64, f64, f64) {
    if hex.len() == 7 && hex.starts_with('#') {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&hex[1..3], 16),
            u8::from_str_radix(&hex[3..5], 16),
            u8::from_str_radix(&hex[5..7], 16),
        ) {
            return (r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0);
        }
    }
    (0.31, 0.55, 1.0)
}

struct Share {
    session: String,
    border: *mut c_void,
    rings: Vec<*mut c_void>,
    pointer: *mut c_void,
    exit: *mut c_void,
}

unsafe impl Send for Share {}

static SHARE: Mutex<Option<Share>> = Mutex::new(None);
static EXIT_HOOK: OnceLock<AppHandle> = OnceLock::new();
static EXIT_CLASS: Mutex<Option<usize>> = Mutex::new(None);

unsafe fn sel(name: &str) -> *mut c_void {
    let text = CString::new(name).unwrap_or_default();
    sel_registerName(text.as_ptr())
}

unsafe fn msg_ptr0(obj: *mut c_void, name: &str) -> *mut c_void {
    let f: unsafe extern "C" fn(*mut c_void, *mut c_void) -> *mut c_void =
        std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    f(obj, sel(name))
}

unsafe fn msg_ptr1(obj: *mut c_void, name: &str, arg: *mut c_void) -> *mut c_void {
    let f: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> *mut c_void =
        std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    f(obj, sel(name), arg)
}

unsafe fn msg_void(obj: *mut c_void, name: &str) {
    let f: unsafe extern "C" fn(*mut c_void, *mut c_void) =
        std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    f(obj, sel(name));
}

unsafe fn msg_void1(obj: *mut c_void, name: &str, arg: *mut c_void) {
    let f: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) =
        std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    f(obj, sel(name), arg);
}

unsafe fn msg_bool(obj: *mut c_void, name: &str, value: bool) {
    let f: unsafe extern "C" fn(*mut c_void, *mut c_void, bool) =
        std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    f(obj, sel(name), value);
}

unsafe fn msg_usize(obj: *mut c_void, name: &str, value: usize) {
    let f: unsafe extern "C" fn(*mut c_void, *mut c_void, usize) =
        std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    f(obj, sel(name), value);
}

unsafe fn msg_f64(obj: *mut c_void, name: &str, value: f64) {
    let f: unsafe extern "C" fn(*mut c_void, *mut c_void, f64) =
        std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    f(obj, sel(name), value);
}

unsafe fn msg_rect(obj: *mut c_void, name: &str, rect: Rect) {
    let f: unsafe extern "C" fn(*mut c_void, *mut c_void, Rect) =
        std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    f(obj, sel(name), rect);
}

unsafe fn new_object(class: *mut c_void) -> *mut c_void {
    msg_ptr0(msg_ptr0(class, "alloc"), "init")
}

unsafe fn new_layer(class: &str) -> *mut c_void {
    let name = CString::new(class).unwrap_or_default();
    new_object(objc_getClass(name.as_ptr()))
}

unsafe fn nsstring(value: &str) -> *mut c_void {
    let text = CString::new(value.replace('\0', "")).unwrap_or_default();
    msg_ptr1(objc_getClass(c"NSString".as_ptr()), "stringWithUTF8String:", text.as_ptr() as *mut c_void)
}

/// A borderless, transparent panel; `click_through` decides whether it can be hit.
unsafe fn panel(click_through: bool) -> (*mut c_void, *mut c_void) {
    let init: unsafe extern "C" fn(*mut c_void, *mut c_void, Rect, usize, usize, bool) -> *mut c_void =
        std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    let window = init(
        msg_ptr0(objc_getClass(c"NSWindow".as_ptr()), "alloc"),
        sel("initWithContentRect:styleMask:backing:defer:"),
        Rect { origin: Point { x: 0.0, y: 0.0 }, size: Size { width: 1.0, height: 1.0 } },
        0,
        2,
        false,
    );
    msg_void1(window, "setBackgroundColor:", msg_ptr0(objc_getClass(c"NSColor".as_ptr()), "clearColor"));
    msg_bool(window, "setOpaque:", false);
    msg_bool(window, "setHasShadow:", false);
    msg_bool(window, "setIgnoresMouseEvents:", click_through);
    msg_usize(window, "setLevel:", 25);
    msg_usize(window, "setCollectionBehavior:", 1 | 256);
    msg_usize(window, "setSharingType:", 0);
    let view = msg_ptr0(window, "contentView");
    msg_bool(view, "setWantsLayer:", true);
    let layer = new_layer("CALayer");
    msg_void1(view, "setLayer:", layer);
    (window, layer)
}

unsafe fn place(window: *mut c_void, bounds: &Bounds) {
    let primary = CGDisplayBounds(CGMainDisplayID());
    let frame = Rect {
        origin: Point { x: bounds.x, y: primary.size.height - bounds.y - bounds.height },
        size: Size { width: bounds.width, height: bounds.height },
    };
    let set_frame: unsafe extern "C" fn(*mut c_void, *mut c_void, Rect, bool) =
        std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    set_frame(window, sel("setFrame:display:"), frame, true);
}

unsafe fn ring_path(layer: *mut c_void, bounds: &Bounds, inset: f64) {
    let rect = Rect {
        origin: Point { x: inset, y: inset },
        size: Size {
            width: (bounds.width - inset * 2.0).max(1.0),
            height: (bounds.height - inset * 2.0).max(1.0),
        },
    };
    let path = CGPathCreateWithRoundedRect(rect, 10.0, 10.0, std::ptr::null());
    msg_void1(layer, "setPath:", path);
    CGPathRelease(path);
}

unsafe fn arrow_path(layer: *mut c_void) {
    const ARROW: [(f64, f64); 7] = [
        (2.0, 1.0),
        (2.0, 20.0),
        (6.4, 15.6),
        (9.4, 22.4),
        (12.9, 20.9),
        (9.9, 14.1),
        (15.8, 14.1),
    ];
    let path = new_object(objc_getClass(c"CGMutablePath".as_ptr()));
    let move_to: unsafe extern "C" fn(*mut c_void, *mut c_void, Point, *const c_void) =
        std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
    move_to(path, sel("moveToPoint:transform:"), Point { x: ARROW[0].0, y: ARROW[0].1 }, std::ptr::null());
    for (x, y) in &ARROW[1..] {
        let add: unsafe extern "C" fn(*mut c_void, *mut c_void, Point) =
            std::mem::transmute(objc_msgSend as unsafe extern "C" fn());
        add(path, sel("addLineToPoint:"), Point { x: *x, y: *y });
    }
    msg_void(path, "closeSubpath");
    msg_void1(layer, "setPath:", path);
}

/// Clicking the exit button ends the takeover immediately.
extern "C" fn exit_clicked(_this: *mut c_void, _cmd: *mut c_void, _event: *mut c_void) {
    if let Some(app) = EXIT_HOOK.get() {
        crate::computer_overlay::stop_from_share(app);
    }
}

unsafe fn exit_view() -> *mut c_void {
    let mut known = EXIT_CLASS.lock().unwrap();
    let class = match *known {
        Some(class) => class as *mut c_void,
        None => {
            let superclass = objc_getClass(c"NSView".as_ptr());
            let class = objc_allocateClassPair(superclass, c"NPiComputerUseExit".as_ptr(), 0);
            let imp: *const c_void = exit_clicked as *const c_void;
            class_addMethod(class, sel("mouseDown:"), imp, c"v@:@".as_ptr());
            objc_registerClassPair(class);
            *known = Some(class as usize);
            class
        }
    };
    new_object(class)
}

unsafe fn make_exit() -> *mut c_void {
    let (window, _layer) = panel(false);
    // The whole window is the button, so a click never has to be aimed precisely.
    let view = exit_view();
    msg_void1(window, "setContentView:", view);
    msg_bool(view, "setWantsLayer:", true);
    let content = new_layer("CALayer");
    msg_void1(view, "setLayer:", content);
    msg_f64(content, "setCornerRadius:", 9.0);
    msg_void1(content, "setBackgroundColor:", CGColorCreateSRGB(0.10, 0.11, 0.14, 0.94));
    msg_void1(content, "setBorderColor:", CGColorCreateSRGB(1.0, 1.0, 1.0, 0.16));
    msg_f64(content, "setBorderWidth:", 1.0);

    let text = new_layer("CATextLayer");
    msg_void1(content, "addSublayer:", text);
    msg_void1(text, "setString:", nsstring("Exit takeover"));
    msg_f64(text, "setFontSize:", 12.5);
    msg_f64(text, "setContentsScale:", 2.0);
    msg_void1(text, "setForegroundColor:", CGColorCreateSRGB(0.96, 0.97, 1.0, 0.95));
    msg_void1(text, "setAlignmentMode:", nsstring("center"));
    msg_rect(text, "setFrame:", Rect {
        origin: Point { x: 0.0, y: 9.0 },
        size: Size { width: 132.0, height: 16.0 },
    });
    window
}

pub(super) fn hide(_app: &AppHandle) -> Result<(), String> {
    unsafe {
        let guard = SHARE.lock().unwrap();
        if let Some(share) = guard.as_ref() {
            msg_void(share.border, "orderOut:");
            msg_void(share.pointer, "orderOut:");
            msg_void(share.exit, "orderOut:");
        }
    }
    Ok(())
}

pub(super) fn current_session() -> Option<String> {
    SHARE.lock().unwrap().as_ref().map(|share| share.session.clone())
}

pub(super) fn remember_exit_hook(app: &AppHandle) {
    let _ = EXIT_HOOK.set(app.clone());
}

#[cfg(target_os = "macos")]
pub(super) fn render(_app: &AppHandle, display: &Display) -> Result<(), String> {
    let (r, g, b) = hex_to_rgb(&display.accent);
    unsafe {
        let mut guard = SHARE.lock().unwrap();
        if guard.is_none() {
            let (border, blayer) = panel(true);
            let rings = RINGS
                .iter()
                .map(|(_, alpha, width)| {
                    let ring = new_layer("CAShapeLayer");
                    msg_void1(blayer, "addSublayer:", ring);
                    let color = CGColorCreateSRGB(r, g, b, *alpha);
                    msg_void1(ring, "setStrokeColor:", color);
                    msg_void1(ring, "setFillColor:", msg_ptr0(objc_getClass(c"NSColor".as_ptr()), "clearColor"));
                    msg_f64(ring, "setLineWidth:", *width);
                    CGColorRelease(color);
                    ring
                })
                .collect();

            let (pointer, player) = panel(true);
            let arrow = new_layer("CAShapeLayer");
            msg_void1(player, "addSublayer:", arrow);
            arrow_path(arrow);
            msg_void1(arrow, "setFillColor:", CGColorCreateSRGB(1.0, 1.0, 1.0, 0.97));
            msg_void1(arrow, "setStrokeColor:", CGColorCreateSRGB(0.05, 0.06, 0.08, 0.95));
            msg_f64(arrow, "setLineWidth:", 1.2);

            let exit = make_exit();
            *guard = Some(Share { session: display.session_id.clone(), border, rings, pointer, exit });
        }
        let share = guard.as_ref().unwrap();

        let target = display.activity.target.clone();
        if let Some(target) = &target {
            place(share.border, &target.bounds);
            for (ring, (inset, _, width)) in share.rings.iter().zip(RINGS.iter()) {
                ring_path(*ring, &target.bounds, *inset);
                msg_f64(*ring, "setLineWidth:", *width);
            }
            msg_void(share.border, "orderFrontRegardless");
        } else {
            msg_void(share.border, "orderOut:");
        }

        match display.activity.cursor {
            Some(point) => {
                place(share.pointer, &Bounds { x: point.x - 2.0, y: point.y - 2.0, width: 20.0, height: 24.0 });
                msg_void(share.pointer, "orderFrontRegardless");
            }
            None => msg_void(share.pointer, "orderOut:"),
        }

        // The exit button rides the top-right corner of the shared target.
        let anchor = target.map(|target| target.bounds).unwrap_or(Bounds {
            x: 0.0,
            y: 0.0,
            width: CGDisplayBounds(CGMainDisplayID()).size.width,
            height: 0.0,
        });
        let width = 132.0;
        place(share.exit, &Bounds {
            x: (anchor.x + anchor.width - width - 18.0).max(18.0),
            y: anchor.y + 18.0,
            width,
            height: 34.0,
        });
        msg_void(share.exit, "orderFrontRegardless");
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub(super) fn render(_app: &AppHandle, _display: &Display) -> Result<(), String> {
    Ok(())
}
