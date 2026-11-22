//! `lib/board-api.ts`: every Board call as plain data (method, path, body, the words for a failure), and `send`, which runs
//! one against the live machine's address on whatever thread calls it. JSON bodies go with `Content-Type:
//! application/json`; the machine's token goes with every call (`http`'s `auth`). A refusal says what Fleet said
//! (`error_message`, in Fleet's order), else the call's own words.
use super::{Board, Card, Lane, Source, SyncResult, list_of};
use crate::fleet::http;
use serde_json::{Map, Value, json};

/// One call to the board routes, before an address is chosen.
#[derive(Clone, Debug, PartialEq)]
pub struct Request {
    pub method: &'static str,
    pub path: String,
    /// `None` for a GET and a DELETE, which Fleet sends with no body.
    pub body: Option<Value>,
    /// The words when the server says nothing usable (`fallbackMessage`).
    pub fallback: &'static str,
}

impl Request {
    /// Runs it against `base`: the answer's JSON (null for an empty 204), or the words for why not. A call nothing answered
    /// says the call's own words too (Fleet shows the browser's "Failed to fetch" there).
    pub fn send(&self, base: &str) -> Result<Value, String> {
        let (status, text) = http::text_with(self.method, base, &self.path, self.body.as_ref()).map_err(|_| self.fallback.to_string())?;
        if !(200..300).contains(&status) {
            return Err(error_message(&text, self.fallback));
        }
        Ok(serde_json::from_str(&text).unwrap_or(Value::Null))
    }

    /// `send`, then the answer read as `T`; an answer that doesn't read is the call's own words.
    fn send_as<T>(&self, base: &str, read: impl Fn(&Value) -> Option<T>) -> Result<T, String> {
        read(&self.send(base)?).ok_or_else(|| self.fallback.to_string())
    }
}

/// `readErrorMessage`: the body's `error`, else `detail`, else `title` (each only when it isn't blank, and kept as written);
/// a body that isn't JSON is said trimmed; else the call's own words.
pub fn error_message(body: &str, fallback: &str) -> String {
    if body.is_empty() {
        return fallback.to_string();
    }
    match serde_json::from_str::<Value>(body) {
        Ok(v) => ["error", "detail", "title"]
            .iter()
            .find_map(|k| v.get(k).and_then(Value::as_str).filter(|s| !s.trim().is_empty()))
            .map_or_else(|| fallback.to_string(), str::to_string),
        Err(_) if !body.trim().is_empty() => body.trim().to_string(),
        Err(_) => fallback.to_string(),
    }
}

fn enc(text: &str) -> String {
    crate::conv::export_history::encode_component(text)
}

pub fn boards_path() -> String {
    "/api/boards".to_string()
}

pub fn board_path(board: &str) -> String {
    format!("/api/boards/{}", enc(board))
}

pub fn lanes_path(board: &str) -> String {
    format!("{}/lanes", board_path(board))
}

pub fn lane_path(board: &str, lane: &str) -> String {
    format!("{}/{}", lanes_path(board), enc(lane))
}

pub fn sources_path(board: &str) -> String {
    format!("{}/sources", board_path(board))
}

pub fn source_path(board: &str, source: &str) -> String {
    format!("{}/{}", sources_path(board), enc(source))
}

pub fn cards_path(board: &str) -> String {
    format!("{}/cards", board_path(board))
}

pub fn card_path(board: &str, card: &str) -> String {
    format!("{}/{}", cards_path(board), enc(card))
}

fn get(path: String, fallback: &'static str) -> Request {
    Request { method: "GET", path, body: None, fallback }
}

fn send(method: &'static str, path: String, body: Value, fallback: &'static str) -> Request {
    Request { method, path, body: Some(body), fallback }
}

fn delete(path: String, fallback: &'static str) -> Request {
    Request { method: "DELETE", path, body: None, fallback }
}

/// A body of only the fields that were given (`UpdateBoardLaneRequest` and the others leave a field out to keep it).
fn fields(pairs: &[(&str, Option<Value>)]) -> Value {
    let map: Map<String, Value> = pairs.iter().filter_map(|(k, v)| v.clone().map(|v| (k.to_string(), v))).collect();
    Value::Object(map)
}

// ─── the requests ───────────────────────────────────────────────────────────

pub fn list_boards() -> Request {
    get(boards_path(), "Failed to load boards.")
}

pub fn create_board(name: &str) -> Request {
    send("POST", boards_path(), json!({ "name": name }), "Failed to create board.")
}

pub fn update_board(board: &str, name: &str) -> Request {
    send("PATCH", board_path(board), json!({ "name": name }), "Failed to update board.")
}

pub fn delete_board(board: &str) -> Request {
    delete(board_path(board), "Failed to delete board.")
}

pub fn list_sources(board: &str) -> Request {
    get(sources_path(board), "Failed to load board sources.")
}

/// `createBoardSource`: `config` is the JSON object as text (`sources::add_config`).
pub fn create_source(board: &str, provider_type: &str, config: &str) -> Request {
    send("POST", sources_path(board), json!({ "providerType": provider_type, "config": config }), "Failed to create board source.")
}

/// `updateBoardSource` (nothing on `main` calls it, FB18; ported for the API's tests).
pub fn update_source(board: &str, source: &str, provider_type: Option<&str>, config: Option<&str>) -> Request {
    let body = fields(&[("providerType", provider_type.map(|s| json!(s))), ("config", config.map(|s| json!(s)))]);
    send("PATCH", source_path(board, source), body, "Failed to update board source.")
}

pub fn delete_source(board: &str, source: &str) -> Request {
    delete(source_path(board, source), "Failed to delete board source.")
}

pub fn sync(board: &str) -> Request {
    send("POST", format!("{}/sync", board_path(board)), json!({}), "Failed to sync board.")
}

pub fn list_lanes(board: &str) -> Request {
    get(lanes_path(board), "Failed to load board lanes.")
}

pub fn create_lane(board: &str, name: &str, position: Option<f64>) -> Request {
    let body = fields(&[("name", Some(json!(name))), ("position", position.map(|p| json!(p)))]);
    send("POST", lanes_path(board), body, "Failed to create board lane.")
}

/// What `updateBoardLane` changes; a field left `None` stays as it is.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LaneChange {
    pub name: Option<String>,
    pub position: Option<f64>,
    pub is_inbox: Option<bool>,
}

pub fn update_lane(board: &str, lane: &str, change: &LaneChange) -> Request {
    let body = fields(&[
        ("name", change.name.as_ref().map(|s| json!(s))),
        ("position", change.position.map(|p| json!(p))),
        ("isInbox", change.is_inbox.map(|b| json!(b))),
    ]);
    send("PATCH", lane_path(board, lane), body, "Failed to update board lane.")
}

pub fn delete_lane(board: &str, lane: &str) -> Request {
    delete(lane_path(board, lane), "Failed to delete board lane.")
}

pub fn reorder_lanes(board: &str, lane_ids: &[String]) -> Request {
    send("PATCH", format!("{}/reorder", lanes_path(board)), json!({ "laneIds": lane_ids }), "Failed to reorder board lanes.")
}

pub fn list_cards(board: &str) -> Request {
    get(cards_path(board), "Failed to load board cards.")
}

pub fn create_card(board: &str, lane: &str, title: &str, position: Option<f64>) -> Request {
    let body = fields(&[("laneId", Some(json!(lane))), ("title", Some(json!(title))), ("position", position.map(|p| json!(p)))]);
    send("POST", cards_path(board), body, "Failed to create board card.")
}

/// What `updateBoardCard` changes; a field left `None` stays as it is.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CardChange {
    pub title: Option<String>,
    pub lane_id: Option<String>,
    pub position: Option<f64>,
}

pub fn update_card(board: &str, card: &str, change: &CardChange) -> Request {
    let body = fields(&[
        ("title", change.title.as_ref().map(|s| json!(s))),
        ("laneId", change.lane_id.as_ref().map(|s| json!(s))),
        ("position", change.position.map(|p| json!(p))),
    ]);
    send("PATCH", card_path(board, card), body, "Failed to update board card.")
}

pub fn delete_card(board: &str, card: &str) -> Request {
    delete(card_path(board, card), "Failed to delete board card.")
}

pub fn archive_card(board: &str, card: &str) -> Request {
    send("POST", format!("{}/archive", card_path(board, card)), json!({}), "Failed to archive board card.")
}

/// `moveBoardCard`: `position` is the card's **index** in the target lane (the server clamps it), not a position number.
pub fn move_card(board: &str, card: &str, lane: &str, index: usize) -> Request {
    send("POST", format!("{}/move", card_path(board, card)), json!({ "laneId": lane, "position": index }), "Failed to move board card.")
}

// ─── the calls, typed ───────────────────────────────────────────────────────
//
// Each runs on the caller's thread and answers what `lib/board-api.ts`'s function resolves to, or its error's message.

pub fn fetch_boards(base: &str) -> Result<Vec<Board>, String> {
    list_boards().send_as(base, |v| list_of(v, Board::from_json))
}

pub fn fetch_lanes(base: &str, board: &str) -> Result<Vec<Lane>, String> {
    list_lanes(board).send_as(base, |v| list_of(v, Lane::from_json))
}

pub fn fetch_cards(base: &str, board: &str) -> Result<Vec<Card>, String> {
    list_cards(board).send_as(base, |v| list_of(v, Card::from_json))
}

pub fn fetch_sources(base: &str, board: &str) -> Result<Vec<Source>, String> {
    list_sources(board).send_as(base, |v| list_of(v, Source::from_json))
}

/// A call that answers one board.
pub fn board(base: &str, request: &Request) -> Result<Board, String> {
    request.send_as(base, Board::from_json)
}

/// A call that answers one lane.
pub fn lane(base: &str, request: &Request) -> Result<Lane, String> {
    request.send_as(base, Lane::from_json)
}

/// A call that answers one card.
pub fn card(base: &str, request: &Request) -> Result<Card, String> {
    request.send_as(base, Card::from_json)
}

/// A call that answers one source.
pub fn source(base: &str, request: &Request) -> Result<Source, String> {
    request.send_as(base, Source::from_json)
}

/// `syncBoard`: the counts of one sync.
pub fn run_sync(base: &str, board: &str) -> Result<SyncResult, String> {
    sync(board).send_as(base, SyncResult::from_json)
}

/// A call that answers nothing (a 204): a delete or a lane reorder.
pub fn no_content(base: &str, request: &Request) -> Result<(), String> {
    request.send(base).map(drop)
}
