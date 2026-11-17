//! The notice card's scenes (piece 9L): the four looks of `NoticeCard.vue`, each posted and opened the way the store
//! would at a quiet moment. Invented text throughout.
use crate::State;
use crate::fleet::notices::{Action, ActionTone, IconRef, Item, ItemKind, Link, Notice, Tone};
use crate::icons::{LIGHTBULB, ROTATE_CW, UNDO_2};

#[derive(Clone, Copy)]
pub enum Case {
    /// A title, a line and a link.
    Plain,
    /// A list to read, with a "more" line and buttons.
    Items,
    /// The amber tint, for a card that asks before something stops work.
    Warn,
    /// A card with an Undo that only lasts the hold, and its bar draining.
    Countdown,
}

fn action(label: &str, tone: ActionTone, run: &str) -> Action {
    Action { label: label.into(), icon: None, tone, run: run.into() }
}

fn item(label: Option<&str>, text: &str, kind: ItemKind) -> Item {
    Item { text: text.into(), label: label.map(Into::into), kind: Some(kind) }
}

pub fn notice(case: Case) -> Notice {
    match case {
        Case::Plain => Notice {
            body: Some("Installs when you restart.".into()),
            link: Some(Link { label: "What's new".into(), href: "https://example.test/notes".into(), run: None }),
            ..Notice::new("scene:plain", "Fleet 0.37.0 is ready").chip("Fleet 0.37.0 ready")
        },
        Case::Items => Notice {
            items: vec![
                item(Some("Sessions"), "Pick up where a restart cut a session off.", ItemKind::New),
                item(Some("Composer"), "Drafts are kept for every session.", ItemKind::Highlight),
                item(None, "The list no longer jumps while a session is archived.", ItemKind::Fixed),
            ],
            more: Some("+2 more fixes".into()),
            actions: vec![action("Later", ActionTone::Quiet, "later"), action("Restart now", ActionTone::Primary, "restart")],
            ..Notice::new("scene:items", "Fleet 0.37.0 is ready").chip("Fleet 0.37.0 ready")
        },
        Case::Warn => Notice {
            icon: Some(IconRef(&ROTATE_CW)),
            tone: Tone::Warn,
            body: Some("3 sessions were cut off.".into()),
            actions: vec![action("Continue all", ActionTone::Primary, "continue"), action("Dismiss all", ActionTone::Quiet, "dismiss")],
            ..Notice::new("scene:warn", "Fleet restarted").chip("3 sessions cut off")
        },
        Case::Countdown => Notice {
            icon: Some(IconRef(&LIGHTBULB)),
            body: Some("Run the tests before you push.".into()),
            countdown: true,
            hold_secs: Some(6.0),
            actions: vec![Action { icon: Some(IconRef(&UNDO_2)), ..action("Undo", ActionTone::Quiet, "undo") }],
            ..Notice::new("scene:countdown", "Remembered for this repository")
        },
    }
}

/// Posts the case's notice and opens its card now.
pub fn show(s: &mut State, case: Case) {
    s.notices.post(notice(case), s.clock);
    s.notices.open_next();
}

/// A note an agent saved, as the `memory.saved` event carries it: the card goes through the real event path.
pub fn remembered(s: &mut State) {
    use crate::fleet::events::{Event, Kind};
    let note = serde_json::json!({
        "id": "5c1e9a02", "list": "repository", "text": "Run the formatter before you commit; CI rejects unformatted files.",
        "repository": "/work/example", "updated": "2026-11-16T09:30:00Z",
    });
    let payload = serde_json::json!({"note": note, "repositoryName": "example", "previous": null});
    s.dispatch(&crate::fleet::state::Topic::Sessions, &[Event::Other(Kind::MemorySaved, payload)]);
    s.notices.open_next();
}

/// A night's dream that ended, as `memory.dreamed` carries it: Memory is on, and the card goes through the real event path.
pub fn dreamed(s: &mut State) {
    use crate::fleet::events::{Event, Kind};
    s.pickers.prefs.set("Memory", "true");
    let change = |id: &str, kind: &str| {
        serde_json::json!({ "id": id, "kind": kind, "list": "repository", "noteId": format!("note-{id}"), "text": "A note.", "replaced": [],
            "why": "Seen twice.", "sessions": [], "ofSessions": 0, "quote": null, "reason": null, "undone": false })
    };
    let sessions: Vec<_> =
        (0..14).map(|i| serde_json::json!({ "id": format!("s{i}"), "title": format!("Session {i}"), "harnessType": null })).collect();
    let payload = serde_json::json!({
        "id": "dream-scene", "repository": "/work/example", "repositoryName": "example", "sessionId": null, "harnessType": "opencode2",
        "status": "done", "error": null, "started": "2026-11-17T03:00:00Z", "finished": "2026-11-17T03:09:00Z", "readSessions": sessions,
        "changes": [change("a", "added"), change("b", "rewritten"), change("c", "merged"), change("d", "kept")],
        "runId": "run-scene", "tokens": 48200, "since": null });
    s.dispatch(&crate::fleet::state::Topic::Sessions, &[Event::Other(Kind::MemoryDreamed, payload)]);
    s.notices.open_next();
}

/// The card, already settled: only its chip in the status bar.
pub fn settled(s: &mut State) {
    s.notices.post(notice(Case::Plain), s.clock);
    s.notices.open_next();
    s.notices.settle();
}

/// The card with the right panel shut, so the composer reaches under it.
pub fn lifted(s: &mut State) {
    s.content_open = false;
    show(s, Case::Plain);
}

/// A Fleet restart that cut sessions off, through the real rule: the rows carry the interruptions, the notice is built from
/// them, and its card opens now. `auto` is Fleet continuing them by itself.
pub fn restarted(s: &mut State, auto: bool) {
    use crate::fleet::rows::Interruption;
    let titles = [
        "Fix pagination",
        "Write the release notes",
        "Tidy the settings page",
        "Speed up the list",
        "Check the export",
        "Rename the helpers",
        "Add a retry",
    ];
    s.projects[0].sessions = titles
        .iter()
        .enumerate()
        .map(|(i, title)| {
            let mut session = crate::mock::Session {
                id: format!("restart-{i}"),
                title: (*title).into(),
                status: crate::mock::Status::Idle,
                ago_secs: Some(60),
                branch: "main".into(),
                messages: Vec::new(),
                tokens: 0,
                row: Default::default(),
                listing: Default::default(),
            };
            session.row.interrupted = Some(Interruption {
                at: "2026-11-16T09:00:00Z".into(),
                waiting: i == 1 && !auto,
                last_call: None,
                changed_files: None,
                auto: auto && i < 3,
            });
            session
        })
        .collect();
    s.keep_restart_notice();
    s.notices.open_next();
}

fn machine_info(id: &str, name: &str, version: &str, update: serde_json::Value) -> crate::fleet::machines::Info {
    let json = serde_json::json!({ "id": id, "name": name, "os": "linux", "version": version, "update": update });
    crate::fleet::machines::Info::from_json(&json).expect("an info")
}

/// Home updated itself in the night under a rule nobody picked, through the real rule (`fleet::update_notices`): the card
/// says when, says it once, and offers What's new, Change the rule and Dismiss.
pub fn machine_updated(s: &mut State) {
    s.now_ms = crate::fleet::work::parse_iso_ms("2026-10-20T12:00:00Z");
    s.pickers.prefs.fetched = true;
    let last = serde_json::json!({ "from": "0.59.0", "to": "0.59.1", "at": "2026-10-20T03:12:00Z", "trigger": "idle" });
    let update = serde_json::json!({ "status": "uptodate", "latestVersion": "0.59.1", "lastUpdate": last });
    s.machines.home = Some(machine_info("home-id", "harbor", "0.59.1", update));
    s.keep_update_notices();
    s.notices.open_next();
}

/// Two listed machines have an update waiting, or (`rollout`) a rollout is under way across three: only the status bar chip.
pub fn machines_can_update(s: &mut State, rollout: bool) {
    let list: Vec<serde_json::Value> = ["falcon", "lantern"]
        .iter()
        .map(|n| serde_json::json!({ "id": format!("{n}-id"), "name": n, "baseUrl": "http://127.0.0.1:6001" }))
        .collect();
    s.machines.list = list.iter().filter_map(crate::fleet::machine::Machine::from_json).collect();
    for m in s.machines.list.clone() {
        let update = serde_json::json!({ "status": "available", "latestVersion": "0.59.1" });
        s.machines.identities.insert(m.id.clone(), machine_info(&m.id, &m.name, "0.59.0", update));
    }
    if rollout {
        let steps: Vec<_> = [("harbor", "done"), ("falcon", "waiting"), ("lantern", "pending")]
            .iter()
            .map(|(n, st)| serde_json::json!({ "machineId": format!("{n}-id"), "name": n, "state": st }))
            .collect();
        let body = serde_json::json!({ "id": "r1", "status": "running", "startedAt": "2026-10-20T09:00:00Z", "steps": steps });
        s.updates.rollout = crate::fleet::updates::Rollout::from_json(&body);
    }
    s.keep_update_notices();
}
