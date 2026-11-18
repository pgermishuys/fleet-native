//! The browser tab's app: panel, strip and output (component 14, piece I). The four `BrowserCanvas.test.ts` cases of this piece
//! under their names, and ours for the output's polling, a refused Stop, the port select and the return from a restart.
//! Requests go to a fake Fleet that records them; a test waits for the request it means and counts only those.
use crate::fleet::app_runs::AppStatus;
use crate::fleet::events::{Event, Kind};
use crate::fleet::state::Topic;
use crate::mock::browser_tab::{BrowserCase, PAGE, PREVIEW, TAB};
use crate::tests::{FRAME, click};
use crate::tests_archive_pins::{Fake, Rig};
use crate::web::browser_app::{Panel, is_live, panel_of, panel_title, status_label};
use serde_json::json;

const APP: &str = r#"{"id":"app_1","command":"npm run dev","status":"starting","ports":[]}"#;

fn fleet(output: &'static [&'static str]) -> Fake {
    Fake::start(move |method, path| match (method, path) {
        (_, p) if p.ends_with("/browser/proxy") => (200, json!({"origin": PREVIEW}).to_string()),
        ("POST", p) if p.ends_with("/restart") => (200, APP.into()),
        ("GET", p) if p.contains("/output") => (200, json!({"lines": output, "next": 99}).to_string()),
        _ => (200, "{}".into()),
    })
}

fn event(r: &mut Rig, status: &str, url: Option<&str>) {
    let payload = json!({"sessionId": "mock.0.0", "appId": "app_1", "command": "npm run dev", "status": status, "url": url,
        "ports": [5173, 5174], "exitCode": null, "reason": null});
    r.rt.update(move |s, _| s.dispatch(&Topic::Session("mock.0.0".into()), &[Event::Other(Kind::AppUpdated, payload)]));
    r.rt.frame(FRAME);
}

fn show(fake: &Fake, case: BrowserCase) -> Rig {
    let mut r = Rig::home(fake);
    r.rt.update(move |s, _| s.show_browser_tab(case));
    r.rt.frame(FRAME);
    r.rt.frame(FRAME);
    r
}

fn words(r: &Rig) -> String {
    r.rt.elements().into_iter().filter_map(|e| e.text.or(e.label)).collect::<Vec<_>>().join("\n")
}

fn outputs(fake: &Fake) -> usize {
    fake.calls("/output").len()
}

// "offers Start for a stopped app, and shows the current run's output while it starts"
#[test]
fn offers_start_for_a_stopped_app_and_shows_the_current_runs_output_while_it_starts() {
    let fake = fleet(&[]);
    let mut r = show(&fake, BrowserCase::AppStopped);
    assert!(words(&r).contains("The app is stopped"));
    assert!(crate::web::wanted::shown_browser(r.state()).is_none(), "no web view under the panel");
    click(&mut r.rt, "browser-panel-start");
    r.wait("Fleet's restart", |_| !fake.calls("/apps/app_1/restart").is_empty());
    assert_eq!(fake.calls("/apps/app_1/restart")[0].0, "POST");
    r.wait("the app starting", |s| s.apps.app("app_1").is_some_and(|a| a.status == AppStatus::Starting));
    r.rt.frame(FRAME);
    let text = words(&r);
    assert!(text.contains("Starting"), "{text}");
    assert!(text.contains("> vite"), "{text}");
    assert!(!text.contains("crashed"), "an earlier run's crash doesn't belong under Starting: {text}");
}

// "shows why an app exited and its last output"
#[test]
fn shows_why_an_app_exited_and_its_last_output() {
    let fake = fleet(&[]);
    let r = show(&fake, BrowserCase::AppExited);
    let text = words(&r);
    assert!(text.contains("The app exited with code 1"), "{text}");
    assert!(text.contains("Cannot find module"), "{text}");
    assert!(!text.contains("old run"), "only the current run: {text}");
    assert!(text.contains("exited (1)"), "the strip says it too: {text}");
}

// "opens the output when a build fails"
#[test]
fn opens_the_output_when_a_build_fails() {
    let fake = fleet(&["[vite] Internal server error: Unexpected token"]);
    let mut r = show(&fake, BrowserCase::AppRunning);
    assert!(r.rt.find("browser-output").is_none());
    event(&mut r, "build-failed", Some(PAGE));
    r.wait("the output drawer, with Fleet's lines", |s| s.apps.output("app_1").is_some_and(|o| !o.lines.is_empty()));
    r.rt.frame(FRAME);
    r.rt.frame(FRAME);
    let text = words(&r);
    assert!(text.contains("build failed"), "{text}");
    assert!(text.contains("Internal server error"), "{text}");
    // Once: closing it stays closed while the build stays failed.
    click(&mut r.rt, "browser-output-toggle");
    event(&mut r, "build-failed", Some(PAGE));
    assert!(r.rt.find("browser-output").is_none());
}

// "follows its app's page when the tab has none of its own (started from the + menu)"
#[test]
fn follows_its_apps_page_when_the_tab_has_none_of_its_own() {
    let fake = fleet(&[]);
    let mut r = show(&fake, BrowserCase::Waiting);
    event(&mut r, "starting", None);
    assert!(words(&r).contains("Starting"));
    assert!(fake.calls("/browser/proxy").is_empty());
    event(&mut r, "running", Some("http://localhost:5173/"));
    r.wait("the proxy question for the app's page", |_| !fake.calls("/browser/proxy").is_empty());
    assert_eq!(fake.calls("/browser/proxy")[0].2, json!({"url": "http://localhost:5173/"}));
    r.wait("the page's preview", |s| s.browser_ui(TAB).is_some_and(|u| u.frame_src.is_some()));
    assert_eq!(r.state().browser_ui(TAB).unwrap().frame_src.as_deref(), Some(format!("{PREVIEW}/").as_str()));
}

#[test]
fn each_status_has_its_panel_title_and_strip_word() {
    assert_eq!(panel_of(Some(AppStatus::Starting), true), Some(Panel::Starting));
    assert_eq!(panel_of(Some(AppStatus::Exited), true), Some(Panel::Exited));
    assert_eq!(panel_of(Some(AppStatus::Stopped), false), Some(Panel::Stopped));
    assert_eq!(panel_of(Some(AppStatus::Running), false), Some(Panel::Waiting));
    assert_eq!(panel_of(Some(AppStatus::Running), true), None);
    assert_eq!(panel_of(Some(AppStatus::BuildFailed), true), None, "a failed build still shows its page");
    assert_eq!(panel_of(None, false), Some(Panel::Waiting));
    assert!(is_live(Some(AppStatus::BuildFailed)) && !is_live(Some(AppStatus::Exited)) && !is_live(None));
    let run = |status, code| crate::fleet::app_runs::AppRun {
        id: "a".into(),
        session_id: "s".into(),
        command: "c".into(),
        status,
        exit_code: code,
        url: None,
        ports: vec![],
        printed_urls: vec![],
        reason: None,
    };
    assert_eq!(panel_title(Panel::Exited, Some(&run(AppStatus::Exited, Some(2))), true), "The app exited with code 2");
    assert_eq!(panel_title(Panel::Exited, Some(&run(AppStatus::Exited, None)), true), "The app exited");
    assert_eq!(panel_title(Panel::Waiting, None, true), "Waiting for the app to serve a page\u{2026}");
    assert_eq!(panel_title(Panel::Waiting, None, false), "Loading\u{2026}");
    assert_eq!(status_label(&run(AppStatus::Exited, Some(1))), "exited (1)");
    assert_eq!(status_label(&run(AppStatus::Exited, None)), "exited");
    assert_eq!(status_label(&run(AppStatus::BuildFailed, None)), "build failed");
}

#[test]
fn the_strip_offers_restart_and_stop_while_live_and_start_when_not() {
    let fake = fleet(&[]);
    let r = show(&fake, BrowserCase::AppRunning);
    assert!(r.rt.find("browser-restart").is_some() && r.rt.find("browser-stop").is_some() && r.rt.find("browser-output-toggle").is_some());
    assert_eq!(
        r.rt.elements().into_iter().find(|e| e.key.as_deref() == Some("browser-restart")).and_then(|e| e.label).as_deref(),
        Some("Restart")
    );
    let r = show(&fake, BrowserCase::AppStopped);
    assert!(r.rt.find("browser-stop").is_none());
    assert_eq!(
        r.rt.elements().into_iter().find(|e| e.key.as_deref() == Some("browser-restart")).and_then(|e| e.label).as_deref(),
        Some("Start")
    );
    assert!(r.rt.find("browser-strip").is_some());
}

#[test]
fn the_strip_loses_its_labels_under_440_px() {
    let fake = fleet(&[]);
    let r = show(&fake, BrowserCase::AppRunning);
    let (_, _, width, _) =
        r.rt.elements()
            .into_iter()
            .find(|e| e.key.as_deref() == Some("browser-tab"))
            .map(|e| (e.rect[0], e.rect[1], e.rect[2], e.rect[3]))
            .unwrap();
    let button = r.rt.elements().into_iter().find(|e| e.key.as_deref() == Some("browser-stop")).unwrap();
    assert_eq!(button.rect[2] > 26.5, width > 440.0, "tab {width} wide, button {} wide", button.rect[2]);
}

#[test]
fn output_is_asked_every_second_while_it_is_seen_and_the_app_runs() {
    let fake = fleet(&["listening"]);
    let mut r = show(&fake, BrowserCase::AppRunning);
    assert_eq!(outputs(&fake), 0, "the drawer is closed and the panel isn't up");
    click(&mut r.rt, "browser-output-toggle");
    r.wait("the first output request", |_| outputs(&fake) >= 1);
    r.wait("its lines", |s| s.apps.output("app_1").is_some_and(|o| o.lines == ["listening"]));
    let asked = outputs(&fake);
    r.rt.frame(1.2);
    r.wait("the next second's request", |_| outputs(&fake) > asked);
    // Closed, nothing is wanted.
    click(&mut r.rt, "browser-output-toggle");
    r.rt.frame(1.2);
    assert_eq!(r.state().browser_ui(TAB).unwrap().app.out_key, None);
}

#[test]
fn output_of_an_exited_app_is_asked_once() {
    let fake = fleet(&["Error: Cannot find module './routes'"]);
    let mut r = show(&fake, BrowserCase::AppExited);
    r.wait("the one output request", |_| outputs(&fake) >= 1);
    assert_eq!(r.state().browser_ui(TAB).unwrap().app.out_key, Some(false));
    let asked = outputs(&fake);
    for _ in 0..3 {
        r.rt.frame(1.2);
    }
    assert_eq!(outputs(&fake), asked, "not live: once");
}

#[test]
fn a_refused_stop_shows_for_ten_seconds() {
    let fake = Fake::start(|method, path| match (method, path) {
        ("POST", p) if p.ends_with("/stop") => (409, json!({"error": "It isn't running."}).to_string()),
        _ => (200, "{}".into()),
    });
    let mut r = show(&fake, BrowserCase::AppRunning);
    click(&mut r.rt, "browser-stop");
    r.wait("the refusal", |s| s.browser_ui(TAB).is_some_and(|u| u.app.action_error.is_some()));
    r.rt.frame(FRAME);
    assert!(words(&r).contains("It isn't running."));
    assert!(!r.state().browser_ui(TAB).unwrap().app.busy, "the buttons are back");
    r.rt.frame(9.0);
    assert!(r.rt.find("browser-action-error").is_some(), "still there at nine seconds");
    r.rt.frame(2.0);
    assert!(r.rt.find("browser-action-error").is_none(), "gone after ten");
}

#[test]
fn the_port_select_is_for_an_app_with_more_than_one_port_and_shows_that_port() {
    let fake = fleet(&[]);
    let mut r = show(&fake, BrowserCase::AppRunning);
    assert!(r.rt.find("browser-port").is_some());
    assert!(words(&r).contains(":5173"));
    click(&mut r.rt, "browser-port");
    assert!(words(&r).contains(":5174"));
    click(&mut r.rt, "browser-port-list-opt1");
    r.wait("the other port asked of Fleet", |_| {
        fake.calls("/browser/proxy").iter().any(|c| c.2 == json!({"url": "http://localhost:5174/"}))
    });
    r.wait("its preview", |s| s.browser_ui(TAB).is_some_and(|u| u.text() == "http://localhost:5174/"));
    // One port: no select.
    r.rt.update(|s, _| {
        s.apps.apply_event(
            &json!({"appId": "app_1", "sessionId": "mock.0.0", "status": "running", "command": "npm run dev", "ports": [5173]}),
        );
    });
    r.rt.frame(FRAME);
    assert!(r.rt.find("browser-port").is_none());
}

#[test]
fn back_from_a_restart_shows_its_page_again_follows_a_new_port_and_pulses() {
    let fake = fleet(&[]);
    let mut r = show(&fake, BrowserCase::AppRunning);
    let hello = json!({"fleet": 1, "type": "hello", "hmr": "vite", "href": format!("{PREVIEW}/")});
    r.rt.update(move |s, _| s.hear_preview(TAB, &hello));
    event(&mut r, "exited", None);
    assert!(words(&r).contains("The app exited"));
    r.rt.update(|s, _| s.clock = 50.0);
    event(&mut r, "running", Some("http://localhost:5199/"));
    assert_eq!(r.state().canvases.updated_at(TAB), Some(50.0), "back, so the tab pulses");
    r.wait("the page on its new port", |s| s.browser_ui(TAB).is_some_and(|u| u.text() == "http://localhost:5199/"));
    assert!(r.state().browser_ui(TAB).unwrap().expecting_load, "the load that follows isn't the page reloading itself");
}

#[test]
fn the_app_loads_when_its_tab_opens() {
    let fake = fleet(&[]);
    let mut r = show(&fake, BrowserCase::AppRunning);
    r.wait("the app read when the tab opens", |_| fake.calls("/apps/app_1").iter().any(|c| c.0 == "GET"));
}
