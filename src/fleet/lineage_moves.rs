//! Moving a fork or a started session out of its parent, or back under it (`stores/lineage-moves.ts`, rows 2.105-2.107, 4.27).
//! The list changes at once and the server is told right away; Undo (the archive's toast) moves it back for six seconds. A
//! refusal puts the list back and says why. The call goes to the machine the session is on.
use super::archive_queue::{UNDO_SECS, toast_title};
use super::groups::{Listing, lineage_origin_of};
use super::{http, store};
use crate::{State, Tx};
use serde_json::{Value, json};

/// `PATCH /api/sessions/{id}/lineage`.
pub fn lineage_path(id: &str) -> String {
    format!("/api/sessions/{}/lineage", crate::conv::image_sniff::encode_component(id))
}

pub fn lineage_body(detached: bool) -> Value {
    json!({ "detached": detached })
}

/// What a refusal says when the server gives no words.
pub const MOVE_FAILED: &str = "Couldn't move the session.";

/// `movableOutOf`: the parent a fork or a started session can be moved out of now; none for a sub-agent's session.
pub fn movable_out_of(l: &Listing) -> Option<String> {
    lineage_origin_of(l).filter(|(_, kind)| *kind != super::groups::LineageKind::Subagent).filter(|_| !l.detached).map(|(id, _)| id)
}

/// `movableBackUnder`: the parent a session the user moved out can go back under.
pub fn movable_back_under(l: &Listing) -> Option<String> {
    if l.detached { lineage_origin_of(l).map(|(id, _)| id) } else { None }
}

/// `messageFor`: `Moved "<title>" out of "<parent>"` and its three kin.
pub fn message_for(title: &str, parent: Option<&str>, detached: bool) -> String {
    let (verb, place) = if detached { ("out of", "its parent") } else { ("back under", "its parent") };
    match parent.map(str::trim).filter(|p| !p.is_empty()) {
        Some(parent) => format!("Moved \"{title}\" {verb} \"{parent}\""),
        None => format!("Moved \"{title}\" {verb} {place}"),
    }
}

/// The move Undo can reverse.
#[derive(Clone, Debug, PartialEq)]
pub struct Pending {
    pub id: String,
    /// Whether it moved out (true) or back under its parent (false); Undo does the other.
    pub detached: bool,
    pub message: String,
    pub until: f64,
}

#[derive(Default)]
pub struct LineageMoves {
    pub pending: Option<Pending>,
    pub error: Option<String>,
}

impl State {
    /// The menu's Move out of / Move back under. A new move clears the one waiting for Undo.
    pub fn move_lineage(&mut self, id: &str, detached: bool, tx: &mut Tx) {
        self.lineage_moves.pending = None;
        self.lineage_moves.error = None;
        let title = toast_title(&self.title_of(id).unwrap_or_default());
        let parent = self.row_copy_of(id).and_then(|s| lineage_origin_of(&s.listing)).and_then(|(parent, _)| self.title_of(&parent));
        let message = message_for(&title, parent.as_deref(), detached);
        let owned = id.to_string();
        self.apply_lineage(id, detached, tx, move |s, tx, ok| {
            if ok {
                s.lineage_moves.pending = Some(Pending { id: owned, detached, message, until: s.clock + UNDO_SECS });
                tx.wake_after(std::time::Duration::from_millis(100));
            }
        });
    }

    /// Undo: the opposite move, which offers nothing further.
    pub fn undo_lineage(&mut self, tx: &mut Tx) {
        let Some(p) = self.lineage_moves.pending.take() else { return };
        self.apply_lineage(&p.id, !p.detached, tx, |_, _, _| {});
    }

    /// Changes the row at once and tells the server; a refusal puts the row back and keeps the words for the toast.
    fn apply_lineage(&mut self, id: &str, detached: bool, tx: &mut Tx, done: impl FnOnce(&mut State, &mut Tx, bool) + Send + 'static) {
        let Some(was) = self.row_copy_of(id).map(|s| s.listing.detached) else {
            done(self, tx, false);
            return;
        };
        self.set_detached(id, detached);
        let Some(link) = self.link_for(id) else {
            done(self, tx, true);
            return;
        };
        let (sent, body, path) = (id.to_string(), lineage_body(detached), lineage_path(id));
        link.off_thread(
            move |base| {
                let (status, text) = http::text_with("PATCH", base, &path, Some(&body))?;
                if (200..300).contains(&status) {
                    Ok(())
                } else if text.trim().is_empty() {
                    // No words from the server: Fleet's own.
                    Err(MOVE_FAILED.to_string())
                } else {
                    Err(store::error_text(status, &text))
                }
            },
            move |s, tx, result| {
                let ok = result.is_ok();
                if let Err(why) = result {
                    s.set_detached(&sent, was);
                    s.lineage_moves.error = Some(if why.is_empty() { MOVE_FAILED.into() } else { why });
                }
                done(s, tx, ok);
                s.list_again(&sent);
            },
        );
    }

    /// The clock's part: lets the Undo go once its time is up. True while there is a drain to draw.
    pub fn keep_lineage(&mut self, now: f64, tx: &mut Tx) -> bool {
        let Some(p) = &self.lineage_moves.pending else { return false };
        if now >= p.until {
            self.lineage_moves.pending = None;
        } else {
            tx.wake_after(std::time::Duration::from_millis(100));
        }
        true
    }
}
