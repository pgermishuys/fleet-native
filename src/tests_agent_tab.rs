//! Agent's view (component 10, piece M): `AgentTabView.vue` has no test of its own, so these are read from its rules (rows
//! 5.58 to 5.68), headless like the rest of `tests.rs`.
use crate::fleet::agent_tab::{self, frame_key, frame_path, fresh, placed};
use crate::mock::agent_tab::Case;
use crate::mock::failure::{NOW, chrono_free_iso};
use crate::tests::{FRAME, app};
use crate::tests_machines_page::exists;
use rune::Runtime;

fn shown(case: Case) -> Runtime<crate::State> {
    let mut rt = app();
    rt.update(move |s, _| s.show_agent_tab(case, true));
    for _ in 0..3 {
        rt.frame(FRAME);
    }
    rt
}

fn rect(rt: &Runtime<crate::State>, key: &str) -> [f32; 4] {
    rt.presented(key).map(|p| p.0).unwrap_or_else(|| panic!("{key} is drawn"))
}

fn tick(rt: &mut Runtime<crate::State>, now: f64) {
    rt.update(move |s, tx| {
        s.keep_agent_view(now, tx);
    });
}

#[test]
fn the_frame_sits_at_the_top_scaled_to_fit() {
    let rt = shown(Case::Ring);
    let (stage, picture) = (rect(&rt, "agent-tab-view"), rect(&rt, "agent-tab-picture"));
    let scale = (stage[2] / 1280.0).min(stage[3] / 800.0);
    assert!((picture[2] - 1280.0 * scale).abs() < 1.0 && (picture[3] - 800.0 * scale).abs() < 1.0, "{picture:?} in {stage:?}");
    assert!((picture[1] - stage[1]).abs() < 1.0, "pinned to the top");
    // Wider than the page's shape: centred across.
    let p = placed([1000.0, 400.0]).unwrap();
    assert_eq!((p.scale, p.left), (0.5, 180.0));
    // Taller: the room goes below, and the caption rides the picture's edge.
    let p = placed([640.0, 800.0]).unwrap();
    assert_eq!((p.scale, p.left, p.bottom(800.0)), (0.5, 0.0, 400.0 + 12.0));
    assert!(placed([0.0, 400.0]).is_none());
}

#[test]
fn the_ring_and_caption_go_six_seconds_after_the_step() {
    let at = chrono_free_iso(NOW);
    assert!(fresh(Some(&at), NOW + 5_999) && !fresh(Some(&at), NOW + 6_000));
    assert!(!fresh(None, NOW) && !fresh(Some("never"), NOW));
    let mut rt = shown(Case::Ring);
    assert!(exists(&rt, "agent-tab-ring") && exists(&rt, "agent-tab-caption"));
    rt.update(|s, _| s.now_ms = Some(NOW + 6_000));
    tick(&mut rt, 10.0);
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(!exists(&rt, "agent-tab-ring") && !exists(&rt, "agent-tab-caption"), "both end on the tick");
    assert!(exists(&rt, "agent-tab-picture"), "the picture stays");
}

#[test]
fn the_ring_is_the_box_scaled_and_padded() {
    let rt = shown(Case::Ring);
    let (stage, ring) = (rect(&rt, "agent-tab-view"), rect(&rt, "agent-tab-ring"));
    let p = placed([stage[2], stage[3]]).unwrap();
    let [x, y, w, h] = p.ring(&crate::conv::browser::StepBox { x: 464.0, y: 250.0, width: 361.0, height: 250.0 });
    assert!(((ring[0] - stage[0]) - x).abs() < 1.0 && ((ring[1] - stage[1]) - y).abs() < 1.0, "{ring:?} {x} {y}");
    assert!((ring[2] - w).abs() < 1.0 && (ring[3] - h).abs() < 1.0);
}

#[test]
fn a_failed_step_is_captioned_and_a_step_without_a_box_has_no_ring() {
    let rt = shown(Case::Failed);
    assert!(exists(&rt, "agent-tab-caption") && !exists(&rt, "agent-tab-ring"));
}

#[test]
fn says_loading_then_closed_when_there_is_no_picture() {
    use crate::view::agent_tab::empty_words;
    assert_eq!(empty_words(false), "Loading the agent's tab…");
    assert_eq!(empty_words(true), "The agent's tab is closed.");
    let rt = shown(Case::Loading);
    assert!(exists(&rt, "agent-tab-empty") && !exists(&rt, "agent-tab-picture") && !rt.state.agent_view.failed);
    let rt = shown(Case::Closed);
    assert!(exists(&rt, "agent-tab-empty") && rt.state.agent_view.failed);
}

#[test]
fn a_picture_already_shown_stays_up_through_a_failed_load() {
    let mut rt = shown(Case::Ring);
    let (session, tab) = rt.state.agent_view.target.clone().unwrap();
    let next = frame_key(&session, &tab, 1);
    rt.update(move |s, _| {
        s.on_image_bytes(&next, Err("HTTP 404".into()));
    });
    tick(&mut rt, 20.0);
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(exists(&rt, "agent-tab-picture") && !exists(&rt, "agent-tab-empty"));
}

#[test]
fn the_last_picture_stays_up_until_the_next_has_loaded() {
    let mut rt = shown(Case::Ring);
    let (session, tab) = rt.state.agent_view.target.clone().unwrap();
    let first = rt.state.agent_view.shown.clone().unwrap();
    // The tick asks for frame 1; while it is on its way the first is still shown.
    tick(&mut rt, 20.0);
    let n = rt.state.agent_view.nonce();
    assert!(n >= 1);
    assert_eq!(rt.state.agent_view.shown.as_deref(), Some(first.as_str()));
    // When it has loaded it takes over, and the older one is let go.
    let next = frame_key(&session, &tab, n);
    rt.update({
        let next = next.clone();
        move |s, _| s.supply_image(&next, crate::mock::images::PAGE)
    });
    tick(&mut rt, 20.1);
    assert_eq!(rt.state.agent_view.shown.as_deref(), Some(next.as_str()));
    assert!(rt.state.images.ready(&first).is_none(), "the older picture was forgotten");
}

#[test]
fn no_new_frame_is_asked_for_while_the_view_is_hidden() {
    let mut rt = shown(Case::Ring);
    let before = rt.state.agent_view.nonce();
    rt.update(|s, _| s.quiet.visible = false);
    tick(&mut rt, 30.0);
    tick(&mut rt, 31.0);
    assert_eq!(rt.state.agent_view.nonce(), before, "the window is hidden");
    rt.update(|s, _| s.quiet.visible = true);
    tick(&mut rt, 32.0);
    assert_eq!(rt.state.agent_view.nonce(), before + 1, "back on screen");
    // Another tab in front of the canvas is the same as hiding it.
    rt.update(|s, _| {
        let mut tabs = s.open_canvases();
        tabs.active_id = "context".into();
        let id = s.canvas_session();
        s.canvases.replace(&id, tabs);
    });
    tick(&mut rt, 33.0);
    assert_eq!(rt.state.agent_view.nonce(), before + 1);
}

#[test]
fn asks_for_the_frame_on_the_tick_700_ms_apart_with_a_fresh_nonce() {
    let mut rt = shown(Case::Ring);
    let before = rt.state.agent_view.nonce();
    tick(&mut rt, 40.0);
    assert_eq!(rt.state.agent_view.nonce(), before + 1);
    tick(&mut rt, 40.3);
    assert_eq!(rt.state.agent_view.nonce(), before + 1, "not before 700 ms");
    let (session, tab) = rt.state.agent_view.target.clone().unwrap();
    assert_eq!(
        frame_path(&session, &tab, 3),
        format!("/api/sessions/{session}/agent-browser/tabs/{tab}/frame?n=3"),
        "the path Fleet serves"
    );
    assert_eq!(frame_path("a b", "t/1", 0), "/api/sessions/a%20b/agent-browser/tabs/t%2F1/frame?n=0");
    assert_eq!(agent_tab::FRAME_MS, 700);
}

#[test]
fn now_follows_the_tick_even_when_the_view_is_hidden() {
    let mut rt = shown(Case::Ring);
    rt.update(|s, _| {
        s.quiet.visible = false;
        s.now_ms = Some(NOW + 9_000);
    });
    tick(&mut rt, 50.0);
    assert_eq!(rt.state.agent_view.now, NOW + 9_000);
}

#[test]
fn the_size_tag_is_left_out_of_a_narrow_canvas() {
    assert!(agent_tab::tag_shown(421.0) && !agent_tab::tag_shown(420.0));
    assert!(!exists(&shown(Case::Narrow), "agent-tab-tag"));
}

#[test]
fn leaving_the_session_stops_watching() {
    let mut rt = shown(Case::Ring);
    rt.update(|s, _| s.agent_view.target = Some(("another".into(), "tab".into())));
    tick(&mut rt, 60.0);
    assert!(rt.state.agent_view.target.is_none());
}
