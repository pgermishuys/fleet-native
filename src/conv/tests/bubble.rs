//! `MessageBubble.meta-pill.test.ts`, `MessageBubble.command.test.ts` and the cluster rule of `lib/activity-messages.ts`
//! (the mounting ones become the checks on what the view is told).
use crate::conv::bubble::*;
use crate::fleet::model::{ModKind, SlashCommand};

const ALL: [Cluster; 4] = [Cluster::Single, Cluster::First, Cluster::Middle, Cluster::Last];

#[test]
fn shows_on_an_assistant_message_that_ends_its_group() {
    for c in [Cluster::Single, Cluster::Last] {
        assert!(show_meta(false, c), "{c:?}");
    }
}

#[test]
fn is_left_off_an_assistant_message_another_follows_closely() {
    for c in [Cluster::First, Cluster::Middle] {
        assert!(!show_meta(false, c), "{c:?}");
    }
}

#[test]
fn stays_beside_your_own_clustered_messages() {
    for c in ALL {
        assert!(show_meta(true, c), "{c:?}");
    }
}

#[test]
fn names_the_cluster_position_from_its_neighbours() {
    assert_eq!(cluster_position(false, false), Cluster::Single);
    assert_eq!(cluster_position(false, true), Cluster::First);
    assert_eq!(cluster_position(true, true), Cluster::Middle);
    assert_eq!(cluster_position(true, false), Cluster::Last);
}

fn tidy() -> SlashCommand {
    SlashCommand { name: "tidy".into(), arguments: Some("src/auth".into()) }
}

#[test]
fn offers_the_expanded_prompt_behind_show_prompt() {
    let template = "Tidy up the code in src/auth without changing what it does.\n\n1. Read every file in that area.";
    assert!(has_expanded_prompt(template, &tidy()));
}

#[test]
fn offers_no_prompt_when_the_message_is_the_command_itself() {
    assert!(!has_expanded_prompt("/tidy src/auth", &tidy()));
    assert!(!has_expanded_prompt("  /tidy src/auth\n", &tidy()));
}

#[test]
fn shows_a_command_without_arguments_by_name() {
    let init = SlashCommand { name: "init".into(), arguments: None };
    assert!(has_expanded_prompt("Create or update AGENTS.md for this repo.", &init));
    assert!(!has_expanded_prompt("/init", &init));
}

#[test]
fn says_who_sent_or_rewrote_a_message() {
    assert_eq!(mod_mark_text(ModKind::Sent, "Fleet Nudge"), ("Sent by Fleet Nudge".to_string(), "Fleet sent this message, not you."));
    assert_eq!(mod_mark_text(ModKind::Sent, "Tidy").1, "A mod sent this message, not you.");
    assert_eq!(
        mod_mark_text(ModKind::Rewritten, "Tidy"),
        ("Rewritten by Tidy".to_string(), "A mod reworded what you typed before it was sent.")
    );
}

#[test]
fn reads_markdown_only_where_there_is_some() {
    assert!(!looks_like_markdown("What does this repo do?"));
    assert!(looks_like_markdown("Run `cargo test`"));
    assert!(looks_like_markdown("Steps:\n1. one\n2. two"));
    assert!(looks_like_markdown("```\ncode\n```"));
}
