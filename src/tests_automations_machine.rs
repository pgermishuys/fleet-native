//! Component 19, piece M: an automation that runs on another machine. Fleet's six cases of `AutomationMachine.test.ts` under
//! their names, against two fake Fleets: home (atlas, with `/work/kettle`) and the machine `m-harbor` (harbor, with
//! `/srv/code/harbor-api`, which holds its own token). The automation is always home's: its list, create and runs go there.
//! What the composer offers (folders, harnesses, the catalog) is asked of the machine the runs go to, with that machine's
//! token; each test counts only those requests and checks that none went to the other machine.
use crate::fleet::auth::Token;
use crate::fleet::new_session::folder_menu::CHIP as FOLDER_CHIP;
use crate::fleet::new_session::machines::MACHINE_CHIP;
use crate::fleet::new_session::request::{Folder, Workspace};
use crate::fleet::settings::HOME_MACHINE_KEY;
use crate::tests::click;
use crate::tests_automations_agent_model::chip_text;
use crate::tests_automations_composer::{plan, submit_shown, type_over};
use crate::tests_automations_page::{Rig, rig, weekly};
use crate::tests_automations_store::Fake;
use serde_json::{Value, json};

const HARBOR: &str = "m-harbor";
const HARBOR_API: &str = "/srv/code/harbor-api";
const KETTLE: &str = "/work/kettle";
/// An invented token: harbor's requests must carry it, home's must not.
const HARBOR_TOKEN: &str = "fmt_invented_harbor";

/// What the composer asks a machine for: its folders, its harnesses and what one offers a folder.
const COMPOSER_READS: [&str; 3] = ["/api/repositories", "/api/harnesses", "/catalog"];

fn nightly(extra: Value) -> Value {
    let mut extra = extra;
    for (k, v) in json!({"workspaceId": HARBOR_API, "isolation": "worktree", "targetMachineId": HARBOR, "triggerType": "schedule",
        "triggerConfig": "0 2 * * *"})
    .as_object()
    .unwrap()
    {
        if extra.get(k).is_none() {
            extra[k] = v.clone();
        }
    }
    weekly("a-night", "Nightly check", extra)
}

/// The run that went to harbor; its session is one harbor lists.
fn on_harbor(state: &str) -> Value {
    json!({"id": "r1", "automationId": "a-night", "trigger": "manual", "startedAt": "2026-09-15T07:00:00Z", "state": state,
        "sessionId": "h-crane", "machineId": HARBOR, "machineName": "harbor"})
}

/// The page on home, `machines` saying whether harbor is listed. Each machine remembers its own last folder, as New session
/// left it, and harbor answers with its own token.
fn screen(machines: bool, home_list: Vec<Value>, harbor_list: Vec<Value>) -> Rig {
    let mut r = rig(home_list, harbor_list);
    *r.home.repositories.lock().unwrap() = vec![json!({"name": "kettle", "path": KETTLE, "parentRoot": "/work"})];
    *r.harbor.repositories.lock().unwrap() = vec![json!({"name": "harbor-api", "path": HARBOR_API, "parentRoot": "/srv/code"})];
    *r.harbor.token.lock().unwrap() = Some(HARBOR_TOKEN.into());
    r.rt.update(move |s, _| {
        if machines {
            let harbor = s.machines.list.iter_mut().find(|m| m.id == HARBOR).expect("harbor is listed");
            harbor.token = Token::new(HARBOR_TOKEN);
            harbor.hold_token();
            // Harbor answers here (the invented set has it down).
            if let Some(heard) = s.machines.others.get_mut(HARBOR) {
                heard.error = None;
            }
        } else {
            s.machines.list.clear();
            s.machines.others.clear();
        }
        for (machine, path) in [(HOME_MACHINE_KEY, KETTLE), (HARBOR, HARBOR_API)] {
            s.sidebar.update_new_session_defaults(machine, |d| d.remember(&Folder::Repository(path.into()), &Workspace::New, None));
        }
    });
    r.open();
    r.wait("the list", |s| !s.shown_automations().is_loading);
    r.frames();
    r
}

/// New automation and the sentence typed, once home's folder is in.
fn start_new(r: &mut Rig, words: &str) {
    click(&mut r.rt, "new-automation");
    r.frames();
    r.wait("home's folder", |s| s.automations_nav.draft.folder == Some(Folder::Repository(KETTLE.into())));
    type_over(r, words);
}

/// Opens the Runs on menu, checks its heading, and picks `key`'s row.
fn pick_machine(r: &mut Rig, key: &str) {
    click(&mut r.rt, MACHINE_CHIP);
    r.frames();
    // `.ns-pop__label` is drawn in capitals (`text-transform: uppercase`).
    assert!(r.words().lines().any(|l| l == "RUNS ON"), "the menu's heading: {}", r.words());
    click(&mut r.rt, &format!("{MACHINE_CHIP}-{key}"));
    r.frames();
}

/// The composer reads one fake heard, from `from` on: (route, carried harbor's token).
fn composer_reads(fake: &Fake, from: usize) -> Vec<(String, bool)> {
    let all = fake.requests.lock().unwrap();
    all.iter()
        .skip(from)
        .filter(|(_, route, _)| COMPOSER_READS.iter().any(|r| route.ends_with(r)))
        .map(|(_, route, tokened)| (route.clone(), *tokened))
        .collect()
}

fn heard(fake: &Fake) -> usize {
    fake.requests.lock().unwrap().len()
}

/// Sends the composer and returns the body home was sent.
fn create(r: &mut Rig) -> Value {
    click(&mut r.rt, "automation-submit");
    let heard = r.home.handle();
    r.wait("the create", move |_| heard("POST", "/api/automations") > 0);
    r.home.body_of("POST", "/api/automations")
}

// ─── AutomationMachine.test.ts: "An automation that runs on another machine" ────────────────────────────────────────

#[test]
fn has_no_machine_chip_while_this_is_the_only_machine() {
    let mut r = screen(false, vec![], vec![]);
    start_new(&mut r, "Every night at 2, check the build");

    assert!(!r.has(MACHINE_CHIP));
    assert!(!plan(&r).starts_with("On "), "{}", plan(&r));
}

#[test]
fn runs_on_the_machine_picked_its_folders_harnesses_and_models_and_the_machine_is_sent() {
    let mut r = screen(true, vec![], vec![]);
    start_new(&mut r, "Every night at 2, check the build");

    assert!(chip_text(&r, MACHINE_CHIP).contains("atlas"), "{}", chip_text(&r, MACHINE_CHIP));
    assert!(chip_text(&r, FOLDER_CHIP).contains("kettle"));
    // Home's own catalog for kettle is answered before the pick, so a slow runner can't hear it after the count starts.
    r.wait("home's catalog", |s| s.automation_offers_agent_and_model());
    r.frames();
    let (home_from, harbor_from) = (heard(&r.home), heard(&r.harbor));

    pick_machine(&mut r, HARBOR);
    r.wait("harbor's folder", |s| s.automations_nav.draft.folder == Some(Folder::Repository(HARBOR_API.into())));
    r.wait("harbor's catalog", |s| s.automation_offers_agent_and_model());
    r.frames();

    assert!(chip_text(&r, MACHINE_CHIP).contains("harbor"));
    assert!(chip_text(&r, FOLDER_CHIP).contains("harbor-api"));
    let asked = composer_reads(&r.harbor, harbor_from);
    for route in COMPOSER_READS {
        assert!(asked.iter().any(|(r, _)| r.ends_with(route)), "harbor was asked for {route}: {asked:?}");
    }
    assert!(asked.iter().all(|(_, tokened)| *tokened), "every read went to harbor with its own token");
    assert_eq!(composer_reads(&r.home, home_from), [], "nothing for harbor went home");
    assert!(plan(&r).starts_with("On harbor: "), "{}", plan(&r));
    // What was typed stays.
    assert_eq!(r.rt.state.automations_nav.draft.text, "Every night at 2, check the build");

    let body = create(&mut r);

    assert_eq!(body["workspaceId"], HARBOR_API);
    assert_eq!(body["isolation"], "worktree");
    assert_eq!(body["targetMachineId"], HARBOR);
    assert_eq!(r.harbor.calls("POST", "/api/automations"), 0, "the automation is home's; only its runs go to harbor");
}

#[test]
fn goes_back_to_this_machine_with_its_own_folder() {
    let mut r = screen(true, vec![], vec![]);
    start_new(&mut r, "Every night at 2, check the build");
    pick_machine(&mut r, HARBOR);
    r.wait("harbor's folder", |s| s.automations_nav.draft.folder == Some(Folder::Repository(HARBOR_API.into())));

    pick_machine(&mut r, HOME_MACHINE_KEY);
    r.wait("home's folder", |s| s.automations_nav.draft.folder == Some(Folder::Repository(KETTLE.into())));
    r.frames();

    assert!(chip_text(&r, FOLDER_CHIP).contains("kettle"));
    assert!(!plan(&r).starts_with("On "), "{}", plan(&r));
    let body = create(&mut r);
    assert_eq!(body["targetMachineId"], Value::Null);
}

#[test]
fn shows_the_machine_in_the_list_and_on_its_runs_and_opens_a_run_s_session_on_that_machine_in_place() {
    let here = weekly("a-here", "Weekly digest", json!({"workspaceId": KETTLE, "targetMachineId": null}));
    let mut r = screen(true, vec![nightly(json!({})), here], vec![]);
    *r.home.runs.lock().unwrap() = vec![on_harbor("done")];
    r.frames();

    assert_eq!(r.texts_of("automation-row-machine"), ["harbor"], "only the row that runs on harbor names it");

    click(&mut r.rt, "automation-row-a-night");
    r.wait("its runs", |s| s.automations_nav.runs.loaded);
    r.wait("harbor's folder read", |s| s.feeds.repos(HARBOR).scanned_at.is_some());
    r.frames();

    assert!(chip_text(&r, MACHINE_CHIP).contains("harbor"));
    assert!(chip_text(&r, FOLDER_CHIP).contains("harbor-api"));
    assert!(!submit_shown(&r), "nothing changed");
    assert_eq!(r.texts_of("automation-run-machine"), ["on harbor"]);
    assert_eq!(r.harbor.calls("GET", "/api/automations/a-night/runs"), 0, "the runs are home's");

    click(&mut r.rt, "automation-run-r1");

    assert_eq!(r.rt.state.rail, crate::Rail::Sessions);
    assert_eq!(r.rt.state.route, crate::fleet::store::Route::Session("h-crane".into()));
    assert_eq!(r.rt.state.machine_key_of("h-crane"), HARBOR, "the session is harbor's");
}

#[test]
fn says_when_a_run_s_machine_didn_t_answer_rather_than_done() {
    let mut r = screen(true, vec![nightly(json!({}))], vec![]);
    *r.home.runs.lock().unwrap() = vec![on_harbor("unanswered")];

    click(&mut r.rt, "automation-row-a-night");
    r.wait("its runs", |s| s.automations_nav.runs.loaded);
    r.frames();

    assert_eq!(r.texts_of("automation-run-state"), ["harbor didn't answer"]);
    assert!(!r.texts_of("automation-run-state").iter().any(|t| t == "Done"));
}

#[test]
fn shown_for_another_machine_than_the_live_one_no_runs_on_chip_the_machine_an_automation_runs_on_is_named() {
    // The view shows harbor: its list is harbor's, whose machines this machine's list doesn't know.
    let last = json!({"id": "r0", "automationId": "a-night", "trigger": "schedule", "startedAt": "2026-09-14T02:00:00Z",
        "state": "done", "machineId": "m-elsewhere", "machineName": "relay"});
    let mut r = screen(true, vec![], vec![nightly(json!({"targetMachineId": "m-elsewhere", "lastRun": last}))]);
    r.rt.update(|s, _| s.automations_nav.choice.pick(HARBOR));
    r.frames();
    r.wait("harbor's list", |s| s.shown_automations().list.iter().any(|a| a.id == "a-night"));
    r.frames();

    click(&mut r.rt, "automation-row-a-night");
    r.frames();

    assert!(!r.has(MACHINE_CHIP));
    assert_eq!(r.texts_of("automation-runs-on"), ["Runs on relay"]);
}

// ─── native ─────────────────────────────────────────────────────────────────────────────────────────────────────────

#[test]
fn a_view_of_another_machine_asks_that_machine_for_the_composer_s_folders_whatever_the_state_says() {
    let mut r = screen(true, vec![], vec![nightly(json!({"targetMachineId": "m-elsewhere"}))]);
    r.rt.update(|s, _| s.automations_nav.choice.pick(HARBOR));
    r.frames();
    r.wait("harbor's list", |s| s.shown_automations().list.iter().any(|a| a.id == "a-night"));
    let home_from = heard(&r.home);

    click(&mut r.rt, "automation-row-a-night");
    r.wait("harbor's folders", |s| s.feeds.repos(HARBOR).scanned_at.is_some());
    r.frames();

    assert_eq!(r.rt.state.composer_machine(true), HARBOR);
    assert!(chip_text(&r, FOLDER_CHIP).contains("harbor-api"));
    assert_eq!(r.texts_of("automation-runs-on"), ["Runs on another machine"], "no run has named it yet");
    assert!(plan(&r).starts_with("On harbor: "), "{}", plan(&r));
    assert_eq!(composer_reads(&r.home, home_from), []);
}

#[test]
fn another_machine_s_saved_plain_folder_becomes_a_repository_once_that_machine_lists_it() {
    // Saved on the folder as it is: read before harbor's repositories came, it is a plain folder.
    let mut r = screen(true, vec![nightly(json!({"isolation": "existing"}))], vec![]);
    click(&mut r.rt, "automation-row-a-night");
    r.wait("harbor's folders", |s| s.feeds.repos(HARBOR).scanned_at.is_some());
    r.frames();

    assert_eq!(r.rt.state.automations_nav.edit.folder, Some(Folder::Repository(HARBOR_API.into())));
    assert!(!submit_shown(&r), "reading the folder is no change");
}
