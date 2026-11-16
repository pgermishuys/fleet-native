//! The `#` place's plain rules (`lib/github-reference.ts`, `lib/github-session-source.ts`; rows A6.1, A6.11, A7.9).
use super::feeds::Repository;
use super::places::{find_hash_trigger, remove_label, remove_trigger, repository_for_preset};
use super::request::GitHubPreset;

fn preset(repo: &str, kind: &str) -> GitHubPreset {
    GitHubPreset {
        source_type: kind.into(),
        owner: "acme".into(),
        repo: repo.into(),
        number: 7,
        title: "t".into(),
        body: None,
        html_url: String::new(),
        repo_full_name: format!("acme/{repo}"),
        suggested_branch: None,
    }
}

#[test]
fn find_hash_trigger_opens_at_the_start_or_after_a_space_and_not_inside_a_word() {
    let t = find_hash_trigger("Fix #31", 7).expect("open");
    assert_eq!((t.start, t.end, t.query.as_str()), (4, 7, "31"));
    assert_eq!(find_hash_trigger("#", 1).map(|t| t.query), Some(String::new()));
    assert!(find_hash_trigger("Port it to C#", 13).is_none());
    assert!(find_hash_trigger("Fix #31 now", 11).is_none(), "a space after the query closes it");
    assert!(find_hash_trigger("Fix ##3", 7).is_none());
    assert!(find_hash_trigger("Fix #31", 0).is_none(), "no caret, no trigger");
}

#[test]
fn remove_trigger_leaves_no_double_space_and_trims_at_the_start() {
    let t = find_hash_trigger("Fix #31 now", 7).expect("open");
    assert_eq!(remove_trigger("Fix #31 now", &t), ("Fix now".to_string(), 4));
    let t = find_hash_trigger("#31 now", 3).expect("open");
    assert_eq!(remove_trigger("#31 now", &t), ("now".to_string(), 0));
    let t = find_hash_trigger("Fix #31", 7).expect("open");
    assert_eq!(remove_trigger("Fix #31", &t), ("Fix ".to_string(), 4));
}

#[test]
fn repository_for_preset_matches_the_name_exactly_then_ignoring_case() {
    let repo = |name: &str| Repository { name: name.into(), path: format!("/src/{name}"), parent_root: "/src".into() };
    let list = [repo("Rocket"), repo("rocket")];
    assert_eq!(repository_for_preset(&preset("rocket", "github-issue"), &list).map(|r| r.path.as_str()), Some("/src/rocket"));
    assert_eq!(repository_for_preset(&preset("ROCKET", "github-issue"), &list).map(|r| r.path.as_str()), Some("/src/Rocket"));
    assert!(repository_for_preset(&preset("comet", "github-issue"), &list).is_none());
}

#[test]
fn the_remove_button_says_what_it_removes() {
    assert_eq!(remove_label(&preset("r", "github-issue")), "Remove issue #7");
    assert_eq!(remove_label(&preset("r", "github-pull-request")), "Remove pull request #7");
}
