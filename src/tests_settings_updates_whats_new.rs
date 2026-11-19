//! Component 16, piece X: Settings → Updates & about › What's new (`WhatsNewSection.test.ts`, 5) and what Fleet says after an
//! update (`use-update-notices.test.ts`: the cases about What's new and "Updated to …", for the one Fleet native asks). Against a
//! fake Fleet with invented releases; nothing is read from GitHub and nothing is updated.
use crate::fleet::notices::{ActionTone, ItemKind, Notice};
use crate::tests::click;
use crate::tests_updates::{Rig, rig_with};
use crate::{Rail, State};
use rune::Runtime;
use serde_json::{Value, json};

const PR: &str = "https://github.com/example/harbor/pull";

fn body(n: u32, title: &str) -> String {
    format!("## What's Changed\n* {title} by @maker in {PR}/{n}\n")
}

fn release(version: &str, day: &str, body: String) -> Value {
    json!({ "version": version, "publishedAt": format!("2026-10-{day}T08:00:00Z"), "body": body, "url": format!("https://x/v{version}") })
}

fn three() -> Vec<Value> {
    vec![
        release("0.47.0", "08", body(395, "feat(settings): What's new in Fleet")),
        release("0.46.0", "07", body(391, "fix(sessions): dragging sessions works again")),
        release("0.45.0", "06", body(386, "Pin sessions above the projects")),
    ]
}

fn status(current: &str, status: &str, latest: Option<&str>) -> Value {
    json!({ "currentVersion": current, "status": status, "latestVersion": latest, "checkedAt": null, "error": null })
}

/// The page, with the fake answering these releases and this status.
fn page(installed: &str, state: &str, latest: Option<&str>, releases: Vec<Value>, error: Option<&str>) -> Rig {
    let mut r = rig_with(installed, None, false, |f| {
        f.status = Some(status(installed, state, latest));
        f.releases = Some(json!({ "releases": releases, "fetchedAt": "2026-10-08T09:00:00Z", "error": error }));
    });
    // The fake has no harness set up, so the first-run wizard would cover the page.
    r.rt.update(|s, _| s.harness_setup.cloud = true);
    r.wait("the notes and the status to be read", |s| {
        s.updates.releases.get("home").is_some_and(|n| n.loaded) && s.updates.status.get("home").is_some_and(|x| x.status.is_some())
    });
    r.rt.frame(1.0 / 60.0);
    r.rt.frame(1.0 / 60.0);
    r
}

fn text(rt: &Runtime<State>, key: &str) -> String {
    let all = rt.elements();
    let at = all.iter().position(|e| e.key.as_deref() == Some(key) && !e.exiting).unwrap_or_else(|| panic!("no element keyed {key:?}"));
    if let Some(own) = all[at].text.clone().filter(|t| !t.is_empty()) {
        return own;
    }
    let inside = |mut i: usize| {
        while let Some(p) = all[i].parent {
            if p == at {
                return true;
            }
            i = p;
        }
        false
    };
    (0..all.len()).filter(|&i| inside(i)).filter_map(|i| all[i].text.clone()).collect::<Vec<_>>().join(" ")
}

fn rect(rt: &Runtime<State>, key: &str) -> [f32; 4] {
    rt.elements().iter().find(|e| e.key.as_deref() == Some(key)).unwrap_or_else(|| panic!("no element keyed {key:?}")).rect
}

/// Scrolls the page until the element is on screen and has stopped moving (the window is 1900 tall; What's new is the page's
/// last group, and a wheel's scroll glides).
fn reveal(rt: &mut Runtime<State>, key: &str) {
    for _ in 0..60 {
        let before = rect(rt, key);
        if before[1] < 0.0 || before[1] + before[3] > 1880.0 {
            rt.wheel(900.0, 600.0, 400.0);
        }
        for _ in 0..20 {
            rt.frame(crate::tests::FRAME);
        }
        let after = rect(rt, key);
        if after == before && after[1] >= 0.0 && after[1] + after[3] <= 1880.0 {
            return;
        }
    }
    panic!("{key:?} never came to rest on screen");
}

/// What this feature posted, leaving out the machines chip the fake's rollout makes.
fn news(rt: &Runtime<State>) -> Vec<Notice> {
    rt.state
        .notices
        .notices()
        .iter()
        .filter(|n| n.id.starts_with("whats-new:") || n.id.starts_with("updated:") || n.id.starts_with("update:"))
        .cloned()
        .collect()
}

fn has(rt: &Runtime<State>, key: &str) -> bool {
    rt.elements().iter().any(|e| e.key.as_deref() == Some(key) && !e.exiting)
}

// ─── WhatsNewSection.test.ts ─────────────────────────────────────────────────────

#[test]
fn opens_the_version_waiting_to_install_and_the_installed_one_with_their_changes() {
    let r = page("0.46.0", "staged", Some("0.47.0"), three(), None);
    let rt = &r.rt;
    assert!(has(rt, "whats-new-notes-0.47.0") && has(rt, "whats-new-notes-0.46.0"), "the two are open");
    assert!(!has(rt, "whats-new-notes-0.45.0"), "the older one is folded");
    assert!(text(rt, "whats-new-summary-0.47.0").contains("Ready to install"));
    assert!(text(rt, "whats-new-summary-0.46.0").contains("Installed"));
    let older = text(rt, "whats-new-summary-0.45.0");
    assert!(!older.contains("Ready") && !older.contains("Installed") && !older.contains("Available"), "no badge: {older}");
    assert_eq!(text(rt, "whats-new-h-0.47.0-new"), "NEW");
    assert_eq!(text(rt, "whats-new-change-0.47.0-new-0"), "Settings What's new in Fleet #395");
    assert_eq!(text(rt, "whats-new-h-0.46.0-fixed"), "FIXED");
}

#[test]
fn opens_a_pull_request_in_the_browser_not_in_fleet_s_window() {
    let mut r = page("0.46.0", "uptodate", Some("0.46.0"), three(), None);
    reveal(&mut r.rt, "whats-new-change-0.46.0-fixed-0");
    let [x, y, w, h] = rect(&r.rt, "whats-new-change-0.46.0-fixed-0");
    // The pull request ends the line: its number is under the row's right edge.
    let (x, y) = (x + w - 4.0, y + h / 2.0);
    r.rt.pointer_move(x, y);
    r.rt.pointer_down(x, y);
    r.rt.pointer_up(x, y);
    r.rt.frame(1.0 / 60.0);
    assert_eq!(r.rt.take_url_requests(), [format!("{PR}/391")]);
}

#[test]
fn goes_to_settings_updates_and_about_and_scrolls_to_the_version_a_whats_new_link_named() {
    let mut r = page("0.46.0", "uptodate", Some("0.46.0"), three(), None);
    r.rt.update(|s, _| {
        s.rail = Rail::Sessions;
        s.open_whats_new(Some("0.45.0"), None);
    });
    r.wait("the request to be handled", |s| s.updates.whats_new.request.is_none() && s.updates.whats_new.flash.is_some());
    assert_eq!(r.rt.state.rail, Rail::Settings);
    assert_eq!(r.rt.state.settings.page, crate::fleet::settings_nav::Page::Updates);
    assert!(has(&r.rt, "whats-new-notes-0.45.0"), "the row opened");
    assert_eq!(r.rt.state.updates.whats_new.flash.as_ref().map(|f| f.0.as_str()), Some("0.45.0"), "and flashes");
    // The scroll goes out on the next look, once the row has been drawn.
    assert_eq!(r.rt.state.updates.whats_new.scroll.as_deref(), Some("whats-new-version-0.45.0"));
    r.rt.frame(1.0 / 60.0);
    r.rt.frame(1.0 / 60.0);
    assert_eq!(r.rt.state.updates.whats_new.scroll, None, "asked for once");
}

#[test]
fn keeps_showing_the_saved_notes_when_github_can_t_be_reached() {
    let r = page("0.46.0", "uptodate", Some("0.46.0"), three(), Some("Couldn't reach GitHub"));
    assert_eq!(
        text(&r.rt, "whats-new-stale"),
        "Couldn't check GitHub for newer notes (Couldn't reach GitHub). These are from Oct 8, 2026."
    );
    assert_eq!((0..3).filter(|i| has(&r.rt, &format!("whats-new-summary-0.4{}.0", 7 - i))).count(), 3);
}

#[test]
fn points_to_github_when_fleet_has_no_notes_at_all() {
    let r = page("0.46.0", "error", None, vec![], Some("Couldn't reach GitHub"));
    assert!(text(&r.rt, "whats-new-empty").contains("Fleet couldn't get the release notes (Couldn't reach GitHub)."));
    assert!(text(&r.rt, "whats-new-empty").ends_with("They're on GitHub."));
}

// Ours.

#[test]
fn a_row_opens_and_closes_from_its_chevron() {
    let mut r = page("0.46.0", "uptodate", Some("0.46.0"), three(), None);
    assert!(!has(&r.rt, "whats-new-notes-0.45.0"));
    reveal(&mut r.rt, "whats-new-summary-0.45.0");
    click(&mut r.rt, "whats-new-summary-0.45.0");
    assert!(has(&r.rt, "whats-new-notes-0.45.0"));
    reveal(&mut r.rt, "whats-new-summary-0.46.0");
    click(&mut r.rt, "whats-new-summary-0.46.0");
    assert!(!has(&r.rt, "whats-new-notes-0.46.0"), "an open one closes");
}

#[test]
fn a_request_with_since_opens_every_version_an_update_skipped() {
    let mut r = page("0.47.0", "uptodate", Some("0.47.0"), three(), None);
    r.rt.update(|s, _| s.open_whats_new(Some("0.46.0"), Some("0.44.0")));
    r.wait("the request to be handled", |s| s.updates.whats_new.request.is_none());
    assert!(has(&r.rt, "whats-new-notes-0.46.0") && has(&r.rt, "whats-new-notes-0.45.0"));
}

#[test]
fn the_flash_fades_out_and_the_request_with_no_version_goes_to_the_card() {
    let mut r = page("0.46.0", "uptodate", Some("0.46.0"), three(), None);
    r.rt.update(|s, _| s.open_whats_new(None, None));
    r.wait("the request to be handled", |s| s.updates.whats_new.request.is_none());
    assert_eq!(r.rt.state.updates.whats_new.flash, None, "no version, no row to flash");
    r.rt.update(|s, _| s.open_whats_new(Some("0.45.0"), None));
    r.wait("the flash to start", |s| s.updates.whats_new.flash.is_some());
    r.rt.update(|s, _| s.clock += 2.0);
    r.wait("the flash to end", |s| s.updates.whats_new.flash.is_none());
}

// ─── use-update-notices.test.ts ──────────────────────────────────────────────────

fn v037() -> Value {
    release(
        "0.37.0",
        "12",
        "## What's Changed\n\
         * feat(conversation): open files a reply names in a tab by @maker in https://github.com/example/harbor/pull/431\n\
         * fix(files): show images as a picture by @maker in https://github.com/example/harbor/pull/432\n\
         * fix(claude-code): keep what the agent left running by @maker in https://github.com/example/harbor/pull/433\n\
         * fix(api): answer a missing file with a 404 by @maker in https://github.com/example/harbor/pull/434\n\
         * fix(ui): keep the last tab open by @maker in https://github.com/example/harbor/pull/435\n"
            .to_string(),
    )
}

fn v0361() -> Value {
    release(
        "0.36.1",
        "09",
        "## What's Changed\n\
         * feat(skills): pick a skill by @maker in https://github.com/example/harbor/pull/421\n\
         * fix(skills): list the built-in ones by @maker in https://github.com/example/harbor/pull/422\n"
            .to_string(),
    )
}

/// A Fleet that now runs `now` and this device last ran `last` (None: it never ran one), with these notes.
fn restarted(now: &str, state: &str, last: Option<&str>, releases: Vec<Value>) -> Rig {
    let mut r = rig_with(now, None, false, |f| {
        f.status = Some(status(now, state, None));
        f.releases = Some(json!({ "releases": releases, "fetchedAt": null, "error": null }));
    });
    let last = last.map(String::from);
    r.rt.update(move |s, _| {
        s.rail = Rail::Sessions;
        if let Some(last) = last {
            s.sidebar.save_last_run_version("server", &last);
        }
    });
    r.wait("this run's version to be noted", |s| {
        s.after_update_noted() && s.updates.status.get("home").is_some_and(|x| x.status.is_some())
    });
    r
}

fn posted(rt: &Runtime<State>) -> Vec<Notice> {
    news(rt)
}

fn press(r: &mut Rig, run: &str) {
    let run = run.to_string();
    r.rt.update(move |s, _| {
        s.notices.tell(&run);
        s.keep_update_buttons();
    });
}

fn open_card(r: &mut Rig, id: &str) -> Notice {
    r.wait("the card to be posted", |s| s.notices.get(id).is_some());
    r.rt.update(|s, _| s.notices.open_next());
    r.rt.state.notices.open().cloned().expect("an open card")
}

#[test]
fn opens_what_s_new_once_after_restarting_into_a_newer_version_with_the_changes_most_worth_a_look() {
    let mut r = restarted("0.37.0", "uptodate", Some("0.36.1"), vec![v037(), v0361()]);
    let card = open_card(&mut r, "whats-new:server:0.37.0");
    assert_eq!((card.title.as_str(), card.chip.as_deref()), ("What's new in Fleet 0.37", Some("What's new in 0.37")));
    assert_eq!((card.more.as_deref(), card.hold_secs), (Some("+2 more fixes"), Some(20.0)));
    assert!(card.body.as_deref().is_some_and(|b| b.starts_with("Updated from 0.36.1 \u{b7} ")), "{:?}", card.body);
    let items: Vec<_> = card.items.iter().map(|i| (i.kind, i.label.as_deref().unwrap_or(""), i.text.as_str())).collect();
    assert_eq!(
        items,
        [
            (Some(ItemKind::New), "Conversation", "Open files a reply names in a tab"),
            (Some(ItemKind::Fixed), "Files", "Show images as a picture"),
            (Some(ItemKind::Fixed), "Claude Code", "Keep what the agent left running"),
        ]
    );
    assert_eq!(
        card.actions.iter().map(|a| (a.label.as_str(), a.tone)).collect::<Vec<_>>(),
        [("See all changes", ActionTone::Primary), ("Close", ActionTone::Quiet)]
    );
    // See all changes opens What's new at that version, and the card settles into its chip.
    press(&mut r, &card.actions[0].run);
    let request = r.rt.state.updates.whats_new.request.clone().expect("a request");
    assert_eq!((request.version.as_deref(), request.since.as_deref()), (Some("0.37.0"), None));
    assert!(r.rt.state.notices.open().is_none(), "the card settled");
    let chips: Vec<_> = r.rt.state.notices.chips().iter().filter_map(|n| n.chip.clone()).filter(|c| c.starts_with("What's new")).collect();
    assert_eq!(chips, ["What's new in 0.37"]);
}

#[test]
fn shows_what_s_new_once_per_version_not_again_on_the_next_start() {
    let mut r = restarted("0.37.0", "uptodate", Some("0.36.1"), vec![v037()]);
    open_card(&mut r, "whats-new:server:0.37.0");
    assert_eq!(r.rt.state.sidebar.last_run_version("server"), Some("0.37.0"), "this device keeps the version it ran");
    // Fleet starts again on the same device: it kept 0.37.0, so nothing is news.
    let kept = r.rt.state.sidebar.last_run_version("server").map(String::from);
    let again = restarted("0.37.0", "uptodate", kept.as_deref(), vec![v037()]);
    assert!(posted(&again.rt).is_empty());
}

#[test]
fn covers_every_version_an_update_skipped_over_in_one_card() {
    let older = release("0.36.0", "05", "## What's Changed\n* feat: old by @maker in https://github.com/example/harbor/pull/410\n".into());
    let mut r = restarted("0.37.0", "uptodate", Some("0.36.0"), vec![v037(), v0361(), older]);
    let card = open_card(&mut r, "whats-new:server:0.37.0");
    assert_eq!(card.title, "What's new since Fleet 0.36.0");
    assert_eq!(card.body.as_deref(), Some("2 updates \u{b7} 0.36.1 and 0.37.0"));
    assert_eq!(card.more.as_deref(), Some("+4 more in 0.36.1 and 0.37.0"));
    let texts: Vec<_> = card.items.iter().map(|i| i.text.as_str()).collect();
    assert_eq!(texts, ["Open files a reply names in a tab", "Pick a skill", "Show images as a picture"]);
    press(&mut r, &card.actions[0].run);
    let request = r.rt.state.updates.whats_new.request.clone().expect("a request");
    assert_eq!((request.version.as_deref(), request.since.as_deref()), (Some("0.37.0"), Some("0.36.0")));
}

#[test]
fn says_only_updated_to_when_the_notes_for_the_new_version_aren_t_there() {
    let mut r = restarted("0.37.0", "uptodate", Some("0.36.1"), vec![v0361()]);
    r.wait("the quiet chip", |s| s.notices.get("updated:server:0.37.0").is_some());
    assert!(r.rt.state.notices.next_waiting().is_none(), "no card waits");
    assert!(r.rt.state.notices.get("updated:server:0.37.0").is_some_and(|n| n.quiet));
    let asked = r.home.lock().unwrap().releases_asked;
    assert_eq!(asked, 2, "the notes were read once more because they lacked the version");
}

#[test]
fn says_it_updated_after_restarting_into_a_newer_version_then_goes_away() {
    let mut r = restarted("0.37.0", "uptodate", Some("0.36.1"), vec![]);
    r.wait("the quiet chip", |s| s.notices.get("updated:server:0.37.0").is_some());
    let chip = r.rt.state.notices.get("updated:server:0.37.0").cloned().expect("the chip");
    assert_eq!((chip.chip.as_deref(), chip.quiet), (Some("Updated to 0.37.0"), true));
    assert!(r.rt.state.notices.next_waiting().is_none());
    press(&mut r, chip.on_chip_click.as_deref().expect("a click"));
    assert_eq!(r.rt.state.updates.whats_new.request.as_ref().and_then(|q| q.version.as_deref()), Some("0.37.0"));
    r.rt.update(|s, _| {
        let later = s.clock + 60.0;
        s.notices.expire(later);
    });
    assert!(posted(&r.rt).is_empty(), "it went away by itself");
}

#[test]
fn says_nothing_on_the_first_run_or_the_same_version() {
    let r = restarted("0.37.0", "uptodate", None, vec![v037()]);
    assert!(posted(&r.rt).is_empty());
    assert_eq!(r.rt.state.sidebar.last_run_version("server"), Some("0.37.0"));
    let same = restarted("0.37.0", "uptodate", Some("0.37.0"), vec![v037()]);
    assert!(posted(&same.rt).is_empty());
}

#[test]
fn says_nothing_about_this_fleet_s_update_the_machines_chip_covers_it() {
    let r = restarted("0.36.1", "staged", Some("0.36.1"), vec![]);
    assert!(posted(&r.rt).is_empty());
}

#[test]
fn says_nothing_while_it_downloads_or_when_the_desktop_app_manages_updates() {
    let downloading = restarted("0.36.1", "downloading", Some("0.36.1"), vec![]);
    assert!(posted(&downloading.rt).is_empty());
    // A Fleet the app manages never says what's new from the server's version.
    let managed = restarted("0.37.0", "managed", Some("0.36.1"), vec![v037()]);
    assert!(posted(&managed.rt).is_empty());
    assert_eq!(managed.rt.state.sidebar.last_run_version("server"), Some("0.36.1"), "and doesn't note it");
}

#[test]
fn opens_what_s_new_in_fleet_at_that_version_not_github() {
    let mut r = restarted("0.37.0", "uptodate", Some("0.37.0"), vec![v037()]);
    press(&mut r, "fleet-update:whats-new:fleet-updated:home:0.37.0");
    assert_eq!(r.rt.state.updates.whats_new.request.as_ref().and_then(|q| q.version.as_deref()), Some("0.37.0"));
    assert!(r.rt.take_url_requests().is_empty(), "nothing opened in the browser");
}
