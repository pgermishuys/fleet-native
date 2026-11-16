//! Invented data for Settings → Machines' scenes (component 17, piece J): the machines of `machines::Case::Several`, each with
//! what it says it can run, falcon with its "agents can send it work" setting, and this machine's access (a made-up token).
use super::machines::Case;
use crate::State;
use crate::fleet::machine_actions::{Access, AccessAddress, AccessState};
use crate::fleet::machines::{AgentWork, Info};
use serde_json::json;

/// What a machine says about itself on `GET /api/machine`, with the harnesses it has.
fn info(id: &str, name: &str, level: &str, working: u32, needs_you: u32) -> Info {
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
        self.rail = crate::Rail::Settings;
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

/// An identity with harnesses, for a test that checks a machine that isn't answering shows none of it.
#[cfg(test)]
pub fn info_for_tests(id: &str, name: &str) -> Info {
    info(id, name, "ask", 0, 0)
}
