//! The tool card's Show and "Diagram retired" (component 10, piece C; rows 3.10 and 6.14), from `ToolCard.rows.test.ts`,
//! `ToolCard.test.ts` and `activity-stream-utils.test.ts`. The scene is `mock::tools::of_canvases`.
use crate::conv::card::canvas_id_of;
use crate::tests::{FRAME, app, click};
use crate::{State, mock};
use rune::Runtime;
use serde_json::json;

fn scene() -> Runtime<State> {
    let mut rt = app();
    rt.update(|s, _| s.show_conversation(mock::tools::of_canvases()));
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt
}

fn has(rt: &Runtime<State>, key: &str) -> bool {
    rt.find(key).is_some()
}

// ToolCard.rows.test.ts
#[test]
fn offers_show_only_on_a_finished_call_that_has_a_canvas() {
    let rt = scene();
    assert!(has(&rt, "tool-k1-show"), "finished, with a canvas");
    assert!(!has(&rt, "tool-k2-show"), "a call that failed");
    assert!(!has(&rt, "tool-k3-show"), "a call still running, with no canvas yet");
}

// activity-stream-utils.test.ts
#[test]
fn shows_a_browser_tools_title_and_address_and_the_canvas_it_opened() {
    let done = json!({"status": "completed", "metadata": {"canvasId": "cv_1", "version": 2}});
    let running = json!({"status": "running"});
    assert_eq!(canvas_id_of("fleet_app_start", &done).as_deref(), Some("cv_1"));
    assert_eq!(canvas_id_of("fleet_app_start", &running), None);
}

// activity-stream-utils.test.ts
#[test]
fn offers_no_canvas_for_a_retired_diagram_call_by_any_harnesss_name_for_it() {
    let state = json!({"status": "completed", "metadata": {"canvasId": "cv_1", "version": 1}});
    for tool in ["fleet_canvas_open", "fleet_canvas_patch", "mcp__fleet__fleet_canvas_open", "mcp__fleet__fleet_canvas_patch"] {
        assert_eq!(canvas_id_of(tool, &state), None, "{tool}");
    }
}

// ToolCard.test.ts
#[test]
fn says_quietly_that_a_retired_diagram_is_gone_instead_of_offering_to_show_it() {
    let rt = scene();
    for id in ["k4", "k5"] {
        assert!(!has(&rt, &format!("tool-{id}-show")), "{id}");
        assert!(has(&rt, &format!("tool-{id}-retired")), "{id}");
    }
    assert!(!has(&rt, "tool-k1-retired"), "a live canvas isn't retired");
}

#[test]
fn pressing_show_brings_the_canvas_forward_opens_the_panel_and_leaves_the_card_as_it_was() {
    let mut rt = scene();
    let before = rt.state.tool_open.clone();
    rt.update(|s, _| s.set_content_open(false));
    rt.frame(FRAME);
    click(&mut rt, "tool-k1-show");
    assert!(rt.state.content_open);
    assert_eq!(rt.state.tool_open, before, "Show doesn't open or close the card");
}

// MessageToolList.test.ts: "passes a row's events up with the call's title and id" (show-canvas)
#[test]
fn show_asks_for_the_canvas_the_call_opened_and_the_tab_comes_forward_when_it_is_open() {
    let mut rt = scene();
    let id = rt.state.canvas_session();
    rt.update(|s, _| {
        let canvas = crate::fleet::canvases::read_server_canvas(
            &json!({"canvasId": "cv_1", "kind": "page", "title": "Totals", "version": 1, "state": {}}),
        );
        s.canvases.apply_canvas_event(&id, crate::fleet::canvases::CanvasEvent::Updated(canvas.unwrap()));
        s.canvases.activate(&id, "files");
    });
    click(&mut rt, "tool-k1-show");
    assert_eq!(rt.state.active_canvas(), "canvas:cv_1");
}

// CanvasHost.test.ts: "closes a server canvas through the server" (the request itself is in `fleet::tests::server_canvases`)
#[test]
fn closes_a_server_canvas_through_the_server() {
    let mut rt = scene();
    let id = rt.state.canvas_session();
    rt.update(|s, _| {
        let page = json!({"canvasId": "cv_1", "kind": "page", "title": "Totals", "version": 1, "state": {}});
        let canvas = crate::fleet::canvases::read_server_canvas(&page).unwrap();
        s.canvases.apply_canvas_event(&id, crate::fleet::canvases::CanvasEvent::Updated(canvas));
        s.canvases.activate(&id, "canvas:cv_1");
        s.set_content_open(true);
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    click(&mut rt, &crate::fleet::canvas_host::close_key("canvas:cv_1"));
    assert!(rt.state.open_canvases().canvases.iter().all(|c| c.server.is_none()));
}
