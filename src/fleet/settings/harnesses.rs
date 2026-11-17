//! Settings → Harnesses, the page's own rules (`HarnessesSection.vue`, `lib/harness-display.ts`): what each harness says about
//! itself, which tab shows, the default, the Enabled switch, the harness's own switches, and what the page asks the machine.
//! Every request goes to the machine the Settings switch names (`State::settings_target`), never home by default.
use super::super::new_session::harness_choice::{DEFAULT_HARNESS_PREFERENCE_KEY, HarnessEntry, HarnessSetting, Harnesses};
use super::super::prefs::Prefs;
use super::HOME_MACHINE_KEY;
use crate::asks::rules::PermissionLevel;
use crate::fleet::http;
use crate::fleet::link::now_ms;
use crate::fleet::row_status::{format_date, to_timestamp};
use crate::{State, Tx};
use std::time::Duration;

/// A harness's state, or "disabled" when the user turned it off (`HarnessStatus`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Ready,
    NotInstalled,
    SignInRequired,
    NotWorking,
    UpdateNeeded,
    Disabled,
}

impl Status {
    /// `harnessState`: the state the server sends, else `ready` if it is available and `not-working` if not.
    pub fn of(h: &HarnessEntry, enabled: bool) -> Status {
        if !enabled {
            return Status::Disabled;
        }
        match h.state.as_str() {
            "ready" => Status::Ready,
            "not-installed" => Status::NotInstalled,
            "sign-in-required" => Status::SignInRequired,
            "not-working" => Status::NotWorking,
            "update-needed" => Status::UpdateNeeded,
            _ if h.available => Status::Ready,
            _ => Status::NotWorking,
        }
    }

    /// `harnessStatusLabel`.
    pub fn label(self) -> &'static str {
        match self {
            Status::Ready => "Ready",
            Status::NotInstalled => "Not installed",
            Status::SignInRequired => "Sign-in needed",
            Status::NotWorking => "Not working",
            Status::UpdateNeeded => "Update needed",
            Status::Disabled => "Disabled",
        }
    }
}

/// `harnessDisplay`: the eyebrow and the description, with a plain stand-in for a Fleet that sends no presentation.
pub fn eyebrow_line(h: &HarnessEntry) -> String {
    let eyebrow = h.card.eyebrow.as_deref().unwrap_or("Harness");
    let description = h.card.description.as_deref().unwrap_or("Harness runtime registered by the backend.");
    format!("{eyebrow} \u{b7} {description}")
}

/// `summaryForHarness`.
pub fn summary(h: &HarnessEntry, enabled: bool) -> String {
    if !enabled {
        return "Disabled until enabled by the user.".into();
    }
    if !h.available {
        return h.reason.clone().unwrap_or_else(|| format!("{} is registered but not currently available.", h.display_name));
    }
    // The version is weave-copilot's, the program Fleet runs for GitHub Copilot; the CLI it drives has its own.
    if let (true, Some(version)) = (h.kind == "copilot", &h.version) {
        let cli = h.card.cli_version.as_ref().map(|c| format!(" \u{b7} Copilot CLI {c}")).unwrap_or_default();
        return format!("weave-copilot {version}{cli} is installed and ready.");
    }
    format!("{} is installed and ready.", h.display_name)
}

/// `harnessLocation`: "1.18.30 · /path/to/opencode".
pub fn location(h: &HarnessEntry) -> Option<String> {
    let parts: Vec<&str> = [h.version.as_deref(), h.executable_path.as_deref()].into_iter().flatten().filter(|p| !p.is_empty()).collect();
    (!parts.is_empty()).then(|| parts.join(" \u{b7} "))
}

/// `formatRelative` in words: "just now", "2 minutes ago", "yesterday", then a date from eight weeks (`lib/format/time.ts`).
pub fn relative(at: i64, now: i64, offset_secs: i64) -> String {
    const MINUTE: i64 = 60_000;
    const HOUR: i64 = 60 * MINUTE;
    const DAY: i64 = 24 * HOUR;
    const WEEK: i64 = 7 * DAY;
    let diff = now - at;
    let span = diff.abs();
    if span < MINUTE {
        return "just now".into();
    }
    if span >= 8 * WEEK {
        return format_date(at, now, offset_secs);
    }
    let (value, unit) = if span < HOUR {
        (span / MINUTE, "minute")
    } else if span < DAY {
        (span / HOUR, "hour")
    } else if span < WEEK {
        (span / DAY, "day")
    } else {
        (span / WEEK, "week")
    };
    let words = |n: i64| format!("{n} {unit}{}", if n == 1 { "" } else { "s" });
    match (diff >= 0, value, unit) {
        (true, 1, "day") => "yesterday".into(),
        (false, 1, "day") => "tomorrow".into(),
        (true, 1, "week") => "last week".into(),
        (false, 1, "week") => "next week".into(),
        (true, n, _) => format!("{} ago", words(n)),
        (false, n, _) => format!("in {}", words(n)),
    }
}

/// What the page keeps: the machine it last asked, what is on its way, and the one switch being saved.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HarnessesPage {
    /// The machine the page asked when it opened; a different one under the Settings switch is asked again.
    pub asked: Option<String>,
    /// Checks of every harness that are under way (`isLoading` of `useHarnesses`).
    pub checking: u32,
    /// The harness switch being saved (its preference key), and the last save that failed (`savingSetting`, `settingError`).
    pub saving: Option<String>,
    pub error: Option<(String, String)>,
    /// The machine shown says Fleet's agent browser is off (it is on until the machine says otherwise).
    pub browser_off: bool,
    /// Profiles: the editor that is open, the notice, the reads under way (`HarnessProfilesPanel`).
    pub profiles: super::harness_profiles::Panel,
}

/// A switch is on as saved; unset (or empty), it is what the harness says its default is (`isSettingOn`).
pub fn setting_on(setting: &HarnessSetting, prefs: &Prefs) -> bool {
    match prefs.get(&setting.key) {
        None | Some("") => setting.default,
        Some(saved) => saved == "true",
    }
}

impl State {
    /// The key of the machine the page shows.
    pub fn harness_machine(&self) -> String {
        self.settings.choice.key(&self.machines).to_string()
    }

    /// The machine's preferences as the page reads them.
    pub fn harness_prefs(&self) -> Prefs {
        self.prefs_for(&self.harness_machine())
    }

    /// The harnesses of the machine shown, with its default filled in.
    pub fn harness_list(&self) -> Harnesses {
        self.harness_choice(&self.harness_machine())
    }

    /// Whether a harness is on: the saved switch, else what the server says (`prefsStore.get("{type}.enabled", …)`).
    pub fn harness_enabled(&self, h: &HarnessEntry) -> bool {
        self.harness_prefs().get(&format!("{}.enabled", h.kind)).map_or(h.user_enabled, |v| v == "true")
    }

    /// The tab shown: the one picked (kept while Settings is open), else the default harness's, else the first.
    pub fn harness_tab(&self) -> Option<HarnessEntry> {
        let list = self.harness_list();
        let pick = |kind: &str| list.all.iter().find(|h| h.kind == kind).cloned();
        self.settings.harness.as_deref().and_then(pick).or_else(|| pick(&list.default_type())).or_else(|| list.all.first().cloned())
    }

    /// Picking a tab (`activeHarness`).
    pub fn select_harness_tab(&mut self, kind: &str) {
        self.settings.harness = Some(kind.to_string());
    }

    /// "Checked 2 minutes ago", from the first harness's `checkedAt`; none from a Fleet that checks on every request.
    pub fn harness_checked(&self) -> Option<(String, String)> {
        let at = self.harness_list().all.first()?.card.checked_at.clone()?;
        let (now, offset) = (self.now_ms.unwrap_or_else(now_ms), crate::conv::time::local_offset_secs());
        let ms = to_timestamp(&at, offset)?;
        Some((format!("Checked {}", relative(ms, now, offset)), crate::ui::age::exact(&at, now, offset)))
    }

    /// What "What it may do" says about a harness: the level it runs at, and whether it gets the browser (`HarnessesSection`).
    pub fn harness_may_do(&self, h: &HarnessEntry) -> String {
        if !h.card.supports_permission_levels {
            return format!("Fleet can't hand {} a permission level or the browser: it asks as its own settings say.", h.display_name);
        }
        let prefs = self.harness_prefs();
        let level = PermissionLevel::of(Some(&h.kind), |k| prefs.get(k).map(str::to_string));
        let browser = if h.card.supports_agent_browser && !self.settings.harnesses.browser_off { "" } else { ", without the browser" };
        format!("Runs at {}{browser}.", level.name())
    }

    /// Opening the page asks the machine, as `onMounted` does: every harness is checked again behind the saved list, the
    /// preferences are read, and so is the agent browser's switch. The machine under the Settings switch changing asks again.
    pub fn keep_harnesses_page(&mut self, tx: &mut Tx) -> bool {
        let here = self.rail == crate::Rail::Settings && self.settings.page == crate::fleet::settings_nav::Page::Harnesses;
        if !here {
            return std::mem::take(&mut self.settings.harnesses.asked).is_some();
        }
        let machine = self.harness_machine();
        if self.harness_checked().is_some() {
            // The label's clock runs every 15 s.
            tx.wake_after(Duration::from_secs(15));
        }
        let profiles = self.keep_profiles_panel() | self.limit_profile_name();
        if self.settings.harnesses.asked.as_deref() == Some(machine.as_str()) {
            return profiles;
        }
        self.settings.harnesses.asked = Some(machine.clone());
        self.settings.harnesses.error = None;
        self.settings.harnesses.browser_off = false;
        self.check_harnesses_again();
        self.read_preferences(&machine);
        self.read_browser_switch(&machine);
        true
    }

    /// "Check again": the machine checks every harness again, and the answer is waited for.
    pub fn check_harnesses_again(&mut self) {
        let machine = self.harness_machine();
        if self.feed_base(&machine).is_none() {
            return;
        }
        self.settings.harnesses.checking += 1;
        self.read_harnesses_then(&machine, true, |s| s.settings.harnesses.checking = s.settings.harnesses.checking.saturating_sub(1));
    }

    pub fn harnesses_checking(&self) -> bool {
        self.settings.harnesses.checking > 0
    }

    fn read_browser_switch(&mut self, machine: &str) {
        let key = machine.to_string();
        self.feed_call(
            machine,
            |base| http::text_with("GET", base, "/api/agent-browser/settings", None).ok().filter(|(s, _)| (200..300).contains(s)),
            move |s, _, answer| {
                let Some((_, body)) = answer else { return };
                let enabled = serde_json::from_str::<serde_json::Value>(&body).ok().and_then(|v| v.get("enabled")?.as_bool());
                if let (Some(on), true) = (enabled, s.harness_machine() == key) {
                    s.settings.harnesses.browser_off = !on;
                }
            },
        );
    }

    /// Whether the machine's preferences are still on their way: the page's switches wait for them.
    pub fn harness_prefs_loading(&self) -> bool {
        self.link.is_some() && !self.harness_prefs().fetched
    }

    /// `prefsStore.set` on the machine shown: the value shows at once; a refusal puts the old one back and reads them again,
    /// and `then` hears how it went.
    pub fn save_harness_pref(&mut self, key: &str, value: &str, then: impl FnOnce(&mut State, Result<(), String>) + Send + 'static) {
        let machine = self.harness_machine();
        let before = self.prefs_for(&machine).get(key).map(str::to_string);
        self.set_pref_on(&machine, key, value);
        let (k, v) = (key.to_string(), value.to_string());
        self.feed_call(
            &machine.clone(),
            {
                let (k, v) = (k.clone(), v.clone());
                move |base| http::put_preference(base, &k, &v)
            },
            {
                let machine = machine.clone();
                move |s, _, result| {
                    if result.is_err() {
                        s.roll_back_pref_on(&machine, &k, &v, before);
                        s.read_preferences(&machine);
                    }
                    then(s, result);
                }
            },
        );
    }

    fn set_pref_on(&mut self, machine: &str, key: &str, value: &str) {
        if machine == HOME_MACHINE_KEY {
            self.pickers.prefs.set(key, value);
        } else {
            self.feeds.prefs.entry(machine.to_string()).or_default().set(key, value);
        }
    }

    fn roll_back_pref_on(&mut self, machine: &str, key: &str, value: &str, before: Option<String>) {
        if machine == HOME_MACHINE_KEY {
            self.pickers.prefs.roll_back(key, value, before);
        } else {
            self.feeds.prefs.entry(machine.to_string()).or_default().roll_back(key, value, before);
        }
    }

    /// The Enabled switch (`toggleHarness`): never disabled, writes `{type}.enabled`.
    pub fn toggle_harness(&mut self, kind: &str) {
        let Some(h) = self.harness_list().all.into_iter().find(|h| h.kind == kind) else { return };
        let value = if self.harness_enabled(&h) { "false" } else { "true" };
        self.save_harness_pref(&format!("{kind}.enabled"), value, |_, _| {});
    }

    /// The Default harness select (`makeDefaultHarness`): a harness that is off can't be the default.
    pub fn set_default_harness(&mut self, kind: &str) {
        let Some(h) = self.harness_list().all.into_iter().find(|h| h.kind == kind) else { return };
        if !self.harness_enabled(&h) {
            return;
        }
        self.save_harness_pref(DEFAULT_HARNESS_PREFERENCE_KEY, kind, |_, _| {});
    }

    /// A harness's own switch (`toggleSetting`): one at a time; after it is saved GitHub Copilot's catalog is forgotten (its
    /// HydraFusion switch changes the models it offers) and every harness is listed again.
    pub fn toggle_harness_setting(&mut self, setting: &HarnessSetting) {
        if self.settings.harnesses.saving.is_some() {
            return;
        }
        let machine = self.harness_machine();
        let on = setting_on(setting, &self.harness_prefs());
        self.settings.harnesses.saving = Some(setting.key.clone());
        self.settings.harnesses.error = None;
        let setting = setting.clone();
        let key = setting.key.clone();
        self.save_harness_pref(&key, if on { "false" } else { "true" }, move |s, result| {
            s.settings.harnesses.saving = None;
            match result {
                Ok(()) => {
                    if setting.key.starts_with("copilot.") {
                        s.feeds.catalogs.entry(machine.clone()).or_default().forget_harness("copilot");
                    }
                    s.read_harnesses(&machine, true);
                }
                Err(why) => {
                    let words = if why.is_empty() { format!("Fleet couldn't change {}.", setting.label) } else { why };
                    s.settings.harnesses.error = Some((setting.key.clone(), words));
                }
            }
        });
        // With no Fleet to ask (the invented data) the value is already shown and nothing is on its way.
        if self.link.is_none() {
            self.settings.harnesses.saving = None;
        }
    }
}
