//! Fleet's `lib/__tests__/folder-access.test.ts` (`cloneRepository`) and the Clone view's text rules (`NSD/FolderPicker.vue`:
//! `cloneSuggestions`, `cloneTarget`, `cloneDescription`; rows B2.162-B2.166), with invented people.
use super::clone::*;
use super::feeds::NewFolder;
use std::io::Read;
use std::sync::{Arc, Mutex};

/// A body that arrives in the pieces given, as a network does.
struct Chunks(Vec<Vec<u8>>);

impl Read for Chunks {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        if self.0.is_empty() {
            return Ok(0);
        }
        let n = self.0[0].len().min(out.len());
        out[..n].copy_from_slice(&self.0[0][..n]);
        if n == self.0[0].len() {
            self.0.remove(0);
        } else {
            self.0[0].drain(..n);
        }
        Ok(n)
    }
}

fn chunks(parts: &[&str]) -> Chunks {
    Chunks(parts.iter().map(|p| p.as_bytes().to_vec()).collect())
}

const DONE: &str = r#"{"type":"done","folder":{"path":"/src/recipe-box","isGitRepo":true,"addedToFleet":false,"warning":null}}"#;

fn folder() -> NewFolder {
    NewFolder { path: "/src/recipe-box".into(), is_git_repo: true, added_to_fleet: false, warning: None }
}

#[test]
fn clone_repository_reports_each_progress_line_even_one_split_across_chunks_and_resolves_with_the_folder() {
    let progress = r#"{"type":"progress","phase":"Receiving objects","percent":38}"#;
    let (first, rest) = progress.split_at(20);
    let body = chunks(&[first, &format!("{rest}\n"), &format!("{DONE}\n")]);
    let mut seen = Vec::new();
    let made = follow(200, body, "/src/recipe-box", |p| seen.push(p));
    assert_eq!(seen, [CloneProgress { phase: "Receiving objects".into(), percent: 38 }]);
    assert_eq!(made, Ok(folder()));
}

#[test]
fn clone_repository_fails_with_gits_message_when_the_clone_fails_part_way() {
    let body = chunks(&["{\"type\":\"error\",\"error\":\"Couldn't clone: repository not found\"}\n"]);
    let failure = follow(200, body, "/src/b", |_| {}).unwrap_err();
    assert_eq!(failure.message, "Couldn't clone: repository not found");
    assert_eq!(failure.existing, None);
}

#[test]
fn clone_repository_fails_when_the_stream_ends_without_an_answer() {
    assert_eq!(follow(200, chunks(&[""]), "/src/b", |_| {}).unwrap_err().message, "The clone stopped before it finished.");
    let one_progress = chunks(&["{\"type\":\"progress\",\"phase\":\"Counting objects\",\"percent\":1}\n"]);
    assert_eq!(follow(200, one_progress, "/src/b", |_| {}).unwrap_err().message, "The clone stopped before it finished.");
}

#[test]
fn clone_repository_says_when_the_folder_is_already_there() {
    let failure = follow(409, chunks(&["{\"error\":\"/src/b already exists.\"}"]), "/src/b", |_| {}).unwrap_err();
    assert_eq!(failure.message, "/src/b already exists.");
    assert_eq!(failure.existing.as_deref(), Some("/src/b"));
    let other = follow(500, chunks(&["not json"]), "/src/b", |_| {}).unwrap_err();
    assert_eq!((other.message.as_str(), other.existing), ("Couldn't clone that repository.", None));
}

#[test]
fn clone_repository_reads_a_last_line_with_no_newline_and_a_multibyte_name_cut_in_two() {
    let line = r#"{"type":"done","folder":{"path":"/src/café","isGitRepo":false}}"#;
    let bytes = line.as_bytes();
    let at = line.find('é').unwrap() + 1;
    let body = Chunks(vec![bytes[..at].to_vec(), bytes[at..].to_vec()]);
    let made = follow(200, body, "/src/café", |_| {}).unwrap();
    assert_eq!(made.path, "/src/café");
}

struct Fake {
    icon: &'static str,
    repos: Mutex<Vec<RemoteRepository>>,
    refreshed: Mutex<u32>,
}

impl RepositorySource for Fake {
    fn icon(&self) -> &'static str {
        self.icon
    }
    fn refresh(&self) {
        *self.refreshed.lock().unwrap() += 1;
    }
    fn repositories(&self) -> Vec<RemoteRepository> {
        self.repos.lock().unwrap().clone()
    }
}

pub(crate) fn remote(full: &str) -> RemoteRepository {
    RemoteRepository { id: full.into(), full_name: full.into(), name: full.rsplit('/').next().unwrap().into() }
}

fn sources(names: &[&str]) -> (RepositorySources, Arc<Fake>) {
    let fake = Arc::new(Fake { icon: "github", repos: Mutex::new(names.iter().map(|n| remote(n)).collect()), refreshed: Mutex::new(0) });
    let mut all = RepositorySources::default();
    all.register(fake.clone());
    (all, fake)
}

#[test]
fn suggestions_leave_out_what_is_here_and_filter_by_what_is_typed() {
    let (all, _) = sources(&["ada/rocket", "ada/weave-website", "ada/garden-notes"]);
    let here = vec!["Rocket".to_string()];
    let names = |typed: &str| suggestions(&all, &here, typed).into_iter().map(|s| s.full_name).collect::<Vec<_>>();
    assert_eq!(names(""), ["ada/weave-website", "ada/garden-notes"], "a repository already here is not offered, ignoring case");
    assert_eq!(names("garden"), ["ada/garden-notes"]);
    assert_eq!(names("  WEAVE "), ["ada/weave-website"]);
    assert!(names("ada/garden-notes").is_empty(), "once the text reads as a repository nothing is suggested");
}

#[test]
fn suggestions_stop_at_five_and_are_none_without_a_source() {
    let (all, _) = sources(&["a/one", "a/two", "a/three", "a/four", "a/five", "a/six"]);
    assert_eq!(suggestions(&all, &[], "").len(), 5);
    assert!(suggestions(&RepositorySources::default(), &[], "").is_empty());
    assert!(RepositorySources::default().is_empty());
}

#[test]
fn refreshing_asks_every_source_again() {
    let (all, fake) = sources(&[]);
    all.refresh();
    all.refresh();
    assert_eq!(*fake.refreshed.lock().unwrap(), 2);
}

#[test]
fn the_plan_puts_the_clone_in_a_location_or_in_a_parent_typed_and_says_so() {
    let in_location = plan("ada/weave-website", Some("/home/me/work"), "");
    assert_eq!(in_location.target.as_deref(), Some("/home/me/work/weave-website"));
    assert_eq!(in_location.description(), "A clone of ada/weave-website");
    let elsewhere = plan("ada/weave-website", None, " /srv ");
    assert_eq!(elsewhere.target.as_deref(), Some("/srv/weave-website"));
    assert_eq!(elsewhere.description(), "A clone of ada/weave-website, added to your locations");
    assert_eq!(plan("ada/weave-website", None, "").target, None, "no parent yet");
    let not_yet = plan("garden", Some("/home/me/work"), "");
    assert_eq!(not_yet.description(), "A clone");
    assert_eq!(not_yet.target, None);
}
