//! `lib/__tests__/conversation-find.test.ts` under its names, and the state machine's pure parts. `rangesIn` is DOM-only
//! (it walks text nodes) and belongs to the marks (piece Q); `turnIdsOf` is already in `fold.rs`.
use crate::conv::find::{Find, FindUnit, Focus, HitKind, Paging, Press, Scope, count_occurrences, find_hits, focus_of_key};
use crate::conv::find::{focus_keeps_find_key, is_find_shortcut, press};
use rune::{Key, KeyEvent, Mods};

fn unit(id: &str, text: &str, tool_text: &[&str], turn_id: Option<&str>) -> FindUnit {
    FindUnit {
        id: id.into(),
        turn_id: turn_id.map(str::to_string),
        text: text.into(),
        tool_text: tool_text.iter().map(|t| t.to_string()).collect(),
    }
}

#[test]
fn count_occurrences_ignores_case_and_doesnt_overlap() {
    assert_eq!(count_occurrences("Retry retry RETRY", "retry"), 3);
    assert_eq!(count_occurrences("aaaa", "aa"), 2);
    assert_eq!(count_occurrences("nothing", "retry"), 0);
    assert_eq!(count_occurrences("anything", ""), 0);
    assert_eq!(count_occurrences("", "x"), 0);
}

fn units() -> Vec<FindUnit> {
    vec![
        unit("m1", "Please retry the build", &[], Some("t1")),
        unit("m2", "Retrying now; retry 2", &["retry failed: exit 1"], Some("t1")),
        unit("m3", "done", &[], None),
    ]
}

#[test]
fn find_hits_counts_messages_only_by_default_oldest_first() {
    let hits = find_hits(&units(), "retry", Scope::Messages);
    let at: Vec<(&str, usize)> = hits.iter().map(|h| (h.unit_id.as_str(), h.ordinal)).collect();
    assert_eq!(at, [("m1", 0), ("m2", 0), ("m2", 1)]);
}

#[test]
fn find_hits_adds_tool_output_when_asked_after_the_messages_own_text_and_says_where_it_is() {
    let hits = find_hits(&units(), "retry", Scope::Tools);
    assert_eq!(hits.len(), 4);
    let last = &hits[3];
    assert_eq!((last.unit_id.as_str(), last.ordinal, last.kind, last.turn_id.as_deref()), ("m2", 2, HitKind::Tool, Some("t1")));
}

#[test]
fn find_hits_finds_nothing_for_an_empty_query() {
    assert!(find_hits(&units(), "  ", Scope::Messages).is_empty());
    assert!(find_hits(&units(), "", Scope::Tools).is_empty());
}

#[test]
fn find_hits_uses_the_query_as_typed_once_it_has_a_letter() {
    // Not trimmed: "a " searches for the letter and the space.
    let one = [unit("m", "an a b", &[], None)];
    assert_eq!(find_hits(&one, "a ", Scope::Messages).len(), 1);
    assert_eq!(find_hits(&one, "a", Scope::Messages).len(), 2);
    assert_eq!(find_hits(&one, "b ", Scope::Messages).len(), 0);
}

#[test]
fn ctrl_f_focus_rules_lets_fields_editors_the_terminal_and_dialogs_keep_the_key() {
    // The terminal and an editor are drawn by the content panel (component 10): until then they are fields.
    for keeps in [Focus::Field, Focus::Dialog] {
        assert!(focus_keeps_find_key(keeps), "{keeps:?}");
    }
    assert!(!focus_keeps_find_key(Focus::Conversation));
    // The composer has no Ctrl/Cmd+F of its own: it counts as part of the conversation.
    assert!(!focus_keeps_find_key(Focus::Composer));
    assert!(!focus_keeps_find_key(Focus::Nothing));
    // By element key.
    assert_eq!(focus_of_key(None), Focus::Nothing);
    assert_eq!(focus_of_key(Some("composer")), Focus::Composer);
    assert_eq!(focus_of_key(Some("filter")), Focus::Field);
    assert_eq!(focus_of_key(Some("palette-input")), Focus::Field);
    assert_eq!(focus_of_key(Some("agent-filter")), Focus::Field);
    assert_eq!(focus_of_key(Some("turn-fold-m1")), Focus::Conversation);
    assert_eq!(focus_of_key(Some("session-0-1")), Focus::Elsewhere);
    assert_eq!(focus_of_key(Some("rail-settings")), Focus::Elsewhere);
}

fn ev(mods: Mods) -> KeyEvent {
    KeyEvent { key: Key::Command('f'), mods }
}

#[test]
fn ctrl_f_focus_rules_matches_ctrl_f_or_cmd_f_on_a_mac_and_nothing_with_extra_modifiers() {
    // Off macOS Rune sets `cmd` with `ctrl`; on macOS `cmd` alone is ⌘.
    let ctrl = Mods { cmd: true, ctrl: true, ..Mods::default() };
    let cmd = Mods { cmd: true, ..Mods::default() };
    assert!(is_find_shortcut(&ev(ctrl), false));
    assert!(!is_find_shortcut(&ev(cmd), false), "⌘ is the Super key off a Mac");
    assert!(!is_find_shortcut(&ev(Mods { cmd: true, ctrl: true, logo: true, ..Mods::default() }), false));
    assert!(is_find_shortcut(&ev(Mods { logo: true, ..cmd }), true));
    assert!(!is_find_shortcut(&ev(ctrl), true), "⌃⌘F is another chord");
    assert!(!is_find_shortcut(&ev(Mods { shift: true, ..ctrl }), false), "Ctrl+Shift+F is Find in files");
    assert!(!is_find_shortcut(&ev(Mods { alt: true, ..ctrl }), false));
    assert!(!is_find_shortcut(&KeyEvent { key: Key::Command('g'), mods: ctrl }, false));
}

#[test]
fn the_key_opens_from_the_conversation_the_composer_or_nothing_and_not_from_elsewhere() {
    for focus in [Focus::Nothing, Focus::Conversation, Focus::Composer] {
        assert_eq!(press(false, focus, true, true), Press::Open, "{focus:?}");
    }
    for focus in [Focus::Field, Focus::Dialog, Focus::Elsewhere] {
        assert_eq!(press(false, focus, true, true), Press::Ignore, "{focus:?}");
    }
    // Pressed again in the bar's own field: select the query.
    assert_eq!(press(true, Focus::BarField, true, true), Press::SelectField);
    // A side conversation never takes it, and neither does a view that hides the conversation.
    assert_eq!(press(false, Focus::Nothing, false, true), Press::Ignore);
    assert_eq!(press(false, Focus::Nothing, true, false), Press::Ignore);
}

#[test]
fn the_count_reads_nothing_no_matches_or_n_of_m() {
    let mut find = Find::default();
    assert_eq!(find.count_label(), "");
    find.query.text = "retry".into();
    assert_eq!(find.count_label(), "No matches");
    find.set_hits(find_hits(&units(), "retry", Scope::Tools));
    find.current = 2;
    assert_eq!(find.count_label(), "3 of 4");
    find.query.text = "   ".into();
    assert_eq!(find.count_label(), "", "a blank query counts nothing");
}

#[test]
fn stepping_wraps_both_ways_and_does_nothing_without_matches() {
    let mut find = Find::default();
    assert!(find.step(1).is_none());
    find.set_hits(find_hits(&units(), "retry", Scope::Messages));
    for _ in 0..3 {
        find.step(1);
    }
    assert_eq!(find.current, 0);
    assert_eq!(find.step(-1).map(|h| h.unit_id.as_str()), Some("m2"));
    assert_eq!(find.current, 2);
}

#[test]
fn new_hits_keep_the_current_match_unless_it_ran_off_the_end() {
    let mut find = Find::default();
    find.set_hits(find_hits(&units(), "retry", Scope::Tools));
    find.current = 3;
    find.set_hits(find_hits(&units(), "retry", Scope::Tools));
    assert_eq!(find.current, 3);
    find.set_hits(find_hits(&units(), "retry", Scope::Messages));
    assert_eq!(find.current, 0);
}

#[test]
fn paging_runs_only_while_open_unstopped_and_with_more_to_load() {
    let mut find = Find::default();
    assert_eq!(find.older(true, false), Paging::Quiet, "closed");
    find.open();
    assert_eq!(find.older(false, false), Paging::Quiet, "nothing older");
    assert_eq!(find.older(true, false), Paging::Searching);
    assert_eq!(find.older(true, true), Paging::Failed);
    find.paging_stopped = true;
    assert_eq!(find.older(true, false), Paging::Quiet, "Stop");
    assert_eq!(find.older(true, true), Paging::Quiet, "stopped hides the failure too");
    find.close();
    find.open();
    assert_eq!(find.older(true, false), Paging::Searching, "opening again starts it again");
}

#[test]
fn closing_keeps_the_query_and_scope_and_drops_the_hits() {
    let mut find = Find::default();
    find.open();
    find.query.text = "retry".into();
    find.scope = Scope::Tools;
    find.set_hits(find_hits(&units(), "retry", Scope::Tools));
    find.close();
    assert!(!find.open && find.hits.is_empty());
    assert_eq!((find.query.text.as_str(), find.scope), ("retry", Scope::Tools));
}
