//! Invented progress for the Progress tab's scenes and tests (component 10, piece G): a plan in groups, a flat plan, a plan
//! with every step ticked, a todo list with no plan, and nothing at all.
use crate::State;
use crate::fleet::canvases::CanvasKind;
use crate::fleet::progress::{Plan, PlanGroup, PlanStep, ProgressDetail, TodoItem, TodoPriority, TodoStatus};
use crate::fleet::row_chips::Progress;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Case {
    /// Two groups, the first ticked and folded, a step with a sub-checklist, and the live todos under the current step.
    Plan,
    /// One group: no heads, a segment per step.
    Flat,
    /// Every step ticked.
    Done,
    /// A todo list and no plan.
    Todos,
    /// Nothing to track.
    Empty,
}

fn step(n: usize, title: &str, checked: bool, sub: (u32, u32)) -> PlanStep {
    PlanStep {
        key: n.to_string(),
        number: Some(n.to_string()),
        title: title.into(),
        checked,
        sub_done: sub.0,
        sub_total: sub.1,
        ticked_at: checked.then(|| format!("2026-11-17T09:{:02}:00Z", 10 + n * 4)),
        ticked_in_message_id: None,
    }
}

fn todo(content: &str, status: TodoStatus) -> TodoItem {
    TodoItem { content: content.into(), status, priority: TodoPriority::Medium }
}

pub fn todos() -> Vec<TodoItem> {
    vec![
        todo("Add `limit` and `cursor` to the query", TodoStatus::Completed),
        todo("Read the cursor in the list handler", TodoStatus::InProgress),
        todo("Return the next cursor with the page", TodoStatus::Pending),
        todo("Keep the old unpaged route", TodoStatus::Cancelled),
    ]
}

fn plan(ticked: usize, grouped: bool) -> Plan {
    let s = |n: usize, title: &str, sub: (u32, u32)| step(n, title, n <= ticked, sub);
    let first = vec![s(1, "Add `limit` and `cursor` to the query", (0, 0)), s(2, "Return the next cursor", (0, 0))];
    let rest = vec![
        s(3, "Read `nextCursor` in the list", (1, 3)),
        s(4, "Load the next page on scroll", (0, 0)),
        s(5, "Show a spinner at the end", (0, 0)),
    ];
    let groups = if grouped {
        vec![PlanGroup { title: Some("Server".into()), steps: first }, PlanGroup { title: Some("Client".into()), steps: rest }]
    } else {
        vec![PlanGroup { title: None, steps: first.into_iter().chain(rest).collect() }]
    };
    Plan {
        path: ".weave/plans/paging.md".into(),
        title: Some("Page the order list".into()),
        tracked_since: "2026-11-17T09:00:00Z".into(),
        groups,
    }
}

impl State {
    pub fn show_progress_tab(&mut self, case: Case, dark: bool) {
        self.show_context_tab_scene(crate::mock::context_tab::Case::Links, dark);
        self.sidebar_open = true;
        let id = self.canvas_session();
        let (detail, summary) = match case {
            Case::Plan | Case::Flat | Case::Done => {
                let ticked = match case {
                    Case::Done => 5,
                    _ => 2,
                };
                let plan = plan(ticked, case != Case::Flat);
                let current = plan.groups.iter().flat_map(|g| &g.steps).find(|s| !s.checked).map(|s| s.title.clone());
                let summary = Progress { kind: "plan".into(), done: ticked as u32, total: 5, current };
                let todos = if case == Case::Done { vec![] } else { todos() };
                (
                    Some(ProgressDetail {
                        session_id: id.clone(),
                        summary: summary.clone(),
                        todos,
                        updated_at: "2026-11-17T09:31:00Z".into(),
                        plan: Some(plan),
                        subagents: vec![],
                    }),
                    Some(summary),
                )
            }
            Case::Todos => {
                let summary =
                    Progress { kind: "todos".into(), done: 1, total: 3, current: Some("Read the cursor in the list handler".into()) };
                (
                    Some(ProgressDetail {
                        session_id: id.clone(),
                        summary: summary.clone(),
                        todos: todos(),
                        updated_at: "2026-11-17T09:31:00Z".into(),
                        plan: None,
                        subagents: vec![],
                    }),
                    Some(summary),
                )
            }
            Case::Empty => (None, None),
        };
        self.projects[0].sessions[0].row.progress = summary;
        self.progress.set(&id, detail);
        self.edit_canvases(|c, id| c.open(id, CanvasKind::Progress));
    }
}
