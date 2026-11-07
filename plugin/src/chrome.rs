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

/// Puts the traffic lights `left` points from the window's left edge,
/// centered `center` points from its top, growing the title bar to fit.
/// AppKit lays them out again on some window changes, so this runs every
/// frame and only touches frames that moved.
pub fn place_traffic_lights(window: &Window, left: f64, center: f64) {
    let Some(w) = ns_window(window) else { return };
    unsafe {
        let buttons: [*mut Object; 3] = [0u64, 1, 2].map(|kind| msg_send![w, standardWindowButton: kind]);
        if buttons.iter().any(|b| b.is_null()) {
            return;
        }
        let close: Rect = msg_send![buttons[0], frame];
        let minimize: Rect = msg_send![buttons[1], frame];
        let spacing = minimize.origin.x - close.origin.x;
        let height = 2.0 * center;

        let title_bar: *mut Object = msg_send![buttons[0], superview];
        let container: *mut Object = msg_send![title_bar, superview];
        let window_frame: Rect = msg_send![w, frame];
        let mut bar: Rect = msg_send![container, frame];
        if (bar.size.height - height).abs() > 0.5 {
            bar.size.height = height;
            bar.origin.y = window_frame.size.height - height;
            let _: () = msg_send![container, setFrame: bar];
        }
        for (i, button) in buttons.into_iter().enumerate() {
            let mut frame: Rect = msg_send![button, frame];
            let x = left + i as f64 * spacing;
            let y = (height - frame.size.height) / 2.0;
            if (frame.origin.x - x).abs() > 0.5 || (frame.origin.y - y).abs() > 0.5 {
                frame.origin = Point { x, y };
                let _: () = msg_send![button, setFrame: frame];
            }
        }
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
