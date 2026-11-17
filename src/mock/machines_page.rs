//! Invented data for Settings → Machines' scenes (component 17, piece J): the machines of `machines::Case::Several`, each with
//! what it says it can run, falcon with its "agents can send it work" setting, and this machine's access (a made-up token).
use super::machines::Case;
use crate::State;
use crate::fleet::machine_actions::{Access, AccessAddress, AccessState};
use crate::fleet::machines::{AgentWork, Info};
use serde_json::json;

/// What a machine says about itself on `GET /api/machine`, with the harnesses it has.
pub(super) fn info(id: &str, name: &str, level: &str, working: u32, needs_you: u32) -> Info {
    Info::from_json(&json!({
        "id": id, "name": name, "hostName": name, "os": "linux", "version": "0.72.0", "apiVersion": 1, "authMode": "token",
        "remoteReachable": true, "requiresToken": true,
        "capabilities": {
            "permissionLevel": level,
            "harnesses": [
                { "type": "opencode", "name": "OpenCode", "available": true, "enabled": true, "version": "1.18.32" },
                { "type": "claude-code", "name": "Claude Code", "available": true, "enabled": true, "version": "2.1.0" },
                { "type": "pi", "name": "Pi", "available": true, "enabled": false, "version": "0.9.0" },
            ],
            "sessions": { "working": working, "needsYou": needs_you },
        },
    }))
    .expect("an object")
}

/// The token a scene shows. Made up; it opens nothing.
pub const SCENE_TOKEN: &str = "scene-token-4f9a1c7e2b60d835";

impl State {
    /// Settings → Machines with home and the machines of the `Several` set (falcon answers, harbor is down, lantern is being
    /// read). `token` shows this machine's token instead of its dots.
    pub fn show_machines_page(&mut self, dark: bool, token: bool) {
        self.show_machines(Case::Several, dark);
        self.open_settings("machines", None);
        self.machine_page.opened = true;
        self.machine_page.show_token = token;
        self.machines.home = Some(info("home-id", "atlas", "all", 1, 2));
        self.machines.identities.insert("m-falcon".into(), info("m-falcon", "falcon", "edits", 0, 1));
        self.machines.agent_work.insert("m-falcon".into(), AgentWork::Ask);
        self.machines.agent_work.insert("m-harbor".into(), AgentWork::Never);
        self.machines.agent_work.insert("m-lantern".into(), AgentWork::Always);
        self.machine_page.access = AccessState::Loaded(Access {
            token: SCENE_TOKEN.into(),
            token_source: "saved".into(),
            remote_reachable: true,
            requires_token: true,
            addresses: vec![
                AccessAddress { url: "http://atlas.example.test:2113".into(), kind: "hostname".into() },
                AccessAddress { url: "http://192.168.1.20:2113".into(), kind: "lan".into() },
            ],
        });
    }

    /// The cards' ways of being reached: falcon on its local address with Tailscale not answering and four addresses to order, harbor
    /// over SSH with its tunnel down, lantern with one address (piece M).
    pub fn show_machines_page_addresses(&mut self, dark: bool) {
        use crate::fleet::machine::{Address, AddressResult, kind_of};
        self.show_machines_page(dark, false);
        let address = |url: &str, learned: bool, last_result: Option<AddressResult>| Address {
            url: url.into(),
            kind: kind_of(url),
            learned,
            last_result,
            last_tried_at: None,
        };
        for m in &mut self.machines.list {
            match m.id.as_str() {
                "m-falcon" => {
                    m.addresses = vec![
                        address("http://falcon.tail4f2a.ts.net:2113", false, Some(AddressResult::NoAnswer)),
                        address("http://100.88.14.3:2113", true, Some(AddressResult::NoAnswer)),
                        address("http://192.168.1.42:2113", true, Some(AddressResult::Ok)),
                        address("http://falcon:2113", true, None),
                    ];
                    m.base_url = "http://192.168.1.42:2113".into();
                }
                "m-harbor" => {
                    m.ssh_destination = Some("alice@harbor".into());
                    m.tunnel_up = Some(false);
                    m.diagnosis = None;
                }
                "m-lantern" => m.addresses = vec![address(&m.base_url.clone(), false, None)],
                _ => {}
            }
        }
    }

    /// The machine switch of a view, closed or open, with the machines of the `Several` set and the open session on home.
    pub fn show_machine_switch(&mut self, dark: bool, open: bool) {
        self.show_machines(Case::Several, dark);
        self.rail = crate::Rail::Settings;
        self.machines.home = Some(info("home-id", "atlas", "all", 1, 2));
        let mut choice = crate::fleet::machine_choice::MachineChoice::default();
        choice.open = open;
        self.machine_switch_scene = Some(choice);
    }

    /// Home ("atlas") and three machines for a Settings scene, the app on home.
    pub fn show_settings_machines(&mut self, dark: bool) {
        self.show_machines(Case::Several, dark);
        self.machines.home = Some(info("home-id", "atlas", "all", 1, 2));
    }

    /// With no other machine the switch is not drawn.
    pub fn show_machine_switch_home_only(&mut self, dark: bool) {
        self.show_machines(Case::Empty, dark);
        self.machines.list.clear();
        self.machines.others.clear();
        self.rail = crate::Rail::Settings;
        self.machine_switch_scene = Some(Default::default());
    }

    /// Falcon's name in its field, selected, as Rename leaves it.
    pub fn show_machines_page_rename(&mut self, dark: bool) {
        self.show_machines_page(dark, false);
        self.machine_page.rename = rune::TextEdit::new("falcon".to_string());
        self.machine_page.rename.set_selection(0, 6);
        self.machine_page.rename.focused = true;
        self.machine_page.renaming = Some("m-falcon".into());
    }

    /// Harbor's card with its Details open and the New token field under falcon's.
    pub fn show_machines_page_details(&mut self, dark: bool) {
        self.show_machines_page(dark, false);
        self.machine_page.details.push("m-harbor".into());
        self.machine_page.token = rune::TextEdit::new(String::new());
        self.machine_page.token.focused = true;
        self.machine_page.token_for = Some("m-falcon".into());
    }

    /// A card with a refusal under it, and a token that couldn't be replaced.
    pub fn show_machines_page_errors(&mut self, dark: bool) {
        self.show_machines_page(dark, false);
        self.machine_page.errors.insert("m-falcon".into(), "falcon didn't accept that token.".into());
    }
}

/// Which state of the Add a machine form a scene shows.
#[derive(Clone, Copy)]
pub enum AddCase {
    Empty,
    Found,
    Refused,
    Address,
    Token,
    Added,
}

impl State {
    /// The Add a machine form open on the page, in one of its states. The link is an invented one.
    pub fn show_add_machine(&mut self, dark: bool, case: AddCase) {
        use crate::fleet::machine_diagnosis::Diagnosis;
        use crate::fleet::pairing::{Failure, Preview};
        use rune::TextEdit;
        self.show_machines_page(dark, false);
        let form = &mut self.machine_page.add;
        form.open = true;
        let link = "http://100.64.90.72:2113/add-machine#K7QM-R2XD";
        let found =
            Preview { name: "kestrel".into(), os: "linux".into(), version: "0.72.0".into(), base_url: "http://100.64.90.72:2113".into() };
        match case {
            AddCase::Empty => form.link.focused = true,
            AddCase::Found => {
                form.link = TextEdit::new(link.to_string());
                form.found = Some(found);
            }
            AddCase::Refused => {
                form.link = TextEdit::new(link.to_string());
                form.found = Some(found);
                form.error = Some(Failure {
                    message: "That code has expired or was already used. Run fleet node pair on the other machine for a new one.".into(),
                    diagnosis: Diagnosis::from_json(&json!({
                        "code": "pairing-code", "headline": "That code was turned away", "try": "running fleet node pair on kestrel for a new one.",
                        "steps": [
                            { "label": "Name found", "state": "ok" }, { "label": "Machine answers", "state": "ok" },
                            { "label": "Port answers", "state": "ok" }, { "label": "It's Fleet", "state": "ok" },
                            { "label": "Version fits", "state": "ok" },
                            { "label": "Key accepted", "state": "failed", "detail": "The code is unknown, expired or used." },
                        ],
                    })),
                });
            }
            AddCase::Address => {
                form.link = TextEdit::new("100.64.90.72:2113".to_string());
                form.code.focused = true;
                form.looking = true;
            }
            AddCase::Token => {
                form.link = TextEdit::new("100.64.90.72:2113".to_string());
                form.advanced = true;
            }
            AddCase::Added => form.added = Some("kestrel".into()),
        }
        form.mark_looked();
        if matches!(case, AddCase::Added) {
            form.open = false;
        }
    }
}

/// An identity with harnesses, for a test that checks a machine that isn't answering shows none of it.
#[cfg(test)]
pub fn info_for_tests(id: &str, name: &str) -> Info {
    info(id, name, "ask", 0, 0)
}
