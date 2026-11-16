//! `path.rs`: the path view's rows and text rules, with no machine (`NSD/FolderPicker.vue`, rows B2.112-B2.143).
use super::feeds::{Entry, Listing};
use super::path::*;

fn entry(dir: &str, name: &str, git: bool) -> Entry {
    Entry { name: name.into(), path: format!("{dir}/{name}"), is_git_repo: git }
}

fn src() -> Listing {
    Listing {
        path: "/home/me/src".into(),
        exists: true,
        nearest_existing: None,
        entries: vec![entry("/home/me/src", "clients", false), entry("/home/me/src", "comet", true)],
    }
}

fn rows(text: &str, listing: &Listing) -> Vec<(PathAction, String)> {
    let i = PathInputs { text, listing: Some(listing), allow_create: true, home: Some("/home/me") };
    path_rows(&i).into_iter().map(|r| (r.action, format!("{} {}", r.title, r.title_name.unwrap_or_default()).trim().to_string())).collect()
}

#[test]
fn typed_text_splits_into_the_folder_to_list_and_the_name() {
    assert_eq!(split_typed_path("~/src/cl"), TypedPath { folder: "~/src/", name: "cl" });
    assert_eq!(split_typed_path("a\\b"), TypedPath { folder: "a\\", name: "b" });
    assert_eq!(split_typed_path("plain"), TypedPath { folder: "", name: "plain" });
}

#[test]
fn the_folder_to_list_is_inside_the_location_unless_the_text_is_a_path_of_its_own() {
    assert_eq!(folder_to_list("", Some("/home/me/src"), '/'), "/home/me/src");
    assert_eq!(folder_to_list("", None, '/'), "~");
    assert_eq!(folder_to_list("clients/ac", Some("/home/me/src"), '/'), "/home/me/src/clients");
    assert_eq!(folder_to_list("~/src/", Some("/x"), '/'), "~/src");
    assert_eq!(folder_to_list("/", Some("/x"), '/'), "/");
    assert_eq!(folder_to_list("c:/source/", Some("C:\\work"), '\\'), "c:\\source");
    assert_eq!(folder_to_list("C:\\", Some("C:\\work"), '\\'), "C:\\");
}

#[test]
fn only_the_machines_own_home_is_written_as_tilde() {
    assert_eq!(typed_path_for("/home/me/src", Some("/home/me")), "~/src");
    assert_eq!(typed_path_for("/home/me", Some("/home/me")), "~");
    assert_eq!(typed_path_for("/home/me2/src", Some("/home/me")), "/home/me2/src");
    assert_eq!(typed_path_for("/home/ME/src", Some("/home/me")), "~/src");
    assert_eq!(typed_path_for("/home/me/src", Some("/home/someone-else")), "/home/me/src");
    assert_eq!(typed_path_for("/home/me/src", None), "/home/me/src");
    assert_eq!(initial_text(Some("/home/me/src"), Some("/home/someone-else"), '/'), "/home/me/src/");
    assert_eq!(initial_text(None, None, '\\'), "~\\");
}

#[test]
fn up_goes_to_the_parent_and_a_root_stays_a_root() {
    assert_eq!(parent_of("/home/me/src").as_deref(), Some("/home/me"));
    assert_eq!(parent_of("/home").as_deref(), Some("/"));
    assert_eq!(parent_of("/").as_deref(), Some("/"));
    assert_eq!(parent_of("C:\\src").as_deref(), Some("C:\\"));
    assert_eq!(parent_of("nothing"), None);
    assert_eq!(entered_text("/", None), "/");
    assert_eq!(entered_text("C:\\", None), "C:\\");
    assert_eq!(entered_text("/home/me/src", Some("/home/me")), "~/src/");
}

#[test]
fn an_empty_name_opens_here_then_lists_and_makes_nothing() {
    let r = rows("~/src/", &src());
    assert_eq!(r, [(PathAction::Open, "Open src".into()), (PathAction::Enter, "clients".into()), (PathAction::Enter, "comet".into())]);
}

#[test]
fn a_name_that_starts_something_leaves_create_last() {
    let r = rows("~/src/cl", &src());
    assert_eq!(r, [(PathAction::Enter, "clients".into()), (PathAction::Create, "Create cl".into())]);
}

#[test]
fn a_name_nothing_starts_leads_with_create() {
    assert_eq!(rows("~/src/zz", &src()), [(PathAction::Create, "Create zz".into())]);
}

#[test]
fn the_exact_name_opens_first_whatever_its_case_and_makes_nothing() {
    let r = rows("~/src/Comet", &src());
    assert_eq!(r, [(PathAction::Open, "Open comet".into())]);
    let r = rows("~/src/clients", &src());
    assert_eq!(r, [(PathAction::Open, "Open clients".into())]);
}

#[test]
fn a_folder_that_is_not_there_offers_only_create_with_the_folders_above_it() {
    let missing = Listing {
        path: "/home/me/src/clients/acme".into(),
        exists: false,
        nearest_existing: Some("/home/me/src/clients".into()),
        entries: vec![],
    };
    let i = PathInputs { text: "~/src/clients/acme/site", listing: Some(&missing), allow_create: true, home: Some("/home/me") };
    let r = path_rows(&i);
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].title_name.as_deref(), Some("acme/site"));
    assert_eq!(r[0].detail, "in ~/src/clients");
    assert_eq!(r[0].path, "/home/me/src/clients/acme/site");
    let plan = path_create(&i).unwrap();
    assert_eq!(plan.folders, ["acme", "site"]);
    assert_eq!(plan.base, "/home/me/src/clients");
}

#[test]
fn an_invalid_name_gives_a_note_and_no_create() {
    let i = PathInputs { text: "~/src/notes?", listing: Some(&src()), allow_create: true, home: None };
    assert_eq!(path_note(&i, None).as_deref(), Some("“?” can't be in a folder name."));
    assert!(path_rows(&i).iter().all(|r| r.action != PathAction::Create));
}

#[test]
fn without_create_a_missing_folder_says_so_and_a_listing_error_wins() {
    let missing = Listing { path: "/home/me/nope".into(), exists: false, nearest_existing: Some("/home/me".into()), entries: vec![] };
    let i = PathInputs { text: "~/nope", listing: Some(&missing), allow_create: false, home: Some("/home/me") };
    assert_eq!(path_note(&i, None).as_deref(), Some("There's no folder at ~/nope."));
    assert_eq!(path_note(&i, Some("Couldn't list that folder.")).as_deref(), Some("Couldn't list that folder."));
    assert!(path_rows(&i).is_empty());
}

#[test]
fn a_folder_outside_the_locations_adds_it_but_going_into_a_folder_does_not() {
    let roots = vec!["/home/me/src".to_string()];
    let row = |action, path: &str| PathRow {
        id: "x".into(),
        action,
        title: String::new(),
        title_name: None,
        detail: String::new(),
        path: path.into(),
        is_git_repo: false,
    };
    assert!(adds_location(Some(&row(PathAction::Open, "/opt/x")), Some(&roots)));
    assert!(!adds_location(Some(&row(PathAction::Open, "/home/me/src/x")), Some(&roots)));
    assert!(!adds_location(Some(&row(PathAction::Enter, "/opt/x")), Some(&roots)));
    assert!(!adds_location(Some(&row(PathAction::Open, "/opt/x")), None), "not before the locations are known");
}
