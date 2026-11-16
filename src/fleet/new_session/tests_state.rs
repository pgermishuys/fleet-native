//! Fleet's tests for the New session page's remembered choices, the harness choice and the draft, under the same names
//! (`use-new-session-defaults.test.ts`, `use-enabled-harnesses.test.ts`, the new-session part of `workspace-ui.test.ts`;
//! rows A21.170-A21.204). Test names are the describe path, snake_cased. The folders are invented.
use super::defaults::*;
use super::draft::*;
use super::harness_choice::*;
use super::request::{Folder, GitHubPreset, Workspace};
use crate::fleet::settings::{HOME_MACHINE_KEY, Sidebar};
use serde_json::json;

fn repo(path: &str) -> Folder {
    Folder::Repository(path.into())
}

fn dir(path: &str) -> Folder {
    Folder::Directory(path.into())
}

const ROCKET: &str = "/src/rocket";
const COMET: &str = "/src/comet";
const LOGIN: &str = "/src/rocket-worktrees/fix-login";

fn repositories() -> Vec<String> {
    vec![ROCKET.into(), COMET.into()]
}

// ─── useNewSessionDefaults ──────────────────────────────────────────────────

#[test]
fn use_new_session_defaults_new_folders_remembers_the_root_the_last_new_folder_went_into_alongside_everything_else() {
    let mut d = NewSessionDefaults::default();
    assert_eq!(d.last_new_folder_root(), None);

    d.remember_new_folder_root("/work");
    d.remember(&repo(ROCKET), &Workspace::New, None);

    assert_eq!(d.last_new_folder_root(), Some("/work"));
    assert_eq!(d.initial_folder(&repositories()), Some(repo(ROCKET)));
}

#[test]
fn use_new_session_defaults_first_use_has_no_folder_and_no_recent_folders() {
    let d = NewSessionDefaults::default();
    assert_eq!(d.initial_folder(&repositories()), None);
    assert!(d.recent_folders(&repositories()).is_empty());
}

#[test]
fn use_new_session_defaults_first_use_starts_a_repository_on_a_new_worktree() {
    let d = NewSessionDefaults::default();
    assert_eq!(d.workspace_for(ROCKET, Some(&[])), Workspace::New);
    assert_eq!(d.last_worktree_for(ROCKET), None);
}

#[test]
fn use_new_session_defaults_after_creating_sessions_starts_with_the_last_folder() {
    let mut d = NewSessionDefaults::default();
    d.remember(&repo(ROCKET), &Workspace::New, None);
    assert_eq!(d.initial_folder(&repositories()), Some(repo(ROCKET)));
    d.remember(&dir("/tmp/notes"), &Workspace::New, None);
    assert_eq!(d.initial_folder(&repositories()), Some(dir("/tmp/notes")));
    d.remember(&Folder::None, &Workspace::New, None);
    assert_eq!(d.initial_folder(&repositories()), Some(Folder::None));
}

#[test]
fn use_new_session_defaults_after_creating_sessions_lists_recent_folders_newest_first_once_each_without_chats() {
    let mut d = NewSessionDefaults::default();
    d.remember(&repo(ROCKET), &Workspace::New, None);
    d.remember(&dir("/tmp/notes"), &Workspace::Current, None);
    d.remember(&Folder::None, &Workspace::New, None);
    d.remember(&repo(ROCKET), &Workspace::New, None);

    assert_eq!(d.recent_folders(&repositories()), vec![repo(ROCKET), dir("/tmp/notes")]);
}

#[test]
fn use_new_session_defaults_after_creating_sessions_keeps_at_most_five_recent_folders() {
    let mut d = NewSessionDefaults::default();
    for i in 0..7 {
        d.remember(&dir(&format!("/tmp/{i}")), &Workspace::Current, None);
    }
    let paths: Vec<_> = d.recent_folders(&repositories());
    assert_eq!(paths, vec![dir("/tmp/6"), dir("/tmp/5"), dir("/tmp/4"), dir("/tmp/3"), dir("/tmp/2")]);
}

#[test]
fn use_new_session_defaults_after_creating_sessions_remembers_the_workspace_per_repository() {
    let mut d = NewSessionDefaults::default();
    d.remember(&repo(ROCKET), &Workspace::Existing(LOGIN.into()), None);
    d.remember(&repo(COMET), &Workspace::Current, None);

    assert_eq!(d.workspace_for(ROCKET, Some(&[LOGIN.to_string()])), Workspace::Existing(LOGIN.into()));
    assert_eq!(d.last_worktree_for(ROCKET), Some(LOGIN.to_string()));
    assert_eq!(d.workspace_for(COMET, Some(&[])), Workspace::Current);
    assert_eq!(d.last_worktree_for(COMET), None);
}

/// Fleet's "shared by every page that uses it" is two components reading one store; here the one place the choices live
/// is the settings file, so a second reader is a reload of what the first wrote.
#[test]
fn use_new_session_defaults_after_creating_sessions_is_shared_by_every_page_that_uses_it() {
    let mut first = Sidebar::default();
    first.update_new_session_defaults(HOME_MACHINE_KEY, |d| d.remember(&repo(COMET), &Workspace::Current, None));

    let second = Sidebar::parse(&first.to_json());
    let d = second.new_session_defaults(HOME_MACHINE_KEY);
    assert_eq!(d.initial_folder(&repositories()), Some(repo(COMET)));
    assert_eq!(d.workspace_for(COMET, Some(&[])), Workspace::Current);
}

#[test]
fn use_new_session_defaults_stale_entries_falls_back_to_the_most_recent_repository_that_still_exists() {
    let mut d = NewSessionDefaults::default();
    d.remember(&repo(COMET), &Workspace::New, None);
    d.remember(&repo("/src/deleted"), &Workspace::New, None);

    assert_eq!(d.initial_folder(&repositories()), Some(repo(COMET)));
    assert_eq!(d.recent_folders(&repositories()), vec![repo(COMET)]);
}

#[test]
fn use_new_session_defaults_stale_entries_falls_back_to_nothing_when_no_remembered_repository_exists() {
    let mut d = NewSessionDefaults::default();
    d.remember(&repo("/src/deleted"), &Workspace::New, None);
    assert_eq!(d.initial_folder(&repositories()), None);
}

#[test]
fn use_new_session_defaults_stale_entries_falls_back_to_a_new_worktree_when_the_remembered_worktree_is_gone() {
    let mut d = NewSessionDefaults::default();
    d.remember(&repo(ROCKET), &Workspace::Existing(LOGIN.into()), None);
    assert_eq!(d.workspace_for(ROCKET, Some(&[])), Workspace::New);
}

#[test]
fn use_new_session_defaults_stale_entries_keeps_the_remembered_worktree_while_worktrees_are_loading() {
    let mut d = NewSessionDefaults::default();
    d.remember(&repo(ROCKET), &Workspace::Existing(LOGIN.into()), None);
    assert_eq!(d.workspace_for(ROCKET, None), Workspace::Existing(LOGIN.into()));
}

#[test]
fn use_new_session_defaults_stale_entries_ignores_storage_it_cant_read() {
    let mut d = NewSessionDefaults::parse(&json!({
        "lastFolder": { "kind": "repository" },
        "recentFolders": "nope",
        "workspaceByRepository": { ROCKET: 42 },
    }));
    assert_eq!(d.initial_folder(&repositories()), None);
    assert!(d.recent_folders(&repositories()).is_empty());
    assert_eq!(d.workspace_for(ROCKET, Some(&[])), Workspace::New);

    d.remember(&repo(ROCKET), &Workspace::Current, None);
    assert_eq!(d.workspace_for(ROCKET, Some(&[])), Workspace::Current);
}

#[test]
fn use_new_session_defaults_stale_entries_ignores_storage_that_isnt_json() {
    // The file is read by `Sidebar::parse`: text that isn't JSON is an empty store, so nothing is remembered.
    let s = Sidebar::parse("{not json");
    assert_eq!(s.new_session_defaults(HOME_MACHINE_KEY).initial_folder(&repositories()), None);
    let s = Sidebar::parse("[1, 2]");
    assert_eq!(s.new_session_defaults(HOME_MACHINE_KEY).initial_folder(&repositories()), None);
}

// Rows with no Fleet test of their own (A16.8, A16.9, A16.1, A16.11).

#[test]
fn the_choice_is_remembered_per_folder_and_harness_and_default_the_first_time() {
    let mut d = NewSessionDefaults::default();
    let pick = crate::composer::choice::AgentModelChoice { agent: "loom".into(), model: "[\"acme\",\"swift\"]".into() };
    assert_eq!(d.choice_for(&repo(ROCKET), "opencode"), Default::default());

    d.remember(&repo(ROCKET), &Workspace::New, Some(("opencode", &pick)));
    assert_eq!(d.choice_for(&repo(ROCKET), "opencode"), pick);
    assert_eq!(d.choice_for(&repo(ROCKET), "pi"), Default::default());
    assert_eq!(d.choice_for(&repo(COMET), "opencode"), Default::default());

    d.remember(&Folder::None, &Workspace::New, Some(("opencode", &pick)));
    assert_eq!(d.choice_for(&Folder::None, "opencode"), pick);
}

#[test]
fn plain_folders_are_kept_newest_first_once_each_to_thirty() {
    let mut d = NewSessionDefaults::default();
    for i in 0..32 {
        d.remember(&dir(&format!("/tmp/{i}")), &Workspace::Current, None);
    }
    d.remember(&dir("/tmp/20"), &Workspace::Current, None);
    d.remember(&repo(ROCKET), &Workspace::New, None);
    assert_eq!(d.plain_folders().len(), MAX_PLAIN_FOLDERS);
    assert_eq!(d.plain_folders()[0], "/tmp/20");
    assert_eq!(d.plain_folders()[1], "/tmp/31");
    assert!(!d.plain_folders().contains(&"/tmp/0".to_string()));
}

#[test]
fn each_machine_remembers_its_own_folders_under_fleets_keys() {
    assert_eq!(defaults_key("home"), "weave:new-session:defaults");
    assert_eq!(defaults_key("m-mac"), "weave:new-session:defaults:m-mac");

    let mut s = Sidebar::default();
    s.update_new_session_defaults("home", |d| d.remember(&repo(ROCKET), &Workspace::New, None));
    s.update_new_session_defaults("m-mac", |d| d.remember(&dir("/Users/robin/notes"), &Workspace::New, None));
    s.save_default_harness("m-mac", Some("claude-code"));

    let written: serde_json::Value = serde_json::from_str(&s.to_json()).unwrap();
    assert!(written["newSessionDefaults"]["weave:new-session:defaults"].is_object());
    assert!(written["newSessionDefaults"]["weave:new-session:defaults:m-mac"].is_object());

    let back = Sidebar::parse(&s.to_json());
    assert_eq!(back.new_session_defaults("home").initial_folder(&repositories()), Some(repo(ROCKET)));
    assert_eq!(back.new_session_defaults("m-mac").initial_folder(&[]), Some(dir("/Users/robin/notes")));
    assert_eq!(back.new_session_defaults("other").initial_folder(&[]), None);
    assert_eq!(back.saved_default_harness("m-mac"), Some("claude-code"));
    assert_eq!(back.saved_default_harness("home"), None);
}

#[test]
fn remembering_marks_the_file_to_be_written_and_reading_it_does_not() {
    let mut s = Sidebar::default();
    s.adopt(std::path::PathBuf::from("/nowhere"), Sidebar::default());
    assert!(s.take_save().is_none());
    s.update_new_session_defaults("home", |d| d.remember_new_folder_root("/work"));
    assert!(s.take_save().is_some());
    assert!(s.take_save().is_none());
}

// ─── useEnabledHarnesses ────────────────────────────────────────────────────

fn harnesses(all: Vec<HarnessEntry>) -> Harnesses {
    Harnesses { all, load: Load::Done, preferred: None }
}

#[test]
fn use_enabled_harnesses_returns_only_available_harnesses_enabled_by_the_user() {
    let h = harnesses(vec![
        HarnessEntry::ready("opencode"),
        HarnessEntry { user_enabled: false, ..HarnessEntry::ready("claude-code") },
        HarnessEntry { available: false, ..HarnessEntry::ready("codex") },
    ]);
    assert_eq!(h.enabled().iter().map(|x| x.kind.as_str()).collect::<Vec<_>>(), ["opencode"]);
}

#[test]
fn use_enabled_harnesses_takes_the_default_the_server_marks_when_no_preference_is_stored() {
    let h = harnesses(vec![HarnessEntry::ready("claude-code"), HarnessEntry { is_default: true, ..HarnessEntry::ready("opencode") }]);
    assert_eq!(h.default_type(), "opencode");
}

#[test]
fn use_enabled_harnesses_guesses_no_harness_of_its_own_with_none_marked_its_the_first_with_none_at_all_its_empty() {
    assert_eq!(harnesses(vec![HarnessEntry::ready("pi"), HarnessEntry::ready("opencode")]).default_type(), "pi");
    assert_eq!(harnesses(vec![]).default_type(), "");
}

#[test]
fn use_enabled_harnesses_reads_default_harness_type_from_the_preferences_store() {
    let h = Harnesses { preferred: Some("claude-code".into()), ..harnesses(vec![]) };
    assert_eq!(h.default_type(), "claude-code");
}

#[test]
fn use_enabled_harnesses_says_why_the_default_harness_cant_start_a_session_when_none_is_ready() {
    let reason = "OpenCode isn't installed: Fleet couldn't find opencode on PATH or in the folders its installer uses.";
    let h = harnesses(vec![
        HarnessEntry { available: false, reason: Some("Claude Code isn't signed in.".into()), ..HarnessEntry::ready("claude-code") },
        HarnessEntry { is_default: true, available: false, reason: Some(reason.into()), ..HarnessEntry::ready("opencode") },
    ]);
    assert_eq!(h.no_harness_reason().as_deref(), Some(reason));
}

#[test]
fn use_enabled_harnesses_says_every_harness_is_turned_off_when_the_user_turned_them_all_off() {
    let h = harnesses(vec![HarnessEntry { user_enabled: false, ..HarnessEntry::ready("opencode") }]);
    assert_eq!(h.no_harness_reason().as_deref(), Some("Every harness is turned off."));
}

#[test]
fn use_enabled_harnesses_has_no_reason_when_a_harness_is_ready_or_when_the_list_couldnt_load() {
    assert_eq!(harnesses(vec![HarnessEntry::ready("opencode")]).no_harness_reason(), None);
    let failed = Harnesses { load: Load::Failed, ..Default::default() };
    assert_eq!(failed.no_harness_reason(), None);
    // The first load says nothing yet either.
    assert_eq!(Harnesses::default().no_harness_reason(), None);
}

#[test]
fn a_harness_without_a_reason_is_not_ready_by_name_and_the_turned_on_default_is_asked_first() {
    let h = harnesses(vec![
        HarnessEntry { available: false, ..HarnessEntry::ready("pi") },
        HarnessEntry { available: false, display_name: "Codex".into(), ..HarnessEntry::ready("codex") },
    ]);
    // Neither is the default or preferred: the first turned-on one is asked.
    assert_eq!(h.no_harness_reason().as_deref(), Some("pi isn't ready."));
    let h = Harnesses { preferred: Some("codex".into()), ..h };
    assert_eq!(h.no_harness_reason().as_deref(), Some("Codex isn't ready."));
}

#[test]
fn the_harness_a_draft_uses_is_its_own_if_enabled_else_the_default_else_the_first_enabled() {
    let h = harnesses(vec![
        HarnessEntry { available: false, is_default: true, ..HarnessEntry::ready("opencode") },
        HarnessEntry::ready("claude-code"),
        HarnessEntry::ready("pi"),
    ]);
    assert_eq!(h.resolved_type("pi"), "pi");
    assert_eq!(h.resolved_type("opencode"), "claude-code");
    assert_eq!(h.resolved_type(""), "claude-code");
    assert!(h.shows_picker());
    let h = Harnesses { preferred: Some("pi".into()), ..h };
    assert_eq!(h.resolved_type("opencode"), "pi");
    assert_eq!(harnesses(vec![]).resolved_type("x"), "");
    assert!(!harnesses(vec![HarnessEntry::ready("pi")]).shows_picker());
}

#[test]
fn the_harness_list_is_read_as_the_server_sends_it() {
    let body = json!([
        { "type": "opencode", "displayName": "OpenCode", "available": true, "userEnabled": true, "isDefault": true,
          "capabilities": { "supportsProfiles": true } },
        { "type": "pi", "available": false, "userEnabled": true, "reason": "Pi isn't installed." },
        { "displayName": "no type" },
    ]);
    let list = parse_harness_entries(&body);
    assert_eq!(list.len(), 2);
    assert!(list[0].is_default && list[0].supports_profiles && list[0].display_name == "OpenCode");
    assert_eq!(
        (list[1].display_name.as_str(), list[1].reason.as_deref(), list[1].supports_profiles),
        ("pi", Some("Pi isn't installed."), false)
    );
}

// ─── useWorkspaceUiStore: the new session draft ─────────────────────────────

fn issue(title: &str) -> GitHubPreset {
    GitHubPreset {
        source_type: "github-issue".into(),
        owner: "acme".into(),
        repo: "weave".into(),
        number: 42,
        title: title.into(),
        body: Some("body".into()),
        html_url: "https://github.com/acme/weave/issues/42".into(),
        repo_full_name: "acme/weave".into(),
        suggested_branch: None,
    }
}

fn blank() -> NewSessionDraft {
    NewSessionDraft::blank("opencode")
}

#[test]
fn use_workspace_ui_store_holds_a_github_issue_for_the_new_session_page_until_the_page_takes_it() {
    let mut store = DraftStore::default();
    let preset = issue("Fix flaky create session flow");

    store.set_initial_source(Some(preset.clone()));
    assert_eq!(store.initial_source(), Some(&preset));

    store.set_initial_source(None);
    assert_eq!(store.initial_source(), None);
}

#[test]
fn use_workspace_ui_store_new_session_draft_has_a_sidebar_row_while_the_page_is_open_titled_from_the_message_as_its_typed() {
    let mut store = DraftStore::default();
    let restored = store.open(blank());
    assert!(!restored);
    let row = store.row().unwrap();
    assert_eq!((row.title.as_str(), row.project_id.as_deref(), row.is_starting), ("", None, false));

    let d = store.draft_mut().unwrap();
    d.message = "Fix the login redirect\nIt loops".into();
    d.project_id = Some("project-1".into());

    let row = store.row().unwrap();
    assert_eq!((row.title.as_str(), row.project_id.as_deref()), ("Fix the login redirect", Some("project-1")));
}

#[test]
fn use_workspace_ui_store_new_session_draft_prefers_a_typed_title_then_the_github_issues() {
    let mut store = DraftStore::default();
    store.open(blank());
    store.draft_mut().unwrap().github_preset = Some(issue("Login loops"));
    assert_eq!(store.row().unwrap().title, "Login loops");

    store.draft_mut().unwrap().title = "Login work".into();
    assert_eq!(store.row().unwrap().title, "Login work");
}

#[test]
fn use_workspace_ui_store_new_session_draft_keeps_a_draft_with_a_message_when_you_leave_and_gives_it_back() {
    let mut store = DraftStore::default();
    store.open(blank());
    let d = store.draft_mut().unwrap();
    d.message = "Half a thought".into();
    d.folder = Some(dir("/tmp/notes"));

    store.leave();

    assert!(!store.is_page_open());
    assert_eq!(store.row().unwrap().title, "Half a thought");

    assert!(store.open(blank()), "restored");
    let d = store.draft().unwrap();
    assert_eq!(d.message, "Half a thought");
    assert_eq!(d.folder, Some(dir("/tmp/notes")));
}

#[test]
fn use_workspace_ui_store_new_session_draft_drops_an_empty_draft_and_its_row_when_you_leave() {
    let mut store = DraftStore::default();
    store.open(blank());
    store.draft_mut().unwrap().message = "   ".into();

    store.leave();

    assert_eq!(store.row(), None);
    assert!(!store.open(blank()), "not restored");
}

#[test]
fn use_workspace_ui_store_new_session_draft_keeps_a_draft_that_is_starting_a_session() {
    let mut store = DraftStore::default();
    store.open(blank());
    store.draft_mut().unwrap().is_starting = true;

    store.leave();

    assert!(store.row().unwrap().is_starting);
}

#[test]
fn use_workspace_ui_store_new_session_draft_hands_its_row_over_to_the_session_it_became() {
    let mut store = DraftStore::default();
    store.open(blank());
    let row_key = store.row().unwrap().key;

    store.hand_off("session-1");

    assert_eq!(store.row(), None);
    assert_eq!(store.row_key_of("session-1"), Some(row_key.as_str()));
    assert!(!store.open(blank()));
    assert_eq!(store.draft().unwrap().message, "");
    assert_ne!(store.row().unwrap().key, row_key);
}

#[test]
fn use_workspace_ui_store_new_session_draft_machine_starts_a_draft_on_the_live_machine() {
    let mut store = DraftStore::default();
    store.open(blank());
    assert_eq!(store.machine(), None);
}

#[test]
fn use_workspace_ui_store_new_session_draft_machine_moves_the_draft_to_another_machine_its_message_stays_this_machines_folder_and_choices_go()
 {
    let mut store = DraftStore::default();
    store.open(blank());
    let d = store.draft_mut().unwrap();
    d.message = "Fix the sign-in loop".into();
    d.title = "Sign-in".into();
    d.folder = Some(repo("/home/robin/src/rocket"));
    d.has_chosen_folder = true;
    d.workspace = Workspace::Existing("/home/robin/src/rocket-worktrees/x".into());
    d.branch_name = "fix/sign-in".into();
    d.project_id = Some("project-1".into());
    d.agent = "loom".into();
    d.model = "anthropic/claude".into();
    d.has_chosen_agent_or_model = true;

    store.set_machine(Some("m-mac"), "home");
    // The page is rebuilt for the machine: it leaves, then opens the same draft.
    store.leave();
    let restored = store.open(blank());

    assert_eq!(store.machine(), Some("m-mac"));
    assert!(restored);
    let d = store.draft().unwrap();
    assert_eq!((d.message.as_str(), d.title.as_str()), ("Fix the sign-in loop", "Sign-in"));
    assert_eq!((d.folder.clone(), d.has_chosen_folder, d.workspace.clone()), (None, false, Workspace::New));
    assert_eq!((d.branch_name.as_str(), d.project_id.clone(), d.agent.as_str(), d.model.as_str()), ("", None, "", ""));
    assert!(!d.has_chosen_agent_or_model);
    assert_eq!(d.harness_type, "");
}

#[test]
fn use_workspace_ui_store_new_session_draft_machine_keeps_an_empty_draft_while_the_page_is_rebuilt_for_another_machine() {
    let mut store = DraftStore::default();
    store.open(blank());

    store.set_machine(Some("m-mac"), "home");
    store.leave();

    assert!(store.draft().is_some());
    assert_eq!(store.machine(), Some("m-mac"));
}

#[test]
fn use_workspace_ui_store_new_session_draft_machine_keeps_an_empty_draft_and_the_open_page_when_the_new_box_opens_before_the_old_one_leaves_as_vue_rebuilds_it()
 {
    let mut store = DraftStore::default();
    store.open(blank());

    store.set_machine(Some("m-mac"), "home");
    store.open(blank());
    store.leave();

    assert!(store.draft().is_some());
    assert_eq!(store.machine(), Some("m-mac"));
    assert!(store.is_page_open());

    // Leaving the page for good still drops the empty draft and its machine.
    store.leave();
    assert!(!store.is_page_open());
    assert!(store.draft().is_none());
    assert_eq!(store.machine(), None);
}

#[test]
fn use_workspace_ui_store_new_session_draft_machine_goes_back_to_the_live_machine_when_the_draft_is_dropped_or_becomes_a_session() {
    let mut store = DraftStore::default();
    store.open(blank());
    store.set_machine(Some("m-mac"), "home");
    store.leave();
    store.open(blank());

    store.leave();
    assert_eq!(store.machine(), None);

    store.open(blank());
    store.draft_mut().unwrap().message = "Ship it".into();
    store.set_machine(Some("m-mac"), "home");
    store.leave();
    store.open(blank());
    store.hand_off("s1");
    assert_eq!(store.machine(), None);
}

#[test]
fn use_workspace_ui_store_new_session_draft_machine_the_machine_youre_on_is_the_default_so_the_live_machine_is_a_pick() {
    let mut store = DraftStore::default();
    store.open(blank());

    store.set_machine(Some("m-mac"), "m-mac");
    assert_eq!(store.machine(), None);

    store.set_machine(Some("home"), "m-mac");
    assert_eq!(store.machine(), Some("home"));
}

#[test]
fn use_workspace_ui_store_new_session_draft_machine_picking_the_live_machine_is_the_same_as_not_picking_one() {
    let mut store = DraftStore::default();
    store.open(blank());

    store.set_machine(Some("home"), "home");

    assert_eq!(store.machine(), None);
}

// Rows with no Fleet test of their own (A3.2, A3.4's "another box open", A3.12's no-op).

#[test]
fn a_fresh_draft_starts_with_fleets_blank_fields_and_a_numbered_row_key() {
    let mut store = DraftStore::default();
    store.open(NewSessionDraft::blank("pi"));
    let d = store.draft().unwrap();
    assert_eq!(d.row_key, "new-session-draft-1");
    assert_eq!((d.harness_type.as_str(), d.fetch_origin, d.workspace.clone(), d.is_starting), ("pi", true, Workspace::New, false));
    assert_eq!((d.folder.clone(), d.base_branch.clone(), d.harness_profile_id.clone(), d.github_preset.clone()), (None, None, None, None));
    store.hand_off("a");
    store.open(blank());
    assert_eq!(store.draft().unwrap().row_key, "new-session-draft-2");
}

#[test]
fn a_draft_with_an_attached_github_item_is_kept_when_you_leave() {
    let mut store = DraftStore::default();
    store.open(blank());
    store.draft_mut().unwrap().github_preset = Some(issue("Login loops"));
    store.leave();
    assert!(store.draft().is_some());
}

#[test]
fn picking_the_same_machine_again_does_not_wipe_the_draft() {
    let mut store = DraftStore::default();
    store.open(blank());
    store.set_machine(Some("m-mac"), "home");
    store.draft_mut().unwrap().folder = Some(dir("/Users/robin/notes"));
    store.set_machine(Some("m-mac"), "home");
    assert_eq!(store.draft().unwrap().folder, Some(dir("/Users/robin/notes")));
}

// ─── the settings file ──────────────────────────────────────────────────────

/// A folder of its own under the system's temporary one; removed at the end.
fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("fleet-native-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn the_remembered_choices_are_written_to_the_settings_file_and_read_back_after_a_restart() {
    let dir = temp_dir("c07b-file");
    let mut s = Sidebar::default();
    s.update_new_session_defaults(HOME_MACHINE_KEY, |d| {
        d.remember(&repo(ROCKET), &Workspace::Existing(LOGIN.into()), Some(("opencode", &Default::default())));
        d.remember(&dir_folder("/src/notes"), &Workspace::Current, None);
        d.remember_new_folder_root("/src");
    });
    s.save_default_harness("m-mac", Some("claude-code"));
    let (_, json) = {
        s.adopt(dir.clone(), Sidebar::default());
        s.update_new_session_defaults(HOME_MACHINE_KEY, |_| {});
        s.take_save().unwrap()
    };
    crate::fleet::settings::write_file(&dir, &json).unwrap();
    if std::env::var_os("SHOW_SETTINGS_FILE").is_some() {
        println!("{}", std::fs::read_to_string(dir.join(crate::fleet::settings::FILE)).unwrap());
    }

    let back = crate::fleet::settings::load(&dir);
    let d = back.new_session_defaults(HOME_MACHINE_KEY);
    assert_eq!(d.initial_folder(&repositories()), Some(dir_folder("/src/notes")));
    assert_eq!(d.workspace_for(ROCKET, Some(&[LOGIN.to_string()])), Workspace::Existing(LOGIN.into()));
    assert_eq!(d.plain_folders(), ["/src/notes"]);
    assert_eq!(back.saved_default_harness("m-mac"), Some("claude-code"));
    let _ = std::fs::remove_dir_all(&dir);
}

fn dir_folder(path: &str) -> Folder {
    dir(path)
}

/// `GET /api/harnesses` of a scratch Fleet, saved to the file named by `HARNESSES_JSON`.
#[test]
#[ignore = "needs a scratch Fleet's answer in HARNESSES_JSON"]
fn live_harness_list_from_a_scratch_fleet() {
    let path = std::env::var("HARNESSES_JSON").expect("HARNESSES_JSON");
    let body: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let h = Harnesses { all: parse_harness_entries(&body), load: Load::Done, preferred: None };
    for e in &h.all {
        println!(
            "{} available={} enabled={} default={} profiles={}",
            e.kind, e.available, e.user_enabled, e.is_default, e.supports_profiles
        );
    }
    println!("enabled: {:?}", h.enabled().iter().map(|e| e.kind.as_str()).collect::<Vec<_>>());
    println!("default: {:?}  picker: {}  reason: {:?}", h.default_type(), h.shows_picker(), h.no_harness_reason());
}
