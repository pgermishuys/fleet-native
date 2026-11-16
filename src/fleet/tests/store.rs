//! The sessions store (`stores/__tests__/sessions.test.ts`), the list request (`composables/__tests__/use-sessions.test.ts`)
//! and the open session's route, under Fleet's names. Fleet's cases hold for the store because the same functions run.
use super::state::app;
use crate::fleet::groups::ProjectInfo;
use crate::fleet::sessions::{group_sessions, parse_sessions};
use crate::fleet::store::{self, ELSEWHERE_KEPT, Retention, Route, SessionStore};
use crate::mock::{Project, Session};
use serde_json::{Value, json};

fn item(id: &str, title: &str) -> Value {
    json!({
        "session": {"id": id, "title": title, "time": {"created": 1, "updated": 2}},
        "sessionStatus": "active", "activityStatus": "busy", "retentionStatus": "active", "isHidden": false,
        "workspaceDirectory": "/work/project", "projectId": "project-1", "projectName": "Api", "branch": "main",
    })
}

fn sessions_of(rows: Vec<Value>) -> Vec<Session> {
    parse_sessions(&Value::Array(rows), 1_000_000)
}

/// One group holding `rows`, as `setSessions` would leave the store.
fn store_of(rows: Vec<Value>) -> Vec<Project> {
    let infos = [ProjectInfo { id: "project-1".into(), name: "Api".into(), position: 1, ..Default::default() }];
    group_sessions(sessions_of(rows), &infos)
}

fn ids(projects: &[Project]) -> Vec<String> {
    store::rows(projects).map(|s| s.id.clone()).collect()
}

#[test]
fn use_sessions_store_stores_session_list_items_and_the_active_selection() {
    let mut route = Route::None;
    let projects = store_of(vec![item("session-1", "Migration")]);
    assert_eq!(route, Route::None);
    route = Route::Session("session-1".into());
    assert_eq!(projects[0].sessions.len(), 1);
    assert_eq!(projects[0].sessions[0].title, "Migration");
    assert_eq!(route, Route::Session("session-1".into()));
}

#[test]
fn use_sessions_store_puts_a_session_it_hasnt_seen_at_the_top_where_the_newest_first_list_will_have_it() {
    let mut projects = store_of(vec![item("session-1", "Migration")]);
    let created = sessions_of(vec![item("session-2", "Just created")]).remove(0);
    store::upsert(&mut projects, created);
    assert_eq!(ids(&projects), ["session-2", "session-1"]);
}

#[test]
fn use_sessions_store_moves_a_workflow_runs_steps_to_a_project_together_and_nothing_else() {
    let step = |id: &str, run: Option<&str>| {
        let mut row = item(id, id);
        row["projectId"] = json!("scratch");
        row["projectName"] = json!("Scratch");
        row["workflowRunId"] = json!(run);
        row
    };
    let mut projects =
        store_of(vec![step("plan", Some("run-1")), step("implement", Some("run-1")), step("other", Some("run-2")), step("plain", None)]);
    store::patch_project(&mut projects, "implement", Some("project-2"), Some("Site"));
    let placed: Vec<(String, Option<String>, Option<String>)> =
        store::rows(&projects).map(|s| (s.id.clone(), s.listing.project_id.clone(), s.listing.project_name.clone())).collect();
    let at = |id: &str, p: &str, n: &str| (id.to_string(), Some(p.to_string()), Some(n.to_string()));
    assert_eq!(
        placed,
        [
            at("plan", "project-2", "Site"),
            at("implement", "project-2", "Site"),
            at("other", "scratch", "Scratch"),
            at("plain", "scratch", "Scratch")
        ]
    );
}

#[test]
fn use_sessions_store_updates_a_known_session_in_place() {
    let mut projects = store_of(vec![item("session-1", "Migration"), item("session-2", "Second")]);
    // The known session's conversation is not in the list row; upserting it must keep what is held.
    projects[0].sessions[1].messages.push(crate::view::conversation::Message::user("u", "kept"));
    let renamed = sessions_of(vec![item("session-2", "Renamed")]).remove(0);
    store::upsert(&mut projects, renamed);
    assert_eq!(ids(&projects), ["session-1", "session-2"]);
    assert_eq!(projects[0].sessions[1].title, "Renamed");
    assert_eq!(projects[0].sessions[1].messages.len(), 1);
}

fn on_mini(id: &str, title: &str) -> Session {
    sessions_of(vec![item(id, title)]).remove(0)
}

#[test]
fn sessions_on_other_machines_opened_here_finds_them_by_id_without_adding_them_to_the_live_machines_list() {
    let projects = store_of(vec![item("session-1", "Migration")]);
    let mut store = SessionStore::default();
    store.upsert_elsewhere("mini-id", on_mini("mini-1", "Hero image sizes"));
    assert_eq!(ids(&projects), ["session-1"]);
    assert_eq!(store.session_by_id(&projects, "mini-1").unwrap().title, "Hero image sizes");
    assert_eq!(store.session_by_id(&projects, "session-1").unwrap().title, "Migration");
    assert!(store.session_by_id(&projects, "nowhere").is_none());
    assert_eq!(store.machine_of("mini-1"), Some("mini-id"));
}

#[test]
fn sessions_on_other_machines_opened_here_patches_and_removes_them_like_the_live_machines() {
    let mut projects = store_of(vec![]);
    let mut route = Route::Session("mini-1".into());
    let mut store = SessionStore::default();
    store.upsert_elsewhere("mini-id", on_mini("mini-1", "Hero image sizes"));
    store.patch(&mut projects, "mini-1", |s| s.row.activity = "idle".into());
    assert_eq!(store.session_by_id(&projects, "mini-1").unwrap().row.activity, "idle");
    store.patch(&mut projects, "unknown", |_| panic!("an unknown id does nothing"));
    assert!(store.remove(&mut projects, &mut route, "mini-1"));
    assert!(store.session_by_id(&projects, "mini-1").is_none());
    assert_eq!(route, Route::None, "removing the open session clears the route");
}

#[test]
fn sessions_on_other_machines_opened_here_keeps_the_20_opened_most_recently() {
    let projects = store_of(vec![]);
    let mut store = SessionStore::default();
    for n in 0..22 {
        store.upsert_elsewhere("mini-id", on_mini(&format!("mini-{n}"), "Hero image sizes"));
    }
    // Opening one again makes it the newest.
    store.upsert_elsewhere("mini-id", on_mini("mini-2", "Opened again"));
    assert_eq!(store.elsewhere_ids().len(), ELSEWHERE_KEPT);
    assert!(store.session_by_id(&projects, "mini-0").is_none());
    assert!(store.session_by_id(&projects, "mini-1").is_none());
    assert_eq!(store.session_by_id(&projects, "mini-2").unwrap().title, "Opened again");
    assert!(store.session_by_id(&projects, "mini-3").is_some());
}

#[test]
fn sessions_on_other_machines_opened_here_forgets_a_machines_sessions_when_the_machine_is_forgotten() {
    let mut store = SessionStore::default();
    store.upsert_elsewhere("mini-id", on_mini("mini-1", "Hero image sizes"));
    store.upsert_elsewhere("lab-id", on_mini("lab-1", "Index for slow search"));
    store.forget_elsewhere("mini-id");
    assert_eq!(store.elsewhere_ids(), ["lab-1"]);
}

#[test]
fn removing_a_live_session_clears_the_route_only_when_it_was_that_one() {
    let mut projects = store_of(vec![item("a", "A"), item("b", "B")]);
    let mut store = SessionStore::default();
    let mut route = Route::Session("a".into());
    assert!(store.remove(&mut projects, &mut route, "b"));
    assert_eq!(route, Route::Session("a".into()));
    assert!(!store.remove(&mut projects, &mut route, "b"), "a session already gone is not found");
    assert!(store.remove(&mut projects, &mut route, "a"));
    assert_eq!(route, Route::None);
}

// ─── useSessions ─────────────────────────────────────────────────────────────

#[test]
fn use_sessions_fetches_sessions_with_pagination_and_retention_filters() {
    assert_eq!(store::list_path(Retention::Active), "/api/sessions?limit=100&offset=0");
    assert_eq!(store::list_path(Retention::Archived), "/api/sessions?limit=100&offset=0&retentionStatus=archived");
    assert_eq!(store::list_path(Retention::All), "/api/sessions?limit=100&offset=0&retentionStatus=all");
}

#[test]
fn use_sessions_stores_fetch_failures_as_errors() {
    let mut store = SessionStore::default();
    let request = store.begin(false);
    assert!(store.is_loading && !store.is_refreshing);
    assert!(store.finish(request, Err(store::error_text(503, r#"{"error":"boom"}"#))));
    assert_eq!(store.error.as_deref(), Some("boom"));
    assert!(!store.is_loading && !store.is_refreshing);
    assert!(!store.list_loaded, "a failed fetch is not a list");
}

#[test]
fn use_sessions_shows_the_loader_only_for_the_first_fetch_with_nothing_held() {
    let mut store = SessionStore::default();
    let first = store.begin(false);
    assert!(store.is_loading && !store.is_refreshing);
    store.finish(first, Ok(()));
    assert!(store.list_loaded);
    let again = store.begin(true);
    assert!(!store.is_loading && store.is_refreshing);
    store.finish(again, Ok(()));
    // A fetch that failed once still counts as loaded: the next one refreshes.
    let mut failed = SessionStore::default();
    let r = failed.begin(false);
    failed.finish(r, Err("x".into()));
    failed.begin(false);
    assert!(!failed.is_loading && failed.is_refreshing);
}

#[test]
fn use_sessions_drops_an_answer_a_newer_request_has_overtaken() {
    let mut store = SessionStore::default();
    let old = store.begin(false);
    let newer = store.begin(false);
    assert!(!store.finish(old, Err("late".into())), "the stale answer is dropped");
    assert!(store.error.is_none() && store.is_loading, "and changes nothing");
    assert!(store.finish(newer, Ok(())));
    assert!(!store.is_loading);
}

#[test]
fn use_sessions_reads_the_error_from_the_response() {
    assert_eq!(store::error_text(500, r#"{"error":"first","detail":"second","title":"third"}"#), "first");
    assert_eq!(store::error_text(500, r#"{"error":"  ","detail":"second"}"#), "second");
    assert_eq!(store::error_text(500, r#"{"title":"third"}"#), "third");
    assert_eq!(store::error_text(502, "Bad gateway"), "Bad gateway");
    assert_eq!(store::error_text(503, ""), "HTTP 503");
    assert_eq!(store::error_text(503, r#"{"other":1}"#), r#"{"other":1}"#);
}

// ─── the route ───────────────────────────────────────────────────────────────

fn rows_for(order: &[(&str, &str)]) -> Vec<Value> {
    order
        .iter()
        .map(|(id, project)| {
            let mut row = item(id, &format!("Session {id}"));
            row["projectId"] = json!(project);
            row["projectName"] = json!(project);
            row
        })
        .collect()
}

fn load(rt: &mut rune::Runtime<crate::State>, request: u64, rows: Vec<Value>) {
    rt.update(move |s, tx| s.on_sessions(request, sessions_of(rows), Ok(vec![]), tx));
}

fn begin(rt: &mut rune::Runtime<crate::State>) -> u64 {
    let mut request = 0;
    rt.update(|s, _| request = s.store.begin(s.has_rows()));
    request
}

#[test]
fn a_refetch_with_a_changed_order_leaves_the_same_session_open() {
    let mut rt = app();
    let r = begin(&mut rt);
    load(&mut rt, r, rows_for(&[("s1", "alpha"), ("s2", "alpha"), ("s3", "beta")]));
    rt.update(|s, tx| {
        let (p, i) = s.projects.iter().enumerate().find_map(|(p, g)| g.sessions.iter().position(|x| x.id == "s2").map(|i| (p, i))).unwrap();
        s.open_session(p, i, tx);
    });
    assert_eq!(rt.state.route, Route::Session("s2".into()));
    let before = rt.state.open_at();
    // The server now lists the projects' sessions in another order and s2 has moved to the other project.
    let r = begin(&mut rt);
    load(&mut rt, r, rows_for(&[("s3", "beta"), ("s2", "beta"), ("s1", "alpha")]));
    assert_eq!(rt.state.route, Route::Session("s2".into()), "the route names the session, not a place");
    assert_eq!(rt.state.session().id, "s2");
    assert_ne!(rt.state.open_at(), before, "the session moved in the list");
}

#[test]
fn a_list_without_the_open_session_goes_home_with_nothing_selected() {
    let mut rt = app();
    let r = begin(&mut rt);
    load(&mut rt, r, rows_for(&[("s1", "alpha"), ("s2", "alpha")]));
    rt.update(|s, tx| s.open_session(0, 1, tx));
    let r = begin(&mut rt);
    load(&mut rt, r, rows_for(&[("s1", "alpha")]));
    assert_eq!(rt.state.route, Route::Home, "the dashboard, not the first session (component 9, piece C)");
}

#[test]
fn a_stale_list_answer_changes_nothing() {
    let mut rt = app();
    let old = begin(&mut rt);
    let newer = begin(&mut rt);
    load(&mut rt, newer, rows_for(&[("s1", "alpha")]));
    load(&mut rt, old, rows_for(&[("zz", "alpha"), ("s1", "alpha")]));
    assert_eq!(rt.state.projects.iter().map(|p| p.sessions.len()).sum::<usize>(), 1);
    rt.update(move |s, _| s.on_list_failed(old, "late".into()));
    assert!(s_error(&rt).is_none(), "a stale failure sets no error");
}

fn s_error(rt: &rune::Runtime<crate::State>) -> Option<String> {
    rt.state.store.error.clone()
}

#[test]
fn a_poll_that_says_the_same_thing_causes_no_rebuild_but_the_ages_move_on() {
    let mut rt = app();
    let r = begin(&mut rt);
    load(&mut rt, r, rows_for(&[("s1", "alpha"), ("s2", "alpha")]));
    rt.update(|s, _| s.projects[0].sessions[0].messages.push(crate::view::conversation::Message::user("u", "x")));
    rt.update(|s, _| s.projects[0].open = false);
    let r = begin(&mut rt);
    load(&mut rt, r, rows_for(&[("s1", "alpha"), ("s2", "alpha")]));
    assert_eq!(rt.state.projects[0].sessions[0].messages.len(), 1);
    assert!(!rt.state.projects[0].open, "the fold state is untouched");
    assert_eq!(rt.state.store.shown, ["s1", "s2"]);
    // A new title is a change.
    let mut rows = rows_for(&[("s1", "alpha"), ("s2", "alpha")]);
    rows[0]["session"]["title"] = json!("Renamed");
    let r = begin(&mut rt);
    load(&mut rt, r, rows);
    assert_eq!(rt.state.projects[0].sessions[0].title, "Renamed");
}

#[test]
fn the_invented_sessions_have_ids_of_their_own_and_none_goes_to_a_server() {
    let rt = app();
    let id = match &rt.state.route {
        Route::Session(id) => id.clone(),
        Route::None | Route::Home | Route::NewSession { .. } | Route::Welcome | Route::Repositories => {
            panic!("the first invented session is open")
        }
    };
    assert_eq!(id, store::invented_id(0, 0));
    assert!(store::is_invented(&id));
    assert!(rt.state.open_id().is_none());
    assert_eq!(rt.state.draft_owner().as_deref(), Some("mock.0.0"));
}
