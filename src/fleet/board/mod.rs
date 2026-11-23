//! The Board's data (component 23, piece J): the types `lib/board-api.ts` reads from Fleet's `/api/boards` routes
//! (`BoardEndpoints.cs`), read from the JSON the server writes. No view and no state here: `api` makes the calls, `order`
//! sorts and moves, `cards` reads a card (its status, its metadata, its source's name), `sources` reads a source's config.
//! Times stay the server's ISO text, as Fleet keeps them: the sorts compare them as text, which orders ISO times.
pub mod api;
pub mod cards;
pub mod compose;
pub mod drag;
pub mod filters;
pub mod manage;
pub mod order;
pub mod page;
pub mod source_form;
pub mod sources;
pub mod store;

use serde_json::{Value, json};

/// `Board`: one persistent board.
#[derive(Clone, Debug, PartialEq)]
pub struct Board {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub updated_at: String,
}

/// `BoardLane`: a column. `position` is the server's number (1024, 2048, … after a rebalance).
#[derive(Clone, Debug, PartialEq)]
pub struct Lane {
    pub id: String,
    pub board_id: String,
    pub name: String,
    pub position: f64,
    pub is_inbox: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// `BoardSource`: where a board syncs from. `config` is a JSON object written as text (`sources::parse_config` reads it).
#[derive(Clone, Debug, PartialEq)]
pub struct Source {
    pub id: String,
    pub board_id: String,
    pub provider_type: String,
    pub config: String,
    pub last_sync_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// `BoardCard`: a manual card (`source_type` none) or one a source filled (`"github-issue"` for GitHub's sync).
#[derive(Clone, Debug, PartialEq)]
pub struct Card {
    pub id: String,
    pub board_id: String,
    pub lane_id: String,
    pub title: String,
    pub source_type: Option<String>,
    pub source_key: Option<String>,
    /// JSON text for a synced card (`cards::parse_metadata`), anything at all otherwise.
    pub metadata: Option<String>,
    pub position: f64,
    pub archived_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl Card {
    pub fn is_archived(&self) -> bool {
        self.archived_at.is_some()
    }
}

/// `BoardSyncResult`: what one Sync now did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SyncResult {
    pub sources_processed: u64,
    pub issues_fetched: u64,
    pub cards_created: u64,
    pub cards_updated: u64,
    pub cards_marked_stale: u64,
    pub synced_at: String,
}

fn text(v: &Value, key: &str) -> String {
    v.get(key).and_then(Value::as_str).unwrap_or_default().to_string()
}

fn maybe_text(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}

fn number(v: &Value, key: &str) -> f64 {
    v.get(key).and_then(Value::as_f64).unwrap_or(0.0)
}

fn count(v: &Value, key: &str) -> u64 {
    v.get(key).and_then(Value::as_u64).unwrap_or(0)
}

/// A JSON number as the server wrote it: whole numbers without a `.0`, so a body round-trips as it came.
fn json_number(n: f64) -> Value {
    if n.fract() == 0.0 && n.abs() < 9.0e15 { json!(n as i64) } else { json!(n) }
}

fn opt(v: &Option<String>) -> Value {
    v.as_ref().map_or(Value::Null, |s| json!(s))
}

impl Board {
    pub fn from_json(v: &Value) -> Option<Board> {
        Some(Board {
            id: v.get("id")?.as_str()?.to_string(),
            name: text(v, "name"),
            created_at: text(v, "createdAt"),
            updated_at: text(v, "updatedAt"),
        })
    }

    pub fn to_json(&self) -> Value {
        json!({"id": self.id, "name": self.name, "createdAt": self.created_at, "updatedAt": self.updated_at})
    }
}

impl Lane {
    pub fn from_json(v: &Value) -> Option<Lane> {
        Some(Lane {
            id: v.get("id")?.as_str()?.to_string(),
            board_id: text(v, "boardId"),
            name: text(v, "name"),
            position: number(v, "position"),
            is_inbox: v.get("isInbox").and_then(Value::as_bool).unwrap_or(false),
            created_at: text(v, "createdAt"),
            updated_at: text(v, "updatedAt"),
        })
    }

    pub fn to_json(&self) -> Value {
        json!({
            "id": self.id, "boardId": self.board_id, "name": self.name, "position": json_number(self.position),
            "isInbox": self.is_inbox, "createdAt": self.created_at, "updatedAt": self.updated_at,
        })
    }
}

impl Source {
    pub fn from_json(v: &Value) -> Option<Source> {
        Some(Source {
            id: v.get("id")?.as_str()?.to_string(),
            board_id: text(v, "boardId"),
            provider_type: text(v, "providerType"),
            config: text(v, "config"),
            last_sync_at: maybe_text(v, "lastSyncAt"),
            created_at: text(v, "createdAt"),
            updated_at: text(v, "updatedAt"),
        })
    }

    pub fn to_json(&self) -> Value {
        json!({
            "id": self.id, "boardId": self.board_id, "providerType": self.provider_type, "config": self.config,
            "lastSyncAt": opt(&self.last_sync_at), "createdAt": self.created_at, "updatedAt": self.updated_at,
        })
    }
}

impl Card {
    pub fn from_json(v: &Value) -> Option<Card> {
        Some(Card {
            id: v.get("id")?.as_str()?.to_string(),
            board_id: text(v, "boardId"),
            lane_id: text(v, "laneId"),
            title: text(v, "title"),
            source_type: maybe_text(v, "sourceType"),
            source_key: maybe_text(v, "sourceKey"),
            metadata: maybe_text(v, "metadata"),
            position: number(v, "position"),
            archived_at: maybe_text(v, "archivedAt"),
            created_at: text(v, "createdAt"),
            updated_at: text(v, "updatedAt"),
        })
    }

    pub fn to_json(&self) -> Value {
        json!({
            "id": self.id, "boardId": self.board_id, "laneId": self.lane_id, "title": self.title,
            "sourceType": opt(&self.source_type), "sourceKey": opt(&self.source_key), "metadata": opt(&self.metadata),
            "position": json_number(self.position), "archivedAt": opt(&self.archived_at),
            "createdAt": self.created_at, "updatedAt": self.updated_at,
        })
    }
}

impl SyncResult {
    pub fn from_json(v: &Value) -> Option<SyncResult> {
        v.as_object()?;
        Some(SyncResult {
            sources_processed: count(v, "sourcesProcessed"),
            issues_fetched: count(v, "issuesFetched"),
            cards_created: count(v, "cardsCreated"),
            cards_updated: count(v, "cardsUpdated"),
            cards_marked_stale: count(v, "cardsMarkedStale"),
            synced_at: text(v, "syncedAt"),
        })
    }

    pub fn to_json(&self) -> Value {
        json!({
            "sourcesProcessed": self.sources_processed, "issuesFetched": self.issues_fetched,
            "cardsCreated": self.cards_created, "cardsUpdated": self.cards_updated,
            "cardsMarkedStale": self.cards_marked_stale, "syncedAt": self.synced_at,
        })
    }
}

/// A list the server answered: every entry that reads, `None` when the answer isn't a list.
pub fn list_of<T>(v: &Value, read: impl Fn(&Value) -> Option<T>) -> Option<Vec<T>> {
    Some(v.as_array()?.iter().filter_map(read).collect())
}
