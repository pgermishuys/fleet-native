//! Getting to the New session page and leaving it (`routes/sessions.new.tsx`, `use-commands.ts`, `SessionsPanel.vue`;
//! rows A2.1-A2.12, A2.16). Every way in ends in `open_new_session_page`; the page itself is `view::new_session`.
//! Plain state changes: nothing here draws, and the one request runs on a thread.
use super::draft::NewSessionDraft;
use crate::fleet::http;
use crate::fleet::settings::HOME_MACHINE_KEY;
use crate::fleet::store::Route;
use crate::{Rail, State, Tx};
use serde_json::Value;

/// `POST /api/sessions/{id}/new-in-folder`: an empty session in the folder of `id`, on the same harness.
pub fn new_in_folder_path(id: &str) -> String {
    format!("/api/sessions/{id}/new-in-folder")
}

/// The session a `new-in-folder` answer made (`CreateSessionResponse.session.id`); `None` when it names none.
pub fn created_session_id(answer: &Value) -> Option<String> {
    answer.get("session")?.get("id")?.as_str().filter(|id| !id.is_empty()).map(str::to_string)
}

impl State {
    /// Whether the New session page is what the window shows.
    pub fn on_new_session_page(&self) -> bool {
        matches!(self.route, Route::NewSession { .. })
    }

    /// Changes what the window shows. Arriving at the page opens its draft (the one left from last time, or a fresh one);
    /// leaving drops a draft with nothing in it and keeps one with a message, with its list row (A3.3, A3.4, A2.16).
    pub(crate) fn go(&mut self, route: Route) {
        let (was, is) = (self.on_new_session_page(), matches!(route, Route::NewSession { .. }));
        if was && !is {
            self.new_draft.leave();
        }
        if !was && is {
            // The open conversation isn't on screen any more; what it held is kept, so opening it again starts from that.
            self.keep_current_stream();
            let harness = self.feeds.harnesses.get(HOME_MACHINE_KEY).map(|f| f.harnesses.default_type()).unwrap_or_default();
            self.reset_new_box_errors();
            let restored = self.new_draft.open(NewSessionDraft::blank(&harness));
            // A project in the address wins; a kept draft otherwise keeps its own (A3.5).
            if let (Route::NewSession { project_id }, Some(draft)) = (&route, self.new_draft.draft_mut())
                && (project_id.is_some() || !restored)
            {
                draft.project_id = project_id.clone();
            }
        }
        self.route = route;
        if !was && is {
            self.read_folder_feeds();
            self.read_harness_feeds();
        }
    }

    /// `/sessions/new` with an optional `projectId`: the list's button, a project's +, the draft row, the palette, Ctrl+N.
    /// Already on the page, only the project moves (the watch on `search.projectId`, A2.8).
    pub fn open_new_session_page(&mut self, project_id: Option<String>) {
        self.rail = Rail::Sessions;
        if self.on_new_session_page() {
            if let Some(draft) = self.new_draft.draft_mut() {
                draft.project_id = project_id.clone();
            }
            self.route = Route::NewSession { project_id };
            return;
        }
        self.go(Route::NewSession { project_id });
    }

    /// The palette's "New session in this folder" (A2.6): asks the server for an empty session in the open session's folder,
    /// then opens it. A failure is swallowed, as Fleet's palette does.
    pub fn new_session_in_folder(&mut self) {
        let (Some(link), Some(id)) = (&self.link, self.open_id()) else { return };
        link.off_thread(
            move |base| http::post(base, &new_in_folder_path(&id), &serde_json::json!({})),
            |s, _: &mut Tx, answer| {
                let Some(id) = answer.ok().as_ref().and_then(created_session_id) else { return };
                // The list doesn't have the row yet: ask for it, and open the session when it arrives.
                s.new_draft.open_after_list = Some(id);
                if let Some(link) = &s.link {
                    link.refetch_list();
                }
            },
        );
    }

    /// The list arrived: opens the session `new_session_in_folder` made, once it is in it.
    pub(crate) fn open_made_session(&mut self, tx: &mut Tx) {
        let Some(id) = self.new_draft.open_after_list.clone() else { return };
        let Some(at) =
            self.projects.iter().enumerate().find_map(|(p, project)| project.sessions.iter().position(|s| s.id == id).map(|i| (p, i)))
        else {
            return;
        };
        self.new_draft.open_after_list = None;
        self.rail = Rail::Sessions;
        self.open_session(at.0, at.1, tx);
    }
}

/// Scenes for the pictures (`--scene new-session`): the page, and the page left with a typed message so the list's Draft row
/// shows its title in the project the + chose.
impl State {
    pub(crate) fn show_new_session(&mut self, left_with_draft: bool) {
        if !left_with_draft {
            self.open_new_session_page(None);
            return;
        }
        let project = self.projects.get(1).map(|p| p.id.clone());
        self.open_new_session_page(project);
        if let Some(draft) = self.new_draft.draft_mut() {
            draft.message = "Make the image cache survive a restart".into();
        }
        self.set_open((1, 0));
    }

    pub(crate) fn show_new_session_palette(&mut self) {
        self.ui.open = Some(crate::ui::Open::Palette);
        self.ui.palette.query = rune::TextEdit::new("new session");
    }
}
