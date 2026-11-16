//! The folder menu's model: Fleet's `lib/__tests__/new-folder.test.ts` (the cases for the rows) and the list view's rows,
//! groups, notes and order (`NSD/FolderPicker.vue`, rows B2.60-B2.86). Names and cases are Fleet's, with invented people.
use super::feeds::Repository;
use super::folder::*;
use super::request::Folder;

fn roots() -> Vec<String> {
    vec!["/home/me/src".into(), "/home/me/work".into()]
}

#[test]
fn parse_clone_source_reads_each_as_the_github_repository() {
    for (typed, repository, name) in [
        ("ada/recipe-box", "ada/recipe-box", "recipe-box"),
        ("ada/recipe-box.git", "ada/recipe-box", "recipe-box"),
        ("github.com/ada/recipe-box", "ada/recipe-box", "recipe-box"),
        ("https://github.com/ada/recipe-box/", "ada/recipe-box", "recipe-box"),
        ("https://github.com/ada/recipe-box.git", "ada/recipe-box", "recipe-box"),
        ("https://github.com/ada/recipe-box/tree/main/src", "ada/recipe-box", "recipe-box"),
    ] {
        let want = CloneSource { repository: repository.into(), label: repository.into(), name: name.into() };
        assert_eq!(parse_clone_source(typed), Some(want), "{typed}");
    }
}

#[test]
fn parse_clone_source_keeps_the_address_as_typed() {
    for (typed, name) in [
        ("https://gitlab.com/group/sub/project.git", "project"),
        ("ssh://git@example.com/team/repo.git", "repo"),
        ("git@github.com:ada/recipe-box.git", "recipe-box"),
    ] {
        let want = CloneSource { repository: typed.into(), label: typed.into(), name: name.into() };
        assert_eq!(parse_clone_source(typed), Some(want), "{typed}");
    }
}

#[test]
fn parse_clone_source_reads_each_as_something_else() {
    for typed in ["recipe-box", "recipe box", "/home/me/src/weave", "~/src/weave", "file:///etc", ""] {
        assert_eq!(parse_clone_source(typed), None, "{typed:?}");
    }
}

fn typed(text: &str) -> TypedFolderPath {
    folder_path_from(text)
}

#[test]
fn folder_path_from_keeps_case_and_turns_spaces_into_dashes() {
    assert_eq!(typed("  Recipe Box "), TypedFolderPath { segments: vec!["Recipe-Box".into()], invalid: None });
    assert_eq!(typed("..hidden"), TypedFolderPath { segments: vec!["hidden".into()], invalid: None });
    assert_eq!(typed("   "), TypedFolderPath { segments: vec![], invalid: None });
}

#[test]
fn folder_path_from_reads_slashes_as_folders_inside_folders_on_any_machine() {
    assert_eq!(typed("clients\\acme portal").segments, ["clients", "acme-portal"]);
    assert_eq!(typed("clients/acme-portal/").segments, ["clients", "acme-portal"]);
    // Never climbs out of the location.
    assert_eq!(typed("../../etc").segments, ["etc"]);
}

#[test]
fn folder_path_from_names_a_character_no_folder_can_have_instead_of_dropping_it() {
    assert_eq!(typed("what: \"a\"").invalid, Some(':'));
    assert_eq!(typed("notes?").invalid, Some('?'));
}

#[test]
fn the_folder_box_tells_a_path_of_its_own_from_a_name_inside_the_location() {
    for rooted in ["~", "~/src", "/srv", "C:\\source", "c:/source", "\\\\server\\share"] {
        assert!(is_rooted_path(rooted), "{rooted}");
    }
    for inside in ["clients/acme", "acme", ".\\acme"] {
        assert!(!is_rooted_path(inside), "{inside}");
    }
}

#[test]
fn the_folder_box_writes_separators_the_way_the_machine_does() {
    assert_eq!(separator_of(Some("C:\\Users\\me\\source")), '\\');
    assert_eq!(separator_of(Some("/home/me/src")), '/');
    assert_eq!(separator_of(None), '/');
}

#[test]
fn the_folder_box_lists_the_folders_a_create_would_make_between_what_is_there_and_the_target() {
    assert_eq!(folders_between("/home/me/src", "/home/me/src/clients/acme"), ["clients", "acme"]);
    assert_eq!(folders_between("C:\\source\\", "C:\\source\\acme"), ["acme"]);
}

#[test]
fn join_path_uses_the_separator_the_parent_uses() {
    assert_eq!(join_path("/home/me/src", "recipe-box"), "/home/me/src/recipe-box");
    assert_eq!(join_path("/home/me/src/", "recipe-box"), "/home/me/src/recipe-box");
    assert_eq!(join_path("C:\\source", "recipe-box"), "C:\\source\\recipe-box");
}

#[test]
fn where_a_new_folder_goes_finds_the_root_a_path_is_in_and_not_one_that_only_shares_a_prefix() {
    let r = roots();
    assert_eq!(root_containing("/home/me/work/billing", &r), Some(&r[1]));
    assert_eq!(root_containing("/home/me/workshop/x", &r), None);
}

#[test]
fn where_a_new_folder_goes_prefers_the_root_used_last_then_the_root_of_the_folder_in_use_then_the_first() {
    let r = roots();
    assert_eq!(default_new_folder_root(&r, Some("/home/me/work"), Some("/home/me/src/rocket")), Some(&r[1]));
    assert_eq!(default_new_folder_root(&r, Some("/gone"), Some("/home/me/work/billing")), Some(&r[1]));
    assert_eq!(default_new_folder_root(&r, None, None), Some(&r[0]));
    assert_eq!(default_new_folder_root(&[], None, None), None);
}

// ─── the rows ───────────────────────────────────────────────────────────────────

fn repo(name: &str, root: &str) -> Repository {
    Repository { name: name.into(), path: format!("{root}/{name}"), parent_root: root.into() }
}

struct Case {
    repos: Vec<Repository>,
    recent: Vec<Folder>,
    plain: Vec<String>,
    current: Option<Folder>,
    query: String,
    browse: bool,
    create: bool,
    none: bool,
    root: Option<String>,
}

impl Case {
    fn new() -> Case {
        Case {
            repos: vec![repo("rocket", "/home/me/src"), repo("comet", "/home/me/src"), repo("Aurora-docs", "/home/me/src")],
            recent: vec![],
            plain: vec![],
            current: None,
            query: String::new(),
            browse: true,
            create: true,
            none: true,
            root: Some("/home/me/src".into()),
        }
    }
    fn rows(&self) -> Vec<FolderOption> {
        rows(&self.inputs())
    }
    fn inputs(&self) -> Inputs<'_> {
        Inputs {
            repositories: &self.repos,
            recent: &self.recent,
            plain: &self.plain,
            current: self.current.as_ref(),
            query: &self.query,
            allow_browse: self.browse,
            allow_create: self.create,
            allow_none: self.none,
            root: self.root.as_deref(),
            first_branch: "main",
        }
    }
}

fn titles(rows: &[FolderOption]) -> Vec<&str> {
    rows.iter().map(|r| r.title.as_str()).collect()
}

#[test]
fn rows_come_in_order_recent_repositories_folders_new_then_other() {
    let mut c = Case::new();
    c.recent = vec![Folder::Repository("/home/me/src/comet".into()), Folder::Directory("/srv/notes".into())];
    c.plain = vec!["/srv/notes".into(), "/srv/play".into()];
    let rows = c.rows();
    let groups: Vec<_> = rows.iter().map(|r| r.group).collect();
    assert_eq!(
        groups,
        [Group::Recent, Group::Recent, Group::Repositories, Group::Repositories, Group::Folders, Group::Other, Group::Other, Group::Other],
        "{:?}",
        titles(&rows)
    );
    // Recent keeps its own order; Repositories sort by name ignoring case; a recent one isn't listed twice.
    assert_eq!(titles(&rows)[..5], ["comet", "notes", "Aurora-docs", "rocket", "play"]);
}

#[test]
fn rows_match_case_insensitively_on_name_and_path_and_show_the_path_in_mono() {
    let mut c = Case::new();
    c.query = "AURORA".into();
    let rows = c.rows();
    assert_eq!(titles(&rows)[0], "Aurora-docs");
    assert_eq!(rows[0].detail, "~/src/Aurora-docs");
    assert!(rows[0].mono);
    c.query = "me/src/comet".into();
    assert_eq!(c.rows()[0].title, "comet", "the path is searched too");
}

#[test]
fn rows_have_the_other_rows_last_and_only_those_that_are_allowed() {
    let mut c = Case::new();
    let ids = |c: &Case| c.rows().iter().filter(|r| r.group == Group::Other).map(|r| r.id.clone()).collect::<Vec<_>>();
    assert_eq!(ids(&c), ["path", "clone-form", "none"]);
    assert_eq!(c.rows().iter().find(|r| r.id == "path").map(|r| r.title.as_str()), Some("Open or create a folder…"));
    c.create = false;
    assert_eq!(ids(&c), ["path", "none"]);
    assert_eq!(c.rows().iter().find(|r| r.id == "path").map(|r| r.detail.as_str()), Some("Any repository or folder on this machine"));
    c.browse = false;
    c.none = false;
    assert!(ids(&c).is_empty(), "cloud mode or an attached issue offers none of them");
    c.plain = vec!["/srv/play".into()];
    c.recent = vec![Folder::Directory("/srv/play".into())];
    assert!(c.rows().iter().all(|r| r.mark != Mark::Directory), "no directories without browsing");
}

#[test]
fn rows_offer_create_for_a_name_no_repository_has() {
    let mut c = Case::new();
    c.query = "Recipe Box".into();
    let rows = c.rows();
    let create = rows.iter().find(|r| r.id == "create").expect("a Create row");
    assert_eq!((create.title.as_str(), create.title_name.as_deref()), ("Create", Some("Recipe-Box")));
    assert_eq!(create.detail, "~/src/Recipe-Box · git repository on main");
    assert_eq!(group_label(&rows, rows.iter().position(|r| r.id == "create").unwrap_or(0)), Some("No folder matches"));
}

#[test]
fn rows_make_folders_inside_folders_typed_with_slash_or_backslash() {
    let mut c = Case::new();
    for typed in ["clients/acme/portal", "clients\\acme\\portal"] {
        c.query = typed.into();
        let rows = c.rows();
        let create = rows.iter().find(|r| r.id == "create").expect("Create");
        assert_eq!(create.title_name.as_deref(), Some("clients/acme/portal"));
        assert_eq!(create.act, Act::Create { target: "/home/me/src/clients/acme/portal".into(), root: "/home/me/src".into() });
    }
}

#[test]
fn rows_never_offer_to_create_a_repository_thats_already_there() {
    let mut c = Case::new();
    c.query = "Rocket".into();
    let rows = c.rows();
    assert!(rows.iter().all(|r| r.id != "create"));
    assert_eq!(rows[0].title, "rocket", "the repository itself is the match");
    c.query = "rocket/site/x".into();
    assert!(c.rows().iter().any(|r| r.id == "create"), "a folder inside it is not taken");
}

#[test]
fn rows_say_which_character_a_folder_name_cant_have_and_offer_no_create() {
    let mut c = Case::new();
    c.query = "what?".into();
    assert!(c.rows().iter().all(|r| r.id != "create"));
    assert_eq!(invalid_in_query(&c.inputs()), Some('?'));
    assert_eq!(empty_note(&c.inputs(), &c.rows()), None, "the alert speaks, not the note");
    c.query = "ada/recipe-box".into();
    assert_eq!(invalid_in_query(&c.inputs()), None, "a clone address is never a folder name");
}

#[test]
fn rows_send_a_path_of_its_own_to_the_folder_box() {
    let mut c = Case::new();
    for typed in ["~/src/acme", "/srv/acme", "C:\\source\\acme", "\\\\server\\share"] {
        c.query = typed.into();
        let go = c.rows().into_iter().find(|r| r.id == "go").expect(typed);
        assert_eq!((go.title.as_str(), go.title_name.as_deref()), ("Go to", Some(typed)));
        assert_eq!(go.detail, "Open it, or create it if it isn't there");
        assert_eq!(go.act, Act::Path(Some(typed.into())));
        assert!(c.rows().iter().all(|r| r.id != "create"));
    }
    c.create = false;
    assert_eq!(c.rows().into_iter().find(|r| r.id == "go").map(|r| r.detail), Some("Open it".into()));
}

#[test]
fn rows_offer_a_clone_for_a_pasted_repository_and_match_nothing_locally() {
    let mut c = Case::new();
    c.query = "https://github.com/ada/rocket.git".into();
    let rows = c.rows();
    assert!(!has_folder_matches(&rows), "the pasted address never filters local rows");
    let clone = rows.iter().find(|r| r.id == "clone").expect("Clone");
    assert_eq!((clone.title_name.as_deref(), clone.detail.as_str()), (Some("ada/rocket"), "into ~/src/rocket"));
    let at = rows.iter().position(|r| r.id == "clone").unwrap_or(0);
    assert_eq!(group_label(&rows, at), Some("Repository to clone"));
    c.create = false;
    assert!(c.rows().iter().all(|r| r.id != "clone"), "needs create allowed");
}

#[test]
fn rows_need_a_location_for_create_and_clone() {
    let mut c = Case::new();
    c.root = None;
    c.query = "Recipe Box".into();
    assert!(c.rows().iter().all(|r| r.group != Group::New));
    c.query = "ada/recipe-box".into();
    assert!(c.rows().iter().all(|r| r.group != Group::New));
}

#[test]
fn rows_mark_the_current_folder_and_no_folder_when_chat_only() {
    let mut c = Case::new();
    c.current = Some(Folder::Repository("/home/me/src/comet".into()));
    let rows = c.rows();
    assert_eq!(rows.iter().filter(|r| r.selected).map(|r| r.title.as_str()).collect::<Vec<_>>(), ["comet"]);
    c.current = Some(Folder::Directory("/home/me/src/comet".into()));
    assert!(c.rows().iter().all(|r| !r.selected), "the kind counts too");
    c.current = Some(Folder::None);
    assert_eq!(c.rows().iter().filter(|r| r.selected).map(|r| r.id.as_str()).collect::<Vec<_>>(), ["none"]);
}

#[test]
fn groups_label_their_first_row_and_a_rule_goes_above_other() {
    let mut c = Case::new();
    c.recent = vec![Folder::Repository("/home/me/src/comet".into())];
    let rows = c.rows();
    let labels: Vec<_> = (0..rows.len()).map(|i| group_label(&rows, i)).collect();
    assert_eq!(labels, [Some("Recent"), Some("Repositories"), None, None, None, None]);
    assert!((0..rows.len()).filter(|&i| starts_other(&rows, i)).eq([3]));
    c.query = "zzz".into();
    let rows = c.rows();
    assert!(!starts_other(&rows, 0), "no rule when Other is first");
}

#[test]
fn the_empty_notes_say_why_in_fleets_words() {
    let mut c = Case::new();
    c.repos.clear();
    assert_eq!(empty_note(&c.inputs(), &c.rows()).as_deref(), Some("No repositories yet. Type a name to create one."));
    c.create = false;
    assert_eq!(empty_note(&c.inputs(), &c.rows()).as_deref(), Some("No repositories yet. Open a folder to add one."));
    c.browse = false;
    assert_eq!(empty_note(&c.inputs(), &c.rows()).as_deref(), Some("No repositories found in your locations."));
    c.query = "  zzz ".into();
    assert_eq!(empty_note(&c.inputs(), &c.rows()).as_deref(), Some("No folder matches “zzz”."));
    let mut full = Case::new();
    full.repos.clear();
    full.query = "x".into();
    assert_eq!(empty_note(&full.inputs(), &full.rows()), None, "Create is offered");
}

#[test]
fn the_search_box_asks_for_what_it_can_do() {
    assert_eq!(placeholder(true), "Search, name a new folder, or paste a repository…");
    assert_eq!(placeholder(false), "Search repositories…");
}
