//! The panel's tab strip (`CanvasHost.vue`, `CollapsedRightRail.vue`; piece B of docs/parity/10-content-panel.md). Cases carry
//! Fleet's names from `CanvasHost.test.ts`, `CanvasHost.characterise.test.ts` and `CollapsedRightRail.test.ts`.
use crate::fleet::canvases::{CanvasKind, VisualPayload, VisualType};
use crate::tests::{FRAME, app, click};
use rune::{Key, KeyEvent, Mods, Runtime};

fn open() -> Runtime<crate::State> {
    let mut rt = app();
    rt.update(|s, _| s.content_open = true);
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt
}

fn press(rt: &mut Runtime<crate::State>, key: Key) {
    rt.key(KeyEvent { key, mods: Mods::default() });
    rt.frame(FRAME);
}

fn has(rt: &Runtime<crate::State>, key: &str) -> bool {
    rt.find(key).is_some()
}

fn visual(title: &str) -> VisualPayload {
    VisualPayload { kind: VisualType::Markdown, title: Some(title.into()), source_path: None, content: "# A\n".into() }
}

#[test]
fn draws_the_tabs_as_they_always_did() {
    let rt = open();
    for key in ["canvas-tab-changes", "canvas-tab-files", "canvas-add", "canvas-widen", "close-content", "canvas-body"] {
        assert!(has(&rt, key), "{key}");
    }
    assert!(has(&rt, "content") && !has(&rt, "canvas-tab-context"));
}

#[test]
fn shows_changes_and_files_tabs_with_changes_active() {
    let rt = open();
    assert_eq!(rt.state.open_canvases().ids(), ["changes", "files"]);
    assert_eq!(rt.state.active_canvas(), "changes");
    assert!(has(&rt, "changes-canvas"), "Changes' body is piece D's of component 12");
}

#[test]
fn switches_canvases_when_a_tab_is_clicked() {
    let mut rt = open();
    click(&mut rt, "canvas-tab-files");
    assert_eq!(rt.state.active_canvas(), "files");
    click(&mut rt, "canvas-tab-changes");
    assert_eq!(rt.state.active_canvas(), "changes");
}

#[test]
fn adds_a_tab_for_a_document_the_agent_rendered_and_closes_it_again() {
    let mut rt = open();
    rt.update(|s, _| {
        s.edit_canvases(|c, id| c.open_visual(id, visual("Cache design"), true));
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert_eq!(rt.state.active_canvas(), "visual:Cache design");
    click(&mut rt, "canvas-tab-close-visual:Cache design");
    assert_eq!(rt.state.open_canvases().ids(), ["changes", "files"]);
    assert_eq!(rt.state.active_canvas(), "files", "the left neighbour of a tab that was last is the one before it");
}

#[test]
fn does_not_offer_to_close_changes() {
    let rt = open();
    assert!(!has(&rt, "canvas-tab-close-changes"));
    assert!(has(&rt, "canvas-tab-close-files"));
}

#[test]
fn a_middle_click_closes_a_tab_that_can_be_closed() {
    let mut rt = open();
    let (x, y) = rt.find("canvas-tab-files").unwrap();
    rt.pointer_move(x, y);
    rt.button_down(x, y, rune::PointerButton::Middle);
    rt.button_up(x, y, rune::PointerButton::Middle);
    rt.frame(FRAME);
    assert_eq!(rt.state.open_canvases().ids(), ["changes"]);
}

#[test]
fn moves_between_tabs_with_the_arrow_keys() {
    let mut rt = open();
    rt.update(|s, tx| {
        s.open_built_in(CanvasKind::Context, tx);
        s.activate_tab("changes", tx);
    });
    rt.frame(FRAME);
    click(&mut rt, "canvas-tab-changes");
    press(&mut rt, Key::Right);
    assert_eq!(rt.state.active_canvas(), "files");
    press(&mut rt, Key::End);
    assert_eq!(rt.state.active_canvas(), "context");
    press(&mut rt, Key::Right);
    assert_eq!(rt.state.active_canvas(), "changes", "wraps");
    press(&mut rt, Key::Left);
    assert_eq!(rt.state.active_canvas(), "context", "and wraps back");
    press(&mut rt, Key::Home);
    assert_eq!(rt.state.active_canvas(), "changes");
    press(&mut rt, Key::Delete);
    assert_eq!(rt.state.open_canvases().ids().len(), 3, "Changes can't be deleted");
    press(&mut rt, Key::Right);
    press(&mut rt, Key::Delete);
    assert_eq!(rt.state.open_canvases().ids(), ["changes", "context"]);
    assert_eq!(rt.state.active_canvas(), "changes", "the tab that is then active");
}

#[test]
fn shows_an_attention_dot_or_a_count_from_tab_badges() {
    use crate::fleet::canvas_host::Badge;
    let mut rt = open();
    rt.update(|s, _| s.show_tabs(crate::mock::canvas_host::Case::Tabs, true));
    rt.frame(FRAME);
    assert_eq!(rt.state.tab_badge("progress"), Some(Badge::Count("3/7".into(), "3 of 7 done".into())));
    assert_eq!(rt.state.tab_badge("agents"), Some(Badge::Count("1".into(), "1 running or waiting".into())));
    assert_eq!(rt.state.tab_badge("files"), None);
    assert_eq!(rt.state.tab_badge("context"), Some(Badge::Attention("A linked pull request needs attention".into())), "one link fails");
    rt.update(|s, _| s.show_context_tab_scene(crate::mock::context_tab::Case::CardMerged, true));
    assert_eq!(rt.state.tab_badge("context"), Some(Badge::Count("1".into(), "1 linked".into())));
}

#[test]
fn toggles_widen() {
    let mut rt = open();
    assert!(!rt.state.right.widened);
    click(&mut rt, "canvas-widen");
    assert!(rt.state.right.widened);
    click(&mut rt, "canvas-widen");
    assert!(!rt.state.right.widened);
}

#[test]
fn has_no_widen_where_the_panel_already_fills_the_screen() {
    let mut rt = app();
    rt.update(|s, _| {
        s.content_open = true;
        s.sheet_w = 600.0;
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(has(&rt, "content") && !has(&rt, "canvas-widen"));
}

#[test]
fn the_plus_menu_lists_the_built_ins_in_order_and_opens_one_as_the_active_tab() {
    let mut rt = open();
    click(&mut rt, "canvas-add");
    for kind in ["context", "progress", "changes", "files", "turns", "agents"] {
        assert!(has(&rt, &format!("canvas-picker-{kind}")), "{kind}");
    }
    click(&mut rt, "canvas-picker-turns");
    assert!(rt.state.ui.open.is_none(), "the menu closes");
    assert_eq!(rt.state.open_canvases().ids(), ["changes", "files", "turns"]);
    assert_eq!(rt.state.active_canvas(), "turns");
    assert_eq!(rt.state.picker_rows().iter().filter(|(_, open)| *open).count(), 3, "each open one says Open");
}

#[test]
fn a_chip_opens_context_as_the_first_tab_and_its_body_is_drawn() {
    let mut rt = open();
    rt.update(|s, _| s.show_context_tab());
    rt.frame(FRAME);
    assert_eq!(rt.state.open_canvases().ids(), ["context", "changes", "files"]);
    assert!(rt.state.context_visible());
}

#[test]
fn the_progress_strip_opens_the_progress_tab_and_hides_there() {
    let mut rt = open();
    rt.update(|s, _| s.show_tabs(crate::mock::canvas_host::Case::Tabs, true));
    rt.frame(FRAME);
    assert!(has(&rt, "progress-strip"));
    click(&mut rt, "progress-strip");
    assert_eq!(rt.state.active_canvas(), "progress");
    assert!(!has(&rt, "progress-strip"));
}

#[test]
fn shows_the_chevron_the_ring_and_the_count_of_steps_left_when_the_session_has_steps() {
    let rt = app();
    assert!(has(&rt, "content-rail") && has(&rt, "content-rail-ring") && has(&rt, "content-rail-pending"));
}

#[test]
fn leaves_the_ring_out_when_there_are_no_steps() {
    let mut rt = app();
    click(&mut rt, "session-0-1");
    assert!(has(&rt, "content-rail") && !has(&rt, "content-rail-ring"));
}

#[test]
fn asks_to_expand_when_the_rail_is_clicked() {
    let mut rt = app();
    click(&mut rt, "content-rail");
    assert!(rt.state.content_open);
}

#[test]
fn makes_the_button_fill_the_rail_and_stack_from_the_top() {
    let rt = app();
    let (x, y) = rt.find("content-rail").unwrap();
    let (cx, cy) = rt.find("content-rail-ring").unwrap();
    assert!((cx - x).abs() < 1.0, "centred in the rail's width");
    assert!(cy < y, "stacked from the top, not centred down the height");
}
