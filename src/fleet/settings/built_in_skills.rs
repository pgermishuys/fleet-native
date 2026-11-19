//! Settings → Plugins → Built-in skills (`skills/BuiltInSkillsTab.vue`, `composables/use-built-in-skills.ts`,
//! `stores/built-in-skills.ts`): Fleet's own skills, each with a switch, the copy sessions get, and what to do when Fleet changed
//! its version since the user made theirs. Every call goes to the machine Settings shows (`State::settings_target`) on a thread;
//! the answer comes back to the UI thread. The page always reads the list when it opens, even if the conversation already did.
//! Every answer is also put into the conversation's list by name (`State::skills_update`), so a skill's Improve card agrees.
//! With no Fleet to ask (scenes, a run with no server) the invented skills answer and the buttons change them.
use crate::fleet::http;
use crate::fleet::settings_nav::Page;
use crate::fleet::skills::{BuiltInSkill, answer, at, mock_skills, parse_saved, parse_skills};
use crate::{State, Tx};
use serde_json::{Value, json};

pub const PLUGIN_ID: &str = "built-in-skills";

/// Fleet's version as it was when the user's was made, and as it is now (`BuiltInSkillDetail`), for "See what changed".
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FleetChange {
    pub name: String,
    pub before: String,
    pub after: String,
}

/// What the page holds while it is open; leaving it forgets all of it, so each open starts fresh (8.72).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BuiltInPage {
    /// The machine the list was asked of; a different machine reads again.
    pub loaded_for: Option<String>,
    /// Which open this is, so an answer that comes after the page was left or moved is dropped.
    pub seq: u64,
    pub list: Vec<BuiltInSkill>,
    pub loading: bool,
    /// Why the list couldn't be read.
    pub load_error: Option<String>,
    /// The skill whose switch is being saved, and why the last save failed.
    pub saving: Option<String>,
    pub save_error: Option<String>,
    /// The skill a keep or a switch of version is running for (`busyName`), and why the last one failed.
    pub busy: Option<String>,
    pub action_error: Option<String>,
    /// The skill whose Fleet change is open.
    pub fleet_change: Option<FleetChange>,
    /// The skill the Editor or History dialog was asked for (piece AL draws them).
    pub editing: Option<String>,
    pub history: Option<String>,
}

impl BuiltInPage {
    /// `isLoading && skills.length === 0`: the spinner shows only while there is nothing to list.
    pub fn show_loading(&self) -> bool {
        self.loading && self.list.is_empty()
    }

    /// `actionError ?? error`, where `error` is `saveError ?? loadError` (8.71).
    pub fn note(&self) -> Option<&str> {
        self.action_error.as_deref().or(self.save_error.as_deref()).or(self.load_error.as_deref())
    }

    /// `store.update`: the server's answer for one skill goes into the list by name.
    fn update(&mut self, skill: BuiltInSkill) {
        match self.list.iter_mut().find(|s| s.name == skill.name) {
            Some(slot) => *slot = skill,
            None => self.list.push(skill),
        }
    }
}

/// `BuiltInSkillDetail`'s two texts of Fleet's.
fn parse_change(v: &Value) -> Option<(String, String)> {
    let text = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
    Some((text("fleetBefore").unwrap_or_default(), text("fleetContent")?))
}

fn read(fallback: String, got: Result<(u16, String), String>) -> Result<Value, String> {
    answer(&fallback, got)
}

impl State {
    fn built_in_open(&self) -> bool {
        self.rail == crate::Rail::Settings && matches!(&self.settings.page, Page::Plugin(id) if id == PLUGIN_ID)
    }

    fn built_in_link(&self) -> Option<crate::fleet::link::Link> {
        let key = self.settings.choice.key(&self.machines).to_string();
        self.link_for_machine(&key).filter(|_| self.settings_target().is_some())
    }

    /// Whether Settings shows the machine the conversation's own list comes from, so an answer belongs in it too.
    fn built_in_is_home(&self) -> bool {
        match (self.settings_target(), self.link.as_ref()) {
            (Some(target), Some(home)) => target.base == home.base,
            _ => true,
        }
    }

    /// Called from `poll`: while the page is open, reads the list from the machine Settings shows when it hasn't (`store.load()`
    /// on mount); leaving forgets everything.
    pub fn keep_built_in(&mut self, _tx: &mut Tx) -> bool {
        if !self.built_in_open() {
            let had = self.settings.built_in != BuiltInPage::default();
            self.settings.built_in = BuiltInPage::default();
            return had;
        }
        let key = self.settings.choice.key(&self.machines).to_string();
        if self.settings.built_in.loaded_for.as_deref() == Some(key.as_str()) {
            return false;
        }
        let seq = self.settings.built_in.seq + 1;
        self.settings.built_in = BuiltInPage { loaded_for: Some(key), seq, ..Default::default() };
        let Some(link) = self.built_in_link() else {
            self.settings.built_in.list = mock_skills();
            return true;
        };
        self.settings.built_in.loading = true;
        link.off_thread(
            |base| read("Couldn't load Fleet's built-in skills.".into(), http::text_with("GET", base, "/api/skills/built-in", None)),
            move |s: &mut State, _tx: &mut Tx, got: Result<Value, String>| {
                let page = &mut s.settings.built_in;
                if page.seq != seq {
                    return;
                }
                page.loading = false;
                match got {
                    Ok(v) => page.list = parse_skills(&v),
                    Err(why) => page.load_error = Some(why),
                }
            },
        );
        true
    }

    /// The server's answer for one skill goes into the page's list and the conversation's (8.73).
    fn built_in_took(&mut self, skill: BuiltInSkill) {
        if self.built_in_is_home() {
            self.skills_update(skill.clone());
        }
        self.settings.built_in.update(skill);
    }

    /// The switch (`setEnabled`): one change at a time, a second press while one saves is ignored. The list takes the server's
    /// answer, not the click's; on a failure the switch stays and the note says why.
    pub fn built_in_set_enabled(&mut self, name: &str, enabled: bool) {
        if self.settings.built_in.saving.is_some() {
            return;
        }
        let fallback = format!("Couldn't turn {name} {}.", if enabled { "on" } else { "off" });
        self.settings.built_in.save_error = None;
        let Some(link) = self.built_in_link() else {
            if let Some(skill) = self.settings.built_in.list.iter().find(|s| s.name == name).cloned() {
                self.built_in_took(BuiltInSkill { enabled, ..skill });
            }
            return;
        };
        self.settings.built_in.saving = Some(name.to_string());
        let (seq, path, body) = (self.settings.built_in.seq, at(name), json!({"enabled": enabled}));
        link.off_thread(
            move |base| read(fallback, http::text_with("PUT", base, &path, Some(&body))),
            move |s: &mut State, _tx: &mut Tx, got: Result<Value, String>| {
                if s.settings.built_in.seq != seq {
                    return;
                }
                s.settings.built_in.saving = None;
                match got.and_then(|v| parse_saved(&v).ok_or_else(|| "No skill came back.".to_string())) {
                    Ok(skill) => s.built_in_took(skill),
                    Err(why) => s.settings.built_in.save_error = Some(why),
                }
            },
        );
    }

    /// `toggleFleetChange`: opens what Fleet changed for one skill, or closes it; one is open at a time.
    pub fn built_in_toggle_change(&mut self, name: &str) {
        if self.settings.built_in.fleet_change.as_ref().is_some_and(|c| c.name == name) {
            self.settings.built_in.fleet_change = None;
            return;
        }
        self.settings.built_in.action_error = None;
        let Some(link) = self.built_in_link() else {
            // Invented: the same two texts the diff scene shows.
            self.settings.built_in.fleet_change = Some(FleetChange {
                name: name.to_string(),
                before: crate::view::skill_diff::BEFORE.into(),
                after: crate::view::skill_diff::AFTER.into(),
            });
            return;
        };
        let (seq, path, fallback, owner) = (self.settings.built_in.seq, at(name), format!("Couldn't load {name}."), name.to_string());
        link.off_thread(
            move |base| read(fallback, http::text_with("GET", base, &path, None)),
            move |s: &mut State, _tx: &mut Tx, got: Result<Value, String>| {
                if s.settings.built_in.seq != seq {
                    return;
                }
                match got.and_then(|v| parse_change(&v).ok_or_else(|| "No skill came back.".to_string())) {
                    Ok((before, after)) => s.settings.built_in.fleet_change = Some(FleetChange { name: owner, before, after }),
                    Err(why) => s.settings.built_in.action_error = Some(why),
                }
            },
        );
    }

    /// "Keep mine" (`POST …/keep-mine`) and "Use Fleet's" (`PUT …/active` with no version): one at a time. A success clears the
    /// open diff of that skill; a failure says why (`actionError`, which wins over the load error).
    fn built_in_run(&mut self, name: &str, keep: bool) {
        if self.settings.built_in.busy.is_some() {
            return;
        }
        self.settings.built_in.action_error = None;
        let Some(link) = self.built_in_link() else {
            if let Some(skill) = self.settings.built_in.list.iter().find(|s| s.name == name).cloned() {
                let next = if keep {
                    BuiltInSkill { fleet_changed: false, ..skill }
                } else {
                    BuiltInSkill { version: None, fleet_changed: false, ..skill }
                };
                self.built_in_took(next);
                self.built_in_clear_change(name);
            }
            return;
        };
        self.settings.built_in.busy = Some(name.to_string());
        let fallback = if keep { format!("Couldn't keep your version of {name}.") } else { format!("Couldn't switch {name}.") };
        // `run` in the tab shows its own "Couldn't change <name>." only when the request threw without a message.
        let (seq, owner) = (self.settings.built_in.seq, name.to_string());
        let (method, path, body) = if keep {
            ("POST", format!("{}/keep-mine", at(name)), None)
        } else {
            ("PUT", format!("{}/active", at(name)), Some(crate::fleet::skills::switch_body(None)))
        };
        link.off_thread(
            move |base| read(fallback, http::text_with(method, base, &path, body.as_ref())),
            move |s: &mut State, _tx: &mut Tx, got: Result<Value, String>| {
                if s.settings.built_in.seq != seq {
                    return;
                }
                s.settings.built_in.busy = None;
                match got.and_then(|v| parse_saved(&v).ok_or_else(|| format!("Couldn't change {owner}."))) {
                    Ok(skill) => {
                        s.built_in_took(skill);
                        s.built_in_clear_change(&owner);
                    }
                    Err(why) => s.settings.built_in.action_error = Some(why),
                }
            },
        );
    }

    fn built_in_clear_change(&mut self, name: &str) {
        if self.settings.built_in.fleet_change.as_ref().is_some_and(|c| c.name == name) {
            self.settings.built_in.fleet_change = None;
        }
    }

    pub fn built_in_keep_mine(&mut self, name: &str) {
        self.built_in_run(name, true);
    }

    pub fn built_in_use_fleets(&mut self, name: &str) {
        self.built_in_run(name, false);
    }

    /// "Edit": the Editor dialog (piece AL) opens for this skill.
    pub fn built_in_edit(&mut self, name: &str) {
        self.settings.built_in.editing = Some(name.to_string());
    }

    /// "History": the History dialog (piece AL) opens for this skill.
    pub fn built_in_history(&mut self, name: &str) {
        self.settings.built_in.history = Some(name.to_string());
    }
}
