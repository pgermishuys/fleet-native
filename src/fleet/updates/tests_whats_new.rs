//! Component 16, piece X: the words and rules of What's new and of the notices after an update, without a server.
use super::after_update::*;
use super::notes::{ReleaseNote, whats_new_summary};
use super::whats_new::*;

fn note(version: &str, body: &str) -> ReleaseNote {
    ReleaseNote {
        version: version.into(),
        published_at: Some("2026-11-16T08:00:00Z".into()),
        body: body.into(),
        url: format!("u/{version}"),
    }
}

fn list(lines: &[&str]) -> String {
    let rows: Vec<String> =
        lines.iter().enumerate().map(|(i, l)| format!("* {l} by @maker in https://github.com/example/harbor/pull/{}\n", 400 + i)).collect();
    format!("## What's Changed\n{}", rows.concat())
}

#[test]
fn a_version_is_ready_installed_or_available() {
    assert_eq!(badge("0.47.0", Some("0.46.0"), Some("0.47.0")), Some(Badge::ReadyToInstall));
    assert_eq!(badge("0.46.0", Some("0.46.0"), Some("0.47.0")), Some(Badge::Installed));
    assert_eq!(badge("0.48.0", Some("0.46.0"), None), Some(Badge::Available));
    assert_eq!(badge("0.45.0", Some("0.46.0"), None), None);
    assert_eq!(badge("0.45.0", None, None), None, "nothing is installed that we know of");
    assert!(Badge::ReadyToInstall.accent() && Badge::Available.accent() && !Badge::Installed.accent());
}

#[test]
fn the_behind_the_scenes_line_says_one_or_many() {
    assert_eq!(internal_line(1), "And 1 behind-the-scenes change.");
    assert_eq!(internal_line(4), "And 4 behind-the-scenes changes.");
}

#[test]
fn the_empty_and_stale_lines_name_the_reason() {
    assert_eq!(empty_line(Some("Couldn't reach GitHub")), "Fleet couldn't get the release notes (Couldn't reach GitHub).");
    assert_eq!(empty_line(None), "There are no release notes yet.");
    assert_eq!(stale_line("offline", "Nov 9, 2026"), "Couldn't check GitHub for newer notes (offline). These are from Nov 9, 2026.");
}

#[test]
fn a_day_has_its_year_when_asked_or_when_it_isnt_this_one() {
    let now = crate::fleet::work::parse_iso_ms("2026-11-17T12:00:00Z").unwrap();
    assert_eq!(day(Some("2026-11-16T08:00:00Z"), false, now, 0), "Nov 16");
    assert_eq!(day(Some("2026-11-16T08:00:00Z"), true, now, 0), "Nov 16, 2026");
    assert_eq!(day(Some("2025-12-31T08:00:00Z"), false, now, 0), "Dec 31, 2025");
    assert_eq!(day(None, true, now, 0), "");
    assert_eq!(day(Some("not a time"), true, now, 0), "");
}

#[test]
fn a_request_opens_the_version_it_names_and_with_since_every_one_after() {
    let all = vec![note("0.47.0", ""), note("0.46.1", ""), note("0.46.0", ""), note("0.45.0", "")];
    let one = Request { version: Some("0.45.0".into()), since: None };
    assert_eq!(versions_to_open(&one, &all), ["0.45.0"]);
    let since = Request { version: Some("0.47.0".into()), since: Some("0.45.0".into()) };
    assert_eq!(versions_to_open(&since, &all), ["0.47.0", "0.46.1", "0.46.0"]);
    assert!(versions_to_open(&Request::default(), &all).is_empty(), "no version: only the card");
    let missing = Request { version: Some("0.99.0".into()), since: None };
    assert!(versions_to_open(&missing, &all).is_empty(), "a version with no notes opens nothing");
}

#[test]
fn a_short_version_drops_a_zero_patch_and_a_leading_v() {
    assert_eq!(short_version("0.49.0"), "0.49");
    assert_eq!(short_version("0.47.1"), "0.47.1");
    assert_eq!(short_version("v0.50.0"), "0.50");
    assert_eq!(short_version("1.0.0"), "1.0");
    assert_eq!(short_version("0.50.10"), "0.50.10");
}

#[test]
fn versions_are_listed_oldest_first_as_two_or_a_span() {
    assert_eq!(version_list(&["0.49.0", "0.48.0"]), "0.48.0 and 0.49.0");
    assert_eq!(version_list(&["0.49.0", "0.48.0", "0.45.0"]), "0.45.0 to 0.49.0");
}

#[test]
fn the_card_says_what_it_leaves_out() {
    let releases = vec![note("0.37.0", &list(&["feat: one", "fix: two", "fix: three", "fix: four", "fix: five"]))];
    let summary = whats_new_summary(&releases, "0.36.1", "0.37.0", 3).expect("notes");
    let copy = whats_new_copy(&summary, "0.36.1", "0.37.0", "Nov 16");
    assert_eq!(copy.title, "What's new in Fleet 0.37");
    assert_eq!(copy.body, "Updated from 0.36.1 \u{b7} Nov 16");
    assert_eq!(copy.more.as_deref(), Some("+2 more fixes"));
    assert_eq!(whats_new_copy(&summary, "0.36.1", "0.37.0", "").body, "Updated from 0.36.1", "no date, no dot");
}

#[test]
fn several_versions_say_since_and_list_them() {
    let releases = vec![
        note("0.37.0", &list(&["feat: one", "fix: two"])),
        note("0.36.1", &list(&["feat: three", "fix: four", "fix: five"])),
        note("0.36.0", &list(&["feat: old"])),
    ];
    let summary = whats_new_summary(&releases, "0.36.0", "0.37.0", 3).expect("notes");
    let copy = whats_new_copy(&summary, "0.36.0", "0.37.0", "Nov 16");
    assert_eq!(copy.title, "What's new since Fleet 0.36.0");
    assert_eq!(copy.body, "2 updates \u{b7} 0.36.1 and 0.37.0");
    assert_eq!(copy.more.as_deref(), Some("+2 more in 0.36.1 and 0.37.0"));
    let card = whats_new_notice(&summary, "0.36.0", "0.37.0", "Nov 16");
    assert_eq!(card.id, "whats-new:server:0.37.0");
    assert_eq!(card.hold_secs, Some(HOLD_SECS));
    assert_eq!(card.chip.as_deref(), Some("What's new in 0.37"));
    assert_eq!(card.on_chip_click.as_deref(), Some("fleet-update:open-whats-new:0.37.0:0.36.0"), "a skipped version is named");
    assert_eq!(card.actions.iter().map(|a| a.label.as_str()).collect::<Vec<_>>(), ["See all changes", "Close"]);
    assert_eq!(card.actions[0].run, "fleet-update:see-all-changes:0.37.0:0.36.0");
}

#[test]
fn the_quiet_chip_says_updated_to_and_goes_after_twelve_seconds() {
    let n = updated_notice("0.37.0");
    assert_eq!((n.id.as_str(), n.title.as_str()), ("updated:server:0.37.0", "Updated to Fleet 0.37.0. See what's new."));
    assert!(n.quiet);
    assert_eq!((n.chip.as_deref(), n.expires_secs), (Some("Updated to 0.37.0"), Some(UPDATED_CHIP_SECS)));
    assert_eq!(n.on_chip_click.as_deref(), Some("fleet-update:open-whats-new:0.37.0"));
}

#[test]
fn an_update_is_a_newer_version_than_the_one_last_run() {
    assert_eq!(updated_from(Some("0.36.1"), "0.37.0").as_deref(), Some("0.36.1"));
    assert_eq!(updated_from(Some("0.37.0"), "0.37.0"), None, "the same version");
    assert_eq!(updated_from(Some("0.38.0"), "0.37.0"), None, "an older one is a downgrade, not news");
    assert_eq!(updated_from(None, "0.37.0"), None, "the first run says nothing");
    assert_eq!(updated_from(Some(""), "0.37.0"), None);
}

#[test]
fn a_button_names_a_version_and_perhaps_the_one_it_skipped_from() {
    assert_eq!(parse_open("0.37.0"), ("0.37.0", None));
    assert_eq!(parse_open("0.37.0:0.36.0"), ("0.37.0", Some("0.36.0")));
}
