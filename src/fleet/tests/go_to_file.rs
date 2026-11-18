//! `stores/go-to-file.ts`: the focus request is taken once, for its session and path only.
use super::super::go_to_file::*;

#[test]
fn takes_the_focus_request_once_for_its_session_and_path() {
    let mut g = GoToFile::default();
    g.ask_focus("s1", "src/a.ts", Some(12));
    assert_eq!(g.take_focus("s2", "src/a.ts"), None);
    assert_eq!(g.take_focus("s1", "src/b.ts"), None);
    assert_eq!(g.take_focus("s1", "src/a.ts"), Some(Some(12)));
    assert_eq!(g.take_focus("s1", "src/a.ts"), None);
}

#[test]
fn a_pick_without_a_line_gives_no_line() {
    let mut g = GoToFile::default();
    g.ask_focus("s1", "a.ts", None);
    assert_eq!(g.take_focus("s1", "a.ts"), Some(None));
}

#[test]
fn shows_and_hides_a_sessions_picker() {
    let mut g = GoToFile::default();
    g.show("s1");
    assert_eq!(g.session.as_deref(), Some("s1"));
    g.hide();
    assert_eq!(g.session, None);
}
