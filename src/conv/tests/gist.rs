use crate::conv::gist::{plain_inline, reasoning_gist};

const THINKING: &str = "The endpoint loads every order and sorts in memory. That's why it grows with the account.\n\nA cursor on (created_at, id) uses the existing index. Offset paging would scan skipped rows.";

#[test]
fn is_the_first_sentence_of_a_finished_block() {
    assert_eq!(reasoning_gist(THINKING, None, false), "The endpoint loads every order and sorts in memory.");
}

#[test]
fn prefers_the_summary_the_harness_sent() {
    assert_eq!(reasoning_gist(THINKING, Some("**Choosing a paging scheme**\n\nMore detail."), false), "**Choosing a paging scheme**");
}

#[test]
fn follows_the_newest_sentence_while_the_model_is_still_writing() {
    assert_eq!(reasoning_gist(THINKING, Some("Ignored while live"), true), "Offset paging would scan skipped rows.");
    assert_eq!(reasoning_gist("First thought. Now I am wri", None, true), "Now I am wri");
}

#[test]
fn takes_markdown_line_markers_off() {
    assert_eq!(reasoning_gist("## Planning\n\nRest", None, false), "Planning");
    assert_eq!(reasoning_gist("- check the index\n- write tests", None, false), "check the index");
    assert_eq!(reasoning_gist("> quoted line", None, false), "quoted line");
    assert_eq!(reasoning_gist("1) first\n2) second", None, false), "first");
    // Seven marks are not a heading, and a dash with no space after it is not a bullet.
    assert_eq!(reasoning_gist("####### seven", None, false), "####### seven");
    assert_eq!(reasoning_gist("-1 is not a bullet", None, false), "-1 is not a bullet");
}

#[test]
fn is_empty_for_an_empty_block_and_cut_short_for_a_very_long_one() {
    assert_eq!(reasoning_gist("  \n ", None, false), "");
    let long = reasoning_gist(&"x".repeat(500), None, false);
    assert_eq!(long.chars().count(), 241);
    assert!(long.ends_with('\u{2026}'));
}

#[test]
fn a_label_drops_the_marks_of_inline_markdown() {
    assert_eq!(plain_inline("**Choosing** a `paging` scheme"), "Choosing a paging scheme");
    // An underscore is kept: it is as often in a name (`snake_case`) as an emphasis mark.
    assert_eq!(plain_inline("snake_case and *more*"), "snake_case and more");
}
