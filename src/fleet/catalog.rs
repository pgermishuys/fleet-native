//! What the composer's pickers offer and hold (docs/parity/05-composer.md, piece D): the open session's agents and
//! models (`use-agents.ts`, `use-models.ts`), the harness list (`use-harnesses.ts`) and the preferences, the pick
//! kept per session, the effort toggle's state, and what a sent prompt carries. The parsers are plain functions;
//! the loads run on threads (`Link::off_thread`) and every result comes back through a `State` method here.
use super::events::{Event, Kind};
use super::http::{self, Pick};
use super::prefs::Prefs;
use crate::State;
use crate::composer::choice::{
    AgentModelChoice, AgentOption, Catalog, CatalogAgent, CatalogProvider, DefaultLabels, ModelOption, ModelReference,
    SessionDefaultsInput, describe_session_defaults, keep_offered, model_from_key, plain_default_label, session_default_model_key,
};
use crate::composer::drafts::Choice;
use crate::composer::effort::{self, EffortMemory, effort_label, effort_preference_key, effort_to_send, next_effort};
use crate::composer::status::{Caps, HarnessInfo, SteerDelivery, harness_capabilities};
use rune::Tx;
use serde_json::Value;
use std::collections::HashMap;

/// `NARROW_COMPOSER_PX`: a composer narrower than this says the plain name on its chips (row 15.4).
pub const NARROW_COMPOSER_PX: f32 = 640.0;

fn text(v: &Value, k: &str) -> Option<String> {
    v.get(k).and_then(Value::as_str).map(str::to_string)
}

/// `{providerID, modelID}` with neither part empty.
pub fn model_reference(v: &Value) -> Option<ModelReference> {
    let (provider, model) = (text(v, "providerID")?, text(v, "modelID")?);
    (!provider.is_empty() && !model.is_empty()).then(|| ModelReference::new(&provider, &model))
}

/// `toAgentOptions` over the answer of `GET /api/sessions/{id}/agents` (`{agents}` or a bare list): hidden agents
/// are dropped, the list's order is kept (its first is the default).
pub fn parse_agents(body: &Value) -> Vec<AgentOption> {
    let list = body.get("agents").unwrap_or(body);
    list.as_array()
        .into_iter()
        .flatten()
        .filter(|a| a.get("hidden") != Some(&Value::Bool(true)))
        .filter_map(|a| {
            let name = text(a, "name")?;
            Some(AgentOption {
                id: name.clone(),
                name,
                description: text(a, "description").unwrap_or_default(),
                model: a.get("model").and_then(model_reference),
            })
        })
        .collect()
}

/// `toModelOptions` over the answer of `GET /api/sessions/{id}/models` (a list of providers, or `{providers}`).
pub fn parse_models(body: &Value) -> Vec<ModelOption> {
    let list = body.get("providers").unwrap_or(body);
    let mut out = Vec::new();
    for provider in list.as_array().into_iter().flatten() {
        let (Some(pid), Some(pname)) = (text(provider, "id"), text(provider, "name")) else { continue };
        for m in provider.get("models").and_then(Value::as_array).into_iter().flatten() {
            let (Some(id), Some(name)) = (text(m, "id"), text(m, "name")) else { continue };
            let mut option = ModelOption::new(&pid, &pname, &id, &name);
            option.variants =
                m.get("variants").and_then(Value::as_array).into_iter().flatten().filter_map(|v| v.as_str().map(str::to_string)).collect();
            option.price_tier = text(m, "priceTier");
            option.capability = text(m, "capability");
            option.context_window = m.get("contextWindow").and_then(Value::as_u64);
            out.push(option);
        }
    }
    out
}

/// `GET /api/harnesses`: each harness's type and what it can do.
pub fn parse_harnesses(body: &Value) -> Vec<HarnessInfo> {
    body.as_array()
        .into_iter()
        .flatten()
        .filter_map(|h| {
            let caps = h.get("capabilities").unwrap_or(&Value::Null);
            let flag = |k: &str| caps.get(k).and_then(Value::as_bool) == Some(true);
            Some(HarnessInfo {
                kind: text(h, "type")?,
                supports_steering: flag("supportsSteering"),
                steers_by_default: flag("steersByDefault"),
                steer_delivery: match caps.get("steerDelivery").and_then(Value::as_str) {
                    Some("next-step") => Some(SteerDelivery::NextStep),
                    Some("immediate") => Some(SteerDelivery::Immediate),
                    _ => None,
                },
                supports_shell_commands: flag("supportsShellCommands"),
                supports_side_conversations: flag("supportsSideConversations"),
                supports_image_attachments: flag("supportsImageAttachments"),
            })
        })
        .collect()
}

/// The lists one session's pickers draw.
#[derive(Debug, Default)]
pub struct SessionCatalog {
    pub agents: Vec<AgentOption>,
    pub models: Vec<ModelOption>,
    pub agents_loaded: bool,
    pub models_loaded: bool,
    /// A model or agent picked earlier that the session no longer offers went back to Default (row 3.19): once.
    offered_checked: bool,
}

impl SessionCatalog {
    /// Lists already loaded (the invented sessions').
    pub fn loaded(agents: Vec<AgentOption>, models: Vec<ModelOption>) -> SessionCatalog {
        SessionCatalog { agents, models, agents_loaded: true, models_loaded: true, offered_checked: true }
    }
}

/// Everything the pickers keep, on `State`.
#[derive(Debug, Default)]
pub struct Pickers {
    catalogs: HashMap<String, SessionCatalog>,
    pub harnesses: Vec<HarnessInfo>,
    pub prefs: Prefs,
    /// Usage limits: the providers' and each harness's (`fleet::usage`).
    pub usage: super::usage::Usage,
    efforts: HashMap<String, EffortMemory>,
    /// The lists the invented sessions show, so scenes and tests need no server.
    mock: SessionCatalog,
    empty: SessionCatalog,
    /// The composer frame's measured width, for the narrow rule.
    pub composer_w: f32,
}

impl Pickers {
    /// `mock` shows invented lists (no `--server`).
    pub fn new(mock: bool) -> Pickers {
        let mut p = Pickers::default();
        if mock {
            p.mock = crate::mock::catalog();
            p.prefs.fetched = true;
        }
        p
    }

    /// A scene: the invented lists with `n` more models, to see a menu that has to scroll.
    pub fn add_mock_models(&mut self, n: usize) {
        for i in 1..=n {
            let provider = if i % 2 == 0 { ("orchard", "Orchard") } else { ("meadow", "Meadow") };
            self.mock.models.push(ModelOption::new(provider.0, provider.1, &format!("m{i}"), &format!("Model {i}")));
        }
    }

    /// A scene: `n` more models of one provider (`(id, name)`).
    pub fn add_mock_provider_models(&mut self, provider: (&str, &str), n: usize) {
        for i in 1..=n {
            self.mock.models.push(ModelOption::new(provider.0, provider.1, &format!("{}-{i}", provider.0), &format!("{} {i}", provider.1)));
        }
    }

    pub fn catalog_of(&self, session: &str) -> Option<&SessionCatalog> {
        self.catalogs.get(session)
    }
}

/// What a chip shows: its text and, when the narrow rule shortened it, the whole label for the tooltip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chip {
    pub label: String,
    pub help: Option<String>,
}

/// What the effort toggle draws.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffortView {
    pub level: String,
    pub variants: Vec<String>,
    pub note: Option<String>,
    pub model_name: String,
}

impl EffortView {
    /// The tooltip (row 4.3).
    pub fn help(&self) -> String {
        if self.level.is_empty() {
            "Default: the harness's own level, nothing sent".to_string()
        } else if self.model_name.is_empty() {
            effort_label(&self.level)
        } else {
            format!("{}, remembered for {}", effort_label(&self.level), self.model_name)
        }
    }
}

/// What `Catalog`'s once-only fallback needs: the lists as the choice logic's own shape.
fn catalog_of(agents: &[AgentOption], models: &[ModelOption]) -> Catalog {
    let mut providers: Vec<CatalogProvider> = Vec::new();
    for m in models {
        match providers.iter_mut().find(|p| p.id == m.provider_id) {
            Some(p) => p.models.push((m.id.clone(), m.name.clone())),
            None => providers.push(CatalogProvider {
                id: m.provider_id.clone(),
                name: m.provider.clone(),
                models: vec![(m.id.clone(), m.name.clone())],
            }),
        }
    }
    Catalog {
        agents: agents.iter().map(|a| CatalogAgent { name: a.id.clone(), model: a.model.clone(), hidden: false }).collect(),
        providers,
        ..Catalog::default()
    }
}

impl State {
    /// The open session's lists: the invented ones with no server, the loaded ones (empty until they arrive) with one.
    pub fn lists(&self) -> &SessionCatalog {
        if self.link.is_none() {
            return &self.pickers.mock;
        }
        self.draft_owner().and_then(|id| self.pickers.catalogs.get(&id)).unwrap_or(&self.pickers.empty)
    }

    /// The agent, model and effort picked for the open session's next prompt (every one "" is Default).
    pub fn choice(&self) -> Choice {
        self.draft_owner().map(|id| self.drafts.choice(&id)).unwrap_or_default()
    }

    /// What the open session's harness can do (row 12.2); nothing until the harness list has arrived.
    pub fn caps(&self) -> Caps {
        harness_capabilities(self.session().row.harness_type.as_deref(), &self.pickers.harnesses)
    }

    /// Whether the composer is narrower than 640 px (row 15.4); unmeasured counts as roomy.
    pub fn composer_narrow(&self) -> bool {
        self.pickers.composer_w > 0.0 && self.pickers.composer_w < NARROW_COMPOSER_PX
    }

    /// The model that answered last (`lastAssistantModelId`): from the open conversation, else the list's row.
    fn last_model_id(&self) -> Option<String> {
        let from_stream = self.stream.as_ref().filter(|_| self.stream_of == self.open_id()).and_then(|st| {
            st.messages
                .iter()
                .rev()
                .find(|m| m.role == crate::view::conversation::Role::Assistant && m.model_id.as_deref().is_some_and(|id| !id.is_empty()))
                .and_then(|m| m.model_id.clone())
        });
        from_stream.or_else(|| self.session().row.last_model_id.clone())
    }

    /// What "Default" means for the open session (row 3.18).
    pub fn default_labels(&self) -> DefaultLabels {
        let (lists, row) = (self.lists(), &self.session().row);
        let choice = self.choice();
        let last = self.last_model_id();
        describe_session_defaults(&SessionDefaultsInput {
            session_agent: row.selected_agent.as_deref(),
            session_model: row.selected_model.as_ref(),
            draft_agent: &choice.agent,
            default_agent: lists.agents.first().map_or("", |a| a.id.as_str()),
            agents: &lists.agents,
            models: &lists.models,
            last_model_id: last.as_deref(),
        })
    }

    fn chip(&self, label: String) -> Chip {
        let plain = if self.composer_narrow() { plain_default_label(&label) } else { label.clone() };
        let help = (plain != label).then_some(label);
        Chip { label: plain, help }
    }

    /// The agent chip (rows 3.1, 3.2): the pick's own name (its id when the harness no longer lists it), else the
    /// Default label.
    pub fn agent_chip(&self) -> Chip {
        let pick = self.choice().agent;
        let label = if pick.is_empty() { self.default_labels().agent_label } else { pick };
        self.chip(label)
    }

    /// The model chip: the picked model's name, its id from the key when the list lacks it, else the Default label.
    pub fn model_chip(&self) -> Chip {
        let key = self.choice().model;
        let label = if key.is_empty() {
            self.default_labels().model_label
        } else {
            self.lists()
                .models
                .iter()
                .find(|m| m.selection_key == key)
                .map(|m| m.name.clone())
                .or_else(|| model_from_key(&key).map(|m| m.model_id))
                .unwrap_or(key)
        };
        self.chip(label)
    }

    /// The model the next prompt runs on (`effortModelKey`): the picked one, else the session's own, its agent's, the
    /// one that answered last, else the list's first.
    fn effective_model_key(&self) -> String {
        let (lists, row, choice) = (self.lists(), &self.session().row, self.choice());
        if !choice.model.is_empty() {
            return choice.model;
        }
        let agent =
            [choice.agent.as_str(), row.selected_agent.as_deref().unwrap_or(""), lists.agents.first().map_or("", |a| a.id.as_str())]
                .into_iter()
                .find(|a| !a.is_empty())
                .unwrap_or("");
        let agent_model = lists.agents.iter().find(|a| a.id == agent).and_then(|a| a.model.as_ref());
        session_default_model_key(row.selected_model.as_ref(), agent_model, self.last_model_id().as_deref(), &lists.models)
            .or_else(|| lists.models.first().map(|m| m.selection_key.clone()))
            .unwrap_or_default()
    }

    /// The effective model's name and effort levels.
    fn effective_model(&self) -> (String, String, Vec<String>) {
        let key = self.effective_model_key();
        let found = self.lists().models.iter().find(|m| m.selection_key == key);
        (key.clone(), found.map(|m| m.name.clone()).unwrap_or_default(), found.map(|m| m.variants.clone()).unwrap_or_default())
    }

    /// The toggle (rows 4.1, 4.2): only for a model with levels.
    pub fn effort_view(&self) -> Option<EffortView> {
        let (_, model_name, variants) = self.effective_model();
        if variants.is_empty() {
            return None;
        }
        let owner = self.draft_owner()?;
        let note = self.pickers.efforts.get(&owner).and_then(|m| m.note(self.clock)).map(str::to_string);
        Some(EffortView { level: self.choice().effort, variants, note, model_name })
    }

    /// Lets the effort memory look at the pick again (rows 4.4, 4.5): after the lists, the preferences or a pick changed.
    pub fn settle_pickers(&mut self, tx: &mut Tx) {
        let Some(owner) = self.draft_owner() else { return };
        let (key, name, variants) = self.effective_model();
        let harness = self.session().row.harness_type.clone();
        let mut choice = self.drafts.choice(&owner);
        let picked = choice.model.clone();
        let input = effort::Input {
            fetched: self.pickers.prefs.fetched,
            harness_type: harness.as_deref(),
            model_key: &key,
            model_name: &name,
            picked: &picked,
            variants: &variants,
        };
        let memory = self.pickers.efforts.entry(owner.clone()).or_insert_with(|| EffortMemory::new(&picked));
        if memory.update(&input, &mut choice.effort, &self.pickers.prefs.values, self.clock) {
            self.drafts.set_choice(&owner, choice);
        }
        if memory.note(self.clock).is_some() {
            tx.wake_after(std::time::Duration::from_secs_f64(effort::NOTE_FOR + 0.1));
        }
    }

    pub fn pick_agent(&mut self, id: &str, tx: &mut Tx) {
        let Some(owner) = self.draft_owner() else { return };
        let mut choice = self.drafts.choice(&owner);
        choice.agent = id.to_string();
        self.drafts.set_choice(&owner, choice);
        self.settle_pickers(tx);
    }

    pub fn pick_model(&mut self, key: &str, tx: &mut Tx) {
        let Some(owner) = self.draft_owner() else { return };
        let mut choice = self.drafts.choice(&owner);
        choice.model = key.to_string();
        self.drafts.set_choice(&owner, choice);
        self.settle_pickers(tx);
    }

    /// The toggle's click (rows 4.3, 4.4): the next level, remembered for this model on the server.
    pub fn cycle_effort(&mut self) {
        let (Some(owner), Some(view)) = (self.draft_owner(), self.effort_view()) else { return };
        let (key, name, variants) = self.effective_model();
        let harness = self.session().row.harness_type.clone();
        let mut choice = self.drafts.choice(&owner);
        let picked = choice.model.clone();
        let input = effort::Input {
            fetched: self.pickers.prefs.fetched,
            harness_type: harness.as_deref(),
            model_key: &key,
            model_name: &name,
            picked: &picked,
            variants: &variants,
        };
        let pref_key = effort_preference_key(harness.as_deref(), &key);
        let before = self.pickers.prefs.get(&pref_key).map(str::to_string);
        let level = next_effort(&view.level, &view.variants);
        let memory = self.pickers.efforts.entry(owner.clone()).or_insert_with(|| EffortMemory::new(&picked));
        let (key, value) = memory.pick(&input, &level, &mut choice.effort, &mut self.pickers.prefs.values);
        self.drafts.set_choice(&owner, choice);
        self.put_preference(&key, &value, before);
    }

    /// `usePreferencesStore().set`: the value shows now; a refusal puts the old one back and asks the server again.
    pub fn put_preference(&mut self, key: &str, value: &str, before: Option<String>) {
        self.pickers.prefs.values.insert(key.to_string(), value.to_string());
        let Some(link) = &self.link else { return };
        let (k, v) = (key.to_string(), value.to_string());
        let (k2, v2) = (k.clone(), v.clone());
        link.off_thread(
            move |base| http::put_preference(base, &k, &v),
            move |s, tx, result| {
                if result.is_ok() {
                    return;
                }
                s.pickers.prefs.roll_back(&k2, &v2, before);
                s.load_preferences();
                s.settle_pickers(tx);
            },
        );
    }

    /// What a prompt sent now carries (rows 3.20, 4.6): the agent and model picked, the effort, and the id the message gets.
    pub fn prompt_pick(&self, user_message_id: Option<String>) -> Pick {
        let choice = self.choice();
        let (_, _, variants) = self.effective_model();
        Pick {
            agent: Some(choice.agent).filter(|a| !a.is_empty()),
            model: model_from_key(&choice.model).map(|m| (m.provider_id, m.model_id)),
            effort: effort_to_send(&choice.effort, Some(&variants)),
            user_message_id,
        }
    }

    /// After a send that named an agent or model, the session's row says so (row 14.9).
    pub fn note_pick_on_row(&mut self, pick: &Pick) {
        let Some(session) = self.session_mut() else { return };
        if let Some(agent) = &pick.agent {
            session.row.selected_agent = Some(agent.clone());
        }
        if let Some((provider, model)) = &pick.model {
            session.row.selected_model = Some(ModelReference::new(provider, model));
        }
    }

    // ─── loading ────────────────────────────────────────────────────────────────

    /// The harness list and the preferences, once the server is there (and again after a reconnect).
    pub fn load_pickers(&self) {
        let Some(link) = &self.link else { return };
        link.off_thread(http::harnesses, |s, tx, result| {
            if let Ok(body) = result {
                s.pickers.harnesses = parse_harnesses(&body);
                s.settle_pickers(tx);
            }
        });
        self.load_preferences();
        self.load_usage();
        if let Some(id) = self.open_id() {
            self.load_catalog(&id);
        }
    }

    pub fn load_preferences(&self) {
        let Some(link) = &self.link else { return };
        link.off_thread(http::preferences, |s, tx, result| {
            if let Ok(body) = result {
                s.pickers.prefs.adopt(&body);
                s.settle_pickers(tx);
            }
        });
    }

    /// Asks for a session's agents and models (rows 14.1, 14.2). The old lists stay until the new ones arrive, and a
    /// failed request keeps them: the pickers have no loading or error state (row 3.21).
    pub fn load_catalog(&self, session: &str) {
        let Some(link) = &self.link else { return };
        let id = session.to_string();
        let asked = id.clone();
        link.off_thread(
            move |base| http::agents(base, &asked),
            move |s, tx, result| {
                if let Ok(body) = result {
                    s.on_agents(&id, parse_agents(&body), tx);
                }
            },
        );
        let id = session.to_string();
        let asked = id.clone();
        link.off_thread(
            move |base| http::models(base, &asked),
            move |s, tx, result| {
                if let Ok(body) = result {
                    s.on_models(&id, parse_models(&body), tx);
                }
            },
        );
    }

    pub fn on_agents(&mut self, session: &str, agents: Vec<AgentOption>, tx: &mut Tx) {
        let c = self.pickers.catalogs.entry(session.to_string()).or_default();
        c.agents = agents;
        c.agents_loaded = true;
        self.lists_changed(session, tx);
    }

    pub fn on_models(&mut self, session: &str, models: Vec<ModelOption>, tx: &mut Tx) {
        let c = self.pickers.catalogs.entry(session.to_string()).or_default();
        c.models = models;
        c.models_loaded = true;
        self.lists_changed(session, tx);
    }

    /// A pick the first lists don't offer goes back to Default, once (row 3.19); a list that changes later doesn't.
    fn lists_changed(&mut self, session: &str, tx: &mut Tx) {
        let Some(c) = self.pickers.catalogs.get_mut(session) else { return };
        if c.agents_loaded && c.models_loaded && !c.offered_checked {
            c.offered_checked = true;
            let catalog = catalog_of(&c.agents, &c.models);
            let choice = self.drafts.choice(session);
            let kept = keep_offered(&AgentModelChoice { agent: choice.agent.clone(), model: choice.model.clone() }, &catalog);
            if kept.agent != choice.agent || kept.model != choice.model {
                self.drafts.set_choice(session, Choice { agent: kept.agent, model: kept.model, ..choice });
            }
        }
        if self.draft_owner().as_deref() == Some(session) {
            self.settle_pickers(tx);
        }
    }

    /// `harness.catalog_changed` on the `sessions` topic: the open session, if it is one of those named, asks again.
    pub fn on_catalog_event(&self, event: &Event) {
        let Event::Other(Kind::HarnessCatalogChanged, props) = event else { return };
        let Some(open) = self.open_id() else { return };
        let named =
            props.get("sessionIds").and_then(Value::as_array).is_some_and(|ids| ids.iter().any(|i| i.as_str() == Some(open.as_str())));
        if named {
            self.load_catalog(&open);
        }
    }
}
