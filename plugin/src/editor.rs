use crate::params::{Kind, Pid, ThousandParams, COUNT};
use baseview::{Event, EventStatus, Size, Window, WindowHandle, WindowOpenOptions, WindowScalePolicy};
use nih_plug::prelude::{Editor, GuiContext, ParentWindowHandle};
use raw_window_handle::{AppKitWindowHandle, HasRawWindowHandle, RawWindowHandle};
use serde_json::{json, Value};
use std::any::Any;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;
use wry::{WebView, WebViewBuilder};

const SIZE: (u32, u32) = (960, 540);
const UI_HTML: &str = include_str!("../../ui/index.html");
/// Set to a dev server URL (e.g. http://localhost:5173) to load the UI live.
const DEV_URL_VAR: &str = "THOUSAND_SYNTHS_UI_URL";

pub struct WebEditor {
    params: Arc<ThousandParams>,
    changed: Arc<AtomicBool>,
}

impl WebEditor {
    pub fn new(params: Arc<ThousandParams>) -> Self {
        Self { params, changed: Arc::new(AtomicBool::new(true)) }
    }
}

struct Parent(ParentWindowHandle);

unsafe impl HasRawWindowHandle for Parent {
    fn raw_window_handle(&self) -> RawWindowHandle {
        match self.0 {
            ParentWindowHandle::AppKitNsView(ns_view) => {
                let mut handle = AppKitWindowHandle::empty();
                handle.ns_view = ns_view;
                RawWindowHandle::AppKit(handle)
            }
            _ => unreachable!("1000 Synths only supports macOS"),
        }
    }
}

struct Open(WindowHandle);

impl Drop for Open {
    fn drop(&mut self) {
        self.0.close();
    }
}

// The handle is only touched on the GUI thread that created it.
unsafe impl Send for Open {}

impl Editor for WebEditor {
    fn spawn(&self, parent: ParentWindowHandle, context: Arc<dyn GuiContext>) -> Box<dyn Any + Send> {
        let options = WindowOpenOptions {
            title: "1000 Synths".into(),
            size: Size::new(SIZE.0 as f64, SIZE.1 as f64),
            scale: WindowScalePolicy::SystemScaleFactor,
        };
        let params = self.params.clone();
        let changed = self.changed.clone();
        changed.store(true, Ordering::Release);
        let handle = Window::open_parented(&Parent(parent), options, move |window| {
            let (tx, inbox) = channel();
            let builder = WebViewBuilder::new_as_child(window)
                .with_bounds(wry::Rect { x: 0, y: 0, width: SIZE.0, height: SIZE.1 })
                .with_accept_first_mouse(true)
                .with_devtools(cfg!(debug_assertions))
                .with_background_color((232, 230, 225, 255))
                .with_ipc_handler(move |msg| {
                    let _ = tx.send(msg);
                });
            let builder = match std::env::var(DEV_URL_VAR) {
                Ok(url) => builder.with_url(&url),
                Err(_) => builder.with_html(UI_HTML),
            };
            let webview = builder.and_then(|b| b.build()).ok();
            Handler { context, params, changed, webview, inbox, sent: [f32::NAN; COUNT], ready: false }
        });
        Box::new(Open(handle))
    }

    fn size(&self) -> (u32, u32) {
        SIZE
    }

    fn set_scale_factor(&self, _factor: f32) -> bool {
        false
    }

    // These can arrive on the audio thread, so they only raise a flag; the
    // window diffs and sends values on its own thread.
    fn param_value_changed(&self, _id: &str, _normalized_value: f32) {
        self.changed.store(true, Ordering::Release);
    }

    fn param_modulation_changed(&self, _id: &str, _modulation_offset: f32) {}

    fn param_values_changed(&self) {
        self.changed.store(true, Ordering::Release);
    }
}

struct Handler {
    context: Arc<dyn GuiContext>,
    params: Arc<ThousandParams>,
    changed: Arc<AtomicBool>,
    webview: Option<WebView>,
    inbox: Receiver<String>,
    sent: [f32; COUNT],
    ready: bool,
}

impl Handler {
    fn send(&self, message: Value) {
        if let Some(webview) = &self.webview {
            let _ = webview.evaluate_script(&format!("window.onPluginMessage?.({message})"));
        }
    }

    fn handle(&mut self, raw: &str) {
        let Ok(msg) = serde_json::from_str::<Value>(raw) else {
            return;
        };
        let pid = msg["id"].as_str().and_then(Pid::from_id);
        match (msg["type"].as_str(), pid) {
            (Some("ready"), _) => {
                self.ready = true;
                self.sent = [f32::NAN; COUNT];
                self.send(json!({ "type": "init", "params": self.describe() }));
                self.changed.store(true, Ordering::Release);
            }
            (Some("begin"), Some(pid)) => unsafe { self.context.raw_begin_set_parameter(self.params.ptr(pid)) },
            (Some("set"), Some(pid)) => {
                if let Some(v) = msg["value"].as_f64() {
                    unsafe { self.context.raw_set_parameter_normalized(self.params.ptr(pid), v.clamp(0.0, 1.0) as f32) }
                }
            }
            (Some("end"), Some(pid)) => unsafe { self.context.raw_end_set_parameter(self.params.ptr(pid)) },
            _ => {}
        }
    }

    fn describe(&self) -> Vec<Value> {
        Pid::ALL
            .iter()
            .map(|&pid| {
                let spec = pid.spec();
                let (kind, options, steps) = match spec.kind {
                    Kind::Float { .. } => ("float", Value::Null, 0),
                    Kind::Int { min, max } => ("int", Value::Null, max - min),
                    Kind::Enum(names) => ("enum", json!(names), names.len() as i32 - 1),
                    Kind::Bool => ("bool", Value::Null, 1),
                };
                json!({
                    "id": spec.id, "name": spec.name, "group": spec.group, "kind": kind,
                    "options": options, "steps": steps,
                    "default": spec.normalize(spec.default),
                })
            })
            .collect()
    }

    fn push_changes(&mut self) {
        let mut changes = Vec::new();
        for &pid in Pid::ALL {
            let value = self.params.normalized(pid);
            let last = &mut self.sent[pid as usize];
            if *last != value {
                *last = value;
                changes.push(json!({ "id": pid.spec().id, "value": value, "text": self.params.display(pid, value) }));
            }
        }
        if !changes.is_empty() {
            self.send(json!({ "type": "values", "values": changes }));
        }
    }
}

impl baseview::WindowHandler for Handler {
    fn on_frame(&mut self, _window: &mut Window) {
        while let Ok(msg) = self.inbox.try_recv() {
            self.handle(&msg);
        }
        if self.ready && self.changed.swap(false, Ordering::AcqRel) {
            self.push_changes();
        }
    }

    fn on_event(&mut self, _window: &mut Window, _event: Event) -> EventStatus {
        EventStatus::Ignored
    }
}
