//! The Board's sources form (`BoardSourceConfig.vue`, script): the sources of the open board, the form's three fields and the
//! one error line. Drawn only in manage mode and gone with it: leaving manage mode unmounts the component, so its form, its
//! list and its error reset and the next visit loads again (22.14). The requests go straight to the live machine, not through
//! the store, as Fleet's component calls `board-api` itself: they don't count as the board's changes and the board's banner
//! never hears of them. With no server (scenes, tests without a Fleet) the load answers the invented sources.
use super::sources;
use super::{Source, api};
use crate::State;
use rune::TextEdit;

const LOAD_FAILED: &str = "Failed to load board sources.";
const CREATE_FAILED: &str = "Failed to create board source.";
const REMOVE_FAILED: &str = "Failed to remove board source.";

/// What the invented answers (no server) say to the load.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MockSources {
    /// The invented sources (`mock::board::sources`).
    #[default]
    Listed,
    /// None configured.
    None,
    /// The load fails.
    Failed,
}

#[derive(Debug, Default)]
pub struct SourceForm {
    /// The board the component is watching (`props.boardId`): `None` until the first look, then the board's id or none.
    watching: Option<Option<String>>,
    pub sources: Vec<Source>,
    pub loading: bool,
    pub creating: bool,
    /// The source being removed (`pendingDeleteSourceId`).
    pub removing: Option<String>,
    pub error: Option<String>,
    /// The picked repository's full name (`selectedRepository`); empty for "Select a repository".
    pub repository: String,
    pub labels: TextEdit,
    pub assigned_to_me: bool,
    /// Counts the form's mounts and loads, so an answer that comes after the form went or the board changed is dropped.
    turn: u64,
    pub mock: MockSources,
}

impl SourceForm {
    /// Unmounted: everything back to a fresh component's, and what is still on its way dropped.
    pub fn unmount(&mut self) {
        *self = SourceForm { turn: self.turn + 1, mock: self.mock, ..Default::default() };
    }

    /// Whether the form is mounted (the Board is shown in manage mode and has looked at its board).
    pub fn is_mounted(&self) -> bool {
        self.watching.is_some()
    }

    /// `hasBoard`.
    fn has_board(&self) -> bool {
        matches!(&self.watching, Some(Some(_)))
    }

    /// `normalizedSelectedLabels`.
    pub fn label_filter(&self) -> String {
        self.labels.text.clone()
    }

    /// `isDuplicateSource`.
    pub fn is_duplicate(&self) -> bool {
        sources::is_duplicate(&self.sources, &self.repository, &self.label_filter(), self.assigned_to_me)
    }

    /// `canSubmit`.
    pub fn can_submit(&self) -> bool {
        self.has_board()
            && !self.repository.trim().is_empty()
            && !self.loading
            && !self.creating
            && self.removing.is_none()
            && !self.is_duplicate()
    }

    /// The fields' `disabled`: while sources load, one is added or one is removed.
    pub fn fields_off(&self) -> bool {
        self.loading || self.creating || self.removing.is_some()
    }
}

fn words(error: String, fallback: &str) -> String {
    if error.trim().is_empty() { fallback.to_string() } else { error }
}

impl State {
    /// Mounts the form while the Board is in manage mode, and watches the board's id (`watch(() => props.boardId, …,
    /// { immediate: true })`): sources load when the id is known and again when it changes. True when something changed.
    pub(crate) fn keep_board_sources(&mut self) -> bool {
        // The component is mounted only while the Board is shown with its switch on, in manage mode.
        let shown = self.rail == crate::Rail::Board && self.labs_feature(crate::fleet::labs::Feature::Board) == Some(true);
        if !shown || !self.board.is_manage_mode() {
            let mounted = self.board.source_form.watching.is_some();
            if mounted {
                self.board.source_form.unmount();
            }
            return mounted;
        }
        let id = self.board.board.as_ref().map(|b| b.id.clone());
        if self.board.source_form.watching.as_ref() == Some(&id) {
            return false;
        }
        let first = self.board.source_form.watching.is_none();
        if first && let Some(place) = self.board_source() {
            (place.mount)(self);
        }
        let form = &mut self.board.source_form;
        form.watching = Some(id.clone());
        form.turn += 1;
        form.error = None;
        form.sources.clear();
        form.loading = false;
        if let Some(id) = id {
            self.board_sources_load(&id);
        }
        true
    }

    fn board_sources_load(&mut self, board: &str) {
        let form = &mut self.board.source_form;
        form.loading = true;
        let (turn, mock) = (form.turn, form.mock);
        let Some((base, post)) = self.board_server() else {
            let answer = match mock {
                MockSources::Listed => Ok(crate::mock::board::sources()),
                MockSources::None => Ok(Vec::new()),
                MockSources::Failed => Err(String::new()),
            };
            return self.board_sources_loaded(turn, answer);
        };
        let board = board.to_string();
        std::thread::spawn(move || {
            let answer = api::fetch_sources(&base, &board);
            post.update(move |s, _| s.board_sources_loaded(turn, answer));
        });
    }

    fn board_sources_loaded(&mut self, turn: u64, answer: Result<Vec<Source>, String>) {
        let form = &mut self.board.source_form;
        if form.turn != turn {
            return;
        }
        form.loading = false;
        match answer {
            Ok(list) => form.sources = list,
            Err(e) => {
                form.error = Some(words(e, LOAD_FAILED));
                form.sources.clear();
            }
        }
    }

    /// "Add source" / Enter in the form (`handleAddSource`): nothing unless `can_submit` and a plugin offers a source. The
    /// source joins the list when the server has made it; the label filter and the toggle reset, the repository stays.
    pub fn board_source_add(&mut self) {
        let Some(place) = self.board_source() else { return };
        let form = &mut self.board.source_form;
        let Some(Some(board)) = form.watching.clone() else { return };
        if !form.can_submit() {
            return;
        }
        form.creating = true;
        form.error = None;
        let turn = form.turn;
        let config = sources::add_config(&form.repository, &form.label_filter(), form.assigned_to_me);
        let Some((base, post)) = self.board_server() else {
            // No server: the source is made as the server would make it, so a scene can show one added.
            let made = Source {
                id: format!("source-new-{turn}"),
                board_id: board,
                provider_type: place.id.to_string(),
                config,
                last_sync_at: None,
                created_at: String::new(),
                updated_at: String::new(),
            };
            return self.board_source_added(turn, Ok(made));
        };
        let id = place.id;
        std::thread::spawn(move || {
            let answer = api::source(&base, &api::create_source(&board, id, &config));
            post.update(move |s, _| s.board_source_added(turn, answer));
        });
    }

    fn board_source_added(&mut self, turn: u64, answer: Result<Source, String>) {
        let form = &mut self.board.source_form;
        if form.turn != turn {
            return;
        }
        form.creating = false;
        match answer {
            Ok(source) => {
                form.sources.push(source);
                form.labels = TextEdit::default();
                form.assigned_to_me = false;
            }
            Err(e) => form.error = Some(words(e, CREATE_FAILED)),
        }
    }

    /// "Remove" on a source (`handleRemoveSource`).
    pub fn board_source_remove(&mut self, source: &str) {
        let form = &mut self.board.source_form;
        let Some(Some(board)) = form.watching.clone() else { return };
        form.removing = Some(source.to_string());
        form.error = None;
        let turn = form.turn;
        let source = source.to_string();
        let Some((base, post)) = self.board_server() else {
            return self.board_source_removed(turn, &source, Ok(()));
        };
        std::thread::spawn(move || {
            let answer = api::no_content(&base, &api::delete_source(&board, &source));
            post.update(move |s, _| s.board_source_removed(turn, &source, answer));
        });
    }

    fn board_source_removed(&mut self, turn: u64, source: &str, answer: Result<(), String>) {
        let form = &mut self.board.source_form;
        if form.turn != turn {
            return;
        }
        form.removing = None;
        match answer {
            Ok(()) => form.sources.retain(|s| s.id != source),
            Err(e) => form.error = Some(words(e, REMOVE_FAILED)),
        }
    }

    /// The assigned-to-me toggle (the checkbox).
    pub fn board_source_toggle_assigned(&mut self) {
        let form = &mut self.board.source_form;
        if !form.fields_off() {
            form.assigned_to_me = !form.assigned_to_me;
        }
    }
}
