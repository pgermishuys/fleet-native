//! The changes list (component 12, piece A): `use-diffs.test.ts` under Fleet's names, over `Diffs` itself where the case is a
//! rule and over a fake server where it is the thread, the events and the 250 ms window; and `diffBaseLabel`.
use super::super::diffs::*;
use super::super::events::{Event, Kind};
use super::super::link::{Job, Link, Post};
use super::super::sessions;
use super::super::state::Topic;
use super::reconnect::app;
use super::*;
use crate::State;
use std::io::Write;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};

fn item(file: &str, additions: u64) -> Value {
    json!({"file": file, "status": "modified", "additions": additions, "deletions": 0})
}

fn body(items: &[Value]) -> (u16, Value) {
    (200, json!({"diffs": items, "available": true}))
}

fn load(d: &mut Diffs, answer: (u16, Value)) {
    let ticket = d.begin(false).unwrap();
    d.finish(&ticket, Ok(answer));
}

fn opened(session: &str) -> Diffs {
    let mut d = Diffs::default();
    d.reset(session);
    d
}

// ─── use-diffs.test.ts ──────────────────────────────────────────────────────

#[test]
fn is_not_loaded_until_the_first_answer_arrives_and_not_loaded_again_for_another_session() {
    let mut d = opened("session-1");
    let ticket = d.begin(false).unwrap();
    assert!(!d.has_loaded && d.is_loading());
    d.finish(&ticket, Ok(body(&[])));
    assert!(d.has_loaded && !d.is_loading());
    d.reset("session-2");
    assert!(!d.has_loaded);
}

#[test]
fn fetches_latest_diffs_when_a_turn_ends_for_the_current_session() {
    let mut d = opened("session-1");
    let ticket = d.on_turn_ended().unwrap();
    assert_eq!(ticket.session, "session-1");
    d.finish(
        &ticket,
        Ok((200, json!({"diffs": [{"file": "src/App.vue", "status": "modified", "additions": 3, "deletions": 1}], "available": true}))),
    );
    assert_eq!(d.files().len(), 1);
    assert_eq!((d.files()[0].file.as_str(), d.files()[0].additions, d.files()[0].deletions), ("src/App.vue", 3, 1));
    assert!(d.available && !d.is_stale);
}

#[test]
fn keeps_the_same_list_when_a_refresh_finds_nothing_new_and_indexes_it_by_path() {
    let mut d = opened("session-1");
    let wire = |n| {
        (
            200,
            json!({"diffs": [item("src/App.vue", 3), {"file": "src/main.ts", "status": "added", "additions": n, "deletions": 0}], "available": true}),
        )
    };
    load(&mut d, wire(9));
    assert_eq!(d.by_file("src/main.ts").map(|f| f.additions), Some(9));
    // Nothing changed: what was held stays, down to a flag the key doesn't cover.
    let held = d.files().as_ptr();
    load(&mut d, wire(9));
    assert_eq!(d.files().as_ptr(), held);
    load(&mut d, wire(10));
    assert_ne!(d.files().as_ptr(), held);
    assert_eq!(d.by_file("src/main.ts").map(|f| f.additions), Some(10));
    assert!(d.by_file("src/other.ts").is_none());
}

#[test]
fn tracks_stale_state_until_the_next_successful_fetch() {
    let mut d = opened("session-1");
    d.mark_stale();
    assert!(d.is_stale);
    load(&mut d, body(&[item("src/App.vue", 1)]));
    assert!(!d.is_stale);
}

#[test]
fn clears_stale_state_when_session_or_instance_changes() {
    let mut d = opened("session-1");
    d.mark_stale();
    assert!(d.is_stale);
    d.reset("session-2");
    assert!(!d.is_stale);
}

#[test]
fn ignores_turn_ended_events_for_other_sessions() {
    let mut rt = app();
    rt.update(|s, _| {
        s.diffs.reset("session-1");
        // A topic that isn't the session's own, and no link either: nothing goes out and nothing is loading.
        s.dispatch(&Topic::Session("session-2".into()), &[Event::TurnEnded]);
        s.dispatch(&Topic::Sessions, &[Event::TurnEnded]);
        assert!(!s.diffs.is_loading());
    });
}

#[test]
fn exposes_unavailable_state_from_the_diffs_response() {
    let mut d = opened("session-1");
    load(&mut d, (200, json!({"diffs": [], "available": false})));
    assert!(d.files().is_empty() && !d.available && d.error.is_none());
}

#[test]
fn exposes_what_the_changes_are_compared_with_keeping_it_while_it_doesnt_move() {
    let answer =
        |commit: &str| (200, json!({"diffs": [], "available": true, "base": {"kind": "branch", "branch": "main", "commit": commit}}));
    let branch = |commit: &str| Some(Base::Branch { branch: Some("main".into()), commit: Some(commit.into()) });
    let mut d = opened("session-1");
    load(&mut d, answer("aaa1111"));
    assert_eq!(d.base, branch("aaa1111"));
    load(&mut d, answer("aaa1111"));
    assert_eq!(d.base, branch("aaa1111"));
    // The branch was rebased onto a newer main.
    load(&mut d, answer("bbb2222"));
    assert_eq!(d.base, branch("bbb2222"));
    d.reset("session-2");
    assert_eq!(d.base, None);
}

#[test]
fn documents_that_backend_session_diff_summaries_include_before_and_after_content() {
    let mut d = opened("session-1");
    let summary = json!({"file": "src/App.vue", "status": "modified", "additions": 1, "deletions": 1,
        "before": "original content", "after": "modified content", "isBinary": false, "isTruncated": false});
    load(&mut d, (200, json!({"diffs": [summary], "available": true})));
    let f = &d.files()[0];
    assert_eq!(f.status, FileStatus::Modified);
    assert_eq!((f.before.as_str(), f.after.as_str(), f.binary, f.truncated), ("original content", "modified content", false, false));
}

#[test]
fn leaves_a_closed_panels_diffs_alone_while_files_change_and_catches_up_when_it_opens() {
    let mut d = opened("session-1");
    for _ in 0..3 {
        // Nothing shows the diffs: no window opens, no request goes out.
        assert!(!d.on_files_changed(false));
        assert!(d.window_ended("session-1").is_none());
    }
    assert!(d.is_stale && !d.is_loading());
    let ticket = d.on_wanted().unwrap();
    d.finish(&ticket, Ok(body(&[item("a.ts", 1)])));
    assert!(!d.is_stale);
    // Opening with a list that isn't stale asks for nothing.
    assert!(d.on_wanted().is_none());
    // And an edit once it is open opens the window, which ends in a request.
    assert!(d.on_files_changed(true));
    assert!(d.window_ended("session-1").is_some());
}

#[test]
fn asks_once_when_the_sessions_page_and_the_right_panel_both_open_the_same_session_but_afresh_after_an_edit() {
    let mut d = opened("session-join");
    let first = d.begin(true).unwrap();
    // The panel asks while the page's request is on its way: it waits for that one.
    assert!(d.begin(true).is_none());
    d.finish(&first, Ok(body(&[item("a.ts", 1)])));
    assert_eq!(d.files().len(), 1);
    // After an edit or a revert nothing is joined: the answer on its way may predate the change.
    let old = d.begin(false).unwrap();
    let fresh = d.begin(false).unwrap();
    d.finish(&old, Ok(body(&[])));
    assert_eq!(d.files().len(), 1, "the older answer is dropped");
    d.finish(&fresh, Ok(body(&[])));
    assert!(d.files().is_empty());
}

#[test]
fn refreshes_during_a_long_run_of_edits_not_only_once_they_stop_and_never_runs_two_at_once() {
    // An edit every 200 ms for two seconds; a request takes 400 ms; a window is 250 ms. Time runs in 50 ms steps.
    let mut d = opened("session-burst");
    let (mut window_ends, mut answer_at, mut on_the_way, mut sent) = (None::<u32>, None::<u32>, None::<Ticket>, 0);
    for now in (0..=4000u32).step_by(50) {
        if let (Some(t), Some(at)) = (&on_the_way, answer_at)
            && now >= at
        {
            let next = d.finish(&t.clone(), Ok(body(&[])));
            on_the_way = next.clone();
            answer_at = next.map(|_| now + 400);
        }
        if window_ends.is_some_and(|at| now >= at) {
            window_ends = None;
            if let Some(t) = d.window_ended("session-burst") {
                (on_the_way, answer_at) = (Some(t), Some(now + 400));
            }
        }
        if now < 2000 && now % 200 == 0 && d.on_files_changed(true) {
            window_ends = Some(now + 250);
        }
        // Count requests as they appear: a new ticket is one whose answer time was just set.
        if answer_at == Some(now + 400) {
            sent += 1;
        }
    }
    // Two seconds of edits: refreshed while they went on, one at a time.
    assert!(
        sent >= 3,
        "refreshed during the edits, not only after (each ticket is the only one on its way: `begin` refuses a second refresh): {sent} requests"
    );
    assert!(!d.is_loading());
}

#[test]
fn one_refresh_still_running_when_the_window_ends_is_followed_by_exactly_one_more() {
    let mut d = opened("s");
    assert!(d.on_files_changed(true));
    let running = d.window_ended("s").unwrap();
    // Two more windows end while it runs: one follow-up, not two.
    assert!(d.on_files_changed(true));
    assert!(d.window_ended("s").is_none());
    assert!(d.on_files_changed(true));
    assert!(d.window_ended("s").is_none());
    let next = d.finish(&running, Ok(body(&[]))).unwrap();
    assert!(d.finish(&next, Ok(body(&[]))).is_none());
}

#[test]
fn an_answer_for_an_older_request_or_another_session_is_dropped() {
    let mut d = opened("a");
    let old = d.begin(false).unwrap();
    d.reset("b");
    d.finish(&old, Ok(body(&[item("x.ts", 1)])));
    assert!(d.files().is_empty() && !d.has_loaded && d.session() == "b");
}

#[test]
fn a_refusal_a_bare_array_and_a_failure_are_read() {
    let mut d = opened("s");
    load(
        &mut d,
        (
            200,
            json!({"diffs": [], "available": false, "refusal": {"message": "Not yours", "fixCommand": "git config --global --add safe.directory ."}}),
        ),
    );
    assert_eq!(d.refusal, Some(Refusal { message: "Not yours".into(), fix_command: "git config --global --add safe.directory .".into() }));
    // An old server answers a bare array, which counts as available.
    load(&mut d, (200, json!([item("a.ts", 2)])));
    assert!(d.available && d.refusal.is_none() && d.files().len() == 1);
    let t = d.begin(false).unwrap();
    d.finish(&t, Ok((500, json!({}))));
    assert_eq!(d.error.as_deref(), Some("HTTP 500"));
    assert!(!d.available && d.base.is_none() && d.has_loaded);
    assert_eq!(d.files().len(), 1, "the list stays while a refresh fails");
    let t = d.begin(false).unwrap();
    d.finish(&t, Err("connection refused".into()));
    assert_eq!(d.error.as_deref(), Some("connection refused"));
}

#[test]
fn reads_the_mock_answers() {
    let mut d = opened("s");
    load(&mut d, (200, crate::mock::diffs::changed()));
    let statuses: Vec<_> = d.files().iter().map(|f| f.status).collect();
    assert_eq!(statuses, [FileStatus::Added, FileStatus::Modified, FileStatus::Deleted, FileStatus::Modified]);
    assert!(d.by_file("package-lock.json").unwrap().truncated);
    load(&mut d, (200, crate::mock::diffs::unchanged()));
    assert!(d.files().is_empty() && d.base == Some(Base::Session));
}

// ─── lib/diff-base.ts ───────────────────────────────────────────────────────

#[test]
fn diff_base_label_for_a_branch_says_what_it_is_compared_with_and_where_it_left_main() {
    let label = diff_base_label(Some(&Base::Branch { branch: Some("develop".into()), commit: Some("abc1234def".into()) })).unwrap();
    assert_eq!(label.text, "Compared with develop");
    assert_eq!(label.commit.as_deref(), Some("abc1234"));
    assert_eq!(
        label.detail,
        "What this branch changes since it left develop at abc1234, committed or not. Rebasing onto develop or pulling it doesn't add other people's work."
    );
}

#[test]
fn diff_base_label_without_a_branch_name_or_commit_says_main_and_leaves_the_commit_out() {
    for branch in [None, Some(String::new())] {
        let label = diff_base_label(Some(&Base::Branch { branch, commit: None })).unwrap();
        assert_eq!((label.text.as_str(), label.commit), ("Compared with main", None));
        assert!(label.detail.starts_with("What this branch changes since it left main, committed or not."));
    }
}

#[test]
fn diff_base_label_for_a_session_base_and_for_none() {
    let label = diff_base_label(Some(&Base::Session)).unwrap();
    assert_eq!(label.text, "Since this session started");
    assert_eq!(label.commit, None);
    assert!(label.detail.starts_with("This folder isn't on a branch of its own"));
    assert!(diff_base_label(None).is_none());
}

// ─── over a fake server: the thread, the events, the window ────────────────

#[derive(Default)]
struct Seen {
    asked: Vec<String>,
    answer: String,
}

struct Live {
    rt: rune::Runtime<State>,
    id: String,
    jobs: std::sync::mpsc::Receiver<Job>,
    seen: Arc<Mutex<Seen>>,
}

fn serve(mut stream: std::net::TcpStream, seen: Arc<Mutex<Seen>>) {
    let heard = crate::test_http::read(&mut stream);
    if heard.method.is_empty() {
        return;
    }
    let request = heard.head.lines().next().unwrap_or("").to_string();
    let path = request.split(' ').nth(1).unwrap_or("").to_string();
    let body = if path.starts_with("/hubs/session-events/negotiate") {
        "{\"negotiateVersion\":1,\"connectionToken\":\"tok\"}".to_string()
    } else if path.starts_with("/api/sessions?") || path == "/api/sessions" {
        fixture("api-sessions.json")
    } else if path.ends_with("/diffs") {
        let mut seen = seen.lock().unwrap();
        seen.asked.push(path.clone());
        seen.answer.clone()
    } else {
        "{}".to_string()
    };
    let _ = stream.write_all(
        format!("HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len())
            .as_bytes(),
    );
}

impl Live {
    fn start() -> Live {
        Live::start_answering(crate::mock::diffs::changed().to_string())
    }

    /// The first `/diffs` request goes out inside `start`, so a test that needs a particular first answer must give it here,
    /// not set it afterwards (the request may already have been answered with the default).
    fn start_answering(answer: String) -> Live {
        let listener = crate::test_http::bind().unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let seen = Arc::new(Mutex::new(Seen { answer, ..Default::default() }));
        let shared = seen.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let seen = shared.clone();
                std::thread::spawn(move || serve(stream, seen));
            }
        });
        let (tx, jobs) = channel::<Job>();
        let tx = Mutex::new(tx);
        let post = Post::new(move |job| {
            let _ = tx.lock().unwrap().send(job);
        });
        let mut rt = app();
        let link = Link::start(base, post);
        rt.update(move |s, tx| {
            s.link = Some(link);
            s.on_list(sessions::parse_list(&serde_json::from_str(&fixture("api-sessions.json")).unwrap(), 0), tx);
        });
        let id = rt.state.open_id().unwrap();
        Live { rt, id, jobs, seen }
    }

    fn wait(&mut self, what: &str, done: impl Fn(&State, usize) -> bool) {
        let (seen, described) = (self.seen.clone(), self.seen.clone());
        crate::test_wait::drive(
            &mut self.rt,
            &self.jobs,
            what,
            |s| done(s, seen.lock().unwrap().asked.len()),
            |s| format!("{}; asked {:?}", crate::test_wait::app_state(s), described.lock().unwrap().asked),
        );
    }

    fn event(&mut self, event: Event) {
        let id = self.id.clone();
        self.rt.update(move |s, _| s.dispatch(&Topic::Session(id), &[event]));
    }

    fn files_changed(&mut self) {
        let id = self.id.clone();
        self.event(Event::Other(Kind::FilesChanged, json!({"sessionId": id, "files": [{"path": "a.ts", "changeType": "modified"}]})));
    }
}

#[test]
fn opening_a_session_asks_once_and_reads_the_list() {
    let mut live = Live::start();
    live.wait("the list", |s, _| s.diffs.has_loaded);
    let id = live.id.clone();
    assert_eq!(live.rt.state.diffs.session(), id);
    assert_eq!(live.rt.state.diffs.files().len(), 4);
    // Opening again with a list that is loaded and fresh asks for nothing.
    live.rt.update(|s, tx| s.after_open(tx));
    live.rt.frame(1.0 / 60.0);
    assert_eq!(live.seen.lock().unwrap().asked, [format!("/api/sessions/{id}/diffs")]);
}

#[test]
fn a_turn_ending_on_the_open_session_asks_at_once() {
    let mut live = Live::start();
    live.wait("the first list", |s, _| s.diffs.has_loaded);
    live.seen.lock().unwrap().answer = crate::mock::diffs::unchanged().to_string();
    live.event(Event::TurnEnded);
    live.wait("the list after the turn", |s, asked| asked == 2 && s.diffs.has_loaded && s.diffs.files().is_empty());
}

#[test]
fn edits_gather_in_one_window_even_while_the_panel_is_folded() {
    let mut live = Live::start();
    live.wait("the first list", |s, _| s.diffs.has_loaded);
    live.rt.update(|s, _| s.content_open = false);
    live.seen.lock().unwrap().answer = crate::mock::diffs::unchanged().to_string();
    // The desktop's panel has no `isWanted`: a folded panel still learns of the change, three edits in one request.
    for _ in 0..3 {
        live.files_changed();
    }
    live.wait("the window's request", |s, asked| asked == 2 && s.diffs.files().is_empty() && !s.diffs.is_stale);
    assert_eq!(live.seen.lock().unwrap().asked.len(), 2);
}

// ─── the panel opening by itself, and the Changes count ───────────────────

#[test]
fn the_panel_stays_folded_for_a_session_with_nothing_to_show_and_opens_on_its_first_change() {
    let mut live = Live::start_answering(crate::mock::diffs::unchanged().to_string());
    live.wait("the empty list", |s, _| s.diffs.has_loaded);
    live.rt.frame(1.0 / 60.0);
    assert!(!live.rt.state.content_open, "nothing to show: the panel is folded");
    live.seen.lock().unwrap().answer = crate::mock::diffs::changed().to_string();
    live.files_changed();
    live.wait("the change", |s, _| s.diffs.files().len() == 4);
    live.wait("its first change opens the panel", |s, _| s.content_open);
    live.rt.update(|s, _| s.set_content_open(false));
    live.rt.frame(1.0 / 60.0);
    assert!(!live.rt.state.content_open, "the user's choice is kept, though the session still has changes");
}

#[test]
fn a_session_that_already_had_changes_when_the_app_loaded_is_not_opened() {
    let mut live = Live::start();
    live.rt.update(|s, _| s.sidebar.set_right_collapsed(None, true));
    live.wait("the list", |s, _| s.diffs.has_loaded);
    live.rt.frame(1.0 / 60.0);
    assert_eq!(live.rt.state.diffs.files().len(), 4);
    assert!(!live.rt.state.content_open, "it was never seen empty, so there is no first change");
}

#[test]
fn the_context_tab_is_added_without_focus_the_first_time_an_automation_started_the_session() {
    let mut live = Live::start();
    live.wait("the list", |s, _| s.diffs.has_loaded);
    let id = live.id.clone();
    assert!(!live.rt.state.open_canvases().ids().contains(&"context"));
    live.rt.update(move |s, _| {
        let found = s.projects.iter_mut().flat_map(|p| p.sessions.iter_mut()).find(|x| x.id == id).unwrap();
        found.row.origin = Some(super::super::rows::Origin {
            source_type: "automation".into(),
            title: Some("Nightly".into()),
            resource_url: None,
            resource_id: None,
            provider_id: String::new(),
        });
    });
    live.rt.frame(1.0 / 60.0);
    let tabs = live.rt.state.open_canvases();
    assert_eq!(tabs.ids().first().copied(), Some("context"), "added first");
    assert_eq!(tabs.active_id, "changes", "without taking focus");
    live.rt.update(|s, _| {
        s.edit_canvases(|c, id| c.close(id, "context"));
    });
    live.rt.frame(1.0 / 60.0);
    assert!(!live.rt.state.open_canvases().ids().contains(&"context"), "closed once, it stays closed");
}

// SessionsV2RightPanel.test.ts. Native has one `State.diffs` however many parts want it, so the first and third cases say
// the same thing: one request per session open, whoever asks.
#[test]
fn does_not_run_its_own_diffs_instance_when_the_sessions_route_already_holds_one() {
    let mut live = Live::start();
    live.wait("the list", |s, _| s.diffs.has_loaded);
    let id = live.id.clone();
    // The panel asking too, as it does on every open, joins the list that is loaded and fresh.
    for _ in 0..3 {
        live.rt.update(|s, tx| s.after_open(tx));
        live.rt.frame(1.0 / 60.0);
    }
    assert_eq!(live.seen.lock().unwrap().asked, [format!("/api/sessions/{id}/diffs")]);
}

#[test]
fn runs_its_own_diffs_instance_when_no_route_holds_one() {
    let mut live = Live::start();
    live.wait("the panel's own request", |s, asked| asked == 1 && s.diffs.has_loaded);
    assert_eq!(live.rt.state.diffs.session(), live.id);
    assert_eq!(live.seen.lock().unwrap().asked.len(), 1);
}

#[test]
fn doesnt_take_a_failed_diffs_for_an_empty_session_so_the_panel_isnt_shut_over_the_error() {
    let mut d = opened("s");
    let t = d.begin(false).unwrap();
    d.finish(&t, Ok((500, json!({}))));
    assert!(d.has_loaded && d.error.is_some() && d.files().is_empty());
    let defaults = crate::fleet::canvases::Canvases::default().session("s").canvases;
    let content = crate::fleet::canvases::PanelContent {
        changed_files: d.files().len(),
        canvases: &defaults,
        changes_failed: d.error.is_some(),
        ..Default::default()
    };
    assert!(crate::fleet::canvases::panel_has_content(content), "the panel opens on the error and Try again");
}
