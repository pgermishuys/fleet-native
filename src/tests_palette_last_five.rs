//! Component 26, piece L: the palette's last five commands (rows 2.1-2.6, 4.15). Fleet's `shell-font-scale.test.ts` says the
//! shell's text follows the font-size setting because it is in rem; here that is one zoom, so the test is that every way
//! of changing the size moves the same zoom, and that the palette cases of `use-commands.test.ts` hold for each command.
use crate::State;
use crate::fleet::settings::appearance::FontSize;
use crate::fleet::store::Retention;
use crate::tests::{app, press_key};
use crate::ui::commands;
use rune::{Key, Mods, Runtime};

fn label(rt: &Runtime<State>, id: &str) -> String {
    commands::find(&rt.state, id).expect("a command").label
}

fn ctrl() -> Mods {
    Mods { cmd: true, ctrl: !cfg!(target_os = "macos"), ..Mods::default() }
}

#[test]
fn app_shell_font_sizes_follow_the_font_size_setting() {
    let mut rt = app();
    rt.update(|s, _| s.sidebar.update_appearance(|a| a.font_size = FontSize::L));
    rt.frame(0.1);
    assert!((rt.zoom() - 15.0 / 14.0).abs() < 1e-4, "{}", rt.zoom());
    // The zoom commands go on top of it, as Fleet's percent does on the root size.
    rt.update(|s, tx| commands::run(s, tx, "zoom-in"));
    rt.frame(0.1);
    assert!((rt.zoom() - 15.0 / 14.0 * 1.1).abs() < 1e-4, "{}", rt.zoom());
}

#[test]
fn zoom_in_and_out_step_ten_percent_and_stop_at_70_and_150() {
    let mut rt = app();
    for _ in 0..8 {
        rt.update(|s, tx| commands::run(s, tx, "zoom-in"));
    }
    assert_eq!(rt.state.zoom_pct, 150.0);
    for _ in 0..10 {
        rt.update(|s, tx| commands::run(s, tx, "zoom-out"));
    }
    assert_eq!(rt.state.zoom_pct, 70.0);
}

#[test]
fn the_zoom_keys_do_what_the_commands_do() {
    let mut rt = app();
    press_key(&mut rt, Key::Command('='), ctrl());
    assert_eq!(rt.state.zoom_pct, 110.0);
    press_key(&mut rt, Key::Command('-'), ctrl());
    press_key(&mut rt, Key::Command('-'), ctrl());
    assert_eq!(rt.state.zoom_pct, 90.0);
}

#[test]
fn show_inline_tool_diffs_switches_and_the_label_follows() {
    let mut rt = app();
    assert_eq!(label(&rt, "toggle-diff-view"), "Show inline tool diffs");
    rt.update(|s, tx| commands::run(s, tx, "toggle-diff-view"));
    assert!(rt.state.inline_diffs);
    assert_eq!(label(&rt, "toggle-diff-view"), "Hide inline tool diffs");
    rt.update(|s, tx| commands::run(s, tx, "toggle-diff-view"));
    assert!(!rt.state.inline_diffs);
}

#[test]
fn show_all_sessions_switches_the_filter_and_the_label_follows() {
    let mut rt = app();
    assert_eq!(label(&rt, "toggle-activity-filter"), "Show all sessions");
    rt.update(|s, tx| commands::run(s, tx, "toggle-activity-filter"));
    assert_eq!(rt.state.store.retention, Retention::All);
    assert_eq!(label(&rt, "toggle-activity-filter"), "Show active sessions");
    rt.update(|s, tx| commands::run(s, tx, "toggle-activity-filter"));
    assert_eq!(rt.state.store.retention, Retention::Active);
}

#[test]
fn refresh_sessions_runs_and_is_not_a_placeholder() {
    let mut rt = app();
    rt.update(|s, tx| commands::run(s, tx, "refresh-sessions"));
    // With no server (mock data) there is nothing to read again, and no "isn't in the native app yet" toast.
    assert!(rt.state.ui.toast.is_none());
}

#[test]
fn go_to_file_and_find_in_files_are_not_placeholders() {
    for id in ["go-to-file", "find-in-files"] {
        let c = commands::find(&app().state, id).expect("a command");
        assert!(!matches!(c.run, commands::Run::Waiting(_)), "{id}");
    }
}
