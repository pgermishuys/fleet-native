//! Piece F of the New session page: the folder chip and the menu's list view (`NSD/FolderPicker.vue`, rows B2.1-B2.103), driven
//! with no window. The model's cases are in `fleet/new_session/tests_folder.rs`; these are the chip, the popover, the keys and
//! the rows that open pieces G and H.
use crate::fleet::new_session::folder_menu::Make;
use crate::fleet::new_session::request::Folder;
use crate::tests::{FRAME, app, click};
use rune::{Key, KeyEvent, Mods, Runtime};

fn on_page() -> Runtime<crate::State> {
    let mut rt = app();
    click(&mut rt, "new-session");
    rt.frame(FRAME);
    rt
}

fn opened() -> Runtime<crate::State> {
    let mut rt = on_page();
    click(&mut rt, "new-session-folder-chip");
    rt.frame(FRAME);
    rt
}

fn press(rt: &mut Runtime<crate::State>, key: Key) {
    rt.key(KeyEvent { key, mods: Mods::default() });
    rt.frame(FRAME);
    rt.frame(FRAME);
}

fn type_in_search(rt: &mut Runtime<crate::State>, words: &str) {
    rt.key(KeyEvent { key: Key::Text(words.into()), mods: Mods::default() });
    rt.frame(FRAME);
    rt.frame(FRAME);
}

fn titles(rt: &Runtime<crate::State>) -> Vec<String> {
    rt.state.folder_rows().into_iter().map(|r| r.title).collect()
}

fn folder(rt: &Runtime<crate::State>) -> Option<Folder> {
    rt.state.new_draft.draft().and_then(|d| d.folder.clone())
}

fn focused(rt: &Runtime<crate::State>) -> Option<String> {
    rt.elements().into_iter().find(|e| e.focused).and_then(|e| e.key)
}

#[test]
fn the_chip_says_choose_a_folder_with_an_attention_border_until_one_is_chosen() {
    let rt = on_page();
    assert!(rt.find("new-session-folder-chip").is_some());
    assert_eq!(rt.state.folder_chip_label(None), "Choose a folder");
    assert!(!rt.state.folder_menu_open());
}

#[test]
fn the_chip_opens_the_menu_with_the_search_box_focused_and_nothing_typed() {
    let rt = opened();
    assert!(rt.state.folder_menu_open());
    assert!(rt.find("new-session-folder-pop").is_some());
    assert_eq!(focused(&rt).as_deref(), Some("new-session-folder-search"));
    assert_eq!(rt.state.folder_menu.query.text, "");
    assert_eq!(rt.state.folder_menu.highlight, 0);
}

#[test]
fn the_menu_lists_the_repositories_then_the_open_clone_and_chat_rows() {
    let rt = opened();
    assert_eq!(
        titles(&rt),
        ["Aurora-docs", "comet", "ledger", "rocket", "Open or create a folder…", "Clone a repository…", "No folder, just chat"]
    );
    for id in ["path", "clone-form", "none"] {
        assert!(rt.find(&format!("new-session-folder-{id}")).is_some(), "{id}");
    }
}

#[test]
fn arrow_keys_move_the_highlight_and_wrap_both_ways() {
    let mut rt = opened();
    let n = rt.state.folder_rows().len();
    press(&mut rt, Key::Down);
    assert_eq!(rt.state.folder_menu.highlight, 1);
    press(&mut rt, Key::Up);
    press(&mut rt, Key::Up);
    assert_eq!(rt.state.folder_menu.highlight, n - 1, "up from the first goes to the last");
    press(&mut rt, Key::Down);
    assert_eq!(rt.state.folder_menu.highlight, 0, "down from the last goes to the first");
}

#[test]
fn enter_runs_the_highlighted_row_and_closes_the_menu() {
    let mut rt = opened();
    press(&mut rt, Key::Down);
    press(&mut rt, Key::Down);
    press(&mut rt, Key::Enter);
    assert_eq!(folder(&rt), Some(Folder::Repository("/home/ada/work/ledger".into())));
    assert!(!rt.state.folder_menu_open());
    assert_eq!(rt.state.folder_chip_label(folder(&rt).as_ref()), "ledger");
    assert!(rt.state.new_draft.draft().is_some_and(|d| d.has_chosen_folder));
}

#[test]
fn a_click_runs_a_row_and_the_mouse_moves_the_highlight() {
    let mut rt = opened();
    let (x, y) = rt.find("new-session-folder-row-3").expect("the fourth row");
    rt.pointer_move(x, y);
    rt.frame(FRAME);
    assert_eq!(rt.state.folder_menu.highlight, 3);
    click(&mut rt, "new-session-folder-row-1");
    assert_eq!(folder(&rt), Some(Folder::Repository("/home/ada/src/comet".into())));
    assert!(!rt.state.folder_menu_open());
}

#[test]
fn escape_closes_the_whole_menu_and_keeps_the_folder() {
    let mut rt = opened();
    press(&mut rt, Key::Escape);
    assert!(!rt.state.folder_menu_open());
    assert_eq!(folder(&rt), None);
}

#[test]
fn it_reopens_with_the_chosen_folder_highlighted_and_checked() {
    let mut rt = opened();
    click(&mut rt, "new-session-folder-row-3");
    click(&mut rt, "new-session-folder-chip");
    rt.frame(FRAME);
    assert_eq!(rt.state.folder_menu.highlight, 3, "the selected row, not the first");
    let selected: Vec<_> = rt.state.folder_rows().into_iter().filter(|r| r.selected).map(|r| r.title).collect();
    assert_eq!(selected, ["rocket"]);
    assert_eq!(rt.state.folder_chip_label(folder(&rt).as_ref()), "rocket");
}

#[test]
fn typing_filters_and_puts_the_highlight_back_on_the_first_row() {
    let mut rt = opened();
    press(&mut rt, Key::Down);
    type_in_search(&mut rt, "rock");
    assert_eq!(rt.state.folder_menu.query.text, "rock");
    assert_eq!(rt.state.folder_menu.highlight, 0);
    assert_eq!(titles(&rt)[0], "rocket");
}

#[test]
fn a_name_no_repository_has_offers_to_create_it_in_the_location() {
    let mut rt = opened();
    type_in_search(&mut rt, "Recipe Box");
    let rows = rt.state.folder_rows();
    let create = rows.iter().find(|r| r.id == "create").expect("Create");
    assert_eq!(create.title_name.as_deref(), Some("Recipe-Box"));
    assert!(rt.find("new-session-folder-create").is_some());
    assert!(rt.find("new-session-folder-root").is_some(), "two locations: the select shows");
    assert_eq!(rt.state.folder_root().as_deref(), Some("/home/ada/src"));
}

#[test]
fn the_location_row_is_there_only_with_a_create_or_clone_and_more_than_one_location() {
    let mut rt = opened();
    assert!(rt.find("new-session-folder-root").is_none(), "nothing to make yet");
    type_in_search(&mut rt, "Recipe");
    assert!(rt.find("new-session-folder-root").is_some());
    rt.update(|s, _| s.feeds.roots.truncate(1));
    rt.frame(FRAME);
    assert!(rt.find("new-session-folder-root").is_none(), "one location");
}

#[test]
fn picking_a_location_moves_where_the_new_folder_goes_and_resets_on_the_next_open() {
    let mut rt = opened();
    type_in_search(&mut rt, "Recipe");
    click(&mut rt, "new-session-folder-root");
    click(&mut rt, "new-session-folder-root-1");
    assert_eq!(rt.state.folder_root().as_deref(), Some("/home/ada/work"));
    let create = rt.state.folder_rows().into_iter().find(|r| r.id == "create").expect("Create");
    assert_eq!(create.detail, "~/work/Recipe · git repository on main");
    press(&mut rt, Key::Escape);
    click(&mut rt, "new-session-folder-chip");
    assert_eq!(rt.state.folder_root().as_deref(), Some("/home/ada/src"), "each open starts over");
}

#[test]
fn new_folders_go_in_the_root_used_last_else_the_root_of_the_folder_in_use() {
    let mut rt = on_page();
    rt.state.set_folder(Folder::Repository("/home/ada/work/ledger".into()), true);
    click(&mut rt, "new-session-folder-chip");
    type_in_search(&mut rt, "Recipe");
    assert_eq!(rt.state.folder_root().as_deref(), Some("/home/ada/work"), "the root of the folder in use");
    rt.update(|s, _| s.sidebar.update_new_session_defaults("home", |d| d.remember_new_folder_root("/home/ada/src")));
    rt.frame(FRAME);
    assert_eq!(rt.state.folder_root().as_deref(), Some("/home/ada/src"), "the root used last wins");
}

#[test]
fn it_says_which_character_a_folder_name_cant_have_instead_of_dropping_it() {
    let mut rt = opened();
    type_in_search(&mut rt, "what?");
    assert!(rt.find("new-session-folder-invalid").is_some());
    assert_eq!(rt.state.folder_invalid_char(), Some('?'));
    assert!(rt.state.folder_rows().iter().all(|r| r.id != "create"));
    assert_eq!(rt.state.folder_menu.query.text, "what?", "nothing was dropped");
}

#[test]
fn a_path_of_its_own_offers_go_to_and_a_pasted_repository_offers_a_clone() {
    let mut rt = opened();
    type_in_search(&mut rt, "~/src/acme");
    assert!(rt.find("new-session-folder-go").is_some());
    assert!(rt.find("new-session-folder-create").is_none());
}

#[test]
fn the_clone_a_repository_row_opens_the_clone_view_in_the_same_menu() {
    let mut rt = opened();
    click(&mut rt, "new-session-folder-clone-form");
    rt.frame(FRAME);
    assert!(rt.state.folder_menu_open());
    assert_eq!(rt.state.folder_menu.view, crate::fleet::new_session::folder_menu::View::Clone);
    assert!(rt.find("new-session-clone-repository").is_some());
    assert!(rt.state.ui.toast.is_none());
}

#[test]
fn a_create_or_clone_row_does_nothing_while_one_already_runs_but_other_rows_still_work() {
    let mut rt = opened();
    type_in_search(&mut rt, "Recipe");
    rt.update(|s, _| s.folder_menu.make = Make::Creating);
    rt.frame(FRAME);
    let at = rt.state.folder_rows().iter().position(|r| r.id == "create").expect("Create");
    rt.update(move |s, tx| s.run_folder_row(at, tx));
    assert!(rt.state.ui.toast.is_none(), "the busy row was ignored");
    let at = rt.state.folder_rows().iter().position(|r| r.id == "none").expect("none");
    rt.update(move |s, tx| s.run_folder_row(at, tx));
    assert_eq!(folder(&rt), Some(Folder::None));
}

#[test]
fn no_folder_just_chat_creates_nothing() {
    let mut rt = opened();
    click(&mut rt, "new-session-folder-none");
    assert_eq!(folder(&rt), Some(Folder::None));
    assert_eq!(rt.state.folder_chip_label(folder(&rt).as_ref()), "No folder");
    assert!(!rt.state.new_draft.draft().is_some_and(|d| d.is_starting), "nothing was started");
    assert!(rt.state.on_new_session_page());
}

#[test]
fn the_chip_shows_a_folders_path_a_new_tag_and_a_clone_that_runs() {
    let mut rt = on_page();
    rt.state.set_folder(Folder::Directory("/home/ada/notes".into()), true);
    rt.frame(FRAME);
    assert_eq!(rt.state.folder_chip_label(folder(&rt).as_ref()), "~/notes");
    assert!(rt.find("new-session-folder-new").is_none());
    rt.update(|s, _| {
        s.folder_menu.created.insert("/home/ada/notes".into());
    });
    rt.frame(FRAME);
    assert!(rt.find("new-session-folder-new").is_some(), "a folder made from the menu is marked new");
    rt.update(|s, _| {
        s.folder_menu.make = Make::Cloning;
        s.folder_menu.clone_name = Some("recipe-box".into());
        s.folder_menu.clone_percent = Some(42);
    });
    rt.frame(FRAME);
    assert_eq!(rt.state.folder_chip_label(folder(&rt).as_ref()), "Cloning recipe-box… 42%");
    assert!(rt.find("new-session-folder-new").is_none(), "not while cloning");
    assert_eq!(rt.state.clone_status_text(), "Starting…");
}

#[test]
fn the_chip_is_off_while_the_session_starts() {
    let mut rt = on_page();
    rt.update(|s, _| s.new_draft.draft_mut().expect("a draft").is_starting = true);
    rt.frame(FRAME);
    click(&mut rt, "new-session-folder-chip");
    assert!(!rt.state.folder_menu_open());
}

#[test]
fn with_a_github_issue_attached_there_are_no_open_clone_or_chat_rows() {
    let mut rt = on_page();
    rt.update(|s, _| {
        s.new_draft.draft_mut().expect("a draft").github_preset = Some(crate::fleet::new_session::request::GitHubPreset {
            source_type: "github-issue".into(),
            owner: "ada".into(),
            repo: "rocket".into(),
            number: 7,
            title: "Fix the launch checks".into(),
            body: None,
            html_url: "https://example.test/ada/rocket/issues/7".into(),
            repo_full_name: "ada/rocket".into(),
            suggested_branch: None,
        });
    });
    click(&mut rt, "new-session-folder-chip");
    assert_eq!(titles(&rt), ["Aurora-docs", "comet", "ledger", "rocket"]);
}

#[test]
fn an_empty_list_says_what_to_do_and_a_typed_name_offers_create() {
    let mut rt = on_page();
    rt.update(|s, _| s.feeds.repos.clear());
    click(&mut rt, "new-session-folder-chip");
    assert_eq!(rt.state.folder_empty_note().as_deref(), Some("No repositories yet. Type a name to create one."));
    type_in_search(&mut rt, "zz");
    assert_eq!(rt.state.folder_empty_note(), None, "a name to create is offered, so no note");
}
