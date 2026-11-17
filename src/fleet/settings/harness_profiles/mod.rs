//! Settings → Harnesses → Profiles (`HarnessProfilesPanel.vue`, `stores/harness-profiles.ts`, `lib/harness-profile.ts`): a
//! harness's profiles, the default, and the requests that make, change and remove them. A profile written here is the one the
//! new-session box's chip lists: both read `feeds.profiles` (B5.158). The editor in a row is piece I's; this holds the state
//! that says which one is open.
mod editor;
mod jsonc;
#[cfg(test)]
mod tests;

pub use editor::{Busy, Draft, MAX_NAME_CHARS, UNTITLED, saved_words};
pub use jsonc::{NEW_PROFILE_CONTENT, parse_jsonc, profile_summary};

use crate::State;
use crate::fleet::http;
use crate::fleet::new_session::feeds::Profile;
use serde_json::Value;
use std::collections::HashSet;

/// Which editor is open: a profile's id, or a new one (optionally a copy of another: its name and content).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Editing {
    Edit(String),
    New(Option<(String, String)>),
}

/// What the panel keeps: the editor that is open, the line it says after one closes, a failed change of the default, and which
/// harness's profiles are being read or couldn't be.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Panel {
    /// The harness the three below belong to; another harness's tab shows none of them (each tab is its own panel in Fleet).
    pub harness: String,
    pub editing: Option<Editing>,
    /// The open editor's form: its two boxes, what it is busy with and what the harness said.
    pub draft: Option<Draft>,
    pub notice: Option<String>,
    pub list_error: Option<String>,
    asking: HashSet<(String, String)>,
    failed: HashSet<(String, String)>,
}

/// Where a harness's list is: asked and not answered, not readable, or answered (`loadState`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Load {
    Loading,
    Failed,
    Loaded,
}

// A text edit is equal to itself.
impl Eq for Panel {}

impl Panel {
    /// The panel as `harness`'s tab shows it: the editor and the lines are `harness`'s own, or nothing.
    pub fn of(&self, harness: &str) -> (Option<&Editing>, Option<&str>, Option<&str>) {
        if self.harness == harness {
            (self.editing.as_ref(), self.notice.as_deref(), self.list_error.as_deref())
        } else {
            (None, None, None)
        }
    }

    /// A scene's list that couldn't be read.
    pub fn mark_failed(&mut self, key: (String, String)) {
        self.failed.insert(key);
    }

    fn mine(&mut self, harness: &str) {
        if self.harness != harness {
            *self = Panel {
                harness: harness.to_string(),
                asking: std::mem::take(&mut self.asking),
                failed: std::mem::take(&mut self.failed),
                ..Panel::default()
            };
        }
    }
}

/// "No open sessions" / "Used by 2 open sessions".
pub fn usage(open: u32) -> String {
    match open {
        0 => "No open sessions".into(),
        n => format!("Used by {n} open {}", if n == 1 { "session" } else { "sessions" }),
    }
}

/// What a refused request says (`errorMessage`): the body's `error`, `message` or `detail`, else `HTTP n`.
pub fn error_words(status: u16, body: &str) -> String {
    let said = serde_json::from_str::<Value>(body).ok().and_then(|b| {
        ["error", "message", "detail"].iter().find_map(|k| b.get(k).and_then(Value::as_str).filter(|t| !t.is_empty()).map(str::to_string))
    });
    said.unwrap_or_else(|| format!("HTTP {status}"))
}

/// The list after `id` became the default, or none became it.
pub fn with_default(list: &[Profile], id: Option<&str>) -> Vec<Profile> {
    list.iter().map(|p| Profile { is_default: Some(p.id.as_str()) == id, ..p.clone() }).collect()
}

/// The list with `made` on its end (`create`).
pub fn with_created(list: &[Profile], made: Profile) -> Vec<Profile> {
    list.iter().cloned().chain(std::iter::once(made)).collect()
}

/// The list with the profile `id` replaced by what the machine answered (`update`).
pub fn with_updated(list: &[Profile], id: &str, now: Profile) -> Vec<Profile> {
    list.iter().map(|p| if p.id == id { now.clone() } else { p.clone() }).collect()
}

/// The list without `id` (`remove`).
pub fn without(list: &[Profile], id: &str) -> Vec<Profile> {
    list.iter().filter(|p| p.id != id).cloned().collect()
}

/// What the harness said of a profile it tried (`HarnessProfileCheck`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Check {
    pub ok: bool,
    pub error: Option<String>,
    pub details: Option<String>,
}

impl Check {
    fn parse(status: u16, body: &str) -> Check {
        if !(200..300).contains(&status) {
            return Check { ok: false, error: Some(error_words(status, body)), details: None };
        }
        let v: Value = serde_json::from_str(body).unwrap_or_default();
        let text = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
        // Fleet sends the details as a list of lines; one line as a string is read too.
        let details = match v.get("details") {
            Some(Value::Array(lines)) => Some(lines.iter().filter_map(Value::as_str).collect::<Vec<_>>().join("\n")),
            Some(Value::String(line)) => Some(line.clone()),
            _ => None,
        }
        .filter(|d| !d.is_empty());
        Check { ok: v.get("ok").and_then(Value::as_bool) == Some(true), error: text("error"), details }
    }
}

type Then<T> = Box<dyn FnOnce(&mut State, Result<T, String>) + Send>;

impl State {
    /// The profiles of `harness` on the machine the page shows; none until read.
    pub fn harness_profiles(&self, harness: &str) -> Vec<Profile> {
        self.feeds.profiles_of(&self.harness_machine(), harness).to_vec()
    }

    /// `loadState`: no list yet is loading, until the machine failed to give one.
    pub fn profiles_load(&self, harness: &str) -> Load {
        let key = (self.harness_machine(), harness.to_string());
        if self.feeds.profiles.contains_key(&key) {
            Load::Loaded
        } else if self.settings.harnesses.profiles.failed.contains(&key) {
            Load::Failed
        } else {
            Load::Loading
        }
    }

    /// Reads the shown harness's profiles when its panel is first built (`onMounted`), once; a failure waits for a retry.
    /// Returns whether it asked.
    pub fn keep_profiles_panel(&mut self) -> bool {
        let Some(h) = self.harness_tab().filter(|h| h.supports_profiles) else { return false };
        let key = (self.harness_machine(), h.kind.clone());
        let panel = &self.settings.harnesses.profiles;
        if self.feeds.profiles.contains_key(&key) || panel.asking.contains(&key) || panel.failed.contains(&key) {
            return false;
        }
        self.load_profiles(&h.kind)
    }

    /// `loadProfiles`: a failed load is asked again, and says "Loading" meanwhile.
    pub fn load_profiles(&mut self, harness: &str) -> bool {
        let machine = self.harness_machine();
        let key = (machine.clone(), harness.to_string());
        if self.settings.harnesses.profiles.asking.contains(&key) {
            return false;
        }
        let kind = harness.to_string();
        let asked = self.read_profiles_then(&machine, harness, move |s, ok| {
            let panel = &mut s.settings.harnesses.profiles;
            panel.asking.remove(&key);
            if !ok {
                panel.failed.insert(key);
            }
        });
        if asked {
            let panel = &mut self.settings.harnesses.profiles;
            panel.failed.remove(&(machine.clone(), kind.clone()));
            panel.asking.insert((machine, kind));
        }
        asked
    }

    /// An editor opens: the line and the error above the list are cleared (`edit`).
    pub fn open_profile_editor(&mut self, harness: &str, editing: Editing) {
        let panel = &mut self.settings.harnesses.profiles;
        panel.mine(harness);
        let profile = match &editing {
            Editing::Edit(id) => self.harness_profiles(harness).into_iter().find(|p| &p.id == id),
            Editing::New(_) => None,
        };
        let panel = &mut self.settings.harnesses.profiles;
        panel.draft = Some(Draft::open(&editing, profile.as_ref().map(|p| (p.name.as_str(), p.content.as_str()))));
        panel.editing = Some(editing);
        panel.notice = None;
        panel.list_error = None;
    }

    /// An editor closes, and says `message` where the notice goes (`closeEditor`).
    pub fn close_profile_editor(&mut self, harness: &str, message: Option<String>) {
        let panel = &mut self.settings.harnesses.profiles;
        panel.mine(harness);
        panel.editing = None;
        panel.draft = None;
        if message.is_some() {
            panel.notice = message;
        }
    }

    /// `makeDefault`: the profile `id` (or none) is what new sessions start with.
    pub fn set_profile_default(&mut self, harness: &str, id: Option<&str>) {
        let machine = self.harness_machine();
        let name = id.and_then(|id| self.harness_profiles(harness).into_iter().find(|p| p.id == id)).map(|p| p.name);
        self.settings.harnesses.profiles.mine(harness);
        self.settings.harnesses.profiles.list_error = None;
        let (kind, wanted) = (harness.to_string(), id.map(str::to_string));
        let asked = wanted.clone();
        let sent = self.feed_call(
            &machine.clone(),
            {
                let kind = kind.clone();
                move |base| http::profile_set_default(base, &kind, asked.as_deref())
            },
            move |s, _, answer| {
                s.settings.harnesses.profiles.mine(&kind);
                match answer {
                    Ok((status, _)) if (200..300).contains(&status) => {
                        let list = with_default(s.feeds.profiles_of(&machine, &kind), wanted.as_deref());
                        s.feeds.profiles.insert((machine, kind), list);
                        s.settle_choices();
                        s.settings.harnesses.profiles.notice = Some(match name {
                            Some(name) => format!("New sessions start with {name}."),
                            None => "New sessions start with no profile.".into(),
                        });
                    }
                    Ok((status, body)) => s.settings.harnesses.profiles.list_error = Some(error_words(status, &body)),
                    Err(_) => s.settings.harnesses.profiles.list_error = Some("Couldn't change the default.".into()),
                }
            },
        );
        if !sent {
            self.settings.harnesses.profiles.list_error = Some("Couldn't change the default.".into());
        }
    }

    /// `create`: the machine tries the profile with the harness first and refuses it with the harness's reason.
    pub fn create_profile(
        &mut self,
        harness: &str,
        name: &str,
        content: &str,
        then: impl FnOnce(&mut State, Result<Profile, String>) + Send + 'static,
    ) {
        let (kind, name, content) = (harness.to_string(), name.to_string(), content.to_string());
        let call = {
            let kind = kind.clone();
            move |base: &str| http::profile_create(base, &kind, &name, &content)
        };
        self.profile_write(call, Box::new(then), move |s, machine, made| {
            let list = with_created(s.feeds.profiles_of(machine, &kind), made);
            s.feeds.profiles.insert((machine.to_string(), kind.clone()), list);
        });
    }

    /// `update`: the profile `id` as the machine answers it (with `openSessions`).
    pub fn update_profile(
        &mut self,
        harness: &str,
        id: &str,
        name: &str,
        content: &str,
        then: impl FnOnce(&mut State, Result<Profile, String>) + Send + 'static,
    ) {
        let (kind, id, name, content) = (harness.to_string(), id.to_string(), name.to_string(), content.to_string());
        let call = {
            let (kind, id) = (kind.clone(), id.clone());
            move |base: &str| http::profile_update(base, &kind, &id, &name, &content)
        };
        self.profile_write(call, Box::new(then), move |s, machine, now| {
            let list = with_updated(s.feeds.profiles_of(machine, &kind), &id, now);
            s.feeds.profiles.insert((machine.to_string(), kind.clone()), list);
        });
    }

    /// `remove`.
    pub fn delete_profile(&mut self, harness: &str, id: &str, then: impl FnOnce(&mut State, Result<(), String>) + Send + 'static) {
        let (kind, id) = (harness.to_string(), id.to_string());
        let machine = self.harness_machine();
        let (call_kind, call_id) = (kind.clone(), id.clone());
        let sent = self.feed_call(
            &machine.clone(),
            move |base| http::profile_delete(base, &call_kind, &call_id),
            move |s, _, answer| match answer {
                Ok((status, _)) if (200..300).contains(&status) => {
                    let list = without(s.feeds.profiles_of(&machine, &kind), &id);
                    s.feeds.profiles.insert((machine, kind), list);
                    s.settle_choices();
                    then(s, Ok(()));
                }
                Ok((status, body)) => then(s, Err(error_words(status, &body))),
                Err(why) => then(s, Err(why)),
            },
        );
        let _ = sent;
    }

    /// `check`: the harness tries `content` without saving it. A refusal is an answer (`ok: false`), not an error.
    pub fn check_profile(&mut self, harness: &str, content: &str, then: impl FnOnce(&mut State, Result<Check, String>) + Send + 'static) {
        let (kind, content) = (harness.to_string(), content.to_string());
        let machine = self.harness_machine();
        self.feed_call(
            &machine,
            move |base| http::profile_check(base, &kind, &content),
            move |s, _, answer| then(s, answer.map(|(status, body)| Check::parse(status, &body))),
        );
    }

    /// A write that answers with the profile: `keep` puts it in the list, the chip hears of it, then `then` is told.
    fn profile_write(
        &mut self,
        call: impl FnOnce(&str) -> Result<(u16, String), String> + Send + 'static,
        then: Then<Profile>,
        keep: impl FnOnce(&mut State, &str, Profile) + Send + 'static,
    ) {
        let machine = self.harness_machine();
        let here = machine.clone();
        self.feed_call(&machine, call, move |s, _, answer| {
            let result = match answer {
                Ok((status, body)) if (200..300).contains(&status) => {
                    serde_json::from_str::<Value>(&body).ok().and_then(|v| Profile::parse(&v)).ok_or_else(|| format!("HTTP {status}"))
                }
                Ok((status, body)) => Err(error_words(status, &body)),
                Err(why) => Err(why),
            };
            if let Ok(profile) = &result {
                keep(s, &here, profile.clone());
                s.settle_choices();
            }
            then(s, result);
        });
    }
}
