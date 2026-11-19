//! Component 16, piece AL: the skill Editor and History dialogs (rows 8.75 to 8.89). Fleet has no component tests for these two,
//! so these come from reading `SkillEditorDialog.vue` and `SkillHistoryDialog.vue`, against a fake Fleet that answers
//! `/api/skills/built-in/<name>`. A test waits for the request it means and counts only those.
use crate::fleet::settings::skill_editor::Start;
use crate::tests::{FRAME, click};
use crate::tests_settings_general::{Rig, unread};
use rune::{Key, KeyEvent, Mods, TextEdit};
use serde_json::{Value, json};

const LIST: &str = "/api/skills/built-in";
const NAME: &str = "fleet-run";
const FLEET: &str = "---\nname: fleet-run\n---\nRun the app.\n";
const MINE: &str = "---\nname: fleet-run\n---\nRun the app, then say what you saw.\n";

fn detail(your: Option<&str>, version: Option<u32>, versions: Value) -> Value {
    json!({
        "name": NAME, "description": "Run the app.", "enabled": true, "fleetContent": FLEET, "yourContent": your,
        "version": version, "fleetChanged": false, "fleetBefore": null, "versions": versions,
    })
}

fn two_versions() -> Value {
    json!([
        { "number": 2, "createdAt": "2026-10-14T09:00:00Z", "note": "Say what you saw", "sessionId": "s1", "sessionTitle": "Fix the pager", "active": true },
        { "number": 1, "createdAt": "2026-10-10T09:00:00Z", "note": null, "sessionId": null, "sessionTitle": null, "active": false },
    ])
}

fn serve(r: &Rig, route: &str, code: u16, body: Value) {
    r.fake.lock().unwrap().routes.insert(route.to_string(), (code, body));
}

/// Settings → Built-in skills, on a Fleet whose one skill answers `skill` for its detail.
fn mount(skill: Value) -> Rig {
    let r = unread(&[]);
    let listed = json!({ "name": NAME, "description": "Run the app.", "enabled": true, "version": skill["version"],
        "versionCount": skill["versions"].as_array().map_or(0, |a| a.len()) });
    serve(&r, &format!("GET {LIST}"), 200, json!([listed]));
    serve(&r, &format!("GET {LIST}/{NAME}"), 200, skill);
    let mut r = r;
    r.rt.update(|s, _| s.open_settings("plugin:built-in-skills", None));
    r.wait("the list of built-in skills", |s| s.settings.built_in.loaded_for.is_some() && !s.settings.built_in.loading);
    r.frames();
    r
}

fn calls(r: &Rig, method: &str, path: &str) -> Vec<String> {
    r.fake.lock().unwrap().calls.iter().filter(|(m, p, _)| m == method && p == path).map(|(_, _, b)| b.clone()).collect()
}

fn edit(r: &mut Rig) {
    click(&mut r.rt, &format!("built-in-skill-edit-{NAME}"));
    r.wait("the skill for the Editor", |s| s.settings.editor.detail.is_some() || s.settings.editor.error.is_some());
    r.frames();
}

fn history(r: &mut Rig) {
    click(&mut r.rt, &format!("built-in-skill-history-{NAME}"));
    r.wait("the skill for History", |s| s.settings.history.detail.is_some() || s.settings.history.error.is_some());
    r.frames();
}

fn type_into(r: &mut Rig, key: &str, words: &str) {
    click(&mut r.rt, key);
    r.rt.key(KeyEvent { key: Key::Text(words.into()), mods: Mods::default() });
    r.rt.frame(FRAME);
    r.rt.frame(FRAME);
}

/// A disabled control fades to half; the fade is a spring, so let it settle before looking.
fn disabled(r: &mut Rig, key: &str) -> bool {
    for _ in 0..40 {
        r.rt.frame(FRAME);
    }
    r.info(key).unwrap_or_else(|| panic!("no element keyed {key:?}")).opacity < 0.9
}

fn set_draft(r: &mut Rig, text: &str) {
    let text = text.to_string();
    r.rt.update(move |s, _| s.settings.editor.draft = TextEdit::new(text));
    r.frames();
}

// ---- the Editor ---------------------------------------------------------------------------------------------------------

#[test]
fn the_next_version_is_one_more_than_the_newest_and_one_when_there_are_none() {
    let mut r = mount(detail(Some(MINE), Some(2), two_versions()));
    edit(&mut r);
    assert!(r.words().contains("Saving makes this your version 3."), "{}", r.words());
    assert!(r.words().contains("Save as version 3"));
    let mut r = mount(detail(None, None, json!([])));
    edit(&mut r);
    assert!(r.words().contains("Saving makes this your version 1."));
    assert!(r.words().contains("Save as version 1"));
}

#[test]
fn starts_from_the_text_sessions_get_and_save_waits_for_a_change() {
    let mut r = mount(detail(Some(MINE), Some(2), two_versions()));
    edit(&mut r);
    assert_eq!(r.rt.state.settings.editor.draft.text, MINE, "your version, not Fleet's");
    assert!(disabled(&mut r, "skill-editor-save"));
    assert!(disabled(&mut r, "skill-editor-show-changes"), "nothing to show yet");
    set_draft(&mut r, &format!("{MINE}One more line.\n"));
    assert!(!disabled(&mut r, "skill-editor-save"));
    set_draft(&mut r, MINE);
    assert!(disabled(&mut r, "skill-editor-save"), "back to the current text");
}

#[test]
fn save_posts_the_text_with_the_note_trimmed_and_the_dialog_closes() {
    let mut r = mount(detail(None, None, json!([])));
    edit(&mut r);
    set_draft(&mut r, MINE);
    type_into(&mut r, "skill-editor-note", "  Say what you saw  ");
    let saved = detail(Some(MINE), Some(1), json!([{ "number": 1, "createdAt": "2026-10-14T09:00:00Z", "active": true }]));
    serve(&r, &format!("POST {LIST}/{NAME}/versions"), 200, saved);
    click(&mut r.rt, "skill-editor-save");
    r.wait("the save", |s| !s.settings.editor.open);
    let sent: Value = serde_json::from_str(&calls(&r, "POST", &format!("{LIST}/{NAME}/versions"))[0]).unwrap();
    assert_eq!(sent, json!({ "content": MINE, "note": "Say what you saw", "sessionId": null }));
    let listed = &r.rt.state.settings.built_in.list[0];
    assert_eq!((listed.version, listed.version_count), (Some(1), 1), "the page's list took the answer");
}

#[test]
fn an_empty_note_goes_as_null() {
    let mut r = mount(detail(None, None, json!([])));
    edit(&mut r);
    set_draft(&mut r, MINE);
    type_into(&mut r, "skill-editor-note", "   ");
    serve(&r, &format!("POST {LIST}/{NAME}/versions"), 200, detail(Some(MINE), Some(1), json!([{ "number": 1 }])));
    click(&mut r.rt, "skill-editor-save");
    r.wait("the save", |s| !s.settings.editor.open);
    let sent: Value = serde_json::from_str(&calls(&r, "POST", &format!("{LIST}/{NAME}/versions"))[0]).unwrap();
    assert_eq!(sent["note"], Value::Null);
}

#[test]
fn start_from_fleets_version_shows_only_with_a_version_and_a_different_draft() {
    let mut r = mount(detail(None, None, json!([])));
    edit(&mut r);
    assert!(r.info("skill-editor-from-fleets").is_none(), "no version of yours: Fleet's is the text already");
    let mut r = mount(detail(Some(MINE), Some(2), two_versions()));
    edit(&mut r);
    assert!(r.info("skill-editor-from-fleets").is_some());
    click(&mut r.rt, "skill-editor-from-fleets");
    assert_eq!(r.rt.state.settings.editor.draft.text, FLEET);
    r.frames();
    assert!(r.info("skill-editor-from-fleets").is_none(), "the draft is Fleet's now");
    assert!(!disabled(&mut r, "skill-editor-save"), "and it differs from what sessions get");
}

#[test]
fn show_changes_draws_the_diff_of_the_current_text_against_the_draft() {
    let mut r = mount(detail(Some(MINE), Some(2), two_versions()));
    edit(&mut r);
    set_draft(&mut r, &format!("{MINE}One more line.\n"));
    click(&mut r.rt, "skill-editor-show-changes");
    assert!(r.info("skill-editor-diff").is_some());
    assert_eq!(r.text("skill-editor-diff-label"), "fleet-run/SKILL.md");
    assert!(r.words().contains("Hide changes"));
    set_draft(&mut r, MINE);
    assert!(r.info("skill-editor-diff").is_none(), "no change, no diff");
}

#[test]
fn a_failed_save_keeps_the_dialog_with_the_servers_words_and_a_second_save_works() {
    let mut r = mount(detail(None, None, json!([])));
    edit(&mut r);
    set_draft(&mut r, MINE);
    serve(&r, &format!("POST {LIST}/{NAME}/versions"), 500, json!({ "error": "The disk is full." }));
    click(&mut r.rt, "skill-editor-save");
    r.wait("the failed save", |s| s.settings.editor.error.is_some());
    r.frames();
    assert!(r.rt.state.settings.editor.open && !r.rt.state.settings.editor.saving);
    assert!(r.words().contains("The disk is full."));
    assert_eq!(r.rt.state.settings.editor.draft.text, MINE, "the text is kept");
}

#[test]
fn a_skill_that_wont_load_says_why_and_has_no_text_box() {
    let r = unread(&[]);
    serve(&r, &format!("GET {LIST}"), 200, json!([{ "name": NAME, "description": "", "enabled": true }]));
    serve(&r, &format!("GET {LIST}/{NAME}"), 404, json!({}));
    let mut r = r;
    r.rt.update(|s, _| s.open_settings("plugin:built-in-skills", None));
    r.wait("the list", |s| s.settings.built_in.loaded_for.is_some() && !s.settings.built_in.loading);
    r.frames();
    edit(&mut r);
    assert!(r.words().contains("Couldn't load fleet-run."), "{}", r.words());
    assert!(r.info("skill-editor-text").is_none());
    assert!(disabled(&mut r, "skill-editor-save"));
}

#[test]
fn improves_edit_by_hand_starts_from_its_text_and_note_and_names_the_session() {
    let mut r = mount(detail(None, None, json!([])));
    r.rt.update(|s, _| {
        let start = Start { content: Some(MINE.into()), note: Some("Say what you saw".into()), session_id: Some("s9".into()) };
        s.skill_editor_open(NAME, start, true);
    });
    r.wait("the skill for the Editor", |s| s.settings.editor.detail.is_some());
    r.frames();
    assert_eq!(r.rt.state.settings.editor.draft.text, MINE);
    assert_eq!(r.rt.state.settings.editor.note.text, "Say what you saw");
    assert!(!disabled(&mut r, "skill-editor-save"), "the draft already differs from Fleet's text");
    serve(&r, &format!("POST {LIST}/{NAME}/versions"), 200, detail(Some(MINE), Some(1), json!([{ "number": 1 }])));
    click(&mut r.rt, "skill-editor-save");
    r.wait("the save", |s| !s.settings.editor.open);
    let sent: Value = serde_json::from_str(&calls(&r, "POST", &format!("{LIST}/{NAME}/versions"))[0]).unwrap();
    assert_eq!(sent["sessionId"], "s9");
    let saved = r.rt.state.settings.editor.saved.clone().expect("what the opener hears");
    assert_eq!(saved.version, Some(1));
}

#[test]
fn each_open_starts_fresh() {
    let mut r = mount(detail(None, None, json!([])));
    edit(&mut r);
    set_draft(&mut r, MINE);
    type_into(&mut r, "skill-editor-note", "typed");
    click(&mut r.rt, "skill-editor-cancel");
    assert!(!r.rt.state.settings.editor.open);
    edit(&mut r);
    assert_eq!(r.rt.state.settings.editor.draft.text, FLEET);
    assert_eq!(r.rt.state.settings.editor.note.text, "");
    assert!(!r.rt.state.settings.editor.changes);
}

#[test]
fn escape_closes_the_editor_but_not_while_it_saves() {
    let mut r = mount(detail(None, None, json!([])));
    edit(&mut r);
    r.rt.update(|s, _| s.settings.editor.saving = true);
    r.rt.update(|s, _| s.skill_editor_close());
    assert!(r.rt.state.settings.editor.open, "a save is under way");
    r.rt.update(|s, _| s.settings.editor.saving = false);
    r.rt.key(KeyEvent { key: Key::Escape, mods: Mods::default() });
    r.frames();
    assert!(!r.rt.state.settings.editor.open);
}

// ---- History ------------------------------------------------------------------------------------------------------------

#[test]
fn history_lists_the_versions_newest_first_then_fleets_row() {
    let mut r = mount(detail(Some(MINE), Some(2), two_versions()));
    history(&mut r);
    assert!(r.info("skill-version-2").is_some() && r.info("skill-version-1").is_some() && r.info("skill-version-fleet").is_some());
    assert!(r.text("skill-version-when-2").contains("Fix the pager"), "{}", r.text("skill-version-when-2"));
    assert!(r.words().contains("\u{201c}Say what you saw\u{201d}"));
    assert!(r.words().contains("No note"), "version 1 has none");
    assert!(r.info("skill-version-in-use-2").is_some(), "the version in use shows its pill");
    assert!(r.info("skill-version-use-2").is_none());
    assert!(r.info("skill-version-use-1").is_some());
    assert!(r.info("skill-version-use-fleet").is_some(), "Fleet's row offers itself while a version is in use");
    let (y2, y1) = (r.info("skill-version-2").unwrap().rect[1], r.info("skill-version-1").unwrap().rect[1]);
    assert!(y2 < y1, "newest first");
}

#[test]
fn a_skill_with_no_versions_shows_only_the_fleet_row() {
    let mut r = mount(detail(None, None, json!([])));
    // The page offers History only once there are versions; open it directly.
    r.rt.update(|s, _| s.skill_history_open(NAME, true));
    r.wait("the skill for History", |s| s.settings.history.detail.is_some());
    r.frames();
    assert!(r.info("skill-version-fleet").is_some());
    assert!(r.info("skill-version-in-use-fleet").is_some(), "no version of yours is in use");
    assert!(r.info("skill-version-use-fleet").is_none());
    assert!(r.info("skill-version-1").is_none());
}

#[test]
fn use_this_version_moves_the_in_use_pill_and_updates_the_lists() {
    let mut r = mount(detail(Some(MINE), Some(2), two_versions()));
    history(&mut r);
    let after = json!([
        { "number": 2, "createdAt": "2026-10-14T09:00:00Z", "note": "Say what you saw", "active": false },
        { "number": 1, "createdAt": "2026-10-10T09:00:00Z", "note": null, "active": true },
    ]);
    serve(&r, &format!("PUT {LIST}/{NAME}/active"), 200, detail(Some(FLEET), Some(1), after));
    click(&mut r.rt, "skill-version-use-1");
    r.wait("the switch", |s| s.settings.history.busy.is_none() && s.settings.history.detail.as_ref().is_some_and(|d| d.version == Some(1)));
    r.frames();
    assert_eq!(calls(&r, "PUT", &format!("{LIST}/{NAME}/active")), vec![json!({ "version": 1 }).to_string()]);
    assert!(r.info("skill-version-in-use-1").is_some() && r.info("skill-version-in-use-2").is_none());
    assert!(r.info("skill-version-use-2").is_some());
    assert_eq!(r.rt.state.settings.built_in.list[0].version, Some(1), "the page's list agrees");
}

#[test]
fn use_fleets_sends_a_null_version_and_a_second_press_while_one_runs_is_ignored() {
    let mut r = mount(detail(Some(MINE), Some(2), two_versions()));
    history(&mut r);
    serve(&r, &format!("PUT {LIST}/{NAME}/active"), 200, detail(None, None, two_versions()));
    r.rt.update(|s, _| {
        s.skill_history_use(None);
        s.skill_history_use(Some(1));
    });
    r.wait("the switch", |s| s.settings.history.busy.is_none());
    assert_eq!(calls(&r, "PUT", &format!("{LIST}/{NAME}/active")), vec![json!({ "version": null }).to_string()], "one request only");
    r.frames();
    assert!(r.info("skill-version-in-use-fleet").is_some());
}

#[test]
fn a_failed_switch_says_why_under_the_list() {
    let mut r = mount(detail(Some(MINE), Some(2), two_versions()));
    history(&mut r);
    serve(&r, &format!("PUT {LIST}/{NAME}/active"), 500, json!({}));
    click(&mut r.rt, "skill-version-use-1");
    r.wait("the failed switch", |s| s.settings.history.error.is_some());
    r.frames();
    assert!(r.words().contains("Couldn't switch fleet-run."), "{}", r.words());
    assert!(r.info("skill-version-1").is_some(), "the list stays");
}

#[test]
fn compare_opens_one_diff_at_a_time_and_hide_closes_it() {
    let mut r = mount(detail(Some(MINE), Some(2), two_versions()));
    history(&mut r);
    serve(&r, &format!("GET {LIST}/{NAME}/versions/1"), 200, json!({ "content": "one\n" }));
    serve(&r, &format!("GET {LIST}/{NAME}/versions/2"), 200, json!({ "content": MINE }));
    click(&mut r.rt, "skill-version-compare-1");
    r.wait("version 1's text", |s| s.settings.history.comparing.is_some());
    r.frames();
    assert_eq!(r.text("skill-version-diff-1-label"), "Fleet's \u{2192} v1");
    click(&mut r.rt, "skill-version-compare-2");
    r.wait("version 2's text", |s| s.settings.history.comparing.as_ref().is_some_and(|c| c.number == 2));
    r.frames();
    assert!(r.info("skill-version-diff-1").is_none() && r.info("skill-version-diff-2").is_some(), "one at a time");
    click(&mut r.rt, "skill-version-compare-2");
    r.frames();
    assert!(r.info("skill-version-diff-2").is_none(), "Hide closes it");
    assert!(r.words().contains("Compare with Fleet's"));
}

#[test]
fn a_version_that_wont_load_says_which() {
    let mut r = mount(detail(Some(MINE), Some(2), two_versions()));
    history(&mut r);
    serve(&r, &format!("GET {LIST}/{NAME}/versions/1"), 404, json!({}));
    click(&mut r.rt, "skill-version-compare-1");
    r.wait("the failure", |s| s.settings.history.error.is_some());
    r.frames();
    assert!(r.words().contains("Couldn't load version 1 of fleet-run."), "{}", r.words());
}

#[test]
fn history_load_failure_hides_the_spinner_and_says_why() {
    let r = unread(&[]);
    serve(&r, &format!("GET {LIST}"), 200, json!([{ "name": NAME, "description": "", "enabled": true, "versionCount": 1 }]));
    serve(&r, &format!("GET {LIST}/{NAME}"), 500, json!({ "error": "Fleet is busy." }));
    let mut r = r;
    r.rt.update(|s, _| s.open_settings("plugin:built-in-skills", None));
    r.wait("the list", |s| s.settings.built_in.loaded_for.is_some() && !s.settings.built_in.loading);
    r.frames();
    history(&mut r);
    assert!(r.info("skill-history-loading").is_none());
    assert!(r.words().contains("Fleet is busy."));
}

#[test]
fn typing_in_the_text_box_edits_the_draft_and_ctrl_s_saves_it() {
    let mut r = mount(detail(None, None, json!([])));
    edit(&mut r);
    click(&mut r.rt, "skill-editor-text");
    r.rt.key(KeyEvent { key: Key::Text("Keep it short. ".into()), mods: Mods::default() });
    r.frames();
    assert!(r.rt.state.settings.editor.draft.text.contains("Keep it short. "), "{:?}", r.rt.state.settings.editor.draft.text);
    serve(&r, &format!("POST {LIST}/{NAME}/versions"), 200, detail(Some(MINE), Some(1), json!([{ "number": 1 }])));
    r.rt.key(KeyEvent { key: Key::Text("s".into()), mods: Mods { ctrl: true, ..Mods::default() } });
    r.wait("the save", |s| !s.settings.editor.open);
    assert_eq!(calls(&r, "POST", &format!("{LIST}/{NAME}/versions")).len(), 1);
}
