//! Component 16, piece AK: the Built-in skills plugin's page body. Fleet's six `BuiltInSkillsTab.test.ts` cases under their names,
//! against a fake Fleet that answers `/api/skills/built-in`, and what the table in docs/parity/16-settings.md says about the rest
//! (8.60 to 8.73). A test waits for the request it means and counts only those.
use crate::fleet::settings::built_in_skills::PLUGIN_ID;
use crate::tests::click;
use crate::tests_settings_general::{Rig, unread};
use serde_json::{Value, json};

const LIST: &str = "/api/skills/built-in";

fn skills() -> Vec<Value> {
    vec![
        json!({ "name": "fleet-code-review", "description": "Review a change for bugs.", "enabled": false }),
        json!({ "name": "fleet-run", "description": "Run the app.", "enabled": true }),
    ]
}

fn with(base: &Value, extra: Value) -> Value {
    let mut v = base.clone();
    for (k, x) in extra.as_object().unwrap() {
        v[k] = x.clone();
    }
    v
}

fn label_of(r: &Rig, key: &str) -> String {
    r.info(key).unwrap_or_else(|| panic!("no element keyed {key:?}")).label.unwrap_or_default()
}

fn serve(r: &Rig, route: &str, code: u16, body: Value) {
    r.fake.lock().unwrap().routes.insert(route.to_string(), (code, body));
}

/// The page, on a Fleet that lists `list`, once the list has come.
fn mount(list: Vec<Value>) -> Rig {
    let r = unread(&[]);
    serve(&r, &format!("GET {LIST}"), 200, Value::Array(list));
    open(r)
}

fn open(mut r: Rig) -> Rig {
    r.rt.update(|s, _| s.open_settings(&format!("plugin:{PLUGIN_ID}"), None));
    r.wait("the list of built-in skills", |s| s.settings.built_in.loaded_for.is_some() && !s.settings.built_in.loading);
    r.frames();
    r
}

fn calls(r: &Rig, method: &str, path: &str) -> Vec<String> {
    r.fake.lock().unwrap().calls.iter().filter(|(m, p, _)| m == method && p == path).map(|(_, _, b)| b.clone()).collect()
}

fn on(r: &Rig, name: &str) -> bool {
    r.rt.state.settings.built_in.list.iter().find(|s| s.name == name).is_some_and(|s| s.enabled)
}

fn switch(name: &str) -> String {
    format!("built-in-skill-switch-{name}")
}

#[test]
fn says_which_copy_sessions_get_with_history_once_there_are_versions() {
    let base = skills();
    let r = mount(vec![with(&base[0], json!({ "version": 2, "versionCount": 2 })), with(&base[1], json!({ "versionCount": 1 }))]);
    assert_eq!(label_of(&r, "built-in-skill-version-fleet-code-review"), "Yours \u{b7} v2");
    assert_eq!(label_of(&r, "built-in-skill-version-fleet-run"), "Fleet's");
    assert!(r.info("built-in-skill-history-fleet-code-review").is_some());
    // Back on Fleet's with a version kept: History finds it again.
    assert!(r.info("built-in-skill-history-fleet-run").is_some());
    assert!(r.info("built-in-skill-fleets-fleet-code-review").is_some(), "Use Fleet's is offered while a version is in use");
    assert!(r.info("built-in-skill-fleets-fleet-run").is_none());
    assert!(r.info("built-in-skill-edit-fleet-run").is_some(), "Edit is always there");
}

#[test]
fn says_when_fleet_changed_its_version_and_keep_mine_puts_that_away() {
    let base = skills();
    let changed = with(&base[0], json!({ "version": 1, "versionCount": 1, "fleetChanged": true }));
    let mut r = mount(vec![changed.clone(), base[1].clone()]);
    serve(
        &r,
        &format!("POST {LIST}/fleet-code-review/keep-mine"),
        200,
        with(
            &changed,
            json!({ "fleetChanged": false, "fleetContent": "", "yourContent": "", "fleetBefore": null, "versions": [{ "number": 1 }] }),
        ),
    );
    assert!(r.words().contains("Fleet changed its version"));
    assert!(r.words().contains("since you made yours. Sessions still use yours."));
    click(&mut r.rt, "built-in-skill-keep-mine-fleet-code-review");
    r.wait("the keep-mine answer", |s| s.settings.built_in.busy.is_none() && !s.settings.built_in.list[0].fleet_changed);
    r.frames();
    assert_eq!(calls(&r, "POST", &format!("{LIST}/fleet-code-review/keep-mine")).len(), 1);
    assert!(r.info("built-in-skill-fleet-changed-fleet-code-review").is_none());
    assert_eq!(label_of(&r, "built-in-skill-version-fleet-code-review"), "Yours \u{b7} v1");
}

#[test]
fn lists_fleets_skills_with_whether_each_is_on() {
    let r = mount(skills());
    assert_eq!(calls(&r, "GET", LIST).len(), 1);
    let words = r.words();
    assert!(words.contains("fleet-code-review") && words.contains("Review a change for bugs."));
    assert_eq!(r.info(&switch("fleet-code-review")).unwrap().role, "switch");
    assert_eq!(r.info(&switch("fleet-code-review")).unwrap().label.as_deref(), Some("Use fleet-code-review in new sessions"));
    assert!(!on(&r, "fleet-code-review"));
    assert!(on(&r, "fleet-run"));
}

#[test]
fn shows_a_skill_that_comes_with_mods_as_on_or_off_with_mods_with_no_switch() {
    let r = mount(vec![
        json!({ "name": "fleet-mod", "description": "Write mods.", "enabled": true, "withMods": true }),
        json!({ "name": "fleet-mods-off", "description": "Write mods.", "enabled": false, "withMods": true }),
    ]);
    assert_eq!(label_of(&r, "built-in-skill-mods-fleet-mod"), "On with Mods");
    assert_eq!(label_of(&r, "built-in-skill-mods-fleet-mods-off"), "Off \u{b7} turns on with Mods");
    assert!(r.info(&switch("fleet-mod")).is_none());
    assert!(r.info(&switch("fleet-mods-off")).is_none());
}

#[test]
fn turns_a_skill_on_through_the_api_and_shows_the_servers_answer() {
    let base = skills();
    let mut r = mount(skills());
    serve(&r, &format!("PUT {LIST}/fleet-code-review"), 200, with(&base[0], json!({ "enabled": true })));
    click(&mut r.rt, &switch("fleet-code-review"));
    r.frames();
    // One change at a time: the choice is one preference on the server.
    assert_eq!(r.rt.state.settings.built_in.saving.as_deref(), Some("fleet-code-review"));
    assert!(r.info(&switch("fleet-run")).unwrap().opacity < 1.0, "the other switches are disabled while one saves");
    click(&mut r.rt, &switch("fleet-run"));
    r.wait("the PUT", |s| s.settings.built_in.saving.is_none());
    r.frames();
    let puts = calls(&r, "PUT", &format!("{LIST}/fleet-code-review"));
    assert_eq!(puts.len(), 1);
    assert_eq!(serde_json::from_str::<Value>(&puts[0]).unwrap(), json!({ "enabled": true }));
    assert!(calls(&r, "PUT", &format!("{LIST}/fleet-run")).is_empty(), "a press while one saves is ignored");
    assert!(on(&r, "fleet-code-review"));
    assert!(r.info(&switch("fleet-run")).unwrap().opacity > 0.99);
}

#[test]
fn shows_why_a_change_failed_and_keeps_the_switch_where_it_was() {
    let mut r = mount(skills());
    serve(&r, &format!("PUT {LIST}/fleet-code-review"), 404, json!({ "error": "BuiltInSkill with id 'fleet-code-review' was not found." }));
    click(&mut r.rt, &switch("fleet-code-review"));
    r.wait("the failed PUT", |s| s.settings.built_in.saving.is_none() && s.settings.built_in.save_error.is_some());
    r.frames();
    assert_eq!(r.info("built-in-skills-error").unwrap().role, "alert");
    assert!(r.words().contains("BuiltInSkill with id 'fleet-code-review' was not found."));
    assert!(!on(&r, "fleet-code-review"));
}

// ─── What the rows say beyond Fleet's tests ──────────────────────────────────────────────────

#[test]
fn a_failed_switch_with_no_words_from_the_server_says_what_it_tried() {
    let mut r = mount(skills());
    serve(&r, &format!("PUT {LIST}/fleet-run"), 500, json!({}));
    click(&mut r.rt, &switch("fleet-run"));
    r.wait("the failed PUT", |s| s.settings.built_in.save_error.is_some());
    assert!(r.rt.state.settings.built_in.note().unwrap().starts_with("Couldn't turn fleet-run off."));
}

#[test]
fn a_list_that_is_not_an_array_draws_the_intro_alone() {
    let r = unread(&[]);
    serve(&r, &format!("GET {LIST}"), 200, json!({ "skills": [] }));
    let r = open(r);
    assert!(r.rt.state.settings.built_in.list.is_empty());
    assert!(r.info("built-in-skills-intro").is_some());
    assert!(r.info("built-in-skills-error").is_none(), "no empty state and no error");
}

#[test]
fn an_unreadable_list_says_so_with_the_servers_words_or_the_fallback() {
    let r = unread(&[]);
    serve(&r, &format!("GET {LIST}"), 404, json!({}));
    let r = open(r);
    assert!(r.words().contains("Couldn't load Fleet's built-in skills."));
}

#[test]
fn see_what_changed_opens_one_diff_and_hide_closes_it() {
    let base = skills();
    let changed = with(&base[0], json!({ "version": 1, "versionCount": 1, "fleetChanged": true }));
    let mut r = mount(vec![changed.clone(), with(&base[1], json!({ "version": 1, "versionCount": 1, "fleetChanged": true }))]);
    serve(
        &r,
        &format!("GET {LIST}/fleet-code-review"),
        200,
        with(&changed, json!({ "fleetBefore": "one\ntwo\n", "fleetContent": "one\nthree\n", "yourContent": "x", "versions": [] })),
    );
    click(&mut r.rt, "built-in-skill-see-change-fleet-code-review");
    r.wait("the detail", |s| s.settings.built_in.fleet_change.is_some());
    r.frames();
    assert!(r.info("built-in-skill-diff-fleet-code-review").is_some());
    assert!(r.words().contains("What Fleet changed in its version"));
    assert_eq!(label_of(&r, "built-in-skill-see-change-fleet-code-review"), "Hide what changed");
    click(&mut r.rt, "built-in-skill-see-change-fleet-code-review");
    r.frames();
    assert!(r.info("built-in-skill-diff-fleet-code-review").is_none());
    assert_eq!(label_of(&r, "built-in-skill-see-change-fleet-code-review"), "See what changed");
}

#[test]
fn use_fleets_switches_to_fleets_copy_and_one_action_runs_at_a_time() {
    let base = skills();
    let mine = with(&base[0], json!({ "version": 2, "versionCount": 2 }));
    let mut r = mount(vec![mine.clone(), with(&base[1], json!({ "version": 1, "versionCount": 1 }))]);
    serve(&r, &format!("PUT {LIST}/fleet-code-review/active"), 200, with(&base[0], json!({ "version": null, "versions": [{}, {}] })));
    click(&mut r.rt, "built-in-skill-fleets-fleet-code-review");
    r.frames();
    click(&mut r.rt, "built-in-skill-fleets-fleet-run");
    r.wait("the switch", |s| s.settings.built_in.busy.is_none());
    r.frames();
    let sent = calls(&r, "PUT", &format!("{LIST}/fleet-code-review/active"));
    assert_eq!(sent.len(), 1);
    assert_eq!(serde_json::from_str::<Value>(&sent[0]).unwrap(), json!({ "version": null }));
    assert!(calls(&r, "PUT", &format!("{LIST}/fleet-run/active")).is_empty());
    assert_eq!(label_of(&r, "built-in-skill-version-fleet-code-review"), "Fleet's");
    assert_eq!(r.rt.state.settings.built_in.list[0].version_count, 2, "the count is the number of versions the answer lists");
}

#[test]
fn a_failed_action_says_why_and_wins_over_the_load_error() {
    let base = skills();
    let mut r = mount(vec![with(&base[0], json!({ "version": 1, "versionCount": 1 })), base[1].clone()]);
    serve(&r, &format!("PUT {LIST}/fleet-code-review/active"), 500, json!({ "error": "The disk is full." }));
    click(&mut r.rt, "built-in-skill-fleets-fleet-code-review");
    r.wait("the failed switch", |s| s.settings.built_in.action_error.is_some());
    assert_eq!(r.rt.state.settings.built_in.note(), Some("The disk is full."));
}

#[test]
fn edit_and_history_ask_for_their_dialog_and_leaving_the_page_forgets_them() {
    let base = skills();
    let mut r = mount(vec![with(&base[0], json!({ "versionCount": 1 })), base[1].clone()]);
    click(&mut r.rt, "built-in-skill-edit-fleet-code-review");
    click(&mut r.rt, "built-in-skill-history-fleet-code-review");
    assert_eq!(r.rt.state.settings.built_in.editing.as_deref(), Some("fleet-code-review"));
    assert_eq!(r.rt.state.settings.built_in.history.as_deref(), Some("fleet-code-review"));
    r.rt.update(|s, _| s.open_settings("plugins", None));
    r.frames();
    assert_eq!(r.rt.state.settings.built_in, Default::default());
}

#[test]
fn every_answer_goes_into_the_conversations_list_by_name() {
    let base = skills();
    let mut r = unread(&[]);
    serve(&r, &format!("GET {LIST}"), 200, Value::Array(skills()));
    r.rt.update(|s, _| s.skills_ensure_loaded());
    r.wait("the conversation's list", |s| s.skills.list.is_some());
    let mut r = open(r);
    serve(
        &r,
        &format!("PUT {LIST}/fleet-code-review"),
        200,
        with(&base[0], json!({ "enabled": true, "version": 3, "versions": [{}, {}, {}] })),
    );
    click(&mut r.rt, &switch("fleet-code-review"));
    r.wait("the PUT", |s| s.settings.built_in.saving.is_none());
    let shared = r.rt.state.skills.list.clone().unwrap();
    assert!(shared[0].enabled && shared[0].version == Some(3) && shared[0].version_count == 3);
}
