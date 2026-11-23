//! What the session's files were as each turn ended (`useTurnStats`, `GET /api/sessions/{id}/turn-stats`), and the session
//! diff's counts (`useFileStats`), for the calls that wrote a whole file: the folded turn's "edited N" counts them
//! (`withFileStats` in `ActivityStream.vue`).
use super::http;
use super::turns_tab::file_stats;
use super::work::parse_iso_ms;
use crate::State;
use crate::conv::fold::{FileStats, TurnStatsSnapshot};
use crate::conv::message_list::FileStat;
use serde_json::Value;

/// The snapshots of one session, and what they were read for.
#[derive(Debug, Default)]
pub struct TurnStats {
    session: String,
    snapshots: Vec<TurnStatsSnapshot>,
    /// `(session, working)` the last read was made for: Fleet reads again as the session starts and stops working.
    asked_for: Option<(String, bool)>,
    request: u64,
}

impl TurnStats {
    pub fn snapshots_of(&self, session: &str) -> &[TurnStatsSnapshot] {
        if self.session == session { &self.snapshots } else { &[] }
    }
}

/// `{ snapshots: [{ endedAt, files: [{ file, additions, deletions }] }] }`; a snapshot with no readable time is left out.
pub fn parse_snapshots(body: &Value) -> Vec<TurnStatsSnapshot> {
    let Some(list) = body.get("snapshots").and_then(Value::as_array) else { return Vec::new() };
    list.iter()
        .filter_map(|snapshot| {
            let ended_at = snapshot.get("endedAt").and_then(Value::as_str).and_then(parse_iso_ms)?;
            let files: FileStats = snapshot
                .get("files")
                .and_then(Value::as_array)
                .map(|files| {
                    files
                        .iter()
                        .filter_map(|f| {
                            let path = f.get("file")?.as_str()?.to_string();
                            let count = |k: &str| f.get(k).and_then(Value::as_u64).unwrap_or(0) as u32;
                            Some((path, FileStat { additions: count("additions"), deletions: count("deletions"), status: None }))
                        })
                        .collect()
                })
                .unwrap_or_default();
            Some(TurnStatsSnapshot { ended_at, files })
        })
        .collect()
}

impl State {
    /// The session diff's counts, when the open session's list is the one loaded and available.
    pub fn conversation_file_stats(&self) -> Option<FileStats> {
        let session = self.open_id()?;
        file_stats(self, &session)
    }

    /// Reads the turn snapshots when a call wrote a whole file, and again as the session starts or stops working
    /// (`refreshOn: isStreaming`). Called by `refold`.
    pub(crate) fn keep_turn_stats(&mut self, has_whole_file_write: bool, working: bool) {
        let Some(id) = self.open_id() else { return };
        if !has_whole_file_write {
            return;
        }
        let key = (id.clone(), working);
        if self.turn_stats.asked_for.as_ref() == Some(&key) {
            return;
        }
        let Some(link) = self.link_for(&id) else { return };
        if self.turn_stats.session != id {
            self.turn_stats = TurnStats { session: id.clone(), ..Default::default() };
        }
        self.turn_stats.asked_for = Some(key);
        self.turn_stats.request += 1;
        let (mine, asked) = (self.turn_stats.request, id.clone());
        link.off_thread(
            move |base| http::turn_stats(base, &asked),
            move |s, _, result| {
                if s.turn_stats.session != id || s.turn_stats.request != mine {
                    return;
                }
                if let Ok((status, body)) = result
                    && (200..300).contains(&status)
                {
                    s.turn_stats.snapshots = parse_snapshots(&body);
                    s.refold();
                }
            },
        );
    }
}
