//! Invented links for the Context tab's scenes and tests (component 8, piece I): every state of the tab the scratch Fleet
//! can't make (it has no GitHub), shaped as the server sends them (`mock::links`).
use super::context::{SESSION, failing, issue, popover_pr, run, thread};
use super::links::pull_request;
use crate::State;
use crate::fleet::rows::Origin;
use serde_json::{Value, json};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Case {
    /// No links: "Nothing attached yet".
    Empty,
    /// The first load hasn't answered.
    Loading,
    /// The load failed with HTTP 500.
    Error,
    /// One of each section: the origin issue, this session's pull request, pinned and mentioned links.
    Links,
    /// The attach form, with a link typed.
    Attach,
    /// The attach form after the server said the link isn't a pull request or issue.
    AttachError,
    /// GitHub isn't connected: every link says so, and the notice asks once (two pinned links, as a scratch Fleet makes them).
    NotConnected,
    /// A header chip asked for the pull request: its row flashes.
    Flash,
    /// An automation started the session.
    Automation,
    /// Piece J's cards. A pull request with failing checks (the Checks signal opens itself) and a running one.
    CardFailing,
    /// Everything green, with two unresolved threads and a requested change: the Review signal opened.
    CardReview,
    /// Merged: the one static line.
    CardMerged,
    /// A merge conflict, checks passing.
    CardConflict,
    /// GitHub hasn't answered yet.
    CardPending,
    /// GitHub labels, a head and base, and the diff stat; Checks and Review closed.
    CardLabels,
}

fn threads() -> Vec<Value> {
    vec![
        thread("t1", "src/cache.rs", 12, "mira", "This keeps every thumbnail for the whole session. Can the cache have a size limit?"),
        thread("t2", "src/render/tiles.rs", 48, "jonas", "Please name this after what it holds."),
    ]
}

fn card_links(case: Case) -> Vec<Value> {
    let green = || vec![run("lint", "completed", Some("success")), run("build", "completed", Some("success"))];
    match case {
        Case::CardFailing => vec![popover_pr(
            "open",
            vec![
                run("lint", "completed", Some("success")),
                run("build", "in_progress", None),
                run("e2e", "completed", Some("failure")),
                run("unit", "completed", Some("failure")),
            ],
            None,
            vec![],
            false,
            false,
        )],
        Case::CardReview => vec![popover_pr("open", green(), None, threads(), true, false)],
        Case::CardMerged => vec![popover_pr("merged", vec![], Some("APPROVED"), vec![], false, false)],
        Case::CardConflict => vec![popover_pr("open", green(), Some("APPROVED"), vec![], false, true)],
        Case::CardPending => {
            let mut wire = pull_request(SESSION, 187, "open", vec![], None);
            wire["enrichmentStatus"] = "pending".into();
            wire["statusLabel"] = "".into();
            wire["title"] = "example/demo #187: Cache thumbnails between reloads".into();
            vec![wire]
        }
        Case::CardLabels => {
            let mut wire = popover_pr("open", green(), Some("APPROVED"), vec![], false, false);
            let mut meta: Value = serde_json::from_str(wire["metadataJson"].as_str().unwrap()).unwrap();
            meta["labels"] = json!([{"name": "bug", "color": "d73a4a"}, {"name": "performance", "color": "0e8a16"}, {"name": "needs design", "color": "fbca04"}]);
            wire["metadataJson"] = meta.to_string().into();
            vec![wire]
        }
        _ => Vec::new(),
    }
}

fn mentioned(wire: Value) -> Value {
    let mut wire = wire;
    wire["relationship"] = "mentioned".into();
    wire
}

fn pinned(mut wire: Value) -> Value {
    wire["relationship"] = "pinned".into();
    wire
}

fn open(mut wire: Value) -> Value {
    wire["statusLabel"] = "Open".into();
    wire
}

fn links(case: Case) -> Vec<Value> {
    match case {
        Case::Empty | Case::Loading | Case::Error | Case::Automation => Vec::new(),
        Case::CardFailing | Case::CardReview | Case::CardMerged | Case::CardConflict | Case::CardPending | Case::CardLabels => {
            card_links(case)
        }
        Case::Links | Case::Flash => {
            let mut pinned = issue(9, "pinned");
            pinned["title"] = "example/demo #9: Cache size should be configurable".into();
            vec![
                open(issue(42, "origin")),
                open(failing(187, "own")),
                open(pinned),
                mentioned(open(issue(7, "mentioned"))),
                mentioned(open(pull_request(SESSION, 12, "open", vec![], None))),
            ]
        }
        // What a Fleet with no GitHub makes of two attached links: the scratch Fleet's own state.
        Case::NotConnected | Case::Attach | Case::AttachError => {
            let mut one = pinned(pull_request(SESSION, 187, "open", vec![], None));
            let mut two = pinned(issue(42, "pinned"));
            for wire in [&mut one, &mut two] {
                wire["enrichmentStatus"] = "not_connected".into();
                wire["metadataJson"] = Value::Null;
                wire["title"] = wire["resourceId"].clone();
                wire["statusLabel"] = "".into();
                wire["lastCheckedAt"] = Value::Null;
            }
            vec![one, two]
        }
    }
}

impl State {
    /// The content panel open on the Context tab with `case` held.
    pub fn show_context_tab_scene(&mut self, case: Case, dark: bool) {
        self.show_chips(super::context::Case::Chips, dark);
        let id = self.session().id.clone();
        self.smart_links.set_links(&id, &links(case));
        self.content_open = true;
        self.show_context_tab();
        self.context.loaded_for = Some(id.clone());
        match case {
            Case::Loading => {
                self.smart_links.by_session.remove(&id);
                self.smart_links.set_loading(&id, true);
            }
            Case::Error => {
                self.smart_links.by_session.remove(&id);
                self.smart_links.set_load_error(&id, Some("Fleet answered HTTP 500.".into()));
            }
            Case::Attach => {
                self.context.start_attach();
                self.context.url = rune::TextEdit::new("https://github.com/example/demo/pull/201");
            }
            Case::AttachError => {
                self.context.start_attach();
                self.context.url = rune::TextEdit::new("https://example.test/not-github");
                self.context.attach_error = Some("Paste a link to a GitHub pull request or issue.".into());
            }
            Case::Flash => self.smart_links.request_focus(&id, "link-187"),
            Case::CardReview => {
                self.context.review_open.insert("link-187".into());
            }
            _ => {}
        }
        if case == Case::Automation {
            self.projects[0].sessions[0].row.origin = Some(Origin {
                source_type: "automation".into(),
                title: Some("Nightly dependency sweep".into()),
                resource_url: None,
                resource_id: Some("auto-1".into()),
                provider_id: "builtin.automation".into(),
            });
            self.smart_links.origins.insert(
                id,
                crate::fleet::smart_links::read_origin_records(&json!([{
                    "sourceType": "automation", "title": "Nightly dependency sweep", "summary": "Bumped 3 packages",
                    "actionId": "start-session", "createdAt": "2024-10-04T09:00:00Z"}])),
            );
        } else {
            self.projects[0].sessions[0].row.origin = None;
        }
        // The cases that match the scratch Fleet's session have no branch and use the folder as it is.
        let plain = matches!(case, Case::Empty | Case::NotConnected | Case::Attach | Case::AttachError);
        self.projects[0].sessions[0].row.isolation = Some(if plain { "existing" } else { "worktree" }.into());
        if plain {
            self.projects[0].sessions[0].branch.clear();
        }
    }
}

impl State {
    /// The Context tab with the progress strip under it (piece K): `done` of `total` todos, `current` the one in hand.
    pub fn show_progress_strip(&mut self, done: u32, total: u32, current: Option<&str>, dark: bool) {
        self.show_context_tab_scene(Case::Links, dark);
        self.projects[0].sessions[0].row.progress =
            Some(crate::fleet::row_chips::Progress { kind: "todos".into(), done, total, current: current.map(str::to_string) });
    }

    /// The strip in its plan form (rows 9.2-9.4): the open session has a markdown checklist, `ticked` of its five steps
    /// done, and the line names the step after them.
    pub fn show_progress_strip_plan(&mut self, ticked: usize, dark: bool) {
        use crate::fleet::progress::{Plan, PlanGroup, PlanStep, ProgressDetail};
        self.show_context_tab_scene(Case::Links, dark);
        let step = |n: usize, title: &str| PlanStep {
            key: n.to_string(),
            number: Some(n.to_string()),
            title: title.into(),
            checked: n <= ticked,
            sub_done: 0,
            sub_total: 0,
            ticked_at: (n <= ticked).then(|| "2026-11-17T09:30:00Z".to_string()),
            ticked_in_message_id: None,
        };
        let plan = Plan {
            path: ".weave/plans/paging.md".into(),
            title: Some("Page the order list".into()),
            tracked_since: "2026-11-17T09:00:00Z".into(),
            groups: vec![
                PlanGroup {
                    title: Some("Server".into()),
                    steps: vec![step(1, "Add `limit` and `cursor` to the query"), step(2, "Return the next cursor")],
                },
                PlanGroup {
                    title: Some("Client".into()),
                    steps: vec![
                        step(3, "Read `nextCursor` in the list"),
                        step(4, "Load the next page on scroll"),
                        step(5, "Show a spinner at the end"),
                    ],
                },
            ],
        };
        let summary = crate::fleet::row_chips::Progress {
            kind: "plan".into(),
            done: ticked as u32,
            total: 5,
            current: plan.groups.iter().flat_map(|g| &g.steps).find(|s| !s.checked).map(|s| s.title.clone()),
        };
        self.projects[0].sessions[0].row.progress = Some(summary.clone());
        let id = self.projects[0].sessions[0].id.clone();
        self.progress.set(
            &id,
            Some(ProgressDetail {
                session_id: id.clone(),
                summary,
                todos: vec![],
                updated_at: "2026-11-17T09:31:00Z".into(),
                plan: Some(plan),
                subagents: vec![],
            }),
        );
    }
}
