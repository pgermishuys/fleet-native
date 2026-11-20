//! Model roles (component 18, piece E; `WorkflowModelRoles.vue`, `use-model-roles.ts`, and the Models popover of
//! `WorkflowRunBox.vue`): which of the user's models does each kind of work, per harness, kept in the `WorkflowModelRoles`
//! preference; the dialog that sets them; and what a run changes for itself from its Run box. Plain state changes: nothing here
//! draws. The model and effort menus are the composer's selector menu (`fleet::selector`) pointed at a role.
use super::run_box::workflow_harnesses;
use super::{MODEL_ROLES_PREFERENCE_KEY, ModelChoice, ModelRoles, Role, StepKind, key_from_path, parse_model_roles, path_from_key};
use crate::composer::choice::{AgentModelChoice, ModelOption, describe_defaults};
use crate::composer::selector::{Item, default_item};
use crate::fleet::new_session::feed_state::CatalogKey;
use crate::fleet::new_session::feeds::Catalog;
use crate::{State, Tx};
use serde_json::{Map, Value};

pub const MODELS_CHIP: &str = "workflow-models";
pub const MODELS_POPOVER: &str = "workflow-models";
/// The selector menu's names for a role's model and its effort (`Open::Menu`).
pub const MODEL_MENU: &str = "wf-model";
pub const EFFORT_MENU: &str = "wf-effort";

/// Where a role's menu opened: the dialog, or the Run box's popover (which draws again when the menu closes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    Dialog,
    RunBox,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RoleMenu {
    pub place: Place,
    pub role: Role,
    pub effort: bool,
}

/// What the dialog holds besides the preference: the harness picked, the role being saved and the menu open.
#[derive(Debug, Default)]
pub struct Roles {
    pub harness: Option<String>,
    pub saving: Option<Role>,
    pub menu: Option<RoleMenu>,
    /// Where the open menu's trigger starts, measured: the menu slides left to stay inside the dialog.
    pub trigger_x: f32,
}

/// The preference as Fleet writes it: `{harness: {role: {model, effort}}}`, nulls kept.
pub fn roles_json(roles: &ModelRoles) -> String {
    let mut all = Map::new();
    for (harness, by_role) in roles {
        let mut one = Map::new();
        for (role, choice) in by_role {
            let mut c = Map::new();
            c.insert("model".into(), choice.model.clone().map_or(Value::Null, Value::String));
            c.insert("effort".into(), choice.effort.clone().map_or(Value::Null, Value::String));
            one.insert(role.key().into(), Value::Object(c));
        }
        all.insert(harness.clone(), Value::Object(one));
    }
    Value::Object(all).to_string()
}

/// `setOverride`'s rule: picking what the role already maps to is no override.
pub fn with_override(
    overrides: &[(Role, ModelChoice)],
    role: Role,
    model: Option<String>,
    mapped: Option<&str>,
) -> Vec<(Role, ModelChoice)> {
    let mut next: Vec<(Role, ModelChoice)> = overrides.iter().filter(|(r, _)| *r != role).cloned().collect();
    if model.as_deref() != mapped {
        next.push((role, ModelChoice { model, effort: None }));
        next.sort_by_key(|(r, _)| *r);
    }
    next
}

impl State {
    /// `useModelRoles().roles`: what the machine the page asks keeps.
    pub fn model_roles(&self) -> ModelRoles {
        let prefs = self.prefs_for(&self.workflows_machine());
        parse_model_roles(prefs.get(MODEL_ROLES_PREFERENCE_KEY))
    }

    /// `choiceFor`.
    pub fn role_choice(&self, harness: &str, role: Role) -> ModelChoice {
        self.model_roles().get(harness).and_then(|r| r.get(&role)).cloned().unwrap_or_default()
    }

    /// The harnesses roles are kept for: enabled ones that run workflow steps, as (type, name).
    pub fn roles_harnesses(&self) -> Vec<(String, String)> {
        workflow_harnesses(&self.run_box_harnesses()).into_iter().map(|h| (h.kind.clone(), h.display_name.clone())).collect()
    }

    /// The dialog's harness: the one picked, else the default when it is one of them, else the first.
    pub fn roles_harness_type(&self) -> String {
        if let Some(picked) = &self.workflows_nav.roles.harness {
            return picked.clone();
        }
        let harnesses = self.run_box_harnesses();
        let default = harnesses.default_type();
        let runs = workflow_harnesses(&harnesses);
        if runs.iter().any(|h| h.kind == default) { default } else { runs.first().map(|h| h.kind.clone()).unwrap_or_default() }
    }

    /// `workflowHarnessesNote`: which harnesses can run workflows, for when none of them is on.
    pub fn roles_harness_note(&self) -> String {
        let harnesses = self.run_box_harnesses();
        let names: Vec<&str> = harnesses.all.iter().filter(|h| h.supports_workflow_steps).map(|h| h.display_name.as_str()).collect();
        let list = match names.as_slice() {
            [] => return "None of the harnesses here can run workflows yet.".to_string(),
            [one] => one.to_string(),
            [init @ .., last] => format!("{} and {last}", init.join(", ")),
        };
        format!("Workflows run on {list}. Turn {} on in Settings \u{2192} Harnesses.", if names.len() == 1 { "it" } else { "one of them" })
    }

    /// The steps that ask for `role` across the library, each title once.
    pub fn role_used_by(&self, role: Role) -> Vec<String> {
        let mut titles: Vec<String> = Vec::new();
        for w in self.workflows_nav.library.iter().flat_map(|l| l.workflows.iter()) {
            for step in &w.steps {
                if step.kind == StepKind::Agent && step.model.as_deref() == Some(role.key()) && !titles.contains(&step.title) {
                    titles.push(step.title.clone());
                }
            }
        }
        titles
    }

    // ─── What the menus list ───────────────────────────────────────────────────────────────────────────────

    fn role_catalog(&self) -> Option<&Catalog> {
        self.feeds.catalogs.get(&self.workflows_machine()).and_then(|f| f.shown.as_ref())
    }

    pub fn role_models(&self) -> Vec<ModelOption> {
        self.role_catalog().map(|c| c.models.clone()).unwrap_or_default()
    }

    /// `isSupported`: the harness can list its models here.
    pub fn role_models_supported(&self) -> bool {
        self.role_catalog().is_some_and(|c| c.supported)
    }

    /// The harness the open menu is about.
    pub fn role_menu_harness(&self) -> Option<String> {
        match self.workflows_nav.roles.menu?.place {
            Place::Dialog => Some(self.roles_harness_type()),
            Place::RunBox => Some(self.run_box_harness_type()),
        }
    }

    /// The Default row of a model menu: the dialog says what the composer would use; the Run box doesn't.
    pub fn role_default_item(&self, place: Place) -> Item {
        match place {
            Place::RunBox => default_item("Default model", "The composer's default model", false),
            Place::Dialog => {
                let catalog = self.role_catalog().filter(|c| c.supported).map(crate::fleet::new_session::harness::page_catalog_for_choice);
                let label = describe_defaults(catalog.as_ref(), &AgentModelChoice::default()).model_label;
                let label = match label.strip_prefix("Default (").and_then(|l| l.strip_suffix(')')) {
                    Some(name) => format!("Default model \u{b7} {name}"),
                    None => "Default model".to_string(),
                };
                default_item(&label, "The model the session composer uses when you don't pick one", false)
            }
        }
    }

    /// The model's selection key now: this run's override in the Run box, else what the role maps to.
    pub fn role_model_key(&self, place: Place, role: Role) -> String {
        if place == Place::RunBox
            && let Some((_, over)) = self.workflows_nav.run_box.overrides.iter().find(|(r, _)| *r == role)
        {
            return key_from_path(over.model.as_deref());
        }
        let harness = if place == Place::Dialog { self.roles_harness_type() } else { self.run_box_harness_type() };
        key_from_path(self.role_choice(&harness, role).model.as_deref())
    }

    /// What the trigger says: the model's name, or what Default is.
    pub fn role_model_label(&self, place: Place, role: Role) -> String {
        let key = self.role_model_key(place, role);
        if key.is_empty() {
            return self.role_default_item(place).label;
        }
        let models = self.role_models();
        models.iter().find(|m| m.selection_key == key).map_or_else(|| self.role_default_item(place).label, |m| m.name.clone())
    }

    /// `modelLabel` for a harness with no catalog: the path's model, or Default.
    pub fn role_fixed_label(&self, role: Role) -> String {
        let harness = self.run_box_harness_type();
        let path = match self.workflows_nav.run_box.overrides.iter().find(|(r, _)| *r == role) {
            Some((_, over)) => over.model.clone(),
            None => self.role_choice(&harness, role).model,
        };
        let Some(path) = path else { return "Default model".into() };
        let key = key_from_path(Some(&path));
        self.role_models()
            .iter()
            .find(|m| m.selection_key == key)
            .map_or_else(|| path.split_once('/').map_or(path.clone(), |(_, m)| m.to_string()), |m| m.name.clone())
    }

    /// The efforts the role's model offers: "Effort: default", then each variant. One item when the model has none.
    pub fn role_effort_items(&self, role: Role) -> Vec<Item> {
        let key = self.role_model_key(Place::Dialog, role);
        let variants = self.role_models().into_iter().find(|m| m.selection_key == key).map(|m| m.variants).unwrap_or_default();
        std::iter::once(Item { label: "Effort: default".into(), description: "The model's own effort".into(), ..Item::default() })
            .chain(variants.into_iter().map(|v| Item { id: v.clone(), label: format!("Effort: {v}"), ..Item::default() }))
            .collect()
    }

    pub fn role_effort_label(&self, role: Role) -> String {
        let effort = self.role_choice(&self.roles_harness_type(), role).effort.unwrap_or_default();
        self.role_effort_items(role).into_iter().find(|i| i.id == effort).map_or_else(|| "Effort: default".into(), |i| i.label)
    }

    /// The model menu's Default row and models, or the effort rows, for the open role menu (`selector_data`).
    pub(crate) fn role_menu_data(&self, effort: bool) -> Option<(Vec<Item>, Option<Vec<crate::composer::selector::Section>>)> {
        let menu = self.workflows_nav.roles.menu?;
        if effort {
            return Some((self.role_effort_items(menu.role), None));
        }
        let harness = self.role_menu_harness();
        Some(crate::composer::selector::model_items(
            self.role_default_item(menu.place),
            &self.role_models(),
            harness.as_deref(),
            &self.model_picker(),
            false,
        ))
    }

    /// What is picked in the open role menu (`v-model`).
    pub(crate) fn role_menu_selected(&self, effort: bool) -> String {
        let Some(menu) = self.workflows_nav.roles.menu else { return String::new() };
        if effort {
            return self.role_choice(&self.roles_harness_type(), menu.role).effort.unwrap_or_default();
        }
        self.role_model_key(menu.place, menu.role)
    }

    // ─── Opening, picking, closing ─────────────────────────────────────────────────────────────────────────

    pub fn open_role_menu(&mut self, place: Place, role: Role, effort: bool) {
        self.workflows_nav.roles.menu = Some(RoleMenu { place, role, effort });
        self.open_selector(if effort { EFFORT_MENU } else { MODEL_MENU });
    }

    /// A role menu closed (by a pick, Escape or a press outside): from the Run box the popover shows again.
    pub(crate) fn role_menu_closed(&mut self) {
        if let Some(menu) = self.workflows_nav.roles.menu.take()
            && menu.place == Place::RunBox
        {
            self.ui.open = Some(crate::ui::Open::Popover(MODELS_POPOVER));
        }
    }

    /// A row of the open role menu was picked: the menu is already closed.
    pub(crate) fn pick_role(&mut self, menu: RoleMenu, id: &str) {
        match (menu.place, menu.effort) {
            (Place::Dialog, false) => {
                // A new model starts on its own default effort.
                self.save_role_choice(menu.role, ModelChoice { model: path_from_key(id), effort: None }, true);
            }
            (Place::Dialog, true) => {
                let model = self.role_choice(&self.roles_harness_type(), menu.role).model;
                self.save_role_choice(menu.role, ModelChoice { model, effort: Some(id.to_string()).filter(|e| !e.is_empty()) }, false);
            }
            (Place::RunBox, _) => {
                let mapped = self.role_choice(&self.run_box_harness_type(), menu.role).model;
                let run_box = &mut self.workflows_nav.run_box;
                run_box.overrides = with_override(&run_box.overrides, menu.role, path_from_key(id), mapped.as_deref());
            }
        }
    }

    /// `setChoice`: the preference written with `role` changed; the value shows at once.
    fn save_role_choice(&mut self, role: Role, choice: ModelChoice, saving: bool) {
        let harness = self.roles_harness_type();
        let mut roles = self.model_roles();
        roles.entry(harness).or_default().insert(role, choice);
        let machine = self.workflows_machine();
        if saving {
            self.workflows_nav.roles.saving = Some(role);
        }
        self.save_pref_on_machine(&machine, MODEL_ROLES_PREFERENCE_KEY, &roles_json(&roles), move |s, _| {
            if saving && s.workflows_nav.roles.saving == Some(role) {
                s.workflows_nav.roles.saving = None;
            }
        });
    }

    pub fn pick_roles_harness(&mut self, kind: &str) {
        self.workflows_nav.roles.harness = Some(kind.to_string());
    }

    /// The dialog closes for good, with whatever menu it had open.
    pub fn close_roles(&mut self) {
        self.workflows_nav.roles_open = false;
        if self.workflows_nav.roles.menu.is_some_and(|m| m.place == Place::Dialog) {
            self.workflows_nav.roles.menu = None;
            self.close_selector();
        }
    }

    pub fn open_models_popover(&mut self) {
        self.ui.open = Some(crate::ui::Open::Popover(MODELS_POPOVER));
    }

    pub fn models_popover_open(&self) -> bool {
        matches!(self.ui.open, Some(crate::ui::Open::Popover(MODELS_POPOVER)))
            || self.workflows_nav.roles.menu.is_some_and(|m| m.place == Place::RunBox)
    }

    // ─── Upkeep ────────────────────────────────────────────────────────────────────────────────────────────

    /// What the page asks the harness for: the dialog's harness while it is open, else the Run box's once it has a repository.
    fn roles_catalog_wanted(&self) -> Option<CatalogKey> {
        let nav = &self.workflows_nav;
        let (harness, directory) = if nav.roles_open {
            (self.roles_harness_type(), nav.repository_path().map(str::to_string))
        } else if let Some(path) = nav.repository_path() {
            (self.run_box_harness_type(), Some(path.to_string()))
        } else {
            (String::new(), None)
        };
        (!harness.is_empty()).then_some(CatalogKey { harness, directory, profile: None })
    }

    /// Asks for the models of the harness the page shows, when that changes. True when it asked.
    pub(crate) fn keep_roles(&mut self, _tx: &mut Tx) -> bool {
        let machine = self.workflows_machine();
        if self.feed_base(&machine).is_none() {
            return false;
        }
        let wanted = self.roles_catalog_wanted();
        if self.feeds.catalogs.get(&machine).and_then(|f| f.wanted.clone()) == wanted {
            return false;
        }
        self.select_catalog(&machine, wanted);
        true
    }
}
