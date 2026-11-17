//! Component 16, piece M: Settings → What agents can do, Edit files and Run commands. `permission-reach.test.ts` and the cases of
//! `PermissionsSection.test.ts` marked "A" in docs/parity/16-settings.md, under their names, against a fake Fleet that answers
//! the harness list and the preferences and records every `PUT /api/preferences/{key}`.
use crate::asks::rules::PermissionLevel::{self, All, Ask, Edits};
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::settings::permissions::Reach::{Allowed, Ask as AskReach};
use crate::fleet::settings::permissions::{Row, default_tag, level_with, reach_of};
use crate::tests::{FRAME, app};
use crate::tests_settings_harnesses::{Fake, Shared, info, serve};
use crate::{Rail, State, test_wait};
use rune::Runtime;
use serde_json::{Value, json};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

// ─── permission-reach.test.ts ───────────────────────────────────────────────────

#[test]
fn reads_each_level_as_two_rows() {
    assert_eq!((reach_of(Ask).edits, reach_of(Ask).commands), (AskReach, AskReach));
    assert_eq!((reach_of(Edits).edits, reach_of(Edits).commands), (Allowed, AskReach));
    assert_eq!((reach_of(All).edits, reach_of(All).commands), (Allowed, Allowed));
}

#[test]
fn from_a_level_a_row_set_gives() {
    let cases = [
        (All, Row::Edits, AskReach, Ask),
        (All, Row::Commands, AskReach, Edits),
        (Edits, Row::Edits, AskReach, Ask),
        (Edits, Row::Commands, Allowed, All),
        (Ask, Row::Edits, Allowed, Edits),
        (Ask, Row::Commands, Allowed, All),
    ];
    for (level, row, value, expected) in cases {
        assert_eq!(level_with(level, row, value), expected, "from {level:?}, {row:?} {value:?}");
    }
}

#[test]
fn never_lets_commands_run_unasked_while_edits_ask() {
    for level in [Ask, Edits, All] {
        for row in [Row::Edits, Row::Commands] {
            for value in [Allowed, AskReach] {
                let reach = reach_of(level_with(level, row, value));
                assert!(!(reach.edits == AskReach && reach.commands == Allowed), "{level:?} {row:?} {value:?}");
                assert_eq!(reach.on(row), value);
            }
        }
    }
}

#[test]
fn words_the_default_tag() {
    assert_eq!(default_tag(true, "Allowed"), "default");
    assert_eq!(default_tag(false, "Allowed"), "default: Allowed");
}

// ─── PermissionsSection.test.ts ─────────────────────────────────────────────────

/// OpenCode 2 and Claude Code take a level and the browser; Pi takes neither. Claude Code says what its `edits` level means.
fn list() -> Value {
    json!([
        info("opencode2", "OpenCode 2", "ready", json!({})),
        info(
            "claude-code",
            "Claude Code",
            "ready",
            json!({"presentation": {"eyebrow": "CLI harness", "description": "", "permissionModes": {"edits": "Accept edits, ask for commands"}}})
        ),
        info("pi", "Pi", "ready", json!({})),
    ])
}

struct Rig {
    rt: Runtime<State>,
    queue: Receiver<Job>,
    fake: Shared,
}

fn rig(prefs: &[(&str, &str)], browser_on: bool) -> Rig {
    let fake: Shared = Arc::new(Mutex::new(Fake {
        harnesses: list(),
        prefs: prefs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
        browser_on,
        ..Default::default()
    }));
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
    rt.set_viewport((1280.0, 900.0), 1.0);
    rt.update(move |s, _| {
        s.link = Some(link);
        s.pickers.prefs = Default::default();
        s.rail = Rail::Settings;
        s.settings.set("permissions", None);
    });
    let mut r = Rig { rt, queue, fake };
    r.rt.frame(FRAME);
    r.wait("the harnesses and the preferences", |s| s.harness_list().all.len() == 3 && s.harness_prefs().fetched);
    r
}

impl Rig {
    fn wait(&mut self, what: &str, done: impl FnMut(&State) -> bool) {
        test_wait::drive(&mut self.rt, &self.queue, what, done, |s| format!("{:?}", s.settings.permissions));
        self.frames();
    }
    fn frames(&mut self) {
        self.rt.frame(FRAME);
        self.rt.frame(FRAME);
    }
    fn info(&self, key: &str) -> Option<rune::ElementInfo> {
        self.rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting)
    }
    fn click(&mut self, key: &str) {
        crate::tests::click(&mut self.rt, key);
        self.frames();
    }
    /// A segment is on when it has a fill.
    fn on(&self, key: &str) -> bool {
        self.info(key).unwrap_or_else(|| panic!("no element keyed {key:?}")).fill.a > 0.0
    }
    fn words(&self, key: &str) -> String {
        let all = self.rt.elements();
        let at = all.iter().position(|e| e.key.as_deref() == Some(key)).unwrap_or_else(|| panic!("no element keyed {key:?}"));
        let prefix = all[at].path.clone();
        all.iter().filter(|e| e.path.starts_with(&prefix)).filter_map(|e| e.text.clone()).collect::<Vec<_>>().join("")
    }
    fn put(&mut self, key: &str, value: &str) {
        let fake = self.fake.clone();
        let (k, v) = (key.to_string(), value.to_string());
        self.wait(&format!("{key} = {value} to reach Fleet"), move |_| fake.lock().unwrap().puts.contains(&(k.clone(), v.clone())));
    }
    fn level(&self) -> PermissionLevel {
        self.rt.state.permission_default()
    }
}

#[test]
fn keeps_today_s_defaults_edits_and_commands_allowed_each_marked_default() {
    let r = rig(&[], false);
    assert!(r.on("reach-edits-allowed") && !r.on("reach-edits-ask"));
    assert!(r.on("reach-commands-allowed") && !r.on("reach-commands-ask"));
    for row in ["edits", "commands"] {
        assert_eq!(r.words(&format!("reach-tag-{row}")), "default");
    }
}

#[test]
fn lists_edit_files_then_run_commands() {
    let r = rig(&[], false);
    let (edits, commands) = (r.info("reach-row-edits").unwrap(), r.info("reach-row-commands").unwrap());
    assert!(edits.rect[1] < commands.rect[1]);
    assert!(r.words("reach-row-edits").contains("Change and create files in the session's folder or worktree."));
    assert!(r.words("reach-row-commands").contains("The shell, web fetch, the browser and other tools."));
    assert!(r.words("reach-row-commands").contains("A command can change files too, so these go together"));
}

#[test]
fn saves_edit_files_ask_as_the_ask_level_which_makes_commands_ask_too() {
    let mut r = rig(&[], false);
    r.click("reach-edits-ask");
    assert_eq!(r.level(), Ask, "the value shows at once");
    r.put("PermissionLevel", "ask");
    assert!(r.on("reach-commands-ask"));
    assert_eq!(r.words("reach-tag-edits"), "default: Allowed");
}

#[test]
fn saves_run_commands_ask_as_the_edits_level_edits_still_run() {
    let mut r = rig(&[], false);
    r.click("reach-commands-ask");
    r.put("PermissionLevel", "edits");
    assert!(r.on("reach-edits-allowed"));
    assert_eq!(r.words("reach-tag-commands"), "default: Allowed");
    assert_eq!(r.words("reach-tag-edits"), "default");
}

#[test]
fn lists_a_harness_at_a_level_of_its_own_under_the_rows_it_differs_on_and_one_fleet_can_t_set_on_both() {
    let r = rig(&[("PermissionLevel", "ask"), ("PermissionLevel.claude-code", "edits")], false);
    assert_eq!(r.words("reach-exception-edits-claude-code"), "Claude CodeAllowed");
    assert!(r.info("reach-exception-commands-claude-code").is_none(), "commands still ask for it, as for the default");
    assert!(r.words("reach-unleveled-edits-pi").contains("its own settings: Fleet can't set it"));
    assert!(r.info("reach-unleveled-commands-pi").is_some());
    // What Fleet hands the harness shows as the chip's tooltip, in the harness's own words.
    assert_eq!(r.info("reach-exception-edits-claude-code").unwrap().help.as_deref(), Some("Accept edits, ask for commands"));
}

#[test]
fn adds_an_exception_from_plus_exception_and_a_harness_set_back_to_the_default_s_value_follows_the_default_again() {
    let mut r = rig(&[], false);
    r.click("reach-add-commands");
    assert_eq!(r.words("reach-add-list-menu"), "OpenCode 2 \u{b7} AskClaude Code \u{b7} Ask", "Pi can't take a level, so it isn't offered");
    r.click("reach-add-list-opt1");
    r.put("PermissionLevel.claude-code", "edits");
    assert_eq!(r.words("reach-exception-commands-claude-code"), "Claude CodeAsk");
    r.click("reach-exception-commands-claude-code-select");
    assert_eq!(r.words("reach-chip-list-menu"), "Allowed (as the default)Ask");
    r.click("reach-chip-list-opt0");
    r.put("PermissionLevel.claude-code", "");
    assert!(r.info("reach-exception-commands-claude-code").is_none(), "it follows the default again");
}

#[test]
fn says_whether_agents_get_the_browser_on_run_commands_and_opens_the_browser_group_from_it() {
    let mut r = rig(&[], true);
    assert_eq!(r.words("reach-browser-chip"), "BrowserOn, not in Pi");
    r.click("reach-browser-chip");
    assert_eq!(r.rt.state.settings.anchor.as_deref(), Some("browser"));
    assert_eq!(r.rt.state.settings.page, crate::fleet::settings_nav::Page::Permissions);
}

#[test]
fn shows_the_browser_as_off_when_agents_can_t_use_it() {
    let mut r = rig(&[], false);
    r.wait("the browser's answer", |s| s.settings.harnesses.browser_off);
    assert_eq!(r.words("reach-browser-chip"), "BrowserOff");
}

// Ours: the rest of the rows' rules.

#[test]
fn names_a_browser_no_harness_here_can_use() {
    let mut r = rig(&[], true);
    r.fake.lock().unwrap().harnesses = json!([info("pi", "Pi", "ready", json!({}))]);
    r.rt.update(|s, _| s.read_harnesses("home", true));
    r.wait("only Pi", |s| s.permission_harnesses().len() == 1);
    assert_eq!(r.words("reach-browser-chip"), "BrowserNo harness here can use it");
    assert!(r.info("reach-add-edits").is_none(), "no harness can take a level, so there is none to add");
}

#[test]
fn a_harness_that_is_turned_off_is_not_listed() {
    let r = rig(&[("claude-code.enabled", "false"), ("PermissionLevel", "ask"), ("PermissionLevel.claude-code", "edits")], false);
    assert!(r.info("reach-exception-edits-claude-code").is_none());
    assert_eq!(r.rt.state.permission_harnesses().len(), 2);
}

#[test]
fn a_refused_save_puts_the_old_level_back() {
    let mut r = rig(&[], false);
    r.fake.lock().unwrap().refuse.push("PermissionLevel".into());
    r.click("reach-edits-ask");
    r.wait("the old level back", |s| s.permission_default() == All);
    assert!(r.on("reach-edits-allowed"));
}

#[test]
fn the_page_asks_the_machine_settings_shows() {
    use crate::mock::machines::Case;
    let mut rt = app();
    rt.set_viewport((1280.0, 900.0), 1.0);
    rt.update(|s, _| {
        s.show_machines(Case::Several, true);
        s.rail = Rail::Settings;
        s.settings.set("permissions", None);
        s.feeds.prefs.entry("m-harbor".to_string()).or_default().set("PermissionLevel", "ask");
        s.feeds.prefs.get_mut("m-harbor").unwrap().fetched = true;
    });
    assert_eq!(rt.state.permission_default(), All, "home asks nothing");
    rt.update(|s, _| s.settings.choice.pick("m-harbor"));
    assert_eq!(rt.state.permission_default(), Ask, "harbor's own preferences");
}
