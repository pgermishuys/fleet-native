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
        // A page no rail owns also takes the window from the open conversation.
        let away = |r: &Route| matches!(r, Route::Welcome | Route::Repositories | Route::Home);
        if away(&route) && !away(&self.route) {
            self.keep_current_stream();
        }
        let (was_home, is_home) = (self.route == Route::Home, route == Route::Home);
        self.route = route;
        if is_home && !was_home {
            self.landing.home_shown();
            self.landing_check();
        } else if was_home && !is_home {
            self.landing.home_left();
        }
        if !was && is {
            self.read_folder_feeds();
            self.read_harness_feeds();
        }
        self.clear_notification_for_open();
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
    /// puts its row in the list at once (an upsert, as `startSessionInFolderOf` does) and opens it. A failure is swallowed, as
    /// Fleet's palette does.
    pub fn new_session_in_folder(&mut self) {
        let (Some(link), Some(source)) = (&self.link, self.open_id()) else { return };
        link.off_thread(
            {
                let source = source.clone();
                move |base| http::post(base, &new_in_folder_path(&source), &serde_json::json!({}))
            },
            move |s, tx: &mut Tx, answer| {
                let Ok(answer) = answer else { return };
                if created_session_id(&answer).is_none() {
                    return;
                }
                s.made_done(&source, HOME_MACHINE_KEY, crate::fleet::fork::Made::InFolder, answer, tx);
            },
        );
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

    pub(crate) fn show_draft_row_starting(&mut self) {
        self.show_new_session(true);
        if let Some(draft) = self.new_draft.draft_mut() {
            draft.is_starting = true;
        }
    }

    pub(crate) fn show_new_session_palette(&mut self) {
        self.ui.open = Some(crate::ui::Open::Palette);
        self.ui.palette.query = rune::TextEdit::new("new session");
    }
}
