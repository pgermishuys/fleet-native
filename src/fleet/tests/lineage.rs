//! `lib/__tests__/session-lineage.test.ts` ("lineageOf", "moving a session out of its parent", "nestLineage",
//! "runningSubagentsBySession", "layoutLineage") under Fleet's names, and the rows of `ProjectGroup.vue` that follow from them
//! (`childRowsOf`, the indents). The Agents tab's cases (`buildAgentsLineage`, `summarizeAgentActivity`) are another component's.
use crate::fleet::groups::{LineageKind, Listed, Listing, lineage_of, lineage_origin_of};
use crate::fleet::lineage::*;
use crate::fleet::lineage_moves::{message_for, movable_back_under, movable_out_of};
use crate::fleet::work::{RunningWorkItem, WorkKind, WorkStatus, to_item};
use serde_json::json;

struct Item {
    id: String,
    listing: Listing,
}

impl Listed for Item {
    fn id(&self) -> &str {
        &self.id
    }
    fn title(&self) -> &str {
        ""
    }
    fn status_word(&self) -> &str {
        ""
    }
    fn listing(&self) -> &Listing {
        &self.listing
    }
}

fn item(id: &str, edit: impl FnOnce(&mut Listing)) -> Item {
    let mut listing = Listing::default();
    edit(&mut listing);
    Item { id: id.into(), listing }
}

fn fork(id: &str, of: &str) -> Item {
    item(id, |l| {
        l.forked_from = Some(of.into());
        l.spawn_kind = Some("fork".into());
    })
}

fn started(id: &str, by: &str) -> Item {
    item(id, |l| l.spawned_by = Some(by.into()))
}

fn work(id: &str, session: &str, child: Option<&str>, status: WorkStatus) -> RunningWorkItem {
    let mut w = to_item(&json!({ "id": id, "sessionId": session, "kind": "subagent", "title": "general" })).unwrap();
    w.child_session_id = child.map(str::to_string);
    w.status = status;
    w
}

fn names(items: &[Item], rows: &[usize]) -> Vec<String> {
    rows.iter().map(|&n| items[n].id.clone()).collect()
}

#[test]
fn lineage_of_reads_a_fork_a_started_session_and_a_subagents_hidden_session() {
    assert_eq!(lineage_of(&fork("f", "a").listing), Some(("a".into(), LineageKind::Fork)));
    assert_eq!(lineage_of(&started("s", "a").listing), Some(("a".into(), LineageKind::Started)));
    let sub = item("c", |l| l.parent = Some("a".into()));
    assert_eq!(lineage_of(&sub.listing), Some(("a".into(), LineageKind::Subagent)));
    assert_eq!(lineage_of(&item("x", |_| {}).listing), None);
}

#[test]
fn lineage_of_lets_spawn_kind_decide_when_a_session_is_both_a_fork_and_started() {
    let both = |kind: &str| {
        item("b", |l| {
            l.forked_from = Some("fork-parent".into());
            l.spawned_by = Some("start-parent".into());
            l.spawn_kind = Some(kind.into());
        })
    };
    assert_eq!(lineage_of(&both("fork").listing), Some(("fork-parent".into(), LineageKind::Fork)));
    assert_eq!(lineage_of(&both("api").listing), Some(("start-parent".into(), LineageKind::Started)));
}

#[test]
fn moving_a_session_out_gives_a_session_the_user_moved_out_no_parent_though_it_keeps_where_it_came_from() {
    let mut out = fork("f", "a");
    out.listing.detached = true;
    assert_eq!(lineage_of(&out.listing), None);
    assert_eq!(lineage_origin_of(&out.listing), Some(("a".into(), LineageKind::Fork)));
    // A sub-agent's session cannot be detached.
    let sub = item("c", |l| {
        l.parent = Some("a".into());
        l.detached = true;
    });
    assert_eq!(lineage_of(&sub.listing), Some(("a".into(), LineageKind::Subagent)));
}

#[test]
fn moving_a_session_out_moves_forks_and_started_sessions_out_never_a_subagents_session() {
    assert_eq!(movable_out_of(&fork("f", "a").listing).as_deref(), Some("a"));
    assert_eq!(movable_out_of(&started("s", "a").listing).as_deref(), Some("a"));
    assert_eq!(movable_out_of(&item("c", |l| l.parent = Some("a".into())).listing), None);
    assert_eq!(movable_out_of(&item("x", |_| {}).listing), None, "one the user started");
    let mut out = fork("f", "a");
    out.listing.detached = true;
    assert_eq!(movable_out_of(&out.listing), None);
    assert_eq!(movable_back_under(&out.listing).as_deref(), Some("a"));
    assert_eq!(movable_back_under(&fork("f", "a").listing), None);
}

#[test]
fn moving_a_session_out_puts_it_at_the_top_of_the_list() {
    let mut f = fork("f", "a");
    let items = vec![item("a", |_| {}), Item { id: "f".into(), listing: f.listing.clone() }];
    assert_eq!(nest_lineage(&items).roots, vec![0]);
    f.listing.detached = true;
    let items = vec![item("a", |_| {}), f];
    assert_eq!(nest_lineage(&items).roots, vec![0, 1], "it stands on its own");
}

#[test]
fn nest_lineage_nests_forks_and_started_sessions_under_their_parent_in_list_order() {
    let items = vec![item("a", |_| {}), fork("f1", "a"), started("s1", "a"), item("b", |_| {})];
    let n = nest_lineage(&items);
    assert_eq!(names(&items, &n.roots), ["a", "b"]);
    let kids: Vec<usize> = n.children[&0].iter().map(|(i, _)| *i).collect();
    assert_eq!(names(&items, &kids), ["f1", "s1"]);
    assert_eq!(n.children[&0][0].1, LineageKind::Fork);
    assert_eq!(n.children[&0][1].1, LineageKind::Started);
}

#[test]
fn nest_lineage_keeps_a_child_at_the_top_when_its_parent_isnt_in_the_list() {
    let items = vec![fork("f", "gone"), item("a", |_| {})];
    assert_eq!(nest_lineage(&items).roots, vec![0, 1]);
}

#[test]
fn nest_lineage_nests_a_fork_of_a_started_session_under_that_session_and_lists_descendants_depth_first() {
    let items = vec![item("a", |_| {}), started("s", "a"), fork("f", "s"), started("t", "a")];
    let n = nest_lineage(&items);
    assert_eq!(n.roots, vec![0]);
    let found = descendants(0, &n);
    let seen: Vec<(String, usize)> = found.iter().map(|d| (items[d.index].id.clone(), d.depth)).collect();
    assert_eq!(seen, [("s".to_string(), 1), ("f".to_string(), 2), ("t".to_string(), 1)]);
}

#[test]
fn nest_lineage_keeps_every_session_in_a_loop_of_parents_at_the_top() {
    let items = vec![fork("a", "b"), fork("b", "a"), item("c", |_| {})];
    assert_eq!(nest_lineage(&items).roots, vec![0, 1, 2]);
    // A session that only hangs from a loop is not in it: it nests.
    let items = vec![fork("a", "b"), fork("b", "a"), fork("d", "a")];
    let n = nest_lineage(&items);
    assert_eq!(n.roots, vec![0, 1]);
    assert_eq!(n.children[&0], vec![(2, LineageKind::Fork)]);
}

#[test]
fn running_subagents_by_session_keeps_running_subagents_that_have_a_session_by_the_session_that_started_them() {
    let all = vec![
        work("w1", "p", Some("c1"), WorkStatus::Running),
        work("w2", "p", Some("c2"), WorkStatus::Completed),
        work("w3", "p", None, WorkStatus::Running),
        work("w4", "q", Some("c4"), WorkStatus::Running),
        { RunningWorkItem { kind: WorkKind::Shell, ..work("w5", "p", Some("c5"), WorkStatus::Running) } },
    ];
    let by = running_subagents(&all);
    assert_eq!(by["p"].iter().map(|w| w.id.as_str()).collect::<Vec<_>>(), ["w1"]);
    assert_eq!(by["q"].len(), 1);
    assert_eq!(by.len(), 2);
}

#[test]
fn what_shows_under_a_session_is_its_running_subagents_first_then_each_child_with_its_own_subagents() {
    let items = vec![item("a", |_| {}), fork("f", "a")];
    let n = nest_lineage(&items);
    let by = running_subagents(&[
        work("w1", "a", Some("c1"), WorkStatus::Running),
        work("w2", "c1", Some("c2"), WorkStatus::Running),
        work("w3", "f", Some("c3"), WorkStatus::Running),
    ]);
    let rows = child_rows(&items, 0, &n, &by);
    let shape: Vec<String> = rows
        .iter()
        .map(|r| match r {
            ChildRow::Subagent { work, depth } => format!("sub {} {depth}", work.id),
            ChildRow::Session { index, depth, .. } => format!("session {} {depth}", items[*index].id),
        })
        .collect();
    assert_eq!(shape, ["sub w1 1", "sub w2 2", "session f 1", "sub w3 2"]);
    // A sub-agent that is its own parent doesn't loop.
    let by = running_subagents(&[work("w1", "a", Some("a"), WorkStatus::Running)]);
    assert_eq!(child_rows(&items, 0, &n, &by).len(), 2);
}

#[test]
fn layout_lineage_stops_a_line_at_the_middle_of_its_parents_last_child_and_runs_it_past_the_earlier_ones() {
    let lines = layout_lineage(&[1, 1, 1]);
    assert_eq!(lines.iter().map(|l| l.last).collect::<Vec<_>>(), [false, false, true]);
    assert!(lines.iter().all(|l| l.through.is_empty()));
}

#[test]
fn layout_lineage_keeps_an_ancestors_line_running_past_its_descendants_while_it_has_later_children() {
    let lines = layout_lineage(&[1, 2, 2, 1]);
    assert_eq!(lines[1].through, vec![1], "the first line runs past the grandchildren");
    assert_eq!(lines[2].through, vec![1]);
    assert!(lines[1].depth == 2 && !lines[1].last && lines[2].last);
    assert!(lines[3].last && lines[3].through.is_empty());
    let lines = layout_lineage(&[1, 2]);
    assert!(lines[1].through.is_empty(), "no later child of the first: its line ends");
}

#[test]
fn layout_lineage_draws_rows_deeper_than_the_cap_at_the_caps_indent_and_line() {
    let lines = layout_lineage(&[1, 2, 3, 4, 5]);
    assert_eq!(lines.iter().map(|l| l.depth).collect::<Vec<_>>(), [1, 2, 3, 3, 3]);
    assert_eq!((indent(1), indent(2), indent(3), indent(4), indent(9)), (25.0, 41.0, 57.0, 57.0, 57.0));
    assert_eq!((line_x(1), line_x(2), line_x(3)), (13.0, 29.0, 45.0));
}

#[test]
fn the_kind_labels_and_the_agent_state_are_fleets() {
    assert_eq!([LineageKind::Subagent, LineageKind::Fork, LineageKind::Started].map(kind_label), ["subagent", "fork", "started"]);
    assert_eq!(agent_state("idle", "waiting_input"), AgentState::Waiting);
    assert_eq!(agent_state("waiting_input", "idle"), AgentState::Waiting);
    assert_eq!(agent_state("active", "busy"), AgentState::Running);
    assert_eq!(agent_state("error", ""), AgentState::Failed);
    assert_eq!(agent_state("stopped", "idle"), AgentState::Idle);
}

#[test]
fn the_move_toast_names_the_parent_or_says_its_parent() {
    assert_eq!(message_for("Fork", Some("Main"), true), "Moved \"Fork\" out of \"Main\"");
    assert_eq!(message_for("Fork", None, true), "Moved \"Fork\" out of its parent");
    assert_eq!(message_for("Fork", Some("Main"), false), "Moved \"Fork\" back under \"Main\"");
    assert_eq!(message_for("Fork", Some("  "), false), "Moved \"Fork\" back under its parent");
}
