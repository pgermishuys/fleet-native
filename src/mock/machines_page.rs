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
            host: "0.0.0.0".into(),
            port: 2113,
            addresses: vec![
                AccessAddress { url: "http://atlas.example.test:2113".into(), kind: "hostname".into() },
                AccessAddress { url: "http://192.168.1.20:2113".into(), kind: "lan".into() },
            ],
        });
        self.show_screens();
    }

    /// "Phones and browsers" in view: home and falcon only, as Fleet's page shows with one other machine. `address`: the "Use my own
    /// address" card open with what a Fleet that listens only on this machine says.
    pub fn show_machines_phones(&mut self, dark: bool, address: bool) {
        self.show_machines_page(dark, false);
        self.machines.list.retain(|m| m.id == "m-falcon");
        if address {
            if let AccessState::Loaded(a) = &mut self.machine_page.access {
                a.addresses.clear();
            }
            let phone = &mut self.machine_data.phone;
            phone.editing_url = true;
            phone.error = Some("Fleet only listens on this machine, so no phone can reach it.".into());
        }
    }

    /// "Phones and browsers": a browser that connected through atlas and a phone through falcon, harbor not answering; the page's
    /// clock fixed so "last seen" reads the same every time.
    pub fn show_screens(&mut self) {
        use crate::fleet::machines::screens::{NotAnswering, Screen, Screens};
        self.now_ms = crate::fleet::work::parse_iso_ms("2026-11-22T12:00:00Z");
        let screen = |id: &str, name: &str, platform: &str, through: &str, added: &str, seen: &str| Screen {
            id: id.into(),
            name: name.into(),
            platform: Some(platform.into()),
            created_at: added.into(),
            last_used_at: seen.into(),
            paired_via: Some(format!("m-{through}")),
            is_machine: false,
            through_name: Some(through.into()),
            fleet_wide: true,
        };
        self.machine_data.screens = Screens {
            screens: vec![
                screen("screen:b1", "Edge on Windows", "web", "atlas", "2026-11-20T09:00:00Z", "2026-11-22T11:58:30Z"),
                screen("screen:p1", "Pixel 8", "android", "falcon", "2026-11-03T18:20:00Z", "2026-11-21T21:10:00Z"),
            ],
            not_answering: vec![NotAnswering { id: "m-harbor".into(), name: "harbor".into() }],
        };
        self.machine_data.screens_loaded = true;
    }

    /// The cards' ways of being reached: falcon on its local address with Tailscale not answering and four addresses to order, harbor
    /// over SSH with its tunnel down, lantern with one address (piece M).
    pub fn show_machines_page_addresses(&mut self, dark: bool) {
        use crate::fleet::machine::{Address, AddressResult, kind_of};
        self.show_machines_page(dark, false);
        self.manage_machine("m-falcon");
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
        self.manage_machine("m-falcon");
    }

    /// A machine's own page (Manage): home's or another machine's.
    pub fn show_machine_page(&mut self, dark: bool, key: &str) {
        self.show_machines_page(dark, false);
        self.manage_machine(key);
    }

    /// Harbor's page with its Details open.
    pub fn show_machines_page_details(&mut self, dark: bool) {
        self.show_machines_page(dark, false);
        self.machine_page.details.push("m-harbor".into());
        self.manage_machine("m-harbor");
    }

    /// Falcon's page with the New token field open and a token typed in, masked.
    pub fn show_machines_page_token_form(&mut self, dark: bool) {
        self.show_machines_page(dark, false);
        self.machine_page.token = rune::TextEdit::new("falcon-scene-token".to_string()).secure();
        self.machine_page.token.focused = true;
        self.machine_page.token_for = Some("m-falcon".into());
        self.manage_machine("m-falcon");
    }

    /// A card with a refusal under it, and a token that couldn't be replaced.
    pub fn show_machines_page_errors(&mut self, dark: bool) {
        self.show_machines_page(dark, false);
        self.machine_page.errors.insert("m-falcon".into(), "falcon didn't accept that token.".into());
        self.manage_machine("m-falcon");
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
        form.mode = crate::fleet::machines::add::AddMode::Link;
        let link = "http://100.64.90.72:6262/add-machine#K7QM-R2XD";
        let found =
            Preview { name: "kestrel".into(), os: "linux".into(), version: "0.72.0".into(), base_url: "http://100.64.90.72:6262".into() };
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
                form.link = TextEdit::new("100.64.90.72:6262".to_string());
                form.code.focused = true;
                form.looking = true;
            }
            AddCase::Token => {
                form.link = TextEdit::new("100.64.90.72:6262".to_string());
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

/// Which state of Add a machine's Found and On a new server tabs a scene shows.
#[derive(Clone, Copy)]
pub enum TabCase {
    FoundEmpty,
    FoundWaiting,
    FoundRefused,
    Server,
    ServerCopied,
    ServerExpired,
    ServerRefused,
    LinkHint,
}

impl State {
    /// Add a machine open on a tab. The waiting machines, addresses and codes are invented; the wall clock is fixed so the
    /// countdown reads the same every time.
    pub fn show_add_machine_tab(&mut self, dark: bool, case: TabCase) {
        use crate::fleet::machines::add::AddMode;
        use crate::fleet::machines::found::{FoundMachine, JoinCode};
        use crate::fleet::pairing::Failure;
        self.show_machines_page(dark, false);
        // 2026-11-19T10:00:00Z.
        self.now_ms = Some(1_795_082_400_000);
        let waiting = |id: &str, name: &str, os: &str, hint: &str, source: &str| FoundMachine {
            machine_id: id.into(),
            name: name.into(),
            os: Some(os.into()),
            base_url: "http://192.168.1.40:6262".into(),
            hint: hint.into(),
            source: source.into(),
            expires_at: "2026-11-19T10:10:00.000Z".into(),
        };
        let code = |expires: &str| JoinCode {
            code: "J8RT-4WXN".into(),
            expires_at: expires.into(),
            addresses: vec!["http://100.64.0.2:6262".into(), "http://192.168.1.20:6262".into()],
            target: "http://100.64.0.2:6262,http://192.168.1.20:6262#J8RT-4WXN".into(),
            machine_name: "studio".into(),
        };
        let form = &mut self.machine_page.add;
        form.open = true;
        form.mode = match case {
            TabCase::FoundEmpty | TabCase::FoundWaiting | TabCase::FoundRefused => AddMode::Found,
            TabCase::LinkHint => AddMode::Link,
            _ => AddMode::Server,
        };
        let server = &mut form.server;
        match case {
            TabCase::FoundEmpty => self.machine_data.found = Some(Vec::new()),
            TabCase::FoundWaiting | TabCase::FoundRefused => {
                self.machine_data.found = Some(vec![
                    waiting("m-atlas", "atlas", "linux", "7K3M", "Tailscale"),
                    waiting("m-quarry", "quarry", "macos", "P4WT", "Home network"),
                ]);
                let mut typed = rune::TextEdit::new("Q9ZD".to_string());
                typed.focused = true;
                form.waiting.rest.insert("m-atlas".into(), typed);
                if matches!(case, TabCase::FoundRefused) {
                    form.waiting.failure.insert(
                        "m-atlas".into(),
                        Failure::said("That code has expired or was already used. Run fleet node up on atlas again."),
                    );
                }
            }
            TabCase::Server | TabCase::ServerCopied => {
                server.code = Some(code("2026-11-19T10:30:00.000Z"));
                server.countdown = "in 30 min".into();
                if matches!(case, TabCase::ServerCopied) {
                    server.copied_until = Some(f64::MAX);
                }
            }
            TabCase::ServerExpired => {
                server.code = Some(code("2026-11-19T09:59:00.000Z"));
                server.expired = true;
                server.countdown = "now".into();
            }
            TabCase::ServerRefused => {
                server.failure = Some(
                    "studio only listens on this machine, so a new server can't reach it. Let others reach it first (Settings → \
                     Machines), then make a join code."
                        .into(),
                )
            }
            TabCase::LinkHint => form.link_hint = Some("quarry".into()),
        }
    }
}
