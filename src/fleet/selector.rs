//! What the open selector menu (agent or model) holds and does: the filter, the highlighted row, where the rows are
//! on screen, and the picks and stars it makes. Ports the script half of `SelectorDropdown.vue`, `ModelSelector.vue`
//! and `AgentSelector.vue`; the rules are in `composer/selector.rs`, the drawing in `view/selector.rs`.
use crate::composer::choice::{AgentOption, DefaultLabels, ModelOption};
use crate::composer::groups::{ModelPicker, PREFERENCE_KEY};
use crate::composer::selector::{self as rules, Item, Section, Shown};
use crate::{State, Tx};
use rune::TextEdit;

/// The menu's own state; one menu is open at a time (`Ui.open`).
#[derive(Default)]
pub struct Menu {
    pub filter: TextEdit,
    /// The row Enter takes: the arrows and the pointer move it.
    pub active: usize,
    /// The filter text `active` was last placed for: typing puts it on the first match.
    placed_for: String,
    /// Where each row is on screen, `(top, height)`, and where the list's visible part ends: for the "N more" cue.
    pub rows: Vec<(f32, f32)>,
    pub list_bottom: f32,
    /// The window, measured from the root.
    pub window: (f32, f32),
}

impl Menu {
    pub fn more_below(&self, count: usize) -> usize {
        rules::more_below(&self.rows[..count.min(self.rows.len())], self.list_bottom)
    }
}

impl State {
    /// `harnessType`: stars and sections need the session's harness. The invented sessions have none and show an
    /// OpenCode-like one so scenes and tests draw the whole menu.
    pub fn harness_key(&self) -> Option<String> {
        // On the New session page the menus are about the harness the draft would start on.
        if self.on_new_session_page() {
            return Some(self.resolved_harness()).filter(|h| !h.is_empty());
        }
        match self.session().row.harness_type.clone() {
            Some(h) => Some(h),
            None if self.is_mock() => Some("opencode".into()),
            None => None,
        }
    }

    fn model_picker(&self) -> ModelPicker {
        ModelPicker::parse(self.pickers.prefs.get(PREFERENCE_KEY).unwrap_or(""))
    }

    /// What the menus list: the open session's agents and models, or on the New session page the folder's catalog.
    fn selector_lists(&self) -> (Vec<AgentOption>, Vec<ModelOption>, DefaultLabels) {
        if self.on_new_session_page() {
            return (self.page_agents(), self.page_models(), self.page_default_labels());
        }
        let lists = self.lists();
        (lists.agents.clone(), lists.models.clone(), self.default_labels())
    }

    /// The rows the menu `name` lists, flat, and in sections when the model picker has a harness.
    pub fn selector_data(&self, name: &str) -> (Vec<Item>, Option<Vec<Section>>) {
        let (agents, models, labels) = self.selector_lists();
        if name == "agent" {
            return (rules::agent_items(rules::default_item(&labels.agent_label, &labels.agent_description, false), &agents), None);
        }
        let default = rules::default_item(&labels.model_label, &labels.model_description, true);
        rules::model_items(default, &models, self.harness_key().as_deref(), &self.model_picker(), true)
    }

    /// What is drawn now, rows numbered.
    pub fn selector_shown(&self, name: &str) -> Vec<Shown> {
        let (items, sections) = self.selector_data(name);
        let mut view = rules::shown(&items, sections.as_deref(), &self.ui.sel.filter.text);
        // Only a provider heading carries the bar, so only a menu with provider headings has any.
        for section in view.iter_mut().filter(|v| v.provider) {
            section.usage = section.rows.first().and_then(|r| self.provider_usage(&r.item.provider_id));
        }
        view
    }

    /// The id of the row picked now (`v-model`): "" is Default.
    pub fn selector_selected(&self, name: &str) -> String {
        if self.on_new_session_page() {
            let d = self.new_draft.draft();
            return d.map(|d| if name == "agent" { d.agent.clone() } else { d.model.clone() }).unwrap_or_default();
        }
        let choice = self.choice();
        if name == "agent" { choice.agent } else { choice.model }
    }

    /// Opens the menu: nothing typed, the highlight on the selected row (the filter is focused by the menu's modal).
    pub fn open_selector(&mut self, name: &'static str) {
        self.ui.sel.filter = TextEdit::new("");
        self.ui.open = Some(crate::ui::Open::Menu(name));
        let shown = self.selector_shown(name);
        self.ui.sel.active = rules::start_active(&shown, "", &self.selector_selected(name));
        self.ui.sel.placed_for = String::new();
        self.ui.sel.rows.clear();
    }

    /// Closes it and forgets what was typed.
    pub fn close_selector(&mut self) {
        self.ui.open = None;
        self.ui.sel.filter = TextEdit::new("");
    }

    /// Typing puts the highlight on the first match, not on Default, which every filter keeps. True when it moved.
    pub fn keep_selector(&mut self) -> bool {
        let Some(crate::ui::Open::Menu(name @ ("agent" | "model"))) = self.ui.open else { return false };
        if self.ui.sel.filter.text == self.ui.sel.placed_for {
            return false;
        }
        let text = self.ui.sel.filter.text.clone();
        let shown = self.selector_shown(name);
        self.ui.sel.active = rules::start_active(&shown, &text, &self.selector_selected(name));
        self.ui.sel.placed_for = text;
        true
    }

    pub fn move_selector(&mut self, name: &str, down: bool, tx: &mut Tx) {
        self.keep_selector();
        let shown = self.selector_shown(name);
        let rows = rules::rows(&shown);
        self.ui.sel.active = rules::move_active(self.ui.sel.active, down, rows.len());
        if let Some(row) = rows.get(self.ui.sel.active) {
            tx.scroll_into_view_in(format!("{name}-pop-rows"), format!("{name}-opt{}", row.index));
        }
    }

    /// Enter: picks the highlighted row.
    pub fn pick_active(&mut self, name: &'static str, tx: &mut Tx) {
        self.keep_selector();
        let shown = self.selector_shown(name);
        let id = rules::rows(&shown).get(self.ui.sel.active).map(|r| r.item.id.clone());
        if let Some(id) = id {
            self.pick_selector(name, &id, tx);
        }
    }

    /// A click on a row: sets the draft's pick, closes the menu and clears the filter.
    pub fn pick_selector(&mut self, name: &str, id: &str, tx: &mut Tx) {
        self.close_selector();
        if self.on_new_session_page() {
            if name == "agent" {
                self.pick_page_agent(id);
            } else {
                self.pick_page_model(id);
            }
            self.settle_choices();
            return;
        }
        if name == "agent" {
            self.pick_agent(id, tx);
        } else {
            self.pick_model(id, tx);
        }
    }

    /// A prompt or queued item sent with a model picked lists that model first under Recent (`recordRecent`, row 3.16).
    /// Nothing is written for Default, for a model the list doesn't have, or when it already leads the harness's list.
    pub fn record_recent_model(&mut self, pick: &super::http::Pick) {
        let Some((provider, model)) = &pick.model else { return };
        let Some(harness) = self.harness_key() else { return };
        let selection_key = crate::composer::choice::key_for_model(&crate::composer::choice::ModelReference::new(provider, model));
        let (_, models, _) = self.selector_lists();
        let Some(key) = rules::star_key(&models, &harness, &selection_key) else { return };
        let before = self.pickers.prefs.get(PREFERENCE_KEY).map(str::to_string);
        let current = self.model_picker();
        let next = current.record_recent(&key);
        if next == current {
            return;
        }
        self.put_preference(PREFERENCE_KEY, &next.to_json(), before);
    }

    /// The star: adds the model to the favourites or takes it out, and writes the `modelPicker` preference. It neither
    /// picks the row nor closes the menu.
    pub fn toggle_star(&mut self, selection_key: &str) {
        let Some(harness) = self.harness_key() else { return };
        let (_, models, _) = self.selector_lists();
        let Some(key) = rules::star_key(&models, &harness, selection_key) else { return };
        let before = self.pickers.prefs.get(PREFERENCE_KEY).map(str::to_string);
        let next = self.model_picker().toggle_favourite(&key);
        self.put_preference(PREFERENCE_KEY, &next.to_json(), before);
    }
}
