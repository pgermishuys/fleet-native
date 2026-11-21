//! Component 19, piece K: where an automation runs. Fleet's two K cases of `AutomationsPage.test.ts` under their names ("asks
//! where it runs when no folder was used before, and opens the folder menu instead of creating", "renames an automation
//! from the … chip, saving on Enter"), against a fake Fleet; then native cases for the rules the strip keeps: another folder
//! clears the base, the current checkout has no base chip, an older target stays until changed, skipping off sends 0, a
//! workflow's "…" says it always skips, and closing a menu gives the box the keys.
use crate::fleet::new_session::request::{Folder, Workspace};
use crate::tests::{click, press_key};
use crate::tests_automations_composer::{KETTLE, open, page, plan, start_new, submit_shown, weekly_digest};
use crate::tests_automations_page::Rig;
use rune::{Key, Mods};
use serde_json::json;

const TEAPOT: &str = "/work/teapot";

fn label_of(r: &Rig, key: &str) -> String {
    r.rt.elements().iter().find(|e| e.key.as_deref() == Some(key)).and_then(|e| e.label.clone()).unwrap_or_default()
}

/// Presses the element whose key starts with `prefix` and whose label is `label` (a folder or base row, by its name).
fn click_row(r: &mut Rig, prefix: &str, label: &str) {
    let key =
        r.rt.elements()
            .iter()
            .find(|e| e.key.as_deref().is_some_and(|k| k.starts_with(prefix)) && e.label.as_deref() == Some(label))
            .and_then(|e| e.key.clone())
            .unwrap_or_else(|| panic!("no {prefix}… row named {label}"));
    click(&mut r.rt, &key);
    r.frames();
}

fn focused(r: &Rig) -> Option<String> {
    r.rt.elements().iter().find(|e| e.focused).and_then(|e| e.key.clone())
}

/// A new automation in kettle, a new worktree each run; teapot is the machine's other repository.
fn drafted(words: &str) -> Rig {
    let mut r = page(vec![weekly_digest(json!({}))], true);
    r.home.repositories.lock().unwrap().push(json!({"name": "teapot", "path": TEAPOT, "parentRoot": "/work"}));
    start_new(&mut r, words);
    r.wait("the folder New session used last", |s| s.automations_nav.draft.folder.is_some());
    r.frames();
    r
}

fn send(r: &mut Rig) -> serde_json::Value {
    click(&mut r.rt, "automation-submit");
    let heard = r.home.handle();
    r.wait("the create", move |_| heard("POST", "/api/automations") > 0);
    r.home.body_of("POST", "/api/automations")
}

// ─── AutomationsPage.test.ts: "Automations screen" (the K cases) ────────────────────────────────────────────────────

#[test]
fn asks_where_it_runs_when_no_folder_was_used_before_and_opens_the_folder_menu_instead_of_creating() {
    // Nothing remembered from New session: the draft has no folder.
    let mut r = page(vec![weekly_digest(json!({}))], false);
    start_new(&mut r, "Every Monday at 9am, summarise the open PRs");

    assert!(plan(&r).contains("Choose where it runs."), "{}", plan(&r));
    assert_eq!(label_of(&r, "new-session-folder-chip"), "Choose a folder");
    click(&mut r.rt, "automation-submit");
    r.frames();

    assert_eq!(r.home.calls("POST", "/api/automations"), 0);
    assert!(r.rt.state.folder_menu_open(), "the folder menu opened instead");
    assert!(r.has("new-session-folder-pop"));
    assert!(r.has("new-session-folder-none"), "a session's runs may have no folder");
    assert!(!r.has("new-session-folder-clone-form"), "no create or clone from here");
    // The menu offers the machine's repository and No folder, and choosing one is the draft's folder.
    click_row(&mut r, "new-session-folder-row-", "kettle");
    assert_eq!(r.rt.state.automations_nav.draft.folder, Some(Folder::Repository(KETTLE.into())));
    assert!(r.rt.state.automations_nav.draft.has_chosen_folder);
    assert_eq!(label_of(&r, "new-session-folder-chip"), "kettle");
}

#[test]
fn renames_an_automation_from_the_chip_saving_on_enter() {
    let mut r = page(vec![weekly_digest(json!({}))], true);
    open(&mut r, "a1");

    click(&mut r.rt, "automation-more-chip");
    r.frames();
    assert_eq!(r.rt.state.automations_nav.name_box.text, "Weekly PR digest");
    assert_eq!(focused(&r).as_deref(), Some("automation-name"), "the Name field has the keys");
    press_key(&mut r.rt, Key::Command('a'), Mods::default());
    press_key(&mut r.rt, Key::Text("Monday PR digest".into()), Mods::default());
    // Enter is done: it saves, as Enter in the box does, rather than leaving a Save to find.
    press_key(&mut r.rt, Key::Enter, Mods::default());
    let heard = r.home.handle();
    r.wait("the save", move |_| heard("PUT", "/api/automations/a1") > 0);
    r.wait("saved", |s| s.automations_nav.notice.is_some());
    r.frames();

    assert_eq!(r.home.body_of("PUT", "/api/automations/a1")["name"], json!("Monday PR digest"));
    assert!(!r.rt.state.automation_more_open(), "the menu closed");
    assert!(r.words().contains("Monday PR digest"), "the header names it: {}", r.words());
}

// ─── What the strip keeps ───────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn another_folder_clears_the_base_and_a_repository_gets_a_new_worktree() {
    let mut r = drafted("Every Monday at 9am, summarise the open PRs");
    assert!(r.has("new-session-workspace-chip") && r.has("new-session-base-chip"), "a repository on a new worktree");
    r.wait("kettle's branches", |s| s.repository_detail().is_some());
    r.frames();
    assert_eq!(label_of(&r, "new-session-base-chip"), "origin/main");

    click(&mut r.rt, "new-session-base-chip");
    r.frames();
    assert!(!r.has("new-session-base-fetch") && !r.has("new-session-branch-name"), "no Fetch origin first, no branch name");
    click_row(&mut r, "new-session-base-row-", "release/2.0");
    assert_eq!(r.rt.state.automations_nav.draft.base_branch.as_deref(), Some("release/2.0"));
    assert_eq!(label_of(&r, "new-session-base-chip"), "release/2.0");

    // Kettle again is the same place: the base stays.
    click(&mut r.rt, "new-session-folder-chip");
    r.frames();
    click_row(&mut r, "new-session-folder-row-", "kettle");
    assert_eq!(r.rt.state.automations_nav.draft.base_branch.as_deref(), Some("release/2.0"));

    click(&mut r.rt, "new-session-folder-chip");
    r.frames();
    click_row(&mut r, "new-session-folder-row-", "teapot");
    let draft = &r.rt.state.automations_nav.draft;
    assert_eq!(draft.folder, Some(Folder::Repository(TEAPOT.into())));
    assert_eq!(draft.base_branch, None, "another folder forgets the base");
    assert_eq!(draft.workspace, Workspace::New);

    let body = send(&mut r);
    assert_eq!(body["workspaceId"], json!(TEAPOT));
    assert_eq!(body["isolation"], json!("worktree"));
    assert_eq!(body["baseBranch"], json!(null));
}

#[test]
fn the_current_checkout_has_no_base_chip_and_runs_in_the_folder_as_it_is() {
    let mut r = drafted("Every Monday at 9am, summarise the open PRs");
    click(&mut r.rt, "new-session-workspace-chip");
    r.frames();
    assert!(!r.has("new-session-workspace-loading"), "no existing worktrees are offered, nor waited for");
    assert!(!r.rt.elements().iter().any(|e| e.key.as_deref().is_some_and(|k| k.starts_with("new-session-workspace-worktree-"))));
    click(&mut r.rt, "new-session-workspace-current");
    r.frames();

    assert_eq!(r.rt.state.automations_nav.draft.workspace, Workspace::Current);
    assert!(!r.has("new-session-base-chip"));
    assert_eq!(focused(&r).as_deref(), Some("automation-message"), "closing gives the box the keys");
    let body = send(&mut r);
    assert_eq!(body["isolation"], json!("existing"));
    assert_eq!(body["baseBranch"], json!(null));
}

#[test]
fn an_older_target_stays_until_it_is_changed() {
    let mut r = page(vec![weekly_digest(json!({"targetType": "most_recent_session"}))], true);
    open(&mut r, "a1");
    assert_eq!(label_of(&r, "automation-runs-in-chip"), "The most recent session");

    click(&mut r.rt, "automation-runs-in-chip");
    r.frames();
    assert!(r.rt.elements().iter().any(|e| e.key.as_deref() == Some("automation-runs-in-older")), "offered as it was set up");
    click(&mut r.rt, "automation-runs-in-older");
    r.frames();
    assert_eq!(r.rt.state.automations_nav.edit.target_type, "most_recent_session");
    assert!(!submit_shown(&r), "picking it again changes nothing");

    click(&mut r.rt, "automation-runs-in-chip");
    r.frames();
    click(&mut r.rt, "automation-runs-in-same");
    r.frames();
    assert_eq!(label_of(&r, "automation-runs-in-chip"), "Same session each run");
    assert!(submit_shown(&r));
    click(&mut r.rt, "automation-submit");
    let heard = r.home.handle();
    r.wait("the save", move |_| heard("PUT", "/api/automations/a1") > 0);
    assert_eq!(r.home.body_of("PUT", "/api/automations/a1")["targetType"], json!("same_session"));

    // Once changed, the older target isn't offered any more.
    r.wait("saved", |s| s.automations_nav.notice.is_some());
    r.frames();
    click(&mut r.rt, "automation-runs-in-chip");
    r.frames();
    assert!(!r.has("automation-runs-in-older"));
}

#[test]
fn skipping_off_sends_zero_runs_at_once() {
    let mut r = drafted("Every Monday at 9am, summarise the open PRs");
    click(&mut r.rt, "automation-more-chip");
    r.frames();
    assert!(r.has("automation-skip"));
    assert!(r.rt.state.automations_nav.draft.skip, "on by default");
    click(&mut r.rt, "automation-skip");
    r.frames();
    assert!(!r.rt.state.automations_nav.draft.skip);
    press_key(&mut r.rt, Key::Escape, Mods::default());
    r.frames();
    assert!(!r.rt.state.automation_more_open());

    let body = send(&mut r);
    assert_eq!(body["maxConcurrentRuns"], json!(0));
}

#[test]
fn the_name_typed_names_the_draft_and_its_placeholder_is_the_auto_name() {
    let mut r = drafted("Every Monday at 9am, summarise the open PRs");
    click(&mut r.rt, "automation-more-chip");
    r.frames();
    assert_eq!(r.rt.state.automations_nav.draft.name_placeholder(), "Summarise the open PRs");
    press_key(&mut r.rt, Key::Text("PR digest".into()), Mods::default());
    r.frames();
    assert_eq!(label_of(&r, "automation-draft-row"), "PR digest", "the draft row follows the name");
    // Enter on a new one only closes the menu: Send still decides.
    press_key(&mut r.rt, Key::Enter, Mods::default());
    r.frames();
    assert!(!r.rt.state.automation_more_open());
    assert_eq!(r.home.calls("POST", "/api/automations"), 0);
    assert_eq!(focused(&r).as_deref(), Some("automation-message"));
}

#[test]
fn a_workflow_always_skips_and_has_no_box_to_tick() {
    let mut r = drafted("Every Monday at 9am, summarise the open PRs");
    r.rt.update(|s, _| s.automations_nav.draft.target_type = "workflow".into());
    click(&mut r.rt, "automation-more-chip");
    r.frames();
    assert!(r.has("automation-always-skips"));
    assert!(!r.has("automation-skip"));
    // A workflow needs a repository: no "No folder" row.
    press_key(&mut r.rt, Key::Escape, Mods::default());
    r.frames();
    click(&mut r.rt, "new-session-folder-chip");
    r.frames();
    assert!(!r.has("new-session-folder-none"), "{:?}", r.words());
}
