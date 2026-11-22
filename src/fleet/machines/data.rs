//! What Settings → Machines keeps from these requests while the page is open, and the calls that fill it. Each call is made on a
//! thread, on home's link, and the answer reaches the state through `Link::off_thread`; nothing here is drawn yet (the pieces
//! after it are the views).
use super::found::{FoundMachine, JoinCode, load_found, make_join_code};
use super::reach::{ReachMode, ReachState};
use super::screens::{self, Screens};
use super::{found, reach};
use crate::{State, Tx};

#[derive(Default)]
pub struct Data {
    /// Machines waiting to be added; None until looked for.
    pub found: Option<Vec<FoundMachine>>,
    pub join_code: JoinState,
    pub screens: Screens,
    /// The list was asked for at least once.
    pub screens_loaded: bool,
    /// Who can reach the desktop app; None until read, or when this Fleet can't say.
    pub reach: Option<ReachState>,
    /// What the last change of reach said when the server refused it.
    pub reach_error: Option<String>,
    /// What the last Remove said when the server refused it.
    pub screen_error: Option<String>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub enum JoinState {
    #[default]
    Idle,
    Making,
    Ready(JoinCode),
    Failed(String),
}

impl State {
    /// Looks for waiting machines (`loadFound`).
    pub fn look_for_found(&mut self, _tx: &mut Tx) {
        let Some(link) = self.link.clone() else {
            self.machine_data.found.get_or_insert_with(Vec::new);
            return;
        };
        link.off_thread(load_found, |s, _, machines| s.machine_data.found = Some(machines));
    }

    /// Makes a join code on home for a new server (`makeJoinCode`).
    pub fn make_join_code(&mut self, _tx: &mut Tx) {
        let Some(link) = self.link.clone() else { return };
        self.machine_data.join_code = JoinState::Making;
        link.off_thread(make_join_code, |s, _, result| {
            s.machine_data.join_code = match result {
                Ok(code) => JoinState::Ready(code),
                Err(why) => JoinState::Failed(why),
            };
        });
    }

    /// Reads the phones and browsers that have keys (`refreshDevices`).
    pub fn load_screens(&mut self, _tx: &mut Tx) {
        let Some(link) = self.link.clone() else { return };
        link.off_thread(screens::load, |s, _, list| {
            s.machine_data.screens = list;
            s.machine_data.screens_loaded = true;
        });
    }

    /// Removes a phone or browser on every machine, then reads the list again.
    pub fn remove_screen(&mut self, id: &str, _tx: &mut Tx) {
        let Some(link) = self.link.clone() else { return };
        self.machine_data.screen_error = None;
        let id = id.to_string();
        link.off_thread(
            move |base| screens::remove(base, &id).map(|()| screens::load(base)),
            |s, _, result| match result {
                Ok(list) => s.machine_data.screens = list,
                Err(why) => s.machine_data.screen_error = Some(why),
            },
        );
    }

    /// Reads who can reach the desktop app (`useMachineReach.load`). Without an answer the page behaves as for a Fleet that can't
    /// change it.
    pub fn load_reach(&mut self, _tx: &mut Tx) {
        self.read_reach();
    }

    /// `load_reach` for a caller with no `Tx` (the page's visit, run from `poll`).
    pub fn read_reach(&mut self) {
        let Some(link) = self.link.clone() else { return };
        link.off_thread(reach::load, |s, _, result| s.machine_data.reach = result.ok().flatten());
    }

    /// Changes who can reach the desktop app (`useMachineReach.set`); a refusal keeps the old state and says why.
    pub fn set_reach(&mut self, mode: ReachMode, own_address: Option<String>, _tx: &mut Tx) {
        let Some(link) = self.link.clone() else { return };
        self.machine_data.reach_error = None;
        link.off_thread(
            move |base| reach::save(base, mode, own_address.as_deref()),
            |s, _, result| match result {
                Ok(state) => s.machine_data.reach = Some(state),
                Err(why) => s.machine_data.reach_error = Some(why),
            },
        );
    }
}

pub use found::LOOK_EVERY;
