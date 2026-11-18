//! The + menu's Browser dialog (component 14, piece J; rows 14.70 to 14.72). `BrowserOpenDialog` has no Fleet test, so these are
//! ours, read from its rules: the hint for each target, the suggestions' order, Run and Open, and an error that keeps the dialog.
//! Requests go to a fake Fleet that records them; a test waits for the request it means.
use crate::conv::browser::BrowserTarget;
use crate::fleet::app_runs::SessionApps;
use crate::mock::browser_agent::OpenCase;
use crate::tests::{FRAME, click};
use crate::tests_archive_pins::{Fake, Rig};
use crate::tests_machines_page::exists;
use crate::web::browser_open::{FIELD, hint, submit_word, suggestions};
use serde_json::json;

const CANVASES: &str = r#"[{"canvasId":"cv_9","kind":"browser","title":"Docs","version":1,"state":{"url":"http://localhost:5199/"}}]"#;

fn fleet(refuse: bool) -> Fake {
    Fake::start(move |method, path| match (method, path) {
        ("GET", p) if p.ends_with("/apps") => (
            200,
            json!({"previewCommand": "npm run dev", "apps": [
                {"id": "a1", "command": "npm run build", "status": "exited", "ports": []},
                {"id": "a2", "command": "npm run dev", "status": "running", "ports": [5173]},
                {"id": "a3", "command": "pnpm dev", "status": "stopped", "ports": []}]})
            .to_string(),
        ),
        ("POST", p) if refuse && (p.ends_with("/apps") || p.ends_with("/browser")) => {
            (400, json!({"error": "Nothing here can be run."}).to_string())
        }
        ("POST", p) if p.ends_with("/apps") => {
            (200, json!({"app": {"id": "a9", "command": "npm run dev", "status": "starting", "ports": []}, "canvasId": "cv_9"}).to_string())
        }
        ("POST", p) if p.ends_with("/browser") => (200, json!({"canvasId": "cv_9"}).to_string()),
        ("GET", p) if p.ends_with("/canvases") => (200, CANVASES.into()),
        _ => (200, "{}".into()),
    })
}

fn opened(fake: &Fake) -> Rig {
    let mut r = Rig::home(fake);
    r.rt.update(|s, tx| s.open_browser_dialog(tx));
    r.rt.frame(FRAME);
    r
}

#[test]
fn says_what_each_target_does() {
    assert_eq!(hint(None), "A command, like npm run dev, or an address on Fleet's machine, like localhost:5173.");
    let address = crate::conv::browser::browser_target("5173");
    assert_eq!(hint(address.as_ref()), "Shows http://localhost:5173/ through Fleet.");
    let command = crate::conv::browser::browser_target("npm run dev");
    assert_eq!(hint(command.as_ref()), "Runs it in this session's folder and shows the page it serves.");
    assert_eq!((submit_word(address.as_ref()), submit_word(command.as_ref()), submit_word(None)), ("Open", "Run", "Run"));
}

#[test]
fn lists_the_projects_command_then_the_apps_newest_first_without_repeats() {
    let app =
        |c: &str| crate::fleet::app_runs::from_response("s", &json!({"id": c, "command": c, "status": "running", "ports": []})).unwrap();
    let known = SessionApps {
        apps: vec![app("npm run build"), app("npm run dev"), app("pnpm dev"), app("")],
        preview_command: Some("npm run dev".into()),
    };
    assert_eq!(suggestions(&known), ["npm run dev", "pnpm dev", "npm run build"]);
    assert!(suggestions(&SessionApps::default()).is_empty());
}

#[test]
fn opens_empty_then_fills_with_the_command_that_served_a_page_unless_the_user_typed() {
    let fake = fleet(false);
    let mut r = opened(&fake);
    assert_eq!(r.state().web.open.as_ref().unwrap().value.text, "", "it opens empty");
    r.wait("the commands used here before", |s| s.web.open.as_ref().is_some_and(|d| !d.suggestions.is_empty()));
    let d = r.state().web.open.as_ref().unwrap();
    assert_eq!(d.value.text, "npm run dev");
    assert_eq!(d.suggestions, ["npm run dev", "pnpm dev", "npm run build"]);
    // Typing first: the suggestion arriving later doesn't take the field.
    let mut r = Rig::home(&fake);
    r.rt.update(|s, tx| s.open_browser_dialog(tx));
    r.rt.frame(FRAME);
    focus_field(&mut r.rt);
    type_in(&mut r.rt, "5173");
    r.wait("the commands used here before", |s| s.web.open.as_ref().is_some_and(|d| !d.suggestions.is_empty()));
    assert_eq!(r.state().web.open.as_ref().unwrap().value.text, "5173");
}

#[test]
fn a_suggestion_fills_the_field_and_is_marked_on() {
    let fake = fleet(false);
    let mut r = opened(&fake);
    r.wait("the commands used here before", |s| s.web.open.as_ref().is_some_and(|d| !d.suggestions.is_empty()));
    click(&mut r.rt, "browser-open-suggestion-1");
    r.rt.frame(FRAME);
    assert_eq!(r.state().web.open.as_ref().unwrap().value.text, "pnpm dev");
    assert!(r.state().web.open.as_ref().unwrap().touched);
}

#[test]
fn run_sends_the_command_and_shows_the_new_tab() {
    let fake = fleet(false);
    let mut r = opened(&fake);
    r.wait("the commands used here before", |s| s.web.open.as_ref().is_some_and(|d| !d.suggestions.is_empty()));
    click(&mut r.rt, "browser-open-submit");
    r.wait("the dialog closed", |s| s.web.open.is_none());
    assert_eq!(
        fake.calls("/apps").iter().filter(|c| c.0 == "POST").map(|c| c.2.clone()).collect::<Vec<_>>(),
        [json!({"command": "npm run dev"})]
    );
    r.wait("the new tab active", |s| s.active_canvas() == crate::fleet::canvases::server_tab_id("cv_9"));
}

#[test]
fn open_sends_the_address_and_shows_the_new_tab() {
    let fake = fleet(false);
    let mut r = opened(&fake);
    focus_field(&mut r.rt);
    type_in(&mut r.rt, "5199");
    r.rt.frame(FRAME);
    click(&mut r.rt, "browser-open-submit");
    r.wait("the dialog closed", |s| s.web.open.is_none());
    assert_eq!(
        fake.calls("/browser").iter().filter(|c| c.0 == "POST").map(|c| c.2.clone()).collect::<Vec<_>>(),
        [json!({"url": "http://localhost:5199/"})]
    );
    r.wait("the new tab active", |s| s.active_canvas() == crate::fleet::canvases::server_tab_id("cv_9"));
}

#[test]
fn a_refusal_stays_in_the_dialog_and_typing_takes_it_away() {
    let fake = fleet(true);
    let mut r = opened(&fake);
    focus_field(&mut r.rt);
    type_in(&mut r.rt, "oops");
    r.rt.frame(FRAME);
    click(&mut r.rt, "browser-open-submit");
    r.wait("Fleet's refusal", |s| s.web.open.as_ref().is_some_and(|d| d.error.is_some()));
    r.rt.frame(FRAME);
    let d = r.state().web.open.as_ref().unwrap();
    assert!(!d.busy && d.error.as_deref().is_some_and(|e| e.contains("Nothing here can be run.")));
    assert!(exists(&r.rt, "browser-open-error"));
    focus_field(&mut r.rt);
    type_in(&mut r.rt, "x");
    r.rt.frame(FRAME);
    assert!(r.state().web.open.as_ref().unwrap().error.is_none());
}

#[test]
fn run_is_off_without_a_target_or_while_busy() {
    let fake = fleet(false);
    let mut r = opened(&fake);
    click(&mut r.rt, "browser-open-submit");
    r.rt.frame(FRAME);
    assert!(fake.calls("/apps").iter().all(|c| c.0 != "POST"), "nothing to send");
    let mut busy = crate::tests::app();
    busy.update(|s, _| s.show_browser_open(OpenCase::Busy, true));
    busy.frame(FRAME);
    assert!(busy.state.web.open.as_ref().unwrap().busy);
    click(&mut busy, "browser-open-submit");
    assert!(busy.state.web.open.is_some());
}

#[test]
fn the_plus_menu_offers_browser_and_opens_the_dialog() {
    let mut rt = crate::tests::app();
    rt.update(|s, _| s.show_browser_tab(crate::mock::browser_tab::BrowserCase::Shown));
    rt.frame(FRAME);
    click(&mut rt, "canvas-add");
    rt.frame(FRAME);
    assert!(exists(&rt, "canvas-picker-browser"));
    click(&mut rt, "canvas-picker-browser");
    rt.frame(FRAME);
    assert!(rt.state.web.open.is_some() && exists(&rt, "browser-open-dialog"));
    assert!(
        crate::conv::browser::browser_target("").is_none()
            && matches!(crate::conv::browser::browser_target("x y"), Some(BrowserTarget::Command(_)))
    );
}

fn focus_field(rt: &mut rune::Runtime<crate::State>) {
    rt.update(|_, tx| tx.focus(FIELD, false));
    rt.frame(FRAME);
}

/// Types into the field the dialog focused (the composer helper clicks the composer first).
fn type_in(rt: &mut rune::Runtime<crate::State>, words: &str) {
    rt.key(rune::KeyEvent { key: rune::Key::Text(words.into()), mods: Default::default() });
    rt.frame(FRAME);
}
