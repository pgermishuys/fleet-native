//! Component 16, piece D: Settings → General. Fleet's four `GeneralSection.test.ts` cases under their names, against a fake
//! Fleet that answers `GET /api/preferences` and records `PUT /api/preferences/{key}`; then the wording of every row, a refused
//! save, and the waits a switch has (loading, saving).
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::settings::switches as sw;
use crate::tests::{FRAME, app, click};
use crate::{Rail, State, test_wait};
use rune::Runtime;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub(crate) struct Fake {
    pub(crate) prefs: HashMap<String, String>,
    pub(crate) puts: Vec<(String, String)>,
    /// Keys the server refuses to save.
    pub(crate) refuse: Vec<String>,
}

pub(crate) type Shared = Arc<Mutex<Fake>>;

fn serve(fake: Shared) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a port");
    let base = format!("http://{}", listener.local_addr().expect("an address"));
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let fake = fake.clone();
            std::thread::spawn(move || answer(stream, fake));
        }
    });
    base
}

fn answer(mut stream: std::net::TcpStream, fake: Shared) {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let (head, mut body) = loop {
        let n = stream.read(&mut chunk).unwrap_or(0);
        buf.extend_from_slice(&chunk[..n]);
        if let Some(at) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break (String::from_utf8_lossy(&buf[..at]).to_string(), buf[at + 4..].to_vec());
        }
        if n == 0 {
            return;
        }
    };
    let wanted: usize =
        head.lines().find_map(|l| l.to_lowercase().strip_prefix("content-length:").and_then(|v| v.trim().parse().ok())).unwrap_or(0);
    while body.len() < wanted {
        let n = stream.read(&mut chunk).unwrap_or(0);
        if n == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..n]);
    }
    let mut first = head.lines().next().unwrap_or("").split(' ');
    let (method, path) = (first.next().unwrap_or(""), first.next().unwrap_or(""));
    let mut f = fake.lock().unwrap();
    let (status, reply) = if method == "GET" && path == "/api/preferences" {
        ("200 OK", json!(f.prefs))
    } else if method == "PUT" && path.starts_with("/api/preferences/") {
        let key = path.trim_start_matches("/api/preferences/").replace("%2E", ".");
        let value = serde_json::from_slice::<Value>(&body).ok().and_then(|v| v["value"].as_str().map(str::to_string)).unwrap_or_default();
        f.puts.push((key.clone(), value.clone()));
        if f.refuse.contains(&key) {
            ("500 Error", json!({ "error": "no" }))
        } else {
            f.prefs.insert(key, value);
            ("200 OK", json!({}))
        }
    } else {
        ("200 OK", json!({}))
    };
    let reply = reply.to_string();
    let out = format!(
        "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{reply}",
        reply.len()
    );
    let _ = stream.write_all(out.as_bytes());
}

pub(crate) struct Rig {
    pub(crate) rt: Runtime<State>,
    queue: Receiver<Job>,
    pub(crate) fake: Shared,
}

/// A connected app on Settings → General, with these preferences already read.
fn rig(prefs: &[(&str, &str)]) -> Rig {
    rig_on("general", prefs)
}

/// The same on another Settings page.
pub(crate) fn rig_on(page: &'static str, prefs: &[(&str, &str)]) -> Rig {
    let mut r = unread(prefs);
    r.rt.update(move |s, _| s.open_settings(page, None));
    r.wait("the preferences", |s| s.pickers.prefs.fetched);
    r.frames();
    r
}

/// The same, before the first read: the switches are waiting.
pub(crate) fn unread(prefs: &[(&str, &str)]) -> Rig {
    let fake: Shared =
        Arc::new(Mutex::new(Fake { prefs: prefs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(), ..Default::default() }));
    let base = serve(fake.clone());
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    let link = Link::start(
        base,
        Post::new(move |job| {
            let _ = jobs.lock().unwrap().send(job);
        }),
    );
    let mut rt = app();
    rt.update(move |s, _| {
        s.link = Some(link);
        s.pickers.prefs = Default::default();
        s.rail = Rail::Settings;
        s.settings.set("general", None);
    });
    rt.frame(FRAME);
    Rig { rt, queue, fake }
}

impl Rig {
    pub(crate) fn wait(&mut self, what: &str, done: impl FnMut(&State) -> bool) {
        test_wait::drive(&mut self.rt, &self.queue, what, done, |s| format!("prefs {:?}", s.pickers.prefs));
    }
    pub(crate) fn frames(&mut self) {
        self.rt.frame(FRAME);
        self.rt.frame(FRAME);
    }
    pub(crate) fn wait_put(&mut self, key: &str, value: &str) {
        let fake = self.fake.clone();
        let (k, v) = (key.to_string(), value.to_string());
        self.wait(&format!("{key} = {value} to reach Fleet"), move |_| fake.lock().unwrap().puts.iter().any(|(a, b)| *a == k && *b == v));
        // The answer is read on the UI thread; wait until the key stops saving.
        let key = key.to_string();
        self.wait(&format!("{key} to stop saving"), move |s| !s.setting_saving(&key));
        self.frames();
    }
    pub(crate) fn puts(&self) -> Vec<(String, String)> {
        self.fake.lock().unwrap().puts.clone()
    }
    pub(crate) fn on(&self, switch: sw::Switch) -> bool {
        switch.enabled(&self.rt.state.pickers.prefs)
    }
    pub(crate) fn info(&self, key: &str) -> Option<rune::ElementInfo> {
        self.rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting)
    }
    pub(crate) fn text(&self, key: &str) -> String {
        self.info(key).unwrap_or_else(|| panic!("no element keyed {key:?}")).text.unwrap_or_default()
    }
    /// Everything the page says, joined.
    pub(crate) fn words(&self) -> String {
        self.rt.elements().iter().filter_map(|e| e.text.clone()).collect::<Vec<_>>().join("\n")
    }
}

// ─── GeneralSection.test.ts ─────────────────────────────────────────────────────

#[test]
fn shows_starters_by_default_and_saves_turning_them_off() {
    let mut r = rig(&[]);
    assert_eq!(r.info("starters-switch").unwrap().role, "switch");
    assert!(r.on(sw::STARTERS));
    click(&mut r.rt, "starters-switch");
    assert!(!r.on(sw::STARTERS), "the value shows at once");
    r.wait_put("NewSessionStarters", "false");
    assert!(!r.on(sw::STARTERS));
}

#[test]
fn has_no_usage_limits_switch_and_a_status_bar_switch_that_is_on_until_turned_off() {
    let mut r = rig(&[]);
    assert!(r.info("usage-limits-switch").is_none());
    assert!(r.info("usage-limits-chip-switch").is_some());
    assert!(r.on(sw::USAGE_LIMITS_CHIP));
    click(&mut r.rt, "usage-limits-chip-switch");
    r.wait_put("UsageLimitsChip", "false");
    assert!(!r.rt.state.usage_chip_shown(), "the status bar chip follows the switch");
}

#[test]
fn large_pastes_always_shows_and_saves_the_choice() {
    let mut r = rig(&[]);
    assert!(r.text("large-pastes-select").contains("Ask") || r.words().contains("Ask"));
    assert!(r.words().contains("When you paste 2,000 characters or 25 lines or more."));
    click(&mut r.rt, "large-pastes-select");
    r.frames();
    // The three choices, in Fleet's order and words.
    for words in ["Ask", "Always attach", "Paste as text"] {
        assert!(r.words().contains(words), "{words} is offered");
    }
    let attach = r.rt.elements().into_iter().find(|e| e.text.as_deref() == Some("Always attach")).expect("the choice");
    let (x, y) = (attach.rect[0] + attach.rect[2] / 2.0, attach.rect[1] + attach.rect[3] / 2.0);
    r.rt.pointer_move(x, y);
    r.rt.pointer_down(x, y);
    r.rt.pointer_up(x, y);
    r.frames();
    r.wait_put("LargePastes", "attach");
    assert_eq!(r.rt.state.pickers.prefs.get("LargePastes"), Some("attach"));
    assert!(r.words().contains("Always attach"), "the select shows the choice");
}

#[test]
fn groups_the_everyday_settings_and_keeps_each_full_text_behind_more() {
    let mut r = rig(&[]);
    for id in ["new-sessions", "away", "rail"] {
        assert!(r.info(&format!("settings-group-{id}")).is_some(), "group {id}");
    }
    assert!(r.info("general-section").is_some());
    assert!(!r.words().contains("nothing is sent until you press Enter"));
    click(&mut r.rt, "setting-more-starters");
    assert!(r.text("setting-more-text-starters").contains("nothing is sent until you press Enter"));
}

// ─── ours ───────────────────────────────────────────────────────────────────────

#[test]
fn every_row_says_its_summary_and_its_more_text_word_for_word() {
    let mut r = rig(&[]);
    let rows: [(&str, &str, &str); 9] = [
        (
            "starters",
            "A few sentences to start from on the new-session page.",
            "On the new-session page, offer a few sentences to start from, such as \"Explore the code\". A starter fills the box; nothing is sent until you press Enter.",
        ),
        ("large-pastes", "When you paste 2,000 characters or 25 lines or more.", ""),
        (
            "notifications",
            "When a session needs you or finishes, unless it's the one in front of you.",
            "Tell me when a session needs an answer, or finishes, while I'm looking at something else. Nothing is sent about the session in front of you.",
        ),
        (
            "recap",
            "A one-line recap when a turn finishes out of view. One extra request each.",
            "When a turn finishes while you're looking at something else, write a one-line recap above the composer for when you come back. Each recap is one extra request to the session's model.",
        ),
        (
            "restart-continue",
            "After a restart, picks up the sessions it cut off mid-turn, two at a time.",
            "When Fleet restarts while a session is mid-turn, Fleet sends it a message labelled \"Sent by Fleet after the restart\" that names the call that didn't finish. Only sessions cut off less than 2 hours ago, not waiting on a question, with their folder still there. Two at a time. A notice lists them and has Stop all.",
        ),
        (
            "retry-after-limits",
            "Sends \"Continue where you left off.\" once a rate limit resets.",
            "When a model provider's rate limit or usage limit stops a turn, send \"Continue where you left off.\" when the limit resets, or after a wait when the provider doesn't say. What you queued waits for it.",
        ),
        (
            "usage-chip",
            "The tightest provider limit, grey until 80%.",
            "Fleet asks the providers you've signed in to how much you've used, read-only: Claude automatically, the others after you press Allow. The chip shows the tightest limit, grey until 80%. Turn it off to hide it; nothing else changes.",
        ),
        (
            "board",
            "The Board entry in the rail, with its panels.",
            "Show the Board entry in the left rail and enable the board workspace panels.",
        ),
        ("starters", "", ""),
    ];
    let page = r.words();
    for (id, summary, more) in rows {
        if summary.is_empty() {
            continue;
        }
        assert!(page.contains(summary), "{id}'s summary");
        if more.is_empty() {
            assert!(r.info(&format!("setting-more-{id}")).is_none(), "{id} has no More link");
            continue;
        }
        click(&mut r.rt, &format!("setting-more-{id}"));
        assert_eq!(r.text(&format!("setting-more-text-{id}")), more, "{id}'s More text");
    }
    for title in [
        "Show starters",
        "Large pastes",
        "Desktop notifications",
        "Session recap",
        "Continue cut-off sessions by itself",
        "Try again when a limit resets",
        "Show usage in the status bar",
        "Board",
    ] {
        assert!(page.contains(title), "{title}");
    }
}

#[test]
fn each_switch_is_on_or_off_by_its_default_until_the_server_says_otherwise() {
    let r = rig(&[]);
    let defaults = [
        (sw::STARTERS, true),
        (sw::DESKTOP_NOTIFICATIONS, false),
        (sw::SESSION_RECAP, false),
        (sw::RESTART_CONTINUE_AUTO, false),
        (sw::RETRY_AFTER_LIMITS, true),
        (sw::USAGE_LIMITS_CHIP, true),
        (sw::BOARD, false),
    ];
    for (switch, on) in defaults {
        assert_eq!(r.on(switch), on, "{}", switch.key);
    }
    // "On unless it is false" and "off unless it is true".
    let r = rig(&[("NewSessionStarters", "false"), ("SessionRecap", "true"), ("RetryAfterLimits", "yes"), ("RestartContinueAuto", "yes")]);
    assert!(!r.on(sw::STARTERS) && r.on(sw::SESSION_RECAP) && r.on(sw::RETRY_AFTER_LIMITS) && !r.on(sw::RESTART_CONTINUE_AUTO));
}

#[test]
fn every_switch_saves_its_own_key_and_the_board_comes_to_the_rail() {
    let mut r = rig(&[]);
    let cases = [
        ("notifications-switch", "DesktopNotifications", "true"),
        ("recap-switch", "SessionRecap", "true"),
        ("restart-continue-switch", "RestartContinueAuto", "true"),
        ("retry-after-limits-switch", "RetryAfterLimits", "false"),
        ("board-switch", "features.board.enabled", "true"),
    ];
    for (switch, key, value) in cases {
        click(&mut r.rt, switch);
        r.wait_put(key, value);
    }
    assert!(r.rt.state.rail_list().iter().any(|e| e.rail == Rail::Board), "Board is in the rail");
    assert_eq!(r.info("notifications-switch").unwrap().label.as_deref(), Some("Enable desktop notifications"));
    assert_eq!(r.info("recap-switch").unwrap().label.as_deref(), Some("Enable session recap"));
    assert_eq!(r.info("board-switch").unwrap().label.as_deref(), Some("Enable Board feature"));
}

#[test]
fn desktop_notifications_are_saved_and_the_row_says_this_app_shows_none_yet() {
    let mut r = rig(&[]);
    assert!(r.words().contains("This app doesn't show them yet."));
    click(&mut r.rt, "notifications-switch");
    r.wait_put("DesktopNotifications", "true");
    assert!(r.on(sw::DESKTOP_NOTIFICATIONS));
}

#[test]
fn a_refused_save_puts_the_old_value_back_and_reads_them_again() {
    let mut r = rig(&[]);
    r.fake.lock().unwrap().refuse.push("SessionRecap".into());
    click(&mut r.rt, "recap-switch");
    assert!(r.on(sw::SESSION_RECAP));
    let fake = r.fake.clone();
    r.wait("the refusal", move |s| !s.setting_saving("SessionRecap") && !fake.lock().unwrap().puts.is_empty());
    r.wait("the old value back", |s| !sw::SESSION_RECAP.enabled(&s.pickers.prefs));
    assert!(!r.on(sw::SESSION_RECAP));
}

#[test]
fn the_switches_wait_while_the_preferences_load_and_the_page_asks_again_when_it_opens() {
    let mut r = unread(&[("SessionRecap", "true")]);
    r.frames();
    assert!(r.rt.state.preferences_loading());
    let before = r.info("recap-switch").unwrap();
    assert!(before.opacity < 1.0, "a switch that waits is faded");
    click(&mut r.rt, "recap-switch");
    assert!(r.puts().is_empty(), "a press while loading saves nothing");
    // Opening the page reads them: what another device changed shows.
    r.rt.update(|s, _| s.open_settings("general", None));
    r.wait("the preferences", |s| s.pickers.prefs.fetched);
    r.fake.lock().unwrap().prefs.insert("SessionRecap".into(), "false".into());
    r.rt.update(|s, _| s.open_settings("general", None));
    r.wait("what the phone changed", |s| !sw::SESSION_RECAP.enabled(&s.pickers.prefs));
    r.frames();
    assert!(r.info("recap-switch").unwrap().opacity > 0.9);
}

#[test]
fn a_switch_that_is_saving_does_not_press_again() {
    let mut r = rig(&[]);
    r.rt.update(|s, _| {
        s.settings.saving.insert("SessionRecap".into());
    });
    r.frames();
    click(&mut r.rt, "recap-switch");
    assert!(!r.on(sw::SESSION_RECAP) && r.puts().is_empty());
}

#[test]
fn the_groups_are_anchors_a_link_can_scroll_to() {
    use crate::fleet::settings::anchor::{Settle, settle};
    use crate::fleet::settings_nav::Page;
    for (anchor, key) in [("new-sessions", "settings-group-new-sessions"), ("away", "settings-group-away"), ("rail", "settings-group-rail")]
    {
        assert_eq!(settle(&Page::General, Some(anchor)), Settle::Scroll { key: key.into(), release: true });
    }
}
