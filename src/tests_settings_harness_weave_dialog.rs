//! Component 16, piece Z: `WeaveConfigDialog.test.ts` (7) and the config-draft case of `HarnessesSection.test.ts`, under their
//! names, against the fake Fleet piece Y's tests use. Ours besides: Copy, Esc in the prompt box, the draft rule.
use crate::tests_settings_harness_weave::{Fake, Rig, detection, old_legacy, rig, v2_install, weave_install};
use rune::{Key, KeyEvent, Mods, TextEdit};
use serde_json::{Value, json};

fn opencode(installs: Vec<Value>) -> Value {
    detection("opencode", "OpenCode", installs, None)
}

fn cfg(r: &Rig) -> crate::fleet::settings::harness_weave_config::Config {
    r.rt.state.settings.harnesses.weave.config.clone()
}

impl Rig {
    /// A window tall enough for the page, the answer in, and the dialog open.
    fn open_dialog(&mut self) {
        self.rt.set_viewport((1280.0, 2600.0), 1.0);
        self.answered();
        self.frames();
        self.click("weave-edit-config");
        self.settle();
    }
    /// What appears fades in and takes a press only once it has settled.
    fn settle(&mut self) {
        for _ in 0..30 {
            self.rt.frame(crate::tests::FRAME);
        }
    }
    /// A press, then the layout and the fades settling.
    fn go(&mut self, key: &str) {
        self.click(key);
        self.settle();
    }
    fn set_editor(&mut self, words: &str) {
        let words = words.to_string();
        self.rt.update(move |s, _| s.settings.harnesses.weave.config.editor = TextEdit::new(words));
        self.settle();
    }
    fn calls(&self, route: &str) -> Vec<(String, String, String, Value)> {
        self.fake.lock().unwrap().config_calls.iter().filter(|c| c.1 == route).cloned().collect()
    }
    fn wait_calls(&mut self, route: &'static str, n: usize) {
        let fake = self.fake.clone();
        self.wait(&format!("{n} request(s) to {route}"), move |_| {
            fake.lock().unwrap().config_calls.iter().filter(|c| c.1 == route).count() >= n
        });
    }
    fn key(&mut self, key: Key) {
        self.rt.key(KeyEvent { key, mods: Mods::default() });
        self.frames();
    }
}

fn check_ok() -> (u16, Value) {
    (
        200,
        json!([{"harnessType": "opencode", "harnessName": "OpenCode", "flavor": "weave", "check": {"ok": true, "agents": ["loom", "reviewer"]}}]),
    )
}

// ─── WeaveConfigDialog.test.ts ──────────────────────────────────────────────────

#[test]
fn can_t_keep_the_config_in_fleet_when_the_installed_weave_can_t_read_fleet_s_folder() {
    let mut r = rig(Fake { weave: vec![opencode(vec![old_legacy()])], ..Fake::default() });
    r.open_dialog();
    assert!(r.has("weave-source-fleet"));
    assert!(r.disabled("weave-source-fleet"));
    assert!(r.words().contains("Needs Weave Legacy 0.9.0 or later"));
    crate::tests::click(&mut r.rt, "weave-source-fleet");
    assert!(!cfg(&r).fleet_source, "a disabled card takes no press");
}

#[test]
fn asks_for_weave_to_load_not_for_an_update_when_the_listed_weave_didn_t_load() {
    let broken = json!({"flavor": "weave", "package": "@weaveio/weave-adapter-opencode2", "entry": "@weaveio/weave-adapter-opencode2@0.3.0",
        "acceptsFleetConfig": false, "error": "No matching version found for @ai-sdk/gateway@3.0.225."});
    let mut r = rig(Fake { weave: vec![opencode(vec![broken])], ..Fake::default() });
    r.open_dialog();
    assert!(r.disabled("weave-source-fleet"));
    assert!(r.words().contains("Needs Weave to load first"));
    assert!(!r.words().contains("or later"));
}

#[test]
fn starts_fleet_s_config_from_a_short_template_and_tries_it_with_the_harness() {
    let mut r = rig(Fake { weave: vec![opencode(vec![weave_install()])], check: Some(check_ok()), ..Fake::default() });
    r.open_dialog();
    r.go("weave-source-fleet");
    assert_eq!(cfg(&r).active, "config.weave");
    assert!(cfg(&r).editor.text.contains("Fleet's Weave config"));
    assert!(r.has("weave-editor"));

    r.set_editor("agent reviewer {}");
    r.go("weave-test");
    r.wait_calls("/api/weave/check", 1);
    r.wait("the result", |s| s.settings.harnesses.weave.config.checks.is_some());
    let calls = r.calls("/api/weave/check");
    assert_eq!(calls.len(), 1, "one Weave, one check");
    assert_eq!(calls[0].3, json!({"flavor": "weave", "files": {"config.weave": "agent reviewer {}"}}));
    assert!(r.words().contains("OpenCode loads 2 Weave agents"), "{}", r.words());
    assert!(r.words().contains("reviewer"));
    assert!(r.calls("/api/weave").is_empty(), "Test saves nothing");
}

#[test]
fn shows_where_weave_couldn_t_read_the_config_and_that_nothing_was_saved() {
    let refused = json!({"saved": false, "config": null, "checks": [{"harnessType": "opencode", "harnessName": "OpenCode", "flavor": "weave",
        "check": {"ok": false, "agents": [], "error": "OpenCode started, but Weave added no agents.", "details": ["config.weave:2:1 UnclosedBlock"]}}]});
    let mut r = rig(Fake {
        weave: vec![opencode(vec![weave_install()])],
        source: "fleet",
        files: json!({"config.weave": "agent a {}"}),
        save: Some((200, refused)),
        ..Fake::default()
    });
    r.open_dialog();
    r.set_editor("agent a {");
    r.go("weave-save");
    r.wait_calls("/api/weave", 1);
    r.wait("the refusal", |s| s.settings.harnesses.weave.config.checks.is_some());
    let calls = r.calls("/api/weave");
    assert_eq!(calls[0].0, "PUT");
    assert_eq!(calls[0].3, json!({"source": "fleet", "files": {"config.weave": "agent a {"}}));
    assert!(r.words().contains("config.weave:2:1 UnclosedBlock"), "{}", r.words());
    assert!(r.words().contains("OpenCode started, but Weave added no agents."));
    assert!(r.words().contains("Nothing was saved"));
    assert!(!r.words().contains("Saved."));
}

#[test]
fn adds_a_prompt_file_next_to_the_config() {
    let mut r =
        rig(Fake { weave: vec![opencode(vec![weave_install()])], source: "fleet", files: json!({"config.weave": ""}), ..Fake::default() });
    r.open_dialog();
    r.go("weave-add-prompt");
    assert!(cfg(&r).adding && r.has("weave-new-prompt"));
    r.rt.update(|s, _| s.settings.harnesses.weave.config.new_prompt = TextEdit::new("team"));
    r.settle();
    r.go("weave-prompt-add");
    assert!(r.has("weave-tab-prompts/team.md"));
    assert_eq!(cfg(&r).active, "prompts/team.md");
    assert!(!cfg(&r).adding);
    assert!(cfg(&r).files.contains_key("prompts/team.md"));
    assert!(r.has("weave-remove-prompts/team.md"));
    r.go("weave-remove-prompts/team.md");
    assert!(!r.has("weave-tab-prompts/team.md"));
    assert_eq!(cfg(&r).active, "config.weave");
}

#[test]
fn lists_running_folders_until_each_has_reloaded() {
    let mut r = rig(Fake {
        weave: vec![opencode(vec![weave_install()])],
        source: "fleet",
        files: json!({"config.weave": ""}),
        apply: json!({"folders": [{"directory": "/work/alpha", "reloaded": true}, {"directory": "/work/beta", "reloaded": false}]}),
        ..Fake::default()
    });
    r.open_dialog();
    assert!(r.has("weave-apply"));
    let words = r.words();
    assert!(words.contains("Applied to 1 of 2 running folders"), "{words}");
    assert!(words.contains("Reloaded"));
    assert!(words.contains("Waiting \u{b7} turn running"));
    assert!(words.contains("alpha") && words.contains("/work/beta"));
}

#[test]
fn switching_back_to_the_user_s_own_file_is_saved_as_such() {
    let mut r =
        rig(Fake { weave: vec![opencode(vec![weave_install()])], source: "fleet", files: json!({"config.weave": "x"}), ..Fake::default() });
    r.open_dialog();
    r.go("weave-source-own");
    r.go("weave-save");
    r.wait_calls("/api/weave", 1);
    let calls = r.calls("/api/weave");
    assert_eq!(calls[0].3, json!({"source": "own", "files": {"config.weave": "x"}}));
    r.wait("the notice", |s| s.settings.harnesses.weave.config.notice.is_some());
    assert!(r.words().contains("Saved. New sessions read your own Weave files."));
}

// ─── HarnessesSection.test.ts ───────────────────────────────────────────────────

#[test]
fn keeps_the_weave_config_draft_across_a_tab_switch() {
    let mut r =
        rig(Fake { weave: vec![opencode(vec![weave_install()]), detection("opencode2", "OpenCode 2", vec![], None)], ..Fake::default() });
    r.open_dialog();
    r.go("weave-source-fleet");
    assert!(cfg(&r).fleet_source);
    r.set_editor("agent mine {}");
    r.key(Key::Escape);
    assert!(!cfg(&r).open, "Esc closes the dialog");
    r.tab("opencode2");
    r.tab("opencode");
    r.go("weave-edit-config");
    assert!(cfg(&r).open && cfg(&r).fleet_source);
    assert_eq!(cfg(&r).current_files().get("config.weave").map(String::as_str), Some("agent mine {}"));
}

// ─── Ours ───────────────────────────────────────────────────────────────────────

#[test]
fn esc_in_the_prompt_box_cancels_the_prompt_and_keeps_the_dialog() {
    let mut r =
        rig(Fake { weave: vec![opencode(vec![weave_install()])], source: "fleet", files: json!({"config.weave": ""}), ..Fake::default() });
    r.open_dialog();
    r.go("weave-add-prompt");
    crate::tests::click(&mut r.rt, "weave-new-prompt");
    r.key(Key::Escape);
    assert!(!cfg(&r).adding, "the form turned back into the + prompt tab");
    assert!(cfg(&r).open, "the dialog stayed");
    assert!(r.has("weave-add-prompt"));
}

#[test]
fn a_prompt_name_must_be_plain_and_new() {
    let mut r = rig(Fake {
        weave: vec![opencode(vec![weave_install()])],
        source: "fleet",
        files: json!({"config.weave": "", "prompts/team.md": ""}),
        ..Fake::default()
    });
    r.open_dialog();
    r.go("weave-add-prompt");
    for (typed, said) in
        [("a b", Some("Use letters, digits, '-', '_' and '.'.")), ("team", Some("There's already a prompt with that name.")), ("ok", None)]
    {
        r.rt.update(move |s, _| s.settings.harnesses.weave.config.new_prompt = TextEdit::new(typed));
        r.settle();
        assert_eq!(cfg(&r).prompt_problem(), said, "{typed}");
        assert_eq!(r.has("weave-prompt-problem"), said.is_some());
        assert_eq!(r.disabled("weave-prompt-add"), said.is_some());
    }
}

#[test]
fn copies_the_own_file_into_fleet_s_config_and_saves_nothing() {
    let mut r = rig(Fake {
        weave: vec![opencode(vec![weave_install()])],
        own: json!({"weave": {"path": "~/.weave/config.weave", "files": {"config.weave": "agent own {}", "prompts/p.md": "hi"}}}),
        ..Fake::default()
    });
    r.open_dialog();
    assert!(!cfg(&r).fleet_source);
    r.go("weave-copy-own");
    r.wait("the copy", |s| s.settings.harnesses.weave.config.notice.is_some());
    assert_eq!(r.calls("/api/weave/own")[0].2, "flavor=weave");
    let c = cfg(&r);
    assert!(c.fleet_source);
    assert_eq!(c.files.get("config.weave").map(String::as_str), Some("agent own {}"));
    assert!(c.files.contains_key("prompts/p.md"));
    assert_eq!(c.notice.as_deref(), Some("Copied from ~/.weave/config.weave. Not saved yet."));
    assert!(r.calls("/api/weave").is_empty());
    assert!(r.has("weave-save") && !r.disabled("weave-save"), "a copy is a change to save");
}

#[test]
fn an_empty_own_file_says_there_was_nothing_to_copy() {
    let mut r = rig(Fake { weave: vec![opencode(vec![weave_install()])], ..Fake::default() });
    r.open_dialog();
    r.go("weave-copy-own");
    r.wait("the answer", |s| s.settings.harnesses.weave.config.notice.is_some());
    assert_eq!(cfg(&r).notice.as_deref(), Some("Your own Weave files are empty, so there was nothing to copy."));
    assert!(cfg(&r).fleet_source, "Fleet is picked all the same, as Fleet does");
}

#[test]
fn save_waits_for_a_change_and_files_of_a_weave_this_machine_does_not_run_stay_out_of_sight() {
    let mut r = rig(Fake {
        weave: vec![detection("opencode", "OpenCode", vec![v2_install()], None)],
        source: "fleet",
        files: json!({"config.weave": "a", "weave-opencode.jsonc": "{}"}),
        ..Fake::default()
    });
    r.open_dialog();
    assert!(r.disabled("weave-save"), "nothing changed and the source is the saved one");
    assert!(r.has("weave-tab-config.weave") && !r.has("weave-tab-weave-opencode.jsonc"));
    r.set_editor("b");
    assert!(!r.disabled("weave-save"));
    assert!(cfg(&r).files.contains_key("weave-opencode.jsonc"), "kept, out of the way");
}

#[test]
fn a_load_does_not_overwrite_a_draft_with_changes_but_fills_a_clean_one() {
    let mut r = rig(Fake {
        weave: vec![opencode(vec![weave_install()])],
        source: "fleet",
        files: json!({"config.weave": "saved"}),
        ..Fake::default()
    });
    r.open_dialog();
    assert_eq!(cfg(&r).editor.text, "saved");
    r.set_editor("mine");
    r.go("weave-source-own");
    let before = r.reads();
    r.rt.update(|s, _| s.ask_weave(false));
    let fake = r.fake.clone();
    r.wait("a second read of the config", move |s| !s.settings.harnesses.weave.loading && fake.lock().unwrap().reads.len() > before);
    assert_eq!(cfg(&r).current_files().get("config.weave").map(String::as_str), Some("mine"));
    assert!(!cfg(&r).fleet_source, "the draft's own pick stays");
}
