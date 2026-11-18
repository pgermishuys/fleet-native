//! The CI walk (component 14, piece A): with `FLEET_NATIVE_WEB_DEMO=<address>` the app opens an invented page canvas whose
//! page is served at that address (`tests/fixtures/web/`), walks the steps a page tab must survive, and after each one runs
//! `WEB_DEMO_SHOT` (with `{name}`) so CI can grab the whole screen. `WEB_DEMO_CLICK` (`{x} {y}`, screen pixels) and
//! `WEB_DEMO_TYPE` (`{text}`) give it real input; without them the keyboard steps are skipped and say so.
use super::platform::{LOUD, evaluate};
use crate::State;
use crate::fleet::canvases::server_tab_id;
use rune::Tx;
use std::sync::Mutex;
use std::sync::atomic::Ordering;
use std::time::Duration;

const STEPS: [&str; 15] = [
    "1-shown",
    "2-gutter",
    "3-palette",
    "4-palette-closed",
    "5-page-keys",
    "6-composer-keys",
    "7-widened",
    "8-dialog",
    "9-dialog-closed",
    "10-toast",
    "11-panel-closed",
    "12-panel-reopened",
    "13-resized",
    "14-narrow",
    "15-narrow-menu",
];
/// The page canvas the walk shows, and the page's field it types into.
const TAB: &str = "page_walk";

/// Where the walk is.
#[derive(Debug, Default)]
pub struct Walk {
    step: usize,
    /// When the current step's picture is due, on the poll clock.
    due: f64,
    /// A picture or the input is being made on another thread.
    busy: bool,
}

/// The walk's address, when one was asked for.
pub fn asked() -> Option<String> {
    std::env::var("FLEET_NATIVE_WEB_DEMO").ok().filter(|v| !v.is_empty())
}

/// Opens the invented page in the first session's panel, in a window that fits a CI runner's screen.
pub fn start(s: &mut State, base: String, tx: &mut Tx) {
    LOUD.store(true, Ordering::Relaxed);
    s.set_open((0, 0));
    let canvas = serde_json::json!({
        "canvasId": TAB, "kind": "page", "title": "Tide planner", "version": 1,
        "state": {"pageId": "walk", "entry": "index.html", "source": "pages/tide-planner.html", "shownAt": "2026-11-17T09:00:00Z"},
    });
    // As the server would list it, so it is a tab (component 10, piece C).
    let session = s.canvas_session();
    s.canvases.set_server_canvases(&session, crate::fleet::canvases::read_server_canvas(&canvas).into_iter().collect());
    s.web.mock_base = Some(base);
    s.sidebar_open = false;
    s.open_page(TAB);
    s.web.walk = Some(Walk { step: 0, due: s.clock + 15.0, busy: false });
    tx.resize_window_instant((1000.0, 680.0));
    // A Windows runner's screen is 1024 by 768 and the window opens part off it: put it in the corner, so every grab holds all of it.
    #[cfg(windows)]
    if let Some(app) = super::platform::app() {
        app.set_position((0, 0));
    }
}

/// From `poll`: takes the step's picture once it is due.
pub fn tick(s: &mut State, tx: &mut Tx) {
    let Some(walk) = s.web.walk.as_mut() else { return };
    tx.wake_after(Duration::from_millis(100));
    if walk.busy || s.clock < walk.due {
        return;
    }
    walk.busy = true;
    let name = STEPS[walk.step];
    eprintln!("[{:6.2}] picture {name}", s.clock);
    std::thread::spawn(move || {
        run("WEB_DEMO_SHOT", &[("{name}", name.to_string())]);
        if let Some(app) = super::platform::app() {
            app.update(|s: &mut State, tx| next(s, tx));
        }
    });
}

fn next(s: &mut State, tx: &mut Tx) {
    let Some(walk) = s.web.walk.as_mut() else { return };
    walk.step += 1;
    walk.busy = false;
    walk.due = s.clock + 3.0;
    tx.wake_after(Duration::from_millis(100));
    match STEPS.get(walk.step).copied() {
        Some("2-gutter") => {
            // Where a drag of the gutter 160 points to the left lands (`RightPanel::drag`).
            s.right.user_sized = true;
            s.right.width = 520.0;
        }
        Some("3-palette") => s.ui.open = Some(crate::ui::Open::Palette),
        Some("4-palette-closed") => s.ui.open = None,
        Some("5-page-keys") => {
            walk.due += 4.0;
            type_in_page(s.web.screen.unwrap_or_default());
        }
        Some("6-composer-keys") => {
            walk.due += 4.0;
            type_in_composer(s);
        }
        Some("7-widened") => s.set_widened(true, tx),
        Some("8-dialog") => {
            s.set_widened(false, tx);
            let call = crate::fleet::actions::Call::ClearAttachments { id: "s1".into() };
            s.ask(crate::ui::confirm::Confirm::clear_attachments("tide notes".into(), call), tx);
        }
        Some("9-dialog-closed") => s.ui.confirm = None,
        Some("10-toast") => s.toast("Saved the tide table".into(), None, 20.0, tx),
        Some("11-panel-closed") => {
            s.ui.toast = None;
            s.content_open = false;
        }
        // The panel is made again, so is the page: its tick count starts from zero (the page is dropped, decision 5).
        Some("12-panel-reopened") => s.content_open = true,
        Some("13-resized") => {
            report(s);
            tx.resize_window_instant((900.0, 600.0));
        }
        // Too narrow for a third column: the panel lies over the conversation, closed until asked (`rightPanelFits`).
        Some("14-narrow") => tx.resize_window_instant((700.0, 600.0)),
        Some("15-narrow-menu") => s.ui.open = Some(crate::ui::Open::Palette),
        _ => {
            report(s);
            tx.quit();
        }
    }
}

fn run(var: &str, subs: &[(&str, String)]) {
    let Ok(mut cmd) = std::env::var(var) else { return };
    for (k, v) in subs {
        cmd = cmd.replace(k, v);
    }
    #[cfg(windows)]
    let status = {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: a console window would open over the app and into the picture.
        std::process::Command::new("cmd").args(["/C", &cmd]).creation_flags(0x0800_0000).status()
    };
    #[cfg(not(windows))]
    let status = std::process::Command::new("sh").args(["-c", &cmd]).status();
    if let Err(e) = status {
        eprintln!("{var}: {e}");
    }
}

fn real_input() -> bool {
    std::env::var("WEB_DEMO_CLICK").is_ok() && std::env::var("WEB_DEMO_TYPE").is_ok()
}

fn click_and_type([x, y]: [f32; 2], text: &str) {
    run("WEB_DEMO_CLICK", &[("{x}", format!("{x:.0}")), ("{y}", format!("{y:.0}"))]);
    std::thread::sleep(Duration::from_millis(400));
    run("WEB_DEMO_TYPE", &[("{text}", text.to_string())]);
}

fn centre([x, y, w, h]: [f32; 4]) -> [f32; 2] {
    [x + w / 2.0, y + h / 2.0]
}

/// A real click on the page's own field, found by asking the page where it is, then real keys.
fn type_in_page(screen: [f32; 4]) {
    if !real_input() {
        eprintln!("no real input on this system: the page's keyboard step is skipped");
        return;
    }
    // The callback is `Fn`; the click happens once.
    let once = Mutex::new(true);
    evaluate(&server_tab_id(TAB), "JSON.stringify(document.getElementById('note').getBoundingClientRect())", move |json| {
        if !std::mem::take(&mut *once.lock().unwrap()) {
            return;
        }
        // Some systems hand back the JSON string as a JSON string, quotes escaped.
        let flat = json.replace('\\', "");
        let num = |k: &str| flat.split(&format!("\"{k}\":")).nth(1).and_then(|r| r.split([',', '}']).next()?.parse::<f32>().ok());
        match (num("x"), num("y"), num("width"), num("height")) {
            (Some(x), Some(y), Some(w), Some(h)) => {
                let at = centre([screen[0] + x, screen[1] + y, w, h]);
                std::thread::spawn(move || click_and_type(at, "tides"));
            }
            _ => eprintln!("no rect for the page's field in {json}"),
        }
    });
}

/// A real click on the composer, then real keys: they must reach Rune, not the page.
fn type_in_composer(s: &State) {
    if !real_input() {
        eprintln!("no real input on this system: the composer's keyboard step is skipped");
        return;
    }
    let (Some(composer), Some(screen), Some(rect)) = (s.place.composer, s.web.screen, s.web.rect) else {
        eprintln!("the composer or the page hasn't been laid out");
        return;
    };
    // Window points to screen points: the page area is in both.
    let at = centre([composer[0] + screen[0] - rect[0], composer[1] + screen[1] - rect[1], composer[2], composer[3]]);
    std::thread::spawn(move || click_and_type(at, "harbour"));
}

/// Where the typed words landed, for the log.
fn report(s: &State) {
    eprintln!("composer: {:?}", s.draft.text);
    evaluate(
        &server_tab_id(TAB),
        "document.getElementById('note').value + ' | ticks ' + document.getElementById('ticks').textContent",
        |v| eprintln!("page field: {v}"),
    );
}
