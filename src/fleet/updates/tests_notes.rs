//! Component 16, piece U: `release-notes.test.ts` under Fleet's names (as snake case), then what it has no test for.
use super::notes::*;
use super::{compare_versions, parse_chosen_rule};
use serde_json::json;
use std::cmp::Ordering;

// v0.44.0's notes: written highlights, then GitHub's list (names and repositories invented).
const V044: &str = "## Fleet on your phone

Pair a phone from **Settings → Machines → Add a phone**.

- **Known issue:** the `fleet` launcher doesn't accept `--require-token` yet.

## What's Changed
* fix(ui): the UX review's Fix now items (1–10) by @pgermishuys in https://github.com/pgermishuys/weave-fleet/pull/375
* feat(phone): Fleet on your phone: pairing, notifications by @pgermishuys in https://github.com/pgermishuys/weave-fleet/pull/377
* Pin sessions above the projects by @pgermishuys in https://github.com/pgermishuys/weave-fleet/pull/386
* fleet_page_show checks the page by @pgermishuys in https://github.com/pgermishuys/weave-fleet/pull/388

## New Contributors
* @someone made their first contribution in https://github.com/pgermishuys/weave-fleet/pull/370

**Full Changelog**: https://github.com/pgermishuys/weave-fleet/compare/v0.43.0...v0.44.0
";

// A release the way GitHub generates them: no written intro, tests and refactors in among the changes.
const V049: &str = "## What's Changed
* test(preview): hold the stranger port so the app can't be given it by @pgermishuys in https://github.com/pgermishuys/weave-fleet/pull/429
* fix(analytics): open on a range that ends today by @pgermishuys in https://github.com/pgermishuys/weave-fleet/pull/430
* fix(files): show images in a file tab as a picture, not in the editor by @pgermishuys in https://github.com/pgermishuys/weave-fleet/pull/431
* fix(sessions): line up a lone session's title with a parent's under a project by @pgermishuys in https://github.com/pgermishuys/weave-fleet/pull/432
* feat(conversation): open files a reply names in a tab, at the line by @pgermishuys in https://github.com/pgermishuys/weave-fleet/pull/434
* fix(claude-code): keep what the agent left running past an idle stop by @pgermishuys in https://github.com/pgermishuys/weave-fleet/pull/433
* refactor(client): one event hub connection per machine by @pgermishuys in https://github.com/pgermishuys/weave-fleet/pull/414
* docs: Fleet nodes, one interface for many machines by @pgermishuys in https://github.com/pgermishuys/weave-fleet/pull/401


**Full Changelog**: https://github.com/pgermishuys/weave-fleet/compare/v0.48.0...v0.49.0";

fn release(version: &str, body: &str) -> ReleaseNote {
    ReleaseNote {
        version: version.into(),
        published_at: Some("2026-10-07T08:06:17Z".into()),
        body: body.into(),
        url: format!("https://example.test/v{version}"),
    }
}

fn changes(lines: &[&str]) -> String {
    let list: Vec<String> =
        lines.iter().enumerate().map(|(i, l)| format!("* {l} by @someone in https://github.com/o/r/pull/{}", i + 1)).collect();
    format!("## What's Changed\n{}", list.join("\n"))
}

fn pr(n: u64) -> Option<Pr> {
    Some(Pr { number: n, url: format!("https://github.com/pgermishuys/weave-fleet/pull/{n}") })
}

fn change(title: &str, scope: Option<&str>, n: u64) -> ReleaseChange {
    ReleaseChange { title: title.into(), scope: scope.map(str::to_string), pr: pr(n) }
}

// ─── parseReleaseNotes ────────────────────────────────────────────────────────

#[test]
fn keeps_what_was_written_above_the_change_list_and_groups_the_pull_requests_into_new_and_fixed() {
    let notes = parse_release_notes(V044);
    assert_eq!(
        notes.intro,
        "## Fleet on your phone\n\nPair a phone from **Settings → Machines → Add a phone**.\n\n- **Known issue:** the `fleet` launcher doesn't accept `--require-token` yet."
    );
    assert_eq!(
        notes.added,
        [
            change("Fleet on your phone: pairing, notifications", Some("Phone"), 377),
            change("Pin sessions above the projects", None, 386),
            change("fleet_page_show checks the page", None, 388)
        ]
    );
    assert_eq!(notes.fixed, [change("The UX review's Fix now items (1–10)", None, 375)]);
}

#[test]
fn leaves_out_the_full_changelog_and_new_contributors() {
    let markdown = release_notes_markdown(&parse_release_notes(V044));
    assert!(!markdown.contains("Full Changelog"));
    assert!(!markdown.contains("first contribution"));
}

#[test]
fn reads_a_release_with_nothing_but_a_placeholder_as_an_intro() {
    assert_eq!(
        parse_release_notes("Release v0.1.0"),
        ParsedReleaseNotes { intro: "Release v0.1.0".into(), added: vec![], fixed: vec![], internal: 0 }
    );
}

#[test]
fn counts_tests_refactors_and_docs_instead_of_listing_them_as_new() {
    let notes = parse_release_notes(V049);
    assert_eq!(notes.added.iter().map(|c| c.title.as_str()).collect::<Vec<_>>(), ["Open files a reply names in a tab, at the line"]);
    assert_eq!(notes.fixed.len(), 4);
    assert_eq!(notes.internal, 3);
    assert!(!format!("{notes:?}").contains("stranger port"));
}

#[test]
fn keeps_the_scope_as_a_label_but_not_one_that_only_names_a_layer_of_fleet() {
    let notes = parse_release_notes(&changes(&[
        "fix(claude-code): a",
        "feat(status-bar): b",
        "feat(client): c",
        "fix(opencode2): d",
        "Pin sessions above the projects",
    ]));
    let all: Vec<(&str, Option<&str>)> = notes.added.iter().chain(&notes.fixed).map(|c| (c.title.as_str(), c.scope.as_deref())).collect();
    assert_eq!(
        all,
        [
            ("B", Some("Status bar")),
            ("C", None),
            ("Pin sessions above the projects", None),
            ("A", Some("Claude Code")),
            ("D", Some("OpenCode 2"))
        ]
    );
}

// ─── changeSummary, introHighlight ────────────────────────────────────────────

#[test]
fn counts_whats_new_and_whats_fixed() {
    assert_eq!(change_summary(&parse_release_notes(V049)), "1 new, 4 fixed");
    assert_eq!(change_summary(&parse_release_notes(&changes(&["fix: a"]))), "1 fixed");
    assert_eq!(change_summary(&parse_release_notes(&changes(&["test: a"]))), "");
}

#[test]
fn takes_the_first_heading_or_sentence_written_by_hand_as_plain_text() {
    assert_eq!(intro_highlight(&parse_release_notes(V044).intro).as_deref(), Some("Fleet on your phone"));
    assert_eq!(
        intro_highlight("Files a reply names open in a **tab**, see [the docs](https://x.test).").as_deref(),
        Some("Files a reply names open in a tab, see the docs.")
    );
}

#[test]
fn skips_a_known_issue() {
    assert_eq!(intro_highlight("**Known issue (from v0.44.0, still open):** the launcher."), None);
    assert_eq!(intro_highlight(""), None);
}

// ─── whatsNewSummary ──────────────────────────────────────────────────────────

fn releases() -> Vec<ReleaseNote> {
    vec![
        release("0.49.0", V049),
        release(
            "0.48.0",
            &changes(&["feat(composer): pick a skill", "feat(folders): one box to open a folder", "fix: a reply so far", "test: wait"]),
        ),
        release("0.47.1", &changes(&["fix(launcher): accepts --require-token"])),
    ]
}

fn item(kind: ItemKind, text: &str, scope: Option<&str>) -> WhatsNewItem {
    WhatsNewItem { kind, text: text.into(), scope: scope.map(str::to_string) }
}

#[test]
fn lists_whats_new_before_whats_fixed_with_the_rest_counted() {
    let summary = whats_new_summary(&releases(), "0.48.0", "0.49.0", WHATS_NEW_ITEMS).expect("a summary");
    assert_eq!(summary.releases.iter().map(|r| r.version.as_str()).collect::<Vec<_>>(), ["0.49.0"]);
    assert_eq!(
        summary.items,
        [
            item(ItemKind::New, "Open files a reply names in a tab, at the line", Some("Conversation")),
            item(ItemKind::Fixed, "Open on a range that ends today", Some("Analytics")),
            item(ItemKind::Fixed, "Show images in a file tab as a picture, not in the editor", Some("Files")),
        ]
    );
    assert_eq!((summary.more, summary.more_are_fixes), (2, true));
}

#[test]
fn puts_what_was_written_by_hand_first() {
    let written = release("0.49.0", &format!("Files a reply names now open in a tab.\n\n{V049}"));
    let summary = whats_new_summary(&[written], "0.48.0", "0.49.0", WHATS_NEW_ITEMS).expect("a summary");
    assert_eq!(summary.items[0], item(ItemKind::Highlight, "Files a reply names now open in a tab.", None));
}

#[test]
fn covers_every_version_an_update_skipped_over_newest_first() {
    let summary = whats_new_summary(&releases(), "0.47.1", "0.49.0", WHATS_NEW_ITEMS).expect("a summary");
    assert_eq!(summary.releases.iter().map(|r| r.version.as_str()).collect::<Vec<_>>(), ["0.49.0", "0.48.0"]);
    assert_eq!(
        summary.items.iter().map(|i| i.text.as_str()).collect::<Vec<_>>(),
        ["Open files a reply names in a tab, at the line", "Pick a skill", "One box to open a folder"]
    );
    assert_eq!((summary.more, summary.more_are_fixes), (5, true));
}

#[test]
fn is_null_without_the_running_versions_notes_so_the_caller_says_less() {
    let older = &releases()[1..];
    assert_eq!(whats_new_summary(older, "0.48.0", "0.49.0", WHATS_NEW_ITEMS), None);
    assert_eq!(whats_new_summary(&[], "0.48.0", "0.49.0", WHATS_NEW_ITEMS), None);
}

#[test]
fn is_null_when_the_update_has_nothing_but_behind_the_scenes_changes() {
    assert_eq!(whats_new_summary(&[release("0.49.1", &changes(&["test: a", "ci: b"]))], "0.49.0", "0.49.1", WHATS_NEW_ITEMS), None);
}

// ─── releaseNotesMarkdown ─────────────────────────────────────────────────────

#[test]
fn puts_the_intros_headings_under_the_version_and_lists_changes_with_their_pull_request() {
    let markdown = release_notes_markdown(&parse_release_notes(V044));
    assert!(markdown.contains("### Fleet on your phone"));
    assert!(markdown.contains(
        "#### New\n\n- *Phone* Fleet on your phone: pairing, notifications [#377](https://github.com/pgermishuys/weave-fleet/pull/377)"
    ));
    assert!(markdown.contains("#### Fixed\n\n- The UX review's Fix now items (1–10) [#375]"));
}

#[test]
fn puts_a_changes_scope_before_it_for_a_quiet_label() {
    assert!(release_notes_markdown(&parse_release_notes(V049)).contains("- *Analytics* Open on a range that ends today [#430]"));
}

// ─── compareVersions, whatsNewRange ───────────────────────────────────────────

#[test]
fn compares_numerically() {
    assert_eq!(compare_versions("0.10.0", "0.9.0"), Ordering::Greater);
    assert_eq!(compare_versions("v0.46.0", "0.46.0"), Ordering::Equal);
    assert_eq!(compare_versions("0.45.0", "0.46.0"), Ordering::Less);
}

fn all_releases() -> Vec<ReleaseNote> {
    ["0.46.0", "0.45.0", "0.44.0", "0.43.0", "0.42.0", "0.41.0", "0.40.0", "0.39.0", "0.38.0"].iter().map(|v| release(v, "")).collect()
}

fn versions(list: &[ReleaseNote]) -> Vec<&str> {
    list.iter().map(|r| r.version.as_str()).collect()
}

#[test]
fn opens_every_version_newer_than_the_installed_one_and_the_installed_one_and_folds_five_older() {
    let range = whats_new_range(&all_releases(), Some("0.44.0"));
    assert_eq!(versions(&range.open), ["0.46.0", "0.45.0", "0.44.0"]);
    assert_eq!(versions(&range.folded), ["0.43.0", "0.42.0", "0.41.0", "0.40.0", "0.39.0"]);
}

#[test]
fn opens_the_newest_when_the_installed_version_isnt_known() {
    let range = whats_new_range(&all_releases(), None);
    assert_eq!(versions(&range.open), ["0.46.0"]);
    assert_eq!(range.folded.len(), 5);
}

#[test]
fn opens_the_newest_for_a_build_newer_than_every_release_one_built_from_source() {
    let range = whats_new_range(&all_releases(), Some("0.47.0"));
    assert_eq!(versions(&range.open), ["0.46.0"]);
    assert_eq!(versions(&range.folded), ["0.45.0", "0.44.0", "0.43.0", "0.42.0", "0.41.0"]);
}

// ─── not in Fleet's tests ─────────────────────────────────────────────────────

#[test]
fn reads_the_releases_answer_and_a_release_with_no_link_gets_the_github_one() {
    let got = Releases::from_json(&json!({ "releases": [{ "version": "0.59.1", "publishedAt": "2026-10-20T08:00:00Z", "body": "b" }, { "nope": 1 }], "fetchedAt": "2026-10-20T08:01:00Z", "error": "offline" }))
        .expect("releases");
    assert_eq!(got.releases.len(), 1);
    assert_eq!(got.releases[0].url, "https://github.com/pgermishuys/weave-fleet/releases/tag/v0.59.1");
    assert_eq!((got.fetched_at.as_deref(), got.error.as_deref()), (Some("2026-10-20T08:01:00Z"), Some("offline")));
    assert!(Releases::from_json(&json!({ "error": "x" })).is_none());
}

#[test]
fn reads_a_line_that_is_not_githubs_as_a_plain_change_and_keeps_the_case_of_an_identifier() {
    let notes = parse_release_notes(
        "## What's Changed\n- feat!: breaking thing\n* plain line without a link\n* fix(Status_Bar): x by @bot[bot] in https://github.com/o/r/pull/9",
    );
    assert_eq!(notes.added.iter().map(|c| c.title.as_str()).collect::<Vec<_>>(), ["Breaking thing", "Plain line without a link"]);
    assert_eq!(
        notes.fixed,
        [ReleaseChange {
            title: "X".into(),
            scope: Some("Status bar".into()),
            pr: Some(Pr { number: 9, url: "https://github.com/o/r/pull/9".into() })
        }]
    );
    assert_eq!(parse_chosen_rule(None), None);
}

#[test]
fn headings_one_and_two_deep_in_the_intro_go_to_three_and_deeper_ones_stay() {
    let notes = ParsedReleaseNotes { intro: "# One\n## Two\n### Three\ntext # not a heading".into(), ..Default::default() };
    assert_eq!(release_notes_markdown(&notes), "### One\n### Two\n### Three\ntext # not a heading");
}
