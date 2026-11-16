//! What the harness, profile, agent and model chips show and do on the New session page (`NewSessionComposer.vue`'s harness,
//! profile and catalog parts, `HarnessPicker.vue`, `ProfilePicker.vue`, `lib/harness-profile.ts`; rows A10, B6, B7). Plain
//! state changes: nothing here draws. The lists come from piece C's reads (`feed_calls.rs`); this decides which chips there are,
//! what they say, and keeps the draft's choices in step with what the folder's harness offers.
use super::feed_state::CatalogKey;
use super::feeds::{Catalog, Profile};
use super::harness_choice::Harnesses;
use super::request::{Folder, Workspace};
use crate::State;
use crate::composer::choice::{AgentModelChoice, describe_defaults, keep_offered, model_from_key};
use crate::fleet::settings::HOME_MACHINE_KEY;
use serde_json::{Map, Value};

/// The profile id that means "no profile: my own opencode.json as it is" (`NO_PROFILE`).
pub const NO_PROFILE: &str = "none";

pub const HARNESS_POPOVER: &str = "harness";
pub const PROFILE_POPOVER: &str = "profile";
pub const HARNESS_CHIP: &str = "new-session-harness";
pub const PROFILE_CHIP: &str = "new-session-profile";

// ─── profileSummary ─────────────────────────────────────────────────────────────

/// `parseJsonc`: JSON with comments and trailing commas, as opencode.json allows. `None` when it isn't an object.
pub fn parse_jsonc(text: &str) -> Option<Map<String, Value>> {
    match serde_json::from_str::<Value>(&strip_jsonc(text)).ok()? {
        Value::Object(map) => Some(map),
        _ => None,
    }
}

/// Takes out `//` and `/* */` comments outside strings, then a comma before a closing bracket.
fn strip_jsonc(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let (mut i, mut in_string) = (0, false);
    while i < chars.len() {
        let (c, next) = (chars[i], chars.get(i + 1).copied());
        if in_string {
            out.push(c);
            if c == '\\' {
                if let Some(n) = next {
                    out.push(n);
                }
                i += 1;
            } else if c == '"' {
                in_string = false;
            }
        } else if c == '"' {
            in_string = true;
            out.push(c);
        } else if c == '/' && next == Some('/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            out.push('\n');
        } else if c == '/' && next == Some('*') {
            i += 2;
            while i < chars.len() && !(chars[i] == '*' && chars.get(i + 1) == Some(&'/')) {
                i += 1;
            }
            i += 1;
        } else {
            out.push(c);
        }
        i += 1;
    }
    drop_trailing_commas(&out)
}

/// `.replace(/,(\s*[}\]])/g, "$1")`, but not inside a string.
fn drop_trailing_commas(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let (mut in_string, mut i) = (false, 0);
    while i < chars.len() {
        let c = chars[i];
        if in_string {
            out.push(c);
            if c == '\\' {
                if let Some(&n) = chars.get(i + 1) {
                    out.push(n);
                }
                i += 1;
            } else if c == '"' {
                in_string = false;
            }
        } else if c == '"' {
            in_string = true;
            out.push(c);
        } else if c == ',' && chars[i + 1..].iter().find(|c| !c.is_whitespace()).is_some_and(|c| matches!(c, '}' | ']')) {
            // dropped
        } else {
            out.push(c);
        }
        i += 1;
    }
    out
}

/// `profileSummary`: one line on what an OpenCode profile changes, for the picker's rows.
pub fn profile_summary(content: &str) -> String {
    let Some(config) = parse_jsonc(content) else { return "Can't read this profile".into() };
    let count = |v: Option<&Value>| v.and_then(Value::as_object).map_or(0, Map::len);
    let plural = |n: usize, word: &str| format!("{n} {word}{}", if n == 1 { "" } else { "s" });
    let mcp = match config.get("mcp") {
        Some(Value::Object(o)) if o.contains_key("servers") => o.get("servers"),
        other => other,
    };
    let providers = count(config.get("provider")) + count(config.get("providers"));
    let agents = count(config.get("agent")) + count(config.get("agents"));
    let list = |key: &str| -> Option<String> {
        config.get(key).and_then(Value::as_array).map(|a| a.iter().map(js_string).collect::<Vec<_>>().join(", "))
    };

    let mut parts: Vec<String> = Vec::new();
    if let Some(model) = config.get("model").and_then(Value::as_str) {
        parts.push(model.into());
    }
    if let Some(agent) = config.get("default_agent").and_then(Value::as_str) {
        parts.push(format!("default agent {agent}"));
    }
    if providers > 0 {
        parts.push(plural(providers, "provider"));
    }
    if count(mcp) > 0 {
        parts.push(plural(count(mcp), "MCP server"));
    }
    if agents > 0 {
        parts.push(plural(agents, "agent"));
    }
    if let Some(only) = list("enabled_providers") {
        parts.push(format!("only {only}"));
    }
    if let Some(off) = list("disabled_providers") {
        parts.push(format!("turns off {off}"));
    }
    if parts.is_empty() {
        let settings = config.keys().filter(|k| *k != "$schema").count();
        parts.push(if settings == 0 { "No settings yet".into() } else { plural(settings, "setting") });
    }
    parts.join(" · ")
}

/// How `Array.join` writes an element: a string as it is, anything else as JSON.
fn js_string(v: &Value) -> String {
    v.as_str().map_or_else(|| v.to_string(), str::to_string)
}

// ─── the page ───────────────────────────────────────────────────────────────────

/// What the page remembers about the chips, so it asks and applies once per change rather than on every look.
#[derive(Default)]
pub struct ChipState {
    profiles_asked: Option<(String, String)>,
    /// The folder and harness the draft's agent and model were last taken from the folder's remembered choice for.
    remembered_for: Option<(Folder, String)>,
}

/// What a session is created with from the chips (`harnessType`, `harnessProfileId`, `agent`, `model` of the request).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SentChoices {
    pub harness_type: Option<String>,
    /// Only while the profile chip shows; otherwise the server's default applies (B6.12).
    pub harness_profile_id: Option<String>,
    pub agent: Option<String>,
    pub model: Option<(String, String)>,
}

impl State {
    pub(crate) fn harnesses_here(&self) -> Harnesses {
        self.harness_choice(&self.target_key())
    }

    /// `resolvedHarnessType`: the harness the draft really uses.
    pub fn resolved_harness(&self) -> String {
        let draft = self.new_draft.draft().map(|d| d.harness_type.clone()).unwrap_or_default();
        self.harnesses_here().resolved_type(&draft)
    }

    /// B7.1: only with a choice.
    pub fn shows_harness_picker(&self) -> bool {
        self.harnesses_here().shows_picker()
    }

    /// B7.2: the display name, the type id when the list lacks it.
    pub fn harness_chip_label(&self) -> String {
        let draft = self.new_draft.draft().map(|d| d.harness_type.clone()).unwrap_or_default();
        self.harnesses_here().all.iter().find(|h| h.kind == draft).map_or(draft, |h| h.display_name.clone())
    }

    pub fn harness_names(&self) -> Vec<(String, String)> {
        self.harnesses_here().enabled().into_iter().map(|h| (h.kind.clone(), h.display_name.clone())).collect()
    }

    pub fn harness_menu_open(&self) -> bool {
        self.ui.open == Some(crate::ui::Open::Popover(HARNESS_POPOVER))
    }

    pub fn profile_menu_open(&self) -> bool {
        self.ui.open == Some(crate::ui::Open::Popover(PROFILE_POPOVER))
    }

    pub fn open_harness_menu(&mut self) {
        self.ui.open = Some(crate::ui::Open::Popover(HARNESS_POPOVER));
    }

    pub fn open_profile_menu(&mut self) {
        self.ui.open = Some(crate::ui::Open::Popover(PROFILE_POPOVER));
    }

    pub fn close_chip_menu(&mut self) {
        if self.harness_menu_open() || self.profile_menu_open() {
            self.ui.open = None;
        }
    }

    /// `update:modelValue` of the harness picker: the draft's harness; the menu closes.
    pub fn choose_harness(&mut self, kind: &str) {
        if let Some(d) = self.new_draft.draft_mut() {
            d.harness_type = kind.to_string();
        }
        self.close_chip_menu();
        self.settle_choices();
    }

    fn supports_profiles(&self) -> bool {
        let resolved = self.resolved_harness();
        self.harnesses_here().enabled().iter().any(|h| h.kind == resolved && h.supports_profiles)
    }

    /// The harness's profiles on this machine, none when it has no profiles to offer (B6.1).
    pub fn page_profiles(&self) -> Vec<Profile> {
        if !self.supports_profiles() {
            return Vec::new();
        }
        self.feeds.profiles_of(&self.target_key(), &self.resolved_harness()).to_vec()
    }

    pub fn shows_profile_picker(&self) -> bool {
        !self.page_profiles().is_empty()
    }

    /// `selectedProfileId`: the picked one, else the default, else none (B6.10).
    pub fn selected_profile_id(&self) -> String {
        let profiles = self.page_profiles();
        let picked = self.new_draft.draft().and_then(|d| d.harness_profile_id.clone());
        match picked {
            Some(p) if p == NO_PROFILE || profiles.iter().any(|x| x.id == p) => p,
            _ => profiles.iter().find(|p| p.is_default).map_or_else(|| NO_PROFILE.to_string(), |p| p.id.clone()),
        }
    }

    /// B6.2: the profile's name; "No profile" for the sentinel or an id nothing has.
    pub fn profile_chip_label(&self) -> String {
        let id = self.selected_profile_id();
        self.page_profiles().into_iter().find(|p| p.id == id).map_or_else(|| "No profile".into(), |p| p.name)
    }

    pub fn choose_profile(&mut self, id: &str) {
        if let Some(d) = self.new_draft.draft_mut() {
            d.harness_profile_id = Some(id.to_string());
        }
        self.close_chip_menu();
        self.settle_choices();
    }

    /// The folder the harness is asked about (A10.7): an existing worktree as it is, otherwise the folder itself (a new
    /// worktree is a checkout of it); none for a quick chat.
    fn catalog_directory(&self) -> Option<String> {
        let d = self.new_draft.draft()?;
        match (d.folder.as_ref()?, &d.workspace) {
            (Folder::None, _) => None,
            (Folder::Repository(_), Workspace::Existing(path)) => Some(path.clone()),
            (Folder::Repository(p) | Folder::Directory(p), _) => Some(p.clone()),
        }
    }

    /// What the page asks the harness for: nothing until a folder is settled, and the profile only while its chip shows.
    pub fn catalog_key(&self) -> Option<CatalogKey> {
        let d = self.new_draft.draft()?;
        d.folder.as_ref()?;
        let harness = self.resolved_harness();
        if harness.is_empty() {
            return None;
        }
        let profile = self.shows_profile_picker().then(|| self.selected_profile_id());
        Some(CatalogKey { harness, directory: self.catalog_directory(), profile })
    }

    /// The catalog on screen. It may be the previous folder's while another loads (`page_catalog_is_current`).
    pub fn page_catalog(&self) -> Option<&Catalog> {
        self.feeds.catalogs.get(&self.target_key()).and_then(|f| f.shown.as_ref())
    }

    fn page_catalog_is_current(&self) -> bool {
        self.feeds.catalogs.get(&self.target_key()).is_some_and(|f| f.is_current())
    }

    /// A9.9: the agent and model chips show only when the harness can list them here.
    pub fn offers_agent_and_model(&self) -> bool {
        self.page_catalog().is_some_and(|c| c.supported)
    }

    /// A10.14: sent unless the current catalog says the harness can't take them.
    pub fn sends_agent_and_model(&self) -> bool {
        !(self.page_catalog_is_current() && self.page_catalog().is_some_and(|c| !c.supported))
    }

    fn draft_choice(&self) -> AgentModelChoice {
        self.new_draft.draft().map(|d| AgentModelChoice { agent: d.agent.clone(), model: d.model.clone() }).unwrap_or_default()
    }

    /// A10.10: what Default means for the folder.
    pub fn page_default_labels(&self) -> crate::composer::choice::DefaultLabels {
        let catalog = self.page_catalog().filter(|c| c.supported).map(page_catalog_for_choice);
        describe_defaults(catalog.as_ref(), &self.draft_choice())
    }

    pub fn page_agents(&self) -> Vec<crate::composer::choice::AgentOption> {
        self.page_catalog().map(|c| c.agents.clone()).unwrap_or_default()
    }

    pub fn page_models(&self) -> Vec<crate::composer::choice::ModelOption> {
        self.page_catalog().map(|c| c.models.clone()).unwrap_or_default()
    }

    /// The agent chip: the pick, else what Default is.
    pub fn page_agent_chip(&self) -> String {
        let agent = self.draft_choice().agent;
        if agent.is_empty() { self.page_default_labels().agent_label } else { agent }
    }

    /// The model chip: the pick's name (its id when the list lacks it), else what Default is.
    pub fn page_model_chip(&self) -> String {
        let key = self.draft_choice().model;
        if key.is_empty() {
            return self.page_default_labels().model_label;
        }
        self.page_models()
            .iter()
            .find(|m| m.selection_key == key)
            .map(|m| m.name.clone())
            .or_else(|| model_from_key(&key).map(|m| m.model_id))
            .unwrap_or(key)
    }

    /// A10.11: picking either counts as the person's choice, so the folder's remembered ones stop replacing them.
    pub fn pick_page_agent(&mut self, id: &str) {
        if let Some(d) = self.new_draft.draft_mut() {
            d.agent = id.to_string();
            d.has_chosen_agent_or_model = true;
        }
    }

    pub fn pick_page_model(&mut self, key: &str) {
        if let Some(d) = self.new_draft.draft_mut() {
            d.model = key.to_string();
            d.has_chosen_agent_or_model = true;
        }
    }

    /// What the request is built from the chips (A10.6, A10.14).
    pub fn sent_choices(&self) -> SentChoices {
        let Some(d) = self.new_draft.draft() else { return SentChoices::default() };
        let sends = self.sends_agent_and_model();
        SentChoices {
            harness_type: Some(self.resolved_harness()).filter(|h| !h.is_empty()),
            harness_profile_id: self.shows_profile_picker().then(|| self.selected_profile_id()),
            agent: (sends && !d.agent.is_empty()).then(|| d.agent.clone()),
            model: if sends { model_from_key(&d.model).map(|m| (m.provider_id, m.model_id)) } else { None },
        }
    }

    /// Reads what the chips need when the page opens or the machine changes, and starts the settling.
    pub fn read_harness_feeds(&mut self) {
        let key = self.target_key();
        self.chips = ChipState::default();
        self.read_harnesses(&key, false);
        self.read_preferences(&key);
        // The folder menu's repositories are read on mount (`useRepositories`); nothing else asks for them.
        self.read_repositories(&key, false);
        self.settle_choices();
    }

    /// The watches of `NewSessionComposer.vue` that keep the draft in step with the harness list, the profiles and the
    /// folder's catalog (A10.3, A10.5, A10.12, A10.13). Cheap and idempotent: called after every change that could matter.
    pub fn settle_choices(&mut self) {
        if !self.on_new_session_page() || self.new_draft.draft().is_none() {
            return;
        }
        let key = self.target_key();
        // A stored harness that is no longer enabled follows the resolved one.
        let harnesses = self.harnesses_here();
        let draft_type = self.new_draft.draft().map(|d| d.harness_type.clone()).unwrap_or_default();
        if !harnesses.enabled().iter().any(|h| h.kind == draft_type) {
            let resolved = harnesses.resolved_type(&draft_type);
            if let Some(d) = self.new_draft.draft_mut() {
                d.harness_type = resolved;
            }
        }
        let resolved = self.resolved_harness();
        if self.supports_profiles() {
            let asked = Some((key.clone(), resolved.clone()));
            if self.chips.profiles_asked != asked {
                self.chips.profiles_asked = asked;
                self.read_profiles(&key, &resolved);
            }
        }
        // Each folder starts with the agent and model last used there, until the person picks their own.
        if let Some(d) = self.new_draft.draft()
            && let Some(folder) = d.folder.clone()
            && !d.has_chosen_agent_or_model
            && self.chips.remembered_for.as_ref() != Some(&(folder.clone(), resolved.clone()))
        {
            let remembered = self.sidebar.new_session_defaults(&key).choice_for(&folder, &resolved);
            self.chips.remembered_for = Some((folder, resolved));
            if let Some(d) = self.new_draft.draft_mut() {
                d.agent = remembered.agent;
                d.model = remembered.model;
            }
        }
        self.ask_catalog(&key);
        // An agent or model the folder doesn't offer goes back to Default.
        if self.page_catalog_is_current()
            && let Some(catalog) = self.page_catalog().filter(|c| c.supported)
        {
            let kept = keep_offered(&self.draft_choice(), &page_catalog_for_choice(catalog));
            if let Some(d) = self.new_draft.draft_mut() {
                d.agent = kept.agent;
                d.model = kept.model;
            }
        }
    }

    fn ask_catalog(&mut self, key: &str) {
        let want = self.catalog_key();
        if self.feeds.catalogs.get(key).is_some_and(|f| f.wanted == want) {
            return;
        }
        if !self.is_mock() {
            self.select_catalog(key, want);
            return;
        }
        // With no server the folder's agents and models are invented, so scenes and tests come out the same.
        let now = crate::fleet::link::now_ms();
        let feed = self.feeds.catalogs.entry(key.to_string()).or_default();
        if let super::feed_state::Ask::Fetch(seq) = feed.select(want, now) {
            feed.finish(seq, now, Ok(crate::mock::feeds::catalog()));
        }
    }

    /// `fixNoHarness`: the wizard sets a harness up on this machine; another machine's Settings do it there, so the click opens Harnesses.
    pub fn fix_no_harness(&mut self, _tx: &mut crate::Tx) {
        if self.new_draft.machine().is_none() {
            self.open_harness_setup();
            return;
        }
        self.open_settings("harnesses", None);
    }

    /// What the notice above the box says when no harness can start a session, and the words of its action.
    pub fn no_harness_notice(&self) -> Option<(String, String)> {
        let reason = self.harnesses_here().no_harness_reason()?;
        let action =
            if self.new_draft.machine().is_some() { format!("Open {}'s Settings", self.target_name()) } else { "Set up a harness".into() };
        Some((reason, action))
    }
}

/// The folder's catalog as the choice rules read it (`describeDefaults`, `keepOffered`).
fn page_catalog_for_choice(c: &Catalog) -> crate::composer::choice::Catalog {
    let mut catalog = crate::fleet::catalog::catalog_of(&c.agents, &c.models);
    catalog.default_agent = c.default_agent.clone();
    catalog.default_model = c.default_model.clone();
    catalog
}

// ─── scenes ─────────────────────────────────────────────────────────────────────

/// A state of the four chips for the pictures. The data is invented: two harnesses, two profiles, the mock folder's agents.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scene {
    /// The chips in the box's toolbar, no menu open.
    Chips,
    Harness,
    Profile,
    Agent,
    Model,
    /// No harness can start a session.
    NoHarness,
}

impl State {
    pub(crate) fn show_chip_scene(&mut self, scene: Scene) {
        let key = self.target_key();
        self.feeds.profiles.insert(
            (key, "opencode2".into()),
            vec![
                Profile { id: "work".into(), name: "Work".into(), content: "{ \"model\": \"anthropic/sonnet\" }".into(), is_default: true },
                Profile {
                    id: "local".into(),
                    name: "Local".into(),
                    content: "{\n // a model on this machine\n \"provider\": { \"ollama\": {} },\n \"mcp\": { \"docs\": {} },\n \"enabled_providers\": [\"ollama\"],\n}".into(),
                    is_default: false,
                },
            ],
        );
        if scene == Scene::NoHarness
            && let Some(feed) = self.feeds.harnesses.get_mut(HOME_MACHINE_KEY)
        {
            for h in &mut feed.harnesses.all {
                h.available = false;
                h.reason = Some("OpenCode 2 isn't installed: Fleet couldn't find it on this machine.".into());
            }
        }
        self.open_new_session_page(None);
        self.set_folder(Folder::Repository(crate::mock::feeds::ROCKET.into()), true);
        match scene {
            Scene::Harness => self.open_harness_menu(),
            Scene::Profile => self.open_profile_menu(),
            Scene::Agent => self.open_selector("agent"),
            Scene::Model => self.open_selector("model"),
            Scene::Chips | Scene::NoHarness => {}
        }
    }
}
