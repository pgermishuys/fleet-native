//! Component 16, piece I: `HarnessProfilesPanel.test.ts`'s editor cases (the "I" ones) under their names, and the code box,
//! against the fake Fleet piece H's tests use.
use crate::fleet::settings::harness_profiles::{Editing, Load, saved_words};
use crate::tests_settings_harness_profiles::{Fake, Rig, profile, rig};
use crate::ui::code_box::{caret_line, line_count};
use rune::{Key, KeyEvent, Mods};
use serde_json::json;

impl Rig {
    fn type_into(&mut self, key: &str, words: &str) {
        crate::tests::click(&mut self.rt, key);
        self.rt.key(KeyEvent { key: Key::Text(words.into()), mods: Mods::default() });
        self.frames();
    }
    fn press(&mut self, key: Key, mods: Mods) {
        self.rt.key(KeyEvent { key, mods });
        self.frames();
    }
    /// A window tall enough to hold the page, so the editor's buttons are in view to be clicked.
    fn tall(&mut self) {
        self.rt.set_viewport((1280.0, 2600.0), 1.0);
        self.frames();
    }
    fn open_edit(&mut self, id: &str) {
        self.wait("the list", |s| s.profiles_load("opencode2") == Load::Loaded);
        self.tall();
        self.click(&format!("harness-profile-edit-{id}"));
        self.settle();
    }
    /// The editor opens with the page's layout moving; its buttons take a press only once that has stopped.
    fn settle(&mut self) {
        for _ in 0..30 {
            self.rt.frame(crate::tests::FRAME);
        }
    }
    fn open_new(&mut self) {
        self.wait("the list", |s| s.profiles_load("opencode2") == Load::Loaded);
        self.tall();
        self.click("harness-profile-new");
        self.settle();
    }
    fn content(&self) -> String {
        self.rt.state.settings.harnesses.profiles.draft.as_ref().map(|d| d.content.text.clone()).unwrap_or_default()
    }
    fn set_content(&mut self, text: &str) {
        let text = text.to_string();
        self.rt.update(move |s, _| s.settings.harnesses.profiles.draft.as_mut().unwrap().content = rune::TextEdit::new(text));
        self.settle();
    }
    fn writes(&self) -> Vec<(String, String, serde_json::Value)> {
        self.fake.lock().unwrap().writes.clone()
    }
    fn result(&self) -> Option<crate::fleet::settings::harness_profiles::Check> {
        self.rt.state.settings.harnesses.profiles.draft.as_ref().and_then(|d| d.result.clone())
    }
}

fn one() -> Fake {
    Fake { profiles: vec![profile("work", "Work", false, 1)], ..Fake::default() }
}

#[test]
fn shows_what_the_harness_says_about_a_profile_under_the_editor_its_code_as_code() {
    let mut r = rig(Fake { note: Some("Fleet hands this over as `ACME_CONFIG`. Keep keys out of it.".into()), ..one() });
    r.open_edit("work");
    assert!(r.words().contains("Fleet hands this over as ACME_CONFIG. Keep keys out of it."));
    assert!(r.has("harness-profile-note"));
}

#[test]
fn says_nothing_under_the_editor_when_the_harness_sends_no_note() {
    let mut r = rig(one());
    r.open_edit("work");
    assert!(r.has("harness-profile-editor"));
    assert!(!r.has("harness-profile-note"));
}

#[test]
fn saves_an_edit_and_says_open_sessions_keep_the_old_version_until_fleet_restarts() {
    let mut r = rig(one());
    r.open_edit("work");
    r.set_content("{ \"model\": \"work/other\" }");
    r.click("harness-profile-save");
    r.wait("the close", |s| s.settings.harnesses.profiles.notice.is_some());
    assert_eq!(
        r.writes()[0],
        (
            "PUT".into(),
            "/api/harnesses/opencode2/profiles/work".into(),
            json!({"name": "Work", "content": "{ \"model\": \"work/other\" }"})
        )
    );
    assert_eq!(
        r.text_of("harness-profile-notice"),
        "Saved Work. New sessions use it now; the 1 open session picks it up when Fleet restarts."
    );
    assert!(!r.has("harness-profile-editor"));
}

#[test]
fn keeps_the_editor_open_and_shows_opencode_s_reason_when_saving_fails() {
    let mut r =
        rig(Fake { refuse_write: Some("This profile isn't valid JSON. EndOfFileExpected at line 2, column 1".into()), ..Fake::default() });
    r.open_new();
    r.type_into("harness-profile-name", "Broken");
    r.click("harness-profile-save");
    r.wait("the refusal", |s| s.settings.harnesses.profiles.draft.as_ref().is_some_and(|d| d.result.is_some()));
    assert!(r.words().contains("EndOfFileExpected at line 2, column 1"));
    assert!(r.has("harness-profile-editor-new"));
    assert_eq!(r.writes()[0].2["name"], "Broken");
}

#[test]
fn asks_before_deleting_then_deletes() {
    let mut r = rig(Fake { profiles: vec![profile("work", "Work", true, 0)], ..Fake::default() });
    r.open_edit("work");
    r.click("harness-profile-delete");
    assert!(r.writes().is_empty(), "the first press only asks");
    assert!(r.words().contains("Delete Work?"));
    r.click("harness-profile-delete");
    r.wait("the delete", |s| s.settings.harnesses.profiles.notice.is_some());
    assert_eq!(r.writes()[0].0, "DELETE");
    assert_eq!(r.text_of("harness-profile-notice"), "Deleted Work. New sessions start with no profile.");
    assert!(r.profiles().is_empty());
}

#[test]
fn a_new_profile_starts_with_a_starter_and_an_empty_name_saves_as_untitled() {
    let mut r = rig(Fake::default());
    r.open_new();
    assert!(!r.content().trim().is_empty());
    r.click("harness-profile-save");
    r.wait("the close", |s| s.settings.harnesses.profiles.notice.is_some());
    assert_eq!(r.writes()[0].0, "POST");
    assert_eq!(r.writes()[0].2["name"], "Untitled profile");
    assert_eq!(r.text_of("harness-profile-notice"), "Created Untitled profile. Pick it from the profile chip when you start a session.");
}

#[test]
fn says_when_the_text_isn_t_json_yet_but_not_for_comments_trailing_commas_or_nothing() {
    let mut r = rig(one());
    r.open_edit("work");
    assert!(!r.has("harness-profile-syntax"));
    r.set_content("{ \"model\": ");
    assert_eq!(r.text_of("harness-profile-syntax"), "This isn't valid JSON yet. Comments and trailing commas are fine.");
    r.set_content("{\n  // note\n  \"model\": \"a\",\n}");
    assert!(!r.has("harness-profile-syntax"));
    r.set_content("   ");
    assert!(!r.has("harness-profile-syntax"));
}

#[test]
fn test_says_the_harness_loads_it_or_why_it_doesn_t_without_saving() {
    let mut r = rig(Fake { check: Some(json!({"ok": true})), ..one() });
    r.open_edit("work");
    r.click("harness-profile-test");
    r.wait("the answer", |s| s.settings.harnesses.profiles.draft.as_ref().is_some_and(|d| d.result.is_some()));
    assert!(r.words().contains("OpenCode 2 loads this profile."));
    r.fake.lock().unwrap().check = Some(json!({"ok": false, "error": "Unknown key", "details": ["line 2", "column 4"]}));
    r.click("harness-profile-test");
    r.wait("the refusal", |s| s.settings.harnesses.profiles.draft.as_ref().and_then(|d| d.result.as_ref()).is_some_and(|c| !c.ok));
    assert!(r.words().contains("Unknown key"));
    assert_eq!(r.result().unwrap().details.as_deref(), Some("line 2\ncolumn 4"));
    assert!(r.writes().iter().all(|w| w.1.ends_with("/check")), "a test saves nothing");
}

#[test]
fn duplicate_opens_a_new_profile_named_copy_with_the_text_as_it_is() {
    let mut r = rig(one());
    r.open_edit("work");
    r.click("harness-profile-duplicate");
    assert_eq!(
        r.rt.state.settings.harnesses.profiles.editing,
        Some(Editing::New(Some(("Work copy".into(), "{ \"model\": \"work/model\" }".into()))))
    );
    assert!(r.has("harness-profile-row-new"));
    assert_eq!(r.rt.state.settings.harnesses.profiles.draft.as_ref().unwrap().name.text, "Work copy");
}

#[test]
fn cancel_closes_without_saving_and_the_name_is_cut_at_sixty_characters() {
    let mut r = rig(one());
    r.open_edit("work");
    r.type_into("harness-profile-name", &"x".repeat(80));
    r.rt.update(|s, _| {
        s.limit_profile_name();
    });
    assert_eq!(r.rt.state.settings.harnesses.profiles.draft.as_ref().unwrap().name.text.chars().count(), 60);
    r.click("harness-profile-cancel");
    assert!(!r.has("harness-profile-editor"));
    assert!(r.writes().is_empty());
    assert!(r.rt.state.settings.harnesses.profiles.notice.is_none());
}

#[test]
fn ctrl_or_cmd_s_in_the_code_box_saves() {
    let mut r = rig(one());
    r.open_edit("work");
    crate::tests::click(&mut r.rt, "harness-profile-config");
    r.press(Key::Text("s".into()), Mods { ctrl: true, ..Mods::default() });
    r.wait("the save", |s| s.settings.harnesses.profiles.notice.is_some());
    assert_eq!(r.writes()[0].0, "PUT");
}

#[test]
fn the_code_box_numbers_its_lines_and_follows_the_caret() {
    assert_eq!(line_count(""), 1);
    assert_eq!(line_count("a\nb\n"), 3);
    let mut e = rune::TextEdit::new("a\nbb\nccc");
    e.set_selection(4, 4);
    assert_eq!(caret_line(&e), 1);
    let mut r = rig(one());
    r.open_edit("work");
    r.set_content("a\nb\nc\nd");
    let all = r.rt.elements();
    let numbers = all.iter().find(|e| e.key.as_deref() == Some("harness-profile-config-lines")).expect("the numbers");
    let words: Vec<String> =
        all.iter().filter(|e| e.path.starts_with(&numbers.path) && e.path != numbers.path).filter_map(|e| e.text.clone()).collect();
    assert_eq!(words, ["1", "2", "3", "4"]);
}

#[test]
fn what_a_save_says() {
    assert_eq!(saved_words("W", false, 3), "Saved W.");
    assert_eq!(saved_words("W", true, 0), "Saved W.");
    assert_eq!(saved_words("W", true, 1), "Saved W. New sessions use it now; the 1 open session picks it up when Fleet restarts.");
    assert_eq!(saved_words("W", true, 3), "Saved W. New sessions use it now; the 3 open sessions pick it up when Fleet restarts.");
}

#[test]
fn tab_indents_in_the_code_box_instead_of_leaving_it() {
    let mut r = rig(one());
    r.open_edit("work");
    r.set_content("");
    crate::tests::click(&mut r.rt, "harness-profile-config");
    r.press(Key::Tab, Mods::default());
    assert_eq!(r.content(), "  ");
    r.press(Key::Text("a".into()), Mods::default());
    assert_eq!(r.content(), "  a");
}
