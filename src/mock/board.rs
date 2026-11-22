//! An invented board for the Board's tests and, later, its scenes: four lanes (the first the inbox), manual cards, a card
//! with GitHub's fixture look (`sourceType` "github", as Fleet's own tests have it), a card a real sync wrote
//! (`github-issue`, FB1), a stale one, an archived one, and two sources. No real repository, person or project.
use crate::fleet::board::{Board, Card, Lane};

pub const BOARD_ID: &str = "board-kite";

pub fn board() -> Board {
    Board {
        id: BOARD_ID.into(),
        name: "Kite launch".into(),
        created_at: "2026-11-02T09:00:00Z".into(),
        updated_at: "2026-11-02T09:00:00Z".into(),
    }
}

fn lane(id: &str, name: &str, position: f64, is_inbox: bool) -> Lane {
    Lane {
        id: id.into(),
        board_id: BOARD_ID.into(),
        name: name.into(),
        position,
        is_inbox,
        created_at: "2026-11-02T09:00:00Z".into(),
        updated_at: "2026-11-02T09:00:00Z".into(),
    }
}

pub fn lanes() -> Vec<Lane> {
    vec![
        lane("lane-inbox", "Inbox", 1024.0, true),
        lane("lane-doing", "In progress", 2048.0, false),
        lane("lane-review", "Review", 3072.0, false),
        lane("lane-done", "Done", 4096.0, false),
    ]
}

/// A manual card in `lane` at `position`.
pub fn card(id: &str, lane: &str, title: &str, position: f64) -> Card {
    Card {
        id: id.into(),
        board_id: BOARD_ID.into(),
        lane_id: lane.into(),
        title: title.into(),
        source_type: None,
        source_key: None,
        metadata: None,
        position,
        archived_at: None,
        created_at: "2026-11-03T10:00:00Z".into(),
        updated_at: "2026-11-03T10:00:00Z".into(),
    }
}

pub fn cards() -> Vec<Card> {
    let github = Card {
        source_type: Some("github".into()),
        source_key: Some("github:acme-labs/kite#42".into()),
        metadata: Some(
            r#"{"number":42,"state":"open","labels":["bug","priority:high"],"assignee":"kite-dev","html_url":"https://github.example/acme-labs/kite/issues/42","updated_at":"2026-11-04T08:00:00Z"}"#
                .into(),
        ),
        ..card("card-github", "lane-doing", "Tail wobbles in a crosswind", 2048.0)
    };
    let synced = Card {
        source_type: Some("github-issue".into()),
        source_key: Some("github:acme-labs/kite#57".into()),
        metadata: Some(r#"{"number":57,"state":"open","labels":[],"assignee":null,"html_url":"https://github.example/acme-labs/kite/issues/57","updated_at":"2026-11-04T09:00:00Z"}"#.into()),
        ..card("card-synced", "lane-inbox", "Spool line snags on rewind", 2048.0)
    };
    let stale = Card {
        source_type: Some("github".into()),
        source_key: Some("github:acme-labs/kite#12".into()),
        metadata: Some(r#"{"number":12,"state":"closed","labels":["docs"],"stale":true}"#.into()),
        ..card("card-stale", "lane-review", "Write the field guide", 1024.0)
    };
    let archived = Card { archived_at: Some("2026-11-05T12:00:00Z".into()), ..card("card-archived", "lane-done", "Pick a fabric", 1024.0) };
    vec![
        card("card-sketch", "lane-inbox", "Sketch the frame", 1024.0),
        github,
        card("card-spar", "lane-doing", "Cut the spars", 1024.0),
        synced,
        stale,
        card("card-colours", "lane-done", "Choose the colours", 2048.0),
        archived,
    ]
}

/// Only the tests read them until the sources' scenes do (piece P).
#[cfg(test)]
pub fn sources() -> Vec<crate::fleet::board::Source> {
    let source = |id: &str, config: &str, last_sync_at: Option<&str>| crate::fleet::board::Source {
        id: id.into(),
        board_id: BOARD_ID.into(),
        provider_type: "github".into(),
        config: config.into(),
        last_sync_at: last_sync_at.map(str::to_string),
        created_at: "2026-11-02T09:30:00Z".into(),
        updated_at: "2026-11-02T09:30:00Z".into(),
    };
    vec![
        source("source-kite", r#"{"repository":"acme-labs/kite","labels":"bug, priority:high"}"#, Some("2026-11-04T09:00:00Z")),
        source("source-spool", r#"{"owner":"acme-labs","repo":"spool","assignee":"@me"}"#, None),
    ]
}

/// What a scene of the Board's page shows (component 23, piece L).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Case {
    /// The first load hasn't answered.
    Loading,
    /// A Fleet with no board: "No lanes yet".
    Empty,
    /// The first load failed.
    Failed,
    /// Loaded, and Sync now has just answered: the toast.
    Synced,
}

impl crate::State {
    /// The Board shown with its switch on: the load answers as `case` says (the first poll starts it).
    pub fn show_board(&mut self, case: Case, dark: bool) {
        use crate::fleet::board::page::Feedback;
        use crate::fleet::board::store::MockAnswers;
        self.dark = Some(dark);
        self.pickers.prefs.adopt(&serde_json::json!({ "features.board.enabled": "true" }));
        self.board.mock_answers = match case {
            Case::Loading => MockAnswers::Silent,
            Case::Empty => MockAnswers::Empty,
            Case::Failed => MockAnswers::Failed,
            Case::Synced => MockAnswers::Data,
        };
        if case == Case::Synced {
            let result =
                crate::fleet::board::SyncResult { cards_created: 2, cards_updated: 1, cards_marked_stale: 0, ..Default::default() };
            self.board.page.feedback = Some((Feedback::of(&result), f64::MAX));
        }
        self.enter_labs(crate::Rail::Board);
    }
}
