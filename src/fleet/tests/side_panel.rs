//! Component 15, piece B: the side conversation's panel, open. The view halves of `SideConversationPanel.test.ts` ("shows nothing
//! when the session has no side conversation", "Ctrl+/ folds it and opens it again", "leaves Ctrl+/ alone without a side
//! conversation", "keeps it as a session and opens it") and the panel's own behaviour: the header's buttons, × disabled while
//! forking, focus back in the composer, the card's size and the narrow window.
use super::queue::press;
use super::side::{Fx, side_json};
use crate::fleet::side_view::{Case, FOLD_SECS};
use crate::tests::{FRAME, click};
use crate::{State, spec};
use rune::{App, Env, Key, KeyEvent, Mods, Runtime};

fn ctrl_slash() -> Mods {
    Mods { ctrl: true, ..Mods::default() }
}

fn run(rt: &mut Runtime<State>, seconds: f64) {
    for _ in 0..(seconds / FRAME).ceil() as usize {
        rt.frame(FRAME);
    }
}

fn rect(rt: &Runtime<State>, key: &str) -> [f32; 4] {
    rt.presented(key).map(|p| p.0).unwrap_or_else(|| panic!("{key} is drawn"))
}

fn drawn(rt: &Runtime<State>, key: &str) -> bool {
    rt.presented(key).is_some()
}

fn scene(width: f32, height: f32, case: Case) -> Runtime<State> {
    let mut rt = App::new(spec()).headless_runtime((width, height), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.frame(FRAME);
    rt.update(move |s, _| s.show_side(case, true));
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt
}

fn focused(rt: &mut Runtime<State>) -> Option<String> {
    let mut f = None;
    rt.update(|_, tx| f = tx.focused().map(str::to_string));
    f
}

fn taken(rt: &mut Runtime<State>, mods: Mods) -> bool {
    let ev = KeyEvent { key: Key::Command('/'), mods };
    let mut taken = true;
    rt.update(|s, tx| taken = s.side_key(&ev, tx));
    taken
}

// "shows nothing when the session has no side conversation" (nothing is drawn)
#[test]
fn shows_nothing_when_the_session_has_no_side_conversation() {
    let mut fx = Fx::loaded(None);
    fx.rt.frame(FRAME);
    assert!(!drawn(&fx.rt, "side-conversation") && !drawn(&fx.rt, "side-card"));
    let mut fx = Fx::loaded(Some(side_json(false, None)));
    fx.rt.frame(FRAME);
    assert!(drawn(&fx.rt, "side-conversation") && drawn(&fx.rt, "side-card"));
}

// "Ctrl+/ folds it and opens it again"
#[test]
fn ctrl_slash_folds_it_and_opens_it_again() {
    let mut fx = Fx::loaded(Some(side_json(false, None)));
    fx.rt.frame(FRAME);
    let open = rect(&fx.rt, "side-fold")[3];
    assert!(open > 100.0, "the card is open: {open}");
    press(&mut fx.rt, Key::Command('/'), ctrl_slash());
    assert!(fx.st().minimized(), "folded");
    assert!(fx.rt.state.draft.text.is_empty(), "the key was consumed, nothing was typed");
    run(&mut fx.rt, FOLD_SECS + 0.1);
    assert!(rect(&fx.rt, "side-fold")[3] < 2.0, "the card has folded away");
    fx.wait("the PUT", |f| f.called("PUT /side/minimized").len() == 1);
    assert_eq!(fx.called("PUT /side/minimized"), [serde_json::json!({ "minimized": true })]);
    press(&mut fx.rt, Key::Command('/'), ctrl_slash());
    assert!(!fx.st().minimized(), "open again");
    run(&mut fx.rt, FOLD_SECS + 0.1);
    let back = rect(&fx.rt, "side-fold")[3];
    assert!((back - open).abs() < 3.0, "back to its height: {back} of {open}, share {}", fx.rt.state.side_open_share());
    assert_eq!(focused(&mut fx.rt).as_deref(), Some("composer"), "opening puts the caret in the composer");
}

// the fold runs over 280 ms, ease-out: half way it is neither open nor folded
#[test]
fn the_fold_runs_over_280_ms() {
    let mut fx = Fx::loaded(Some(side_json(false, None)));
    fx.rt.frame(FRAME);
    let open = rect(&fx.rt, "side-fold")[3];
    press(&mut fx.rt, Key::Command('/'), ctrl_slash());
    run(&mut fx.rt, 0.1);
    let h = rect(&fx.rt, "side-fold")[3];
    assert!(h > 1.0 && h < open - 1.0, "folding: {h} of {open}");
}

// "leaves Ctrl+/ alone without a side conversation"
#[test]
fn leaves_ctrl_slash_alone_without_a_side_conversation() {
    let mut fx = Fx::loaded(None);
    assert!(!taken(&mut fx.rt, ctrl_slash()), "left for whatever else wants it");
    assert!(fx.st().side.is_none() && fx.called("PUT /side/minimized").is_empty());
}

// no Alt, no Shift; and not inside a code editor, where it comments a line
#[test]
fn ctrl_slash_with_alt_or_shift_or_inside_a_code_editor_is_left_alone() {
    let mut fx = Fx::loaded(Some(side_json(false, None)));
    for mods in [Mods { alt: true, ..ctrl_slash() }, Mods { shift: true, ..ctrl_slash() }] {
        assert!(!taken(&mut fx.rt, mods));
    }
    // The code editor has the keyboard (the poll keeps `code_edit.focused` with where it is).
    fx.rt.update(|s, _| s.code_edit.focused = true);
    assert!(!taken(&mut fx.rt, ctrl_slash()) && !fx.st().minimized());
    fx.rt.update(|s, _| s.code_edit.focused = false);
    assert!(taken(&mut fx.rt, ctrl_slash()) && fx.st().minimized(), "and out of it the key folds it");
}

// "keeps it as a session and opens it" (the button, and the opening)
#[test]
fn keep_as_session_asks_the_server_and_opens_the_kept_session() {
    let mut fx = Fx::loaded(Some(side_json(false, None)));
    fx.rt.frame(FRAME);
    click(&mut fx.rt, "side-keep");
    fx.wait("the keep", |f| !f.called("POST /side/keep").is_empty());
    fx.wait("the kept session to be opened", |f| f.calls().iter().any(|(r, _)| r.starts_with("GET /side-1")));
    assert!(fx.st().side.is_none(), "the panel goes");
}

#[test]
fn the_minimize_button_folds_it_and_the_cross_discards_it() {
    let mut fx = Fx::loaded(Some(side_json(false, None)));
    fx.rt.frame(FRAME);
    click(&mut fx.rt, "side-minimize");
    assert!(fx.st().minimized());
    fx.wait("the PUT", |f| !f.called("PUT /side/minimized").is_empty());
    let mut fx = Fx::loaded(Some(side_json(false, None)));
    fx.rt.frame(FRAME);
    click(&mut fx.rt, "side-close");
    assert!(fx.st().side.is_none() && fx.st().discarded.is_some(), "discarded at once, with Undo");
}

// × is disabled while forking; Keep and Minimize come with the fork
#[test]
fn the_cross_is_disabled_while_forking_and_keep_and_minimize_wait_for_the_fork() {
    let mut rt = scene(1280.0, 760.0, Case::Starting);
    assert!(drawn(&rt, "side-forking") && drawn(&rt, "side-close"));
    assert!(!drawn(&rt, "side-keep") && !drawn(&rt, "side-minimize"));
    click(&mut rt, "side-close");
    assert!(rt.state.side_shown().is_some_and(|st| st.starting.is_some()), "nothing was discarded");
    let mut rt = scene(1280.0, 760.0, Case::Open);
    assert!(!drawn(&rt, "side-forking") && drawn(&rt, "side-keep") && drawn(&rt, "side-minimize"));
    click(&mut rt, "side-minimize");
    assert!(rt.state.side().unwrap().minimized());
}

#[test]
fn the_card_is_42_percent_of_the_window_and_at_most_400() {
    let rt = scene(1280.0, 760.0, Case::Open);
    assert!((rect(&rt, "side-card")[3] - 760.0 * 0.42).abs() < 1.0);
    let rt = scene(1280.0, 1300.0, Case::Open);
    assert!((rect(&rt, "side-card")[3] - 400.0).abs() < 1.0);
    let w = rect(&rt, "side-card")[2];
    assert!(w <= 760.0 + 1.0, "at most the column wide: {w}");
}

#[test]
fn a_window_640_wide_or_narrower_hides_the_hint_and_the_words() {
    let wide = scene(1280.0, 760.0, Case::Open);
    assert!(drawn(&wide, "side-hint"));
    let narrow = scene(640.0, 760.0, Case::Open);
    assert!(!drawn(&narrow, "side-hint"));
    assert!(drawn(&narrow, "side-keep") && rect(&narrow, "side-keep")[2] < rect(&wide, "side-keep")[2] - 20.0, "icons only");
    // 12 px each side instead of 24.
    let left = rect(&narrow, "side-card")[0] - rect(&narrow, "side-conversation")[0];
    assert!((left - 12.0).abs() < 1.0, "{left}");
}

#[test]
fn the_panel_draws_the_side_sessions_messages_and_not_the_sessions() {
    let rt = scene(1280.0, 760.0, Case::Open);
    assert!(drawn(&rt, "side-r1"), "the side conversation's answer");
    assert!(rect(&rt, "side-r1")[1] > rect(&rt, "side-card")[1], "inside the card");
    assert!(drawn(&rt, "timeline") && drawn(&rt, "side-timeline"), "both streams, each under its own key");
}

#[test]
fn the_title_loses_its_btw_prefix() {
    use crate::view::side::plain_title;
    assert_eq!(plain_title("btw: what changed?"), "what changed?");
    assert_eq!(plain_title("btw:   what changed?"), "what changed?");
    assert_eq!(plain_title("what changed?"), "what changed?");
    assert_eq!(plain_title("btw what changed?"), "btw what changed?");
}
