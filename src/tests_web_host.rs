//! The web view host's rules for everything that can go wrong around it (component 14, piece D): overlays of every kind,
//! toasts beside and over the page, Widen, the drop rule, and what the host asks of a page. Headless: `wanted` is the rule a real
//! window's host follows, so these need no window.
use crate::tests::{FRAME, app, click};
use crate::web::{Wanted, wanted};
use crate::{State, ui::Open};
use rune::Runtime;

const BASE: &str = "http://127.0.0.1:6411";
const PAGE: [f32; 4] = [600.0, 60.0, 360.0, 500.0];

/// The page-pick scene with its page open in the panel and an area laid out for it, as a window would report.
fn page_open() -> Runtime<State> {
    let mut rt = app();
    rt.update(|s, _| {
        s.show_page_fallback();
        s.web.mock_base = Some(BASE.into());
        s.edit_canvases(|all, id| all.activate(id, "changes"));
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    click(&mut rt, "page-pick-open");
    rt.update(|s, _| s.web.rect = Some(PAGE));
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(matches!(wanted(&rt.state), Wanted::Shown(_)), "the page is on screen to begin with");
    rt
}

fn settle(rt: &mut Runtime<State>) {
    for _ in 0..30 {
        rt.frame(FRAME);
    }
}

fn hidden(rt: &Runtime<State>) -> bool {
    matches!(wanted(&rt.state), Wanted::Hidden { .. })
}

#[test]
fn every_menu_dropdown_popover_and_dialog_of_the_open_state_hides_the_page_and_closing_it_brings_it_back() {
    let mut rt = page_open();
    for open in [
        Open::Palette,
        Open::Menu("agent"),
        Open::Context(0, 0),
        Open::ProjectMenu(0),
        Open::MachineContext,
        Open::MachineProjectMenu,
        Open::MachineHeadingMenu,
        Open::Select("default-harness"),
        Open::Popover("context"),
        Open::Directory,
        Open::Dialog,
        Open::Alert,
        Open::Sheet(crate::ui::dialog::SheetSide::Right),
    ] {
        rt.update(|s, _| s.ui.open = Some(open));
        rt.frame(FRAME);
        assert!(hidden(&rt), "{open:?} hides the page");
        rt.update(|s, _| s.ui.open = None);
        rt.frame(FRAME);
        settle(&mut rt);
        assert!(matches!(wanted(&rt.state), Wanted::Shown(_)), "{open:?} closed: the page is back");
    }
}

#[test]
fn a_question_a_dialog_and_the_lightbox_hide_the_page() {
    let mut rt = page_open();
    rt.update(|s, tx| {
        s.ask(
            crate::ui::confirm::Confirm::clear_attachments(
                "notes".into(),
                crate::fleet::actions::Call::ClearAttachments { id: "s1".into() },
            ),
            tx,
        )
    });
    rt.frame(FRAME);
    assert!(hidden(&rt), "a confirmation");
    rt.update(|s, _| s.ui.confirm = None);
    rt.frame(FRAME);
    assert!(matches!(wanted(&rt.state), Wanted::Shown(_)), "answered");

    rt.update(|s, tx| s.open_new_project(None, tx));
    rt.frame(FRAME);
    assert!(hidden(&rt), "the new project dialog");
    rt.update(|s, _| s.close_new_project());
    rt.frame(FRAME);
    assert!(matches!(wanted(&rt.state), Wanted::Shown(_)), "closed");

    rt.update(|s, _| s.open_lightbox("attach-scene0", "checkout.png", "attachment-thumb-scene0"));
    rt.frame(FRAME);
    assert!(hidden(&rt), "the lightbox");
}

#[test]
fn a_toast_hides_the_page_only_where_it_is_over_it() {
    let mut rt = page_open();
    rt.update(|s, tx| s.toast("Saved the tide table".into(), None, 30.0, tx));
    settle(&mut rt);
    let toast = rt.presented("toast").expect("the toast is drawn").0;
    // Beside: the page sits above the toast's row.
    rt.update(|s, _| s.web.rect = Some([toast[0], 0.0, toast[2], (toast[1] - 20.0).max(10.0)]));
    rt.frame(FRAME);
    assert!(matches!(wanted(&rt.state), Wanted::Shown(_)), "the toast is below the page: it stays");
    // Over: the page's area holds the toast.
    rt.update(|s, _| s.web.rect = Some([toast[0] - 10.0, toast[1] - 10.0, toast[2] + 20.0, toast[3] + 20.0]));
    rt.frame(FRAME);
    assert!(hidden(&rt), "the toast is over the page: it hides");
    rt.update(|s, _| s.ui.toast = None);
    settle(&mut rt);
    assert!(matches!(wanted(&rt.state), Wanted::Shown(_)), "the toast is gone: the page is back");
}

#[test]
fn the_floating_things_are_read_by_where_they_are() {
    let rt = page_open();
    let cover = &rt.state.web.cover;
    cover.put(&[("toast", [0.0, 700.0, 420.0, 48.0])], false);
    assert!(!cover.over(PAGE));
    cover.put(&[("notice-card", [580.0, 500.0, 300.0, 100.0])], false);
    assert!(cover.over(PAGE));
    cover.put(&[("rail-tip", [960.0, 60.0, 20.0, 20.0])], false);
    assert!(!cover.over(PAGE), "touching the edge isn't over it");
    cover.put(&[], false);
    assert!(!cover.over(PAGE) && !cover.whole());
}

#[test]
fn widen_keeps_the_page_out_of_the_window_while_the_panel_eases_and_then_places_it() {
    let mut rt = page_open();
    rt.update(|s, tx| s.set_widened(true, tx));
    assert!(hidden(&rt), "the width is easing: Rune reports the rest width at once, so the page waits");
    settle(&mut rt);
    assert!(matches!(wanted(&rt.state), Wanted::Shown(_)), "the ease is over");
}

#[test]
fn another_session_another_route_or_a_closed_panel_leave_nothing_to_show() {
    let mut rt = page_open();
    rt.update(|s, _| s.content_open = false);
    assert_eq!(wanted(&rt.state), Wanted::Nothing, "a closed panel");
    rt.update(|s, _| s.content_open = true);
    rt.update(|s, _| s.rail = crate::Rail::Settings);
    assert_eq!(wanted(&rt.state), Wanted::Nothing, "another route");
    rt.update(|s, _| s.rail = crate::Rail::Sessions);
    assert!(matches!(wanted(&rt.state), Wanted::Shown(_)));
    rt.update(|s, _| s.set_open((0, 1)));
    rt.frame(FRAME);
    assert_eq!(wanted(&rt.state), Wanted::Nothing, "another session's panel doesn't hold this page");
}

fn focused(rt: &mut Runtime<State>) -> Option<String> {
    let mut f = None;
    rt.update(|_, tx| f = tx.focused().map(str::to_string));
    f
}

/// The host gives the keyboard back when `tx.focused()` is some element after a page had it (`platform::take_keys_back`).
/// A press on a Rune button that focuses nothing (the rail's buttons) leaves `focused()` empty, and Rune has no app-wide press
/// hook to tell the host about it: that is the open part of row 14.7 on Windows and macOS (Linux hands the keys over with the
/// click itself).
#[test]
fn a_press_on_a_field_gives_it_focus_after_the_page_had_the_keys_and_a_rail_button_does_not() {
    let mut rt = page_open();
    rt.update(|_, tx| tx.blur());
    assert_eq!(focused(&mut rt), None, "the page has the keys: Rune has none");
    click(&mut rt, "composer");
    assert_eq!(focused(&mut rt).as_deref(), Some("composer"));
    rt.update(|_, tx| tx.blur());
    click(&mut rt, "rail-settings");
    assert_eq!(focused(&mut rt), None, "the gap: a button that takes no focus can't tell the host the keys are Rune's again");
}
