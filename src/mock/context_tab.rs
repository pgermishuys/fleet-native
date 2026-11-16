//! Invented links for the Context tab's scenes and tests (component 8, piece I): every state of the tab the scratch Fleet
//! can't make (it has no GitHub), shaped as the server sends them (`mock::links`).
use super::context::{SESSION, failing, issue};
use super::links::pull_request;
use crate::State;
use crate::fleet::context_tab::PanelTab;
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
        self.context.tab = PanelTab::Context;
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
