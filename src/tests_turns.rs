//! The Turns tab drawn (component 10, piece J): `TurnsCanvas.test.ts` under its names, and the rules read from
//! `TurnsCanvas.vue`: the filter, which turn is open, the one open file, the narrow rule, and the older line.
use crate::mock::turns::Case;
use crate::tests::{FRAME, app, click};
use crate::tests_machines_page::{exists, words};
use rune::Runtime;

fn shown(case: Case) -> Runtime<crate::State> {
    let mut rt = app();
    rt.update(|s, _| s.show_turns_tab(case, true));
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt
}

#[test]
fn says_the_turns_are_loading_not_no_turns_yet_until_the_sessions_history_arrives() {
    let mut rt = shown(Case::Loading);
    assert!(exists(&rt, "turns-loading") && !exists(&rt, "turns-empty"));
    rt.update(|s, _| s.loading = false);
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(!exists(&rt, "turns-loading"));
    assert_eq!(words(&rt, "turns-empty"), "No turns yet\nThe first prompt of this session starts one.".replace('\n', ""));
}

#[test]
fn asks_the_conversation_to_scroll_to_the_rounds_first_message() {
    let mut rt = shown(Case::Main);
    assert!(rt.state.content_open);
    // The newest round is open to begin with, so its button is the one on screen.
    assert!(exists(&rt, "turn-jump-u5") && !exists(&rt, "turn-jump-u4"));
    click(&mut rt, "turn-jump-u5");
    // The panel steps aside, then the conversation unpins and centres the message.
    assert!(!rt.state.content_open);
}

#[test]
fn lists_every_round_newest_first_with_its_prompt_and_the_totals() {
    let rt = shown(Case::Main);
    assert_eq!(rt.state.turns_tab.turns().iter().map(|t| t.id.as_str()).collect::<Vec<_>>(), ["u5", "u4", "u3", "u2", "u1"]);
    assert_eq!(words(&rt, "turn-prompt-u5"), "What does the paging loop do now?");
    assert_eq!(words(&rt, "turn-prompt-u1"), "run-tests");
    let total = words(&rt, "turns-total");
    assert!(total.starts_with("5 turns") && total.contains('+') && total.contains('\u{2212}'), "{total}");
    assert_eq!(words(&rt, "turns-order"), "Newest first");
}

#[test]
fn the_newest_turn_is_open_until_a_click_overrides_it() {
    let mut rt = shown(Case::Main);
    assert!(exists(&rt, "turn-detail-u5") && !exists(&rt, "turn-detail-u2"));
    click(&mut rt, "turn-row-u2");
    assert!(exists(&rt, "turn-detail-u2") && exists(&rt, "turn-detail-u5"), "a click opens that turn and leaves the other");
    click(&mut rt, "turn-row-u5");
    assert!(!exists(&rt, "turn-detail-u5"), "the choice overrides the default");
    click(&mut rt, "turn-row-u2");
    assert!(!exists(&rt, "turn-detail-u2"));
}

#[test]
fn a_round_that_wrote_nothing_says_so_and_a_round_that_wrote_a_file_lists_it() {
    let mut rt = shown(Case::Main);
    assert_eq!(words(&rt, "turn-note-u5"), "Nothing was written this turn.");
    click(&mut rt, "turn-row-u1");
    assert_eq!(words(&rt, "turn-note-u1"), "Nothing was written this turn \u{2014} 2 tool calls, all reads .");
    click(&mut rt, "turn-row-u2");
    assert!(exists(&rt, "turn-file-u2-0"));
    assert_eq!(words(&rt, "turn-meta-u2").split("Agent").count(), 2, "the agent shows once");
    assert!(words(&rt, "turn-cmd-u2-0").ends_with("failed"));
}

#[test]
fn the_filter_keeps_the_turns_that_wrote_a_file_and_counts_every_turn() {
    let mut rt = shown(Case::Main);
    click(&mut rt, "turns-filter");
    assert!(exists(&rt, "turn-row-u2") && exists(&rt, "turn-row-u3"));
    assert!(!exists(&rt, "turn-row-u1") && !exists(&rt, "turn-row-u4") && !exists(&rt, "turn-row-u5"));
    assert!(words(&rt, "turns-total").starts_with("5 turns"), "the totals stay over every turn");
    click(&mut rt, "turns-filter");
    assert!(exists(&rt, "turn-row-u5"));
}

#[test]
fn a_filter_that_leaves_nothing_says_which_history_it_looked_at() {
    let rt = shown(Case::FilterEmpty);
    assert_eq!(words(&rt, "turns-filter-empty"), "No turn in the loaded history changed a file");
}

#[test]
fn one_file_diff_is_open_across_every_turn_and_a_second_click_closes_it() {
    let mut rt = shown(Case::Main);
    click(&mut rt, "turn-row-u3");
    click(&mut rt, "turn-row-u2");
    click(&mut rt, "turn-file-u2-0");
    assert!(exists(&rt, "turn-diff-u2-0"));
    // The page lays out again a frame after the diff opens, so the next row has moved.
    rt.frame(FRAME);
    rt.frame(FRAME);
    click(&mut rt, "turn-file-u3-0");
    assert!(!exists(&rt, "turn-diff-u2-0") && exists(&rt, "turn-diff-u3-0"), "another file's click closes the first");
    click(&mut rt, "turn-file-u3-0");
    assert!(!exists(&rt, "turn-diff-u3-0"));
}

#[test]
fn older_turns_offer_a_button_only_when_there_are_turns_and_more_to_load() {
    let rt = shown(Case::Older);
    assert_eq!(words(&rt, "turns-older"), "Older turns are not loaded yet.Load older");
    assert!(!exists(&shown(Case::Main), "turns-older"));
    assert!(!exists(&shown(Case::Empty), "turns-older"));
}

#[test]
fn at_300_px_or_less_the_facts_and_the_model_changed_pill_go() {
    let mut rt = shown(Case::Main);
    assert!(exists(&rt, "turn-facts-u5"));
    rt.update(|s, _| s.turns_tab.width = 280.0);
    rt.frame(FRAME);
    assert!(!exists(&rt, "turn-facts-u5"));
}

#[test]
fn derives_again_only_when_what_it_reads_changed() {
    let mut rt = shown(Case::Main);
    let calls = rt.state.turns_tab.cached_calls();
    assert!(calls > 0, "the edit and write calls are kept");
    assert!(!rt.state.keep_turns_tab(), "nothing changed, nothing derived");
}

// Row 4.62: Fleet's deep watch sees a reply stream into an older message; the check here read only the last two.
#[test]
fn derives_again_when_an_older_message_changes() {
    let mut rt = shown(Case::Main);
    rt.update(|s, _| {
        let stream = s.stream.as_mut().expect("the case holds a stream");
        assert!(stream.messages.len() > 3);
        stream.messages[0].completed_at = Some(1);
    });
    assert!(rt.state.keep_turns_tab(), "a change in the first message is seen");
    assert!(!rt.state.keep_turns_tab());
    rt.update(|s, _| s.stream.as_mut().unwrap().last_event_id = Some(99));
    assert!(rt.state.keep_turns_tab(), "so is any event");
}
