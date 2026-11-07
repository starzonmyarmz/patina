//! The app's window: no title or title bar, the panel running to the top
//! edge under the traffic lights. The plugin's window belongs to the host
//! and is left alone.

use baseview::Window;
use objc::runtime::{Object, YES};
use objc::{msg_send, sel, sel_impl};
use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};

const FULL_SIZE_CONTENT_VIEW: u64 = 1 << 15;
const TITLE_HIDDEN: i64 = 1;

#[repr(C)]
#[derive(Clone, Copy)]
struct Point {
    x: f64,
    y: f64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Size {
    width: f64,
    height: f64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Rect {
    origin: Point,
    size: Size,
}

fn ns_window(window: &Window) -> Option<*mut Object> {
    let RawWindowHandle::AppKit(handle) = window.raw_window_handle() else { return None };
    let view = handle.ns_view as *mut Object;
    if view.is_null() {
        return None;
    }
    let ns_window: *mut Object = unsafe { msg_send![view, window] };
    (!ns_window.is_null()).then_some(ns_window)
}

/// Hides the title bar and keeps the content at `size`.
pub fn hide_title_bar(window: &Window, size: (u32, u32)) {
    let Some(w) = ns_window(window) else { return };
    unsafe {
        let mask: u64 = msg_send![w, styleMask];
        let _: () = msg_send![w, setStyleMask: mask | FULL_SIZE_CONTENT_VIEW];
        let _: () = msg_send![w, setTitlebarAppearsTransparent: YES];
        let _: () = msg_send![w, setTitleVisibility: TITLE_HIDDEN];
        let _: () = msg_send![w, setContentSize: Size { width: size.0 as f64, height: size.1 as f64 }];
    }
}

/// Moves the window by a drag on the page, in points with y down.
pub fn move_by(window: &Window, dx: f64, dy: f64) {
    let Some(w) = ns_window(window) else { return };
    unsafe {
        let frame: Rect = msg_send![w, frame];
        let origin = Point { x: frame.origin.x + dx, y: frame.origin.y - dy };
        let _: () = msg_send![w, setFrameOrigin: origin];
    }
}
