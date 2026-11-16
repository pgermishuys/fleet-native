//! Piece I of the New session page: the workspace and base chips and their menus (`NSD/WorkspacePicker.vue`, `NSD/BasePicker.vue`,
//! rows B3, B4, A12, A17), driven with no window. Fleet has no tests of its own for the two components: its cases are in
//! `NewSessionComposer.test.ts` (`chips`, `base chip`), and they keep their names here. The page reads from the mock, as
//! the scenes do.
use crate::fleet::new_session::feeds::Worktree;
use crate::fleet::new_session::plan::sentence;
use crate::fleet::new_session::request::{Folder, NewSessionState, Workspace, build_create_session_request};
use crate::fleet::new_session::workspace::{base_empty_note, base_options, order_worktrees, worktree_location};
use crate::fleet::settings::HOME_MACHINE_KEY;
use crate::mock::feeds::ROCKET;
use crate::tests::{FRAME, app, click};
use rune::{Key, KeyEvent, Mods, Runtime};

type Rt = Runtime<crate::State>;

fn page_with(folder: Folder) -> Rt {
    let mut rt = app();
    click(&mut rt, "new-session");
    rt.update(|s, _| s.set_folder(folder, true));
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt
}

fn rocket() -> Rt {
    page_with(Folder::Repository(ROCKET.into()))
}

fn remember_workspace(rt: &mut Rt, workspace: &str) {
    rt.state.sidebar.update_new_session_defaults(HOME_MACHINE_KEY, |d| {
        d.remember(&Folder::Repository(ROCKET.into()), &parse(workspace), None);
    });
}

fn parse(workspace: &str) -> Workspace {
    match workspace {
        "current" => Workspace::Current,
        "new" => Workspace::New,
        path => Workspace::Existing(path.into()),
    }
}

/// A chip as drawn: its label and the muted words around it, joined.
fn words(rt: &Rt, key: &str) -> String {
    let all = rt.elements();
    let chip = all.iter().find(|e| e.key.as_deref() == Some(key)).unwrap_or_else(|| panic!("no element keyed {key:?}"));
    all.iter().filter(|e| e.path.starts_with(&chip.path) && e.path != chip.path).filter_map(|e| e.text.clone()).collect::<String>()
}

fn plan(rt: &Rt) -> String {
    sentence(&rt.state.plan_parts())
}

fn draft(rt: &Rt) -> &crate::fleet::new_session::draft::NewSessionDraft {
    rt.state.new_draft.draft().expect("the page has a draft")
}

fn press(rt: &mut Rt, key: Key) {
    rt.key(KeyEvent { key, mods: Mods::default() });
    rt.frame(FRAME);
    rt.frame(FRAME);
}

fn type_text(rt: &mut Rt, text: &str) {
    rt.key(KeyEvent { key: Key::Text(text.into()), mods: Mods::default() });
    rt.frame(FRAME);
    rt.frame(FRAME);
}

fn open_base(rt: &mut Rt) {
    click(rt, "new-session-base-chip");
    rt.frame(FRAME);
}

fn open_workspace(rt: &mut Rt) {
    click(rt, "new-session-workspace-chip");
    rt.frame(FRAME);
}

fn row_of(rt: &Rt, name: &str) -> usize {
    rt.state.base_rows().iter().position(|b| b.name == name).unwrap_or_else(|| panic!("no branch {name:?}"))
}

/// Clicks a branch's row, scrolling the list to it first as a person would.
fn click_row(rt: &mut Rt, at: usize) {
    let key = format!("new-session-base-row-{at}");
    rt.update(|_, tx| tx.scroll_into_view(key.clone()));
    rt.frame(FRAME);
    rt.frame(FRAME);
    click(rt, &key);
}

fn pick_branch(rt: &mut Rt, name: &str) {
    open_base(rt);
    let at = row_of(rt, name);
    click_row(rt, at);
}

fn type_message(rt: &mut Rt, message: &str) {
    rt.state.new_draft.draft_mut().expect("draft").message = message.into();
    rt.frame(FRAME);
}

/// What Enter would send, from the draft (the box and sending are piece L's).
fn request(rt: &Rt) -> serde_json::Value {
    let d = draft(rt);
    let mut state = NewSessionState::new(d.folder.clone().expect("a folder"), d.workspace.clone());
    state.message = d.message.clone();
    state.base_branch = d.base_branch.clone();
    state.fetch_origin = d.fetch_origin;
    state.branch = Some(d.branch_name.clone()).filter(|b| !b.is_empty());
    let built = build_create_session_request(&state).expect("a request");
    built.options.source["input"].clone()
}

// ─── chips ──────────────────────────────────────────────────────────────────────

#[test]
fn chips_shows_the_workspace_chip_for_a_repository() {
    let rt = rocket();
    assert_eq!(rt.state.folder_chip_label(draft(&rt).folder.as_ref()), "rocket");
    assert_eq!(words(&rt, "new-session-workspace-chip"), "New worktree");
}

#[test]
fn chips_remembers_the_workspace_per_repository() {
    let mut rt = app();
    click(&mut rt, "new-session");
    remember_workspace(&mut rt, "current");
    rt.update(|s, _| s.set_folder(Folder::Repository(ROCKET.into()), true));
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert_eq!(words(&rt, "new-session-workspace-chip"), "Current checkout · fix-login");
    assert!(plan(&rt).contains("Works directly in ~/src/rocket on fix-login."), "{}", plan(&rt));
}

#[test]
fn chips_hides_the_workspace_chip_for_a_plain_folder_and_for_no_folder() {
    let mut rt = page_with(Folder::Directory("/home/ada/notes".into()));
    assert!(rt.find("new-session-workspace-chip").is_none());
    rt.update(|s, _| s.set_folder(Folder::None, true));
    rt.frame(FRAME);
    assert!(rt.find("new-session-workspace-chip").is_none());
    assert!(rt.find("new-session-base-chip").is_none());
}

#[test]
fn chips_previews_the_name_the_worktree_naming_templates_will_give_it() {
    let mut rt = rocket();
    type_message(&mut rt, "Fix the login redirect");
    assert!(
        plan(&rt).contains("New worktree ~/src/rocket-worktrees/fleet-fix-login-redirect on fleet/fix-login-redirect"),
        "{}",
        plan(&rt)
    );
    // A preview: the request carries no branch, and the server does the naming.
    assert!(request(&rt).get("branch").is_none());
}

#[test]
fn chips_follows_the_naming_templates_the_repository_ships() {
    let mut rt = rocket();
    rt.state.feeds.naming.answer.effective.branch = "ada/{slug}".into();
    rt.state.feeds.naming.answer.effective.folder = "{slug}".into();
    type_message(&mut rt, "Fix the login redirect");
    assert!(plan(&rt).contains("~/src/rocket-worktrees/fix-login-redirect on ada/fix-login-redirect"), "{}", plan(&rt));
}

// ─── base chip ──────────────────────────────────────────────────────────────────

#[test]
fn base_chip_says_where_a_new_worktree_starts() {
    let mut rt = rocket();
    assert_eq!(words(&rt, "new-session-base-chip"), "from origin/main");
    type_message(&mut rt, "Fix the login redirect");
    assert!(plan(&rt).contains("on fleet/fix-login-redirect, from origin/main."), "{}", plan(&rt));
    rt.frame(FRAME);
}

#[test]
fn base_chip_isnt_there_for_the_current_checkout() {
    let mut rt = app();
    click(&mut rt, "new-session");
    remember_workspace(&mut rt, "current");
    rt.update(|s, _| s.set_folder(Folder::Repository(ROCKET.into()), true));
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(rt.find("new-session-base-chip").is_none());
    assert!(rt.find("new-session-workspace-chip").is_some());
}

#[test]
fn base_chip_lists_the_default_first_marked_and_the_checked_out_branch() {
    let mut rt = rocket();
    open_base(&mut rt);
    let rows = rt.state.base_rows();
    assert_eq!(rows[0].name, "origin/main", "the default first");
    assert!(rows.iter().any(|b| b.name == "fix-login" && b.is_current), "the checked-out branch is in the list");
    // The first row is the default (and the base in use), tagged " · default" with its check.
    assert!(rt.find("new-session-base-row-0").is_some());
    assert_eq!(rt.state.effective_base().as_deref(), Some("origin/main"));
    assert_eq!(rt.state.pickers_ns.base_highlight, 0);
    let titles: Vec<String> = rt.elements().into_iter().filter(|e| e.role == "menuitem").filter_map(|e| e.label).collect();
    assert_eq!(&titles[..2], ["origin/main", "fix-login"].map(String::from));
}

#[test]
fn base_chip_starts_from_a_chosen_branch_unfetched_and_under_a_typed_name_when_asked() {
    let mut rt = rocket();
    pick_branch(&mut rt, "origin/release/2.0");
    assert_eq!(words(&rt, "new-session-base-chip"), "from origin/release/2.0");

    open_base(&mut rt);
    click(&mut rt, "new-session-base-fetch");
    click(&mut rt, "new-session-branch-name");
    type_text(&mut rt, "hotfix/login");
    assert_eq!(draft(&rt).branch_name, "hotfix/login");
    press(&mut rt, Key::Enter);
    assert!(!rt.state.base_menu_open(), "Enter in the name closes the menu");

    assert_eq!(words(&rt, "new-session-base-chip"), "from origin/release/2.0 · no fetch");
    type_message(&mut rt, "Fix the login redirect");
    assert!(plan(&rt).contains("on hotfix/login, from origin/release/2.0 as last fetched."), "{}", plan(&rt));
    let input = request(&rt);
    assert_eq!(input["branch"], "hotfix/login");
    assert_eq!(input["baseBranch"], "origin/release/2.0");
    assert_eq!(input["fetchOrigin"], false);
}

#[test]
fn base_chip_reopens_with_the_chosen_branch_highlighted_even_after_searching_for_it() {
    let mut rt = rocket();
    open_base(&mut rt);
    type_text(&mut rt, "origin/release");
    assert_eq!(rt.state.base_rows().len(), 1);
    click(&mut rt, "new-session-base-row-0");
    open_base(&mut rt);
    assert_eq!(rt.state.pickers_ns.base_query.text, "", "the search starts empty");
    let at = rt.state.pickers_ns.base_highlight;
    assert_eq!(rt.state.base_rows()[at].name, "origin/release/2.0");
}

#[test]
fn base_chip_sends_no_base_for_the_default_so_the_servers_rules_apply() {
    let mut rt = rocket();
    pick_branch(&mut rt, "main");
    assert_eq!(draft(&rt).base_branch.as_deref(), Some("main"));
    pick_branch(&mut rt, "origin/main");
    assert_eq!(draft(&rt).base_branch, None, "the default is no choice");
    type_message(&mut rt, "Fix the login redirect");
    let input = request(&rt);
    assert!(input.get("baseBranch").is_none());
    assert!(input.get("fetchOrigin").is_none());
}

#[test]
fn base_chip_cant_turn_fetching_off_for_a_local_branch() {
    let mut rt = rocket();
    pick_branch(&mut rt, "main");
    open_base(&mut rt);
    assert!(!rt.state.base_is_origin());
    click(&mut rt, "new-session-base-fetch");
    assert!(draft(&rt).fetch_origin, "the switch does nothing for a local branch");
    let hint = rt.elements().into_iter().filter_map(|e| e.text).any(|t| t == "Only for origin’s branches");
    assert!(hint, "the row says why");
}

#[test]
fn base_chip_forgets_the_base_when_the_folder_changes() {
    let mut rt = rocket();
    pick_branch(&mut rt, "origin/release/2.0");
    open_base(&mut rt);
    click(&mut rt, "new-session-base-fetch");
    press(&mut rt, Key::Escape);
    rt.update(|s, _| s.new_draft.draft_mut().unwrap().branch_name = "hotfix/x".into());
    rt.update(|s, _| s.set_folder(Folder::Repository("/home/ada/src/comet".into()), true));
    let d = draft(&rt);
    assert_eq!((d.base_branch.clone(), d.fetch_origin, d.branch_name.clone()), (None, true, String::new()));
    // Choosing the same folder again keeps what was picked.
    rt.update(|s, _| s.new_draft.draft_mut().unwrap().base_branch = Some("main".into()));
    rt.update(|s, _| s.set_folder(Folder::Repository("/home/ada/src/comet".into()), true));
    assert_eq!(draft(&rt).base_branch.as_deref(), Some("main"));
}

#[test]
fn base_chip_warns_when_the_current_checkout_isnt_on_the_default_branch() {
    let mut rt = app();
    click(&mut rt, "new-session");
    remember_workspace(&mut rt, "current");
    rt.update(|s, _| s.set_folder(Folder::Repository(ROCKET.into()), true));
    assert!(plan(&rt).contains("Works directly in ~/src/rocket on fix-login. That's not main."), "{}", plan(&rt));
    let warn: Vec<_> = rt.state.plan_parts().into_iter().filter(|p| p.warn).map(|p| p.text).collect();
    assert_eq!(warn, [". That's not main."]);
}

// ─── the base menu's keys and notes ─────────────────────────────────────────────

#[test]
fn the_base_search_filters_by_name_without_regard_to_case_and_keeps_the_default_first() {
    let mut rt = rocket();
    open_base(&mut rt);
    type_text(&mut rt, "MAIN");
    let names: Vec<String> = rt.state.base_rows().into_iter().map(|b| b.name).collect();
    assert_eq!(names, ["origin/main", "main"]);
    type_text(&mut rt, "zzz");
    assert!(rt.state.base_rows().is_empty());
    assert_eq!(base_empty_note(false, " MAINzzz "), "No branch matches “MAINzzz”.");
    assert_eq!(base_empty_note(true, ""), "Loading branches…");
    assert_eq!(base_empty_note(false, ""), "No branches found.");
}

#[test]
fn arrows_wrap_and_enter_picks_the_highlighted_branch() {
    let mut rt = rocket();
    open_base(&mut rt);
    let n = rt.state.base_rows().len();
    press(&mut rt, Key::Up);
    assert_eq!(rt.state.pickers_ns.base_highlight, n - 1, "up from the first goes to the last");
    press(&mut rt, Key::Down);
    assert_eq!(rt.state.pickers_ns.base_highlight, 0);
    press(&mut rt, Key::Down);
    press(&mut rt, Key::Down);
    let wanted = rt.state.base_rows()[2].name.clone();
    press(&mut rt, Key::Enter);
    assert!(!rt.state.base_menu_open());
    assert_eq!(draft(&rt).base_branch.as_deref(), Some(wanted.as_str()));
}

#[test]
fn typing_in_the_search_puts_the_highlight_back_on_the_first_row_and_escape_closes() {
    let mut rt = rocket();
    open_base(&mut rt);
    press(&mut rt, Key::Down);
    type_text(&mut rt, "o");
    assert_eq!(rt.state.pickers_ns.base_highlight, 0);
    press(&mut rt, Key::Escape);
    assert!(!rt.state.base_menu_open());
    assert_eq!(draft(&rt).base_branch, None, "Esc picks nothing");
}

#[test]
fn the_base_chip_is_unknown_while_the_branches_load() {
    let mut rt = rocket();
    // Another repository: nothing is in for it yet, so no base is known and the list says it is loading.
    rt.update(|s, _| s.set_folder(Folder::Repository("/home/ada/src/comet".into()), true));
    rt.frame(FRAME);
    assert_eq!(rt.state.effective_base(), None);
    assert_eq!(words(&rt, "new-session-base-chip"), "from the default branch");
    assert!(plan(&rt).contains("from the default branch"), "{}", plan(&rt));
    open_base(&mut rt);
    assert!(rt.elements().into_iter().filter_map(|e| e.text).any(|t| t == "Loading branches…"));
}

#[test]
fn a_chosen_base_shows_before_the_branches_are_known() {
    let mut rt = rocket();
    rt.update(|s, _| s.new_draft.draft_mut().unwrap().base_branch = Some("origin/release/2.0".into()));
    rt.update(|s, _| s.feeds.detail.cancel());
    assert_eq!(rt.state.effective_base().as_deref(), Some("origin/release/2.0"));
    assert_eq!(rt.state.new_worktree_base().as_deref(), Some("origin/release/2.0"));
}

#[test]
fn the_new_branch_field_shows_the_name_the_message_gives_and_the_chip_menu_disables_while_starting() {
    let mut rt = rocket();
    type_message(&mut rt, "Fix the login redirect");
    assert_eq!(rt.state.generated_branch().as_deref(), Some("fleet/fix-login-redirect"));
    rt.update(|s, _| s.new_draft.draft_mut().unwrap().is_starting = true);
    rt.frame(FRAME);
    let chip = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("new-session-base-chip")).unwrap();
    assert!(chip.opacity < 1.0, "a disabled chip dims");
}

// ─── workspace menu ─────────────────────────────────────────────────────────────

#[test]
fn the_workspace_menu_lists_the_two_kinds_then_the_existing_worktrees_with_the_last_used_first() {
    let mut rt = app();
    click(&mut rt, "new-session");
    remember_workspace(&mut rt, "/home/ada/src/rocket-worktrees/spike-cache");
    rt.update(|s, _| s.set_folder(Folder::Repository(ROCKET.into()), true));
    rt.frame(FRAME);
    assert_eq!(words(&rt, "new-session-workspace-chip"), "spike-cache");
    open_workspace(&mut rt);
    for key in ["new-session-workspace-current", "new-session-workspace-new", "new-session-workspace-worktree-0"] {
        assert!(rt.find(key).is_some(), "{key}");
    }
    let listed: Vec<_> = rt.state.workspace_worktrees().into_iter().map(|w| w.path).collect();
    assert_eq!(listed[0], "/home/ada/src/rocket-worktrees/spike-cache", "the one used last first");
    assert_eq!(listed.len(), 3);
    assert!(rt.state.workspace_menu_open());
}

#[test]
fn picking_in_the_workspace_menu_sets_the_workspace_and_closes() {
    let mut rt = rocket();
    open_workspace(&mut rt);
    click(&mut rt, "new-session-workspace-current");
    assert_eq!(draft(&rt).workspace, Workspace::Current);
    assert!(!rt.state.workspace_menu_open());
    assert!(rt.find("new-session-base-chip").is_none(), "the base chip goes with the new worktree");
    open_workspace(&mut rt);
    click(&mut rt, "new-session-workspace-worktree-0");
    let path = rt.state.workspace_worktrees()[0].path.clone();
    assert_eq!(draft(&rt).workspace, Workspace::Existing(path));
    assert!(plan(&rt).starts_with("Continues in ~/elsewhere/rocket-detached."), "{}", plan(&rt));
    open_workspace(&mut rt);
    click(&mut rt, "new-session-workspace-new");
    assert_eq!(draft(&rt).workspace, Workspace::New);
    assert!(rt.find("new-session-base-chip").is_some());
}

#[test]
fn the_workspace_menu_opens_on_the_checked_row_and_escape_closes_it() {
    let mut rt = rocket();
    open_workspace(&mut rt);
    let focused = rt.elements().into_iter().find(|e| e.focused).and_then(|e| e.key);
    assert_eq!(focused.as_deref(), Some("new-session-workspace-new"));
    press(&mut rt, Key::Escape);
    assert!(!rt.state.workspace_menu_open());
    assert_eq!(draft(&rt).workspace, Workspace::New);
}

#[test]
fn a_remembered_worktree_that_is_gone_falls_back_to_a_new_one_when_the_list_arrives() {
    let mut rt = app();
    click(&mut rt, "new-session");
    remember_workspace(&mut rt, "/home/ada/src/rocket-worktrees/gone");
    rt.update(|s, _| s.set_folder(Folder::Repository(ROCKET.into()), true));
    assert_eq!(draft(&rt).workspace, Workspace::Existing("/home/ada/src/rocket-worktrees/gone".into()));
    rt.update(|s, _| s.drop_missing_worktree());
    assert_eq!(draft(&rt).workspace, Workspace::New);
    // One that is still listed stays.
    let kept = "/home/ada/src/rocket-worktrees/release-2-0";
    rt.update(|s, _| s.new_draft.draft_mut().unwrap().workspace = Workspace::Existing(kept.into()));
    rt.update(|s, _| s.drop_missing_worktree());
    assert_eq!(draft(&rt).workspace, Workspace::Existing(kept.into()));
}

#[test]
fn after_creating_sessions_remembers_the_workspace_per_repository() {
    let mut rt = app();
    click(&mut rt, "new-session");
    let other = Folder::Repository("/home/ada/src/comet".into());
    rt.state.sidebar.update_new_session_defaults(HOME_MACHINE_KEY, |d| {
        d.remember(&Folder::Repository(ROCKET.into()), &Workspace::Current, None);
        d.remember(&other, &Workspace::New, None);
    });
    rt.update(|s, _| s.set_folder(Folder::Repository(ROCKET.into()), true));
    assert_eq!(draft(&rt).workspace, Workspace::Current);
    rt.update(|s, _| s.set_folder(other.clone(), true));
    assert_eq!(draft(&rt).workspace, Workspace::New, "each repository has its own");
}

// ─── what the menus list ────────────────────────────────────────────────────────

fn tree(path: &str, branch: Option<&str>) -> Worktree {
    Worktree { path: path.into(), branch: branch.map(Into::into), commit_hash: "abc".into() }
}

#[test]
fn worktrees_come_as_the_server_lists_them_with_the_last_used_first() {
    let list = vec![tree("/a", Some("a")), tree("/b", Some("b")), tree("/c", None)];
    let order = |last| order_worktrees(&list, last).into_iter().map(|w| w.path).collect::<Vec<_>>();
    assert_eq!(order(None), ["/a", "/b", "/c"]);
    assert_eq!(order(Some("/c")), ["/c", "/a", "/b"]);
    assert_eq!(order(Some("/missing")), ["/a", "/b", "/c"]);
}

#[test]
fn a_worktree_beside_the_repository_is_written_with_an_ellipsis_and_any_other_with_a_tilde() {
    assert_eq!(worktree_location("/home/ada/src/rocket", "/home/ada/src/rocket-worktrees/x"), "…-worktrees/x");
    assert_eq!(worktree_location("/home/ada/src/rocket", "/home/ada/elsewhere/x"), "~/elsewhere/x");
}

#[test]
fn base_options_put_the_default_first_and_keep_the_servers_order_for_the_rest() {
    let branch = |name: &str| crate::fleet::new_session::feeds::Branch { name: name.into(), ..Default::default() };
    let list = vec![branch("b"), branch("origin/main"), branch("a")];
    let names = |q: &str| base_options(&list, Some("origin/main"), q).into_iter().map(|b| b.name).collect::<Vec<_>>();
    assert_eq!(names(""), ["origin/main", "b", "a"]);
    assert_eq!(names("  B "), ["b"]);
    assert_eq!(base_options(&list, None, "").len(), 3);
}
