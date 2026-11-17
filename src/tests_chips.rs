//! Piece J of the New session page: the harness, profile, agent and model chips (`HarnessPicker.vue`, `ProfilePicker.vue`, the
//! agent and model parts of `NewSessionComposer.vue`; rows A10, B6, B7), driven with no window. Fleet's cases are in
//! `NewSessionComposer.test.ts` (`chips`, `profile`, `agent and model`) and keep their names here. Sending is piece L's, so
//! "what the request holds" is `sent_choices`, which L builds the request from. The folders and agents are invented.
use crate::composer::choice::{AgentModelChoice, AgentOption, ModelOption, ModelReference};
use crate::fleet::new_session::feed_state::CatalogKey;
use crate::fleet::new_session::feeds::{Catalog, Profile};
use crate::fleet::new_session::harness::NO_PROFILE;
use crate::fleet::new_session::harness_choice::HarnessEntry;
use crate::fleet::new_session::request::{Folder, Workspace};
use crate::fleet::settings::HOME_MACHINE_KEY;
use crate::mock::feeds::ROCKET;
use crate::tests::{FRAME, app, click};
use rune::Runtime;

type Rt = Runtime<crate::State>;

const NOTES: &str = "/home/ada/notes";
const COMET: &str = "/home/ada/src/comet";

fn harness(kind: &str, profiles: bool) -> HarnessEntry {
    HarnessEntry { supports_profiles: profiles, is_default: kind == "opencode", ..HarnessEntry::ready(kind) }
}

fn set_harnesses(rt: &mut Rt, list: Vec<HarnessEntry>) {
    rt.update(|s, _| {
        s.feeds.harnesses.get_mut(HOME_MACHINE_KEY).expect("home's harnesses").harnesses.all = list;
        s.settle_choices();
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
}

fn set_profiles(rt: &mut Rt, harness: &str, list: Vec<Profile>) {
    rt.update(|s, _| {
        s.feeds.profiles.insert((HOME_MACHINE_KEY.into(), harness.into()), list);
        s.settle_choices();
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
}

fn profile(id: &str, name: &str, is_default: bool) -> Profile {
    Profile { id: id.into(), name: name.into(), content: format!("{{ \"model\": \"{id}/model\" }}"), is_default, ..Profile::default() }
}

/// The harness has told the page what it offers for `key`, as an answer a moment ago.
fn seed_catalog(rt: &mut Rt, key: CatalogKey, catalog: Catalog) {
    let now = crate::fleet::link::now_ms();
    let feed = rt.state.feeds.catalogs.entry(HOME_MACHINE_KEY.into()).or_default();
    if let crate::fleet::new_session::feed_state::Ask::Fetch(seq) = feed.select(Some(key), now) {
        feed.finish(seq, now, Ok(catalog));
    }
}

fn wanted(rt: &Rt) -> Option<CatalogKey> {
    rt.state.feeds.catalogs.get(HOME_MACHINE_KEY).and_then(|f| f.wanted.clone())
}

fn key(harness: &str, directory: &str, profile: Option<&str>) -> CatalogKey {
    CatalogKey { harness: harness.into(), directory: Some(directory.into()), profile: profile.map(str::to_string) }
}

fn open_page(rt: &mut Rt) {
    click(rt, "new-session");
}

fn choose(rt: &mut Rt, folder: Folder) {
    rt.update(|s, _| s.set_folder(folder, true));
    rt.frame(FRAME);
    rt.frame(FRAME);
}

/// A page on one plain harness, `opencode`, with no profiles: the box as it always looked.
fn plain() -> Rt {
    let mut rt = app();
    set_harnesses_before_page(&mut rt);
    open_page(&mut rt);
    rt
}

fn set_harnesses_before_page(rt: &mut Rt) {
    rt.state.feeds.harnesses.get_mut(HOME_MACHINE_KEY).expect("home's harnesses").harnesses.all = vec![harness("opencode", false)];
    rt.state.feeds.profiles.clear();
}

fn words(rt: &Rt, key: &str) -> String {
    let all = rt.elements();
    let chip = all.iter().find(|e| e.key.as_deref() == Some(key)).unwrap_or_else(|| panic!("no element keyed {key:?}"));
    all.iter().filter(|e| e.path.starts_with(&chip.path) && e.path != chip.path).filter_map(|e| e.text.clone()).collect::<String>()
}

fn exists(rt: &Rt, key: &str) -> bool {
    rt.find(key).is_some()
}

fn sent(rt: &Rt) -> crate::fleet::new_session::harness::SentChoices {
    rt.state.sent_choices()
}

fn draft(rt: &Rt) -> &crate::fleet::new_session::draft::NewSessionDraft {
    rt.state.new_draft.draft().expect("the page has a draft")
}

// ─── chips ──────────────────────────────────────────────────────────────────────

#[test]
fn chips_shows_the_harness_picker_only_when_more_than_one_harness_is_enabled() {
    let mut rt = plain();
    assert!(!exists(&rt, "new-session-harness"));

    set_harnesses(&mut rt, vec![harness("opencode", false), harness("pi", false)]);
    assert!(exists(&rt, "new-session-harness"));
    assert_eq!(words(&rt, "new-session-harness"), "opencode");
}

#[test]
fn chips_a_harness_that_is_turned_off_or_missing_is_not_offered() {
    let mut rt = plain();
    let mut off = harness("pi", false);
    off.user_enabled = false;
    let mut missing = harness("claude-code", false);
    missing.available = false;
    set_harnesses(&mut rt, vec![harness("opencode", false), off, missing]);
    assert!(!exists(&rt, "new-session-harness"));
}

#[test]
fn chips_the_harness_menu_lists_the_enabled_ones_and_picking_one_starts_the_session_on_it() {
    let mut rt = plain();
    set_harnesses(&mut rt, vec![harness("opencode", false), harness("pi", false)]);
    click(&mut rt, "new-session-harness");
    assert!(rt.state.harness_menu_open());
    assert!(exists(&rt, "new-session-harness-opencode") && exists(&rt, "new-session-harness-pi"));

    click(&mut rt, "new-session-harness-pi");
    assert!(!rt.state.harness_menu_open());
    assert_eq!(draft(&rt).harness_type, "pi");
    assert_eq!(words(&rt, "new-session-harness"), "pi");
    assert_eq!(sent(&rt).harness_type.as_deref(), Some("pi"));
}

#[test]
fn chips_a_stored_harness_that_is_no_longer_enabled_follows_the_default() {
    let mut rt = plain();
    set_harnesses(&mut rt, vec![harness("opencode", false), harness("pi", false)]);
    click(&mut rt, "new-session-harness");
    click(&mut rt, "new-session-harness-pi");
    set_harnesses(&mut rt, vec![harness("opencode", false)]);
    assert_eq!(draft(&rt).harness_type, "opencode");
}

// ─── profile ────────────────────────────────────────────────────────────────────

#[test]
fn chips_profile_has_no_chip_while_there_are_no_profiles_and_sends_none() {
    let mut rt = plain();
    set_harnesses(&mut rt, vec![harness("opencode", true)]);
    set_profiles(&mut rt, "opencode", vec![]);
    choose(&mut rt, Folder::Directory(NOTES.into()));

    assert!(!exists(&rt, "new-session-profile"));
    assert_eq!(sent(&rt).harness_profile_id, None);
}

#[test]
fn chips_profile_has_no_chip_for_a_harness_without_profiles() {
    let mut rt = plain();
    set_profiles(&mut rt, "opencode", vec![profile("work", "Work", true)]);

    assert!(!exists(&rt, "new-session-profile"));
    assert_eq!(sent(&rt).harness_profile_id, None);
}

fn with_profiles() -> Rt {
    let mut rt = plain();
    set_harnesses(&mut rt, vec![harness("opencode", true)]);
    set_profiles(&mut rt, "opencode", vec![profile("work", "Work", true), profile("local", "Local", false)]);
    choose(&mut rt, Folder::Directory(NOTES.into()));
    rt
}

#[test]
fn chips_profile_starts_on_the_default_profile_and_sends_it() {
    let rt = with_profiles();
    assert!(words(&rt, "new-session-profile").contains("Work"));
    assert_eq!(sent(&rt).harness_profile_id.as_deref(), Some("work"));
}

#[test]
fn chips_profile_sends_the_profile_picked_and_none_when_thats_the_pick() {
    let mut rt = with_profiles();
    click(&mut rt, "new-session-profile");
    click(&mut rt, "new-session-profile-local");
    assert_eq!(sent(&rt).harness_profile_id.as_deref(), Some("local"));

    click(&mut rt, "new-session-profile");
    click(&mut rt, "new-session-profile-none");
    assert!(words(&rt, "new-session-profile").contains("No profile"));
    assert_eq!(sent(&rt).harness_profile_id.as_deref(), Some(NO_PROFILE));
}

#[test]
fn chips_profile_asks_for_the_agents_and_models_on_the_sessions_profile() {
    let mut rt = with_profiles();
    assert_eq!(wanted(&rt), Some(key("opencode", NOTES, Some("work"))));

    click(&mut rt, "new-session-profile");
    click(&mut rt, "new-session-profile-none");
    assert_eq!(wanted(&rt), Some(key("opencode", NOTES, Some(NO_PROFILE))));
}

#[test]
fn chips_profile_a_deleted_pick_falls_back_to_the_default() {
    let mut rt = with_profiles();
    click(&mut rt, "new-session-profile");
    click(&mut rt, "new-session-profile-local");
    set_profiles(&mut rt, "opencode", vec![profile("work", "Work", true)]);
    assert_eq!(rt.state.selected_profile_id(), "work");
}

#[test]
fn chips_profile_the_menu_describes_each_profile() {
    let mut rt = with_profiles();
    click(&mut rt, "new-session-profile");
    assert_eq!(words(&rt, "new-session-profile-none"), "No profileYour opencode.json as it is");
    assert_eq!(words(&rt, "new-session-profile-work"), "Work defaultwork/model");
    assert_eq!(words(&rt, "new-session-profile-local"), "Localdefault".replace("default", "local/model"));
}

// ─── agent and model ────────────────────────────────────────────────────────────

fn copilot_catalog() -> Catalog {
    let agent = |name: &str, model: Option<(&str, &str)>| AgentOption {
        id: name.into(),
        name: name.into(),
        description: String::new(),
        model: model.map(|(p, m)| ModelReference::new(p, m)),
    };
    let model = |id: &str, name: &str| ModelOption::new("github-copilot", "GitHub Copilot", id, name);
    Catalog {
        supported: true,
        agents: vec![
            agent("loom", Some(("github-copilot", "claude-opus-4.7"))),
            agent("tapestry", Some(("github-copilot", "claude-sonnet-5"))),
            agent("build", None),
        ],
        models: vec![
            model("claude-opus-4.7", "Claude Opus 4.7"),
            model("claude-sonnet-5", "Claude Sonnet 5"),
            model("claude-haiku-4.5", "Claude Haiku 4.5"),
        ],
        default_agent: Some("loom".into()),
        default_model: None,
    }
}

/// A page on `folder` whose harness answers with the Copilot catalog for it and for `COMET`.
fn copilot_page() -> Rt {
    let mut rt = app();
    set_harnesses_before_page(&mut rt);
    open_page(&mut rt);
    for dir in [ROCKET, COMET] {
        seed_catalog(&mut rt, key("opencode", dir, None), copilot_catalog());
    }
    rt
}

fn remember(rt: &mut Rt, folder: &Folder, agent: &str, model: &str) {
    let choice = AgentModelChoice { agent: agent.into(), model: model.into() };
    rt.state.sidebar.update_new_session_defaults(HOME_MACHINE_KEY, |d| d.remember(folder, &Workspace::New, Some(("opencode", &choice))));
}

/// Opens the `name` menu and clicks the row whose label starts with `label`.
fn pick(rt: &mut Rt, name: &'static str, label: &str) {
    click(rt, name);
    let shown = rt.state.selector_shown(name);
    let row = shown
        .iter()
        .flat_map(|v| v.rows.iter())
        .find(|r| r.item.label.starts_with(label))
        .unwrap_or_else(|| panic!("no {name} option {label:?}"));
    let key = format!("{name}-pop-item{}", row.index);
    click(rt, &key);
}

fn rocket() -> Folder {
    Folder::Repository(ROCKET.into())
}

#[test]
fn agent_and_model_asks_the_harness_about_the_chosen_folder_and_says_what_default_is() {
    let mut rt = copilot_page();
    choose(&mut rt, rocket());

    assert_eq!(wanted(&rt), Some(key("opencode", ROCKET, None)));
    assert_eq!(words(&rt, "agent"), "Default (loom)");
    assert_eq!(words(&rt, "model"), "Default (Claude Opus 4.7)");
}

#[test]
fn agent_and_model_defaults_model_follows_the_picked_agent() {
    let mut rt = copilot_page();
    choose(&mut rt, rocket());

    pick(&mut rt, "agent", "tapestry");

    assert_eq!(words(&rt, "agent"), "tapestry");
    assert_eq!(words(&rt, "model"), "Default (Claude Sonnet 5)");
}

#[test]
fn agent_and_model_starts_the_session_with_the_picked_agent_and_model() {
    let mut rt = copilot_page();
    choose(&mut rt, rocket());

    pick(&mut rt, "agent", "tapestry");
    pick(&mut rt, "model", "Claude Haiku 4.5");

    let sent = sent(&rt);
    assert_eq!(sent.agent.as_deref(), Some("tapestry"));
    assert_eq!(sent.model, Some(("github-copilot".to_string(), "claude-haiku-4.5".to_string())));
}

#[test]
fn agent_and_model_names_no_agent_or_model_when_both_are_default() {
    let mut rt = copilot_page();
    choose(&mut rt, rocket());

    let sent = sent(&rt);
    assert_eq!((sent.agent, sent.model), (None, None));
}

#[test]
fn agent_and_model_remembers_the_choice_for_the_folder_not_for_others() {
    let mut rt = copilot_page();
    remember(&mut rt, &rocket(), "tapestry", &crate::composer::choice::create_model_selection_key("github-copilot", "claude-haiku-4.5"));
    choose(&mut rt, rocket());
    assert_eq!(words(&rt, "agent"), "tapestry");
    assert_eq!(words(&rt, "model"), "Claude Haiku 4.5");

    choose(&mut rt, Folder::Repository(COMET.into()));
    assert_eq!(words(&rt, "agent"), "Default (loom)");
    assert_eq!(words(&rt, "model"), "Default (Claude Opus 4.7)");
}

#[test]
fn agent_and_model_a_pick_of_the_persons_own_is_not_replaced_by_the_folders() {
    let mut rt = copilot_page();
    remember(&mut rt, &Folder::Repository(COMET.into()), "tapestry", "");
    choose(&mut rt, rocket());
    pick(&mut rt, "agent", "build");

    choose(&mut rt, Folder::Repository(COMET.into()));
    assert_eq!(words(&rt, "agent"), "build");
}

#[test]
fn agent_and_model_drops_a_remembered_agent_the_folder_no_longer_has() {
    let mut rt = copilot_page();
    remember(&mut rt, &rocket(), "retired", "");
    choose(&mut rt, rocket());

    assert_eq!(words(&rt, "agent"), "Default (loom)");
    assert_eq!(sent(&rt).agent, None);
}

#[test]
fn agent_and_model_offers_no_choice_and_sends_none_when_the_harness_cant_list_its_agents() {
    let mut rt = app();
    set_harnesses_before_page(&mut rt);
    open_page(&mut rt);
    seed_catalog(&mut rt, key("opencode", ROCKET, None), Catalog { supported: false, ..Catalog::default() });
    remember(&mut rt, &rocket(), "tapestry", "");
    choose(&mut rt, rocket());

    assert!(!exists(&rt, "agent") && !exists(&rt, "model"));
    assert_eq!(sent(&rt).agent, None);
}

#[test]
fn agent_and_model_asks_about_an_existing_worktree_as_it_is() {
    let mut rt = copilot_page();
    let worktree = crate::mock::feeds::worktrees()[1].path.clone();
    seed_catalog(&mut rt, key("opencode", &worktree, None), copilot_catalog());
    rt.state.sidebar.update_new_session_defaults(HOME_MACHINE_KEY, |d| d.remember(&rocket(), &Workspace::Existing(worktree.clone()), None));
    choose(&mut rt, rocket());

    assert_eq!(draft(&rt).workspace, Workspace::Existing(worktree.clone()));
    assert_eq!(wanted(&rt), Some(key("opencode", &worktree, None)));
}

#[test]
fn agent_and_model_nothing_is_asked_until_a_folder_is_chosen() {
    let mut rt = copilot_page();
    rt.update(|s, _| s.settle_choices());
    rt.frame(FRAME);
    assert_eq!(wanted(&rt), None);
    assert!(!exists(&rt, "agent"));
}

#[test]
fn agent_and_model_the_menu_lists_default_then_the_folders_agents() {
    let mut rt = copilot_page();
    choose(&mut rt, rocket());
    click(&mut rt, "agent");
    let labels: Vec<String> = rt.state.selector_shown("agent").iter().flat_map(|v| v.rows.iter().map(|r| r.item.label.clone())).collect();
    assert_eq!(labels, ["Default (loom)", "loom", "tapestry", "build"]);
}

// ─── no harness ─────────────────────────────────────────────────────────────────

#[test]
fn no_harness_says_why_and_offers_to_set_one_up() {
    let mut rt = plain();
    let mut off = harness("opencode", false);
    off.available = false;
    off.reason = Some("OpenCode isn't installed.".into());
    set_harnesses(&mut rt, vec![off]);

    assert_eq!(rt.state.no_harness_notice(), Some(("OpenCode isn't installed.".into(), "Set up a harness".into())));
    assert!(exists(&rt, "new-session-no-harness"));
    click(&mut rt, "new-session-no-harness-fix");
    // The fix opens the setup wizard at its harness step (piece N).
    assert!(rt.state.harness_setup.is_open);
    assert_eq!(rt.state.harness_setup.step, crate::fleet::harness_setup::Step::Harnesses);
}

#[test]
fn agent_and_model_a_session_started_with_a_model_lists_it_as_recent() {
    // A10.15: `modelPicker.recordRecent` after a create that sent a model.
    let mut r = crate::tests_new_send::rig();
    set_harnesses_before_page(&mut r.rt);
    seed_catalog(&mut r.rt, key("opencode", ROCKET, None), copilot_catalog());
    r.rt.frame(FRAME);
    r.rt.frame(FRAME);
    pick(&mut r.rt, "model", "Claude Haiku 4.5");
    r.text("Fix the login redirect");
    r.send_and_wait();
    let body = r.posted().remove(0);
    assert_eq!(body["model"]["modelID"], "claude-haiku-4.5", "the request carried the model: {body}");
    r.wait("the preference to be written", |s| {
        s.pickers.prefs.get(crate::composer::groups::PREFERENCE_KEY).is_some_and(|v| v.contains("claude-haiku-4.5"))
    });
    let fake = r.fake.clone();
    r.wait("the PUT to arrive", move |_| {
        fake.lock().unwrap().preferences.iter().any(|(k, v)| k == "modelPicker" && v.contains("claude-haiku-4.5"))
    });
}
