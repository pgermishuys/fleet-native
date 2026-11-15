//! `lib/__tests__/compaction.test.ts` and `components/session/__tests__/CompactionDivider.test.ts` (the divider's own
//! test is in `src/tests.rs`: it clicks). `formatTokens` cases from `lib/format/__tests__/numbers.test.ts`.
use crate::conv::compaction::{CompactionView, describe_compaction, fold_compaction_summaries, format_tokens};
use crate::fleet::model::{Msg, PartData};
use crate::view::conversation::Role;

fn message(id: &str) -> Msg {
    Msg::new(id, "s1", Role::Assistant)
}

fn divider(id: &str, summary: Option<&str>) -> Msg {
    let mut m = message(id);
    m.parts.push(PartData::Compaction {
        id: format!("{id}-c"),
        trigger: Some("auto".into()),
        tokens_before: None,
        tokens_after: None,
        summary: summary.map(str::to_string),
    });
    m
}

fn summary(id: &str, text: &str) -> Msg {
    let mut m = message(id);
    m.compaction_summary = true;
    m.parts.push(PartData::Text { id: format!("{id}-t"), text: text.into() });
    m
}

#[test]
fn says_how_big_the_context_was_before_and_after_when_the_harness_gives_the_sizes() {
    let both = CompactionView { tokens_before: Some(181_000), tokens_after: Some(34_000), ..Default::default() };
    assert_eq!(describe_compaction(&both), "Context compacted \u{b7} 181k \u{2192} 34k tokens");
    let before = CompactionView { tokens_before: Some(35_438), ..Default::default() };
    assert_eq!(describe_compaction(&before), "Context compacted \u{b7} 35.4k tokens before");
    assert_eq!(describe_compaction(&CompactionView::default()), "Context compacted");
}

#[test]
fn folds_a_summary_message_into_the_divider_before_it() {
    let folded = fold_compaction_summaries(&[message("m1"), divider("m2", None), summary("m3", "What we did."), message("m4")]);
    assert_eq!(folded.summaries.get("m2").map(String::as_str), Some("What we did."));
    assert_eq!(folded.hidden.iter().collect::<Vec<_>>(), ["m3"]);
}

#[test]
fn keeps_a_dividers_own_summary_and_still_hides_the_message() {
    let folded = fold_compaction_summaries(&[divider("m1", Some("Own summary")), summary("m2", "Other")]);
    assert!(!folded.summaries.contains_key("m1"));
    assert!(folded.hidden.contains("m2"));
}

#[test]
fn shows_a_summary_whose_divider_is_on_an_older_page_as_it_is() {
    let folded = fold_compaction_summaries(&[summary("m1", "Orphan"), message("m2")]);
    assert!(folded.hidden.is_empty());
}

#[test]
fn counts_tokens_as_fleet_does() {
    for (n, want) in [
        (950.0, "950"),
        (999.6, "1k"),
        (1_000.0, "1k"),
        (17_100.0, "17.1k"),
        (99_940.0, "99.9k"),
        (99_960.0, "100k"),
        (200_000.0, "200k"),
        (999_960.0, "1M"),
        (1_200_000.0, "1.2M"),
        (3_400_000_000.0, "3.4B"),
    ] {
        assert_eq!(format_tokens(n), want, "{n}");
    }
}
