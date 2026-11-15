//! `composables/__tests__/use-conversation-find.test.ts` under its names, driven at `State` and the headless runtime: the
//! bar's open and close, stepping, the scope chip, the paging of older history and the Ctrl/Cmd+F key.
use super::super::link::{Link, Post};
use super::reconnect::{app, two_sessions};
use super::*;
use crate::State;
use crate::conv::find::Scope;
use rune::{Key, KeyEvent, Mods, Runtime};

/// Three messages in two turns of the conversation: the first turn (u1, a1) folds once it is over.
fn messages() -> Vec<Lifecycle> {
    let mut a1 = life("a1", "assistant", 2, Some("no"), Some("a1-text"));
    let tool = json!({"type": "tool", "id": "a1-tool", "tool": "bash", "callID": "c1", "state": {"status": "completed", "output": "retry in tool"}});
    a1.parts.as_mut().unwrap().push(tool);
    vec![life("u1", "user", 1, Some("retry once"), Some("u1-text")), a1, life("u2", "user", 3, Some("retry twice, retry"), Some("u2-text"))]
}

/// A session open on a (fake) link that goes nowhere, with `messages` and a snapshot that says whether there is more.
fn opened(has_more: bool) -> Runtime<State> {
    let mut rt = app();
    let post = Post::new(|_| {});
    rt.update(move |s, tx| {
        s.link = Some(Link::start("http://127.0.0.1:9".into(), post));
        s.on_list(two_sessions(), tx);
    });
    let id = rt.state.open_id().unwrap();
    let mut snap = snapshot("idle", messages());
    snap["hasMore"] = json!(has_more);
    snap["cursor"] = json!("cursor-1");
    rt.update(move |s, tx| s.on_snapshot(&id, create_state(&snap), tx));
    rt.frame(1.0 / 60.0);
    rt.frame(1.0 / 60.0);
    rt
}

fn type_query(rt: &mut Runtime<State>, query: &str) {
    rt.update(|s, tx| {
        s.find.query.text = query.into();
        s.find_query_changed(tx);
    });
    rt.frame(1.0 / 60.0);
}

fn ctrl_f() -> KeyEvent {
    let mods =
        if cfg!(target_os = "macos") { Mods { cmd: true, ..Mods::default() } } else { Mods { cmd: true, ctrl: true, ..Mods::default() } };
    KeyEvent { key: Key::Command('f'), mods }
}

fn press(rt: &mut Runtime<State>) {
    rt.key(ctrl_f());
    rt.frame(1.0 / 60.0);
}

fn click(rt: &mut Runtime<State>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no element keyed {key:?}"));
    rt.pointer_move(x, y);
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(1.0 / 60.0);
}

fn focused(rt: &Runtime<State>) -> Option<String> {
    rt.elements().into_iter().find(|e| e.focused).and_then(|e| e.key)
}

fn step(rt: &mut Runtime<State>, direction: i32) {
    rt.update(move |s, tx| s.find_step(direction, tx));
}

#[test]
fn steps_through_the_matches_and_wraps_both_ways() {
    let mut rt = opened(false);
    rt.update(|s, tx| s.find_open(tx));
    type_query(&mut rt, "retry");
    assert_eq!(rt.state.find.hits.len(), 3);
    for _ in 0..3 {
        step(&mut rt, 1);
    }
    assert_eq!(rt.state.find.current, 0);
    step(&mut rt, -1);
    assert_eq!(rt.state.find.current, 2);
    assert_eq!(rt.state.find.current_hit().map(|h| h.unit_id.as_str()), Some("u2"));
    // The message is brought into view by the next frame.
    assert_eq!(rt.state.find.jump.as_deref(), Some("u2"));
    rt.frame(1.0 / 60.0);
    rt.frame(1.0 / 60.0);
    assert!(rt.state.find.jump.is_none());
}

#[test]
fn adds_tool_output_with_the_scope_chip_and_reveals_the_folded_turn_it_sits_in() {
    let mut rt = opened(false);
    rt.update(|s, tx| s.find_open(tx));
    assert!(!rt.state.turns_open.contains("u1"), "the first turn is folded");
    type_query(&mut rt, "retry");
    assert_eq!(rt.state.find.hits.len(), 3);
    // The first match is the folded turn's prompt, which carries the turn's id: typing opens it.
    assert!(rt.state.turns_open.contains("u1"));
    rt.update(|s, _| s.turns_open.clear());
    rt.update(|s, _| s.refold());
    rt.update(|s, tx| s.find_set_scope(Scope::Tools, tx));
    assert_eq!(rt.state.find.hits.len(), 4);
    rt.update(|s, _| s.turns_open.clear());
    rt.update(|s, _| s.refold());
    step(&mut rt, 1);
    let hit = rt.state.find.current_hit().unwrap().clone();
    assert_eq!((hit.unit_id.as_str(), hit.turn_id.as_deref()), ("a1", Some("u1")));
    assert!(rt.state.turns_open.contains("u1"), "a match in a folded turn opens it, and it stays open");
    // The chip: Messages sets its own; the other toggles.
    click(&mut rt, "find-scope-messages");
    assert_eq!((rt.state.find.scope, rt.state.find.hits.len()), (Scope::Messages, 3));
    click(&mut rt, "find-scope-tools");
    assert_eq!(rt.state.find.scope, Scope::Tools);
    click(&mut rt, "find-scope-tools");
    assert_eq!(rt.state.find.scope, Scope::Messages);
}

#[test]
fn a_new_query_or_scope_starts_at_the_first_match() {
    let mut rt = opened(false);
    rt.update(|s, tx| s.find_open(tx));
    type_query(&mut rt, "retry");
    step(&mut rt, 1);
    assert_eq!(rt.state.find.current, 1);
    type_query(&mut rt, "retry t");
    assert_eq!(rt.state.find.current, 0);
    step(&mut rt, 1);
    rt.update(|s, tx| s.find_set_scope(Scope::Tools, tx));
    assert_eq!(rt.state.find.current, 0);
}

#[test]
fn loads_older_pages_one_after_another_while_open_and_stops_on_stop_or_close() {
    // The scroll box also asks for older messages near the top, which a short conversation always is: this test drives
    // `find_poll` by hand, with no frames, so only the bar's own asking shows.
    let mut rt = opened(true);
    rt.update(|s, _| s.stream.as_mut().unwrap().is_loading_older = false);
    let loading = |rt: &Runtime<State>| rt.state.stream.as_ref().unwrap().is_loading_older;
    let poll = |rt: &mut Runtime<State>| {
        rt.update(|s, tx| {
            s.find_poll(tx);
        })
    };
    let arrive = |rt: &mut Runtime<State>, cursor: &str, next: &str| {
        let id = rt.state.open_id().unwrap();
        let (cursor, next) = (cursor.to_string(), next.to_string());
        rt.update(move |s, _| s.on_history_page(&id, &cursor, Ok(page(&["m-1"], Some(&next)))));
    };
    poll(&mut rt);
    assert!(!loading(&rt), "nothing loads while the bar is closed");
    rt.update(|s, tx| s.find_open(tx));
    poll(&mut rt);
    assert!(loading(&rt), "opening starts the first page");
    assert_eq!(rt.state.find_paging(), crate::conv::find::Paging::Searching);
    poll(&mut rt);
    arrive(&mut rt, "cursor-1", "cursor-2");
    assert!(!loading(&rt));
    poll(&mut rt);
    assert!(loading(&rt), "each arrival asks for the next");
    // Stop: the page in flight lands, and nothing more is asked.
    rt.update(|s, _| s.find_stop_older());
    arrive(&mut rt, "cursor-2", "cursor-3");
    poll(&mut rt);
    assert!(!loading(&rt));
    assert_eq!(rt.state.find_paging(), crate::conv::find::Paging::Quiet);
    // Closed and opened again: paging starts again; closed, it stops.
    rt.update(|s, tx| s.find_close(tx));
    rt.update(|s, tx| s.find_open(tx));
    poll(&mut rt);
    assert!(loading(&rt));
    arrive(&mut rt, "cursor-3", "cursor-4");
    rt.update(|s, tx| s.find_close(tx));
    poll(&mut rt);
    assert!(!loading(&rt), "a closed bar doesn't page");
}

fn page(ids: &[&str], cursor: Option<&str>) -> super::super::history::Page {
    let messages = ids
        .iter()
        .enumerate()
        .map(|(n, id)| {
            json!({"info": {"id": id, "role": "user", "sessionID": "session-1", "time": {"created": n as i64}},
                   "parts": [{"id": format!("{id}-t"), "sessionID": "session-1", "messageID": id, "type": "text", "text": "an older retry"}]})
        })
        .collect();
    super::super::history::Page { messages, cursor: cursor.map(str::to_string), has_more: cursor.is_some() }
}

#[test]
fn pages_that_arrive_add_their_matches() {
    let mut rt = opened(true);
    rt.update(|s, tx| s.find_open(tx));
    type_query(&mut rt, "retry");
    assert_eq!(rt.state.find.hits.len(), 3);
    let id = rt.state.open_id().unwrap();
    rt.update(move |s, _| s.on_history_page(&id, "cursor-1", Ok(page(&["m-1", "m-2"], None))));
    rt.frame(1.0 / 60.0);
    assert_eq!(rt.state.find.hits.len(), 5, "the older messages' matches are counted, oldest first");
    assert_eq!(rt.state.find.hits[0].unit_id, "m-1");
}

#[test]
fn stops_paging_when_an_older_page_fails_says_so_and_carries_on_after_a_retry() {
    let mut rt = opened(true);
    rt.update(|s, tx| s.find_open(tx));
    rt.frame(1.0 / 60.0);
    let id = rt.state.open_id().unwrap();
    rt.update(move |s, _| s.on_history_page(&id, "cursor-1", Err("timed out".into())));
    rt.frame(1.0 / 60.0);
    let loading = |rt: &Runtime<State>| rt.state.stream.as_ref().unwrap().is_loading_older;
    assert!(!loading(&rt), "a failure stops the paging");
    assert_eq!(rt.state.find_paging(), crate::conv::find::Paging::Failed);
    assert!(rt.find("find-older-failed").is_some() && rt.find("find-older").is_none());
    click(&mut rt, "find-retry-older");
    assert!(loading(&rt), "Try again asks for the page once");
    let id = rt.state.open_id().unwrap();
    rt.update(move |s, _| s.on_history_page(&id, "cursor-1", Ok(page(&["m-1"], Some("cursor-2")))));
    rt.frame(1.0 / 60.0);
    assert!(loading(&rt), "and the paging carries on from there");
    assert_eq!(rt.state.find_paging(), crate::conv::find::Paging::Searching);
}

#[test]
fn closes_to_the_composer() {
    let mut rt = opened(false);
    rt.update(|s, tx| s.find_open(tx));
    rt.frame(1.0 / 60.0);
    assert!(rt.find("find-bar").is_some());
    click(&mut rt, "find-close");
    assert!(!rt.state.find.open);
    assert!(rt.find("find-bar").is_none(), "the bar isn't drawn while closed");
    assert_eq!(focused(&rt).as_deref(), Some("composer"));
}

#[test]
fn escape_in_the_bars_field_closes_it_and_the_query_stays() {
    let mut rt = opened(false);
    press(&mut rt);
    assert_eq!(focused(&rt).as_deref(), Some("find-field"));
    type_query(&mut rt, "retry");
    rt.key(KeyEvent { key: Key::Escape, mods: Mods::default() });
    rt.frame(1.0 / 60.0);
    assert!(!rt.state.find.open);
    assert_eq!(rt.state.find.query.text, "retry");
    // Reopened: the old query is there, selected, and the hits come back.
    press(&mut rt);
    assert_eq!(rt.state.find.hits.len(), 3);
    assert_eq!(rt.state.find.query.selected(), "retry");
}

#[test]
fn enter_steps_to_the_next_match_and_shift_enter_to_the_previous() {
    let mut rt = opened(false);
    press(&mut rt);
    type_query(&mut rt, "retry");
    assert_eq!(focused(&rt).as_deref(), Some("find-field"));
    let enter = |rt: &mut Runtime<State>, shift: bool| {
        rt.key(KeyEvent { key: Key::Enter, mods: Mods { shift, ..Mods::default() } });
        rt.frame(1.0 / 60.0);
    };
    enter(&mut rt, false);
    assert_eq!(rt.state.find.current, 1);
    enter(&mut rt, true);
    enter(&mut rt, true);
    assert_eq!(rt.state.find.current, 2, "wraps back past the first");
}

#[test]
fn ctrl_f_opens_from_the_conversation_or_from_nothing_in_particular_and_takes_the_key() {
    let mut rt = opened(false);
    assert_eq!(focused(&rt), None);
    press(&mut rt);
    assert!(rt.state.find.open);
    assert_eq!(focused(&rt).as_deref(), Some("find-field"));
}

#[test]
fn ctrl_f_selects_the_query_when_pressed_again_inside_the_bars_field() {
    let mut rt = opened(false);
    press(&mut rt);
    type_query(&mut rt, "retry");
    let end = rt.state.find.query.text.len();
    rt.update(move |s, _| s.find.query.set_selection(end, end));
    assert_eq!(rt.state.find.query.selected(), "");
    press(&mut rt);
    assert_eq!(rt.state.find.query.selected(), "retry");
    assert!(rt.state.find.open, "a second press doesn't close it");
}

#[test]
fn ctrl_f_opens_from_the_composer_which_sits_beside_the_conversation_and_leaves_the_draft_alone() {
    let mut rt = opened(false);
    rt.update(|s, _| s.draft = rune::TextEdit::new("half a thought"));
    click(&mut rt, "composer");
    assert_eq!(focused(&rt).as_deref(), Some("composer"));
    press(&mut rt);
    assert!(rt.state.find.open);
    assert_eq!(rt.state.draft.text, "half a thought");
}

#[test]
fn ctrl_f_leaves_the_key_to_a_text_field_the_palette_and_elements_outside_the_conversation() {
    let mut rt = opened(false);
    click(&mut rt, "filter");
    assert_eq!(focused(&rt).as_deref(), Some("filter"));
    press(&mut rt);
    assert!(!rt.state.find.open, "the session filter is a text field");
    rt.update(crate::ui::palette::open);
    rt.frame(1.0 / 60.0);
    press(&mut rt);
    assert!(!rt.state.find.open, "a dialog keeps it");
    rt.update(|s, _| crate::ui::palette::close(s));
    rt.frame(1.0 / 60.0);
    click(&mut rt, "rail-settings");
    press(&mut rt);
    assert!(!rt.state.find.open, "and so does another view");
}

#[test]
fn ctrl_f_stays_shut_when_the_conversation_isnt_the_one_that_takes_the_key() {
    let mut rt = opened(false);
    rt.update(|s, _| s.rail = crate::Rail::Settings);
    rt.frame(1.0 / 60.0);
    press(&mut rt);
    assert!(!rt.state.find.open);
    rt.update(|s, _| s.rail = crate::Rail::Sessions);
    rt.update(|s, _| s.gallery = true);
    press(&mut rt);
    assert!(!rt.state.find.open);
}

#[test]
fn opens_when_the_menu_asks() {
    let mut rt = opened(false);
    rt.update(|s, tx| s.find_request(tx));
    assert!(rt.state.find.open);
    // Another view on screen: the request isn't taken.
    rt.update(|s, tx| {
        s.find_close(tx);
        s.rail = crate::Rail::Settings;
        s.find_request(tx);
    });
    assert!(!rt.state.find.open);
}

#[test]
fn switching_session_closes_the_bar() {
    let mut rt = opened(false);
    rt.update(|s, tx| s.find_open(tx));
    type_query(&mut rt, "retry");
    rt.update(|s, tx| s.open_session(0, 1, tx));
    rt.frame(1.0 / 60.0);
    assert!(!rt.state.find.open);
    assert!(rt.find("find-bar").is_none());
}

#[test]
fn counts_what_the_bar_says_and_disables_the_arrows_without_matches() {
    let mut rt = opened(false);
    press(&mut rt);
    assert_eq!(rt.state.find.count_label(), "");
    type_query(&mut rt, "zzz");
    assert_eq!(rt.state.find.count_label(), "No matches");
    type_query(&mut rt, "retry");
    assert_eq!(rt.state.find.count_label(), "1 of 3");
    step(&mut rt, 1);
    assert_eq!(rt.state.find.count_label(), "2 of 3");
}

#[test]
fn marks_follow_the_hits_text_first_then_the_output_of_calls_and_only_with_the_chip() {
    let mut rt = opened(false);
    rt.update(|s, tx| s.find_open(tx));
    type_query(&mut rt, "retry");
    // Typing found u1's prompt first: its block holds the current mark, the others only the query.
    let m = rt.state.find_marks("u1-text", false).unwrap();
    assert_eq!((m.query.as_str(), m.current), ("retry", Some(0)));
    assert_eq!(rt.state.find_marks("u2-text", false).unwrap().current, None);
    assert!(rt.state.find_marks("a1-tool", true).is_none(), "call output is marked only with the chip, as it is only counted then");
    rt.update(|s, tx| s.find_set_scope(Scope::Tools, tx));
    step(&mut rt, 1);
    assert_eq!(rt.state.find_marks("a1-tool", true).unwrap().current, Some(0));
    assert_eq!(rt.state.find_marks("u1-text", false).unwrap().current, None);
    // The last two are both in u2's words: the second of them is its second mark.
    step(&mut rt, 1);
    step(&mut rt, 1);
    assert_eq!(rt.state.find_marks("u2-text", false).unwrap().current, Some(1));
}

#[test]
fn a_match_in_a_closed_call_opens_its_card_and_no_bar_means_no_marks() {
    let mut rt = opened(false);
    assert!(rt.state.find_marks("u1-text", false).is_none());
    rt.update(|s, tx| s.find_open(tx));
    rt.update(|s, tx| s.find_set_scope(Scope::Tools, tx));
    assert!(!rt.state.tool_open.contains_key("a1-tool"), "the card starts as the call left it: closed");
    type_query(&mut rt, "retry in tool");
    assert_eq!(rt.state.find.current_hit().map(|h| h.kind), Some(crate::conv::find::HitKind::Tool));
    assert!(rt.state.tool_open.get("a1-tool").is_some_and(|t| t.open), "the match is in it, so it opens");
    rt.update(|s, tx| s.find_close(tx));
    assert!(rt.state.find_marks("a1-tool", true).is_none());
}
