use crate::bridge::{Bridge, ContextHost};
use crate::controls::Controls;
use crate::params::ThousandParams;
use baseview::{Event, EventStatus, Size, Window, WindowHandle, WindowOpenOptions, WindowScalePolicy};
use keyboard_types::{KeyState, Modifiers};
use nih_plug::prelude::{Editor, GuiContext, ParentWindowHandle};
use raw_window_handle::{AppKitWindowHandle, HasRawWindowHandle, RawWindowHandle};
use serde_json::{json, Value};
use std::any::Any;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};
use wry::{WebView, WebViewBuilder};

const SIZE: (u32, u32) = (960, 540);
const UI_HTML: &str = include_str!("../../ui/index.html");
/// Set to a dev server URL (e.g. http://localhost:5173) to load the UI live.
const DEV_URL_VAR: &str = "PATINA_UI_URL";

pub struct WebEditor {
    params: Arc<ThousandParams>,
    controls: Arc<Controls>,
    changed: Arc<AtomicBool>,
}

impl WebEditor {
    pub fn new(params: Arc<ThousandParams>, controls: Arc<Controls>) -> Self {
        Self { params, controls, changed: Arc::new(AtomicBool::new(true)) }
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
            _ => unreachable!("Patina only supports macOS"),
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
            title: "Patina".into(),
            size: Size::new(SIZE.0 as f64, SIZE.1 as f64),
            scale: WindowScalePolicy::SystemScaleFactor,
        };
        let params = self.params.clone();
        let controls = self.controls.clone();
        let changed = self.changed.clone();
        changed.store(true, Ordering::Release);
        let handle = Window::open_parented(&Parent(parent), options, move |_window| {
            let (tx, inbox) = channel();
            let bridge = Bridge::new(params.clone(), controls);
            Handler {
                context,
                params,
                changed,
                webview: None,
                tx: Some(tx),
                inbox,
                bridge,
                ready: false,
                page_has_focus: false,
                unsaved_since: None,
                seen_session: u32::MAX,
                seen_tempo: None,
            }
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
    /// Taken when the webview is built.
    tx: Option<Sender<String>>,
    inbox: Receiver<String>,
    bridge: Bridge,
    ready: bool,
    /// Something on the page (a knob, the search box) has keyboard focus.
    page_has_focus: bool,
    /// In the app: when the sound last changed without being saved.
    unsaved_since: Option<Instant>,
    seen_session: u32,
    seen_tempo: Option<f32>,
}

impl Handler {
    /// Built on the first frame rather than in `spawn`: a standalone window
    /// only attaches our view to its NSWindow after `spawn` returns, and
    /// wry needs the window.
    fn build_webview(&mut self, window: &mut Window) {
        let Some(tx) = self.tx.take() else { return };
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
        self.webview = builder.and_then(|b| b.build()).ok();
    }

    fn send(&self, messages: Vec<Value>) {
        let Some(webview) = &self.webview else {
            return;
        };
        for message in messages {
            let _ = webview.evaluate_script(&format!("window.onPluginMessage?.({message})"));
        }
    }

    fn handle(&mut self, raw: &str) {
        let Ok(msg) = serde_json::from_str::<Value>(raw) else {
            return;
        };
        match msg["type"].as_str() {
            Some("focus") => {
                if let Some(webview) = &self.webview {
                    webview.focus();
                }
            }
            Some("keys") => self.page_has_focus = msg["active"].as_bool().unwrap_or(false),
            Some(kind) => {
                self.ready |= kind == "ready";
                let mut out = Vec::new();
                let mut gui = ContextHost::new(&*self.context, &self.params);
                self.bridge.handle(&msg, &mut gui, &mut out);
                self.send(out);
            }
            None => {}
        }
    }
}

impl Handler {
    /// In the app, saves the sound a second after it stops changing, so the
    /// next launch starts where this one left off.
    fn autosave(&mut self, changed: bool) {
        if crate::app::get().is_none() {
            return;
        }
        let session = self.params.session_generation();
        let tempo = self.bridge.tempo();
        if changed || session != self.seen_session || tempo != self.seen_tempo {
            self.seen_session = session;
            self.seen_tempo = tempo;
            self.unsaved_since = Some(Instant::now());
        }
        if self.unsaved_since.is_some_and(|t| t.elapsed() > Duration::from_secs(1)) {
            self.unsaved_since = None;
            crate::app::save(&self.params, self.bridge.tempo(), &ContextHost::new(&*self.context, &self.params));
        }
    }
}

impl Drop for Handler {
    fn drop(&mut self) {
        if crate::app::get().is_some() && self.ready {
            crate::app::save(&self.params, self.bridge.tempo(), &ContextHost::new(&*self.context, &self.params));
        }
    }
}

impl baseview::WindowHandler for Handler {
    fn on_frame(&mut self, window: &mut Window) {
        self.build_webview(window);
        while let Ok(msg) = self.inbox.try_recv() {
            self.handle(&msg);
        }
        if self.ready {
            let changed = self.changed.swap(false, Ordering::AcqRel);
            let mut out = Vec::new();
            let gui = ContextHost::new(&*self.context, &self.params);
            self.bridge.poll(&gui, changed, &mut out);
            self.send(out);
            self.autosave(changed);
        }
    }

    /// baseview's view grabs first-responder status, so keys can land here
    /// instead of in the webview. Forward them while the page has focus;
    /// otherwise let them through so host shortcuts like play keep working.
    fn on_event(&mut self, _window: &mut Window, event: Event) -> EventStatus {
        let Event::Keyboard(e) = event else {
            return EventStatus::Ignored;
        };
        if !self.page_has_focus {
            return EventStatus::Ignored;
        }
        self.send(vec![json!({
            "type": "key",
            "event": if e.state == KeyState::Down { "keydown" } else { "keyup" },
            "key": e.key.to_string(),
            "code": e.code.to_string(),
            "repeat": e.repeat,
            "shiftKey": e.modifiers.contains(Modifiers::SHIFT),
            "altKey": e.modifiers.contains(Modifiers::ALT),
            "ctrlKey": e.modifiers.contains(Modifiers::CONTROL),
            "metaKey": e.modifiers.contains(Modifiers::META),
        })]);
        EventStatus::Captured
    }
}
