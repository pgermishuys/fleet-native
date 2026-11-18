//! The page tab and the web view host's rule, headless (component 14, piece A). A headless run has no window, so the body is
//! the fallback panel (decision 9); `wanted` is the rule a real window's host follows (rows 14.3 and 14.4).
use crate::tests::{FRAME, app, click};
use crate::web::{Placement, Wanted, wanted};

const BASE: &str = "http://127.0.0.1:6411";

/// The page-pick scene with its page opened from the card, served (as far as the rule knows) at `BASE`.
fn opened() -> rune::Runtime<crate::State> {
    let mut rt = app();
    rt.update(|s, _| {
        s.show_page_fallback();
        s.web.mock_base = Some(BASE.into());
        // Back on another tab, so the card's Open page is what brings the page forward.
        s.edit_canvases(|all, id| all.activate(id, "changes"));
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    click(&mut rt, "page-pick-open");
    rt.frame(FRAME);
    rt
}

#[test]
fn open_page_shows_the_page_tab_and_its_fallback_without_a_window() {
    let rt = opened();
    assert!(rt.find("canvas-tab-canvas:page_1").is_some(), "the page's tab is in the strip");
    assert_eq!(rt.state.active_canvas(), "canvas:page_1");
    assert!(rt.find("page-tab-fallback").is_some(), "no window, so no web view: the fallback says so");
    assert!(rt.find("page-tab-open-browser").is_some());
    assert!(rt.find("page-tab-frame").is_none());
}

#[test]
fn another_tab_leaves_the_page_and_its_tab_brings_it_back() {
    let mut rt = opened();
    click(&mut rt, "canvas-tab-changes");
    rt.frame(FRAME);
    assert!(rt.find("page-tab-fallback").is_none());
    // The strip has scrolled to Changes; the tab's own handler, as a press on it would run it.
    rt.update(|s, tx| s.activate_tab("canvas:page_1", tx));
    rt.frame(FRAME);
    assert!(rt.find("page-tab-fallback").is_some());
}

#[test]
fn opening_the_page_twice_keeps_one_tab() {
    let mut rt = opened();
    rt.update(|s, _| s.open_page("page_1"));
    let tabs = rt.state.open_canvases();
    assert_eq!(tabs.canvases.iter().filter(|c| c.id == "canvas:page_1").count(), 1);
    assert_eq!(tabs.active().map(|c| c.title()), Some("Layouts for pocket-notes".to_string()));
}

#[test]
fn wanted_places_the_page_where_its_area_is() {
    let mut rt = opened();
    rt.update(|s, _| s.web.rect = Some([600.0, 60.0, 360.0, 500.0]));
    let expect = Placement {
        tab: "canvas:page_1".into(),
        url: format!("{BASE}/pages/p1/index.html"),
        load_key: "p1::0".into(),
        rect: [600.0, 60.0, 360.0, 500.0],
        html: None,
        place: crate::web::page_theme::PagePlace::Tab,
        browser: false,
    };
    assert_eq!(wanted(&rt.state), Wanted::Shown(expect));
}

#[test]
fn wanted_is_nothing_while_another_tab_is_active() {
    let mut rt = opened();
    rt.update(|s, _| {
        s.web.rect = Some([600.0, 60.0, 360.0, 500.0]);
        s.edit_canvases(|all, id| all.activate(id, "changes"));
    });
    assert_eq!(wanted(&rt.state), Wanted::Nothing);
}

#[test]
fn wanted_is_nothing_on_another_route() {
    let mut rt = opened();
    rt.update(|s, _| {
        s.web.rect = Some([600.0, 60.0, 360.0, 500.0]);
        s.rail = crate::Rail::Settings;
    });
    assert_eq!(wanted(&rt.state), Wanted::Nothing);
}

#[test]
fn wanted_is_nothing_with_the_panel_collapsed() {
    let mut rt = opened();
    rt.update(|s, _| {
        s.web.rect = Some([600.0, 60.0, 360.0, 500.0]);
        s.content_open = false;
    });
    assert_eq!(wanted(&rt.state), Wanted::Nothing);
}

#[test]
fn wanted_hides_the_page_while_a_menu_is_open_and_shows_it_after() {
    let mut rt = opened();
    rt.update(|s, _| {
        s.web.rect = Some([600.0, 60.0, 360.0, 500.0]);
        s.ui.open = Some(crate::ui::Open::Palette);
    });
    assert_eq!(wanted(&rt.state), Wanted::Hidden { tab: "canvas:page_1".into() });
    rt.update(|s, _| s.ui.open = None);
    assert!(matches!(wanted(&rt.state), Wanted::Shown(_)));
}

#[test]
fn wanted_loads_again_when_the_agent_shows_the_page_again() {
    let mut rt = opened();
    rt.update(|s, _| s.web.rect = Some([600.0, 60.0, 360.0, 500.0]));
    let Wanted::Shown(first) = wanted(&rt.state) else { panic!("shown") };
    rt.update(|s, _| {
        let canvas = serde_json::json!({"canvasId": "page_1", "kind": "page", "title": "Layouts", "version": 4,
            "state": {"pageId": "p1", "entry": "index.html", "shownAt": "2026-10-07T02:00:00Z"}});
        let id = s.canvas_session();
        let canvas = crate::fleet::canvases::read_server_canvas(&canvas).expect("a page canvas");
        s.canvases.apply_canvas_event(&id, crate::fleet::canvases::CanvasEvent::Updated(canvas));
    });
    let Wanted::Shown(again) = wanted(&rt.state) else { panic!("shown") };
    assert_eq!(again.url, first.url);
    assert_ne!(again.load_key, first.load_key, "a new shownAt loads the page again");
}

#[test]
fn wanted_is_nothing_before_the_area_is_laid_out() {
    let rt = opened();
    assert_eq!(rt.state.web.rect, None, "headless: the fallback has no area to report");
    assert_eq!(wanted(&rt.state), Wanted::Nothing);
}

// Piece B: the page bridge, as the host's IPC handler drives it (`State::hear_page`), with no window.
use serde_json::{Value, json};

fn hear(rt: &mut rune::Runtime<crate::State>, tab: &str, data: Value) -> Option<Value> {
    let tab = tab.to_string();
    let mut out = None;
    rt.update(|s, tx| out = s.hear_page(&tab, &data, tx));
    out
}

fn typed(rt: &rune::Runtime<crate::State>) -> String {
    rt.state.draft.text.to_string()
}

#[test]
fn puts_a_reply_in_the_composer_and_sends_nothing() {
    let mut rt = opened();
    assert_eq!(hear(&mut rt, "canvas:page_1", json!({"type": "fleet:page-reply", "text": "# Re: Plan\n\nGo with B"})), None);
    rt.frame(FRAME);
    assert_eq!(typed(&rt), "# Re: Plan\n\nGo with B");
    let mut focused = None;
    rt.update(|_, tx| focused = tx.focused().map(str::to_string));
    assert_eq!(focused.as_deref(), Some("composer"), "the composer has the keyboard");
}

#[test]
fn keeps_what_the_user_already_typed_above_the_reply() {
    let mut rt = opened();
    rt.update(|s, _| s.set_draft_text("Thinking about it"));
    hear(&mut rt, "canvas:page_1", json!({"type": "fleet:page-reply", "text": "Answer: B"}));
    assert_eq!(typed(&rt), "Thinking about it\n\nAnswer: B");
}

#[test]
fn ignores_messages_from_any_other_web_view() {
    let mut rt = opened();
    assert_eq!(hear(&mut rt, "canvas:page_2", json!({"type": "fleet:page-reply", "text": "from elsewhere"})), None);
    assert_eq!(hear(&mut rt, "canvas:page_2", json!({"type": "fleet:page-state", "state": {"a": 1}})), None);
    assert_eq!(typed(&rt), "");
    assert_eq!(rt.state.web.kept.kept("p1"), Value::Null);
}

#[test]
fn answers_a_hello_with_the_state_the_page_saved_also_after_the_page_is_shown_again() {
    let mut rt = opened();
    let hello = || json!({"type": "fleet:page-hello"});
    assert_eq!(hear(&mut rt, "canvas:page_1", hello()), Some(json!({"type": "fleet:page-state", "state": null})));
    hear(&mut rt, "canvas:page_1", json!({"type": "fleet:page-state", "state": {"answers": {"one": "2"}}}));
    let saved = Some(json!({"type": "fleet:page-state", "state": {"answers": {"one": "2"}}}));
    assert_eq!(hear(&mut rt, "canvas:page_1", hello()), saved);
    // Another tab, then back: the page is made again and says hello again.
    rt.update(|s, _| {
        let _ = s.edit_canvases(|all, id| all.activate(id, "changes"));
    });
    rt.update(|s, tx| s.activate_tab("canvas:page_1", tx));
    assert_eq!(hear(&mut rt, "canvas:page_1", hello()), saved);
}

#[test]
fn stops_listening_once_the_canvas_is_gone() {
    let mut rt = opened();
    rt.update(|s, _| {
        let _ = s.edit_canvases(|all, id| all.activate(id, "changes"));
    });
    assert_eq!(hear(&mut rt, "canvas:page_1", json!({"type": "fleet:page-reply", "text": "late"})), None);
    assert_eq!(typed(&rt), "");
}

#[test]
fn a_page_cant_send_a_pick_or_a_refusal_itself() {
    let mut rt = opened();
    assert_eq!(hear(&mut rt, "canvas:page_1", json!({"type": "fleet:page-choice", "value": "a"})), None);
    assert_eq!(hear(&mut rt, "canvas:page_1", json!({"type": "fleet:page-choice-refused", "reason": "x"})), None);
}

#[test]
fn the_pages_theme_follows_the_window() {
    let mut rt = opened();
    rt.update(|s, _| s.drawn_dark.set(true));
    assert!(rt.state.page_theme().dark);
    assert_eq!(rt.state.page_theme().variables["--fleet-bg"], "#141418");
    rt.update(|s, _| s.drawn_dark.set(false));
    let light = rt.state.page_theme();
    assert!(!light.dark);
    assert_eq!(light.variables["--fleet-bg"], "#ffffff");
    assert!(light.frame_name(crate::web::page_theme::PagePlace::Tab).starts_with("fleet:{"));
}
