//! The shell driven with no window: a runtime built as a headless render builds it, then
//! clicked and typed at by element key. New behaviour gets a test here (or in a `tests.rs`
//! beside its module) before it gets a snapshot.
use crate::mock::{Msg, Status};
use crate::{Rail, State, send, spec};
use rune::{App, Env, Key, KeyEvent, Mods, Runtime, TextEdit};

const FRAME: f64 = 1.0 / 60.0;

fn app() -> Runtime<State> {
    let mut rt = App::new(spec()).headless_runtime((1280.0, 760.0), Env::new(true, 1.0));
    // Two frames: the sheet measures itself on the first and lays out to it on the second.
    rt.frame(0.0);
    rt.frame(FRAME);
    rt
}

fn click(rt: &mut Runtime<State>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no element keyed {key:?}"));
    rt.pointer_move(x, y);
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(FRAME);
}

fn command(rt: &mut Runtime<State>, c: char) {
    rt.key(KeyEvent { key: Key::Command(c), mods: Mods::default() });
    rt.frame(FRAME);
}

fn run(rt: &mut Runtime<State>, seconds: f64) {
    for _ in 0..(seconds / FRAME).ceil() as usize {
        rt.frame(FRAME);
    }
}

#[test]
fn every_scene_draws_a_frame() {
    for (name, scene) in spec().scenes {
        let mut rt = app();
        rt.update(|s, _| scene(s));
        rt.frame(FRAME);
        assert!(rt.find("rail-sessions").is_some(), "scene {name} lost the rail");
        assert!(rt.find("status-bar").is_some(), "scene {name} lost the status bar");
    }
}

#[test]
fn clicking_a_session_opens_it() {
    let mut rt = app();
    assert_eq!(rt.state.open, (0, 0));
    click(&mut rt, "session-0-1");
    assert_eq!(rt.state.open, (0, 1));
}

#[test]
fn clicking_a_project_folds_it() {
    let mut rt = app();
    assert!(rt.find("session-1-0").is_some());
    click(&mut rt, "project-1");
    assert!(!rt.state.projects[1].open);
    assert!(rt.find("session-1-0").is_none());
}

#[test]
fn the_sidebar_key_hides_the_session_list() {
    let mut rt = app();
    assert!(rt.find("session-list").is_some());
    command(&mut rt, 'b');
    assert!(!rt.state.sidebar_open);
    // The list slides out, so give the exit time to finish.
    run(&mut rt, 1.0);
    assert!(rt.find("session-list").is_none());
}

#[test]
fn next_and_previous_wrap_around_the_list() {
    let mut rt = app();
    command(&mut rt, '[');
    let last = rt.state.projects.len() - 1;
    assert_eq!(rt.state.open, (last, rt.state.projects[last].sessions.len() - 1));
    command(&mut rt, ']');
    assert_eq!(rt.state.open, (0, 0));
}

#[test]
fn a_rail_entry_takes_the_whole_context_area() {
    let mut rt = app();
    click(&mut rt, "rail-automations");
    assert_eq!(rt.state.rail, Rail::Automations);
    // The rail rule: no view keeps another view's panel.
    assert!(rt.find("session-list").is_none());
}

#[test]
fn a_sent_message_gets_its_canned_reply() {
    let mut rt = app();
    rt.update(|s, tx| {
        s.open = (0, 1);
        s.draft = TextEdit::new("  Ship it  ");
        send(s, tx);
    });
    let session = &rt.state.projects[0].sessions[1];
    assert!(matches!(session.messages.last(), Some(Msg::User(text)) if text == "Ship it"));
    assert!(session.status == Status::Working);
    assert!(rt.state.draft.text.is_empty());

    run(&mut rt, 3.0);
    let session = &rt.state.projects[0].sessions[1];
    assert!(matches!(session.messages.last(), Some(Msg::Assistant(_))));
    assert!(session.status == Status::Idle);
}

#[test]
fn an_empty_draft_sends_nothing() {
    let mut rt = app();
    let before = rt.state.session().messages.len();
    rt.update(|s, tx| {
        s.draft = TextEdit::new("   ");
        send(s, tx);
    });
    assert_eq!(rt.state.session().messages.len(), before);
}

#[test]
fn escape_interrupts_the_working_session() {
    let mut rt = app();
    assert!(rt.state.session().status == Status::Working);
    rt.key(KeyEvent { key: Key::Escape, mods: Mods::default() });
    rt.frame(FRAME);
    assert!(rt.state.session().status == Status::Idle);
}
