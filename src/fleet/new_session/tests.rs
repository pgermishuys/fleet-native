//! Fleet's tests for the create request, the plan line and the naming preview, under the same names
//! (`new-session-request.test.ts`, `new-session-plan.test.ts`, `worktree-naming.conformance.test.ts`; rows A21.110-A21.156,
//! A21.167-A21.169, A21.205). Test names are the describe path, snake_cased.
use super::naming::{Context, Templates, resolve_worktree_name};
use super::pattern::Pattern;
use super::plan::{PlanInput, PlanPart, describe_new_session, sentence, tilde_path, worktree_folder_label};
use super::request::*;
use serde_json::{Value, json};
use std::sync::Arc;

const REPO: &str = "/home/me/source/rocket";
const WORKTREE: &str = "/home/me/source/rocket-worktrees/fix-login";

fn state(message: &str) -> NewSessionState {
    let mut s = NewSessionState::new(Folder::Repository(REPO.into()), Workspace::New);
    s.message = message.into();
    s
}

fn build(s: &NewSessionState) -> NewSessionRequest {
    build_create_session_request(s).unwrap_or_else(|e| panic!("Expected a request but got: {e}"))
}

fn key(provider: &str, source_type: &str) -> Value {
    json!({ "providerId": provider, "sourceType": source_type, "actionId": "start-session", "contractVersion": 1 })
}

fn issue() -> GitHubPreset {
    GitHubPreset {
        source_type: "github-issue".into(),
        owner: "acme".into(),
        repo: "rocket".into(),
        number: 42,
        title: "Login redirect loops".into(),
        body: Some("Steps to reproduce…".into()),
        html_url: "https://github.com/acme/rocket/issues/42".into(),
        repo_full_name: "acme/rocket".into(),
        suggested_branch: Some("fix/issue-42".into()),
    }
}

fn image() -> Image {
    Image { mime: "image/png".into(), filename: Some("screenshot.png".into()), data: Arc::from("aGVsbG8=") }
}

fn input_of(request: &NewSessionRequest) -> Value {
    request.options.source["input"].clone()
}

// ─── slugForBranch ──────────────────────────────────────────────────────────

#[test]
fn slug_for_branch_drops_filler_words_and_joins_the_rest_with_hyphens() {
    assert_eq!(slug_for_branch("Can you fix the login redirect?"), "fix-login-redirect");
}

#[test]
fn slug_for_branch_turns_punctuation_into_word_breaks_and_keeps_contractions_whole() {
    assert_eq!(slug_for_branch("Don't cache /api/users (v2)!!"), "dont-cache-api-users-v2");
    assert_eq!(slug_for_branch("Don\u{2019}t stop"), "dont-stop");
}

#[test]
fn slug_for_branch_drops_emoji_and_accents() {
    assert_eq!(slug_for_branch("\u{1f680} D\u{e9}ploy the caf\u{e9} page \u{1f389}"), "deploy-cafe-page");
    // NFKD spells these out; letters that don't decompose are word breaks, as in Fleet.
    assert_eq!(slug_for_branch("\u{c5}ngstr\u{f6}m \u{ff26}\u{ff49}\u{ff58} \u{fb01}le"), "angstrom-fix-file");
    assert_eq!(slug_for_branch("Stra\u{df}e"), "stra-e");
    // A decomposed accent (e + combining acute) folds like the single letter.
    assert_eq!(slug_for_branch("cafe\u{301} menu"), "cafe-menu");
}

#[test]
fn slug_for_branch_cuts_long_messages_at_a_word_boundary_within_40_characters() {
    let slug = slug_for_branch("Refactor the authentication middleware so tokens refresh before they expire");
    assert_eq!(slug, "refactor-authentication-middleware");
    assert!(slug.len() <= 40);
}

#[test]
fn slug_for_branch_cuts_a_single_word_longer_than_the_limit() {
    assert_eq!(slug_for_branch(&"a".repeat(60)), "a".repeat(40));
}

#[test]
fn slug_for_branch_uses_only_the_first_non_empty_line() {
    assert_eq!(slug_for_branch("\n  \nAdd dark mode\nMore detail on the second line"), "add-dark-mode");
    assert_eq!(slug_for_branch("\r\nAdd dark mode\r\nMore"), "add-dark-mode");
}

#[test]
fn slug_for_branch_keeps_filler_words_when_nothing_else_is_left() {
    assert_eq!(slug_for_branch("Can you do this?"), "can-you-do-this");
}

#[test]
fn slug_for_branch_is_empty_when_nothing_usable_is_left() {
    assert_eq!(slug_for_branch(""), "");
    assert_eq!(slug_for_branch("   "), "");
    assert_eq!(slug_for_branch("\u{1f389}\u{1f389} !!"), "");
}

// ─── titleFromMessage ───────────────────────────────────────────────────────

#[test]
fn title_from_message_is_the_first_non_empty_line_spaces_tidied() {
    assert_eq!(title_from_message("\n  Fix the   login redirect \nMore detail"), "Fix the login redirect");
}

#[test]
fn title_from_message_cuts_a_long_line_at_a_word_boundary() {
    let title = title_from_message("Refactor the authentication middleware so that tokens refresh before they expire");
    assert_eq!(title, "Refactor the authentication middleware so that tokens\u{2026}");
    assert!(title.chars().count() <= 60);
}

#[test]
fn title_from_message_cuts_one_very_long_word() {
    assert_eq!(title_from_message(&"x".repeat(80)), format!("{}\u{2026}", "x".repeat(59)));
}

#[test]
fn title_from_message_is_empty_for_an_empty_message() {
    assert_eq!(title_from_message(" \n "), "");
}

// ─── buildCreateSessionRequest ──────────────────────────────────────────────

#[test]
fn build_create_session_request_repository_new_worktree_no_branch_so_the_naming_templates_name_it_on_the_server() {
    let request = build(&state("Fix the login redirect"));
    assert_eq!(request.directory.as_deref(), Some(REPO));
    assert_eq!(
        request.options.to_json(),
        json!({
            "source": { "key": key("builtin.repository", "repository"), "input": { "repositoryPath": REPO, "isolationStrategy": "worktree" } },
            "isolationStrategy": "worktree",
            "title": "Fix the login redirect",
            "initialPrompt": "Fix the login redirect",
        })
    );
}

#[test]
fn build_create_session_request_repository_sends_the_sessions_the_message_references_and_none_without_a_message() {
    let mut s = state("Read @billing-notes");
    s.session_references = vec![("@billing-notes".into(), "ses-1".into())];
    assert_eq!(build(&s).options.to_json()["sessionReferences"], json!([{ "token": "@billing-notes", "sessionId": "ses-1" }]));
    s.message = String::new();
    assert!(build(&s).options.to_json().get("sessionReferences").is_none());
}

#[test]
fn build_create_session_request_repository_new_worktree_without_a_message_still_no_branch() {
    let request = build(&state(""));
    assert_eq!(request.options.branch, None);
    assert_eq!(input_of(&request), json!({ "repositoryPath": REPO, "isolationStrategy": "worktree" }));
    assert_eq!(request.options.initial_prompt, None);
}

#[test]
fn build_create_session_request_repository_new_worktree_a_typed_branch_is_sent_and_wins_over_the_templates() {
    let mut s = state("Fix the login redirect");
    s.branch = Some(" feature/login ".into());
    let request = build(&s);
    assert_eq!(request.options.branch.as_deref(), Some("feature/login"));
    assert_eq!(input_of(&request)["branch"], "feature/login");
}

#[test]
fn build_create_session_request_repository_new_worktree_sends_a_chosen_base_and_fetch_only_when_its_off() {
    let mut s = state("Fix the login redirect");
    s.base_branch = Some("origin/release/2.0".into());
    s.fetch_origin = false;
    assert_eq!(
        input_of(&build(&s)),
        json!({ "repositoryPath": REPO, "isolationStrategy": "worktree", "baseBranch": "origin/release/2.0", "fetchOrigin": false })
    );
    let mut s = state("x");
    s.base_branch = None;
    s.fetch_origin = true;
    assert_eq!(input_of(&build(&s)), json!({ "repositoryPath": REPO, "isolationStrategy": "worktree" }));
}

#[test]
fn build_create_session_request_repository_sends_no_base_for_the_current_checkout_or_an_existing_worktree() {
    let mut s = state("");
    s.base_branch = Some("origin/release/2.0".into());
    s.fetch_origin = false;
    s.workspace = Workspace::Current;
    assert_eq!(input_of(&build(&s)), json!({ "repositoryPath": REPO, "isolationStrategy": "existing" }));
    s.workspace = Workspace::Existing(WORKTREE.into());
    assert_eq!(input_of(&build(&s)), json!({ "repositoryPath": REPO, "isolationStrategy": "worktree", "existingWorktreePath": WORKTREE }));
}

#[test]
fn build_create_session_request_repository_current_checkout_works_in_the_repository_no_branch() {
    let mut s = state("Fix the login redirect");
    s.workspace = Workspace::Current;
    let request = build(&s);
    assert_eq!(request.directory.as_deref(), Some(REPO));
    assert_eq!(request.options.isolation_strategy, "existing");
    assert_eq!(request.options.branch, None);
    assert_eq!(input_of(&request), json!({ "repositoryPath": REPO, "isolationStrategy": "existing" }));
}

#[test]
fn build_create_session_request_repository_existing_worktree_sends_its_path_and_no_branch() {
    let mut s = state("Carry on");
    s.workspace = Workspace::Existing(WORKTREE.into());
    let request = build(&s);
    assert_eq!(request.options.isolation_strategy, "worktree");
    assert_eq!(request.options.branch, None);
    assert_eq!(input_of(&request), json!({ "repositoryPath": REPO, "isolationStrategy": "worktree", "existingWorktreePath": WORKTREE }));
}

#[test]
fn build_create_session_request_directory_works_in_the_folder_whatever_the_workspace() {
    for workspace in [Workspace::Current, Workspace::New, Workspace::Existing(WORKTREE.into())] {
        let mut s = NewSessionState::new(Folder::Directory(" /tmp/scratch ".into()), workspace);
        s.message = "Hello there".into();
        let request = build(&s);
        assert_eq!(request.directory.as_deref(), Some("/tmp/scratch"));
        assert_eq!(request.options.isolation_strategy, "existing");
        assert_eq!(request.options.branch, None);
        assert_eq!(
            request.options.source,
            json!({ "key": key("builtin.local", "directory"), "input": { "directory": "/tmp/scratch", "isolationStrategy": "existing" } })
        );
    }
}

#[test]
fn build_create_session_request_directory_needs_a_path() {
    let s = NewSessionState::new(Folder::Directory("  ".into()), Workspace::New);
    assert_eq!(build_create_session_request(&s), Err("Pick a folder.".to_string()));
    let s = NewSessionState::new(Folder::Repository("".into()), Workspace::New);
    assert_eq!(build_create_session_request(&s), Err("Pick a folder.".to_string()));
}

#[test]
fn build_create_session_request_no_folder_sends_the_images_with_the_first_message() {
    let mut s = state("What is this?");
    s.attachments = vec![image()];
    assert_eq!(
        build(&s).options.to_json()["attachments"],
        json!([{ "mime": "image/png", "filename": "screenshot.png", "data": "aGVsbG8=" }])
    );
}

#[test]
fn build_create_session_request_no_folder_leaves_images_out_when_theres_no_message_or_none_were_added() {
    let mut s = state("");
    s.attachments = vec![image()];
    assert!(build(&s).options.to_json().get("attachments").is_none());
    let mut s = state("Hi");
    s.attachments = vec![];
    assert!(build(&s).options.to_json().get("attachments").is_none());
}

#[test]
fn build_create_session_request_no_folder_starts_a_quick_chat_whatever_the_workspace() {
    for workspace in [Workspace::Current, Workspace::New, Workspace::Existing(WORKTREE.into())] {
        let mut s = NewSessionState::new(Folder::None, workspace);
        s.message = "What is a monad?".into();
        let request = build(&s);
        assert_eq!(request.directory, None);
        assert_eq!(request.options.isolation_strategy, "existing");
        assert_eq!(request.options.branch, None);
        assert_eq!(request.options.initial_prompt.as_deref(), Some("What is a monad?"));
        assert_eq!(request.options.source, json!({ "key": key("builtin.quickchat", "quick-chat"), "input": {} }));
    }
}

fn with_issue(message: &str) -> NewSessionState {
    let mut s = state(message);
    s.github_preset = Some(issue());
    s
}

#[test]
fn build_create_session_request_github_preset_new_worktree_on_the_suggested_branch_titled_after_the_issue() {
    let request = build(&with_issue("Start with the tests"));
    assert_eq!(request.options.branch.as_deref(), Some("fix/issue-42"));
    assert_eq!(request.options.title.as_deref(), Some("Login redirect loops"));
    assert_eq!(request.options.initial_prompt.as_deref(), Some("Start with the tests"));
    assert_eq!(
        request.options.source,
        json!({
            "key": key("builtin.github", "github-issue"),
            "input": { "owner": "acme", "repo": "rocket", "number": 42, "repositoryPath": REPO, "isolationStrategy": "worktree", "branch": "fix/issue-42" },
        })
    );
}

#[test]
fn build_create_session_request_github_preset_without_a_suggested_branch_leaves_the_naming_to_the_server() {
    let mut s = with_issue("Start with the tests");
    s.github_preset.as_mut().unwrap().suggested_branch = None;
    assert_eq!(build(&s).options.branch, None);
}

#[test]
fn build_create_session_request_github_preset_existing_worktree_sends_its_path_instead_of_a_branch() {
    let mut s = with_issue("");
    s.workspace = Workspace::Existing(WORKTREE.into());
    let request = build(&s);
    assert_eq!(request.options.branch, None);
    assert_eq!(
        input_of(&request),
        json!({ "owner": "acme", "repo": "rocket", "number": 42, "repositoryPath": REPO, "isolationStrategy": "worktree", "existingWorktreePath": WORKTREE })
    );
}

#[test]
fn build_create_session_request_github_preset_new_worktree_from_a_chosen_base() {
    let mut s = with_issue("");
    s.base_branch = Some("origin/release/2.0".into());
    s.fetch_origin = false;
    let input = input_of(&build(&s));
    assert_eq!(input["branch"], "fix/issue-42");
    assert_eq!(input["baseBranch"], "origin/release/2.0");
    assert_eq!(input["fetchOrigin"], false);
    let mut s = with_issue("");
    s.workspace = Workspace::Existing(WORKTREE.into());
    s.base_branch = Some("main".into());
    assert!(input_of(&build(&s)).get("baseBranch").is_none());
}

#[test]
fn build_create_session_request_github_preset_current_checkout() {
    let mut s = with_issue("");
    s.workspace = Workspace::Current;
    let request = build(&s);
    assert_eq!(request.options.isolation_strategy, "existing");
    assert_eq!(input_of(&request)["isolationStrategy"], "existing");
    assert!(input_of(&request).get("branch").is_none());
}

#[test]
fn build_create_session_request_github_preset_a_typed_title_wins_over_the_issue_title() {
    let mut s = with_issue("");
    s.title = Some("My take".into());
    assert_eq!(build(&s).options.title.as_deref(), Some("My take"));
}

#[test]
fn build_create_session_request_github_preset_needs_a_repository() {
    for folder in [Folder::None, Folder::Directory("/tmp/scratch".into())] {
        let mut s = NewSessionState::new(folder, Workspace::New);
        s.github_preset = Some(issue());
        assert_eq!(build_create_session_request(&s), Err("Pick the repository for acme/rocket #42.".to_string()));
    }
}

#[test]
fn build_create_session_request_options_passes_title_project_harness_and_cleaned_tags() {
    let mut s = state("");
    s.title = Some("  Login work ".into());
    s.project_id = Some("project-1".into());
    s.harness_type = Some("opencode".into());
    s.tags = vec![" review ".into(), "".into(), "deploy".into()];
    let json = build(&s).options.to_json();
    assert_eq!(json["title"], "Login work");
    assert_eq!(json["projectId"], "project-1");
    assert_eq!(json["harnessType"], "opencode");
    assert_eq!(json["tags"], json!(["review", "deploy"]));
}

#[test]
fn build_create_session_request_options_leaves_out_what_wasnt_chosen() {
    let mut s = state("");
    s.title = Some("  ".into());
    s.project_id = None;
    s.tags = vec!["  ".into()];
    let json = build(&s).options.to_json();
    for key in ["title", "projectId", "harnessType", "tags", "harnessProfileId", "agent", "model", "initialPrompt"] {
        assert!(json.get(key).is_none(), "{key} is sent");
    }
}

#[test]
fn build_create_session_request_options_trims_the_message() {
    assert_eq!(build(&state("\n  Fix it  \n")).options.initial_prompt.as_deref(), Some("Fix it"));
}

#[test]
fn build_create_session_request_options_titles_the_session_after_the_message_when_no_title_is_typed() {
    assert_eq!(build(&state("Fix the login redirect\nIt loops on /login")).options.title.as_deref(), Some("Fix the login redirect"));
    let mut s = state("Fix it");
    s.title = Some("Login work".into());
    assert_eq!(build(&s).options.title.as_deref(), Some("Login work"));
    assert_eq!(build(&with_issue("Fix it")).options.title.as_deref(), Some("Login redirect loops"));
}

// ─── the body that is sent (A20.1, A20.9) ───────────────────────────────────

#[test]
fn the_body_sends_nulls_for_the_first_group_and_spreads_the_rest_only_when_set() {
    let body = build(&state("")).body();
    assert_eq!(
        body,
        json!({
            "directory": REPO, "title": null, "isolationStrategy": "worktree", "branch": null,
            "source": { "key": key("builtin.repository", "repository"), "input": { "repositoryPath": REPO, "isolationStrategy": "worktree" } },
            "harnessType": null, "harnessProfileId": null, "initialPrompt": null, "onComplete": null, "projectId": null, "tags": null,
        })
    );
    let mut s = NewSessionState::new(Folder::None, Workspace::New);
    s.message = "Hi".into();
    s.agent = Some("plan".into());
    s.model = Some(("anthropic".into(), "claude".into()));
    s.attachments = vec![image()];
    s.session_references = vec![("@a".into(), "ses-1".into())];
    s.harness_profile_id = Some("work".into());
    let body = build(&s).body();
    assert_eq!(body["directory"], Value::Null);
    assert_eq!(body["agent"], "plan");
    assert_eq!(body["model"], json!({ "providerID": "anthropic", "modelID": "claude" }));
    assert_eq!(body["attachments"], json!([{ "mime": "image/png", "filename": "screenshot.png", "data": "aGVsbG8=" }]));
    assert_eq!(body["sessionReferences"], json!([{ "token": "@a", "sessionId": "ses-1" }]));
    assert_eq!(body["harnessProfileId"], "work");
    // An image with no name goes as null, as `filename ?? null`.
    s.attachments = vec![Image { filename: None, ..image() }];
    assert_eq!(build(&s).body()["attachments"][0]["filename"], Value::Null);
}

// ─── buildCreatedSessionRow ─────────────────────────────────────────────────

fn response() -> Created {
    Created::from_wire(&json!({
        "instanceId": "instance-1",
        "workspaceId": "workspace-1",
        "session": { "id": "session-1", "title": "Fix the login redirect", "time": { "created": 5, "updated": 5 }, "tags": ["review"] },
        // The server names the worktree, so the branch comes back in the response.
        "branch": "fleet/fix-login-redirect",
        // The server picked the harness (the request named none), so the row has it from the response.
        "harnessType": "claude-code",
    }))
    .unwrap()
}

#[test]
fn build_created_session_row_is_a_running_working_session_in_the_given_project() {
    let created = response();
    let row = build_created_session_row(&created, &build(&state("Fix the login redirect")), Some(("scratch", "Scratch")));
    for (field, want) in [
        ("instanceId", json!("instance-1")),
        ("workspaceId", json!("workspace-1")),
        ("session", created.session.clone()),
        ("sessionStatus", json!("active")),
        ("activityStatus", json!("busy")),
        ("lifecycleStatus", json!("running")),
        ("retentionStatus", json!("active")),
        ("isolationStrategy", json!("worktree")),
        ("branch", json!("fleet/fix-login-redirect")),
        ("projectId", json!("scratch")),
        ("projectName", json!("Scratch")),
        ("harnessType", json!("claude-code")),
        ("tags", json!(["review"])),
    ] {
        assert_eq!(row[field], want, "{field}");
    }
    // The list's own parser reads it, so the row can be handed to the list as it is.
    let listed = crate::fleet::sessions::parse_sessions(&json!([row]), 0);
    assert_eq!(listed[0].id, "session-1");
}

#[test]
fn build_created_session_row_is_idle_when_it_started_without_a_message() {
    let row = build_created_session_row(&response(), &build(&state("")), None);
    assert_eq!(row["sessionStatus"], "idle");
    assert_eq!(row["activityStatus"], "idle");
    assert_eq!(row["projectId"], Value::Null);
}

#[test]
fn build_created_session_row_takes_the_branch_from_the_request_when_the_answer_has_none_and_a_github_start_is_working() {
    let mut s = with_issue("");
    s.branch = None;
    let mut created = response();
    created.branch = None;
    let row = build_created_session_row(&created, &build(&s), None);
    assert_eq!(row["branch"], "fix/issue-42");
    // No message, but a GitHub item is itself the prompt.
    assert_eq!(row["sessionStatus"], "active");
}

#[test]
fn a_chat_row_has_no_directory() {
    let mut s = NewSessionState::new(Folder::None, Workspace::New);
    s.message = "Hi".into();
    let row = build_created_session_row(&response(), &build(&s), None);
    assert_eq!((&row["workspaceDirectory"], &row["sourceDirectory"]), (&json!(""), &Value::Null));
}

#[test]
fn an_answer_without_a_session_id_is_no_session() {
    assert_eq!(Created::from_wire(&json!({ "instanceId": "i", "session": {} })), None);
    assert_eq!(Created::from_wire(&json!({ "session": { "id": "s" } })), None);
}

// ─── the plan line ──────────────────────────────────────────────────────────

fn plan(change: impl FnOnce(&mut PlanInput)) -> String {
    let mut input = PlanInput::new(Some(Folder::Repository("/home/me/src/rocket".into())), Workspace::New);
    change(&mut input);
    sentence(&describe_new_session(&input))
}

#[test]
fn tilde_path_cases() {
    for (path, want) in [
        ("/home/me/src/rocket", "~/src/rocket"),
        ("/Users/me/src/rocket", "~/src/rocket"),
        ("C:\\Users\\me\\src\\rocket", "~\\src\\rocket"),
        ("/home/me", "~"),
        ("/srv/repos/rocket", "/srv/repos/rocket"),
        ("/home/me-too", "~"),
    ] {
        assert_eq!(tilde_path(path), want, "{path}");
    }
}

#[test]
fn worktree_folder_label_names_the_folder_the_server_will_create() {
    assert_eq!(worktree_folder_label("/home/me/src/rocket", "fleet/fix-login"), "rocket-worktrees/fleet-fix-login");
}

#[test]
fn describe_new_session_asks_for_a_folder_when_none_is_chosen() {
    assert_eq!(plan(|i| i.folder = None), "Machine and folder");
}

#[test]
fn describe_new_session_chat_only() {
    assert_eq!(plan(|i| i.folder = Some(Folder::None)), "Chat only. No folder.");
}

#[test]
fn describe_new_session_a_folder_as_it_is() {
    assert_eq!(plan(|i| i.folder = Some(Folder::Directory("/home/me/notes".into()))), "Runs in ~/notes as it is.");
}

#[test]
fn describe_new_session_the_current_checkout_with_its_branch_when_known() {
    assert_eq!(
        plan(|i| {
            i.workspace = Workspace::Current;
            i.current_branch = Some("feature/x".into());
        }),
        "Works directly in ~/src/rocket on feature/x. Edits land in your checkout."
    );
    assert_eq!(plan(|i| i.workspace = Workspace::Current), "Works directly in ~/src/rocket. Edits land in your checkout.");
}

#[test]
fn describe_new_session_a_new_worktree_on_the_branch_it_will_get() {
    assert_eq!(
        plan(|i| i.new_branch = Some("fleet/fix-login".into())),
        "New worktree rocket-worktrees/fleet-fix-login on fleet/fix-login, from the default branch."
    );
    // The resolved path wins over the guessed label, and shows with the home folder as `~`.
    assert_eq!(
        plan(|i| {
            i.new_branch = Some("fleet/fix-login".into());
            i.worktree_path = Some("/home/me/src/rocket-worktrees/fleet-fix-login".into());
        }),
        "New worktree ~/src/rocket-worktrees/fleet-fix-login on fleet/fix-login, from the default branch."
    );
}

#[test]
fn describe_new_session_a_new_worktree_before_theres_a_message() {
    assert_eq!(
        plan(|i| i.new_branch = None),
        "New worktree from the default branch. The branch is named from your message, or the session id without one."
    );
}

#[test]
fn describe_new_session_names_the_base_a_new_worktree_starts_from_and_says_when_it_wont_be_fetched() {
    let named = |base: &str, fetch: bool| {
        plan(|i| {
            i.new_branch = Some("fleet/fix-login".into());
            i.base = Some(base.into());
            i.fetch_origin = fetch;
        })
    };
    assert_eq!(
        named("origin/release/2.0", true),
        "New worktree rocket-worktrees/fleet-fix-login on fleet/fix-login, from origin/release/2.0."
    );
    assert_eq!(
        named("origin/main", false),
        "New worktree rocket-worktrees/fleet-fix-login on fleet/fix-login, from origin/main as last fetched."
    );
    assert_eq!(named("main", false), "New worktree rocket-worktrees/fleet-fix-login on fleet/fix-login, from main.");
    assert_eq!(
        plan(|i| i.base = Some("origin/main".into())),
        "New worktree from origin/main. The branch is named from your message, or the session id without one."
    );
}

#[test]
fn describe_new_session_warns_when_the_current_checkout_isnt_on_the_default_branch() {
    let mut input = PlanInput::new(Some(Folder::Repository("/home/me/src/rocket".into())), Workspace::Current);
    input.current_branch = Some("feature/x".into());
    input.default_branch = Some("main".into());
    let parts = describe_new_session(&input);
    assert_eq!(sentence(&parts), "Works directly in ~/src/rocket on feature/x. That's not main.");
    assert_eq!(parts.iter().filter(|p| p.warn).map(|p| p.text.as_str()).collect::<Vec<_>>(), [". That's not main."]);
    assert_eq!(
        plan(|i| {
            i.workspace = Workspace::Current;
            i.current_branch = Some("main".into());
            i.default_branch = Some("main".into());
        }),
        "Works directly in ~/src/rocket on main. Edits land in your checkout."
    );
}

#[test]
fn describe_new_session_an_existing_worktree() {
    assert_eq!(
        plan(|i| {
            i.workspace = Workspace::Existing("/home/me/src/rocket-worktrees/fix-login".into());
            i.existing_branch = Some("fleet/fix-login".into());
        }),
        "Continues in ~/src/rocket-worktrees/fix-login on fleet/fix-login."
    );
}

#[test]
fn describe_new_session_marks_paths_and_branches_as_code() {
    let mut input = PlanInput::new(Some(Folder::Repository("/home/me/src/rocket".into())), Workspace::New);
    input.new_branch = Some("fleet/fix-login".into());
    let parts: Vec<PlanPart> = describe_new_session(&input);
    assert_eq!(
        parts.iter().filter(|p| p.code).map(|p| p.text.as_str()).collect::<Vec<_>>(),
        ["rocket-worktrees/fleet-fix-login", "fleet/fix-login"]
    );
}

// ─── worktree naming conformance ────────────────────────────────────────────

/// One of the cases in Fleet's `tests/contracts/worktree-naming-cases.json`, copied (they are invented data in Fleet's own
/// repository). `None` for a template means the default.
struct Case {
    name: &'static str,
    branch: Option<&'static str>,
    root: Option<&'static str>,
    folder: Option<&'static str>,
    prefix: Option<&'static str>,
    capture: &'static [(&'static str, &'static str)],
    message: &'static str,
    branch_override: Option<&'static str>,
    expect: (Option<&'static str>, &'static str, &'static str),
}

const CASES: &[Case] = &[
    Case {
        name: "the default template names worktrees the way Fleet always has",
        branch: None,
        root: None,
        folder: None,
        prefix: Some("fleet"),
        capture: &[],
        message: "Add rate limiting to the session create endpoint",
        branch_override: None,
        expect: (
            Some("fleet/add-rate-limiting-session-create"),
            "/home/you/source/weave-fleet-worktrees",
            "fleet-add-rate-limiting-session-create",
        ),
    },
    Case {
        name: "a prefix of your own",
        branch: Some("{prefix}/{slug}"),
        root: None,
        folder: None,
        prefix: None,
        capture: &[],
        message: "Add rate limiting to the session create endpoint",
        branch_override: None,
        expect: (
            Some("pg/add-rate-limiting-session-create"),
            "/home/you/source/weave-fleet-worktrees",
            "pg-add-rate-limiting-session-create",
        ),
    },
    Case {
        name: "a captured ticket is not repeated by the slug",
        branch: Some("feature/{ticket}-{slug}"),
        root: None,
        folder: Some("{ticket}-{slug}"),
        prefix: None,
        capture: &[("ticket", "[A-Z]{2,}-\\d+")],
        message: "PLAT-1841 add rate limiting to the session create endpoint",
        branch_override: None,
        expect: (
            Some("feature/PLAT-1841-add-rate-limiting-session-create"),
            "/home/you/source/weave-fleet-worktrees",
            "PLAT-1841-add-rate-limiting-session-create",
        ),
    },
    Case {
        name: "a capture that does not match collapses away with its separator",
        branch: Some("feature/{ticket}-{slug}"),
        root: None,
        folder: Some("{ticket}-{slug}"),
        prefix: None,
        capture: &[("ticket", "[A-Z]{2,}-\\d+")],
        message: "add rate limiting to the session create endpoint",
        branch_override: None,
        expect: (
            Some("feature/add-rate-limiting-session-create"),
            "/home/you/source/weave-fleet-worktrees",
            "add-rate-limiting-session-create",
        ),
    },
    Case {
        name: "a message with no slug in it slugs as the session id, inside the template",
        branch: Some("{prefix}/{slug}"),
        root: None,
        folder: None,
        prefix: None,
        capture: &[],
        message: "\u{1f680}\u{1f680}\u{1f680}",
        branch_override: None,
        expect: (Some("pg/session-a1b2c3d4"), "/home/you/source/weave-fleet-worktrees", "pg-session-a1b2c3d4"),
    },
    Case {
        name: "a session started without a message still gets the template's branch",
        branch: None,
        root: None,
        folder: None,
        prefix: Some("fleet"),
        capture: &[],
        message: "",
        branch_override: None,
        expect: (Some("fleet/session-a1b2c3d4"), "/home/you/source/weave-fleet-worktrees", "fleet-session-a1b2c3d4"),
    },
    Case {
        name: "no message keeps the rest of a template, a missing capture collapsing away",
        branch: Some("{user}/{ticket}-{slug}"),
        root: None,
        folder: Some("{ticket}-{slug}"),
        prefix: None,
        capture: &[("ticket", "[A-Z]{2,}-\\d+")],
        message: "",
        branch_override: None,
        expect: (Some("robin/session-a1b2c3d4"), "/home/you/source/weave-fleet-worktrees", "session-a1b2c3d4"),
    },
    Case {
        name: "a template that needs no slug still names an empty message",
        branch: Some("{prefix}/{date}-{shortid}"),
        root: None,
        folder: Some("{shortid}"),
        prefix: None,
        capture: &[],
        message: "",
        branch_override: None,
        expect: (Some("pg/2026-09-21-a1b2c3d4"), "/home/you/source/weave-fleet-worktrees", "a1b2c3d4"),
    },
    Case {
        name: "accents fold, punctuation and filler words go",
        branch: None,
        root: None,
        folder: None,
        prefix: Some("fleet"),
        capture: &[],
        message: "Fix the \u{dc}nicode caf\u{e9}?? crash!!",
        branch_override: None,
        expect: (Some("fleet/fix-unicode-cafe-crash"), "/home/you/source/weave-fleet-worktrees", "fleet-fix-unicode-cafe-crash"),
    },
    Case {
        name: "a long sentence is cut at a word boundary",
        branch: None,
        root: None,
        folder: None,
        prefix: Some("fleet"),
        capture: &[],
        message: "Refactor the session orchestrator so that pooled harness processes are reused",
        branch_override: None,
        expect: (
            Some("fleet/refactor-session-orchestrator-pooled"),
            "/home/you/source/weave-fleet-worktrees",
            "fleet-refactor-session-orchestrator-pooled",
        ),
    },
    Case {
        name: "only the first non-empty line names the branch",
        branch: None,
        root: None,
        folder: None,
        prefix: Some("fleet"),
        capture: &[],
        message: "\n   \nFix the login redirect\nand then look at the session list",
        branch_override: None,
        expect: (Some("fleet/fix-login-redirect"), "/home/you/source/weave-fleet-worktrees", "fleet-fix-login-redirect"),
    },
    Case {
        name: "a message of nothing but filler words still names something",
        branch: None,
        root: None,
        folder: None,
        prefix: Some("fleet"),
        capture: &[],
        message: "can you please do this for me",
        branch_override: None,
        expect: (
            Some("fleet/can-you-please-do-this-for-me"),
            "/home/you/source/weave-fleet-worktrees",
            "fleet-can-you-please-do-this-for-me",
        ),
    },
    Case {
        name: "one worktree root outside the repository's parent",
        branch: Some("{user}/{slug}"),
        root: Some("{home}/worktrees/{repo}"),
        folder: Some("{slug}"),
        prefix: None,
        capture: &[],
        message: "Add rate limiting",
        branch_override: None,
        expect: (Some("robin/add-rate-limiting"), "/home/you/worktrees/weave-fleet", "add-rate-limiting"),
    },
    Case {
        name: "worktrees inside the repository",
        branch: Some("{prefix}/{date}-{slug}"),
        root: Some("{repoParent}/{repo}/.worktrees"),
        folder: Some("{slug}"),
        prefix: None,
        capture: &[],
        message: "Add rate limiting",
        branch_override: None,
        expect: (Some("pg/2026-09-21-add-rate-limiting"), "/home/you/source/weave-fleet/.worktrees", "add-rate-limiting"),
    },
    Case {
        name: "a branch chosen by the caller wins, and the folder follows it",
        branch: Some("{prefix}/{slug}"),
        root: None,
        folder: None,
        prefix: None,
        capture: &[],
        message: "Add rate limiting",
        branch_override: Some("hotfix/urgent"),
        expect: (Some("hotfix/urgent"), "/home/you/source/weave-fleet-worktrees", "hotfix-urgent"),
    },
    Case {
        name: "the repository name is available as a token",
        branch: Some("{repo}/{slug}"),
        root: None,
        folder: Some("{repo}-{slug}"),
        prefix: None,
        capture: &[],
        message: "Add rate limiting",
        branch_override: None,
        expect: (Some("weave-fleet/add-rate-limiting"), "/home/you/source/weave-fleet-worktrees", "weave-fleet-add-rate-limiting"),
    },
];

fn context() -> Context {
    Context {
        repository_path: "/home/you/source/weave-fleet".into(),
        user: "robin".into(),
        date: "2026-09-21".into(),
        short_id: "a1b2c3d4".into(),
        home: "/home/you".into(),
    }
}

#[test]
fn worktree_naming_conformance_has_cases_to_run() {
    assert_eq!(CASES.len(), 16);
}

#[test]
fn worktree_naming_conformance_one_test_per_case() {
    for case in CASES {
        let defaults = Templates::default();
        let naming = Templates {
            branch: case.branch.map_or(defaults.branch, str::to_string),
            root: case.root.map_or(defaults.root, str::to_string),
            folder: case.folder.map_or(defaults.folder, str::to_string),
            capture: case.capture.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
            // The fixture's own prefix, unless the case sets one.
            prefix: case.prefix.unwrap_or("pg").to_string(),
        };
        let got = resolve_worktree_name(&naming, &context(), case.message, case.branch_override);
        assert_eq!(got.branch.as_deref(), case.expect.0, "{}: branch", case.name);
        assert_eq!(got.root, case.expect.1, "{}: root", case.name);
        assert_eq!(got.folder, case.expect.2, "{}: folder", case.name);
    }
}

#[test]
fn worktree_naming_preview_leaves_an_empty_messages_branch_to_the_server_when_theres_no_short_id_to_stand_in() {
    // The composer doesn't know the session id, so it can't promise "fleet/session-<id>".
    let context = Context { short_id: String::new(), ..context() };
    assert_eq!(resolve_worktree_name(&Templates::default(), &context, "", None).branch, None);
}

#[test]
fn worktree_naming_preview_the_plan_lines_example() {
    // A17.8: "Fix the login redirect" in ~/src/rocket.
    let context = Context { repository_path: "/home/me/src/rocket".into(), home: "/home/me".into(), ..Context::default() };
    let name = resolve_worktree_name(&Templates::default(), &context, "Fix the login redirect", None);
    assert_eq!((name.branch.as_deref(), name.folder.as_str()), (Some("fleet/fix-login-redirect"), "fleet-fix-login-redirect"));
    assert_eq!(name.root, "/home/me/src/rocket-worktrees");
}

// ─── the capture patterns ───────────────────────────────────────────────────

#[test]
fn a_capture_pattern_reads_like_the_javascript_one() {
    let capture = |pattern: &str, text: &str| Pattern::new(pattern).and_then(|p| p.capture(text));
    assert_eq!(capture("[A-Z]{2,}-\\d+", "fix PLAT-1841 now").as_deref(), Some("PLAT-1841"));
    assert_eq!(capture("[A-Z]{2,}-\\d+", "fix plat-1841 now"), None);
    // A group means "this part"; no group means the whole match.
    assert_eq!(capture("#(\\d+)", "see #77 please").as_deref(), Some("77"));
    assert_eq!(capture("\\bbug-\\w+", "a bug-login report").as_deref(), Some("bug-login"));
    assert_eq!(capture("(?:feat|fix)/(\\w+)", "fix/login").as_deref(), Some("login"));
    // A group that didn't take part gives the whole match.
    assert_eq!(capture("ab(c)?", "abd").as_deref(), Some("ab"));
    assert_eq!(capture("a.*?b", "xaabab").as_deref(), Some("aab"));
    assert_eq!(capture("^[a-z]+$", "abc").as_deref(), Some("abc"));
    assert_eq!(capture("^[a-z]+$", "abc d"), None);
    assert_eq!(capture("[^a-c]+", "abcxyz").as_deref(), Some("xyz"));
}

#[test]
fn a_capture_pattern_this_matcher_cannot_read_is_no_pattern_and_a_runaway_one_gives_up() {
    for pattern in ["(?=a)b", "\\p{L}", "(a", "a{2", "[z-a]", "*a", "\\1"] {
        assert!(Pattern::new(pattern).is_none(), "{pattern}");
    }
    let nasty = Pattern::new("(a+)+b").unwrap();
    assert_eq!(nasty.capture(&"a".repeat(40)), None);
}

// ─── a live Fleet (not run by `cargo test`) ─────────────────────────────────

/// `FLEET_NATIVE_LIVE=http://127.0.0.1:<port> cargo test live_create -- --ignored --nocapture`, against a scratch Fleet
/// (`docs/FLEET.md`) with a git repository at `FLEET_NATIVE_LIVE_REPO`: sends the request the page builds, prints the body
/// and the server's answer, reads the answer into a list row, and shows what a refusal says.
#[test]
#[ignore = "needs a scratch Fleet"]
fn live_create_session_against_a_scratch_fleet() {
    let (Ok(base), Ok(repo)) = (std::env::var("FLEET_NATIVE_LIVE"), std::env::var("FLEET_NATIVE_LIVE_REPO")) else {
        panic!("set FLEET_NATIVE_LIVE and FLEET_NATIVE_LIVE_REPO");
    };
    let mut s = NewSessionState::new(Folder::Repository(repo.clone()), Workspace::New);
    s.message = "Fix the login redirect".into();
    s.harness_type = Some("opencode".into());
    let request = build(&s);
    println!("BODY   {}", request.body());
    let answer = crate::fleet::http::create_session(&base, &request.body()).expect("the server accepts it");
    println!("ANSWER {answer}");
    let created = Created::from_wire(&answer).expect("an answer with a session");
    let row = build_created_session_row(&created, &request, None);
    println!("ROW    {row}");
    assert_eq!(row["sessionStatus"], "active");
    assert!(row["branch"].as_str().is_some_and(|b| b.starts_with("fleet/")));

    // A refusal says what the server said.
    let mut bad = NewSessionState::new(Folder::Repository(repo), Workspace::New);
    bad.branch = Some("bad..name".into());
    let why = crate::fleet::http::create_session(&base, &build(&bad).body()).expect_err("the server refuses a bad branch");
    println!("REFUSED {why}");
    assert!(!why.is_empty() && !why.starts_with("HTTP"));
}
