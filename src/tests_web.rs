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
        load_key: "p1:".into(),
        rect: [600.0, 60.0, 360.0, 500.0],
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
