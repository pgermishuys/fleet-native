//! Spike: the system web view (wry: WebView2, WKWebView, WebKitGTK) as a child of a Rune window,
//! kept exactly over a Rune panel. Ports nothing from Fleet; it answers whether this route works.
//!
//! Interactive: `cargo run`. Drag the gutter, resize the window, open the menu, click in the page.
//! Scripted (`--demo`): walks the steps the research doc shows and, after each, runs the command in
//! `SPIKE_SHOT` with `{name}` replaced, so CI can take a picture of the whole screen. With
//! `SPIKE_CLICK` set (`{x} {y}` in screen pixels), the page's button gets a real mouse click;
//! without it, the page clicks it from script.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use std::cell::{Cell, RefCell};
use std::sync::OnceLock;
use std::time::Duration;

use rune::tok::TextStyle;
use rune::{App, AppHandle, AppSpec, Color, Cursor, Drag, El, Phase, TextEdit, Tx, col, el, row, spacer, text, text_input};
use wry::dpi::{LogicalPosition, LogicalSize};
use wry::{Rect, WebView, WebViewBuilder};

const PAGE: &str = include_str!("page.html");
const LEFT_MIN: f32 = 160.0;
const LEFT_MAX: f32 = 520.0;

thread_local! {
    // The web view is !Send and lives on the UI thread, beside the state rather than in it.
    static WEB: RefCell<Option<WebView>> = const { RefCell::new(None) };
    // The panel's last rect in window points, for a web view made after the first layout.
    static RECT: Cell<[f32; 4]> = const { Cell::new([0.0; 4]) };
}
static HANDLE: OnceLock<AppHandle<State>> = OnceLock::new();

fn c(s: &str) -> Color {
    rune::color::hex(s).expect("a colour literal")
}
fn font(size: f32, weight: f32) -> TextStyle {
    TextStyle::new(size, weight).lh(size * 1.4)
}

#[derive(Default)]
pub struct State {
    left_w: f32,
    menu_open: bool,
    /// The workaround: hide the web view while a menu is open over it.
    hide_under_menu: bool,
    light_page: bool,
    /// What the page last told the app over the bridge.
    page_cards: u32,
    web_rect: [f32; 4],
    web_screen: [f32; 4],
    note: TextEdit,
    clock: f64,
    demo: Option<Demo>,
}

/// The scripted run: each step does something, waits for it to settle, then takes a picture.
#[derive(Default)]
struct Demo {
    step: usize,
    /// When the current step's picture is due, on the poll clock.
    due: f64,
    /// A picture or click is being taken on another thread.
    busy: bool,
}
const STEPS: [&str; 6] = ["1-loaded", "2-clicked", "3-gutter", "4-resized", "5-menu", "6-menu-hidden"];

fn with_web(f: impl FnOnce(&WebView)) {
    WEB.with(|w| {
        if let Some(web) = w.borrow().as_ref() {
            f(web)
        }
    });
}

fn bounds([x, y, w, h]: [f32; 4]) -> Rect {
    Rect { position: LogicalPosition::new(x as f64, y as f64).into(), size: LogicalSize::new(w.max(1.0) as f64, h.max(1.0) as f64).into() }
}

/// Where the web view goes: over the panel, unless a menu is open and the workaround is on.
fn place(s: &State) {
    let hidden = s.menu_open && s.hide_under_menu;
    with_web(|web| {
        let _ = web.set_bounds(bounds(s.web_rect));
        let _ = web.set_visible(!hidden);
    });
}

/// Runs on the UI thread when the native window exists: the web view becomes its child.
fn attach(window: &rune::WindowHandle) {
    // SAFETY: plain COM initialisation of this (the UI) thread; S_FALSE if it already was.
    #[cfg(windows)]
    let _ = unsafe { windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED) };
    #[cfg(target_os = "linux")]
    if let Err(e) = gtk::init() {
        eprintln!("gtk::init failed: {e}");
        return;
    }
    let made = window.with_raw_handle(|handle| {
        WebViewBuilder::new()
            .with_html(PAGE)
            .with_bounds(bounds(RECT.with(Cell::get)))
            .with_ipc_handler(|req| {
                let body = req.body().clone();
                let count = body.split("\"count\":").nth(1).and_then(|n| n.trim_end_matches('}').trim().parse().ok());
                if let (Some(count), Some(app)) = (count, HANDLE.get()) {
                    app.update(move |s: &mut State, _| s.page_cards = count);
                }
            })
            .build_as_child(&handle)
    });
    match made {
        Ok(Ok(web)) => WEB.with(|w| *w.borrow_mut() = Some(web)),
        Ok(Err(e)) => eprintln!("web view: {e}"),
        Err(e) => eprintln!("window handle: {e}"),
    }
}

fn button(key: &'static str, label: &str, on: bool) -> El {
    text(label)
        .key(key)
        .font(font(13.0, 500.0))
        .color(c("#e8e8ec"))
        .px(12.0)
        .py(6.0)
        .radius(8.0)
        .fill(if on { c("#6366f1") } else { c("#26262d") })
        .hover(c("#33333c"))
        .cursor(Cursor::Pointer)
}

fn view(s: &State, _: &mut rune::Cx<State>) -> El {
    let toolbar = row()
        .key("toolbar")
        .w_full()
        .center_y()
        .gap(8.0)
        .px(12.0)
        .py(8.0)
        .child(text("web-content spike").font(font(14.0, 600.0)).color(c("#e8e8ec")))
        .child(spacer())
        .child(button("menu-button", "Menu", s.menu_open).on_press(|s: &mut State| {
            s.menu_open = !s.menu_open;
            place(s);
        }))
        .child(
            button("hide-toggle", if s.hide_under_menu { "Hide under menus: on" } else { "Hide under menus: off" }, s.hide_under_menu)
                .on_press(|s: &mut State| {
                    s.hide_under_menu = !s.hide_under_menu;
                    place(s);
                }),
        )
        .child(button("theme", if s.light_page { "Page: light" } else { "Page: dark" }, false).on_press(|s: &mut State| {
            s.light_page = !s.light_page;
            let vars = if s.light_page {
                r##"{"--fleet-bg":"#ffffff","--fleet-text":"#18181b","--fleet-card":"#f4f4f5","--fleet-muted":"#71717a"}"##
            } else {
                r##"{"--fleet-bg":"#141418","--fleet-text":"#e8e8ec","--fleet-card":"#1b1b20","--fleet-muted":"#8e8e9a"}"##
            };
            with_web(|web| {
                let _ = web.evaluate_script(&format!("window.fleetTheme({vars})"));
            });
        }));

    let left = col()
        .key("left")
        .w(s.left_w)
        .h_full()
        .gap(6.0)
        .padding(12.0)
        .fill(c("#141418"))
        .radius(10.0)
        .child(text("Drawn by Rune").font(font(13.0, 600.0)).color(c("#e8e8ec")))
        .children(
            ["Harbour survey", "Ferry timetable", "Buoy maintenance", "Night shift rota"]
                .map(|name| text(name).font(font(13.0, 400.0)).color(c("#8e8e9a")).px(8.0).py(5.0).radius(6.0).hover(c("#26262d"))),
        )
        .child(
            text(format!("The page has {} cards (told over the bridge)", s.page_cards.max(1)))
                .font(font(12.0, 400.0))
                .color(c("#8e8e9a"))
                .mt(12.0),
        )
        .child(
            text_input(&s.note, font(13.0, 400.0))
                .key("note")
                .placeholder("A Rune text field", c("#71717a"))
                .w_full()
                .px(8.0)
                .py(6.0)
                .radius(8.0)
                .fill(c("#1b1b20"))
                .on_edit(|s: &mut State| &mut s.note),
        );

    let gutter = el()
        .key("gutter")
        .w(8.0)
        .h_full()
        .mx(2.0)
        .radius(4.0)
        .hover(c("#6366f1").alpha(0.3))
        .press(c("#6366f1").alpha(0.3))
        .cursor(Cursor::ResizeColumn)
        .on_drag(|s: &mut State, d: &mut Drag, _| {
            if d.phase == Phase::Down {
                d.data = s.left_w;
            }
            s.left_w = if d.cancelled() { d.data } else { (d.data + d.dx).clamp(LEFT_MIN, LEFT_MAX) };
        });

    // The panel the web view covers. What Rune draws here shows only where the web view isn't.
    let web = col()
        .key("web")
        .grow(1.0)
        .h_full()
        .center()
        .gap(6.0)
        .radius(10.0)
        .fill(c("#2a1d1d"))
        .border(1.0, c("#ef4444"))
        .child(text("Rune's panel under the web view").font(font(13.0, 500.0)).color(c("#fca5a5")))
        .child(
            text(if s.menu_open && s.hide_under_menu { "(the web view is hidden while the menu is open)" } else { "" })
                .font(font(12.0, 400.0))
                .color(c("#fca5a5")),
        )
        .on_layout(|s: &mut State, r| {
            s.web_rect = r;
            RECT.with(|c| c.set(r));
            place(s);
        })
        .on_screen_layout(|s: &mut State, r| s.web_screen = r);

    let [x, y, w, h] = s.web_rect;
    let status = text(format!("web view at x {x:.0}, y {y:.0}, {w:.0} x {h:.0} points; clock {:.1} s", s.clock))
        .font(font(12.0, 400.0))
        .color(c("#8e8e9a"))
        .px(12.0)
        .py(6.0);

    let mut root = col()
        .w_full()
        .h_full()
        .fill(c("#0d0d10"))
        .child(toolbar)
        .child(row().w_full().grow(1.0).min_h(0.0).px(8.0).child(left).child(gutter).child(web))
        .child(status);
    if s.menu_open {
        // A Rune popover straddling the gutter: its left half over Rune, its right half over the web view.
        let item = |label: &str| text(label).font(font(13.0, 400.0)).color(c("#e8e8ec")).px(10.0).py(6.0).radius(6.0).hover(c("#33333c"));
        root = root.child(
            col()
                .key("menu")
                .absolute(s.left_w - 100.0, 52.0)
                .w(260.0)
                .padding(6.0)
                .radius(10.0)
                .fill(c("#1f1f26"))
                .border(1.0, c("#3f3f46"))
                .shadow(8.0, 24.0, Color::BLACK.alpha(0.5))
                .child(item("Open in a new window"))
                .child(item("Reload the page"))
                .child(item("Copy the page's address"))
                .child(item("Show it full size"))
                .child(text("This menu is drawn by Rune.").font(font(12.0, 400.0)).color(c("#8e8e9a")).px(10.0).py(6.0)),
        );
    }
    root
}

fn poll(s: &mut State, now: f64, tx: &mut Tx) -> bool {
    s.clock = now;
    // WebKitGTK runs on GTK's main loop, which nothing else turns in a winit app.
    #[cfg(target_os = "linux")]
    {
        while gtk::events_pending() {
            gtk::main_iteration_do(false);
        }
        tx.wake_after(Duration::from_millis(16));
    }
    let Some(demo) = s.demo.as_mut() else { return false };
    tx.wake_after(Duration::from_millis(100));
    if demo.busy || now < demo.due {
        return false;
    }
    demo.busy = true;
    let name = STEPS[demo.step];
    shoot(name, now);
    false
}

/// Runs `SPIKE_SHOT` for the step on another thread; when it's done, the next step starts.
fn shoot(name: &'static str, now: f64) {
    eprintln!("[{now:6.2}] picture {name}");
    std::thread::spawn(move || {
        run_template("SPIKE_SHOT", &[("{name}", name.to_string())]);
        if let Some(app) = HANDLE.get() {
            app.update(|s: &mut State, tx| next_step(s, tx));
        }
    });
}

fn run_template(var: &str, subs: &[(&str, String)]) {
    let Ok(mut cmd) = std::env::var(var) else { return };
    for (k, v) in subs {
        cmd = cmd.replace(k, v);
    }
    let status = if cfg!(windows) {
        std::process::Command::new("cmd").args(["/C", &cmd]).status()
    } else {
        std::process::Command::new("sh").args(["-c", &cmd]).status()
    };
    if let Err(e) = status {
        eprintln!("{var}: {e}");
    }
}

fn next_step(s: &mut State, tx: &mut Tx) {
    let Some(demo) = s.demo.as_mut() else { return };
    demo.step += 1;
    demo.busy = false;
    demo.due = s.clock + 1.5;
    tx.wake_after(Duration::from_millis(100));
    match STEPS.get(demo.step).copied() {
        Some("2-clicked") => click_page_button(s.web_screen),
        Some("3-gutter") => s.left_w = 380.0,
        Some("4-resized") => tx.resize_window_instant((860.0, 560.0)),
        Some("5-menu") => s.menu_open = true,
        Some("6-menu-hidden") => s.hide_under_menu = true,
        _ => tx.quit(),
    }
    place(s);
}

/// A real click when the platform gave us a way to make one, else the page clicks itself.
fn click_page_button(web_screen: [f32; 4]) {
    if std::env::var("SPIKE_CLICK").is_err() {
        with_web(|web| {
            let _ = web.evaluate_script("document.getElementById('add').click()");
        });
        return;
    }
    with_web(|web| {
        let _ = web.evaluate_script_with_callback("JSON.stringify(document.getElementById('add').getBoundingClientRect())", move |json| {
            let num = |k: &str| json.split(&format!("\\\"{k}\\\":")).nth(1).and_then(|r| r.split([',', '}']).next()?.parse::<f32>().ok());
            let (Some(x), Some(y), Some(w), Some(h)) = (num("x"), num("y"), num("width"), num("height")) else {
                eprintln!("no button rect in {json}");
                return;
            };
            let at = [web_screen[0] + x + w / 2.0, web_screen[1] + y + h / 2.0];
            std::thread::spawn(move || run_template("SPIKE_CLICK", &[("{x}", format!("{:.0}", at[0])), ("{y}", format!("{:.0}", at[1]))]));
        });
    });
}

fn init() -> State {
    let demo = std::env::args().any(|a| a == "--demo");
    State { left_w: 240.0, demo: demo.then(|| Demo { due: 5.0, ..Demo::default() }), ..State::default() }
}

fn main() {
    let spec =
        AppSpec { poll: Some(poll), min_size: Some((640.0, 420.0)), ..AppSpec::new("web-content spike", (1000.0, 660.0), init, view) };
    let app = App::new(spec).on_window(attach);
    let _ = HANDLE.set(app.handle());
    app.run();
}
