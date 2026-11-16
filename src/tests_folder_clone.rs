//! Piece H of the New session page: the folder menu's clone view and a clone that runs (`NSD/FolderPicker.vue`, rows B2.160-B2.170;
//! Fleet's "making a folder" tests A21.79-A21.83; A21.84 is piece F's), driven with no window against the fake machine of `tests_folder_path.rs`,
//! whose clone stream is scripted and held back until the test lets it go on.
use crate::fleet::new_session::clone::{RemoteRepository, RepositorySource};
use crate::fleet::new_session::folder_menu::{Make, View};
use crate::fleet::new_session::request::Folder;
use crate::fleet::settings::HOME_MACHINE_KEY;
use crate::tests::{FRAME, click};
use crate::tests_folder_path::{Rig, Step, rig_with};
use rune::TextEdit;
use serde_json::{Value, json};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};

const PROGRESS: &str = r#"{"type":"progress","phase":"Receiving objects","percent":38}"#;

fn done(path: &str) -> String {
    json!({ "type": "done", "folder": { "path": path, "isGitRepo": true, "addedToFleet": false, "warning": null } }).to_string()
}

fn line(text: &str) -> Step {
    Step::Write(format!("{text}\n"))
}

/// A machine whose clone stream does `steps`, one wait at a time: the test sends on the returned channel to let it go on.
fn rig_cloning(status: u16, steps: Vec<Step>) -> (Rig, Sender<()>) {
    let (go, gate) = channel();
    let rig = rig_with(|c| {
        c.roots = vec!["/home/me/src".into(), "/home/me/work".into()];
        c.clone = (status, steps);
        c.gate = Some(Arc::new(Mutex::new(gate)));
    });
    (rig, go)
}

struct Source(Mutex<Vec<RemoteRepository>>, Mutex<u32>);

impl RepositorySource for Source {
    fn icon(&self) -> &'static str {
        "github"
    }
    fn refresh(&self) {
        *self.1.lock().unwrap() += 1;
    }
    fn repositories(&self) -> Vec<RemoteRepository> {
        self.0.lock().unwrap().clone()
    }
}

fn source(r: &mut Rig, names: &[&str]) -> Arc<Source> {
    let repos =
        names.iter().map(|n| RemoteRepository { id: n.to_string(), full_name: n.to_string(), name: n.rsplit('/').next().unwrap().into() });
    let source = Arc::new(Source(Mutex::new(repos.collect()), Mutex::new(0)));
    let registered = source.clone();
    r.rt.update(move |s, _| s.folder_menu.sources.register(registered));
    source
}

fn clone_requests(r: &Rig) -> Vec<Value> {
    r.asked().iter().filter_map(|l| l.strip_prefix("POST /api/directories/clone ")).filter_map(|b| serde_json::from_str(b).ok()).collect()
}

fn names(r: &Rig) -> Vec<String> {
    r.state().clone_suggestions().into_iter().map(|s| s.full_name).collect()
}

fn open_clone_view(r: &mut Rig) {
    r.open_menu();
    click(&mut r.rt, "new-session-folder-clone-form");
    r.rt.frame(FRAME);
}

#[test]
fn a_pasted_repository_offers_a_clone_which_starting_waits_for() {
    let (mut r, go) = rig_cloning(200, vec![line(PROGRESS), Step::Wait, line(&done("/home/me/src/recipe-box"))]);
    r.search("https://github.com/ada/recipe-box.git");
    let rows = r.state().folder_rows();
    let clone = rows.iter().find(|o| o.id == "clone").expect("a Clone row");
    assert_eq!((clone.title.as_str(), clone.title_name.as_deref()), ("Clone", Some("ada/recipe-box")));
    assert_eq!(clone.detail, "into ~/src/recipe-box");
    assert!(rows.iter().all(|o| o.id != "create"), "a repository is cloned, not made");
    let at = rows.iter().position(|o| o.id == "clone").unwrap();
    assert_eq!(crate::fleet::new_session::folder::group_label(&rows, at), Some("Repository to clone"));

    click(&mut r.rt, "new-session-folder-clone");
    r.wait("git's first words", |s| s.folder_menu.clone_percent == Some(38));
    assert_eq!(clone_requests(&r), [json!({ "repository": "ada/recipe-box", "path": "/home/me/src/recipe-box" })]);
    assert_eq!(r.chip(), "Cloning recipe-box… 38%");
    assert_eq!(r.state().clone_status_text(), "Receiving objects 38%");
    assert!(r.state().folder_clone_busy(), "starting a session waits while the clone runs");

    go.send(()).unwrap();
    r.wait("the clone to end", |s| s.folder_menu.make == Make::Idle);
    assert_eq!(r.folder(), Some(Folder::Repository("/home/me/src/recipe-box".into())));
    assert_eq!(r.chip(), "recipe-box");
    assert!(!r.state().folder_clone_busy());
    assert!(r.state().folder_is_new(r.folder().as_ref()));
    assert!(!r.state().folder_menu_open());
}

#[test]
fn picking_another_folder_while_a_clone_runs_lets_the_clone_finish_without_taking_over() {
    let (mut r, go) = rig_cloning(200, vec![Step::Wait, line(&done("/home/me/src/recipe-box"))]);
    r.search("ada/recipe-box");
    click(&mut r.rt, "new-session-folder-clone");
    r.wait("the request", |_| true);
    r.rt.update(|s, _| {
        s.folder_menu.query = TextEdit::new("comet");
        s.folder_query_changed();
    });
    r.rt.frame(FRAME);
    assert_eq!(r.state().folder_menu.make, Make::Cloning, "typing doesn't stop a clone");
    click(&mut r.rt, "new-session-folder-row-0");
    assert_eq!(r.folder(), Some(Folder::Repository("/home/me/src/comet".into())));
    assert!(!r.state().folder_clone_busy(), "the clone no longer holds the session back");
    assert_eq!(r.chip(), "comet");

    go.send(()).unwrap();
    r.wait("the clone to end", |s| s.folder_menu.make == Make::Idle);
    // The refresh is asked for as the clone ends: the list is read again so the folder shows next time.
    assert!(r.asked().iter().any(|l| l.starts_with("POST /api/repositories/refresh")));
    assert_eq!(r.folder(), Some(Folder::Repository("/home/me/src/comet".into())), "the choice stays");
    assert!(r.state().folder_menu.created.contains("/home/me/src/recipe-box"));
}

#[test]
fn clone_a_repository_clones_one_of_yours_into_a_location_or_anywhere() {
    let (mut r, _go) = rig_cloning(200, vec![line(&done("/home/me/work/weave-website"))]);
    let src = source(&mut r, &["ada/rocket", "ada/weave-website"]);
    open_clone_view(&mut r);
    assert_eq!(r.state().folder_menu.view, View::Clone);
    assert_eq!(*src.1.lock().unwrap(), 1, "the sources are asked again when the view opens");
    assert_eq!(names(&r), ["ada/weave-website"], "only repositories that aren't on this computer yet");

    click(&mut r.rt, "new-session-clone-suggestion-0");
    assert_eq!(r.state().folder_menu.clone_text.text, "ada/weave-website");
    click(&mut r.rt, "new-session-clone-location");
    r.rt.frame(FRAME);
    click(&mut r.rt, "new-session-clone-location-other");
    r.rt.frame(FRAME);
    r.rt.update(|s, _| s.folder_menu.clone_parent = TextEdit::new("/srv"));
    r.rt.frame(FRAME);
    let plan = r.state().clone_plan();
    assert_eq!(plan.target.as_deref(), Some("/srv/weave-website"));
    assert!(plan.description().contains("added to your locations"));
    assert!(r.rt.find("new-session-clone-preview").is_some());

    click(&mut r.rt, "new-session-clone-location");
    r.rt.frame(FRAME);
    click(&mut r.rt, "new-session-clone-location-1");
    assert_eq!(r.state().clone_plan().target.as_deref(), Some("/home/me/work/weave-website"));
    assert!(r.state().can_submit_clone());
    click(&mut r.rt, "new-session-clone-submit");
    r.wait("the clone to end", |s| s.folder_menu.make == Make::Idle && s.folder_menu.clone_name.is_none() && !s.folder_menu_open());
    assert_eq!(clone_requests(&r), [json!({ "repository": "ada/weave-website", "path": "/home/me/work/weave-website" })]);
    assert!(r.chip().contains("weave-website"));
    assert_eq!(r.state().sidebar.new_session_defaults(HOME_MACHINE_KEY).last_new_folder_root(), Some("/home/me/work"));
}

#[test]
fn suggests_nothing_from_github_when_the_account_has_no_repositories_and_filters_by_what_is_typed() {
    let (mut r, _go) = rig_cloning(200, vec![]);
    let src = source(&mut r, &[]);
    open_clone_view(&mut r);
    assert!(names(&r).is_empty());
    assert_eq!(*src.1.lock().unwrap(), 1);

    *src.0.lock().unwrap() = ["ada/weave-website", "ada/garden-notes"]
        .iter()
        .map(|n| RemoteRepository { id: n.to_string(), full_name: n.to_string(), name: n.rsplit('/').next().unwrap().into() })
        .collect();
    assert_eq!(names(&r).len(), 2);
    r.rt.update(|s, _| s.folder_menu.clone_text = TextEdit::new("garden"));
    assert_eq!(names(&r), ["ada/garden-notes"]);
}

#[test]
fn suggests_nothing_when_no_plugin_contributes_a_repository_source() {
    let (mut r, _go) = rig_cloning(200, vec![]);
    open_clone_view(&mut r);
    assert!(r.state().folder_menu.sources.is_empty());
    assert!(names(&r).is_empty());
    assert!(r.rt.find("new-session-clone-suggestions").is_none());
    assert!(r.rt.find("new-session-clone-repository").is_some());
}

#[test]
fn the_clone_view_starts_from_a_pasted_repository_and_its_buttons_follow_what_is_typed() {
    let (mut r, _go) = rig_cloning(200, vec![]);
    r.search("ada/recipe-box");
    click(&mut r.rt, "new-session-folder-clone-form");
    assert_eq!(r.state().folder_menu.clone_text.text, "ada/recipe-box", "what was pasted is carried over");
    assert_eq!(r.state().folder_menu.clone_location.as_deref(), Some("/home/me/src"));
    r.rt.update(|s, _| s.folder_menu.clone_text = TextEdit::new("recipe box"));
    r.rt.frame(FRAME);
    assert!(!r.state().can_submit_clone(), "not a repository");
    assert!(r.rt.find("new-session-clone-preview").is_none());
    r.rt.update(|s, _| s.folder_menu.clone_text = TextEdit::new("ada/recipe-box"));
    assert!(r.state().can_submit_clone());
    click(&mut r.rt, "new-session-clone-cancel");
    assert_eq!(r.state().folder_menu.view, View::List);
}

#[test]
fn a_clone_that_fails_after_the_menu_was_shut_reopens_it_to_show_why_and_offers_the_folder_that_was_there() {
    let body = json!({ "error": "/home/me/src/recipe-box already exists." }).to_string();
    let (mut r, go) = rig_cloning(409, vec![Step::Wait, Step::Write(body)]);
    open_clone_view(&mut r);
    r.rt.update(|s, _| s.folder_menu.clone_text = TextEdit::new("ada/recipe-box"));
    click(&mut r.rt, "new-session-clone-submit");
    r.wait("the request", |s| s.folder_menu.make == Make::Cloning);
    r.rt.update(|s, _| s.close_folder_menu());
    assert!(!r.state().folder_menu_open());

    go.send(()).unwrap();
    r.wait("the failure", |s| s.folder_menu.make == Make::Idle);
    assert!(r.state().folder_menu_open(), "reopened as it was");
    assert_eq!(r.state().folder_menu.view, View::Clone);
    assert_eq!(r.state().folder_menu.error.as_deref(), Some("/home/me/src/recipe-box already exists."));
    assert_eq!(r.state().folder_menu.existing.as_deref(), Some("/home/me/src/recipe-box"));
    assert!(r.rt.find("new-session-folder-use-existing").is_some());
}
