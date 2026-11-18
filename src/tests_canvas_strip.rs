//! The strip's motion and the menu's other groups (piece E of docs/parity/10-content-panel.md): the pulse of a page that updated
//! itself, the slide-in of a new tab, the active tab kept in view, and "From this conversation". Names are Fleet's where it has one
//! (`CanvasHost.test.ts`).
use crate::fleet::canvases::{VisualPayload, VisualType};
use crate::tests::{FRAME, app, click};
use rune::Runtime;

fn open() -> Runtime<crate::State> {
    let mut rt = app();
    rt.update(|s, _| s.content_open = true);
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt
}

fn visual(title: &str, kind: VisualType) -> VisualPayload {
    VisualPayload { kind, title: Some(title.into()), source_path: None, content: "# A\n".into() }
}

fn open_visual(rt: &mut Runtime<crate::State>, title: &str, activate: bool) {
    rt.update(|s, _| {
        s.edit_canvases(|c, id| c.open_visual(id, visual(title, VisualType::Markdown), activate));
    });
    rt.frame(FRAME);
}

fn has(rt: &Runtime<crate::State>, key: &str) -> bool {
    rt.find(key).is_some()
}

/// Sets the app's clock and lets the strip decide what it still has to draw. A frame would set the clock back (`poll`), so
/// what a test reads has to be read straight after, without one.
fn at(rt: &mut Runtime<crate::State>, clock: f64) {
    rt.update(|s, tx| {
        s.clock = clock;
        s.keep_tab_motion(tx);
    });
}

#[test]
fn pulses_a_tab_whose_page_updated_itself() {
    let mut rt = open();
    open_visual(&mut rt, "Cache design", false);
    let id = "visual:Cache design";
    assert_eq!(rt.state.tab_pulse(id), None, "nothing updated yet");
    rt.update(|s, _| s.canvases.mark_updated(id, 10.0));
    at(&mut rt, 10.0);
    assert_eq!(rt.state.tab_pulse(id), Some(0.0));
    at(&mut rt, 10.6);
    assert!((rt.state.tab_pulse(id).unwrap() - 0.5).abs() < 0.01);
    at(&mut rt, 11.3);
    assert_eq!(rt.state.tab_pulse(id), Some(1.0), "the class stays until 1.4 s");
    at(&mut rt, 11.5);
    assert_eq!(rt.state.tab_pulse(id), None, "and then it is gone");
}

#[test]
fn restarts_the_pulse_on_a_second_update() {
    let mut rt = open();
    open_visual(&mut rt, "Cache design", false);
    let id = "visual:Cache design";
    rt.update(|s, _| s.canvases.mark_updated(id, 10.0));
    at(&mut rt, 10.9);
    rt.update(|s, _| s.canvases.mark_updated(id, 10.9));
    at(&mut rt, 11.5);
    assert!((rt.state.tab_pulse(id).unwrap() - 0.5).abs() < 0.01, "counted from the second update");
}

#[test]
fn keeps_drawing_while_a_pulse_runs_and_stops_after() {
    let mut rt = open();
    open_visual(&mut rt, "Cache design", false);
    rt.update(|s, _| s.canvases.mark_updated("visual:Cache design", 20.0));
    rt.update(|s, tx| {
        s.clock = 20.2;
        assert!(s.keep_tab_motion(tx));
        s.clock = 22.0;
        assert!(!s.keep_tab_motion(tx));
    });
}

#[test]
fn slides_a_new_tab_in_once() {
    let mut rt = open();
    assert_eq!(rt.state.canvases.entering, None, "the tabs a session starts with don't slide");
    at(&mut rt, 30.0);
    open_visual(&mut rt, "Cache design", false);
    rt.update(|s, tx| {
        s.keep_strip(tx);
    });
    let entering = rt.state.canvases.entering.clone().expect("the new tab");
    assert_eq!(entering.0, "visual:Cache design");
    at(&mut rt, entering.1 + 0.05);
    let early = rt.state.tab_enter("visual:Cache design").unwrap();
    assert!(early > 0.0 && early < 0.5);
    assert_eq!(rt.state.tab_enter("changes"), None, "only the new one");
    at(&mut rt, entering.1 + 0.4);
    assert_eq!(rt.state.tab_enter("visual:Cache design"), None);
    assert_eq!(rt.state.canvases.entering, None, "the class is dropped, so a redraw doesn't play it again");
}

#[test]
fn scrolls_the_active_tab_into_view_when_the_agent_opens_one_past_the_edge() {
    let mut rt = open();
    for n in 0..12 {
        open_visual(&mut rt, &format!("Design notes number {n}"), false);
    }
    open_visual(&mut rt, "The last one", true);
    for _ in 0..40 {
        rt.frame(FRAME);
    }
    let rect = |rt: &Runtime<crate::State>, key: &str| rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).expect(key).rect;
    let strip = rect(&rt, "canvas-tabs");
    let tab = rect(&rt, "canvas-tab-visual:The last one");
    assert!(tab[0] + tab[2] <= strip[0] + strip[2] + 1.0, "the new tab's right edge is inside the strip");
    assert!(tab[0] >= strip[0] - 1.0, "and its left edge");
}

#[test]
fn has_no_from_mods_group_when_no_mod_declares_a_pane() {
    let mut rt = open();
    click(&mut rt, "canvas-add");
    rt.frame(FRAME);
    assert!(has(&rt, "canvas-picker-context"));
    assert!(!has(&rt, "canvas-picker-visual-0"), "no visuals: no group");
    let words: Vec<String> = rt.elements().into_iter().filter_map(|e| e.text).collect();
    assert!(!words.iter().any(|w| w.to_lowercase().contains("from mods")), "{words:?}");
}

#[test]
fn lists_the_conversations_visuals_newest_first_and_opens_one() {
    let mut rt = open();
    rt.update(|s, _| {
        let id = s.canvas_session();
        s.canvases.set_known_visuals(
            &id,
            vec![visual("Old plan", VisualType::Markdown), visual("Flow", VisualType::Html), visual("Old plan", VisualType::Markdown)],
        );
    });
    open_visual(&mut rt, "Flow", false);
    click(&mut rt, "canvas-add");
    rt.frame(FRAME);
    let rows: Vec<_> = rt.state.conversation_visuals().into_iter().map(|(_, t, open)| (t, open)).collect();
    assert_eq!(rows, [("Old plan".to_string(), false), ("Flow".to_string(), true)], "newest first, one per title, open marked");
    assert!(has(&rt, "canvas-picker-visual-0") && has(&rt, "canvas-picker-visual-1") && !has(&rt, "canvas-picker-visual-2"));
    click(&mut rt, "canvas-picker-visual-0");
    assert_eq!(rt.state.active_canvas(), "visual:Old plan");
}

#[test]
fn scrolls_the_strip_when_it_is_dragged() {
    let mut rt = open();
    for n in 0..12 {
        open_visual(&mut rt, &format!("Design notes number {n}"), false);
    }
    open_visual(&mut rt, "The last one", true);
    for _ in 0..40 {
        rt.frame(FRAME);
    }
    let left =
        |rt: &Runtime<crate::State>| rt.elements().into_iter().find(|e| e.key.as_deref() == Some("canvas-tab-changes")).unwrap().rect[0];
    let before = left(&rt);
    let (x, y) = rt.find("canvas-tabs").unwrap();
    // Dragging the strip to the right brings the tabs that are hidden at its start into view.
    rt.pointer_down(x - 60.0, y);
    for i in 1..=10 {
        rt.pointer_move(x - 60.0 + 20.0 * i as f32, y);
        rt.frame(FRAME);
    }
    rt.pointer_up(x + 140.0, y);
    for _ in 0..10 {
        rt.frame(FRAME);
    }
    assert!(left(&rt) > before + 100.0, "{before} -> {}", left(&rt));
}
