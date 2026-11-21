//! The composer's agent and model chips (rows 10.1-10.3): the harness a run uses, what that harness offers for the run's
//! folder (`useHarnessCatalog`), what Default means there (`describeDefaults`) and the picks. Ports that part of
//! `AutomationComposer.vue`; the chips and their menu are New session's (`view/composer.rs` `selector_chip`,
//! `view/selector.rs`), which ask [`State::automation_selector`] whether they are working on the composer.
use super::composer::ComposerState;
use crate::State;
use crate::composer::choice::{AgentModelChoice, AgentOption, DefaultLabels, ModelOption, describe_defaults, model_from_key};
use crate::fleet::new_session::feed_state::{Ask, CatalogKey};
use crate::fleet::new_session::feeds::parse_catalog;
use crate::fleet::new_session::harness::page_catalog_for_choice;
use crate::fleet::new_session::harness_choice::resolve_default_harness;
use crate::fleet::new_session::host::Host;
use crate::fleet::new_session::request::Folder;

/// The chips' keys (Fleet's test ids).
pub const AGENT_CHIP: &str = "automation-agent";
pub const MODEL_CHIP: &str = "automation-model";

impl State {
    /// The agent and model menus are about the composer: the Automations page is on screen.
    pub fn automation_selector(&self) -> bool {
        self.picker_host() == Host::Automation
    }

    /// The composer being worked on now.
    fn agent_model_composer(&self) -> &ComposerState {
        self.automations_nav.composer(self.composer_editing())
    }

    /// `defaultHarnessType` of the machine the view shows: the user's pick, else the one the server marks, else the first;
    /// empty until the harnesses have loaded.
    pub fn automation_default_harness(&self) -> String {
        let machine = self.automations_machine();
        let all = self.feeds.harnesses.get(&machine).map_or(&[][..], |f| f.harnesses.all.as_slice());
        resolve_default_harness(self.preferred_harness(&machine).as_deref(), all)
    }

    /// `harnessType`: the automation's own, else the default.
    pub fn automation_harness(&self) -> String {
        self.agent_model_composer().harness_type.clone().unwrap_or_else(|| self.automation_default_harness())
    }

    /// What the chips ask the harness for: with a folder (No folder counts), not for a workflow, and once there is a harness.
    /// The directory is the folder's path; a new worktree is a checkout of the repository, so its path stands for it.
    fn automation_catalog_key(&self) -> Option<CatalogKey> {
        let c = self.agent_model_composer();
        let folder = c.folder.as_ref().filter(|_| !c.is_workflow())?;
        let harness = Some(self.automation_harness()).filter(|h| !h.is_empty())?;
        let directory = match folder {
            Folder::None => None,
            Folder::Repository(p) | Folder::Directory(p) => Some(p.clone()),
        };
        Some(CatalogKey { harness, directory, profile: None })
    }

    /// The catalog the chips show: the one asked for (or the previous folder's while it loads).
    fn automation_catalog(&self) -> Option<&crate::fleet::new_session::feeds::Catalog> {
        let feed = &self.automations_nav.composer.catalog;
        feed.wanted.as_ref()?;
        feed.shown.as_ref()
    }

    /// `offersAgentsAndModels && !isWorkflow`: the harness lists agents and models for the run's folder.
    pub fn automation_offers_agent_and_model(&self) -> bool {
        self.automation_catalog_key().is_some() && self.automation_catalog().is_some_and(|c| c.supported)
    }

    fn automation_choice(&self) -> AgentModelChoice {
        let c = self.agent_model_composer();
        AgentModelChoice { agent: c.agent.clone(), model: c.model.clone() }
    }

    /// `describeDefaults`: what Default means in the folder.
    pub fn automation_default_labels(&self) -> DefaultLabels {
        let catalog = self.automation_catalog().filter(|c| c.supported).map(page_catalog_for_choice);
        describe_defaults(catalog.as_ref(), &self.automation_choice())
    }

    pub fn automation_agents(&self) -> Vec<AgentOption> {
        self.automation_catalog().map(|c| c.agents.clone()).unwrap_or_default()
    }

    pub fn automation_models(&self) -> Vec<ModelOption> {
        self.automation_catalog().map(|c| c.models.clone()).unwrap_or_default()
    }

    /// The agent chip: the pick, else what Default is.
    pub fn automation_agent_chip(&self) -> String {
        let agent = self.automation_choice().agent;
        if agent.is_empty() { self.automation_default_labels().agent_label } else { agent }
    }

    /// The model chip: the pick's name (its id when the list lacks it), else what Default is.
    pub fn automation_model_chip(&self) -> String {
        let key = self.automation_choice().model;
        if key.is_empty() {
            return self.automation_default_labels().model_label;
        }
        self.automation_models()
            .iter()
            .find(|m| m.selection_key == key)
            .map(|m| m.name.clone())
            .or_else(|| model_from_key(&key).map(|m| m.model_id))
            .unwrap_or(key)
    }

    /// The id the menu `name` has picked: "" is Default.
    pub fn automation_selected(&self, name: &str) -> String {
        let c = self.agent_model_composer();
        if name == "agent" { c.agent.clone() } else { c.model.clone() }
    }

    /// A row picked in the agent or model menu (`v-model`). The harness is fixed when sent (row 10.3), not here.
    pub fn pick_automation_choice(&mut self, name: &str, id: &str) {
        let editing = self.composer_editing();
        let nav = &mut self.automations_nav;
        let c = if editing { &mut nav.edit } else { &mut nav.draft };
        if name == "agent" {
            c.agent = id.to_string();
        } else {
            c.model = id.to_string();
        }
    }

    /// Asks for the catalog the chips need when it changes (another folder, harness, machine or target): what is cached
    /// shows at once and the harness is asked when it's old or missing. With no server the folder's agents and models are
    /// invented, so scenes come out the same. True when what the chips show changed.
    pub(crate) fn keep_automation_catalog(&mut self) -> bool {
        let want = self.automation_catalog_key();
        if self.automations_nav.composer.catalog.wanted == want {
            return false;
        }
        let machine = self.automations_machine();
        let now = crate::fleet::link::now_ms();
        let mock = self.feed_base(&machine).is_none();
        let feed = &mut self.automations_nav.composer.catalog;
        let Ask::Fetch(seq) = feed.select(want.clone(), now) else { return true };
        if mock {
            feed.finish(seq, now, Ok(crate::mock::feeds::catalog()));
            return true;
        }
        let Some(key) = want else { return true };
        self.feed_call(
            &machine,
            move |base| parse_catalog(crate::fleet::http::harness_catalog(base, &key.harness, key.directory.as_deref(), None)),
            move |s, _, r| {
                s.automations_nav.composer.catalog.finish(seq, crate::fleet::link::now_ms(), r);
            },
        );
        true
    }
}
