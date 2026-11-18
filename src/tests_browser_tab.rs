//! The browser tab: bar, preview and its bridge (component 14, piece H). The seven `BrowserCanvas.test.ts` cases of this piece
//! under their names, and ours for the bar. A headless run has no web view, so the page area is the fallback panel; what the
//! host would load is read from `wanted` and `BrowserUi::frame_src`. Requests go to a fake Fleet that records them.
use crate::mock::browser_tab::{BrowserCase, PAGE, PREVIEW, TAB};
use crate::tests::{FRAME, click};
use crate::tests_archive_pins::{Fake, Rig};
use crate::web::preview_bridge::NavAction;
use serde_json::{Value, json};

const CANVASES: &str = r#"[{"canvasId":"cv_1","kind":"browser","title":"Pocket notes","version":1,"state":{"url":"http://localhost:5173/","appId":"app_1"}},{"canvasId":"cv_2","kind":"browser","title":"Docs","version":1,"state":{"url":"http://localhost:5199/"}}]"#;

fn fleet() -> Fake {
    Fake::start(|method, path| match (method, path) {
        (_, p) if p.ends_with("/browser/proxy") => (200, json!({"origin": PREVIEW}).to_string()),
        ("POST", p) if p.ends_with("/browser") => (200, json!({"canvasId": "cv_2"}).to_string()),
        ("GET", p) if p.ends_with("/canvases") => (200, CANVASES.into()),
        _ => (200, "{}".into()),
    })
}

/// The tab on screen, its first page asked of Fleet and answered.
fn shown(fake: &Fake) -> Rig {
    let mut r = Rig::home(fake);
    r.rt.update(|s, _| s.show_browser_tab(BrowserCase::Fresh));
    r.rt.frame(FRAME);
    r.wait("the page's preview address", |s| s.browser_ui(TAB).is_some_and(|u| u.frame_src.is_some()));
    r
}

fn hear(r: &mut Rig, data: Value) {
    r.rt.update(move |s, _| s.hear_preview(TAB, &data));
    r.rt.frame(FRAME);
}

fn proxied(fake: &Fake) -> Vec<Value> {
    fake.calls("/browser/proxy").into_iter().map(|c| c.2).collect()
}

fn address(r: &Rig) -> String {
    r.state().browser_ui(TAB).map(|u| u.text()).unwrap_or_default()
}

// "frames the page through its preview and shows where the page is"
#[test]
fn frames_the_page_through_its_preview_and_shows_where_the_page_is() {
    let fake = fleet();
    let mut r = shown(&fake);
    assert_eq!(proxied(&fake), vec![json!({"url": PAGE})]);
    let ui = r.state().browser_ui(TAB).unwrap();
    assert_eq!(ui.frame_src.as_deref(), Some(format!("{PREVIEW}/").as_str()));
    assert_eq!(address(&r), PAGE);
    hear(&mut r, json!({"fleet": 1, "type": "hello", "hmr": "vite", "href": format!("{PREVIEW}/cart?item=2#top"), "title": "Cart"}));
    assert_eq!(address(&r), "http://localhost:5173/cart?item=2#top", "the preview's origin is swapped for the app's");
    hear(&mut r, json!({"fleet": 1, "type": "location", "href": format!("{PREVIEW}/pay")}));
    assert_eq!(address(&r), "http://localhost:5173/pay");
}

// "ignores messages from other origins and old bridges": the host hears only the tab's own web view, so the other origin is
// not a case; an old bridge's message and another version are.
#[test]
fn ignores_messages_from_old_bridges() {
    let fake = fleet();
    let mut r = shown(&fake);
    hear(&mut r, json!({"type": "fleet-browser:location", "href": format!("{PREVIEW}/old")}));
    hear(&mut r, json!({"fleet": 2, "type": "location", "href": format!("{PREVIEW}/new")}));
    assert_eq!(address(&r), PAGE);
}

// "pulses its tab when the page hot-updates or reloads itself, not when the canvas reloads it"
#[test]
fn pulses_its_tab_when_the_page_hot_updates_or_reloads_itself_not_when_the_canvas_reloads_it() {
    let fake = fleet();
    let mut r = shown(&fake);
    let hello = json!({"fleet": 1, "type": "hello", "hmr": "vite", "href": format!("{PREVIEW}/")});
    hear(&mut r, hello.clone());
    assert_eq!(r.state().canvases.updated_at(TAB), None, "the first hello is the load that was asked for");
    r.rt.update(|s, _| s.clock = 100.0);
    hear(&mut r, json!({"fleet": 1, "type": "update"}));
    assert_eq!(r.state().canvases.updated_at(TAB), Some(100.0));
    // Reload from the bar, then the hello of that load: not a pulse.
    r.rt.update(|s, _| s.clock = 101.0);
    click(&mut r.rt, "browser-reload");
    hear(&mut r, hello.clone());
    assert_eq!(r.state().canvases.updated_at(TAB), Some(100.0));
    // A hello for the same page nobody asked for: the page reloaded itself.
    r.rt.update(|s, _| s.clock = 105.0);
    hear(&mut r, hello);
    assert_eq!(r.state().canvases.updated_at(TAB), Some(105.0));
}

// "opens a link to another port on Fleet's machine in a new tab through Fleet, not in the frame"
#[test]
fn opens_a_link_to_another_port_in_a_new_tab_through_fleet_not_in_the_frame() {
    let fake = fleet();
    let mut r = shown(&fake);
    hear(&mut r, json!({"fleet": 1, "type": "open", "href": "http://localhost:5199/", "newTab": true}));
    r.wait("the canvases loaded again and that tab active", |s| s.open_canvases().active_id == "canvas:cv_2");
    let opened = fake.calls("/browser").into_iter().filter(|c| c.0 == "POST" && !c.1.ends_with("/proxy")).collect::<Vec<_>>();
    assert_eq!(opened.iter().map(|c| c.2.clone()).collect::<Vec<_>>(), vec![json!({"url": "http://localhost:5199/"})]);
    assert!(!fake.calls("/canvases").is_empty(), "the list was read again");
    assert_eq!(r.state().browser_ui(TAB).unwrap().frame_src.as_deref(), Some(format!("{PREVIEW}/").as_str()), "this tab stays on its page");
}

// If Fleet refuses the new tab, this tab shows the page.
#[test]
fn shows_the_link_in_this_tab_when_fleet_refuses_a_new_one() {
    let fake = Fake::start(|method, path| match (method, path) {
        (_, p) if p.ends_with("/browser/proxy") => (200, json!({"origin": PREVIEW}).to_string()),
        ("POST", p) if p.ends_with("/browser") => (400, json!({"error": "no"}).to_string()),
        _ => (200, "{}".into()),
    });
    let mut r = shown(&fake);
    hear(&mut r, json!({"fleet": 1, "type": "open", "href": "http://localhost:5199/shop", "newTab": true}));
    r.wait("the link shown here", |s| s.browser_ui(TAB).is_some_and(|u| u.text() == "http://localhost:5199/shop"));
}

// "follows a link meant for this tab through the preview of its address"
#[test]
fn follows_a_link_meant_for_this_tab_through_the_preview_of_its_address() {
    let fake = fleet();
    let mut r = shown(&fake);
    hear(&mut r, json!({"fleet": 1, "type": "open", "href": "http://localhost:5199/shop", "newTab": false}));
    r.wait("the other origin's preview", |s| s.browser_ui(TAB).is_some_and(|u| u.text() == "http://localhost:5199/shop"));
    assert_eq!(proxied(&fake).last(), Some(&json!({"url": "http://localhost:5199/shop"})));
    let ui = r.state().browser_ui(TAB).unwrap();
    assert_eq!(ui.frame_src.as_deref(), Some(format!("{PREVIEW}/shop").as_str()));
}

#[test]
fn asks_fleet_once_per_origin() {
    let fake = fleet();
    let mut r = shown(&fake);
    r.rt.update(|s, _| s.browser_show(TAB, "http://localhost:5173/a?b=1#c"));
    assert_eq!(proxied(&fake).len(), 1, "the same origin is not asked again");
    let ui = r.state().browser_ui(TAB).unwrap();
    assert_eq!(ui.frame_src.as_deref(), Some(format!("{PREVIEW}/a?b=1#c").as_str()));
}

// Rows 14.52 and 14.53: what is typed, and what Fleet or the address says when it can't be shown.
#[test]
fn turns_what_is_typed_into_an_address() {
    use crate::web::browser::BrowserUi;
    assert_eq!(BrowserUi::typed_address("5173"), "http://localhost:5173/");
    assert_eq!(BrowserUi::typed_address(" localhost:3000/x "), "http://localhost:3000/x");
    assert_eq!(BrowserUi::typed_address("HTTPS://example.test/"), "HTTPS://example.test/");
}

#[test]
fn says_when_the_address_is_not_one() {
    let fake = fleet();
    let mut r = shown(&fake);
    r.rt.update(|s, _| s.browser_show(TAB, "http://"));
    assert_eq!(r.state().browser_ui(TAB).unwrap().error.as_deref(), Some("http:// isn't an address."));
    r.rt.frame(FRAME);
    assert!(r.rt.find("browser-error").is_some());
    assert!(crate::web::wanted::shown_browser(r.state()).is_none(), "no web view while the tab says an error");
}

#[test]
fn says_what_fleet_said_when_it_wont_show_an_origin() {
    let fake = Fake::start(|_, p| {
        if p.ends_with("/browser/proxy") {
            (502, json!({"error": "Nothing is listening on that port."}).to_string())
        } else {
            (200, "{}".into())
        }
    });
    let mut r = Rig::home(&fake);
    r.rt.update(|s, _| s.show_browser_tab(BrowserCase::Fresh));
    r.rt.frame(FRAME);
    r.wait("Fleet's refusal", |s| s.browser_ui(TAB).is_some_and(|u| u.error.is_some()));
    assert_eq!(
        r.state().browser_ui(TAB).unwrap().error.as_deref(),
        Some("Fleet couldn't show http://localhost:5173: Nothing is listening on that port.")
    );
}

#[test]
fn the_address_is_not_overwritten_while_it_is_being_edited() {
    let fake = fleet();
    let mut r = shown(&fake);
    r.rt.update(|s, _| s.web.browsers.get_mut(TAB).unwrap().editing = true);
    hear(&mut r, json!({"fleet": 1, "type": "location", "href": format!("{PREVIEW}/pay")}));
    assert_eq!(address(&r), PAGE);
}

#[test]
fn back_forward_and_reload_reach_the_page_and_reload_without_a_view_loads_again() {
    let fake = fleet();
    let mut r = shown(&fake);
    let key = |r: &Rig| crate::web::wanted::shown_browser(r.state());
    r.rt.update(|s, _| s.web.rect = Some([600.0, 60.0, 360.0, 500.0]));
    let before = key(&r);
    click(&mut r.rt, "browser-reload");
    assert_ne!(key(&r), before, "no web view: Reload sets the address again");
    click(&mut r.rt, "browser-back");
    assert!(r.state().web.nav.is_empty(), "nothing to tell a page that isn't there");
    // With a web view on screen the page's own history is asked.
    r.rt.update(|s, _| {
        s.web.available = true;
        s.web.applied = Some(crate::web::Wanted::Hidden { tab: TAB.into() });
        s.browser_navigate(TAB, NavAction::Forward);
        s.browser_navigate(TAB, NavAction::Reload);
    });
    let scripts: Vec<_> = r.state().web.nav.iter().map(|n| n.script).collect();
    assert_eq!(scripts, vec!["history.forward()", "location.reload()"]);
}

#[test]
fn open_in_a_new_tab_is_the_previews_address_of_the_page_in_the_bar() {
    let fake = fleet();
    let r = shown(&fake);
    assert_eq!(r.state().browser_outside_url(TAB).as_deref(), Some(format!("{PREVIEW}/").as_str()));
    assert_eq!(crate::web::browser::outside_address("http://localhost:5173/cart?x=1#y", Some(PREVIEW)), format!("{PREVIEW}/cart?x=1#y"));
    assert_eq!(crate::web::browser::outside_address("not an address", Some(PREVIEW)), format!("{PREVIEW}/"));
    assert_eq!(crate::web::browser::outside_address("localhost:5173", None), "localhost:5173");
}

#[test]
fn the_bar_has_its_buttons_and_the_address() {
    let mut rt = crate::tests::app();
    rt.update(|s, _| s.show_browser_tab(BrowserCase::Shown));
    rt.frame(FRAME);
    rt.frame(FRAME);
    for key in ["browser-back", "browser-forward", "browser-reload", "browser-address", "browser-open-new"] {
        assert!(rt.find(key).is_some(), "{key}");
    }
}

#[test]
fn an_app_with_no_page_yet_waits_and_a_tab_with_no_address_loads() {
    let mut rt = crate::tests::app();
    rt.update(|s, _| s.show_browser_tab(BrowserCase::Waiting));
    rt.frame(FRAME);
    assert!(rt.find("browser-panel-title").is_some());
    let text = |rt: &rune::Runtime<crate::State>| rt.elements().into_iter().filter_map(|e| e.text).collect::<Vec<_>>().join("\n");
    assert!(text(&rt).contains("Waiting for the app to serve a page\u{2026}"));
    rt.update(|s, _| s.show_browser_tab(BrowserCase::Loading));
    rt.frame(FRAME);
    assert!(text(&rt).contains("Loading\u{2026}"));
}

// "says the app opens on its machine and links there, instead of a frame that can't load"
#[test]
fn says_the_app_opens_on_its_machine_and_links_there_instead_of_a_frame_that_cant_load() {
    let fake = fleet();
    let mut r = Rig::home(&fake);
    r.rt.update(|s, _| s.show_browser_tab(BrowserCase::Elsewhere));
    r.rt.frame(FRAME);
    r.rt.frame(FRAME);
    let text = r.rt.elements().into_iter().filter_map(|e| e.text.or(e.label)).collect::<Vec<_>>().join("\n");
    assert!(text.contains("This app runs on mini. Its preview opens there."), "{text}");
    assert!(r.rt.find("browser-open-there").is_some());
    assert!(crate::web::wanted::shown_browser(r.state()).is_none(), "no web view");
    assert!(proxied(&fake).is_empty(), "nothing is asked of Fleet for a machine whose sign-in isn't ours");
    assert_eq!(r.state().browser_machine().1.as_deref(), Some("http://10.0.0.7:6262/sessions/mock.0.0"));
}

// "says plainly that a machine without the web app can't open its previews from here"
#[test]
fn says_plainly_that_a_machine_without_the_web_app_cant_open_its_previews_from_here() {
    let mut rt = crate::tests::app();
    rt.update(|s, _| s.show_browser_tab(BrowserCase::ElsewhereNoApp));
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(rt.find("browser-open-there").is_none());
    let text = rt.elements().into_iter().filter_map(|e| e.text).collect::<Vec<_>>().join("\n");
    assert!(text.contains("mini has no web app of its own, so its previews can't open from here yet."), "{text}");
}

#[test]
fn a_percent_sign_is_not_an_address() {
    let fake = fleet();
    let mut r = shown(&fake);
    r.rt.update(|s, _| s.browser_show(TAB, "http://%"));
    assert_eq!(r.state().browser_ui(TAB).unwrap().error.as_deref(), Some("http://% isn't an address."));
}
