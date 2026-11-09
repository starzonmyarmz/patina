use crate::bridge::{Bridge, ContextHost};
use crate::controls::Controls;
use crate::params::ThousandParams;
use baseview::dpi::LogicalSize;
use baseview::{
    Event, EventStatus, HandlerError, TimerHandle, Window, WindowContext, WindowHandler, WindowSettings,
    WindowSize,
};
use keyboard_types::{KeyState, Modifiers};
use nice_plug::context::gui::GuiContext;
use nice_plug::editor::dpi::{LogicalSize as EditorLogicalSize, NativeSize, Size};
use nice_plug::editor::{HostCallbacks, HostMainThreadCaller, HostMethods, SpawnedEditor};
use nice_plug::prelude::{Editor, ParentWindowHandle};
use serde_json::{json, Value};
use std::cell::RefCell;
use std::error::Error;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};
use wry::dpi::{LogicalPosition, LogicalSize as WryLogicalSize};
use wry::{WebView, WebViewBuilder};

const SIZE: (u32, u32) = (960, 540);
/// The traffic lights' left inset and center line in the app, matching the
/// page's header row (14 px down, 26 px tall).
#[cfg(target_os = "macos")]
const TRAFFIC_LIGHTS: (f64, f64) = (20.0, 27.0);
const UI_HTML: &str = include_str!("../../ui/index.html");
/// Set to a dev server URL (e.g. http://localhost:5173) to load the UI live.
const DEV_URL_VAR: &str = "PATINA_UI_URL";
const FRAME: Duration = Duration::from_micros(16_667);

pub struct WebEditor {
    params: Arc<ThousandParams>,
    controls: Arc<Controls>,
    changed: Arc<AtomicBool>,
    scale_factor: Arc<AtomicU64>,
}

impl WebEditor {
    pub fn new(params: Arc<ThousandParams>, controls: Arc<Controls>) -> Self {
        Self {
            params,
            controls,
            changed: Arc::new(AtomicBool::new(true)),
            scale_factor: Arc::new(AtomicU64::new(1f64.to_bits())),
        }
    }
}

struct HostCallbackAdapter(Box<dyn HostCallbacks>);

impl baseview::host::HostCallbacks for HostCallbackAdapter {
    fn request_resize(&mut self, new_size: WindowSize) -> Result<(), HandlerError> {
        self.0.request_resize(new_size.physical.into(), new_size.scale_factor).map_err(HandlerError::from_boxed)
    }

    fn destroyed(&mut self) {
        self.0.destroyed();
    }
}

struct MainThreadCallerAdapter(Box<dyn HostMainThreadCaller>);

impl baseview::host::HostMainThreadCaller for MainThreadCallerAdapter {
    fn call_main_thread(&mut self) {
        self.0.call_main_thread();
    }
}

impl Editor for WebEditor {
    type Handle = WebEditorHandle;

    fn spawn(
        &self,
        parent: Option<ParentWindowHandle>,
        wait_for_parent: bool,
        fallback_scale_factor: Option<f64>,
        context: GuiContext,
        host: Option<HostMethods>,
    ) -> Result<SpawnedEditor<Self::Handle>, Box<dyn Error>> {
        let host = host.map(|host| {
            baseview::host::Host::new()
                .with_callbacks(HostCallbackAdapter(host.callbacks))
                .with_main_thread(MainThreadCallerAdapter(host.main_thread_caller))
        });
        let params = self.params.clone();
        let controls = self.controls.clone();
        let changed = self.changed.clone();
        changed.store(true, Ordering::Release);
        let settings = WindowSettings::new()
            .with_title("Patina")
            .with_size(LogicalSize::new(SIZE.0 as f64, SIZE.1 as f64))
            .with_parent(parent.as_ref())
            .with_wait_for_parent(wait_for_parent)
            .with_fallback_scale_factor(fallback_scale_factor);
        let window = Window::create_with_host(
            settings,
            move |window: WindowContext| -> Result<Handler, HandlerError> {
                let frame = window.create_timer(FRAME)?;
                let (tx, inbox) = channel();
                let bridge = Bridge::new(params.clone(), controls);
                Ok(Handler {
                    window,
                    frame,
                    state: RefCell::new(State {
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
                    }),
                })
            },
            host,
        )?;
        self.scale_factor.store(window.size().scale_factor.to_bits(), Ordering::Relaxed);
        let handle = WebEditorHandle { changed: self.changed.clone(), scale_factor: self.scale_factor.clone() };
        Ok(SpawnedEditor { handle, window })
    }

    fn size(&self) -> NativeSize<u32> {
        let scale_factor = f64::from_bits(self.scale_factor.load(Ordering::Relaxed));
        NativeSize::from_size(Size::Logical(EditorLogicalSize::new(SIZE.0 as f64, SIZE.1 as f64)), scale_factor)
    }
}

pub struct WebEditorHandle {
    changed: Arc<AtomicBool>,
    scale_factor: Arc<AtomicU64>,
}

impl nice_plug::editor::EditorHandle for WebEditorHandle {
    type Window = Window;
    type Error = baseview::Error;

    fn run_until_closed(window: Window) -> Result<(), baseview::Error> {
        window.run_until_closed()
    }

    fn set_parent(&self, parent: ParentWindowHandle, window: &Window) -> Result<(), baseview::Error> {
        window.set_parent(&parent)
    }

    fn show(&self, window: &Window) -> Result<(), baseview::Error> {
        window.show()
    }

    fn hide(&self, window: &Window) -> Result<(), baseview::Error> {
        window.hide()
    }

    fn set_size(&self, _new_size: NativeSize<u32>, _window: &Window) -> Result<(), baseview::Error> {
        Ok(())
    }

    fn set_fallback_scale_factor(&self, scale_factor: f64, window: &Window) -> Result<(), baseview::Error> {
        window.suggest_fallback_scale_factor(scale_factor)?;
        self.scale_factor.store(window.size().scale_factor.to_bits(), Ordering::Relaxed);
        Ok(())
    }

    fn host_main_thread_callback(&self, window: &Window) {
        window.host_main_thread_callback();
    }

    fn state_changed(&self) {
        self.changed.store(true, Ordering::Release);
    }

    // These can arrive on the audio thread, so they only raise a flag; the
    // window diffs and sends values on its own thread.
    fn param_value_changed(&self, _id: &str, _normalized_value: f32) {
        self.changed.store(true, Ordering::Release);
    }

    fn param_modulation_changed(&self, _id: &str, _modulation_offset: f32) {}
}

struct Handler {
    window: WindowContext,
    frame: TimerHandle,
    state: RefCell<State>,
}

struct State {
    context: GuiContext,
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

impl State {
    /// Built on the first frame rather than in `spawn`: a standalone window
    /// only attaches our view to its NSWindow after `spawn` returns, and
    /// wry needs the window.
    fn build_webview(&mut self, window: &WindowContext) {
        let Some(tx) = self.tx.take() else { return };
        let builder = WebViewBuilder::new()
            .with_bounds(wry::Rect {
                position: LogicalPosition::new(0, 0).into(),
                size: WryLogicalSize::new(SIZE.0, SIZE.1).into(),
            })
            .with_accept_first_mouse(true)
            .with_devtools(cfg!(debug_assertions))
            .with_background_color((232, 230, 225, 255))
            .with_ipc_handler(move |msg| {
                let _ = tx.send(msg.into_body());
            });
        let builder = match std::env::var(DEV_URL_VAR) {
            Ok(url) => builder.with_url(&url),
            Err(_) => builder.with_html(UI_HTML),
        };
        self.webview = builder.build_as_child(window).ok();
        #[cfg(target_os = "macos")]
        if crate::app::get().is_some() {
            crate::chrome::hide_title_bar(window, SIZE);
        }
    }

    fn send(&self, messages: Vec<Value>) {
        let Some(webview) = &self.webview else {
            return;
        };
        for message in messages {
            let _ = webview.evaluate_script(&format!("window.onPluginMessage?.({message})"));
        }
    }

    #[cfg_attr(not(target_os = "macos"), allow(unused_variables))]
    fn handle(&mut self, raw: &str, window: &WindowContext) {
        let Ok(msg) = serde_json::from_str::<Value>(raw) else {
            return;
        };
        match msg["type"].as_str() {
            Some("focus") => {
                if let Some(webview) = &self.webview {
                    let _ = webview.focus();
                }
            }
            Some("keys") => self.page_has_focus = msg["active"].as_bool().unwrap_or(false),
            #[cfg(target_os = "macos")]
            Some("moveWindow") if crate::app::get().is_some() => {
                let delta = |key: &str| msg[key].as_f64().unwrap_or(0.0);
                crate::chrome::move_by(window, delta("dx"), delta("dy"));
            }
            Some(kind) => {
                self.ready |= kind == "ready";
                let mut out = Vec::new();
                let mut gui = ContextHost::new(&self.context, &self.params);
                self.bridge.handle(&msg, &mut gui, &mut out);
                self.send(out);
            }
            None => {}
        }
    }
}

impl State {
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
            crate::app::save(&self.params, self.bridge.tempo(), &ContextHost::new(&self.context, &self.params));
        }
    }
}

impl Drop for State {
    fn drop(&mut self) {
        if crate::app::get().is_some() && self.ready {
            crate::app::save(&self.params, self.bridge.tempo(), &ContextHost::new(&self.context, &self.params));
        }
    }
}

impl State {
    fn frame(&mut self, window: &WindowContext) {
        self.build_webview(window);
        #[cfg(target_os = "macos")]
        if crate::app::get().is_some() {
            crate::chrome::place_traffic_lights(window, TRAFFIC_LIGHTS.0, TRAFFIC_LIGHTS.1);
        }
        while let Ok(msg) = self.inbox.try_recv() {
            self.handle(&msg, window);
        }
        if self.ready {
            let changed = self.changed.swap(false, Ordering::AcqRel);
            let mut out = Vec::new();
            let gui = ContextHost::new(&self.context, &self.params);
            self.bridge.poll(&gui, changed, &mut out);
            self.send(out);
            self.autosave(changed);
        }
    }
}

impl WindowHandler for Handler {
    fn draw(&self) -> Result<(), HandlerError> {
        Ok(())
    }

    fn resized(&self, _new_size: WindowSize) -> Result<(), HandlerError> {
        Ok(())
    }

    fn on_timer(&self, timer: &TimerHandle) {
        if *timer == self.frame {
            self.state.borrow_mut().frame(&self.window);
        }
    }

    /// baseview's view grabs first-responder status, so keys can land here
    /// instead of in the webview. Forward them while the page has focus;
    /// otherwise let them through so host shortcuts like play keep working.
    fn on_event(&self, event: Event) -> EventStatus {
        let Event::Keyboard(e) = event else {
            return EventStatus::Ignored;
        };
        let state = self.state.borrow();
        if !state.page_has_focus {
            return EventStatus::Ignored;
        }
        state.send(vec![json!({
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
