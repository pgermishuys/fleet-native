//! The native side of the web view host: the window's handle, the web views (one per page tab, keyed by tab id), the
//! system's start-up (GTK on Linux, COM on Windows) and GTK's loop (rows 14.1, 14.2, 14.5, 14.6, 14.7). Everything here runs
//! on the UI thread; a web view is `!Send`, so the views sit beside the state in a thread-local rather than in it.
use super::{HOST_SCRIPT, Wanted, wanted};
use crate::State;
use rune::{AppHandle, Tx, WindowHandle};
use std::cell::RefCell;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use wry::dpi::{LogicalPosition, LogicalSize};
use wry::{Rect, WebView, WebViewBuilder};

struct View {
    tab: String,
    web: WebView,
    load_key: String,
    /// The last rectangle it had on screen: a hidden view keeps its size, so the page doesn't lay itself out again.
    rect: [f32; 4],
}

thread_local! {
    static WINDOW: RefCell<Option<WindowHandle>> = const { RefCell::new(None) };
    static VIEWS: RefCell<Vec<View>> = const { RefCell::new(Vec::new()) };
}
static APP: OnceLock<AppHandle<State>> = OnceLock::new();
/// The CI walk is running: the host says what it does on stderr.
pub(super) static LOUD: AtomicBool = AtomicBool::new(false);

fn log(what: impl FnOnce() -> String) {
    if LOUD.load(Ordering::Relaxed) {
        eprintln!("web: {}", what());
    }
}

/// The handle the web views' callbacks reach the state through.
pub fn set_app(app: AppHandle<State>) {
    let _ = APP.set(app);
}

pub(super) fn app() -> Option<&'static AppHandle<State>> {
    APP.get()
}

/// `App::on_window`: the native window exists (again, after a Wayland close or a GPU release). Starts the system's side
/// and keeps the handle; views made for an earlier window go with it.
pub fn on_window(window: &WindowHandle) {
    let ready = start_system();
    WINDOW.with(|w| *w.borrow_mut() = Some(window.clone()));
    VIEWS.with(|v| v.borrow_mut().clear());
    log(|| format!("window ready: {ready:?}"));
    if let Some(app) = APP.get() {
        app.update(move |s: &mut State, _| {
            s.web.available = ready.is_ok();
            s.web.failed = ready.err();
            s.web.applied = None;
        });
    }
}

fn start_system() -> Result<(), String> {
    // WebView2 wants a single-threaded apartment on the UI thread; S_FALSE (already one) is fine.
    #[cfg(windows)]
    // SAFETY: plain COM initialisation of the calling (UI) thread.
    let _ = unsafe { windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED) };
    #[cfg(target_os = "linux")]
    gtk::init().map_err(|e| format!("GTK didn't start: {e}"))?;
    Ok(())
}

fn bounds([x, y, w, h]: [f32; 4]) -> Rect {
    Rect { position: LogicalPosition::new(x as f64, y as f64).into(), size: LogicalSize::new(w.max(1.0) as f64, h.max(1.0) as f64).into() }
}

/// Out of the window, at its own size: hiding a WebKitGTK view and showing it again draws it black (decision 6).
fn away([_, _, w, h]: [f32; 4]) -> [f32; 4] {
    [-30_000.0, -30_000.0, w, h]
}

/// From `poll`: turn GTK, take the keyboard back if Rune has it again, and make the views follow `wanted`.
pub fn keep(s: &mut State, tx: &mut Tx) -> bool {
    super::demo::tick(s, tx);
    pump(tx);
    if !s.web.can_show() {
        return false;
    }
    take_keys_back(s, tx);
    let want = wanted(s);
    if s.web.applied.as_ref() == Some(&want) {
        return false;
    }
    let result = apply(&want);
    s.web.applied = Some(want);
    match result {
        Ok(()) => false,
        Err(e) => {
            eprintln!("fleet-native: a web view couldn't be made: {e}");
            s.web.failed = Some(e);
            true
        }
    }
}

/// WebKitGTK runs on GTK's main loop, which nothing else turns in a winit app: every 16 ms while a view exists.
fn pump(tx: &mut Tx) {
    #[cfg(target_os = "linux")]
    if VIEWS.with(|v| !v.borrow().is_empty()) {
        while gtk::events_pending() {
            gtk::main_iteration_do(false);
        }
        tx.wake_after(std::time::Duration::from_millis(16));
    }
    #[cfg(not(target_os = "linux"))]
    let _ = tx;
}

fn apply(want: &Wanted) -> Result<(), String> {
    VIEWS.with(|views| {
        let mut views = views.borrow_mut();
        if let Wanted::Shown(p) = want {
            match views.iter_mut().find(|v| v.tab == p.tab) {
                Some(v) if v.load_key != p.load_key => {
                    log(|| format!("load again {}", p.url));
                    v.web.load_url(&p.url).map_err(|e| e.to_string())?;
                    v.load_key = p.load_key.clone();
                }
                Some(_) => {}
                None => views.push(make(p)?),
            }
        }
        for v in views.iter_mut() {
            let rect = match want {
                Wanted::Shown(p) if p.tab == v.tab => {
                    v.rect = p.rect;
                    p.rect
                }
                _ => away(v.rect),
            };
            v.web.set_bounds(bounds(rect)).map_err(|e| e.to_string())?;
        }
        log(|| format!("{want:?}"));
        Ok(())
    })
}

fn make(p: &super::Placement) -> Result<View, String> {
    log(|| format!("make {} at {:?}", p.url, p.rect));
    let tab = p.tab.clone();
    let builder = WebViewBuilder::new()
        .with_url(&p.url)
        .with_bounds(bounds(p.rect))
        // `.page-canvas__frame { background: #fff }` until the page draws (row 14.10).
        .with_background_color((255, 255, 255, 255))
        // Showing a page doesn't take the keyboard from the composer; a click in it does.
        .with_focused(false)
        .with_initialization_script_for_main_only(HOST_SCRIPT, true)
        .with_ipc_handler(move |req| heard(&tab, req.body()));
    let web = WINDOW.with(|w| {
        let w = w.borrow();
        let window = w.as_ref().ok_or_else(|| "no window yet".to_string())?;
        window.with_raw_handle(|handle| builder.build_as_child(&handle).map_err(|e| e.to_string()))?
    })?;
    Ok(View { tab: p.tab.clone(), web, load_key: p.load_key.clone(), rect: p.rect })
}

/// A message from a page's host script.
fn heard(tab: &str, body: &str) {
    log(|| format!("heard from {tab}: {body}"));
    let kind = serde_json::from_str::<serde_json::Value>(body).ok().and_then(|v| v.get("host")?.as_str().map(str::to_string));
    if kind.as_deref() == Some("focus")
        && let Some(app) = APP.get()
    {
        app.update(|s: &mut State, tx| {
            // The page has the keys: Rune's own focus goes, as an iframe's click takes it from the composer in Fleet.
            if s.web.can_show() {
                s.web.page_has_keys = true;
                tx.blur();
            }
        });
    }
}

/// A Rune element took focus after the page had the keyboard: give it back to the window (decision 8). Linux moves the
/// keyboard with the click by itself, and `focus_parent` there loses it altogether (spike); Windows leaves it in WebView2.
fn take_keys_back(s: &mut State, tx: &Tx) {
    if !s.web.page_has_keys || tx.focused().is_none() {
        return;
    }
    s.web.page_has_keys = false;
    log(|| format!("keys back to {:?}", tx.focused()));
    #[cfg(not(target_os = "linux"))]
    VIEWS.with(|views| {
        for v in views.borrow().iter() {
            let _ = v.web.focus_parent();
        }
    });
}

/// Runs a script in a tab's page and hands its JSON answer back (the CI walk asks where the page's own field is).
pub fn evaluate(tab: &str, js: &str, then: impl Fn(String) + Send + 'static) {
    VIEWS.with(|views| {
        if let Some(v) = views.borrow().iter().find(|v| v.tab == tab) {
            let _ = v.web.evaluate_script_with_callback(js, then);
        }
    });
}
