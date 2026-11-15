//! `lib/session-project-groups.ts`, `lib/session-pins.ts` and `composables/use-projects.ts`, ported under Fleet's own
//! test names where Fleet has them, and with cases of their own for the grouping and the filter, which Fleet has no
//! unit test for.
use crate::fleet::groups::*;
use crate::fleet::sessions;
use serde_json::{Value, json};

#[derive(Clone, Debug, Default)]
struct Item {
    id: String,
    title: String,
    status: String,
    listing: Listing,
}

impl Listed for Item {
    fn id(&self) -> &str {
        &self.id
    }
    fn title(&self) -> &str {
        &self.title
    }
    fn status_word(&self) -> &str {
        &self.status
    }
    fn listing(&self) -> &Listing {
        &self.listing
    }
}

fn item(id: &str) -> Item {
    Item { id: id.into(), title: id.into(), status: "idle".into(), listing: Listing::default() }
}

fn pinned(id: &str, order: f64) -> Item {
    let mut i = item(id);
    i.listing.pin_order = Some(order);
    i
}

fn fork_of(id: &str, parent: &str) -> Item {
    let mut i = item(id);
    i.listing.forked_from = Some(parent.into());
    i.listing.spawn_kind = Some("fork".into());
    i
}

fn ids<T: Listed>(items: &[T]) -> Vec<&str> {
    items.iter().map(|i| i.id()).collect()
}

fn in_project(id: &str, project: Option<&str>, name: Option<&str>) -> Item {
    let mut i = item(id);
    i.listing.project_id = project.map(Into::into);
    i.listing.project_name = name.map(Into::into);
    i
}

fn project(id: &str, name: &str, kind: ProjectKind, position: i64) -> ProjectInfo {
    ProjectInfo { id: id.into(), name: name.into(), kind, position }
}

fn names<T>(groups: &[ProjectTreeGroup<T>]) -> Vec<&str> {
    groups.iter().map(|g| g.name.as_str()).collect()
}

// ─── lib/__tests__/session-pins.test.ts ─────────────────────────────────────

#[test]
fn split_pinned_puts_pinned_sessions_in_their_pinned_order_and_keeps_the_rest_in_the_lists_order() {
    let (pinned_group, rest) =
        split_pinned(vec![item("newest"), pinned("second", 2.0), item("middle"), pinned("first", 0.5), item("oldest")]);
    assert_eq!(ids(&pinned_group), ["first", "second"]);
    assert_eq!(ids(&rest), ["newest", "middle", "oldest"]);
}

#[test]
fn split_pinned_takes_what_came_from_a_pinned_session_with_it_however_deep() {
    let mut grandchild = item("grandchild");
    grandchild.listing.spawned_by = Some("child".into());
    grandchild.listing.spawn_kind = Some("api".into());
    let (pinned_group, rest) = split_pinned(vec![grandchild, fork_of("child", "parent"), pinned("parent", 1.0), item("other")]);
    assert_eq!(ids(&pinned_group), ["parent", "grandchild", "child"]);
    assert_eq!(ids(&rest), ["other"]);
}

#[test]
fn split_pinned_lets_a_pinned_session_stand_on_its_own_when_the_one_it_came_from_isnt_pinned() {
    let mut fork = fork_of("fork", "parent");
    fork.listing.pin_order = Some(1.0);
    let (pinned_group, rest) = split_pinned(vec![fork, item("parent")]);
    assert_eq!(ids(&pinned_group), ["fork"]);
    assert_eq!(ids(&rest), ["parent"]);
}

#[test]
fn split_pinned_leaves_a_session_moved_out_of_a_pinned_parent_where_it_is() {
    let mut fork = fork_of("fork", "parent");
    fork.listing.detached = true;
    let (pinned_group, rest) = split_pinned(vec![fork, pinned("parent", 1.0)]);
    assert_eq!(ids(&pinned_group), ["parent"]);
    assert_eq!(ids(&rest), ["fork"]);
}

#[test]
fn split_pinned_does_not_loop_on_a_lineage_cycle() {
    let (pinned_group, rest) = split_pinned(vec![fork_of("a", "b"), fork_of("b", "a")]);
    assert!(pinned_group.is_empty());
    assert_eq!(ids(&rest), ["a", "b"]);
}

fn pins() -> Vec<Item> {
    vec![pinned("a", 1.0), pinned("b", 2.0), pinned("c", 3.0), item("d")]
}

#[test]
fn pin_order_for_goes_at_the_end_when_theres_nothing_to_sit_before() {
    assert_eq!(pin_order_for(&pins(), "d", None), 4.0);
    assert_eq!(pin_order_for(&[item("d")], "d", None), 1.0);
    assert_eq!(pin_order_for(&pins(), "d", Some("not-pinned")), 4.0);
}

#[test]
fn pin_order_for_sits_halfway_between_the_pinned_session_before_it_and_the_one_it_goes_before() {
    assert_eq!(pin_order_for(&pins(), "d", Some("b")), 1.5);
    assert_eq!(pin_order_for(&pins(), "d", Some("a")), 0.5);
}

#[test]
fn pin_order_for_leaves_itself_out_when_its_already_pinned() {
    assert_eq!(pin_order_for(&pins(), "c", Some("b")), 1.5);
    assert_eq!(pin_order_for(&pins(), "a", None), 4.0);
}

#[test]
fn pinned_neighbour_for_says_which_pinned_session_it_goes_before_when_it_moves_up_or_down() {
    assert_eq!(pinned_neighbour_for(&pins(), "b", -1), Some(Some("a".into())));
    assert_eq!(pinned_neighbour_for(&pins(), "a", 1), Some(Some("c".into())));
    assert_eq!(pinned_neighbour_for(&pins(), "b", 1), Some(None));
}

/// `stores/__tests__/sidebar.test.ts` > pinnedNeighbourFor > can't move past either end, or move a session that isn't pinned
#[test]
fn pinned_neighbour_for_cant_move_past_either_end_or_move_a_session_that_isnt_pinned() {
    assert_eq!(pinned_neighbour_for(&pins(), "a", -1), None);
    assert_eq!(pinned_neighbour_for(&pins(), "c", 1), None);
    assert_eq!(pinned_neighbour_for(&pins(), "d", -1), None);
}

// ─── composables/__tests__/use-projects.test.ts ─────────────────────────────

fn alpha() -> ProjectInfo {
    project("project-1", "Alpha", ProjectKind::User, 1)
}

#[test]
fn use_projects_fetches_projects_immediately_when_enabled() {
    let mut load = ProjectsLoad::new(true);
    let request = load.begin().expect("enabled, so it fetches");
    assert!(load.is_loading);
    let two = vec![alpha(), project("project-2", "Beta", ProjectKind::User, 2)];
    load.finish(request, Ok(two.clone()));
    assert_eq!(load.projects, two);
    assert!(!load.is_loading && !load.is_refreshing);
    assert_eq!(load.error, None);
}

#[test]
fn use_projects_waits_for_enabled_to_become_true_before_fetching() {
    let mut load = ProjectsLoad::new(false);
    assert_eq!(load.begin(), None);
    assert!(load.projects.is_empty());
    assert!(load.set_enabled(true), "turning it on starts a fetch");
    let request = load.begin().unwrap();
    load.finish(request, Ok(vec![alpha()]));
    assert_eq!(load.projects, [alpha()]);
    assert!(!load.set_enabled(true), "already on: nothing new");
}

#[test]
fn use_projects_uses_refreshing_state_on_refetch_after_the_initial_load() {
    let mut load = ProjectsLoad::new(true);
    let first = load.begin().unwrap();
    load.finish(first, Ok(vec![alpha()]));
    let again = load.begin().unwrap();
    assert!(load.is_refreshing && !load.is_loading);
    load.finish(again, Ok(vec![project("project-1", "Updated", ProjectKind::User, 1)]));
    assert!(!load.is_refreshing);
    assert_eq!(load.projects[0].name, "Updated");
}

#[test]
fn use_projects_captures_http_failures_as_composable_errors() {
    let mut load = ProjectsLoad::new(true);
    let request = load.begin().unwrap();
    load.finish(request, Err("HTTP 500".into()));
    assert!(load.projects.is_empty());
    assert_eq!(load.error.as_deref(), Some("HTTP 500"));
    assert!(!load.is_loading);
}

#[test]
fn use_projects_drops_the_reply_to_a_request_a_newer_one_overtook() {
    let mut load = ProjectsLoad::new(true);
    let old = load.begin().unwrap();
    let new = load.begin().unwrap();
    load.finish(old, Ok(vec![alpha()]));
    assert!(load.projects.is_empty() && load.is_loading, "the old reply changes nothing");
    load.finish(new, Ok(vec![]));
    assert!(!load.is_loading);
}

#[test]
fn projects_are_read_from_the_servers_answer() {
    let body = json!([
        {"id": "a", "name": "Billing API", "type": "user", "position": 2, "sessionCount": 1},
        {"id": "b", "name": "Scratch", "type": "scratch", "position": 0},
        {"name": "no id"},
    ]);
    assert_eq!(
        parse_projects(&body),
        [project("a", "Billing API", ProjectKind::User, 2), project("b", "Scratch", ProjectKind::Scratch, 0)]
    );
    assert!(parse_projects(&json!({"error": "x"})).is_empty());
}

// ─── lib/session-project-groups.ts (no Fleet unit test; cases from reading it) ──

fn three() -> Vec<ProjectInfo> {
    vec![
        project("s", "Scratch", ProjectKind::Scratch, 0),
        project("docs", "Docs site", ProjectKind::User, 3),
        project("bill", "Billing API", ProjectKind::User, 2),
        project("empty", "Empty one", ProjectKind::User, 4),
    ]
}

#[test]
fn groups_come_in_the_projects_own_order_and_an_empty_project_still_shows() {
    let groups =
        build_project_groups(vec![in_project("1", Some("docs"), Some("Docs site")), in_project("2", Some("bill"), None)], &three(), None);
    assert_eq!(names(&groups), ["Billing API", "Docs site", "Empty one"]);
    assert_eq!(groups.iter().map(|g| g.session_count).collect::<Vec<_>>(), [1, 1, 0]);
    assert_eq!(ids(&groups[0].sessions), ["2"], "the name comes from the project when the row has none");
}

#[test]
fn projects_with_the_same_position_order_by_name() {
    let projects = vec![
        project("z", "zeta", ProjectKind::User, 1),
        project("b", "Beta", ProjectKind::User, 1),
        project("a", "alpha", ProjectKind::User, 1),
    ];
    assert_eq!(names(&build_project_groups::<Item>(vec![], &projects, None)), ["alpha", "Beta", "zeta"]);
}

#[test]
fn sessions_with_no_project_go_to_ungrouped_which_is_always_last() {
    let groups = build_project_groups(vec![item("1"), in_project("2", Some("docs"), None)], &three(), None);
    assert_eq!(names(&groups), ["Billing API", "Docs site", "Empty one", "Ungrouped"]);
    let last = groups.last().unwrap();
    assert!(last.is_ungrouped && last.id == UNGROUPED_ID && last.project_id.is_none());
    assert!(!last.can_move_up && !last.can_move_down);
}

#[test]
fn an_unknown_project_with_a_name_is_a_group_of_its_own_after_the_known_ones() {
    let groups = build_project_groups(vec![in_project("1", Some("gone"), Some("Old name"))], &three(), None);
    assert_eq!(names(&groups), ["Billing API", "Docs site", "Empty one", "Old name"]);
    assert_eq!(groups[3].id, "gone");
}

#[test]
fn scratch_shows_only_while_it_has_sessions_or_the_draft_will_land_in_it() {
    let none = build_project_groups::<Item>(vec![], &three(), None);
    assert!(!names(&none).contains(&"Scratch"));
    let with = build_project_groups(vec![in_project("1", Some("s"), Some("Scratch"))], &three(), None);
    assert_eq!(with[0].name, "Scratch");
    let draft = build_project_groups::<Item>(vec![], &three(), Some("s"));
    assert_eq!(draft[0].name, "Scratch");
    assert_eq!(draft[0].session_count, 0);
}

#[test]
fn the_draft_group_key_is_the_project_or_scratch_or_ungrouped() {
    let projects = three();
    assert_eq!(draft_group_key_for(Some("docs"), &projects), "docs");
    assert_eq!(draft_group_key_for(Some("gone"), &projects), "s");
    assert_eq!(draft_group_key_for(None, &projects), "s");
    assert_eq!(draft_group_key_for(None, &projects[1..]), "Ungrouped");
    assert_eq!(draft_group_id_for(Some("docs"), &projects).as_deref(), Some("docs"));
    assert_eq!(draft_group_id_for(Some("Ungrouped"), &projects).as_deref(), Some("ungrouped"));
    assert_eq!(draft_group_id_for(None, &projects), None);
}

#[test]
fn a_group_knows_where_a_move_up_or_down_puts_every_project() {
    let groups = build_project_groups::<Item>(vec![], &three(), None);
    let (bill, docs, empty) = (&groups[0], &groups[1], &groups[2]);
    assert!(!bill.can_move_up && bill.can_move_down);
    assert!(docs.can_move_up && docs.can_move_down);
    assert!(empty.can_move_up && !empty.can_move_down);
    let target = |t: &[ReorderTarget]| t.iter().map(|t| (t.project_id.clone(), t.position)).collect::<Vec<_>>();
    assert_eq!(target(&docs.move_up_targets), [("docs".into(), 1), ("bill".into(), 2), ("empty".into(), 3)]);
    assert_eq!(target(&docs.move_down_targets), [("bill".into(), 1), ("empty".into(), 2), ("docs".into(), 3)]);
    assert!(bill.move_up_targets.is_empty());
}

#[test]
fn the_pinned_group_never_moves() {
    let g = pinned_project_group(vec![item("a")]);
    assert_eq!((g.id.as_str(), g.name.as_str(), g.session_count), ("pinned", "Pinned", 1));
    assert!(!g.can_move_up && !g.can_move_down && g.move_up_targets.is_empty());
}

#[test]
fn the_filter_matches_title_id_project_and_status() {
    let projects = three();
    let mut a = in_project("id-77", Some("bill"), None);
    a.title = "Fix paging".into();
    a.status = "waiting_input".into();
    for (q, want) in [
        ("paging", true),
        ("id-77", true),
        ("billing", true),
        ("waiting_input", true),
        ("fix paging id-77", true),
        ("paging billing", false),
        ("nothing", false),
    ] {
        assert_eq!(session_matches_query(&a, q, &projects), want, "{q}");
    }
    assert!(session_matches_query(&item("x"), "ungrouped", &projects), "a session with no project reads as Ungrouped");
}

#[test]
fn a_project_whose_name_matches_keeps_all_its_sessions_the_rest_only_the_matches() {
    let projects = three();
    let mut hit = in_project("1", Some("docs"), None);
    hit.title = "billing report".into();
    let groups = build_project_groups(vec![hit, in_project("2", Some("docs"), None), in_project("3", Some("bill"), None)], &projects, None);
    let kept = filter_project_groups(groups, "docs", |s| session_matches_query(s, "docs", &projects));
    assert_eq!(names(&kept), ["Docs site"], "the others have no match and another name");
    assert_eq!(kept[0].session_count, 2);
    let groups = build_project_groups(vec![in_project("1", Some("docs"), None), in_project("2", Some("bill"), None)], &projects, None);
    let kept = filter_project_groups(groups, "empty", |s| session_matches_query(s, "empty", &projects));
    assert_eq!(names(&kept), ["Empty one"], "a project whose name matches stays with no sessions");
    let groups = build_project_groups(vec![in_project("abc", Some("docs"), None), in_project("xyz", Some("docs"), None)], &projects, None);
    let kept = filter_project_groups(groups, "xy", |s| session_matches_query(s, "xy", &projects));
    assert_eq!((kept.len(), kept[0].session_count, ids(&kept[0].sessions)), (1, 1, vec!["xyz"]));
}

// ─── the list the app draws ─────────────────────────────────────────────────

fn row(id: &str, project: &str, name: Option<&str>, updated: i64) -> Value {
    json!({"session": {"id": id, "title": id, "time": {"updated": updated}}, "projectId": project, "projectName": name,
        "activityStatus": "idle", "sessionStatus": "idle", "retentionStatus": "active"})
}

#[test]
fn the_list_is_in_the_projects_order_with_empty_projects_and_a_count_of_zero() {
    let rows = json!([row("a", "docs", None, 3), row("b", "s", Some("Scratch"), 2), row("c", "bill", Some("Billing API"), 1)]);
    let projects = sessions::parse_list_in(&rows, &three(), 0);
    let order: Vec<(&str, usize)> = projects.iter().map(|p| (p.name.as_str(), p.sessions.len())).collect();
    assert_eq!(order, [("Scratch", 1), ("Billing API", 1), ("Docs site", 1), ("Empty one", 0)]);
    assert_eq!(projects[0].kind, ProjectKind::Scratch);
    assert_eq!((projects[1].id.as_str(), projects[1].position), ("bill", 2));
}
