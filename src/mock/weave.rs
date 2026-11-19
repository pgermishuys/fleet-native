//! The Weave group's invented data (component 16, piece Y): what Fleet says each harness loads of Weave, and the cases a scene
//! draws it in. Package names are the public ones; paths are invented.
use crate::State;
use crate::fleet::settings::harness_weave::{Apply, Detection, Flavor, Install, Notice, NoticeTone, Weave, WeaveView};
use std::collections::BTreeMap;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Case {
    /// Asked, not answered.
    Asking,
    /// Fleet couldn't be asked.
    Failed,
    /// Weave is in no harness's plugin list; Add Weave is offered.
    None,
    /// OpenCode 2 loads Weave and Fleet keeps the config.
    Loaded,
    /// The same, and Weave reads the user's own file.
    Own,
    /// A Weave Legacy too old to read Fleet's folder.
    Outdated,
    /// Weave and Weave Legacy both loaded.
    Both,
    /// A listed Weave that didn't load.
    DidntLoad,
    /// Add Weave under way.
    Adding,
    /// Add Weave answered: loaded.
    NoticeOk,
    /// Added, but the harness didn't load it.
    NoticeWarn,
    /// Add Weave refused.
    NoticeError,
    /// Check again failed with the old answer still shown.
    RecheckError,
    /// Checking again.
    Checking,
}

fn install(flavor: Flavor, package: &str, entry: &str, accepts: bool, error: Option<&str>, added: bool) -> Install {
    Install {
        flavor,
        package: package.into(),
        entry: entry.into(),
        accepts_fleet_config: accepts,
        error: error.map(str::to_string),
        added_by_fleet: added,
    }
}

fn detection(kind: &str, name: &str, installs: Vec<Install>, add_to: Option<&str>) -> Detection {
    Detection {
        harness_type: kind.into(),
        harness_name: name.into(),
        checked: true,
        installs,
        note: None,
        add_to: add_to.map(str::to_string),
    }
}

const V2: &str = "@weaveio/weave-adapter-opencode2";
const ADD_TO: &str = "/home/you/.weave/harnesses/opencode2/config/opencode.json";

fn weave_v2(added: bool) -> Install {
    install(Flavor::Weave, V2, &format!("{V2}@0.2.0-next.3"), true, None, added)
}

impl State {
    /// Settings → Harnesses on OpenCode 2's tab, with the Weave group as `case` has it.
    pub fn show_weave(&mut self, dark: bool, case: Case) {
        self.show_harnesses(dark, crate::mock::harnesses::Case::Mixed, Some("opencode2"));
        let machine = self.harness_machine();
        let none = || detection("opencode2", "OpenCode 2", Vec::new(), Some(ADD_TO));
        let (mine, source_fleet) = match case {
            Case::Asking | Case::Failed => (None, false),
            Case::None | Case::Adding => (Some(none()), false),
            Case::Loaded | Case::NoticeOk | Case::RecheckError | Case::Checking => {
                (Some(detection("opencode2", "OpenCode 2", vec![weave_v2(true)], None)), true)
            }
            Case::Own => (Some(detection("opencode2", "OpenCode 2", vec![weave_v2(false)], None)), false),
            Case::Outdated => (
                Some(detection(
                    "opencode2",
                    "OpenCode 2",
                    vec![install(Flavor::Legacy, "@opencode_weave/weave", "@opencode_weave/weave@0.8.2", false, None, false)],
                    None,
                )),
                false,
            ),
            Case::Both => (
                Some(detection(
                    "opencode2",
                    "OpenCode 2",
                    vec![
                        weave_v2(false),
                        install(Flavor::Legacy, "@opencode_weave/weave", "@opencode_weave/weave@0.9.1", true, None, false),
                    ],
                    None,
                )),
                true,
            ),
            Case::DidntLoad => (
                Some(detection(
                    "opencode2",
                    "OpenCode 2",
                    vec![install(Flavor::Weave, V2, &format!("{V2}@9.9.9"), false, Some("No matching version found"), true)],
                    None,
                )),
                false,
            ),
            Case::NoticeWarn => (
                Some(detection(
                    "opencode2",
                    "OpenCode 2",
                    vec![install(Flavor::Weave, V2, &format!("{V2}@0.2.0-next.3"), true, Some("Plugin timed out while starting"), true)],
                    None,
                )),
                false,
            ),
            Case::NoticeError => (Some(none()), false),
        };
        let mut w = Weave { machine, ..Weave::default() };
        if let Some(mine) = mine {
            let others = detection("claude-code", "Claude Code", Vec::new(), None);
            let mut others = others;
            others.checked = false;
            others.note = Some("Fleet doesn't hand Weave a config in Claude Code yet.".into());
            w.view =
                Some(WeaveView { fleet_source: source_fleet, files: BTreeMap::new(), harnesses: vec![mine, others], apply: None::<Apply> });
        }
        let notice = |text: &str, tone| Some(Notice { text: text.into(), tone });
        let note = match case {
            Case::Failed => {
                w.error = Some("HTTP 500".into());
                None
            }
            Case::RecheckError => {
                w.error = Some("HTTP 500".into());
                None
            }
            Case::Adding => {
                w.changing = Some("opencode2".into());
                None
            }
            Case::Checking => {
                w.loading = true;
                None
            }
            Case::NoticeOk => notice(&format!("Added {V2}@0.2.0-next.3 to {ADD_TO}. OpenCode 2 loaded it."), NoticeTone::Ok),
            Case::NoticeWarn => notice(&format!("Added {V2}@0.2.0-next.3 to {ADD_TO}, but OpenCode 2 didn't load it."), NoticeTone::Warn),
            Case::NoticeError => notice(&format!("Fleet couldn't look up {V2} on npm."), NoticeTone::Error),
            _ => None,
        };
        if let Some(n) = note {
            w.notices.insert("opencode2".into(), n);
        }
        self.settings.harnesses.weave = w;
    }
}
