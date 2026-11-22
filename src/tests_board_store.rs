//! Component 23, piece K: the Board's store. Fleet's `stores/__tests__/board.test.ts` under its names, against a fake Fleet
//! that keeps a board the way the test's mocked API does (lanes and cards renumbered (i+1)*1024 after each change) and
//! records every board request, then native cases: nothing asked until the Board is shown with its switch on, one load
//! ever, a load in flight not doubled, and the invented board with no server.
use crate::fleet::board::cards::Status;
use crate::fleet::board::filters::GroupBy;
use crate::fleet::board::order::POSITION_GAP;
use crate::fleet::board::store::{MockAnswers, Mode, NOT_LOADED, ignore};
use crate::fleet::board::{Board, Card, Lane, SyncResult};
use crate::fleet::link::{Job, Link, Post};
use crate::fleet::settings::HOME_MACHINE_KEY;
use crate::tests::{FRAME, app};
use crate::{Rail, State, test_wait};
use rune::Runtime;
use serde_json::{Value, json};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

/// `createTimestamp`: 27 February 2026, 10:<minute> UTC.
fn ts(minute: u32) -> String {
    format!("2026-02-27T10:{minute:02}:00.000Z")
}

fn board_fixture(id: &str, name: &str, minute: u32) -> Board {
    Board { id: id.into(), name: name.into(), created_at: ts(minute), updated_at: ts(minute) }
}

fn lane_fixture(id: &str, name: &str, position: f64, is_inbox: bool, minute: u32) -> Lane {
    Lane {
        id: id.into(),
        board_id: "board-1".into(),
        name: name.into(),
        position,
        is_inbox,
        created_at: ts(minute),
        updated_at: ts(minute),
    }
}

fn card_fixture(id: &str, lane: &str, title: &str, source_type: Option<&str>, source_key: &str, made: u32) -> Card {
    Card {
        id: id.into(),
        board_id: "board-1".into(),
        lane_id: lane.into(),
        title: title.into(),
        source_type: source_type.map(str::to_string),
        source_key: Some(source_key.into()),
        metadata: Some("Cover kanban workflows with mocked API responses.".into()),
        position: POSITION_GAP,
        archived_at: None,
        created_at: ts(made),
        updated_at: ts(made + 1),
    }
}

/// What the fake Fleet keeps, as `MockBoardState` does.
#[derive(Default)]
struct Kept {
    boards: Vec<Board>,
    lanes: Vec<Lane>,
    cards: Vec<Card>,
    fail_move: Option<String>,
    fail_reorder: Option<String>,
    fail_sync: Option<String>,
    /// The sync case's own answer (the inbox moves and the first lane is renamed).
    sync_moves_inbox: bool,
    /// Whether the Board's switch is on (`GET /api/preferences`).
    board_on: bool,
    /// Every board request, as "METHOD /path".
    heard: Vec<String>,
    /// Every board request's body, beside its "METHOD /path".
    bodies: Vec<(String, Value)>,
}

fn loaded() -> Kept {
    Kept {
        boards: vec![board_fixture("board-1", "Platform Roadmap", 0)],
        lanes: vec![
            lane_fixture("lane-backlog", "Backlog", POSITION_GAP, true, 1),
            lane_fixture("lane-doing", "In Progress", POSITION_GAP * 2.0, false, 2),
            lane_fixture("lane-review", "Review", POSITION_GAP * 3.0, false, 3),
            lane_fixture("lane-done", "Done", POSITION_GAP * 4.0, false, 4),
        ],
        cards: vec![
            card_fixture("card-backlog", "lane-backlog", "Capture manual QA notes", None, "manual-qa", 5),
            card_fixture("card-doing", "lane-doing", "Sync board store with API", Some("github"), "issue-42", 7),
            card_fixture("card-review", "lane-review", "Review kanban UI contracts", Some("weave"), "task-9", 9),
            card_fixture("card-done", "lane-done", "Ship persistent board support", Some("github"), "pr-77", 11),
        ],
        board_on: true,
        ..Default::default()
    }
}

fn empty() -> Kept {
    Kept { board_on: true, ..Default::default() }
}

fn by_place<T>(position: impl Fn(&T) -> f64, made: impl Fn(&T) -> String) -> impl Fn(&T, &T) -> std::cmp::Ordering {
    move |a, b| position(a).partial_cmp(&position(b)).unwrap().then_with(|| made(a).cmp(&made(b)))
}

impl Kept {
    /// `normalizeLanePositions`.
    fn normalize_lanes(&mut self) {
        self.lanes.sort_by(by_place(|l: &Lane| l.position, |l: &Lane| l.created_at.clone()));
        for (i, lane) in self.lanes.iter_mut().enumerate() {
            lane.position = (i + 1) as f64 * POSITION_GAP;
        }
    }

    /// `normalizeCardPositions`.
    fn normalize_cards(&mut self, lane: &str) {
        let mut ids: Vec<(f64, String, String)> = self
            .cards
            .iter()
            .filter(|c| c.lane_id == lane && c.archived_at.is_none())
            .map(|c| (c.position, c.created_at.clone(), c.id.clone()))
            .collect();
        ids.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap().then_with(|| a.1.cmp(&b.1)));
        for (i, (_, _, id)) in ids.iter().enumerate() {
            let card = self.cards.iter_mut().find(|c| &c.id == id).unwrap();
            card.position = (i + 1) as f64 * POSITION_GAP;
        }
    }

    /// `moveCardInState`.
    fn move_card(&mut self, id: &str, lane: &str, index: usize) -> Card {
        let from = self.cards.iter().find(|c| c.id == id).unwrap().lane_id.clone();
        let mut target: Vec<(f64, String, String)> = self
            .cards
            .iter()
            .filter(|c| c.lane_id == lane && c.id != id && c.archived_at.is_none())
            .map(|c| (c.position, c.created_at.clone(), c.id.clone()))
            .collect();
        target.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap().then_with(|| a.1.cmp(&b.1)));
        let mut order: Vec<String> = target.into_iter().map(|t| t.2).collect();
        order.insert(index.min(order.len()), id.to_string());
        for (i, card_id) in order.iter().enumerate() {
            let card = self.cards.iter_mut().find(|c| &c.id == card_id).unwrap();
            card.lane_id = lane.to_string();
            card.position = (i + 1) as f64 * POSITION_GAP;
        }
        self.normalize_cards(&from);
        self.normalize_cards(lane);
        self.cards.iter().find(|c| c.id == id).unwrap().clone()
    }

    fn sync(&mut self) -> SyncResult {
        if self.sync_moves_inbox {
            for lane in &mut self.lanes {
                if lane.id == "lane-backlog" {
                    lane.name = "Triage".into();
                    lane.is_inbox = false;
                }
                if lane.id == "lane-doing" {
                    lane.is_inbox = true;
                }
            }
        }
        let synced = Card {
            source_key: Some("github:acme/rocket#88".into()),
            position: POSITION_GAP * 2.0,
            ..card_fixture("card-synced", "lane-backlog", "Synced issue from GitHub", Some("github"), "", 65)
        };
        if let Some(doing) = self.cards.iter_mut().find(|c| c.id == "card-doing") {
            doing.title = "Sync board store with API (updated)".into();
        }
        self.cards.push(synced);
        self.normalize_cards("lane-backlog");
        self.normalize_cards("lane-doing");
        SyncResult { sources_processed: 1, issues_fetched: 2, cards_created: 1, cards_updated: 1, cards_marked_stale: 0, synced_at: ts(67) }
    }

    /// The board routes, as the test's mocked API answers them. A refusal is a 409 with Fleet's `{error}`.
    fn answer(&mut self, method: &str, route: &str, body: &Value) -> (&'static str, String) {
        let refuse = |words: &str| ("409 Conflict", json!({ "error": words }).to_string());
        let parts: Vec<&str> = route.trim_start_matches("/api/boards").split('/').filter(|p| !p.is_empty()).collect();
        let list = |items: Vec<Value>| ("200 OK", Value::Array(items).to_string());
        match (method, parts.as_slice()) {
            ("GET", []) => list(self.boards.iter().map(Board::to_json).collect()),
            ("POST", []) => {
                let board = board_fixture(&format!("board-{}", self.boards.len() + 1), body["name"].as_str().unwrap_or(""), 50);
                self.boards.push(board.clone());
                ("201 Created", board.to_json().to_string())
            }
            ("GET", [_, "lanes"]) => list(self.lanes.iter().map(Lane::to_json).collect()),
            ("POST", [board, "lanes"]) => {
                let at = self.lanes.iter().map(|l| l.position).fold(0.0, f64::max) + POSITION_GAP;
                let lane = Lane {
                    board_id: board.to_string(),
                    ..lane_fixture(&format!("lane-{}", self.lanes.len() + 1), body["name"].as_str().unwrap_or(""), at, false, 52)
                };
                self.lanes.push(lane.clone());
                self.normalize_lanes();
                ("201 Created", self.lanes.iter().find(|l| l.id == lane.id).unwrap().to_json().to_string())
            }
            ("PATCH", [_, "lanes", "reorder"]) => {
                if let Some(words) = self.fail_reorder.clone() {
                    return refuse(&words);
                }
                let ids: Vec<String> = body["laneIds"].as_array().unwrap().iter().filter_map(|v| v.as_str().map(str::to_string)).collect();
                for (i, id) in ids.iter().enumerate() {
                    if let Some(lane) = self.lanes.iter_mut().find(|l| &l.id == id) {
                        lane.position = (i + 1) as f64 * POSITION_GAP;
                    }
                }
                ("204 No Content", String::new())
            }
            ("GET", [_, "cards"]) => list(self.cards.iter().map(Card::to_json).collect()),
            ("POST", [board, "cards"]) => {
                let lane = body["laneId"].as_str().unwrap_or("").to_string();
                let count = self.cards.iter().filter(|c| c.lane_id == lane && c.archived_at.is_none()).count();
                let card = Card {
                    board_id: board.to_string(),
                    source_key: Some("manual-1".into()),
                    position: (count + 1) as f64 * POSITION_GAP,
                    ..card_fixture(&format!("card-{}", self.cards.len() + 1), &lane, body["title"].as_str().unwrap_or(""), None, "", 60)
                };
                self.cards.push(card.clone());
                self.normalize_cards(&lane);
                ("201 Created", self.cards.iter().find(|c| c.id == card.id).unwrap().to_json().to_string())
            }
            ("POST", [_, "cards", id, "move"]) => {
                if let Some(words) = self.fail_move.clone() {
                    return refuse(&words);
                }
                let card = self.move_card(id, body["laneId"].as_str().unwrap_or(""), body["position"].as_u64().unwrap_or(0) as usize);
                ("200 OK", card.to_json().to_string())
            }
            ("POST", [_, "sync"]) => {
                if let Some(words) = self.fail_sync.clone() {
                    return refuse(&words);
                }
                ("200 OK", self.sync().to_json().to_string())
            }
            _ => ("404 Not Found", json!({ "error": "Not found." }).to_string()),
        }
    }
}

struct Fake {
    base: String,
    kept: Arc<Mutex<Kept>>,
}

fn fake(kept: Kept) -> Fake {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a port");
    let base = format!("http://{}", listener.local_addr().expect("an address"));
    let kept = Arc::new(Mutex::new(kept));
    let shared = kept.clone();
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let kept = shared.clone();
            std::thread::spawn(move || {
                let req = crate::test_http::read(&mut stream);
                if req.route() == "/api/preferences" {
                    let on = if kept.lock().unwrap().board_on { "true" } else { "false" };
                    return crate::test_http::respond_json(&mut stream, "200 OK", &json!({ "features.board.enabled": on }).to_string());
                }
                if !req.route().starts_with("/api/boards") {
                    return crate::test_http::respond_json(&mut stream, "404 Not Found", "{}");
                }
                let (status, body) = {
                    let mut kept = kept.lock().unwrap();
                    kept.heard.push(format!("{} {}", req.method, req.path));
                    kept.bodies.push((format!("{} {}", req.method, req.path), req.json()));
                    kept.answer(&req.method, req.route(), &req.json())
                };
                crate::test_http::respond_json(&mut stream, status, &body);
            });
        }
    });
    Fake { base, kept }
}

struct Rig {
    rt: Runtime<State>,
    queue: Receiver<Job>,
    fake: Fake,
}

/// The app connected to a fake Fleet keeping `kept`, on the Sessions rail with the Board's switch as `kept` says.
fn rig(kept: Kept) -> Rig {
    let on = kept.board_on;
    let fake = fake(kept);
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    let link =
        Link::start_machine(HOME_MACHINE_KEY.to_string(), fake.base.clone(), Post::new(move |job| drop(jobs.lock().unwrap().send(job))));
    let mut rt = app();
    rt.update(move |s, _| {
        s.link = Some(link);
        s.pickers.prefs.adopt(&json!({ "features.board.enabled": if on { "true" } else { "false" } }));
    });
    rt.frame(FRAME);
    Rig { rt, queue, fake }
}

impl Rig {
    /// Applies what the threads hand back until `done` holds; the frames don't move the clock.
    fn wait(&mut self, what: &str, mut done: impl FnMut(&State) -> bool) {
        let queue = &self.queue;
        test_wait::until(
            &mut self.rt,
            what,
            |rt| {
                while let Ok(job) = queue.try_recv() {
                    rt.update(job);
                }
                rt.frame(0.0);
                done(&rt.state)
            },
            |rt| {
                let b = &rt.state.board;
                format!(
                    "board {:?}, loading {}, loaded {}, pending {}, error {:?}, lanes {}, cards {}",
                    b.board.as_ref().map(|b| &b.name),
                    b.is_loading,
                    b.is_loaded,
                    b.pending,
                    b.error,
                    b.lanes.len(),
                    b.cards.len()
                )
            },
        );
    }

    /// Shows the Board and waits for the store's first load to answer.
    fn open(&mut self) {
        self.rt.update(|s, _| s.rail = Rail::Board);
        self.rt.frame(FRAME);
        self.wait("the board's first load", |s| s.board.is_loaded || s.board.error.is_some());
    }

    /// Runs a change and waits for every change in flight to settle.
    fn change(&mut self, what: &str, act: impl FnOnce(&mut State) + Send + 'static) {
        self.rt.update(move |s, _| act(s));
        self.settle(what);
    }

    fn settle(&mut self, what: &str) {
        self.wait(what, |s| s.board.pending == 0);
    }

    /// The board requests heard so far that start with `prefix` ("GET /api/boards/board-1/lanes").
    fn heard(&self, prefix: &str) -> Vec<String> {
        self.fake.kept.lock().unwrap().heard.iter().filter(|h| h.starts_with(prefix)).cloned().collect()
    }

    fn heard_exactly(&self, request: &str) -> usize {
        self.fake.kept.lock().unwrap().heard.iter().filter(|h| *h == request).count()
    }

    fn kept(&self) -> std::sync::MutexGuard<'_, Kept> {
        self.fake.kept.lock().unwrap()
    }

    fn lane_ids(&self) -> Vec<String> {
        self.rt.state.board.sorted_lanes().iter().map(|l| l.id.clone()).collect()
    }

    fn card_ids(&self, lane: &str) -> Vec<String> {
        self.rt.state.board.cards_for_lane(lane).into_iter().map(|c| c.id).collect()
    }
}

// ─── board.test.ts ──────────────────────────────────────────────────────────

#[test]
fn loads_the_persisted_board_and_derives_board_session_stats() {
    let mut rig = rig(loaded());
    rig.open();

    assert_eq!(rig.heard_exactly("GET /api/boards"), 1);
    let b = &rig.rt.state.board;
    assert_eq!(b.board.as_ref().map(|b| b.name.as_str()), Some("Platform Roadmap"));
    let lanes: Vec<(String, usize)> = b.lanes_with_cards().into_iter().map(|(l, cards)| (l.name, cards.len())).collect();
    assert_eq!(lanes, [("Backlog".into(), 1), ("In Progress".into(), 1), ("Review".into(), 1), ("Done".into(), 1)]);
    let stats = b.quick_stats();
    assert_eq!((stats.total, stats.visible, stats.active, stats.completed, stats.projects), (4, 4, 1, 1, 1));
    assert_eq!(b.filter_summary(), "4 visible • grouped by status");
    assert_eq!(b.available_agents(), ["github", "manual", "weave"]);
    let keys: Vec<String> = b.grouped_sessions().into_iter().map(|g| g.key).collect();
    assert_eq!(keys, ["active", "idle", "waiting_input", "completed"]);
}

#[test]
fn defaults_to_work_mode_and_updates_board_mode_through_store_actions() {
    let mut rig = rig(loaded());
    rig.open();

    assert_eq!(rig.rt.state.board.mode, Mode::Work);
    assert!(!rig.rt.state.board.is_manage_mode());

    rig.rt.update(|s, _| s.board.toggle_mode());
    assert_eq!(rig.rt.state.board.mode, Mode::Manage);
    assert!(rig.rt.state.board.is_manage_mode());

    rig.rt.update(|s, _| s.board.set_mode(Mode::Work));
    assert_eq!(rig.rt.state.board.mode, Mode::Work);
    assert!(!rig.rt.state.board.is_manage_mode());
}

#[test]
fn creates_a_board_lane_and_card_through_the_mocked_board_api() {
    let mut rig = rig(empty());
    rig.open();
    assert!(!rig.rt.state.board.has_board());

    let made = Arc::new(Mutex::new(None));
    let kept = made.clone();
    rig.change("the lane", move |s| {
        s.board_create_lane("Inbox", None, Box::new(move |_, answer| *kept.lock().unwrap() = Some(answer.clone())))
    });

    let lane = made.lock().unwrap().take().expect("the lane's answer").expect("the lane was made");
    assert_eq!(rig.heard("POST /api/boards"), ["POST /api/boards", "POST /api/boards/board-1/lanes"]);
    assert_eq!(rig.kept().boards[0].name, "My Board");
    let b = &rig.rt.state.board;
    assert_eq!(b.board.as_ref().map(|b| b.name.as_str()), Some("My Board"));
    let names: Vec<&str> = b.sorted_lanes().iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["Inbox"]);

    let lane_id = lane.id.clone();
    rig.change("the card", move |s| s.board_create_card(&lane_id, "Write frontend regression tests", None, ignore()));

    assert_eq!(rig.heard("POST /api/boards/board-1/cards"), ["POST /api/boards/board-1/cards"]);
    let bodies: Vec<(String, Value)> = rig.kept().bodies.iter().filter(|(r, _)| r.starts_with("POST")).cloned().collect();
    assert_eq!(
        bodies,
        [
            ("POST /api/boards".to_string(), json!({"name": "My Board"})),
            ("POST /api/boards/board-1/lanes".to_string(), json!({"name": "Inbox"})),
            ("POST /api/boards/board-1/cards".to_string(), json!({"laneId": lane.id, "title": "Write frontend regression tests"})),
        ]
    );
    let titles: Vec<String> = rig.rt.state.board.cards_for_lane(&lane.id).into_iter().map(|c| c.title).collect();
    assert_eq!(titles, ["Write frontend regression tests"]);
    assert_eq!(rig.rt.state.board.error, None);
}

#[test]
fn optimistically_reorders_lanes_and_rolls_back_when_the_api_rejects() {
    let mut rig = rig(loaded());
    rig.open();
    let original = rig.lane_ids();
    rig.kept().fail_reorder = Some("Lane reorder failed.".into());

    let order: Vec<String> = ["lane-done", "lane-review", "lane-doing", "lane-backlog"].map(String::from).to_vec();
    let failed = Arc::new(Mutex::new(None));
    let kept = failed.clone();
    let wanted = order.clone();
    rig.rt.update(move |s, _| s.board_reorder_lanes(&wanted, Box::new(move |_, answer| *kept.lock().unwrap() = Some(answer.clone()))));

    assert_eq!(rig.lane_ids(), order, "the new order shows before the server answers");
    rig.settle("the refused reorder");

    assert_eq!(*failed.lock().unwrap(), Some(Err("Lane reorder failed.".to_string())));
    assert_eq!(rig.lane_ids(), original);
    assert_eq!(rig.rt.state.board.error.as_deref(), Some("Lane reorder failed."));
}

#[test]
fn optimistically_moves_cards_and_restores_the_previous_state_on_failure() {
    let mut rig = rig(loaded());
    rig.open();
    let original_lane = rig.rt.state.board.card("card-backlog").map(|c| c.lane_id.clone());
    rig.kept().fail_move = Some("Card move failed.".into());

    let failed = Arc::new(Mutex::new(None));
    let kept = failed.clone();
    rig.rt.update(move |s, _| {
        s.board_move_card("card-backlog", "lane-review", 1, Box::new(move |_, answer| *kept.lock().unwrap() = Some(answer.is_err())))
    });

    assert_eq!(rig.rt.state.board.card("card-backlog").map(|c| c.lane_id.as_str()), Some("lane-review"));
    assert_eq!(rig.card_ids("lane-review"), ["card-review", "card-backlog"]);
    rig.settle("the refused move");

    assert_eq!(*failed.lock().unwrap(), Some(true));
    assert_eq!(rig.rt.state.board.card("card-backlog").map(|c| c.lane_id.clone()), original_lane);
    assert_eq!(rig.card_ids("lane-review"), ["card-review"]);
    assert_eq!(rig.rt.state.board.error.as_deref(), Some("Card move failed."));
}

#[test]
fn syncs_the_board_and_refreshes_lanes_and_cards_from_the_api() {
    let mut rig = rig(Kept { sync_moves_inbox: true, ..loaded() });
    rig.open();

    let result = Arc::new(Mutex::new(None));
    let kept = result.clone();
    rig.change("the sync", move |s| s.board_sync(Box::new(move |_, answer| *kept.lock().unwrap() = Some(answer.clone()))));

    assert_eq!(rig.heard("POST /api/boards/board-1/sync"), ["POST /api/boards/board-1/sync"]);
    assert_eq!(rig.heard_exactly("GET /api/boards/board-1/lanes"), 2);
    assert_eq!(rig.heard_exactly("GET /api/boards/board-1/cards"), 2);
    let counts = SyncResult {
        sources_processed: 1,
        issues_fetched: 2,
        cards_created: 1,
        cards_updated: 1,
        cards_marked_stale: 0,
        synced_at: ts(67),
    };
    assert_eq!(*result.lock().unwrap(), Some(Ok(counts)));
    let b = &rig.rt.state.board;
    assert_eq!(b.inbox_lane().map(|l| l.id.as_str()), Some("lane-doing"));
    let lanes: Vec<(&str, &str, bool)> = b.sorted_lanes().iter().map(|l| (l.id.as_str(), l.name.as_str(), l.is_inbox)).collect();
    assert_eq!(
        lanes,
        [
            ("lane-backlog", "Triage", false),
            ("lane-doing", "In Progress", true),
            ("lane-review", "Review", false),
            ("lane-done", "Done", false)
        ]
    );
    let titles: Vec<String> = b.cards_for_lane("lane-backlog").into_iter().map(|c| c.title).collect();
    assert_eq!(titles, ["Capture manual QA notes", "Synced issue from GitHub"]);
    assert_eq!(b.card("card-doing").map(|c| c.title.as_str()), Some("Sync board store with API (updated)"));
}

#[test]
fn surfaces_sync_errors_without_clearing_the_current_board_state() {
    let mut rig = rig(loaded());
    rig.open();
    rig.kept().fail_sync = Some("Board sync failed.".into());
    let lanes_before = rig.rt.state.board.sorted_lanes().to_vec();
    let backlog_before = rig.rt.state.board.cards_for_lane("lane-backlog");

    let result = Arc::new(Mutex::new(None));
    let kept = result.clone();
    rig.change("the refused sync", move |s| s.board_sync(Box::new(move |_, answer| *kept.lock().unwrap() = Some(answer.clone()))));

    assert_eq!(*result.lock().unwrap(), Some(Err("Board sync failed.".to_string())));
    assert_eq!(rig.heard("POST /api/boards/board-1/sync"), ["POST /api/boards/board-1/sync"]);
    assert_eq!(rig.heard_exactly("GET /api/boards/board-1/lanes"), 1);
    assert_eq!(rig.heard_exactly("GET /api/boards/board-1/cards"), 1);
    let b = &rig.rt.state.board;
    assert_eq!(b.error.as_deref(), Some("Board sync failed."));
    assert_eq!(b.sorted_lanes(), lanes_before.as_slice());
    assert_eq!(b.cards_for_lane("lane-backlog"), backlog_before);
    assert!(b.card("card-synced").is_none());
}

// ─── native ─────────────────────────────────────────────────────────────────

/// Draws a few frames on `rail` so the poll has its chances to ask.
fn show(rig: &mut Rig, rail: Rail) {
    rig.rt.update(move |s, _| s.rail = rail);
    for _ in 0..5 {
        while let Ok(job) = rig.queue.try_recv() {
            rig.rt.update(job);
        }
        rig.rt.frame(FRAME);
    }
}

#[test]
fn asks_nothing_until_the_board_is_shown_with_its_switch_on_and_loads_it_once() {
    let mut rig = rig(Kept { board_on: false, ..loaded() });
    show(&mut rig, Rail::Sessions);
    show(&mut rig, Rail::Board);
    assert_eq!(rig.heard("GET /api/boards"), Vec::<String>::new(), "off, the board asks nothing");
    assert!(!rig.rt.state.board.is_loaded);

    rig.kept().board_on = true;
    rig.rt.update(|s, _| s.pickers.prefs.adopt(&json!({ "features.board.enabled": "true" })));
    show(&mut rig, Rail::Sessions);
    assert_eq!(rig.heard("GET /api/boards"), Vec::<String>::new(), "on, but not shown: still nothing");

    rig.open();
    show(&mut rig, Rail::Sessions);
    show(&mut rig, Rail::Board);
    rig.wait("the lists", |s| s.board.lanes.len() == 4);
    assert_eq!(rig.heard_exactly("GET /api/boards"), 1, "returning to the Board doesn't load it again");
    assert_eq!(rig.heard_exactly("GET /api/boards/board-1/lanes"), 1);
    assert_eq!(rig.heard_exactly("GET /api/boards/board-1/cards"), 1);
}

#[test]
fn a_load_in_flight_is_not_asked_twice_and_try_again_starts_a_new_one() {
    let mut rig = rig(loaded());
    rig.open();
    rig.rt.update(|s, _| {
        s.board_load();
        s.board_load();
    });
    rig.wait("the second load", |s| !s.board.is_loading);
    assert_eq!(rig.heard_exactly("GET /api/boards"), 2, "the call made while one was in flight asked nothing");

    rig.rt.update(|s, _| {
        s.board_load();
        s.board_reload();
    });
    assert!(rig.rt.state.board.is_loading);
    // Try again asks even while a load is in flight: both loads' lists are asked for, and the older answer is dropped.
    let kept = rig.fake.kept.clone();
    rig.wait("the reload, and both loads asked", move |s| {
        !s.board.is_loading && kept.lock().unwrap().heard.iter().filter(|h| *h == "GET /api/boards").count() == 4
    });
    assert_eq!(rig.rt.state.board.lanes.len(), 4);
}

#[test]
fn a_change_counts_while_it_is_out_clears_the_error_first_and_needs_a_board() {
    let mut rig = rig(empty());
    rig.open();
    let failed = Arc::new(Mutex::new(None));
    let kept = failed.clone();
    rig.rt.update(move |s, _| s.board_rename("Kites", Box::new(move |_, answer| *kept.lock().unwrap() = Some(answer.clone()))));
    assert_eq!(*failed.lock().unwrap(), Some(Err(NOT_LOADED.to_string())));
    assert_eq!(rig.rt.state.board.error.as_deref(), Some(NOT_LOADED));
    assert_eq!(rig.rt.state.board.pending, 0);

    // Making a lane makes the board first, so it needs none; the error goes as the change starts.
    let held = Arc::new(AtomicBool::new(false));
    let release = held.clone();
    rig.rt.update(move |s, _| s.board_create_lane("Inbox", None, Box::new(move |_, _| release.store(true, Ordering::SeqCst))));
    assert_eq!((rig.rt.state.board.pending, rig.rt.state.board.is_mutating()), (1, true));
    assert_eq!(rig.rt.state.board.error, None);
    rig.settle("the lane");
    assert!(held.load(Ordering::SeqCst));
    assert!(!rig.rt.state.board.is_mutating());
}

#[test]
fn the_filters_follow_the_cards_and_change_only_the_counts() {
    let mut rig = rig(loaded());
    rig.open();
    rig.rt.update(|s, _| {
        s.board.filters.set_status(Status::Active, false);
        s.board.filters.set_agent("weave", false);
        s.board.filters.group_by = GroupBy::Agent;
    });
    let b = &rig.rt.state.board;
    assert_eq!(b.filter_summary(), "2 visible • grouped by agent");
    assert_eq!(b.lanes_with_cards().iter().map(|(_, c)| c.len()).sum::<usize>(), 4, "the lanes keep every card (FB3)");
    let keys: Vec<String> = b.grouped_sessions().into_iter().map(|g| g.key).collect();
    assert_eq!(keys, ["github", "manual"]);

    // A re-read follows the agents and resets a project that is gone.
    rig.rt.update(|s, _| s.board.filters.project = "Gone".into());
    assert_eq!(rig.rt.state.board.quick_stats().visible, 0);
    rig.kept().cards.retain(|c| c.source_type.as_deref() != Some("weave"));
    rig.change("a sync", |s| s.board_sync(ignore()));
    let b = &rig.rt.state.board;
    assert_eq!(b.filters.project, "all");
    assert!(!b.filters.agents.contains_key("weave"));
}

#[test]
fn with_no_server_the_board_is_the_invented_one() {
    let mut rt = app();
    rt.update(|s, _| {
        s.pickers.prefs.adopt(&json!({ "features.board.enabled": "true" }));
        s.rail = Rail::Board;
    });
    rt.frame(FRAME);
    let b = &rt.state.board;
    assert!(b.is_loaded);
    assert_eq!(b.board.as_ref().map(|b| b.name.as_str()), Some("Kite launch"));
    assert_eq!(b.lanes.len(), 4);

    let mut rt = app();
    rt.update(|s, _| {
        s.board.mock_answers = MockAnswers::Failed;
        s.pickers.prefs.adopt(&json!({ "features.board.enabled": "true" }));
        s.rail = Rail::Board;
    });
    rt.frame(FRAME);
    assert_eq!(rt.state.board.error.as_deref(), Some("Failed to load boards."));
    assert!(!rt.state.board.is_loaded);
}
