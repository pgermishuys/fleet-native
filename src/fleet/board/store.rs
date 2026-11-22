//! The Board's store (`stores/board.ts`): one per app, loaded once when the Board is first shown with its switch on and never
//! again on its own (FB9; Try again reloads). It holds the board, its lanes and cards, the load and the changes in flight, the
//! two modes and the left panel's filters.
//!
//! Every request goes to the live machine on a thread and is handed back on the UI thread. A change counts in `pending`
//! (`isMutating`), clears the error first and says why it failed. Reordering lanes and moving a card show the new order at
//! once and put the old one back if the server refuses. Making a lane or a card on a Fleet with no board makes "My Board"
//! first. With no server (scenes, tests without a Fleet) the load answers the invented board (`mock::board`) as
//! `mock_answers` says, and a change does nothing.
//! No view here: the Board's pieces L to R read these fields.
use super::api::{self, CardChange, LaneChange};
use super::cards::CardSession;
use super::filters::{self, Filters};
use super::{Board, Card, Lane, SyncResult, order};
use crate::State;
use crate::fleet::labs::Feature;
use crate::fleet::link::Post;
use crate::{Rail, mock};

/// The name of the board made on first use (`DEFAULT_BOARD_NAME`).
pub const DEFAULT_BOARD_NAME: &str = "My Board";

/// `toErrorMessage`'s words when a change fails with none of its own.
const CHANGE_FAILED: &str = "Board request failed.";
/// The same for the load.
const LOAD_FAILED: &str = "Failed to load board.";
/// `requireBoardId`'s words: a change asked for before there is a board.
pub const NOT_LOADED: &str = "Board is not loaded.";

/// `BoardMode`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Work,
    Manage,
}

/// What the invented answers (no server) say: a scene picks the state it wants drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MockAnswers {
    /// The invented board (`mock::board`).
    #[default]
    Data,
    /// A Fleet with no board yet.
    Empty,
    /// Never answers: the first load's wait.
    Silent,
    /// The load fails.
    Failed,
}

/// A board with its lanes and cards, as a load or a first-use board hands them back.
type Loaded = Option<(Board, Vec<Lane>, Vec<Card>)>;

#[derive(Debug, Default)]
pub struct BoardStore {
    pub board: Option<Board>,
    /// Sorted (`sortBoardLanes`) after every answer.
    pub lanes: Vec<Lane>,
    /// Every card, archived ones too, sorted (`sortBoardCards`) after every answer.
    pub cards: Vec<Card>,
    pub is_loading: bool,
    pub is_loaded: bool,
    pub error: Option<String>,
    /// Changes in flight (`pendingMutations`).
    pub pending: usize,
    pub mode: Mode,
    pub filters: Filters,
    /// See `MockAnswers`; used only with no server.
    pub mock_answers: MockAnswers,
    /// The Board has been shown once with its switch on, so the one load has started (15.8).
    started: bool,
    /// The number of the load in flight (`loadPromise`); an older load's answer is dropped.
    loading: Option<u64>,
    loads: u64,
}

impl BoardStore {
    pub fn is_mutating(&self) -> bool {
        self.pending > 0
    }

    pub fn has_board(&self) -> bool {
        self.board.is_some()
    }

    pub fn is_manage_mode(&self) -> bool {
        self.mode == Mode::Manage
    }

    /// `toggleBoardMode`.
    pub fn toggle_mode(&mut self) {
        self.mode = if self.is_manage_mode() { Mode::Work } else { Mode::Manage };
    }

    /// `setBoardMode`.
    pub fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
    }

    /// `clearError` (the banner's Dismiss).
    pub fn clear_error(&mut self) {
        self.error = None;
    }

    pub fn sorted_lanes(&self) -> &[Lane] {
        &self.lanes
    }

    /// `inboxLane`.
    pub fn inbox_lane(&self) -> Option<&Lane> {
        self.lanes.iter().find(|l| l.is_inbox)
    }

    pub fn lane(&self, id: &str) -> Option<&Lane> {
        self.lanes.iter().find(|l| l.id == id)
    }

    pub fn card(&self, id: &str) -> Option<&Card> {
        self.cards.iter().find(|c| c.id == id)
    }

    /// `activeCards`.
    pub fn active_cards(&self) -> impl Iterator<Item = &Card> {
        self.cards.iter().filter(|c| !c.is_archived())
    }

    /// `lanesWithCards`.
    pub fn lanes_with_cards(&self) -> Vec<(Lane, Vec<Card>)> {
        order::lanes_with_cards(&self.lanes, &self.cards)
    }

    /// `getCardsForLane`: a lane's active cards in order.
    pub fn cards_for_lane(&self, lane: &str) -> Vec<Card> {
        self.lanes_with_cards().into_iter().find(|(l, _)| l.id == lane).map(|(_, cards)| cards).unwrap_or_default()
    }

    /// `isEmpty`.
    pub fn is_empty(&self) -> bool {
        self.has_board() && self.lanes.is_empty() && self.active_cards().next().is_none()
    }

    /// `sessions`: every card as one of the side panels' sessions.
    pub fn sessions(&self) -> Vec<CardSession> {
        filters::sessions(self.board.as_ref(), &self.lanes, &self.cards)
    }

    pub fn available_projects(&self) -> Vec<String> {
        filters::available_projects(self.board.as_ref())
    }

    pub fn available_agents(&self) -> Vec<String> {
        filters::available_agents(&self.sessions())
    }

    pub fn quick_stats(&self) -> filters::QuickStats {
        filters::quick_stats(&self.sessions(), &self.filters)
    }

    pub fn filter_summary(&self) -> String {
        filters::filter_summary(&self.sessions(), &self.filters)
    }

    pub fn grouped_sessions(&self) -> Vec<filters::Group> {
        filters::grouped(&self.sessions(), &self.filters)
    }

    /// The board and its lists as an answer left them: sorted, and the filters following the agents and the project.
    fn set_lanes(&mut self, mut lanes: Vec<Lane>) {
        order::sort_lanes(&mut lanes);
        self.lanes = lanes;
        self.follow();
    }

    fn set_cards(&mut self, mut cards: Vec<Card>) {
        order::sort_cards(&mut cards);
        self.cards = cards;
        self.follow();
    }

    fn set_loaded(&mut self, loaded: Loaded) {
        match loaded {
            Some((board, lanes, cards)) => {
                self.board = Some(board);
                self.set_lanes(lanes);
                self.set_cards(cards);
            }
            None => {
                self.board = None;
                self.lanes.clear();
                self.cards.clear();
                self.follow();
            }
        }
        self.is_loaded = true;
    }

    fn follow(&mut self) {
        let (agents, projects) = (self.available_agents(), self.available_projects());
        self.filters.follow(&agents, &projects);
    }
}

/// What `ensureBoard` found or made on the way to a change: the board to make the change on and, when the store had none,
/// what to show (a board loaded with its lists, or one made empty).
struct Ensured {
    board: String,
    loaded: Option<Loaded>,
}

/// `ensureBoard` on a thread: the store's board if it has one; else the first board the server lists (with its lanes and
/// cards, as `loadBoard` reads them); else "My Board", made now.
fn ensure(base: &str, have: Option<String>) -> Result<Ensured, String> {
    if let Some(board) = have {
        return Ok(Ensured { board, loaded: None });
    }
    if let Some(loaded) = read_board(base, None)? {
        return Ok(Ensured { board: loaded.0.id.clone(), loaded: Some(Some(loaded)) });
    }
    let made = api::board(base, &api::create_board(DEFAULT_BOARD_NAME))?;
    Ok(Ensured { board: made.id.clone(), loaded: Some(Some((made, Vec::new(), Vec::new()))) })
}

/// `loadBoard`'s reads: the boards, the one to show (the current one while it is listed, else the first), its lanes and cards.
fn read_board(base: &str, current: Option<String>) -> Result<Loaded, String> {
    let boards = api::fetch_boards(base)?;
    let chosen = current.and_then(|id| boards.iter().find(|b| b.id == id).cloned()).or_else(|| boards.first().cloned());
    let Some(board) = chosen else { return Ok(None) };
    let lanes = api::fetch_lanes(base, &board.id)?;
    let cards = api::fetch_cards(base, &board.id)?;
    Ok(Some((board, lanes, cards)))
}

fn words(error: String, fallback: &str) -> String {
    if error.trim().is_empty() { fallback.to_string() } else { error }
}

/// What the changes' threads hand back: a lane or card made (with the board `ensureBoard` found or made, and the lists read
/// again), and a lane or card changed (with the lists when they were read again).
type MadeLane = (Option<Loaded>, Lane, Vec<Lane>);
type MadeCard = (Option<Loaded>, Card, Vec<Card>);
type ChangedLane = (Lane, Option<Vec<Lane>>);
type ChangedCard = (Card, Option<Vec<Card>>);

/// What a change hands back to the UI thread.
type Then<T> = Box<dyn FnOnce(&mut State, &Result<T, String>) + Send>;

impl State {
    /// The live machine's address and a way back to the UI thread; none with no server.
    fn board_server(&self) -> Option<(String, Post)> {
        self.link.as_ref().map(|l| (l.base.clone(), l.post()))
    }

    /// Starts the one load the first time the Board is shown with its switch on (`useBoardStore()` in the first board view
    /// to mount). True when it started.
    pub(crate) fn keep_board(&mut self) -> bool {
        if self.board.started || self.rail != Rail::Board || self.labs_feature(Feature::Board) != Some(true) {
            return false;
        }
        self.board.started = true;
        self.board_load();
        true
    }

    /// `loadBoard`: one load at a time; a call while one is in flight does nothing.
    pub fn board_load(&mut self) {
        if self.board.loading.is_some() {
            return;
        }
        self.board.loads += 1;
        let id = self.board.loads;
        self.board.loading = Some(id);
        self.board.is_loading = true;
        self.board.error = None;
        let Some((base, post)) = self.board_server() else {
            let answer = match self.board.mock_answers {
                MockAnswers::Silent => return,
                MockAnswers::Failed => Err(api::list_boards().fallback.to_string()),
                MockAnswers::Empty => Ok(None),
                MockAnswers::Data => Ok(Some((mock::board::board(), mock::board::lanes(), mock::board::cards()))),
            };
            return self.board_loaded(id, answer);
        };
        let current = self.board.board.as_ref().map(|b| b.id.clone());
        std::thread::spawn(move || {
            let answer = read_board(&base, current);
            post.update(move |s, _| s.board_loaded(id, answer));
        });
    }

    fn board_loaded(&mut self, id: u64, answer: Result<Loaded, String>) {
        if self.board.loading != Some(id) {
            return;
        }
        self.board.loading = None;
        self.board.is_loading = false;
        match answer {
            Ok(loaded) => self.board.set_loaded(loaded),
            Err(e) => self.board.error = Some(words(e, LOAD_FAILED)),
        }
    }

    /// `reloadBoard` (Try again): a new load even while one is in flight; the older one's answer is dropped.
    pub fn board_reload(&mut self) {
        self.board.loading = None;
        self.board_load();
    }

    /// `runMutation`: counts the change, clears the error, runs `work` on a thread against the live machine and hands its
    /// answer to `apply` and then `then` on the UI thread; a failure is said in the error. With no server nothing is sent.
    fn board_change<T: Send + 'static>(
        &mut self,
        work: impl FnOnce(&str) -> Result<T, String> + Send + 'static,
        apply: impl FnOnce(&mut State, &T) + Send + 'static,
        then: Then<T>,
    ) {
        let Some((base, post)) = self.board_server() else { return };
        self.board.pending += 1;
        self.board.error = None;
        std::thread::spawn(move || {
            let answer = work(&base);
            post.update(move |s, _| {
                s.board.pending = s.board.pending.saturating_sub(1);
                match &answer {
                    Ok(value) => apply(s, value),
                    Err(e) => s.board.error = Some(words(e.clone(), CHANGE_FAILED)),
                }
                then(s, &answer);
            });
        });
    }

    /// The board a change is made on (`requireBoardId`); with none the change fails at once with Fleet's words.
    fn board_id_or_fail<T>(&mut self, then: Then<T>) -> Option<(String, Then<T>)> {
        match self.board.board.as_ref().map(|b| b.id.clone()) {
            Some(id) => Some((id, then)),
            None => {
                self.board.error = Some(NOT_LOADED.to_string());
                then(self, &Err(NOT_LOADED.to_string()));
                None
            }
        }
    }

    /// `syncBoard`: Sync now, then the lanes and cards read again. A failure leaves the board as it was.
    pub fn board_sync(&mut self, then: Then<SyncResult>) {
        let Some((id, then)) = self.board_id_or_fail(then) else { return };
        let fresh = std::sync::Arc::new(std::sync::Mutex::new(None));
        let kept = fresh.clone();
        self.board_change(
            move |base| {
                let result = api::run_sync(base, &id)?;
                let lanes = api::fetch_lanes(base, &id)?;
                let cards = api::fetch_cards(base, &id)?;
                *kept.lock().unwrap() = Some((lanes, cards));
                Ok(result)
            },
            move |s, _| {
                if let Some((lanes, cards)) = fresh.lock().unwrap().take() {
                    s.board.set_lanes(lanes);
                    s.board.set_cards(cards);
                }
            },
            then,
        );
    }

    /// `createBoard`.
    pub fn board_create(&mut self, name: &str, then: Then<Board>) {
        let name = name.to_string();
        self.board_change(
            move |base| api::board(base, &api::create_board(&name)),
            |s, board| {
                s.board.set_loaded(Some((board.clone(), Vec::new(), Vec::new())));
            },
            then,
        );
    }

    /// `renameBoard`.
    pub fn board_rename(&mut self, name: &str, then: Then<Board>) {
        let Some((id, then)) = self.board_id_or_fail(then) else { return };
        let name = name.to_string();
        self.board_change(
            move |base| api::board(base, &api::update_board(&id, &name)),
            |s, board| {
                s.board.board = Some(board.clone());
                s.board.follow();
            },
            then,
        );
    }

    /// `removeBoard` (nothing on `main` calls it, FB18).
    pub fn board_remove(&mut self, then: Then<()>) {
        let Some((id, then)) = self.board_id_or_fail(then) else { return };
        self.board_change(move |base| api::no_content(base, &api::delete_board(&id)), |s, _| s.board.set_loaded(None), then);
    }

    /// `createLane`: on the board, made first if there is none, then the lanes read again.
    pub fn board_create_lane(&mut self, name: &str, position: Option<f64>, then: Then<Lane>) {
        let have = self.board.board.as_ref().map(|b| b.id.clone());
        let name = name.to_string();
        self.board_change(
            move |base| {
                let ensured = ensure(base, have)?;
                let made = api::lane(base, &api::create_lane(&ensured.board, &name, position))?;
                let lanes = api::fetch_lanes(base, &ensured.board)?;
                Ok((ensured.loaded, made, lanes))
            },
            |s, (loaded, _, lanes)| {
                if let Some(loaded) = loaded {
                    s.board.set_loaded(loaded.clone());
                }
                s.board.set_lanes(lanes.clone());
            },
            pick(then, |s, (_, made, _): &MadeLane| s.board.lane(&made.id).cloned().unwrap_or(made.clone())),
        );
    }

    /// `updateLane`: a change of place or of the inbox reads the lanes again; a rename patches the one lane.
    pub fn board_update_lane(&mut self, lane: &str, change: LaneChange, then: Then<Lane>) {
        let Some((id, then)) = self.board_id_or_fail(then) else { return };
        let lane = lane.to_string();
        let reread = change.position.is_some() || change.is_inbox.is_some();
        self.board_change(
            move |base| {
                let updated = api::lane(base, &api::update_lane(&id, &lane, &change))?;
                let lanes = if reread { Some(api::fetch_lanes(base, &id)?) } else { None };
                Ok((updated, lanes))
            },
            |s, (updated, lanes)| match lanes {
                Some(lanes) => s.board.set_lanes(lanes.clone()),
                None => {
                    let lanes = order::patch_lane(&s.board.lanes, updated.clone());
                    s.board.set_lanes(lanes);
                }
            },
            pick(then, |s, (updated, _): &ChangedLane| s.board.lane(&updated.id).cloned().unwrap_or(updated.clone())),
        );
    }

    /// `renameLane`.
    pub fn board_rename_lane(&mut self, lane: &str, name: &str, then: Then<Lane>) {
        self.board_update_lane(lane, LaneChange { name: Some(name.to_string()), ..Default::default() }, then);
    }

    /// `setInboxLane`.
    pub fn board_set_inbox_lane(&mut self, lane: &str, then: Then<Lane>) {
        self.board_update_lane(lane, LaneChange { is_inbox: Some(true), ..Default::default() }, then);
    }

    /// `deleteLane`: gone from the list once the server says so.
    pub fn board_delete_lane(&mut self, lane: &str, then: Then<()>) {
        let Some((id, then)) = self.board_id_or_fail(then) else { return };
        let lane = lane.to_string();
        let gone = lane.clone();
        self.board_change(
            move |base| api::no_content(base, &api::delete_lane(&id, &lane)),
            move |s, _| {
                let lanes = s.board.lanes.iter().filter(|l| l.id != gone).cloned().collect();
                s.board.set_lanes(lanes);
            },
            then,
        );
    }

    /// `reorderLanes`: the new order at once, then the server's; the old order back if it refuses.
    pub fn board_reorder_lanes(&mut self, ids: &[String], then: Then<()>) {
        let before = self.board.lanes.clone();
        let reordered = order::reorder_lanes(&self.board.lanes, ids);
        self.board.lanes = reordered;
        let Some((id, then)) = self.board_id_or_fail(Box::new(move |s: &mut State, answer: &Result<(), String>| {
            if answer.is_err() {
                s.board.lanes = before;
            }
            then(s, answer);
        })) else {
            return;
        };
        let ids = ids.to_vec();
        let fresh = std::sync::Arc::new(std::sync::Mutex::new(None));
        let kept = fresh.clone();
        self.board_change(
            move |base| {
                api::no_content(base, &api::reorder_lanes(&id, &ids))?;
                *kept.lock().unwrap() = Some(api::fetch_lanes(base, &id)?);
                Ok(())
            },
            move |s, _| {
                if let Some(lanes) = fresh.lock().unwrap().take() {
                    s.board.set_lanes(lanes);
                }
            },
            then,
        );
    }

    /// `createCard`: on the board, made first if there is none, then the cards read again.
    pub fn board_create_card(&mut self, lane: &str, title: &str, position: Option<f64>, then: Then<Card>) {
        let have = self.board.board.as_ref().map(|b| b.id.clone());
        let (lane, title) = (lane.to_string(), title.to_string());
        self.board_change(
            move |base| {
                let ensured = ensure(base, have)?;
                let made = api::card(base, &api::create_card(&ensured.board, &lane, &title, position))?;
                let cards = api::fetch_cards(base, &ensured.board)?;
                Ok((ensured.loaded, made, cards))
            },
            |s, (loaded, _, cards)| {
                if let Some(loaded) = loaded {
                    s.board.set_loaded(loaded.clone());
                }
                s.board.set_cards(cards.clone());
            },
            pick(then, |s, (_, made, _): &MadeCard| s.board.card(&made.id).cloned().unwrap_or(made.clone())),
        );
    }

    /// `updateCard`: a change of lane or place reads the cards again; a rename patches the one card.
    pub fn board_update_card(&mut self, card: &str, change: CardChange, then: Then<Card>) {
        let Some((id, then)) = self.board_id_or_fail(then) else { return };
        let card = card.to_string();
        let reread = change.lane_id.is_some() || change.position.is_some();
        self.board_change(
            move |base| {
                let updated = api::card(base, &api::update_card(&id, &card, &change))?;
                let cards = if reread { Some(api::fetch_cards(base, &id)?) } else { None };
                Ok((updated, cards))
            },
            |s, (updated, cards)| match cards {
                Some(cards) => s.board.set_cards(cards.clone()),
                None => {
                    let cards = order::patch_card(&s.board.cards, updated.clone());
                    s.board.set_cards(cards);
                }
            },
            pick(then, |s, (updated, _): &ChangedCard| s.board.card(&updated.id).cloned().unwrap_or(updated.clone())),
        );
    }

    /// `renameCard`.
    pub fn board_rename_card(&mut self, card: &str, title: &str, then: Then<Card>) {
        self.board_update_card(card, CardChange { title: Some(title.to_string()), ..Default::default() }, then);
    }

    /// `deleteCard`: gone from the list once the server says so.
    pub fn board_delete_card(&mut self, card: &str, then: Then<()>) {
        let Some((id, then)) = self.board_id_or_fail(then) else { return };
        let card = card.to_string();
        let gone = card.clone();
        self.board_change(
            move |base| api::no_content(base, &api::delete_card(&id, &card)),
            move |s, _| {
                let cards = s.board.cards.iter().filter(|c| c.id != gone).cloned().collect();
                s.board.set_cards(cards);
            },
            then,
        );
    }

    /// `archiveCard`: the server's card replaces it, so it leaves its lane.
    pub fn board_archive_card(&mut self, card: &str, then: Then<Card>) {
        let Some((id, then)) = self.board_id_or_fail(then) else { return };
        let card = card.to_string();
        self.board_change(
            move |base| api::card(base, &api::archive_card(&id, &card)),
            |s, archived| {
                let cards = order::patch_card(&s.board.cards, archived.clone());
                s.board.set_cards(cards);
            },
            then,
        );
    }

    /// `moveCard`: the card at `index` of `lane` at once, then the server's order; the old order back if it refuses.
    pub fn board_move_card(&mut self, card: &str, lane: &str, index: usize, then: Then<Card>) {
        let before = self.board.cards.clone();
        let moved = order::move_card(&self.board.cards, card, lane, index);
        self.board.cards = moved;
        let Some((id, then)) = self.board_id_or_fail(Box::new(move |s: &mut State, answer: &Result<Card, String>| {
            if answer.is_err() {
                s.board.cards = before;
            }
            then(s, answer);
        })) else {
            return;
        };
        let (card, lane) = (card.to_string(), lane.to_string());
        self.board_change(
            move |base| {
                let moved = api::card(base, &api::move_card(&id, &card, &lane, index))?;
                let cards = api::fetch_cards(base, &id)?;
                Ok((moved, Some(cards)))
            },
            |s, (_, cards): &ChangedCard| s.board.set_cards(cards.clone().unwrap_or_default()),
            pick(then, |s, (moved, _): &ChangedCard| s.board.card(&moved.id).cloned().unwrap_or(moved.clone())),
        );
    }

    /// `reorderCard` (nothing on `main` calls it, FB18): a move within the card's own lane. An unknown card says "Card not
    /// found." and sends nothing.
    pub fn board_reorder_card(&mut self, card: &str, index: usize, then: Then<Card>) {
        match self.board.card(card).map(|c| c.lane_id.clone()) {
            Some(lane) => self.board_move_card(card, &lane, index, then),
            None => then(self, &Err("Card not found.".to_string())),
        }
    }
}

/// `then` for a change whose work hands back more than its caller wants: `f` picks the caller's answer from the store as the
/// change left it (Fleet answers the lane or card as the re-read lists have it, else as the server answered).
fn pick<T: 'static, U: 'static>(then: Then<T>, f: impl FnOnce(&State, &U) -> T + Send + 'static) -> Then<U> {
    Box::new(move |s, answer| {
        let picked = match answer {
            Ok(value) => Ok(f(s, value)),
            Err(e) => Err(e.clone()),
        };
        then(s, &picked);
    })
}

/// A `then` that does nothing, for a caller that only reads the store afterwards.
pub fn ignore<T>() -> Then<T> {
    Box::new(|_, _| {})
}
