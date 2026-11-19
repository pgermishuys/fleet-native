//! Settings → Harnesses → Weave (`HarnessWeaveGroup.vue`, `lib/weave-flavors.ts`, `composables/use-weave-config.ts`): which Weave
//! each harness loads, how Add Weave and Remove change a harness's plugin list, and the one answer every tab shares. The page
//! keeps one [`Weave`], not one per tab, so a notice or a lock survives a tab switch. The config dialog (Edit…) is the next
//! piece's; its button only says it was asked for.
use crate::State;
use crate::fleet::http;
use crate::fleet::link::now_ms;
use serde_json::Value;
use std::collections::BTreeMap;

/// How often Settings reads the config again while a save still waits for busy folders (`APPLY_POLL_MS`).
pub const APPLY_POLL_MS: i64 = 3000;

/// Which Weave: Weave (`config.weave`) or Weave Legacy (`weave-opencode.jsonc`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flavor {
    Weave,
    Legacy,
}

impl Flavor {
    pub const BOTH: [Flavor; 2] = [Flavor::Weave, Flavor::Legacy];

    fn parse(word: &str) -> Flavor {
        if word == "legacy" { Flavor::Legacy } else { Flavor::Weave }
    }
    pub fn name(self) -> &'static str {
        match self {
            Flavor::Weave => "Weave",
            Flavor::Legacy => "Weave Legacy",
        }
    }
    /// Where this Weave reads its own config when Fleet keeps none.
    pub fn own_path(self) -> &'static str {
        match self {
            Flavor::Weave => "~/.weave/config.weave",
            Flavor::Legacy => "~/.config/opencode/weave-opencode.jsonc",
        }
    }
    /// The first version that reads the folder Fleet points it at.
    pub fn minimum(self) -> &'static str {
        match self {
            Flavor::Weave => "0.2.0-next.1",
            Flavor::Legacy => "0.9.0",
        }
    }
}

/// A Weave plugin a harness loaded, or tried to (`WeaveInstall`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Install {
    pub flavor: Flavor,
    pub package: String,
    /// The plugin as the harness listed it: a package with its version, or a `file://` path.
    pub entry: String,
    pub accepts_fleet_config: bool,
    /// Why the harness couldn't load it.
    pub error: Option<String>,
    /// Fleet put this entry in the harness's config (Add Weave), so it can take it out again.
    pub added_by_fleet: bool,
}

/// What Fleet found in one harness (`WeaveHarnessDetection`). `checked` is false when Fleet can't hand it a config.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Detection {
    pub harness_type: String,
    pub harness_name: String,
    pub checked: bool,
    pub installs: Vec<Install>,
    pub note: Option<String>,
    /// The config file Add Weave would write.
    pub add_to: Option<String>,
}

/// How far a save has got in running processes (`WeaveApplyStatus`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Apply {
    pub folders: Vec<(String, bool)>,
    pub error: Option<String>,
}

/// `GET /api/weave` (`WeaveConfigView`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WeaveView {
    /// `source` is "fleet": Weave reads the config Fleet keeps, else the user's own file.
    pub fleet_source: bool,
    pub files: BTreeMap<String, String>,
    pub harnesses: Vec<Detection>,
    pub apply: Option<Apply>,
}

fn text(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).filter(|t| !t.is_empty()).map(str::to_string)
}

impl Install {
    fn parse(v: &Value) -> Install {
        Install {
            flavor: Flavor::parse(v.get("flavor").and_then(Value::as_str).unwrap_or("weave")),
            package: text(v, "package").unwrap_or_default(),
            entry: text(v, "entry").unwrap_or_default(),
            accepts_fleet_config: v.get("acceptsFleetConfig").and_then(Value::as_bool) == Some(true),
            error: text(v, "error"),
            added_by_fleet: v.get("addedByFleet").and_then(Value::as_bool) == Some(true),
        }
    }
}

impl Detection {
    fn parse(v: &Value) -> Option<Detection> {
        Some(Detection {
            harness_type: text(v, "harnessType")?,
            harness_name: text(v, "harnessName").unwrap_or_default(),
            checked: v.get("checked").and_then(Value::as_bool) == Some(true),
            installs: v.get("installs").and_then(Value::as_array).map(|a| a.iter().map(Install::parse).collect()).unwrap_or_default(),
            note: text(v, "note"),
            add_to: text(v, "addTo"),
        })
    }
}

impl WeaveView {
    pub fn parse(v: &Value) -> Option<WeaveView> {
        let files = v.get("files").and_then(Value::as_object)?;
        Some(WeaveView {
            fleet_source: v.get("source").and_then(Value::as_str) == Some("fleet"),
            files: files.iter().filter_map(|(k, t)| Some((k.clone(), t.as_str()?.to_string()))).collect(),
            harnesses: v.get("harnesses").and_then(Value::as_array)?.iter().filter_map(Detection::parse).collect(),
            apply: v.get("apply").filter(|a| a.is_object()).map(|a| Apply {
                folders: a
                    .get("folders")
                    .and_then(Value::as_array)
                    .map(|f| {
                        f.iter()
                            .map(|f| (text(f, "directory").unwrap_or_default(), f.get("reloaded").and_then(Value::as_bool) == Some(true)))
                            .collect()
                    })
                    .unwrap_or_default(),
                error: text(a, "error"),
            }),
        })
    }

    /// `applyPending`: a folder still waits to reload.
    pub fn apply_pending(&self) -> bool {
        self.apply.as_ref().is_some_and(|a| a.folders.iter().any(|(_, reloaded)| !reloaded))
    }

    pub fn detection(&self, harness: &str) -> Option<&Detection> {
        self.harnesses.iter().find(|d| d.harness_type == harness)
    }
}

/// `describeWeaveInstall`: what a harness loads of Weave, in a line, or why there is nothing.
pub fn describe_install(d: &Detection) -> String {
    if !d.checked {
        return d.note.clone().unwrap_or_default();
    }
    if d.installs.is_empty() {
        return match &d.add_to {
            Some(to) => format!("Weave isn't in its plugin list. Add Weave puts it in {to}."),
            None => "Weave isn't in its plugin list.".into(),
        };
    }
    let lines: Vec<String> = d
        .installs
        .iter()
        .map(|i| {
            let entry = match i.entry.strip_prefix("file://") {
                Some(path) => format!("{} \u{b7} local build at {path}", i.package),
                None => i.entry.clone(),
            };
            match &i.error {
                Some(why) => format!("{entry} \u{b7} didn't load: {why}"),
                None => entry,
            }
        })
        .collect();
    lines.join(" \u{b7} ")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Ok,
    Warn,
    Off,
}

/// `weavePill`.
pub fn pill(d: &Detection) -> (Tone, String) {
    if !d.checked {
        return (Tone::Off, "Not yet".into());
    }
    if d.installs.is_empty() {
        return (Tone::Off, "No Weave".into());
    }
    let names = d.installs.iter().map(|i| i.flavor.name()).collect::<Vec<_>>().join(" + ");
    if d.installs.iter().any(|i| i.error.is_some()) {
        return (Tone::Warn, format!("{names} \u{b7} didn't load"));
    }
    if d.installs.iter().all(|i| i.accepts_fleet_config) { (Tone::Ok, names) } else { (Tone::Warn, format!("{names} \u{b7} update")) }
}

/// `weaveAddedByFleet`: Fleet can take Weave out of this harness.
pub fn added_by_fleet(d: &Detection) -> bool {
    d.installs.iter().any(|i| i.added_by_fleet)
}

/// `loadedFlavors`: the Weaves some harness loads, in the order they are shown.
pub fn loaded_flavors(harnesses: &[Detection]) -> Vec<Flavor> {
    Flavor::BOTH.into_iter().filter(|f| harnesses.iter().flat_map(|h| &h.installs).any(|i| i.flavor == *f)).collect()
}

/// `outdatedFlavorsOf`: the Weaves loaded only at a version that can't read Fleet's folder. One that didn't load isn't too old.
pub fn outdated_flavors(harnesses: &[Detection]) -> Vec<Flavor> {
    let loaded: Vec<&Install> = harnesses.iter().flat_map(|h| &h.installs).filter(|i| i.error.is_none()).collect();
    Flavor::BOTH
        .into_iter()
        .filter(|f| loaded.iter().any(|i| i.flavor == *f) && !loaded.iter().any(|i| i.flavor == *f && i.accepts_fleet_config))
        .collect()
}

/// `usableFlavorsOf`: the Weaves Fleet can hand a config to.
pub fn usable_flavors(harnesses: &[Detection]) -> Vec<Flavor> {
    let installs: Vec<&Install> = harnesses.iter().flat_map(|h| &h.installs).collect();
    loaded_flavors(harnesses).into_iter().filter(|f| installs.iter().any(|i| i.flavor == *f && i.accepts_fleet_config)).collect()
}

/// This harness loads both Weaves.
pub fn both_loaded(d: &Detection) -> bool {
    d.installs.iter().any(|i| i.flavor == Flavor::Weave) && d.installs.iter().any(|i| i.flavor == Flavor::Legacy)
}

/// How an Add Weave or Remove ended, as the line under the row says it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoticeTone {
    Ok,
    Warn,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
    pub text: String,
    pub tone: NoticeTone,
}

/// What the page keeps of Weave: the answer, the ask in flight, the failed ask, the plugin change, the notices.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Weave {
    /// The machine this answer is from: another machine under the Settings switch is asked afresh.
    pub machine: String,
    pub view: Option<WeaveView>,
    pub loading: bool,
    pub error: Option<String>,
    /// The harness whose plugin list is being changed: they can share one opencode.json, so one at a time.
    pub changing: Option<String>,
    pub notices: BTreeMap<String, Notice>,
    /// The config dialog (Edit…) and its draft, which stay while it is closed.
    pub config: super::harness_weave_config::Config,
    pub next_poll: Option<i64>,
}

/// What one tab's Weave group draws.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Group {
    /// Fleet hasn't answered: the group holds its place with a spinner.
    Asking,
    /// Asking failed and there is no answer to show: the error row.
    Failed(String),
    /// The harness's detection, from the shared answer.
    Detected(Detection),
    /// The answer has nothing for this harness: no group.
    Absent,
}

impl Weave {
    pub fn group(&self, harness: &str) -> Group {
        match (&self.view, &self.error) {
            (None, None) => Group::Asking,
            (None, Some(why)) => Group::Failed(why.clone()),
            (Some(view), _) => view.detection(harness).cloned().map_or(Group::Absent, Group::Detected),
        }
    }

    /// Fleet hasn't answered or failed: a link to the group waits.
    pub fn pending(&self) -> bool {
        self.view.is_none() && self.error.is_none()
    }
}

impl State {
    /// Called every frame on the Harnesses page. The group asks once when it is first drawn (Fleet: when it scrolls into view),
    /// follows a save that waits for busy folders, and picks a tab for the old `weave` link. Returns whether it changed anything.
    pub fn keep_weave(&mut self, tx: &mut crate::Tx) -> bool {
        let machine = self.harness_machine();
        let mut changed = false;
        if self.settings.harnesses.weave.machine != machine {
            self.settings.harnesses.weave = Weave { machine: machine.clone(), ..Weave::default() };
            changed = true;
        }
        if self.harness_tab().is_none() {
            return changed;
        }
        let ready = self.feed_base(&machine).is_some();
        let w = &self.settings.harnesses.weave;
        if ready && w.pending() && !w.loading {
            self.ask_weave(false);
            changed = true;
        }
        changed |= self.pick_weave_tab();
        changed |= self.settings.harnesses.weave.keep_config();
        let (loading, pending) = {
            let w = &self.settings.harnesses.weave;
            (w.loading, w.view.as_ref().is_some_and(WeaveView::apply_pending))
        };
        if pending && !loading {
            let now = self.now_ms.unwrap_or_else(now_ms);
            let due = self.settings.harnesses.weave.next_poll.unwrap_or(now);
            if now >= due {
                self.settings.harnesses.weave.next_poll = Some(now + APPLY_POLL_MS);
                self.ask_weave(false);
                changed = true;
            }
            tx.wake_after(std::time::Duration::from_millis(APPLY_POLL_MS as u64));
        }
        changed
    }

    /// Leaving the page: a failed ask is forgotten, so coming back asks again (Fleet's state is the page's); the answer stays.
    pub fn forget_weave_error(&mut self) {
        self.settings.harnesses.weave.error = None;
    }

    /// 5.26: the old Weave link names no harness. Once Fleet has answered, the default harness's tab if Weave is asked about
    /// there, else the first harness that is.
    fn pick_weave_tab(&mut self) -> bool {
        if self.settings.anchor.as_deref() != Some("weave") || self.settings.harness.is_some() {
            return false;
        }
        let Some(view) = self.settings.harnesses.weave.view.clone() else { return false };
        let list = self.harness_list();
        let default = list.default_type();
        let pick = if view.detection(&default).is_some() {
            Some(default)
        } else {
            list.all.iter().find(|h| view.detection(&h.kind).is_some()).map(|h| h.kind.clone())
        };
        match pick {
            Some(kind) => {
                self.settings.harness = Some(kind);
                true
            }
            None => false,
        }
    }

    /// `load`: Fleet answers from what the harnesses said last; `redetect` asks them again, which can start one.
    pub fn ask_weave(&mut self, redetect: bool) {
        let machine = self.harness_machine();
        self.settings.harnesses.weave.loading = true;
        let here = machine.clone();
        let sent = self.feed_call(
            &machine,
            move |base| http::weave_read(base, redetect),
            move |s, _, answer| {
                if s.harness_machine() != here {
                    return;
                }
                let w = &mut s.settings.harnesses.weave;
                w.loading = false;
                match answer {
                    Ok((status, body)) if (200..300).contains(&status) => {
                        match serde_json::from_str::<Value>(&body).ok().as_ref().and_then(WeaveView::parse) {
                            Some(view) => {
                                w.error = None;
                                w.show(view);
                            }
                            None => w.error = Some("Couldn't load the Weave config.".into()),
                        }
                    }
                    Ok((status, body)) => w.error = Some(super::harness_profiles::error_words(status, &body)),
                    Err(why) => w.error = Some(why),
                }
            },
        );
        if !sent {
            self.settings.harnesses.weave.loading = false;
        }
    }

    /// Check again.
    pub fn check_weave_again(&mut self) {
        if !self.settings.harnesses.weave.loading {
            self.ask_weave(true);
        }
    }

    /// Add Weave (`add`) or take out what it added: Fleet edits the harness's plugin list and has the harness load the change.
    pub fn change_weave_plugin(&mut self, harness: &str, add: bool) {
        let machine = self.harness_machine();
        let w = &mut self.settings.harnesses.weave;
        if w.changing.is_some() {
            return;
        }
        let name = w.view.as_ref().and_then(|v| v.detection(harness)).map_or_else(|| harness.to_string(), |d| d.harness_name.clone());
        w.notices.remove(harness);
        w.changing = Some(harness.to_string());
        let (kind, here) = (harness.to_string(), machine.clone());
        let sent = self.feed_call(
            &machine,
            {
                let kind = kind.clone();
                move |base| http::weave_plugin(base, &kind, add)
            },
            move |s, _, answer| {
                if s.harness_machine() != here {
                    return;
                }
                let w = &mut s.settings.harnesses.weave;
                w.changing = None;
                let notice = match answer {
                    Ok((status, body)) if (200..300).contains(&status) => {
                        let v: Value = serde_json::from_str(&body).unwrap_or_default();
                        match v.get("config").and_then(WeaveView::parse) {
                            Some(config) => {
                                w.show(config);
                                let loaded = v.get("loaded").and_then(Value::as_bool) == Some(true);
                                let message = text(&v, "message").unwrap_or_default();
                                Notice { text: message, tone: if loaded { NoticeTone::Ok } else { NoticeTone::Warn } }
                            }
                            None => Notice { text: format!("Couldn't change {name}'s plugins."), tone: NoticeTone::Error },
                        }
                    }
                    Ok((status, body)) => Notice { text: super::harness_profiles::error_words(status, &body), tone: NoticeTone::Error },
                    Err(why) => Notice { text: why, tone: NoticeTone::Error },
                };
                w.notices.insert(kind, notice);
            },
        );
        if !sent {
            self.settings.harnesses.weave.changing = None;
        }
    }
}
