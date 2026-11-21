//! What mission control's views read and do (component 23, piece D; `useMissionControl`'s `tiles`, `waiting`, `states`,
//! `summary` and its answers): the tiles of every machine as the feeds last read them, and the actions on a card of Needs you.
//! The views call these; nothing here draws.
use super::requests::{self, AnswerOutcome};
use super::{FINISHED_FOR_MS, FeedStatus, MissionMachine, Summary, Tile, build_tiles, needs_you, summarize};
use crate::State;
use crate::fleet::asks::Reply;
use crate::fleet::link::now_ms;
use crate::fleet::settings::HOME_MACHINE_KEY;
use rune::Tx;
use std::collections::HashSet;

/// The words under a card whose answer didn't go through and said nothing more (`finish`).
pub const DIDNT_GO_THROUGH: &str = "It didn't go through.";

/// Everything the views read, worked out once per frame.
pub struct Wall {
    pub tiles: Vec<Tile>,
    /// The tiles that wait on the user: needs-you or error, in the tiles' order.
    pub waiting: Vec<Tile>,
    pub summary: Summary,
    pub machines: Vec<MachineState>,
    /// Every machine is still being asked for the first time (`allAsked`).
    pub all_asked: bool,
}

/// One machine's feed as the menu and the pages read it (`MissionMachine` without its sessions).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MachineState {
    pub id: String,
    pub status: FeedStatus,
    /// No answer yet and not unreachable: its sessions are still coming, not missing (`asked`).
    pub asking: bool,
}

fn asking(m: &MissionMachine) -> bool {
    m.status != FeedStatus::Unreachable && m.last_heard_at.is_none()
}

impl State {
    pub fn mission_wall(&self) -> Wall {
        let machines = self.mission.machines();
        let archiving: HashSet<String> =
            machines.iter().flat_map(|m| m.sessions.iter()).filter(|s| self.archive.hides(&s.id)).map(|s| s.id.clone()).collect();
        let now = self.now_ms.unwrap_or_else(now_ms);
        let tiles = build_tiles(&machines, now, FINISHED_FOR_MS, &archiving);
        let waiting: Vec<Tile> = needs_you(&tiles).into_iter().cloned().collect();
        let summary = summarize(&tiles);
        let all_asked = !machines.is_empty() && machines.iter().all(asking);
        let machines = machines.iter().map(|m| MachineState { id: m.id.clone(), status: m.status, asking: asking(m) }).collect();
        Wall { tiles, waiting, summary, machines, all_asked }
    }

    /// Sends `request` for a card to its machine; the card's buttons stay off until the machine has answered and been read
    /// again, then it shows what went wrong, if anything (`start`, `finish`).
    fn mission_card_send(&mut self, tile: &Tile, request: requests::Request) {
        let key = tile.key.clone();
        self.mission.busy.insert(key.clone());
        let done = move |s: &mut State, outcome: AnswerOutcome| {
            s.mission.busy.remove(&key);
            let words = if outcome.ok || outcome.gone { String::new() } else { outcome.error.unwrap_or_else(|| DIDNT_GO_THROUGH.into()) };
            s.mission.errors.insert(key, words);
        };
        match requests::machine_key(&self.machines, tile) {
            Ok(machine) => self.mission_send(&machine, request, done),
            // A machine that left the list: say so on the card without sending anything.
            Err(words) => done(self, AnswerOutcome { ok: false, gone: false, error: Some(words.to_string()) }),
        }
    }

    /// A single-choice question answered with the option `label` (`answerQuestion`).
    pub fn mission_answer_question(&mut self, tile: &Tile, request_id: &str, label: &str) {
        if self.mission.busy.contains(&tile.key) {
            return;
        }
        let request = requests::question_answer_request(&tile.session_id, request_id, &[vec![label.to_string()]]);
        self.mission_card_send(tile, request);
    }

    /// A permission answered once, always or rejected, on the session that asked, which may be a sub-agent's.
    pub fn mission_answer_permission(&mut self, tile: &Tile, reply: Reply) {
        let Some(super::Ask::Permission(ask)) = &tile.ask else { return };
        if self.mission.busy.contains(&tile.key) {
            return;
        }
        let request = requests::permission_answer_request(&ask.session_id, &ask.id, reply, None);
        self.mission_card_send(tile, request);
    }

    /// Opens the session of a tile: its machine remembers it first, then it opens in Sessions (`open`).
    pub fn mission_open_tile(&mut self, tile: &Tile, tx: &mut Tx) {
        self.machines.remember_sessions(&tile.machine_id, std::slice::from_ref(&tile.session_id));
        self.rail = crate::Rail::Sessions;
        if tile.machine_id == HOME_MACHINE_KEY {
            self.open_dreaming_session(&tile.session_id, tx);
        } else if let Some(row) = self.mission.session(&tile.machine_id, &tile.session_id) {
            self.open_machine_row(&tile.machine_id, row, tx);
        }
    }
}
