//! Component 19, piece L: the composer's agent, model and harness. Fleet's three L cases of `AutomationsPage.test.ts` under
//! their names, against a fake Fleet whose machine has OpenCode (the default) offering loom and tapestry on two GitHub
//! Copilot models; then native cases: no chips for a workflow, the menu lists no recent models, and with no server the
//! chips show the invented catalog.
use crate::fleet::automations::agent_model::{AGENT_CHIP, MODEL_CHIP};
use crate::fleet::automations::composer::TargetKind;
use crate::fleet::new_session::request::Folder;
use crate::tests::click;
use crate::tests_automations_composer::{KETTLE, open, page, start_new, submit_shown, weekly_digest};
use crate::tests_automations_page::Rig;
use serde_json::{Value, json};

/// Waits for the chips: the harnesses read, and the catalog of the composer's folder answered.
fn chips_shown(r: &mut Rig) {
    r.wait("the agent and model chips", |s| s.automation_offers_agent_and_model());
    r.frames();
}

/// Opens the chip's menu and clicks the row labelled `label` (`pick` of Fleet's test).
fn pick(r: &mut Rig, chip: &str, label: &str) {
    click(&mut r.rt, chip);
    r.frames();
    let name = if chip == AGENT_CHIP { "agent" } else { "model" };
    let index =
        r.rt.state
            .selector_shown(name)
            .iter()
            .flat_map(|section| section.rows.iter())
            .find(|row| row.item.label == label)
            .map(|row| row.index)
            .unwrap_or_else(|| panic!("No option \"{label}\""));
    click(&mut r.rt, &format!("{name}-opt{index}"));
    r.frames();
}

/// What a chip says, as drawn: the text inside it.
fn chip_text(r: &Rig, key: &str) -> String {
    let Some(chip) = r.rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)) else { return String::new() };
    let [x, y, w, h] = chip.rect;
    let inside = |e: &&rune::ElementInfo| {
        let [ex, ey, _, _] = e.rect;
        ex >= x && ex < x + w && ey >= y && ey < y + h
    };
    r.rt.elements().iter().filter(inside).filter_map(|e| e.text.clone()).collect::<Vec<_>>().concat()
}

fn send(r: &mut Rig, method: &'static str, path: &'static str) -> Value {
    click(&mut r.rt, "automation-submit");
    let heard = r.home.handle();
    r.wait("the request", move |_| heard(method, path) > 0);
    r.home.body_of(method, path)
}

fn picked(body: &Value) -> Value {
    json!({"agent": body["agent"], "model": body["model"], "harnessType": body["harnessType"]})
}

// ─── AutomationsPage.test.ts: "Automations screen" (the L cases) ────────────────────────────────────────────────────

#[test]
fn runs_a_new_automation_on_the_agent_and_model_picked_for_it_on_that_harness() {
    let mut r = page(vec![weekly_digest(json!({}))], true);
    start_new(&mut r, "Every Monday at 9am, summarise the open PRs");
    chips_shown(&mut r);

    assert_eq!(r.home.catalogs.lock().unwrap().last().cloned(), Some(("opencode".to_string(), Some(KETTLE.to_string()))));
    assert_eq!(chip_text(&r, AGENT_CHIP), "Default (loom)");
    assert_eq!(chip_text(&r, MODEL_CHIP), "Default (Claude Opus 4.7)");

    pick(&mut r, AGENT_CHIP, "tapestry");
    pick(&mut r, MODEL_CHIP, "Claude Haiku 4.5");
    let body = send(&mut r, "POST", "/api/automations");

    assert_eq!(picked(&body), json!({"agent": "tapestry", "model": "github-copilot/claude-haiku-4.5", "harnessType": "opencode"}));
}

#[test]
fn leaves_the_agent_model_and_harness_empty_when_both_are_default() {
    let mut r = page(vec![weekly_digest(json!({}))], true);
    start_new(&mut r, "Every Monday at 9am, summarise the open PRs");
    chips_shown(&mut r);
    let body = send(&mut r, "POST", "/api/automations");

    assert_eq!(picked(&body), json!({"agent": null, "model": null, "harnessType": null}));
}

#[test]
fn shows_an_automation_s_agent_and_model_and_offers_save_once_one_changes() {
    let saved = json!({"agent": "tapestry", "model": "github-copilot/claude-haiku-4.5", "harnessType": "opencode"});
    let mut r = page(vec![weekly_digest(saved)], true);
    open(&mut r, "a1");
    chips_shown(&mut r);

    assert_eq!(chip_text(&r, AGENT_CHIP), "tapestry");
    assert_eq!(chip_text(&r, MODEL_CHIP), "Claude Haiku 4.5");
    assert!(!submit_shown(&r), "nothing changed yet");

    pick(&mut r, MODEL_CHIP, "Default");
    let body = send(&mut r, "PUT", "/api/automations/a1");

    assert_eq!(picked(&body), json!({"agent": "tapestry", "model": null, "harnessType": "opencode"}));
}

// ─── native ─────────────────────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn the_catalog_is_asked_for_the_run_s_folder_once_and_not_again_for_the_same_folder() {
    let mut r = page(vec![weekly_digest(json!({}))], true);
    start_new(&mut r, "Every Monday at 9am, summarise the open PRs");
    chips_shown(&mut r);
    for _ in 0..5 {
        r.frames();
    }
    assert_eq!(r.home.catalogs.lock().unwrap().clone(), vec![("opencode".to_string(), Some(KETTLE.to_string()))]);
}

#[test]
fn a_workflow_has_no_agent_or_model_chips_and_drops_the_picks() {
    let mut r = page(vec![weekly_digest(json!({}))], true);
    start_new(&mut r, "Every Monday at 9am, summarise the open PRs");
    chips_shown(&mut r);
    pick(&mut r, AGENT_CHIP, "tapestry");

    r.rt.update(|s, _| s.automations_nav.draft.set_target_kind(TargetKind::Workflow));
    r.frames();

    assert!(!r.has(AGENT_CHIP) && !r.has(MODEL_CHIP), "a workflow's steps take their models from the roles");
    assert_eq!(r.rt.state.automations_nav.draft.agent, "");
}

#[test]
fn the_model_menu_lists_no_recent_models() {
    let mut r = page(vec![weekly_digest(json!({}))], true);
    // Haiku was used last in a session; the composer's menu still lists the provider's models only.
    r.rt.update(|s, _| {
        let picker = crate::composer::groups::ModelPicker::default().record_recent("opencode:github-copilot/claude-haiku-4.5");
        s.pickers.prefs.set(crate::composer::groups::PREFERENCE_KEY, &picker.to_json());
    });
    start_new(&mut r, "Every Monday at 9am, summarise the open PRs");
    chips_shown(&mut r);
    click(&mut r.rt, MODEL_CHIP);
    r.frames();

    let titles: Vec<String> = r.rt.state.selector_shown("model").iter().filter_map(|v| v.title.clone()).collect();
    assert!(!titles.iter().any(|t| t == "Recent"), "{titles:?}");
}

#[test]
fn the_default_harness_is_the_machine_s_and_the_chips_show_with_no_folder_too() {
    let mut r = page(vec![weekly_digest(json!({}))], false);
    start_new(&mut r, "Every Monday at 9am, summarise the open PRs");
    assert!(!r.has(AGENT_CHIP), "no folder yet: no chips");
    r.rt.update(|s, _| s.automations_nav.draft.set_folder(Folder::None, true, false));
    chips_shown(&mut r);

    assert_eq!(r.rt.state.automation_default_harness(), "opencode");
    assert_eq!(r.home.catalogs.lock().unwrap().last().cloned(), Some(("opencode".to_string(), None)));
}
