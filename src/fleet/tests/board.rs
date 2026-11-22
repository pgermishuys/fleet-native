//! The Board's data and requests (component 23, piece J): the cases of weave-fleet's `lib/__tests__/board-api.test.ts`
//! under their names, against a fake Fleet that records each whole request, and native cases for what Fleet tests only
//! through its store and views: the error words' order, the order and move maths, a card's status from its lane, a card's
//! source and metadata, and a source's config.
use crate::fleet::board::api::{self, CardChange, LaneChange, Request, error_message};
use crate::fleet::board::cards::{self, SortBy, Status, as_session, status_of};
use crate::fleet::board::order::{self, POSITION_GAP};
use crate::fleet::board::sources::{self, SourceConfig};
use crate::fleet::board::{Board, Card, Lane, Source, SyncResult};
use crate::mock::board as mock;
use crate::test_http;
use serde_json::{Value, json};
use std::net::TcpListener;
use std::sync::mpsc::{Receiver, channel};

/// A fake Fleet that answers every request with `status` and `body`, and hands each whole request it read to the test.
fn fake(status: &'static str, body: String) -> (String, Receiver<test_http::Request>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let (tx, heard) = channel();
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let request = test_http::read(&mut stream);
            if request.method.is_empty() {
                continue;
            }
            test_http::respond_json(&mut stream, status, &body);
            let _ = tx.send(request);
        }
    });
    (base, heard)
}

/// The one request the fake heard: a call made once is asked once.
fn only(heard: &Receiver<test_http::Request>) -> test_http::Request {
    let request = crate::test_wait::recv("the board request", heard);
    assert!(heard.try_recv().is_err(), "one call, one request");
    request
}

fn source_fixture() -> Source {
    Source {
        id: "source-1".into(),
        board_id: "board-1".into(),
        provider_type: "github".into(),
        config: r#"{"repository":"acme/rocket"}"#.into(),
        last_sync_at: None,
        created_at: "2026-01-01T00:00:00Z".into(),
        updated_at: "2026-01-01T00:00:00Z".into(),
    }
}

fn is_json_request(request: &test_http::Request) -> bool {
    request.header("content-type").is_some_and(|v| v.starts_with("application/json"))
}

// ─── board-api.test.ts ──────────────────────────────────────────────────────

#[test]
fn lists_board_sources_with_typed_results() {
    let sources = vec![source_fixture()];
    let (base, heard) = fake("200 OK", Value::Array(sources.iter().map(Source::to_json).collect()).to_string());

    let result = api::fetch_sources(&base, "board-1");

    let request = only(&heard);
    assert_eq!((request.method.as_str(), request.path.as_str()), ("GET", "/api/boards/board-1/sources"));
    assert_eq!(result, Ok(sources));
}

#[test]
fn creates_a_board_source_with_the_expected_request_body() {
    let created = source_fixture();
    let (base, heard) = fake("201 Created", created.to_json().to_string());

    let result = api::source(&base, &api::create_source("board-1", "github", r#"{"repository":"acme/rocket"}"#));

    let request = only(&heard);
    assert_eq!((request.method.as_str(), request.path.as_str()), ("POST", "/api/boards/board-1/sources"));
    assert!(is_json_request(&request), "{}", request.head);
    assert_eq!(request.json(), json!({"providerType": "github", "config": "{\"repository\":\"acme/rocket\"}"}));
    assert_eq!(result, Ok(created));
}

#[test]
fn updates_a_board_source_with_the_expected_request_body() {
    let config = r#"{"repository":"acme/rocket","labels":["bug"]}"#;
    let updated = Source { config: config.into(), ..source_fixture() };
    let (base, heard) = fake("200 OK", updated.to_json().to_string());

    let result = api::source(&base, &api::update_source("board-1", "source-1", None, Some(config)));

    let request = only(&heard);
    assert_eq!((request.method.as_str(), request.path.as_str()), ("PATCH", "/api/boards/board-1/sources/source-1"));
    assert!(is_json_request(&request), "{}", request.head);
    // Only the field that was given: `providerType` is left out, so the server keeps it.
    assert_eq!(request.json(), json!({"config": config}));
    assert_eq!(result, Ok(updated));
}

#[test]
fn deletes_a_board_source() {
    let (base, heard) = fake("204 No Content", String::new());

    let result = api::no_content(&base, &api::delete_source("board-1", "source-1"));

    let request = only(&heard);
    assert_eq!((request.method.as_str(), request.path.as_str()), ("DELETE", "/api/boards/board-1/sources/source-1"));
    assert!(request.body.is_empty() && request.header("content-type").is_none(), "a DELETE goes with no body: {}", request.raw());
    assert_eq!(result, Ok(()));
}

#[test]
fn syncs_a_board_and_returns_typed_sync_counts() {
    let counts = SyncResult {
        sources_processed: 1,
        issues_fetched: 2,
        cards_created: 3,
        cards_updated: 4,
        cards_marked_stale: 5,
        synced_at: "2026-01-02T00:00:00Z".into(),
    };
    let (base, heard) = fake("200 OK", counts.to_json().to_string());

    let result = api::run_sync(&base, "board-1");

    let request = only(&heard);
    assert_eq!((request.method.as_str(), request.path.as_str()), ("POST", "/api/boards/board-1/sync"));
    assert!(is_json_request(&request), "{}", request.head);
    assert_eq!(request.json(), json!({}));
    assert_eq!(result, Ok(counts));
}

#[test]
fn surfaces_api_errors_for_source_operations() {
    let (base, heard) = fake("404 Not Found", json!({"error": "Source not found."}).to_string());

    let result = api::no_content(&base, &api::delete_source("board-1", "missing-source"));

    assert_eq!(only(&heard).path, "/api/boards/board-1/sources/missing-source");
    assert_eq!(result, Err("Source not found.".to_string()));
}

// ─── native: the requests and their words ───────────────────────────────────

#[test]
fn an_error_says_error_then_detail_then_title_then_the_text_then_the_call_s_own_words() {
    let fallback = "Failed to move board card.";
    assert_eq!(error_message(r#"{"error":"Lane not found.","detail":"d","title":"t"}"#, fallback), "Lane not found.");
    assert_eq!(
        error_message(r#"{"error":"  ","detail":"Position must be zero or greater.","title":"t"}"#, fallback),
        "Position must be zero or greater."
    );
    assert_eq!(error_message(r#"{"detail":"","title":"Conflict"}"#, fallback), "Conflict");
    assert_eq!(error_message(r#"{"error":7}"#, fallback), fallback, "a field that isn't text is skipped");
    assert_eq!(error_message("  upstream said no \n", fallback), "upstream said no", "a body that isn't JSON is said trimmed");
    assert_eq!(error_message("", fallback), fallback);
    assert_eq!(error_message("   ", fallback), fallback);
}

#[test]
fn a_call_nothing_answers_says_its_own_words() {
    let result = api::fetch_boards(&test_http::dead_address());
    assert_eq!(result, Err("Failed to load boards.".to_string()));
}

#[test]
fn a_refusal_with_an_empty_body_says_the_call_s_own_words() {
    // A sync with no GitHub token is a 401 with nothing in it (BSS).
    let (base, heard) = fake("401 Unauthorized", String::new());
    assert_eq!(api::run_sync(&base, "board-1"), Err("Failed to sync board.".to_string()));
    only(&heard);
}

#[test]
fn every_call_has_fleet_s_path_method_body_and_words() {
    let lane_change = LaneChange { is_inbox: Some(true), ..Default::default() };
    let card_change = CardChange { title: Some("Cut the spars".into()), ..Default::default() };
    let cases: Vec<(Request, &str, &str, Option<Value>, &str)> = vec![
        (api::list_boards(), "GET", "/api/boards", None, "Failed to load boards."),
        (api::create_board("Kite launch"), "POST", "/api/boards", Some(json!({"name": "Kite launch"})), "Failed to create board."),
        (api::update_board("b 1", "Kites"), "PATCH", "/api/boards/b%201", Some(json!({"name": "Kites"})), "Failed to update board."),
        (api::delete_board("b1"), "DELETE", "/api/boards/b1", None, "Failed to delete board."),
        (api::list_sources("b1"), "GET", "/api/boards/b1/sources", None, "Failed to load board sources."),
        (api::delete_source("b1", "s/1"), "DELETE", "/api/boards/b1/sources/s%2F1", None, "Failed to delete board source."),
        (api::sync("b1"), "POST", "/api/boards/b1/sync", Some(json!({})), "Failed to sync board."),
        (api::list_lanes("b1"), "GET", "/api/boards/b1/lanes", None, "Failed to load board lanes."),
        (
            api::create_lane("b1", "Review", None),
            "POST",
            "/api/boards/b1/lanes",
            Some(json!({"name": "Review"})),
            "Failed to create board lane.",
        ),
        (
            api::create_lane("b1", "Review", Some(512.0)),
            "POST",
            "/api/boards/b1/lanes",
            Some(json!({"name": "Review", "position": 512.0})),
            "Failed to create board lane.",
        ),
        (
            api::update_lane("b1", "l1", &lane_change),
            "PATCH",
            "/api/boards/b1/lanes/l1",
            Some(json!({"isInbox": true})),
            "Failed to update board lane.",
        ),
        (api::delete_lane("b1", "l1"), "DELETE", "/api/boards/b1/lanes/l1", None, "Failed to delete board lane."),
        (
            api::reorder_lanes("b1", &["l2".into(), "l1".into()]),
            "PATCH",
            "/api/boards/b1/lanes/reorder",
            Some(json!({"laneIds": ["l2", "l1"]})),
            "Failed to reorder board lanes.",
        ),
        (api::list_cards("b1"), "GET", "/api/boards/b1/cards", None, "Failed to load board cards."),
        (
            api::create_card("b1", "l1", "Sketch the frame", None),
            "POST",
            "/api/boards/b1/cards",
            Some(json!({"laneId": "l1", "title": "Sketch the frame"})),
            "Failed to create board card.",
        ),
        (
            api::update_card("b1", "c1", &card_change),
            "PATCH",
            "/api/boards/b1/cards/c1",
            Some(json!({"title": "Cut the spars"})),
            "Failed to update board card.",
        ),
        (api::delete_card("b1", "c1"), "DELETE", "/api/boards/b1/cards/c1", None, "Failed to delete board card."),
        (api::archive_card("b1", "c1"), "POST", "/api/boards/b1/cards/c1/archive", Some(json!({})), "Failed to archive board card."),
        (
            api::move_card("b1", "c1", "l2", 3),
            "POST",
            "/api/boards/b1/cards/c1/move",
            Some(json!({"laneId": "l2", "position": 3})),
            "Failed to move board card.",
        ),
    ];
    for (request, method, path, body, words) in cases {
        assert_eq!((request.method, request.path.as_str(), &request.body, request.fallback), (method, path, &body, words));
    }
    let source = api::create_source("b1", "github", "{}");
    assert_eq!(source.fallback, "Failed to create board source.");
    assert_eq!(api::update_source("b1", "s1", None, None).fallback, "Failed to update board source.");
}

#[test]
fn reads_the_board_lists_the_server_writes() {
    let lanes: Vec<Value> = mock::lanes().iter().map(Lane::to_json).collect();
    let (base, heard) = fake("200 OK", Value::Array(lanes).to_string());
    assert_eq!(api::fetch_lanes(&base, mock::BOARD_ID), Ok(mock::lanes()));
    assert_eq!(only(&heard).path, "/api/boards/board-kite/lanes");

    let cards: Vec<Value> = mock::cards().iter().map(Card::to_json).collect();
    let (base, heard) = fake("200 OK", Value::Array(cards).to_string());
    assert_eq!(api::fetch_cards(&base, mock::BOARD_ID), Ok(mock::cards()));
    only(&heard);

    let (base, heard) = fake("200 OK", json!([mock::board().to_json()]).to_string());
    assert_eq!(api::fetch_boards(&base), Ok(vec![mock::board()]));
    only(&heard);

    let (base, heard) = fake("200 OK", json!({"not": "a list"}).to_string());
    assert_eq!(api::fetch_boards(&base), Err("Failed to load boards.".to_string()), "an answer that doesn't read says the call's words");
    only(&heard);
}

#[test]
fn a_card_reads_with_its_nulls_and_a_fractional_position() {
    let card = Card::from_json(&json!({
        "id": "c1", "boardId": "b1", "laneId": "l1", "title": "Spool", "sourceType": null, "sourceKey": null,
        "metadata": null, "position": 1536.5, "archivedAt": null, "createdAt": "2026-11-03T10:00:00Z", "updatedAt": "2026-11-03T10:00:00Z"
    }))
    .unwrap();
    assert_eq!((card.source_type, card.position, card.archived_at), (None, 1536.5, None));
    assert!(Board::from_json(&json!({"name": "no id"})).is_none());
}

// ─── native: order ──────────────────────────────────────────────────────────

fn ids<T>(items: &[T], id: impl Fn(&T) -> &str) -> Vec<String> {
    items.iter().map(|i| id(i).to_string()).collect()
}

#[test]
fn lanes_sort_by_position_then_when_they_were_made() {
    let mut lanes = mock::lanes();
    lanes[3].position = 1024.0;
    lanes[3].created_at = "2026-11-01T00:00:00Z".into();
    lanes.reverse();
    assert_eq!(ids(&order::sorted_lanes(&lanes), |l| &l.id), ["lane-done", "lane-inbox", "lane-doing", "lane-review"]);
    assert_eq!(order::inbox_lane(&lanes).map(|l| l.id), Some("lane-inbox".to_string()));
}

#[test]
fn each_lane_holds_its_active_cards_by_position() {
    let grouped = order::lanes_with_cards(&mock::lanes(), &mock::cards());
    let shown: Vec<(String, Vec<String>)> = grouped.iter().map(|(l, cs)| (l.id.clone(), ids(cs, |c| &c.id))).collect();
    assert_eq!(
        shown,
        [
            ("lane-inbox".to_string(), vec!["card-sketch".to_string(), "card-synced".to_string()]),
            ("lane-doing".to_string(), vec!["card-spar".to_string(), "card-github".to_string()]),
            ("lane-review".to_string(), vec!["card-stale".to_string()]),
            ("lane-done".to_string(), vec!["card-colours".to_string()]),
        ],
        "the archived card is in no lane"
    );
}

#[test]
fn cards_sort_by_lane_then_position_then_when_made() {
    let sorted = order::sorted_cards(&mock::cards());
    assert_eq!(
        ids(&sorted, |c| &c.id),
        ["card-spar", "card-github", "card-archived", "card-colours", "card-sketch", "card-synced", "card-stale"]
    );
}

#[test]
fn reordering_lanes_puts_the_named_ones_first_and_renumbers_every_lane() {
    let reordered = order::reorder_lanes(&mock::lanes(), &["lane-review".into(), "missing".into(), "lane-inbox".into()]);
    let shown: Vec<(&str, f64)> = reordered.iter().map(|l| (l.id.as_str(), l.position)).collect();
    assert_eq!(shown, [("lane-review", 1024.0), ("lane-inbox", 2048.0), ("lane-doing", 3072.0), ("lane-done", 4096.0)]);
}

fn lane_order(cards: &[Card], lane: &str) -> Vec<(String, f64)> {
    order::lanes_with_cards(&mock::lanes(), cards)
        .into_iter()
        .find(|(l, _)| l.id == lane)
        .map(|(_, cs)| cs.into_iter().map(|c| (c.id, c.position)).collect())
        .unwrap_or_default()
}

#[test]
fn moving_a_card_to_another_lane_renumbers_both_lanes() {
    let moved = order::move_card(&order::sorted_cards(&mock::cards()), "card-spar", "lane-inbox", 1);
    assert_eq!(
        lane_order(&moved, "lane-inbox"),
        [("card-sketch".into(), 1024.0), ("card-spar".into(), 2048.0), ("card-synced".into(), 3072.0)]
    );
    assert_eq!(lane_order(&moved, "lane-doing"), [("card-github".into(), 1024.0)]);
    assert_eq!(order::sorted_cards(&moved), moved, "the list comes back sorted");
}

#[test]
fn moving_a_card_clamps_its_index_and_skips_archived_cards() {
    let moved = order::move_card(&order::sorted_cards(&mock::cards()), "card-stale", "lane-done", 99);
    assert_eq!(lane_order(&moved, "lane-done"), [("card-colours".into(), 1024.0), ("card-stale".into(), 2048.0)]);
    let archived = moved.iter().find(|c| c.id == "card-archived").unwrap();
    assert_eq!(archived.position, 1024.0, "an archived card keeps its number");
}

#[test]
fn moving_a_card_within_its_lane_renumbers_that_lane_once() {
    let moved = order::move_card(&order::sorted_cards(&mock::cards()), "card-github", "lane-doing", 0);
    assert_eq!(lane_order(&moved, "lane-doing"), [("card-github".into(), POSITION_GAP), ("card-spar".into(), 2.0 * POSITION_GAP)]);
    let cards = mock::cards();
    assert_eq!(order::move_card(&cards, "nobody", "lane-doing", 0), cards, "an unknown card changes nothing");
}

#[test]
fn patching_a_lane_or_card_replaces_it_and_keeps_the_order() {
    let mut review = mock::lanes()[2].clone();
    review.position = 10.0;
    let lanes = order::patch_lane(&mock::lanes(), review);
    assert_eq!(lanes[0].id, "lane-review");
    assert_eq!(lanes.len(), 4);

    let new_card = mock::card("card-new", "lane-inbox", "Test the bridle", 512.0);
    let cards = order::patch_card(&order::sorted_cards(&mock::cards()), new_card);
    assert_eq!(cards.len(), 8);
    assert_eq!(lane_order(&cards, "lane-inbox")[0].0, "card-new");
}

// ─── native: a card's status and reading ────────────────────────────────────

fn lane_named(name: &str, is_inbox: bool) -> Lane {
    Lane { name: name.into(), is_inbox, ..mock::lanes()[1].clone() }
}

#[test]
fn a_card_s_status_comes_from_its_lane_s_name() {
    let card = mock::card("c", "l", "Kite", 1024.0);
    let cases = [
        ("Anything", true, Status::Idle),
        ("Backlog", false, Status::Idle),
        ("To-do or TODO list", false, Status::Idle),
        ("Queued", false, Status::Idle),
        ("Code review", false, Status::Waiting),
        ("Needs approval", false, Status::Waiting),
        ("QA", false, Status::Waiting),
        ("Done", false, Status::Completed),
        ("Shipped", false, Status::Completed),
        ("Blocked", false, Status::Error),
        ("Failed builds", false, Status::Error),
        ("In progress", false, Status::Active),
    ];
    for (name, inbox, want) in cases {
        assert_eq!(status_of(&card, Some(&lane_named(name, inbox))), want, "{name}");
    }
    assert_eq!(status_of(&card, None), Status::Active, "a card whose lane isn't listed");
    // The patterns are tried in order: inbox words before review words.
    assert_eq!(status_of(&card, Some(&lane_named("Review backlog", false))), Status::Idle);
    let archived = Card { archived_at: Some("2026-11-05T12:00:00Z".into()), ..card };
    assert_eq!(status_of(&archived, Some(&lane_named("In progress", false))), Status::Completed);
}

#[test]
fn the_status_and_sort_options_are_fleet_s() {
    let labels: Vec<&str> = Status::ALL.iter().map(|s| s.label()).collect();
    assert_eq!(labels, ["Active", "Idle", "Waiting", "Complete", "Error"]);
    let sorts: Vec<&str> = SortBy::ALL.iter().map(|s| s.label()).collect();
    assert_eq!(sorts, ["Newest first", "Oldest first", "Title (A–Z)"]);
    for dark in [true, false] {
        let t = crate::theme::Theme::of(dark);
        assert_eq!(Status::Active.color(&t), t.running);
        assert_eq!(Status::Idle.color(&t), rune::color::hex("#71717a").unwrap());
        assert_eq!(Status::Waiting.color(&t), rune::color::hex("#f59e0b").unwrap());
    }
}

#[test]
fn a_card_counts_as_a_session_of_its_board() {
    let cards = mock::cards();
    let lanes = mock::lanes();
    let synced = cards.iter().find(|c| c.id == "card-synced").unwrap();
    let session = as_session(synced, Some(&mock::board()), lanes.iter().find(|l| l.id == synced.lane_id));
    assert_eq!(
        (session.project_name.as_str(), session.agent.as_str(), session.status, session.total_tokens, session.cost),
        ("Kite launch", "github-issue", Status::Idle, 0, 0.0)
    );
    let manual = as_session(&cards[0], None, None);
    assert_eq!((manual.project_name.as_str(), manual.agent.as_str()), ("Board", "manual"));
}

#[test]
fn a_synced_card_says_github_issue_sync() {
    let cards = mock::cards();
    let by_id = |id: &str| cards.iter().find(|c| c.id == id).unwrap().clone();
    assert_eq!(cards::source_label(&by_id("card-sketch")), "Manual");
    assert_eq!(cards::source_label(&by_id("card-github")), "GitHub sync");
    // What the server writes for a real sync (FB1): no GitHub look, and its metadata, being an object, is hidden.
    let synced = by_id("card-synced");
    assert_eq!(cards::source_label(&synced), "Github-issue sync");
    assert!(cards::github_meta(&synced).is_none());
    assert_eq!(cards::metadata_text(&synced), None);
    assert_eq!(cards::format_source_type(Some("linear")), "Linear");
    assert_eq!(cards::source_detail(&by_id("card-sketch")), "No source key");
    assert_eq!(cards::source_detail(&synced), "github:acme-labs/kite#57");
}

#[test]
fn a_github_card_reads_its_issue_from_the_metadata() {
    let cards = mock::cards();
    let github = cards.iter().find(|c| c.id == "card-github").unwrap();
    let meta = cards::github_meta(github).unwrap();
    assert_eq!(meta.number, Some(42.0));
    assert_eq!(meta.state.as_deref(), Some("open"));
    assert_eq!(meta.labels, ["bug", "priority:high"]);
    assert_eq!(meta.assignee.as_deref(), Some("kite-dev"));
    assert!(meta.html_url.is_some_and(|u| u.ends_with("/issues/42")));
    assert!(!meta.stale);
    assert_eq!(cards::github_issue_label(Some(&cards::github_meta(github).unwrap())), "Issue #42");

    let stale = cards::github_meta(cards.iter().find(|c| c.id == "card-stale").unwrap()).unwrap();
    assert!(stale.stale);
    assert_eq!((stale.assignee, stale.html_url), (None, None));

    let bare = Card { metadata: Some(r#"{"labels":["", "  ", 3, "ok"],"state":"  ","stale":"true"}"#.into()), ..github.clone() };
    let meta = cards::github_meta(&bare).unwrap();
    assert_eq!((&meta.labels, &meta.state, meta.stale), (&vec!["ok".to_string()], &None, false));
    assert_eq!(cards::github_issue_label(Some(&meta)), "GitHub issue");
}

#[test]
fn metadata_that_isn_t_an_object_shows_as_text() {
    let card = mock::card("c", "l", "Kite", 1024.0);
    for (metadata, text) in [
        (None, None),
        (Some("plain words"), Some("plain words")),
        (Some("[1,2]"), Some("[1,2]")),
        (Some("\"quoted\""), Some("\"quoted\"")),
        (Some("   "), Some("   ")),
        (Some(r#" {"a":1} "#), None),
    ] {
        let card = Card { metadata: metadata.map(str::to_string), ..card.clone() };
        assert_eq!(cards::metadata_text(&card).as_deref(), text, "{metadata:?}");
    }
}

// ─── native: a source's config ──────────────────────────────────────────────

#[test]
fn a_source_s_config_reads_its_repository_labels_and_assignee() {
    let read = |config: &str| sources::parse_config(config);
    assert_eq!(
        read(r#"{"repository":" acme-labs/kite ","labels":["bug"," ","priority:high"],"assignee":" @me "}"#),
        SourceConfig { repository: Some("acme-labs/kite".into()), labels: Some("bug, priority:high".into()), assignee: Some("@me".into()) }
    );
    assert_eq!(read(r#"{"owner":" acme-labs ","repo":"spool","labels":"a,, b "}"#).repository.as_deref(), Some("acme-labs/spool"));
    assert_eq!(read(r#"{"owner":" acme-labs ","repo":"spool","labels":"a,, b "}"#).labels.as_deref(), Some("a, b"));
    assert_eq!(read(r#"{"repository":"  ","labels":[],"assignee":""}"#), SourceConfig::default());
    assert_eq!(read("not json"), SourceConfig::default());
}

#[test]
fn sources_list_by_repository() {
    let mut list = mock::sources();
    list.push(Source { id: "source-odd".into(), config: "{}".into(), ..list[0].clone() });
    let rows = sources::rows(&list);
    let shown: Vec<&str> = rows.iter().map(|r| r.repository.as_str()).collect();
    assert_eq!(shown, ["acme-labs/kite", "acme-labs/spool", "Unknown repository"]);
    assert_eq!(rows[1].assignee.as_deref(), Some("@me"));
}

#[test]
fn a_source_is_a_duplicate_in_any_letters_but_not_with_other_labels_or_assignee() {
    let list = mock::sources();
    assert!(sources::is_duplicate(&list, "ACME-labs/Kite", " Bug ,priority:HIGH", false));
    assert!(!sources::is_duplicate(&list, "acme-labs/kite", "bug", false));
    assert!(!sources::is_duplicate(&list, "acme-labs/kite", "bug, priority:high", true));
    assert!(sources::is_duplicate(&list, "acme-labs/spool", "", true));
    assert!(!sources::is_duplicate(&list, "  ", "", false));
}

#[test]
fn a_new_source_sends_only_what_was_given_in_fleet_s_order() {
    assert_eq!(sources::add_config(" acme-labs/kite ", "", false), r#"{"repository":"acme-labs/kite"}"#);
    assert_eq!(
        sources::add_config("acme-labs/kite", " bug, ,priority:high ", true),
        r#"{"repository":"acme-labs/kite","labels":"bug, priority:high","assignee":"@me"}"#
    );
}
