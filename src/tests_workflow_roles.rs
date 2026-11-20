//! Component 18, piece E: Model roles and the Run box's Models. Fleet's `WorkflowModelRoles.test.ts` (4) and
//! `WorkflowModelRolesEscape.test.ts` (3) under their names, then what the native app keeps: a pick is saved to the
//! preference with the other roles kept, a new model starts on its own effort, an override equal to the role is none, and the
//! Run box's popover.
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::new_session::harness_choice::{HarnessEntry, Harnesses, Load};
use crate::fleet::settings::HOME_MACHINE_KEY;
use crate::fleet::workflows::roles::{Place, with_override};
use crate::fleet::workflows::{ModelChoice, Role};
use crate::mock::workflows::RolesScene;
use crate::tests::{FRAME, app_tall, click};
use crate::tests_workflow_runs::{Fake, fake};
use crate::{State, test_wait};
use rune::{Key, KeyEvent, Mods, Runtime};
use serde_json::{Value, json};
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, channel};

const PUT_ROLES: &str = "/api/preferences/WorkflowModelRoles";

struct Rig {
    rt: Runtime<State>,
    queue: Receiver<Job>,
    home: Fake,
}

/// The Workflows page on a fake Fleet that keeps the roles preference, the dialog or popover drawn from the invented feeds.
fn rig(which: RolesScene) -> Rig {
    let home = fake();
    home.route("GET", "/api/preferences", 200, json!({"Workflows": "true"}));
    home.route("PUT", PUT_ROLES, 200, json!({}));
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    let link =
        Link::start_machine(HOME_MACHINE_KEY.to_string(), home.base.clone(), Post::new(move |job| drop(jobs.lock().unwrap().send(job))));
    let mut rt = app_tall();
    rt.update(move |s, _| {
        s.link = Some(link);
        s.pickers.prefs = Default::default();
        s.load_preferences();
    });
    rt.frame(FRAME);
    test_wait::drive(&mut rt, &queue, "the preferences", |s| s.pickers.prefs.fetched, |_| String::new());
    rt.update(move |s, _| {
        s.show_roles(true, which);
        // The scene is the state: nothing of the box is read from the fake.
        s.workflows_nav.run_box.asked = true;
        s.workflows_nav.run_box.initial_done = true;
        s.pickers.prefs.fetched = true;
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    Rig { rt, queue, home }
}

/// The dialog as the mock draws it, with no Fleet to talk to.
fn bare(which: RolesScene) -> Runtime<State> {
    let mut rt = app_tall();
    rt.update(move |s, _| s.show_roles(true, which));
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt
}

fn has(rt: &Runtime<State>, key: &str) -> bool {
    rt.find(key).is_some()
}

fn words(rt: &Runtime<State>) -> String {
    rt.elements().iter().filter_map(|e| e.text.clone()).collect::<Vec<_>>().join("\n")
}

fn harness(kind: &str, name: &str, workflow_steps: bool, enabled: bool) -> HarnessEntry {
    HarnessEntry {
        kind: kind.into(),
        display_name: name.into(),
        available: true,
        user_enabled: enabled,
        supports_workflow_steps: workflow_steps,
        state: "ready".into(),
        ..HarnessEntry::default()
    }
}

fn set_harnesses(rt: &mut Runtime<State>, all: Vec<HarnessEntry>, default: &str) {
    let default = default.to_string();
    rt.update(move |s, _| {
        s.feeds.harnesses.entry(HOME_MACHINE_KEY.into()).or_default().harnesses =
            Harnesses { all, load: Load::Done, preferred: Some(default) };
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
}

fn escape(rt: &mut Runtime<State>) {
    rt.key(KeyEvent { key: Key::Escape, mods: Mods::default() });
    rt.frame(FRAME);
    rt.frame(FRAME);
}

/// Presses the row of the open menu named `label`.
fn pick(rt: &mut Runtime<State>, label: &str) {
    let key = rt
        .elements()
        .into_iter()
        .find(|e| e.role == "option" && e.label.as_deref() == Some(label))
        .and_then(|e| e.key)
        .unwrap_or_else(|| panic!("no option {label:?} in the menu"));
    click(rt, &key);
    rt.frame(FRAME);
}

fn menu_open(rt: &Runtime<State>) -> bool {
    rt.state.workflows_nav.roles.menu.is_some()
}

// ─── WorkflowModelRoles.test.ts ─────────────────────────────────────────────────────────────────────────────────────

#[test]
fn shows_a_role_for_each_kind_of_work_once_workflows_is_on() {
    let mut rt = bare(RolesScene::Dialog);
    // Only harnesses that run workflow steps have roles to set.
    set_harnesses(
        &mut rt,
        vec![harness("opencode2", "OpenCode 2", true, true), harness("claude-code", "Claude Code", false, true)],
        "opencode2",
    );
    assert!(has(&rt, "workflows-roles-dialog"));
    for role in ["strong", "standard", "fast"] {
        assert!(has(&rt, &format!("workflows-role-{role}")), "{role}");
    }
    assert!(!words(&rt).contains("Claude Code"));
}

#[test]
fn draws_nothing_while_the_dialog_is_closed() {
    let mut rt = bare(RolesScene::Dialog);
    rt.update(|s, _| s.workflows_nav.roles_open = false);
    rt.frame(FRAME);
    assert!(!has(&rt, "workflows-roles-dialog"));
    rt.update(|s, _| s.workflows_nav.roles_open = true);
    rt.frame(FRAME);
    assert!(has(&rt, "workflows-roles-dialog"));
}

#[test]
fn closes_the_dialog_for_good_when_the_page_goes_away_so_coming_back_doesn_t_reopen_it() {
    let mut r = rig(RolesScene::Dialog);
    assert!(r.rt.state.workflows_nav.roles_open);
    r.rt.update(|s, _| s.rail = crate::Rail::Sessions);
    r.rt.frame(FRAME);
    r.rt.frame(FRAME);
    assert!(!r.rt.state.workflows_nav.roles_open);
    r.rt.update(|s, _| s.enter_workflows());
    r.rt.frame(FRAME);
    assert!(!r.rt.state.workflows_nav.roles_open, "the flag was cleared on the way out");
    assert!(!has(&r.rt, "workflows-roles-dialog"));
}

#[test]
fn names_the_harnesses_that_run_workflows_from_their_capabilities_when_none_of_those_is_on() {
    let mut rt = bare(RolesScene::Dialog);
    set_harnesses(
        &mut rt,
        vec![
            harness("opencode", "OpenCode", true, false),
            harness("claude-code", "Claude Code", false, true),
            harness("acme", "Acme Code", true, false),
        ],
        "opencode",
    );
    assert!(
        words(&rt).contains("Workflows run on OpenCode and Acme Code. Turn one of them on in Settings \u{2192} Harnesses."),
        "{}",
        words(&rt)
    );
    assert!(!has(&rt, "workflows-role-strong"));
    set_harnesses(&mut rt, vec![harness("opencode", "OpenCode", true, false)], "opencode");
    assert!(words(&rt).contains("Workflows run on OpenCode. Turn it on in Settings \u{2192} Harnesses."));
    set_harnesses(&mut rt, vec![harness("claude-code", "Claude Code", false, true)], "claude-code");
    assert!(words(&rt).contains("None of the harnesses here can run workflows yet."));
}

// ─── WorkflowModelRolesEscape.test.ts ───────────────────────────────────────────────────────────────────────────────

#[test]
fn closes_only_the_open_model_menu_on_escape_from_its_filter_then_the_dialog_on_the_next() {
    let mut rt = bare(RolesScene::Dialog);
    click(&mut rt, "workflows-role-model-strong");
    assert!(menu_open(&rt) && has(&rt, "wf-model-filter"), "the menu opens with its filter");
    escape(&mut rt);
    assert!(!menu_open(&rt) && !has(&rt, "wf-model-pop-menu"));
    assert!(rt.state.workflows_nav.roles_open, "the dialog stays");
    escape(&mut rt);
    assert!(!rt.state.workflows_nav.roles_open, "the next press closes it");
}

#[test]
fn closes_only_the_menu_on_escape_while_focus_is_on_the_trigger() {
    let mut rt = bare(RolesScene::Dialog);
    click(&mut rt, "workflows-role-model-strong");
    rt.update(|_, tx| tx.focus("workflows-role-model-strong".to_string(), false));
    rt.frame(FRAME);
    escape(&mut rt);
    assert!(!menu_open(&rt));
    assert!(rt.state.workflows_nav.roles_open);
}

#[test]
fn closes_the_dialog_on_escape_when_no_menu_is_open_even_with_focus_on_a_selector() {
    let mut rt = bare(RolesScene::Dialog);
    rt.update(|_, tx| tx.focus("workflows-role-model-strong".to_string(), false));
    rt.frame(FRAME);
    escape(&mut rt);
    assert!(!rt.state.workflows_nav.roles_open);
}

// ─── What the native app keeps ──────────────────────────────────────────────────────────────────────────────────────

#[test]
fn an_override_equal_to_the_role_is_none() {
    let mapped = Some("anthropic/opus");
    let one = with_override(&[], Role::Strong, Some("anthropic/sonnet".into()), mapped);
    assert_eq!(one, vec![(Role::Strong, ModelChoice { model: Some("anthropic/sonnet".into()), effort: None })]);
    assert!(with_override(&one, Role::Strong, Some("anthropic/opus".into()), mapped).is_empty(), "back to what the role says");
    // Default model is a choice too, when the role maps to a model.
    assert_eq!(with_override(&[], Role::Fast, None, mapped), vec![(Role::Fast, ModelChoice::default())]);
    assert!(with_override(&[], Role::Fast, None, None).is_empty(), "and none when the role maps to Default");
    // Other roles stay, in Strong, Standard, Fast order.
    let two = with_override(&one, Role::Standard, Some("orbit/orbit-4".into()), Some("anthropic/sonnet"));
    assert_eq!(two.iter().map(|(r, _)| *r).collect::<Vec<_>>(), vec![Role::Strong, Role::Standard]);
}

#[test]
fn the_models_popover_lists_the_roles_used_and_counts_what_this_run_changes() {
    let mut rt = bare(RolesScene::Popover);
    assert!(has(&rt, "workflow-models-pop"));
    for role in ["strong", "standard", "fast"] {
        assert!(has(&rt, &format!("workflow-model-{role}")), "{role}");
    }
    assert!(words(&rt).contains("Only for this run. The roles themselves are set under Model roles, in the Workflows list."));
    // Strong maps to Opus: picking Opus changes nothing; Haiku is one change.
    click(&mut rt, "workflow-model-strong");
    assert!(has(&rt, "wf-model-pop-menu") && has(&rt, "workflow-models-pop"), "the menu opens over the popover");
    pick(&mut rt, "Opus");
    assert!(rt.state.workflows_nav.run_box.overrides.is_empty());
    assert!(has(&rt, "workflow-models-pop"), "the popover is still up after a pick");
    click(&mut rt, "workflow-model-strong");
    pick(&mut rt, "Haiku");
    assert_eq!(rt.state.workflows_nav.run_box.overrides.len(), 1);
    escape(&mut rt);
    assert!(words(&rt).contains("Models \u{b7} 1 changed"), "{}", words(&rt));
    // Nothing was written: it is only for this run.
    assert_eq!(rt.state.role_choice("opencode2", Role::Strong).model.as_deref(), Some("anthropic/opus"));
}

#[test]
fn the_popover_asks_only_for_the_roles_the_enabled_steps_use() {
    let mut rt = bare(RolesScene::Popover);
    // Fix a bug's optional step is the only Standard step besides Fix; Fast is used by the last one. Drop every Fast step.
    rt.update(|s, _| {
        let w = s.workflows_nav.library.as_mut().unwrap().workflows.iter_mut().find(|w| w.id == "builtin:fix-a-bug").unwrap();
        w.steps.retain(|st| st.model.as_deref() != Some("fast"));
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(has(&rt, "workflow-model-strong") && has(&rt, "workflow-model-standard") && !has(&rt, "workflow-model-fast"));
}

#[test]
fn a_harness_that_lists_no_models_shows_the_fixed_model_name_in_the_popover() {
    let mut rt = bare(RolesScene::Popover);
    rt.update(|s, _| {
        s.feeds.catalogs.get_mut(HOME_MACHINE_KEY).unwrap().shown.as_mut().unwrap().supported = false;
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    let strong = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("workflow-model-strong")).unwrap();
    assert_eq!(strong.text.as_deref(), Some("Opus"), "the role's model, as text and not a menu");
    assert!(strong.role != "button");
}

#[test]
fn picking_a_model_saves_it_with_the_other_roles_kept_and_starts_on_its_own_effort() {
    let mut r = rig(RolesScene::Dialog);
    click(&mut r.rt, "workflows-role-model-strong");
    pick(&mut r.rt, "Haiku");
    let calls = r.home.calls_handle();
    test_wait::drive(
        &mut r.rt,
        &r.queue,
        "the roles preference to be saved",
        move |_| calls().iter().any(|(m, p)| m == "PUT" && p == PUT_ROLES),
        |_| String::new(),
    );
    let body = &r.home.bodies("PUT", PUT_ROLES)[0];
    let saved: Value = serde_json::from_str(body["value"].as_str().unwrap()).unwrap();
    assert_eq!(saved["opencode2"]["strong"], json!({"model": "anthropic/haiku", "effort": null}), "a new model starts on its own effort");
    assert_eq!(saved["opencode2"]["standard"], json!({"model": "anthropic/sonnet", "effort": null}), "the others are kept");
    assert!(words(&r.rt).contains("Haiku"), "the trigger shows it at once");
}

#[test]
fn picking_an_effort_keeps_the_model() {
    let mut r = rig(RolesScene::Dialog);
    click(&mut r.rt, "workflows-role-effort-strong");
    assert!(has(&r.rt, "wf-effort-pop-menu"));
    pick(&mut r.rt, "Effort: low");
    let calls = r.home.calls_handle();
    test_wait::drive(
        &mut r.rt,
        &r.queue,
        "the roles preference to be saved",
        move |_| calls().iter().any(|(m, p)| m == "PUT" && p == PUT_ROLES),
        |_| String::new(),
    );
    let saved: Value = serde_json::from_str(r.home.bodies("PUT", PUT_ROLES)[0]["value"].as_str().unwrap()).unwrap();
    assert_eq!(saved["opencode2"]["strong"], json!({"model": "anthropic/opus", "effort": "low"}));
}

#[test]
fn the_effort_menu_is_off_for_a_model_with_no_efforts_and_for_the_default_model() {
    let mut rt = bare(RolesScene::Dialog);
    let off = |rt: &Runtime<State>, role: &str| {
        rt.elements().into_iter().find(|e| e.key.as_deref() == Some(&format!("workflows-role-effort-{role}"))).unwrap().opacity < 1.0
    };
    assert!(!off(&rt, "strong"), "Opus has efforts");
    assert!(off(&rt, "fast"), "the default model has none to offer");
    rt.update(|s, _| {
        s.pickers
            .prefs
            .values
            .insert("WorkflowModelRoles".into(), json!({"opencode2": {"fast": {"model": "anthropic/haiku", "effort": null}}}).to_string());
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(off(&rt, "fast"), "Haiku lists none");
}

#[test]
fn each_role_says_which_steps_use_it_or_that_none_does() {
    let mut rt = bare(RolesScene::Dialog);
    let text = words(&rt);
    for title in ["Find the cause", "Review", "Fix", "Make it steady", "Push and open the PR", "Run it many times"] {
        assert!(text.contains(title), "{title}");
    }
    rt.update(|s, _| s.workflows_nav.library.as_mut().unwrap().workflows.iter_mut().for_each(|w| w.steps.clear()));
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert_eq!(words(&rt).matches("no steps yet").count(), 3);
}

#[test]
fn a_harness_segment_shows_only_with_more_than_one_workflow_harness_and_roles_are_kept_per_harness() {
    let mut rt = bare(RolesScene::Dialog);
    assert!(has(&rt, "workflows-roles-harness"), "OpenCode 2 and Claude Code both run workflows");
    click(&mut rt, "workflows-roles-harness-claude-code");
    assert_eq!(rt.state.roles_harness_type(), "claude-code");
    assert!(words(&rt).matches("Default model").count() >= 3, "Claude Code has no roles mapped yet");
    set_harnesses(
        &mut rt,
        vec![harness("opencode2", "OpenCode 2", true, true), harness("claude-code", "Claude Code", false, true)],
        "opencode2",
    );
    rt.update(|s, _| s.workflows_nav.roles.harness = None);
    rt.frame(FRAME);
    assert!(!has(&rt, "workflows-roles-harness"), "one harness, no segment");
}

#[test]
fn the_dialog_menu_opens_from_the_dialog_and_the_runbox_one_from_the_popover() {
    let mut rt = bare(RolesScene::Popover);
    click(&mut rt, "workflow-model-standard");
    assert_eq!(rt.state.workflows_nav.roles.menu.map(|m| m.place), Some(Place::RunBox));
    escape(&mut rt);
    assert!(rt.state.models_popover_open() && !menu_open(&rt), "Escape from the menu returns to the popover");
}
