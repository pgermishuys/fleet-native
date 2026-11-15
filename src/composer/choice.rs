//! What a session's agent and model pickers hold, and what "Default" means for them (`lib/agent-model-choice.ts`).
//! A model is picked by its selection key, the JSON pair `["provider","model"]`; "" is Default. The lists these
//! work on come from the session's catalog (piece D fetches them); here they are plain values.
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelReference {
    pub provider_id: String,
    pub model_id: String,
}

impl ModelReference {
    pub fn new(provider: &str, model: &str) -> Self {
        Self { provider_id: provider.into(), model_id: model.into() }
    }
}

/// An agent in the picker (`AgentOption`): its id is its name.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AgentOption {
    pub id: String,
    pub name: String,
    pub description: String,
    /// The agent's own model, when it has one.
    pub model: Option<ModelReference>,
}

/// A model in the picker (`ModelOption`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ModelOption {
    pub id: String,
    pub name: String,
    pub provider_id: String,
    pub selection_key: String,
    /// The provider's name, which the headings show.
    pub provider: String,
    pub description: String,
    pub variants: Vec<String>,
    /// low, medium, high or very_high, when the harness says what the model costs.
    pub price_tier: Option<String>,
    pub capability: Option<String>,
    /// The context window in tokens, when the catalog says.
    pub context_window: Option<u64>,
}

impl ModelOption {
    /// A model with the fields every list needs; the rest are set on the value.
    pub fn new(provider_id: &str, provider: &str, id: &str, name: &str) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            provider_id: provider_id.into(),
            selection_key: create_model_selection_key(provider_id, id),
            provider: provider.into(),
            ..Self::default()
        }
    }
}

/// The agent and model a session starts with, as the pickers hold them, each empty for "Default".
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AgentModelChoice {
    pub agent: String,
    pub model: String,
}

/// What the harness offers a folder (`HarnessCatalog`), for the parts the choice logic reads.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Catalog {
    pub agents: Vec<CatalogAgent>,
    pub providers: Vec<CatalogProvider>,
    pub default_agent: Option<String>,
    pub default_model: Option<ModelReference>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CatalogAgent {
    pub name: String,
    pub model: Option<ModelReference>,
    pub hidden: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CatalogProvider {
    pub id: String,
    pub name: String,
    /// Model id and name.
    pub models: Vec<(String, String)>,
}

/// `createModelSelectionKey`.
pub fn create_model_selection_key(provider_id: &str, model_id: &str) -> String {
    serde_json::to_string(&[provider_id, model_id]).unwrap_or_default()
}

pub fn key_for_model(model: &ModelReference) -> String {
    create_model_selection_key(&model.provider_id, &model.model_id)
}

/// A model selection key back to the model it names; `None` for "Default" or a key that isn't one.
pub fn model_from_key(key: &str) -> Option<ModelReference> {
    if key.is_empty() {
        return None;
    }
    let Value::Array(parts) = serde_json::from_str::<Value>(key).ok()? else { return None };
    match parts.as_slice() {
        [Value::String(p), Value::String(m)] if !p.is_empty() && !m.is_empty() => Some(ModelReference::new(p, m)),
        _ => None,
    }
}

/// `provider/model`, the way automations store a model.
pub fn model_to_path(model: Option<&ModelReference>) -> Option<String> {
    model.map(|m| format!("{}/{}", m.provider_id, m.model_id))
}

/// A path back to a model, split at the first slash: a model id may hold slashes of its own.
pub fn model_from_path(path: Option<&str>) -> Option<ModelReference> {
    let path = path?;
    let slash = path.find('/')?;
    if slash == 0 || slash == path.len() - 1 {
        return None;
    }
    Some(ModelReference::new(&path[..slash], &path[slash + 1..]))
}

/// Keeps only what the catalog still offers: an agent or model that's gone goes back to "Default".
pub fn keep_offered(choice: &AgentModelChoice, catalog: &Catalog) -> AgentModelChoice {
    let agent_offered = choice.agent.is_empty() || catalog.agents.iter().any(|a| a.name == choice.agent && !a.hidden);
    let model_offered = match model_from_key(&choice.model) {
        None => true,
        Some(m) => catalog.providers.iter().any(|p| p.id == m.provider_id && p.models.iter().any(|(id, _)| *id == m.model_id)),
    };
    AgentModelChoice {
        agent: if agent_offered { choice.agent.clone() } else { String::new() },
        model: if model_offered { choice.model.clone() } else { String::new() },
    }
}

/// The model a prompt gets from `agent` (or the default agent) when it names none; `None` when only the harness knows.
pub fn model_for(catalog: &Catalog, agent: &str) -> Option<ModelReference> {
    let name = if agent.is_empty() { catalog.default_agent.as_deref().unwrap_or("") } else { agent };
    let own = catalog.agents.iter().find(|a| a.name == name).and_then(|a| a.model.clone());
    match own {
        Some(m) if !m.provider_id.is_empty() && !m.model_id.is_empty() => Some(m),
        _ => catalog.default_model.clone(),
    }
}

/// A model's name from the catalog, or its id when the catalog doesn't list it (a provider with no key).
pub fn model_name(catalog: &Catalog, model: &ModelReference) -> String {
    catalog
        .providers
        .iter()
        .find(|p| p.id == model.provider_id)
        .and_then(|p| p.models.iter().find(|(id, _)| *id == model.model_id))
        .map_or_else(|| model.model_id.clone(), |(_, name)| name.clone())
}

/// A model id on its own, as a message carries it, as a name you can read; the id again when the list lacks it.
pub fn model_display_name(model_id: Option<&str>, models: &[ModelOption]) -> String {
    match model_id {
        None | Some("") => String::new(),
        Some(id) => models.iter().find(|m| m.id == id).map_or_else(|| id.to_string(), |m| m.name.clone()),
    }
}

/// What "Default" means on each picker, e.g. "Default (loom)" and "Default (Claude Opus 4.7)".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefaultLabels {
    pub agent_label: String,
    pub agent_description: String,
    pub model_label: String,
    pub model_description: String,
}

/// What "Default" means for a folder's choice (component 7 draws it).
pub fn describe_defaults(catalog: Option<&Catalog>, choice: &AgentModelChoice) -> DefaultLabels {
    let default_agent = catalog.and_then(|c| c.default_agent.as_deref()).filter(|a| !a.is_empty());
    let agent_label = default_agent.map_or("Default".to_string(), |a| format!("Default ({a})"));
    let agent_description = default_agent.map_or("The agent this folder uses when you don't pick one".to_string(), |a| {
        format!("The agent this folder uses when you don't pick one: {a}")
    });
    let model = catalog.and_then(|c| model_for(c, &choice.agent));
    let (Some(catalog), Some(model)) = (catalog, model) else {
        return DefaultLabels {
            agent_label,
            agent_description,
            model_label: "Default".into(),
            model_description: "Whatever the harness picks".into(),
        };
    };
    let agent = if choice.agent.is_empty() { default_agent.unwrap_or("") } else { &choice.agent };
    let agents_own = catalog.agents.iter().any(|a| a.name == agent && a.model.as_ref().is_some_and(|m| !m.model_id.is_empty()));
    let name = model_name(catalog, &model);
    DefaultLabels {
        agent_label,
        agent_description,
        model_label: format!("Default ({name})"),
        model_description: if agents_own { format!("{agent}'s own model: {name}") } else { format!("This folder's model: {name}") },
    }
}

/// What `describe_session_defaults` reads.
#[derive(Debug, Clone, Default)]
pub struct SessionDefaultsInput<'a> {
    pub session_agent: Option<&'a str>,
    pub session_model: Option<&'a ModelReference>,
    /// An agent picked for the next prompt, whose own model applies when the session has none.
    pub draft_agent: &'a str,
    pub default_agent: &'a str,
    pub agents: &'a [AgentOption],
    pub models: &'a [ModelOption],
    /// The model that answered last, for a session left to the harness's own model.
    pub last_model_id: Option<&'a str>,
}

/// What "Default" means inside a session: the agent and model it started with or was last given (prompts that name
/// none get them), else the harness's own.
pub fn describe_session_defaults(input: &SessionDefaultsInput) -> DefaultLabels {
    let session_agent = input.session_agent.filter(|a| !a.is_empty());
    let agent = session_agent.unwrap_or(input.default_agent);
    let agent_label = if agent.is_empty() { "Default".to_string() } else { format!("Default ({agent})") };
    let agent_description = session_agent.map_or("Use the session default agent".to_string(), |a| format!("This session's agent: {a}"));

    let for_agent = if input.draft_agent.is_empty() { agent } else { input.draft_agent };
    let agents_own = input.agents.iter().find(|a| a.id == for_agent).and_then(|a| a.model.as_ref());
    let Some(model) = input.session_model.or(agents_own) else {
        let last = model_display_name(input.last_model_id, input.models);
        return if last.is_empty() {
            DefaultLabels {
                agent_label,
                agent_description,
                model_label: "Default".into(),
                model_description: "Use the session default model".into(),
            }
        } else {
            DefaultLabels {
                agent_label,
                agent_description,
                model_label: format!("Default ({last})"),
                model_description: format!("The model that answered last: {last}"),
            }
        };
    };
    let name = input
        .models
        .iter()
        .find(|m| m.provider_id == model.provider_id && m.id == model.model_id)
        .map_or_else(|| model.model_id.clone(), |m| m.name.clone());
    DefaultLabels {
        agent_label,
        agent_description,
        model_label: format!("Default ({name})"),
        model_description: if input.session_model.is_some() {
            format!("This session's model: {name}")
        } else {
            format!("{for_agent}'s own model: {name}")
        },
    }
}

/// The selection key of the model a session's next prompt runs on when no model is picked for it: the one it was
/// given, its agent's own, else the one that answered last. `None` when only the harness knows, so a caller must
/// not guess with the catalog's first entry.
pub fn session_default_model_key(
    session_model: Option<&ModelReference>,
    agent_model: Option<&ModelReference>,
    last_model_id: Option<&str>,
    models: &[ModelOption],
) -> Option<String> {
    if let Some(named) = session_model.or(agent_model) {
        return Some(key_for_model(named));
    }
    let last = last_model_id.filter(|id| !id.is_empty())?;
    models.iter().find(|m| m.id == last).map(|m| m.selection_key.clone())
}

/// "Default (build)" as just "build", for a chip with no room for the wrapper; any other label stays as it is.
pub fn plain_default_label(label: &str) -> String {
    label
        .strip_prefix("Default (")
        .and_then(|rest| rest.strip_suffix(')'))
        .filter(|inner| !inner.is_empty())
        .map_or_else(|| label.to_string(), str::to_string)
}

/// What picking HydraFusion means for the bill; Fleet's Settings switch says the same.
pub const HYDRAFUSION_NOTE: &str = "HydraFusion runs several models per prompt; cost varies per prompt.";

/// `isHydraFusion`: the id (or the Default row's label) names HydraFusion, in any case.
pub fn is_hydrafusion(text: &str) -> bool {
    text.to_lowercase().contains("hydrafusion")
}

/// A model row's description (`ModelSelector.vue`): HydraFusion's note replaces it, and replaces the Default row's
/// too while Default resolves to HydraFusion (`default_label`).
pub fn model_description<'a>(model_id: &str, description: &'a str) -> &'a str {
    if is_hydrafusion(model_id) { HYDRAFUSION_NOTE } else { description }
}

pub fn default_model_description<'a>(default_label: &str, default_description: &'a str) -> &'a str {
    if is_hydrafusion(default_label) { HYDRAFUSION_NOTE } else { default_description }
}
