//! Component 16, piece AM: Settings → Skills & tools → Mods (the kept side). Fleet's `ModsSection.test.ts` (16 cases) and the
//! kept-side `kept-api.test.ts` cases under their names, against a fake Fleet that answers `/api/features/mods`, `/api/mods` and
//! what is under it. A test waits for the request it means and counts only the requests it means.
use crate::fleet::settings::mods_kept::{self as kept, KeptMod};
use crate::fleet::settings::skills::Tab;
use crate::tests::click;
use crate::tests_settings_general::{Rig, unread};
use serde_json::{Value, json};

const NOW: i64 = 1_791_633_600_000; // 2026-10-10T12:00:00Z

fn version(n: i64, over: Value) -> Value {
    let mut v = json!({
        "number": n, "createdAt": "2026-10-08T09:00:00Z", "version": format!("0.{n}.0"), "sha256": "x",
        "sessionId": format!("s-{n}"), "sessionTitle": "Make test output readable", "note": null,
    });
    for (k, x) in over.as_object().into_iter().flatten() {
        v[k] = x.clone();
    }
    v
}

fn chips() -> Value {
    json!({ "name": "test-chips", "description": "Shows test results as chips", "active": 2, "activeVersion": "0.2.0", "off": null,
        "versions": [version(1, json!({})), version(2, json!({ "note": "Chips wrap now" }))] })
}

fn gauge() -> Value {
    json!({ "name": "context-gauge", "description": "A gauge of context use", "active": 1, "activeVersion": "0.1.0",
        "off": { "by": "user", "at": "2026-10-09T09:00:00Z" },
        "versions": [version(1, json!({ "sessionId": null, "sessionTitle": null }))] })
}

fn broken() -> Value {
    json!({ "name": "branch-badge", "description": "A badge for the branch", "active": 3, "activeVersion": "0.3.0",
        "off": { "by": "strikes", "at": "2026-10-09T09:00:00Z", "error": "Cannot read x of undefined" },
        "versions": [version(1, json!({})), version(2, json!({})), version(3, json!({}))] })
}

fn serve(r: &Rig, route: &str, code: u16, body: Value) {
    r.fake.lock().unwrap().routes.insert(route.to_string(), (code, body));
}

/// The Mods tab on a Fleet that has `mods` kept; `switch` is the preference (`Some("false")` for off).
fn mount_with(mods: Vec<Value>, safe: bool, preference: &str, code: u16, reply: Option<Value>) -> Rig {
    let mut r = unread(&[("Mods", preference)]);
    serve(&r, "GET /api/features/mods", 200, json!({ "on": preference == "true", "safeMode": safe }));
    serve(&r, "GET /api/mods", code, reply.unwrap_or_else(|| json!({ "safeMode": safe, "mods": mods })));
    r.rt.update(|s, _| {
        s.now_ms = Some(NOW);
        // Fleet's app reads the preferences when it starts; Skills & tools doesn't ask for them itself.
        s.refresh_preferences();
        s.open_settings("skills", Some("mods"));
    });
    // The tab exists only while the switch is on; with it off, the page opens on Skills and the test opens Mods itself.
    r.wait("the preferences", |s| s.pickers.prefs.fetched);
    if preference != "true" {
        r.rt.update(|s, _| s.settings.skills.tab = Tab::Mods);
    }
    r.wait("the Mods tab to be read", |s| {
        let m = &s.settings.skills.mods;
        s.settings.skills.tab == Tab::Mods
            && m.loaded_for.is_some()
            && !m.loading
            && (m.switch.is_some() || m.kept.is_some() || m.error.is_some())
    });
    r.frames();
    r
}

fn mount(mods: Vec<Value>) -> Rig {
    mount_with(mods, false, "true", 200, None)
}

fn calls(r: &Rig, method: &str, path: &str) -> Vec<String> {
    r.fake.lock().unwrap().calls.iter().filter(|c| c.0 == method && c.1 == path).map(|c| c.2.clone()).collect()
}

fn count(r: &Rig, method: &str, path: &str) -> usize {
    calls(r, method, path).len()
}

fn has(r: &Rig, key: &str) -> bool {
    r.info(key).is_some()
}

/// A click on an element keyed `key`, scrolled into reach first by the wheel (the tab is taller than the window).
fn press(r: &mut Rig, key: &str) {
    for _ in 0..60 {
        let e = r.info(key).unwrap_or_else(|| panic!("no element keyed {key:?}"));
        let (top, bottom) = (e.rect[1], e.rect[1] + e.rect[3]);
        if top > 80.0 && bottom < 700.0 {
            break;
        }
        r.rt.wheel(700.0, 400.0, if top <= 80.0 { -240.0 } else { 240.0 });
        r.rt.frame(crate::tests::FRAME);
    }
    click(&mut r.rt, key);
}

/// Every word drawn inside the element at `key`.
fn words_in(r: &Rig, key: &str) -> String {
    let all = r.rt.elements();
    let at = all.iter().position(|e| e.key.as_deref() == Some(key)).unwrap_or_else(|| panic!("no element keyed {key:?}"));
    let mut out = Vec::new();
    let mut stack = vec![at];
    while let Some(i) = stack.pop() {
        out.extend(all[i].text.clone());
        let mut kids: Vec<usize> = (0..all.len()).filter(|&j| all[j].parent == Some(i)).collect();
        kids.reverse();
        stack.extend(kids);
    }
    out.join(" ")
}

fn faded(r: &Rig, key: &str) -> bool {
    r.info(key).is_some_and(|e| e.opacity < 0.9)
}

fn is_on(r: &Rig, name: &str) -> bool {
    state_of(r, name).is_on()
}

fn state_of(r: &Rig, name: &str) -> KeptMod {
    r.rt.state.settings.skills.mods.mods().iter().find(|m| m.name == name).cloned().unwrap_or_else(|| panic!("no mod {name}"))
}

// ─── ModsSection.test.ts ────────────────────────────────────────────────────────

#[test]
fn lists_every_kept_mod_with_version_description_kept_date_and_the_session_it_came_from() {
    let r = mount(vec![chips(), gauge(), broken()]);
    let words = words_in(&r, "mod-row-test-chips");
    for want in ["test-chips", "v2", "0.2.0", "Shows test results as chips", "kept 2 days ago", "Make test output readable"] {
        assert!(words.contains(want), "missing {want:?} in:\n{words}");
    }
    assert!(has(&r, "mod-session-test-chips"), "a mod with a session id links to it");
    assert!(!has(&r, "mod-session-context-gauge"), "no session, no link");
    assert!(!words_in(&r, "mod-row-context-gauge").contains("from"));
}

#[test]
fn the_session_title_opens_that_session() {
    let mut r = mount(vec![chips()]);
    press(&mut r, "mod-session-test-chips");
    assert_eq!(r.rt.state.rail, crate::Rail::Sessions);
}

#[test]
fn shows_loading_then_the_error_with_the_server_s_words_and_a_try_again() {
    let mut r = mount_with(vec![], false, "true", 500, Some(json!({ "error": "Mods are off." })));
    assert!(r.words().contains("Mods are off."));
    assert!(has(&r, "mods-error"));
    // Loading: the spinner and its words replace the list until the read ends.
    r.rt.update(|s, _| s.settings.skills.mods.loading = true);
    r.frames();
    assert!(r.words().contains("Loading your mods\u{2026}"));
    r.rt.update(|s, _| s.settings.skills.mods.loading = false);
    r.frames();
    let before = count(&r, "GET", "/api/mods");
    press(&mut r, "mods-error-retry");
    let fake = r.fake.clone();
    r.wait("the second read", move |_| fake.lock().unwrap().calls.iter().filter(|c| c.1 == "/api/mods").count() > before);
    assert_eq!(count(&r, "GET", "/api/mods"), before + 1);
}

#[test]
fn says_so_when_nothing_is_kept_yet() {
    let r = mount(vec![]);
    let words = r.words();
    assert!(words.contains("No kept mods yet"));
    assert!(words.contains("When an agent writes a mod in a session, you can keep it from there, or add one from the Showcase below."));
    assert!(has(&r, "mods-showcase"), "the Showcase stays under the empty state");
}

#[test]
fn shows_the_safe_mode_callout_and_turns_mods_back_on() {
    let mut r = mount_with(vec![chips()], true, "true", 200, None);
    assert!(words_in(&r, "mods-safe-mode").contains("Mods are stopped for now (Start without mods)."));
    serve(&r, "PUT /api/mods/safe-mode", 200, json!({ "safeMode": false, "mods": [chips()] }));
    press(&mut r, "mods-safe-mode-off");
    let fake = r.fake.clone();
    r.wait("the safe-mode request", move |_| fake.lock().unwrap().calls.iter().any(|c| c.0 == "PUT" && c.1 == "/api/mods/safe-mode"));
    assert_eq!(calls(&r, "PUT", "/api/mods/safe-mode"), vec![json!({ "on": false }).to_string()]);
    r.wait("the callout to go", |s| !s.settings.skills.mods.safe_mode());
    r.frames();
    assert!(!has(&r, "mods-safe-mode"));
}

#[test]
fn links_to_plugins_mods_where_the_switch_and_the_runtime_are() {
    let mut r = mount(vec![chips()]);
    assert!(r.words().contains("The switch and the mod runtime (Bun) are in"));
    press(&mut r, "mods-features-link");
    assert_eq!(r.rt.state.settings.page, crate::fleet::settings_nav::Page::Plugin("mods".into()));
}

#[test]
fn points_at_plugins_mods_instead_when_the_mods_switch_is_off() {
    let r = mount_with(vec![chips()], false, "false", 200, None);
    assert!(has(&r, "mods-switched-off"));
    assert!(r.words().contains("Mods are off. Turn Mods on in"));
    assert!(!has(&r, "mod-row-test-chips"));
    assert_eq!(count(&r, "GET", "/api/mods"), 0, "no list is asked for while the switch is off");
}

// ─── ModsSection.test.ts: Showcase ──────────────────────────────────────────────

#[test]
fn lists_every_sample_with_a_showcase_tag_what_it_does_and_where_to_see_it() {
    let r = mount(vec![]);
    let names: Vec<&str> = crate::fleet::settings::mods_showcase::GROUPS.iter().flat_map(|g| g.samples.iter().map(|s| s.name)).collect();
    assert_eq!(
        names,
        [
            "test-chips",
            "build-summary",
            "git-glance",
            "http-check",
            "lint-summary",
            "edit-size",
            "search-hits",
            "branch-context",
            "tests-after-edits",
            "push-guard",
            "press-counter",
            "release-board"
        ]
    );
    for n in &names {
        assert!(has(&r, &format!("mods-sample-{n}")), "no row for {n}");
    }
    let words = r.words();
    for want in ["SHOWCASE", "Draws curl responses as a status pill and size", "Try it: Run curl -i in any session."] {
        assert!(words.contains(want), "missing {want:?}");
    }
}

#[test]
fn says_added_for_a_sample_that_s_kept_and_on_and_offers_add_again_when_it_s_off() {
    let mut other = gauge();
    other["name"] = json!("git-glance");
    let r = mount(vec![chips(), other]);
    assert!(has(&r, "mods-sample-added-test-chips"));
    assert!(!has(&r, "mods-sample-add-test-chips"));
    assert!(has(&r, "mods-sample-add-git-glance"));
}

#[test]
fn add_posts_the_sample_s_own_route() {
    let mut r = mount(vec![]);
    press(&mut r, "mods-sample-add-build-summary");
    let fake = r.fake.clone();
    r.wait("the sample request", move |_| {
        fake.lock().unwrap().calls.iter().any(|c| c.0 == "POST" && c.1 == "/api/mods/samples/build-summary")
    });
    assert_eq!(count(&r, "POST", "/api/mods/samples/build-summary"), 1);
    r.wait("Add to finish", |s| s.settings.skills.mods.adding.is_none());
    assert!(r.rt.state.settings.skills.mods.sample_error.is_none());
}

#[test]
fn shows_the_server_s_refusal_on_that_sample_s_row() {
    let mut r = mount(vec![]);
    serve(&r, "POST /api/mods/samples/push-guard", 400, json!({ "error": "The mods runtime isn't ready." }));
    // Its row is far below the fold of the 760-point test window; the click path is the one `add_posts_...` takes.
    r.rt.update(|s, _| s.add_mod_sample("push-guard"));
    r.wait("the refusal", |s| s.settings.skills.mods.sample_error.is_some());
    r.frames();
    assert_eq!(words_in(&r, "mods-sample-error-push-guard"), "The mods runtime isn't ready.");
    assert!(!has(&r, "mods-sample-error-build-summary"));
}

#[test]
fn the_sample_refusal_falls_back_to_the_status() {
    let mut r = mount(vec![]);
    serve(&r, "POST /api/mods/samples/push-guard", 502, json!({}));
    r.rt.update(|s, _| s.add_mod_sample("push-guard"));
    r.wait("the refusal", |s| s.settings.skills.mods.sample_error.is_some());
    assert_eq!(r.rt.state.settings.skills.mods.sample_error.clone().unwrap().1, "The sample couldn't be added (HTTP 502).");
}

#[test]
fn the_showcase_isn_t_shown_while_mods_are_off() {
    let r = mount_with(vec![chips()], false, "false", 200, None);
    assert!(!has(&r, "mods-showcase"));
}

// ─── ModsSection.test.ts: on and off ────────────────────────────────────────────

#[test]
fn reflects_each_mod_s_state_in_its_switch_and_flips_it() {
    let mut r = mount(vec![chips(), gauge(), broken()]);
    assert!(is_on(&r, "test-chips") && has(&r, "mod-switch-test-chips"));
    assert!(!is_on(&r, "context-gauge") && has(&r, "mod-switch-context-gauge"));
    assert!(has(&r, "mod-off-context-gauge"));
    assert!(!has(&r, "mod-off-branch-badge"), "a mod that turned itself off says why instead of Off");
    serve(&r, "POST /api/mods/test-chips/off", 200, {
        let mut m = chips();
        m["off"] = json!({ "by": "user", "at": "2026-10-10T11:00:00Z" });
        m
    });
    press(&mut r, "mod-switch-test-chips");
    r.wait("the mod to turn off", |s| s.settings.skills.mods.mods().iter().any(|m| m.name == "test-chips" && !m.is_on()));
    assert_eq!(count(&r, "POST", "/api/mods/test-chips/off"), 1);
}

#[test]
fn says_why_a_mod_turned_itself_off_and_turns_it_on_again() {
    let mut r = mount(vec![chips(), gauge(), broken()]);
    assert!(words_in(&r, "mod-strikes-branch-badge").contains("Turned off after 3 failures: Cannot read x of undefined"));
    assert!(!has(&r, "mod-turn-on-context-gauge"));
    let mut back = broken();
    back["off"] = Value::Null;
    serve(&r, "POST /api/mods/branch-badge/on", 200, back);
    press(&mut r, "mod-turn-on-branch-badge");
    r.wait("the mod to turn on", |s| s.settings.skills.mods.mods().iter().any(|m| m.name == "branch-badge" && m.is_on()));
    assert_eq!(count(&r, "POST", "/api/mods/branch-badge/on"), 1);
    r.frames();
    assert!(!has(&r, "mod-strikes-branch-badge"));
}

// ─── ModsSection.test.ts: Undo ──────────────────────────────────────────────────

#[test]
fn undo_says_where_it_goes_back_to() {
    let mut r = mount(vec![chips(), gauge()]);
    assert_eq!(r.rt.state.settings.skills.mods.mods()[0].undo_label(), "Undo (back to v1)");
    assert!(r.words().contains("Undo (back to v1)"));
    let mut back = chips();
    back["active"] = json!(1);
    serve(&r, "POST /api/mods/test-chips/undo", 200, back);
    press(&mut r, "mod-undo-test-chips");
    r.wait("Undo to be answered", |s| s.settings.skills.mods.mods()[0].active == Some(1) && s.settings.skills.mods.busy.is_empty());
    assert_eq!(count(&r, "POST", "/api/mods/test-chips/undo"), 1);
}

#[test]
fn undo_is_disabled_with_the_reason_on_v1_when_already_off() {
    let mut r = mount(vec![chips(), gauge()]);
    let why = "Nothing to undo: v1 is the first version and the mod is already off.";
    assert_eq!(r.text("mod-undo-why-context-gauge"), why);
    assert!(faded(&r, "mod-undo-context-gauge"));
    press(&mut r, "mod-undo-context-gauge");
    r.frames();
    assert_eq!(count(&r, "POST", "/api/mods/context-gauge/undo"), 0, "a disabled Undo asks for nothing");
}

#[test]
fn undo_shows_the_server_s_refusal() {
    let mut r = mount(vec![chips()]);
    serve(&r, "POST /api/mods/test-chips/undo", 409, json!({ "error": "Nothing to undo." }));
    press(&mut r, "mod-undo-test-chips");
    r.wait("the refusal", |s| s.settings.skills.mods.row_errors.contains_key("test-chips"));
    r.frames();
    assert!(words_in(&r, "mod-error-test-chips").contains("Nothing to undo."));
}

// ─── kept-api.test.ts and stores/mods ───────────────────────────────────────────

#[test]
fn reads_the_switch_as_null_on_a_404_a_failure_or_bad_json() {
    let ok = |body: &str| Ok((200, body.to_string()));
    assert_eq!(kept::parse_switch(ok(r#"{"on":true,"safeMode":false}"#)), Some(kept::Switch { on: true, safe_mode: false }));
    assert_eq!(kept::parse_switch(Ok((404, "{}".into()))), None);
    assert_eq!(kept::parse_switch(Err("refused".into())), None);
    assert_eq!(kept::parse_switch(ok("not json")), None);
}

#[test]
fn builds_the_paths_encoding_the_name() {
    assert_eq!(kept::mod_path("a b/c", "/undo"), "/api/mods/a%20b%2Fc/undo");
    assert_eq!(kept::sample_path("test-chips"), "/api/mods/samples/test-chips");
}

#[test]
fn a_refusal_carries_the_server_s_words_and_status() {
    let r = mount(vec![]);
    serve(&r, "POST /api/mods/x/on", 422, json!({ "error": "No such mod." }));
    let base = r.rt.state.link.as_ref().unwrap().base.clone();
    let got = kept::post_mod(&base, "x", "/on").unwrap_err();
    assert_eq!((got.status, got.message.as_str()), (422, "No such mod."));
    serve(&r, "POST /api/mods/y/on", 500, Value::Null);
    let got = kept::post_mod(&base, "y", "/on").unwrap_err();
    assert_eq!(got.status, 500);
}

#[test]
fn a_kept_list_reloads_on_mods_changed_but_only_once_it_was_read() {
    use crate::fleet::events::{Event, Kind};
    use crate::fleet::state::Topic;
    let mut r = mount(vec![chips()]);
    let before = count(&r, "GET", "/api/mods");
    serve(&r, "GET /api/mods", 200, json!({ "safeMode": false, "mods": [chips(), gauge()] }));
    r.rt.update(|s, _| s.dispatch(&Topic::Sessions, &[Event::Other(Kind::ModsChanged, json!({ "reason": "kept" }))]));
    r.wait("the list to reload", |s| s.settings.skills.mods.mods().len() == 2);
    assert_eq!(count(&r, "GET", "/api/mods"), before + 1);
    // A reason that isn't about the list asks for nothing.
    r.rt.update(|s, _| s.dispatch(&Topic::Sessions, &[Event::Other(Kind::ModsChanged, json!({ "reason": "draft" }))]));
    r.frames();
    assert_eq!(count(&r, "GET", "/api/mods"), before + 1);
    assert!(!is_on(&r, "context-gauge"));
}

#[test]
fn one_request_per_row_at_a_time() {
    let mut r = mount(vec![chips()]);
    r.rt.update(|s, _| {
        s.set_mod_on("test-chips", false);
        s.set_mod_on("test-chips", false);
    });
    r.wait("the answer", |s| s.settings.skills.mods.busy.is_empty());
    assert_eq!(count(&r, "POST", "/api/mods/test-chips/off"), 1);
}
