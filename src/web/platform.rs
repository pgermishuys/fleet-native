//! The native side of the web view host: the window's handle, the web views (one per page tab, keyed by tab id), the
//! system's start-up (GTK on Linux, COM on Windows) and GTK's loop (rows 14.1, 14.2, 14.5, 14.6, 14.7). Everything here runs
//! on the UI thread; a web view is `!Send`, so the views sit beside the state in a thread-local rather than in it.
use super::page_theme::PagePlace;
use super::{Wanted, inject, wanted};
use crate::State;
use rune::{AppHandle, Tx, WindowHandle};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use wry::dpi::{LogicalPosition, LogicalSize};
use wry::{Rect, WebView, WebViewBuilder};

struct View {
    tab: String,
    place: PagePlace,
    /// A browser tab's preview (row 14.54): no theme goes to it.
    browser: bool,
    web: WebView,
    load_key: String,
    /// The last rectangle it had on screen: a hidden view keeps its size, so the page doesn't lay itself out again.
    rect: [f32; 4],
    /// Linux only: WebKitGTK's view is black until it has drawn, so it waits out of the window until the page has loaded (row
    /// 14.10); the area under it is the white of `Theme::page_frame` meanwhile. Elsewhere a view is revealed at once.
    revealed: bool,
    loaded: Rc<Cell<bool>>,
    born: std::time::Instant,
}

/// A page that never reports it has loaded is shown anyway after this long.
const REVEAL_AFTER: std::time::Duration = std::time::Duration::from_secs(6);

impl View {
    /// Becomes true once, when the view may come into the window.
    fn reveal_now(&mut self) -> bool {
        if self.revealed || !(self.loaded.get() || self.born.elapsed() > REVEAL_AFTER) {
            return false;
        }
        self.revealed = true;
        true
    }
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
    for request in std::mem::take(&mut s.web.nav) {
        evaluate(&request.tab, request.script, |_| {});
    }
    let theme = s.page_theme();
    if s.web.theme.as_ref() != Some(&theme) {
        // A change of theme (or of the font size) reaches every page that's open (row 14.28).
        if s.web.theme.is_some() {
            retheme_all(&theme);
        }
        s.web.theme = Some(theme);
    }
    let want = wanted(s);
    // Every view is looked at (no short circuit): each one decides for itself.
    #[allow(clippy::unnecessary_fold)]
    let revealed = VIEWS.with(|v| v.borrow_mut().iter_mut().fold(false, |any, view| view.reveal_now() || any));
    if s.clock < s.web.settle_until {
        // Waiting for the panel to finish easing (Widen): look again when it has.
        tx.wake_after(std::time::Duration::from_millis(40));
    }
    if !revealed && s.web.applied.as_ref() == Some(&want) {
        return false;
    }
    let before = VIEWS.with(|v| v.borrow().len());
    let result = apply(&want, s.web.theme.as_ref().expect("set above"));
    s.web.applied = Some(want);
    match result {
        // A view that went leaves its pixels on screen until Rune draws over them (an error shown where the page was).
        Ok(()) => {
            let gone = VIEWS.with(|v| v.borrow().len()) < before;
            if gone {
                // The last view's window is destroyed by GTK's loop, which `pump` only turns while a view exists.
                drain_gtk();
            }
            gone
        }
        Err(e) => {
            eprintln!("fleet-native: a web view couldn't be made: {e}");
            s.web.failed = Some(e);
            true
        }
    }
}

fn drain_gtk() {
    #[cfg(target_os = "linux")]
    while gtk::events_pending() {
        gtk::main_iteration_do(false);
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

fn apply(want: &Wanted, theme: &super::page_theme::PageTheme) -> Result<(), String> {
    VIEWS.with(|views| {
        let mut views = views.borrow_mut();
        // A page tab's view lives only while its tab is the one on screen, hidden or not; another tab, another route, a closed
        // panel or another session drop it (decision 5, row 14.14). What the page saved comes back through `fleet:page-hello`.
        let before = views.len();
        views.retain(|v| want.tab() == Some(v.tab.as_str()));
        if views.len() != before {
            log(|| format!("dropped {} view(s)", before - views.len()));
        }
        if let Wanted::Shown(p) = want {
            match views.iter_mut().find(|v| v.tab == p.tab) {
                Some(v) if v.load_key != p.load_key => {
                    log(|| format!("load again {}", p.url));
                    match &p.html {
                        Some(html) => v.web.load_html(html),
                        None => v.web.load_url(&p.url),
                    }
                    .map_err(|e| e.to_string())?;
                    v.load_key = p.load_key.clone();
                }
                Some(_) => {}
                None => views.push(make(p, &theme.frame_name(p.place))?),
            }
        }
        for v in views.iter_mut() {
            let rect = match want {
                Wanted::Shown(p) if p.tab == v.tab => {
                    v.rect = p.rect;
                    if v.revealed { p.rect } else { away(p.rect) }
                }
                _ => away(v.rect),
            };
            v.web.set_bounds(bounds(rect)).map_err(|e| e.to_string())?;
        }
        log(|| format!("{want:?}"));
        Ok(())
    })
}

fn make(p: &super::Placement, name: &str) -> Result<View, String> {
    log(|| format!("make {} at {:?}", p.url, p.rect));
    let tab = p.tab.clone();
    let loaded = Rc::new(Cell::new(false));
    let done = loaded.clone();
    let revealed = !cfg!(target_os = "linux");
    let builder = WebViewBuilder::new();
    let builder = match &p.html {
        // A document of the agent's: no scripts (its policy is in the text), and a link goes to the system's browser.
        Some(html) => builder.with_html(html).with_navigation_handler(|url| {
            let own = url.starts_with("about:") || url.starts_with("data:") || url.starts_with("blob:");
            if !own {
                open_document_link(url);
            }
            own
        }),
        None => builder.with_url(&p.url),
    };
    let builder = builder
        .with_bounds(bounds(if revealed { p.rect } else { away(p.rect) }))
        .with_on_page_load_handler(move |event, _| {
            if matches!(event, wry::PageLoadEvent::Finished) {
                done.set(true);
            }
        })
        // `allow-popups` and `allow-downloads` go to the system browser, which has the user's downloads folder and windows
        // (decision 12). The window is refused; a download is cancelled and the same address opens there.
        .with_new_window_req_handler(|url, _| {
            open_document_link(url);
            wry::NewWindowResponse::Deny
        })
        .with_download_started_handler(|url, _| {
            open_document_link(url);
            false
        })
        // The inspector is for debug builds only (row 14.15); the page's own context menu is the system's.
        .with_devtools(cfg!(debug_assertions))
        // `.page-canvas__frame { background: #fff }` until the page draws (row 14.10).
        .with_background_color((255, 255, 255, 255))
        // Showing a page doesn't take the keyboard from the composer; a click in it does.
        .with_focused(false)
        .with_initialization_script_for_main_only(if p.browser { inject::preview_script().to_string() } else { inject::script(name) }, true)
        .with_ipc_handler(move |req| heard(&tab, req.body()));
    let web = WINDOW.with(|w| {
        let w = w.borrow();
        let window = w.as_ref().ok_or_else(|| "no window yet".to_string())?;
        window.with_raw_handle(|handle| builder.build_as_child(&handle).map_err(|e| e.to_string()))?
    })?;
    Ok(View {
        tab: p.tab.clone(),
        place: p.place,
        browser: p.browser,
        web,
        load_key: p.load_key.clone(),
        rect: p.rect,
        revealed,
        loaded,
        born: std::time::Instant::now(),
    })
}

/// A link in an agent's document goes to the system's browser; the document stays as it was.
fn open_document_link(url: String) {
    log(|| format!("outside: {url}"));
    if let Some(app) = APP.get() {
        app.update(move |_: &mut State, tx| {
            let _ = tx.open_url(&url);
        });
    }
}

fn retheme_all(theme: &super::page_theme::PageTheme) {
    let message = theme.message();
    VIEWS.with(|views| {
        for v in views.borrow().iter().filter(|v| !v.browser) {
            let _ = v.web.evaluate_script(&inject::retheme(&theme.frame_name(v.place), &message));
        }
    });
}

/// A message from a page's host script: that it took the keyboard, or a Fleet message from the page.
fn heard(tab: &str, body: &str) {
    log(|| format!("heard from {tab}: {body}"));
    let parsed = serde_json::from_str::<serde_json::Value>(body).ok();
    if let Some(data) = parsed.as_ref().and_then(|v| v.get("preview")).cloned() {
        let tab = tab.to_string();
        if let Some(app) = APP.get() {
            app.update(move |s: &mut State, _| s.hear_preview(&tab, &data));
        }
        return;
    }
    if let Some(data) = parsed.as_ref().and_then(|v| v.get("page")).cloned() {
        let tab = tab.to_string();
        if let Some(app) = APP.get() {
            app.update(move |s: &mut State, tx| {
                if let Some(reply) = s.hear_page(&tab, &data, tx) {
                    evaluate(&tab, &inject::deliver(&reply), |_| {});
                }
            });
        }
        return;
    }
    let kind = parsed.as_ref().and_then(|v| v.get("host")?.as_str().map(str::to_string));
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
