//! Second spike: Chromium (CEF) rendering off screen into a `rune::Pixels`, so the page is just
//! another picture in Rune's tree: it scrolls with a conversation, clips to its rounded corners and
//! sits under a Rune menu. Build with `--features osr --bin osr`; CEF's binaries come from
//! `CEF_PATH` (see the research doc).
//!
//! `--demo` clicks the page's button, scrolls the list and opens the menu, running `SPIKE_SHOT`
//! after each, as the wry spike does.

use std::cell::{Cell, RefCell};
use std::sync::OnceLock;
use std::time::Duration;

use cef::rc::Rc as _;
use cef::*;
use rune::tok::TextStyle;
use rune::{AppHandle, AppSpec, Color, Drag, El, Phase, Pixels, ScrollSpec, Tx, col, row, spacer, text};

const PAGE: &str = include_str!("page.html");
/// The page's box in the conversation, in points (Fleet sizes inline pages to their content).
const PAGE_H: f32 = 300.0;

thread_local! {
    static BROWSER: RefCell<Option<Browser>> = const { RefCell::new(None) };
    /// The page's box in points, which CEF asks for in `view_rect`.
    static VIEW: Cell<(i32, i32)> = const { Cell::new((600, 300)) };
}
static FRAME: OnceLock<Pixels> = OnceLock::new();
static HANDLE: OnceLock<AppHandle<State>> = OnceLock::new();

fn c(s: &str) -> Color {
    rune::color::hex(s).expect("a colour literal")
}
fn font(size: f32, weight: f32) -> TextStyle {
    TextStyle::new(size, weight).lh(size * 1.4)
}

#[derive(Default)]
pub struct State {
    menu_open: bool,
    frames: u32,
    clock: f64,
    demo: Option<(usize, f64, bool)>,
}
const STEPS: [&str; 4] = ["osr-1-loaded", "osr-2-clicked", "osr-3-scrolled", "osr-4-menu"];

#[derive(Clone)]
struct Paint;

wrap_render_handler! {
    struct PaintBuilder { handler: Paint }
    impl RenderHandler {
        fn view_rect(&self, _browser: Option<&mut Browser>, rect: Option<&mut Rect>) {
            if let Some(rect) = rect {
                let (w, h) = VIEW.with(Cell::get);
                rect.width = w.max(1);
                rect.height = h.max(1);
            }
        }
        fn on_paint(&self, _browser: Option<&mut Browser>, _type_: PaintElementType, _dirty: Option<&[Rect]>,
                    buffer: *const u8, width: ::std::os::raw::c_int, height: ::std::os::raw::c_int) {
            if buffer.is_null() || width <= 0 || height <= 0 {
                return;
            }
            // SAFETY: CEF hands a width x height BGRA buffer that lives for this call.
            let bgra = unsafe { std::slice::from_raw_parts(buffer, (width * height * 4) as usize) };
            let mut rgba = bgra.to_vec();
            for px in rgba.chunks_exact_mut(4) {
                px.swap(0, 2);
            }
            FRAME.get_or_init(Pixels::new).set(width as u32, height as u32, rgba);
            if let Some(app) = HANDLE.get() {
                app.update(|s: &mut State, _| s.frames += 1);
            }
        }
    }
}

wrap_client! {
    struct ClientBuilder { render: RenderHandler }
    impl Client {
        fn render_handler(&self) -> Option<RenderHandler> { Some(self.render.clone()) }
    }
}

wrap_app! {
    struct CefAppBuilder { _n: u8 }
    impl App {
        fn on_before_command_line_processing(&self, _type: Option<&CefStringUtf16>, line: Option<&mut CommandLine>) {
            if let Some(line) = line {
                // No GPU on the test machine; software compositing paints into on_paint all the same.
                line.append_switch(Some(&"disable-gpu".into()));
                line.append_switch(Some(&"disable-gpu-compositing".into()));
            }
        }
    }
}

fn host() -> Option<BrowserHost> {
    BROWSER.with(|b| b.borrow().as_ref().and_then(|b| b.host()))
}

fn mouse(x: f32, y: f32) -> MouseEvent {
    MouseEvent { x: x as i32, y: y as i32, modifiers: 0 }
}

fn open_browser() {
    let dir = std::env::temp_dir().join("web-content-osr");
    let _ = std::fs::create_dir_all(&dir);
    let page = dir.join("page.html");
    // The page calls window.ipc, which only wry provides; CEF's bridge would be a message router.
    let _ = std::fs::write(&page, PAGE.replace("window.ipc.postMessage", "window.ipc?.postMessage"));
    let info = WindowInfo { windowless_rendering_enabled: 1, ..Default::default() };
    let settings = BrowserSettings { windowless_frame_rate: 60, ..Default::default() };
    let url = format!("file://{}", page.display());
    let mut client = ClientBuilder::new(PaintBuilder::new(Paint));
    let browser = browser_host_create_browser_sync(Some(&info), Some(&mut client), Some(&url.as_str().into()), Some(&settings), None, None);
    BROWSER.with(|b| *b.borrow_mut() = browser);
}

fn view(s: &State, _: &mut rune::Cx<State>) -> El {
    let frame = FRAME.get_or_init(Pixels::new);
    let bubble = |who: &str, words: &str| {
        col()
            .w_full()
            .gap(4.0)
            .child(text(who).font(font(12.0, 600.0)).color(c("#8e8e9a")))
            .child(text(words).font(font(14.0, 400.0)).color(c("#e8e8ec")))
    };
    // The page is a picture in the conversation: it scrolls, clips and sits under menus like any other.
    let page = rune::el()
        .key("page")
        .w_full()
        .h(PAGE_H)
        .radius(12.0)
        .border(1.0, c("#3f3f46"))
        .fill(c("#141418"))
        .pixels(frame)
        .on_layout(|_: &mut State, [_, _, w, h]| {
            VIEW.with(|v| v.set((w as i32, h as i32)));
            if let Some(host) = host() {
                host.was_resized();
            }
        })
        .on_drag(|_: &mut State, d: &mut Drag, _| {
            let Some(host) = host() else { return };
            let ev = mouse(d.x, d.y);
            match d.phase {
                Phase::Down => {
                    host.set_focus(1);
                    host.send_mouse_click_event(Some(&ev), MouseButtonType::default(), 0, 1);
                }
                Phase::Move => host.send_mouse_move_event(Some(&ev), 0),
                Phase::Up => host.send_mouse_click_event(Some(&ev), MouseButtonType::default(), 1, 1),
            }
        });
    let mut list = col().w_full().gap(18.0).px(32.0).py(24.0);
    for i in 0..3 {
        list = list.child(bubble("You", &format!("Question {}: when is the next spring tide at the harbour mouth?", i + 1)));
        list = list.child(bubble("Agent", "Here is the planner as a page. It runs its own script; the cards come from the button."));
    }
    list = list.child(page);
    for i in 0..4 {
        list = list.child(bubble("Agent", &format!("Follow-up note {}: the page above is CEF painting into a Rune picture.", i + 1)));
    }
    let toolbar = row()
        .w_full()
        .center_y()
        .px(12.0)
        .py(8.0)
        .child(text("web-content spike: CEF off screen").font(font(14.0, 600.0)).color(c("#e8e8ec")))
        .child(spacer())
        .child(text(format!("{} frames from CEF", s.frames)).font(font(12.0, 400.0)).color(c("#8e8e9a")).px(8.0))
        .child(
            text("Menu")
                .key("menu-button")
                .font(font(13.0, 500.0))
                .color(c("#e8e8ec"))
                .px(12.0)
                .py(6.0)
                .radius(8.0)
                .fill(c("#26262d"))
                .on_press(|s: &mut State| s.menu_open = !s.menu_open),
        );
    let mut root =
        col().w_full().h_full().fill(c("#0d0d10")).child(toolbar).child(
            col().key("stream").w_full().grow(1.0).min_h(0.0).fill(c("#101014")).scroll(ScrollSpec { stick_bottom: false }).child(list),
        );
    if s.menu_open {
        let item = |l: &str| text(l).font(font(13.0, 400.0)).color(c("#e8e8ec")).px(10.0).py(6.0).radius(6.0);
        root = root.child(
            col()
                .key("menu")
                .pin(None, Some(48.0), Some(12.0), None)
                .w(280.0)
                .padding(6.0)
                .radius(10.0)
                .fill(c("#1f1f26"))
                .border(1.0, c("#3f3f46"))
                .shadow(8.0, 24.0, Color::BLACK.alpha(0.5))
                .child(item("Open in a new window"))
                .child(item("Reload the page"))
                .child(item("Show it full size"))
                .child(text("Drawn by Rune, over the page.").font(font(12.0, 400.0)).color(c("#8e8e9a")).px(10.0).py(6.0)),
        );
    }
    root
}

fn poll(s: &mut State, now: f64, tx: &mut Tx) -> bool {
    s.clock = now;
    if BROWSER.with(|b| b.borrow().is_none()) {
        open_browser();
    }
    do_message_loop_work();
    tx.wake_after(Duration::from_millis(16));
    let Some((step, due, busy)) = s.demo.as_mut() else { return false };
    if *busy || now < *due {
        return false;
    }
    *busy = true;
    let name = STEPS[*step];
    eprintln!("[{now:6.2}] picture {name}");
    std::thread::spawn(move || {
        if let Ok(cmd) = std::env::var("SPIKE_SHOT") {
            let _ = std::process::Command::new("sh").args(["-c", &cmd.replace("{name}", name)]).status();
        }
        if let Some(app) = HANDLE.get() {
            app.update(next_step);
        }
    });
    false
}

fn next_step(s: &mut State, tx: &mut Tx) {
    let Some((step, due, busy)) = s.demo.as_mut() else { return };
    *step += 1;
    *busy = false;
    *due = s.clock + 1.5;
    tx.wake_after(Duration::from_millis(100));
    match STEPS.get(*step).copied() {
        // Where the page's button sits in the page (page.html: 20px padding, heading, one line).
        Some("osr-2-clicked") => {
            if let Some(host) = host() {
                let ev = mouse(80.0, 82.0);
                host.send_mouse_click_event(Some(&ev), MouseButtonType::default(), 0, 1);
                host.send_mouse_click_event(Some(&ev), MouseButtonType::default(), 1, 1);
            }
        }
        Some("osr-3-scrolled") => tx.scroll_to_bottom("stream"),
        Some("osr-4-menu") => s.menu_open = true,
        _ => tx.quit(),
    }
}

fn init() -> State {
    let demo = std::env::args().any(|a| a == "--demo");
    State { demo: demo.then_some((0, 6.0, false)), ..State::default() }
}

fn main() {
    let _ = api_hash(sys::CEF_API_VERSION_LAST, 0);
    let args = Args::new();
    let mut app = CefAppBuilder::new(0);
    // CEF starts its renderer and GPU helpers as copies of this executable; they stop here.
    let code = execute_process(Some(args.as_main_args()), Some(&mut app), std::ptr::null_mut());
    if code >= 0 {
        std::process::exit(code);
    }
    let cef_dir = std::env::var("CEF_PATH").unwrap_or_default();
    let settings = Settings {
        no_sandbox: 1,
        windowless_rendering_enabled: 1,
        external_message_pump: 1,
        resources_dir_path: cef_dir.as_str().into(),
        locales_dir_path: format!("{cef_dir}/locales").as_str().into(),
        root_cache_path: std::env::temp_dir().join("web-content-osr/cache").to_string_lossy().as_ref().into(),
        ..Default::default()
    };
    assert_eq!(initialize(Some(args.as_main_args()), Some(&settings), Some(&mut app), std::ptr::null_mut()), 1, "CEF didn't start");
    let spec = AppSpec { poll: Some(poll), ..AppSpec::new("web-content spike (CEF)", (900.0, 640.0), init, view) };
    let app = rune::App::new(spec);
    let _ = HANDLE.set(app.handle());
    app.run();
    BROWSER.with(|b| b.borrow_mut().take());
    shutdown();
}
