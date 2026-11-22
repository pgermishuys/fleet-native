//! The kit driven with no window: keys and clicks by element key, state read back.
use super::Open;
use crate::{State, spec};
use rune::{App, Env, Key, KeyEvent, Mods, Runtime, TextEdit};

const FRAME: f64 = 1.0 / 60.0;

fn app() -> Runtime<State> {
    let mut rt = App::new(spec()).headless_runtime((1280.0, 760.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.frame(FRAME);
    rt
}

fn key(rt: &mut Runtime<State>, k: Key) {
    rt.key(KeyEvent { key: k, mods: Mods::default() });
    rt.frame(FRAME);
    rt.frame(FRAME);
}

fn ctrl(rt: &mut Runtime<State>, c: char) {
    rt.key(KeyEvent { key: Key::Command(c), mods: Mods::default() });
    rt.frame(FRAME);
    rt.frame(FRAME);
}

fn click(rt: &mut Runtime<State>, k: &str) {
    let (x, y) = rt.find(k).unwrap_or_else(|| panic!("no element keyed {k:?}"));
    rt.pointer_move(x, y);
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(FRAME);
    rt.frame(FRAME);
}

#[test]
fn ctrl_k_opens_and_closes_the_palette() {
    let mut rt = app();
    ctrl(&mut rt, 'k');
    assert_eq!(rt.state.ui.open, Some(Open::Palette));
    assert!(rt.find("palette-dialog").is_some());
    ctrl(&mut rt, 'k');
    assert_eq!(rt.state.ui.open, None);
}

#[test]
fn escape_closes_the_palette() {
    let mut rt = app();
    ctrl(&mut rt, 'k');
    key(&mut rt, Key::Escape);
    assert_eq!(rt.state.ui.open, None);
    assert!(rt.state.ui.palette.query.text.is_empty());
}

#[test]
fn arrows_move_the_highlight_and_wrap() {
    let mut rt = app();
    ctrl(&mut rt, 'k');
    assert_eq!(rt.state.ui.palette.hl, 0);
    key(&mut rt, Key::Down);
    assert_eq!(rt.state.ui.palette.hl, 1);
    key(&mut rt, Key::Up);
    key(&mut rt, Key::Up);
    let n = super::palette::rows(&rt.state).len();
    assert_eq!(rt.state.ui.palette.hl, n - 1, "Up from the first row wraps to the last");
}

#[test]
fn enter_runs_the_highlighted_command_and_closes() {
    let mut rt = app();
    assert!(rt.state.sidebar_open);
    rt.update(|s, tx| {
        super::palette::open(s, tx);
        s.ui.palette.query = TextEdit::new("hide sidebar");
    });
    rt.frame(FRAME);
    key(&mut rt, Key::Enter);
    assert!(!rt.state.sidebar_open, "Hide sidebar ran");
    assert_eq!(rt.state.ui.open, None);
}

#[test]
fn a_command_chosen_in_the_palette_runs_and_no_command_waits_on_a_component_any_more() {
    let mut rt = app();
    rt.update(|s, tx| {
        super::palette::open(s, tx);
        s.ui.palette.query = TextEdit::new("inline tool diffs");
    });
    rt.frame(FRAME);
    key(&mut rt, Key::Enter);
    assert!(rt.state.inline_diffs);
    assert!(rt.state.ui.toast.is_none());
}

#[test]
fn the_filter_keeps_matches_and_the_empty_state_shows() {
    let mut rt = app();
    rt.update(|s, _| s.ui.palette.query = TextEdit::new("zzzz"));
    assert!(super::palette::rows(&rt.state).is_empty());
    rt.update(|s, _| s.ui.palette.query = TextEdit::new("theme"));
    let labels: Vec<_> = super::palette::rows(&rt.state)
        .into_iter()
        .filter_map(|(_, r)| if let super::palette::Row::Cmd(c) = r { Some(c.id) } else { None })
        .collect();
    assert!(labels.contains(&"cycle-theme".to_string()), "{labels:?}");
}

#[test]
fn a_level_has_a_back_row_and_backspace_leaves_it() {
    let mut rt = app();
    rt.update(|s, tx| {
        super::palette::open(s, tx);
        s.ui.palette.level = Some("nav-go-to-session".into());
    });
    rt.frame(FRAME);
    assert!(matches!(super::palette::rows(&rt.state)[0].1, super::palette::Row::Back));
    key(&mut rt, Key::Backspace);
    assert!(rt.state.ui.palette.level.is_none());
}

#[test]
fn disabled_commands_do_not_run() {
    let mut rt = app();
    // No session is open on the blank state, so Interrupt is disabled.
    rt.update(|s, tx| {
        s.set_open((9, 9));
        super::palette::open(s, tx);
        s.ui.palette.query = TextEdit::new("interrupt session");
    });
    rt.frame(FRAME);
    key(&mut rt, Key::Enter);
    assert_eq!(rt.state.ui.open, Some(Open::Palette), "a disabled row leaves the palette open");
}

#[test]
fn the_composer_chip_opens_a_menu_and_a_choice_closes_it() {
    let mut rt = app();
    click(&mut rt, "agent");
    assert_eq!(rt.state.ui.open, Some(Open::Menu("agent")));
    click(&mut rt, "agent-pop-item2");
    assert_eq!(rt.state.choice().agent, "plan");
    assert_eq!(rt.state.ui.open, None);
}

#[test]
fn escape_closes_a_menu() {
    let mut rt = app();
    click(&mut rt, "model");
    key(&mut rt, Key::Escape);
    assert_eq!(rt.state.ui.open, None);
}

#[test]
fn menu_arrows_move_focus_and_enter_chooses() {
    let mut rt = app();
    click(&mut rt, "model");
    // The highlight opens on the selected row (Default); the filter keeps focus while the arrows move it.
    key(&mut rt, Key::Down);
    key(&mut rt, Key::Down);
    key(&mut rt, Key::Enter);
    assert_eq!(rt.state.ui.open, None, "Enter chose an item");
    assert_eq!(rt.state.choice().model, rt.state.lists().models[1].selection_key);
}

#[test]
fn select_opens_and_chooses() {
    let mut rt = app();
    rt.update(|s, _| s.gallery = true);
    rt.frame(FRAME);
    rt.frame(FRAME);
    click(&mut rt, "g-select");
    assert_eq!(rt.state.ui.open, Some(Open::Select("g")));
    click(&mut rt, "g-select-pop-opt1");
    assert_eq!(rt.state.ui.g_select.as_deref(), Some("opus"));
    assert_eq!(rt.state.ui.open, None);
}

#[test]
fn the_switch_and_tabs_respond() {
    let mut rt = app();
    rt.update(|s, _| s.gallery = true);
    rt.frame(FRAME);
    rt.frame(FRAME);
    let before = rt.state.ui.g_switch;
    click(&mut rt, "g-switch");
    assert_ne!(rt.state.ui.g_switch, before);
    click(&mut rt, "g-tab2");
    assert_eq!(rt.state.ui.g_tab, 2);
}

#[test]
fn a_dialog_closes_on_escape_but_an_alert_ignores_a_press_outside() {
    let mut rt = app();
    rt.update(|s, _| {
        s.gallery = true;
        s.ui.open = Some(Open::Dialog);
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    key(&mut rt, Key::Escape);
    assert_eq!(rt.state.ui.open, None);
    rt.update(|s, _| s.ui.open = Some(Open::Alert));
    rt.frame(FRAME);
    rt.frame(FRAME);
    click(&mut rt, "g-alert-overlay");
    assert_eq!(rt.state.ui.open, Some(Open::Alert), "an alert only closes by its buttons");
    click(&mut rt, "g-alert-cancel");
    assert_eq!(rt.state.ui.open, None);
}

#[test]
fn the_status_bar_hint_opens_the_palette() {
    let mut rt = app();
    click(&mut rt, "palette-hint");
    assert_eq!(rt.state.ui.open, Some(Open::Palette));
}

#[test]
fn a_toast_goes_when_its_time_is_up() {
    let mut rt = app();
    rt.update(|s, tx| s.toast("Archived.".into(), Some("Undo".into()), 1.0, tx));
    rt.frame(FRAME);
    assert!(rt.find("toast").is_some());
    for _ in 0..90 {
        rt.frame(FRAME);
    }
    assert!(rt.state.ui.toast.is_none());
}
