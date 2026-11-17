//! Invented dreams for Settings → Memory's Dreaming panel when the app has no Fleet to ask (and for its scenes and tests).
//! Nothing here is from a real machine.
use crate::fleet::settings::dreaming::{Dream, parse_dream};
use crate::fleet::store::invented_id;
use serde_json::{Value, json};

fn session(n: usize, title: &str, harness: Option<&str>) -> Value {
    json!({ "id": invented_id(0, n), "title": title, "harnessType": harness })
}

fn change(id: &str, kind: &str, list: Option<&str>, text: Option<&str>, why: &str) -> Value {
    json!({ "id": id, "kind": kind, "list": list, "noteId": format!("note-{id}"), "text": text, "replaced": [], "why": why,
            "sessions": [], "ofSessions": 0, "quote": null, "reason": null, "undone": false })
}

fn with(mut change: Value, fields: Value) -> Value {
    for (k, v) in fields.as_object().expect("an object") {
        change[k] = v.clone();
    }
    change
}

fn dream(status: &str, changes: Vec<Value>, fields: Value) -> Dream {
    let mut d = json!({
        "id": "dream-1", "repository": "/work/lantern", "repositoryName": "lantern", "sessionId": invented_id(0, 3), "harnessType": "opencode2",
        "status": status, "error": null, "started": "2026-10-20T03:00:00Z", "finished": "2026-10-20T03:09:00Z",
        "readSessions": [session(0, "Speed up the nightly suite", None), session(1, "Fix the flaky snapshot test", None), session(2, "Prepare the 2.4 release", None)],
        "changes": changes, "runId": "run-1", "tokens": 48200, "since": null });
    for (k, v) in fields.as_object().expect("an object") {
        d[k] = v.clone();
    }
    parse_dream(&d).expect("a dream")
}

fn evidence() -> Value {
    json!({ "sessions": [session(1, "Fix the flaky snapshot test", Some("opencode2")), session(2, "Prepare the 2.4 release", Some("claude-code"))],
            "ofSessions": 3, "evidence": "tests/render/snapshots.rs shows the folder next to each test.",
            "quote": "snapshots/ is created beside the test file, never at the top level." })
}

/// A night's dream with something of every applied kind, and one note left as is, and one seen once.
pub fn last_night() -> Dream {
    dream(
        "done",
        vec![
            with(
                change(
                    "c1",
                    "added",
                    Some("repository"),
                    Some("Run `just lint` before pushing; CI runs it first."),
                    "Said by two sessions.",
                ),
                evidence(),
            ),
            with(
                change(
                    "c2",
                    "rewritten",
                    Some("repository"),
                    Some("Snapshots live next to the test file, in a `snapshots/` folder."),
                    "The old words were half true.",
                ),
                json!({ "replaced": [{ "id": "n-old", "text": "Snapshots live in a shared folder." }] }),
            ),
            with(
                change(
                    "c3",
                    "confirmed",
                    Some("repository"),
                    Some("Releases are cut from the `release/` branches only."),
                    "Checked against the release script.",
                ),
                json!({ "how": "read scripts/release.sh and the tags" }),
            ),
            change("c4", "left", Some("machine"), Some("Port 8080 is taken by another service."), "Not seen this week."),
            change("c5", "skipped", Some("repository"), Some("The linter wants `--fix` in CI."), "Seen in one session only."),
        ],
        json!({}),
    )
}

/// Two of the user's own notes that look out of date, one with the evidence and one without a note to act on.
pub fn flags() -> Dream {
    let mut d = dream(
        "done",
        vec![
            with(
                change(
                    "f1",
                    "flagged",
                    Some("repository"),
                    Some("Run the suite with `make check`."),
                    "There is no `check` target any more.",
                ),
                evidence(),
            ),
            with(
                change(
                    "f2",
                    "flagged",
                    Some("machine"),
                    Some("The staging box is at the old address."),
                    "A session reached it at a new one.",
                ),
                json!({ "noteId": null }),
            ),
            change("c1", "added", Some("repository"), Some("Run `just lint` before pushing; CI runs it first."), "Said by two sessions."),
        ],
        json!({}),
    );
    d.tokens = Some(912);
    d
}

/// A dream still reading, then running.
pub fn running() -> Dream {
    dream("running", vec![], json!({ "finished": null, "tokens": null }))
}

pub fn failed() -> Dream {
    dream("failed", vec![], json!({ "error": "The harness stopped before it wrote anything.", "finished": null }))
}

/// Rewritten, merged and retired changes, with Why open on the first.
pub fn rewritten_merged() -> Dream {
    dream(
        "done",
        vec![
            with(
                with(
                    change(
                        "m1",
                        "rewritten",
                        Some("repository"),
                        Some("Snapshots live next to the test file, in a `snapshots/` folder."),
                        "Two notes disagreed.",
                    ),
                    json!({ "replaced": [{ "id": "n1", "text": "Snapshots live in a shared folder." }] }),
                ),
                evidence(),
            ),
            with(
                change(
                    "m2",
                    "merged",
                    Some("repository"),
                    Some("Run `just lint` and `just fmt` before pushing."),
                    "Two notes said the same thing.",
                ),
                json!({ "replaced": [{ "id": "n2", "text": "Run `just lint` before pushing." }, { "id": "n3", "text": "Run `just fmt` before pushing." }] }),
            ),
            with(
                change("m3", "retired", Some("machine"), None, "The service it names was removed."),
                json!({ "replaced": [{ "id": "n4", "text": "Port 8080 is taken by another service." }] }),
            ),
            with(
                change(
                    "m4",
                    "kept",
                    Some("repository"),
                    Some("Releases are cut from `release/` branches only."),
                    "Checked again; it held.",
                ),
                json!({ "undone": true }),
            ),
            change("m5", "skipped", Some("repository"), Some("The linter wants `--fix` in CI."), "Seen in one session only."),
            change("m6", "skipped", None, None, "Only one session mentioned the proxy."),
        ],
        json!({}),
    )
}

/// What a scene shows of the panel.
#[derive(Clone, Copy)]
pub enum Variant {
    LastNight,
    Flags,
    FlagEditing,
    Running,
    Failed,
    Evidence,
    RewrittenMerged,
    Error,
}

pub fn scene(s: &mut crate::State, dark: bool, variant: Variant) {
    use crate::fleet::settings::memory::MemoryPage;
    s.dark = Some(dark);
    s.now_ms = crate::fleet::work::parse_iso_ms("2026-10-20T12:00:00Z");
    s.open_settings("memory", None);
    let key = s.settings.choice.key(&s.machines).to_string();
    let mut o = super::memory::overview();
    o.enabled = true;
    let mut page = MemoryPage {
        overview: Some(o),
        notes: Some(super::memory::rich_notes()),
        repository: Some(super::memory::REPOSITORY.into()),
        loaded_for: Some(key),
        ..Default::default()
    };
    page.dreaming.asked = Some((page.request, page.repository.clone()));
    let d = &mut page.dreaming;
    let shown = match variant {
        Variant::LastNight => last_night(),
        Variant::Flags => flags(),
        Variant::FlagEditing => {
            d.editing = Some("f1".into());
            d.edit = rune::TextEdit::new("Run the suite with `just check`.");
            flags()
        }
        Variant::Running => running(),
        Variant::Failed => failed(),
        Variant::Evidence => {
            d.open.insert("c1".into());
            last_night()
        }
        Variant::RewrittenMerged => {
            d.open.insert("m1".into());
            d.show_skipped = true;
            rewritten_merged()
        }
        Variant::Error => {
            d.error = Some("Couldn't load the dreams.".into());
            last_night()
        }
    };
    d.was_busy = crate::fleet::settings::dreaming::is_busy(Some(&shown));
    d.dream = Some(shown);
    s.settings.memory_ui = Default::default();
    s.settings.memory = page;
}

// ─── The nightly row ────────────────────────────────────────────────────────

/// The invented answer for "Dream each night" when the app has no Fleet: switched off, as a new install is.
pub fn nightly() -> crate::fleet::settings::dream_nightly::Nightly {
    crate::fleet::settings::dream_nightly::Nightly {
        enabled: false,
        automation_id: None,
        paused: false,
        time: "03:00".into(),
        time_zone: None,
        harness_type: None,
        model: None,
        budget_tokens: 300_000,
        next_run_at: None,
        recent: vec![],
    }
}

#[derive(Clone, Copy)]
pub enum NightlyVariant {
    Off,
    On,
    Paused,
    /// The read failed and there is no answer: the error with Try again stands where the row would be.
    LoadFailed,
}

/// Settings → Memory with the nightly row in one of its states, the dreaming panel under it.
pub fn nightly_scene(s: &mut crate::State, dark: bool, variant: NightlyVariant) {
    use crate::composer::choice::ModelOption;
    use crate::fleet::new_session::feeds::Catalog;
    scene(s, dark, Variant::LastNight);
    let page = &mut s.settings.memory;
    page.nightly.asked = Some(page.request);
    // Fleet's picture has no dream to show under the row, so the panel is the bare header here too.
    page.dreaming.dream = None;
    page.dreaming.was_busy = false;
    let mut view = nightly();
    match variant {
        NightlyVariant::Off => {}
        NightlyVariant::On | NightlyVariant::Paused => {
            view.enabled = true;
            view.time = "03:30".into();
            view.harness_type = Some("opencode2".into());
            view.model = Some("anthropic/sonnet".into());
            view.budget_tokens = 500_000;
            if matches!(variant, NightlyVariant::Paused) {
                view.paused = true;
            }
            view.automation_id = Some("automation-dreaming".into());
            page.nightly.catalog_for = Some("opencode2".into());
            page.nightly.catalog = Some(Catalog {
                supported: true,
                models: vec![
                    ModelOption::new("anthropic", "Anthropic", "sonnet", "Claude Sonnet"),
                    ModelOption::new("anthropic", "Anthropic", "haiku", "Claude Haiku"),
                ],
                ..Default::default()
            });
        }
        NightlyVariant::LoadFailed => {
            page.nightly.error = Some("Nightly dreaming is unavailable.".into());
        }
    }
    if !matches!(variant, NightlyVariant::LoadFailed) {
        page.nightly.time_shown = view.time.clone();
        page.nightly.time = rune::TextEdit::new(view.time.clone());
        page.nightly.nightly = Some(view);
    }
}
