//! The selector dropdown's rules with nothing of Rune in them (`SelectorDropdown.vue`, `ModelSelector.vue`,
//! `AgentSelector.vue`): which rows a filter keeps, how the sections read while nothing is typed, where the
//! highlight starts and moves, what a model's row says, and where the menu goes. The view is `view/selector.rs`.
use super::choice::{AgentOption, ModelOption, default_model_description, model_description};
use super::format::format_tokens;
use super::groups::{Level, ModelPicker, ModelSection, group_models, model_picker_key};

/// The menu's width, and the room it leaves at the window's edge (`.selector-dropdown`, `MENU_WIDTH`).
pub const MENU_WIDTH: f32 = 380.0;
pub const EDGE: f32 = 8.0;
/// The gap between the chip and the menu (`bottom: calc(100% + 6px)`).
pub const GAP: f32 = 6.0;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Item {
    pub id: String,
    pub label: String,
    pub description: String,
    /// The provider's name.
    pub meta: String,
    pub price: Option<String>,
    /// The tooltip of the price chip.
    pub price_note: Option<String>,
    /// `Some`: the row has a star, filled when `true`.
    pub starred: Option<bool>,
    pub context_size: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Section {
    pub title: Option<String>,
    pub provider: bool,
    pub items: Vec<Item>,
}

/// A section as drawn: its rows already numbered across the whole menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shown {
    pub title: Option<String>,
    pub provider: bool,
    /// The provider is the heading, or the same on every row: not worth repeating on each.
    pub hide_meta: bool,
    pub rows: Vec<Row>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub item: Item,
    pub index: usize,
}

/// The substring a row is searched by: label, description, meta and id, in that order, lower case.
fn searchable(item: &Item) -> String {
    [item.label.as_str(), item.description.as_str(), item.meta.as_str(), item.id.as_str()]
        .iter()
        .filter(|v| !v.is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// `filteredItems`: nothing typed keeps all; else the rows that contain the text. Default (empty id) always stays.
pub fn filter(items: &[Item], query: &str) -> Vec<Item> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return items.to_vec();
    }
    items.iter().filter(|i| i.id.is_empty() || searchable(i).contains(&query)).cloned().collect()
}

/// `view`: sections while nothing is typed (when there are any), else one run with no heading.
pub fn shown(items: &[Item], sections: Option<&[Section]>, query: &str) -> Vec<Shown> {
    let flat;
    let source: &[Section] = match sections {
        Some(s) if query.trim().is_empty() => s,
        _ => {
            flat = [Section { title: None, provider: false, items: filter(items, query) }];
            &flat
        }
    };
    let mut index = 0;
    source
        .iter()
        .map(|s| {
            let mut metas: Vec<&str> = s.items.iter().map(|i| i.meta.as_str()).filter(|m| !m.is_empty()).collect();
            metas.sort_unstable();
            metas.dedup();
            let hide_meta = s.provider || metas.len() <= 1;
            let rows = s
                .items
                .iter()
                .map(|item| {
                    index += 1;
                    Row { item: item.clone(), index: index - 1 }
                })
                .collect();
            Shown { title: s.title.clone(), provider: s.provider, hide_meta, rows }
        })
        .collect()
}

pub fn rows(view: &[Shown]) -> Vec<&Row> {
    view.iter().flat_map(|s| s.rows.iter()).collect()
}

/// Where the highlight starts: on the selected row when nothing is typed (the first row if it isn't listed); on the
/// first match, not on Default, once something is.
pub fn start_active(view: &[Shown], query: &str, selected: &str) -> usize {
    let all = rows(view);
    if query.trim().is_empty() {
        all.iter().position(|r| r.item.id == selected).unwrap_or(0)
    } else {
        all.iter().position(|r| !r.item.id.is_empty()).unwrap_or(0)
    }
}

/// ArrowDown or ArrowUp: one row, clamped (no wrap).
pub fn move_active(active: usize, down: bool, len: usize) -> usize {
    if down { (active + 1).min(len.saturating_sub(1)) } else { active.saturating_sub(1) }
}

/// The left shift that keeps a 380 px menu inside the first clipping ancestor, or the window, with 8 px to spare
/// (`max(0, min(left + 380 - (clip - 8), left - 8))`).
pub fn shift_left(left: f32, clip_right: f32) -> f32 {
    (left + MENU_WIDTH - (clip_right - EDGE)).min(left - EDGE).max(0.0)
}

/// `max-width: calc(100vw - 16px)`: a window narrower than 396 shrinks the menu.
pub fn menu_width(window_w: f32) -> f32 {
    MENU_WIDTH.min((window_w - 2.0 * EDGE).max(0.0))
}

/// `max-height: min(320px, 60vh)`.
pub fn menu_height(window_h: f32) -> f32 {
    320f32.min(window_h * 0.6)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    pub down: bool,
    /// Set only by `auto`: the room on the chosen side.
    pub max_height: Option<f32>,
}

/// `place()`: "up" always opens above with no cap. "auto" opens below only when the room above is shorter than the
/// menu and the room below is bigger, and caps the height to the room on its side (at least 120).
pub fn place(auto: bool, chip_top: f32, chip_bottom: f32, bounds_top: f32, bounds_bottom: f32, window_h: f32) -> Placement {
    if !auto {
        return Placement { down: false, max_height: None };
    }
    let above = chip_top - bounds_top - GAP;
    let below = bounds_bottom - chip_bottom - GAP;
    let height = menu_height(window_h);
    let down = above < height && below > above;
    let room = (if down { below } else { above }) - EDGE;
    Placement { down, max_height: Some(120f32.max(height.min(room))) }
}

/// "N more": the rows whose middle is below the list's visible bottom. Each row is `(top, height)` in the same space.
pub fn more_below(rows: &[(f32, f32)], visible_bottom: f32) -> usize {
    rows.iter().filter(|(top, h)| top + h / 2.0 > visible_bottom).count()
}

const PRICES: [(&str, &str); 4] = [("low", "Low cost"), ("medium", "Medium cost"), ("high", "High cost"), ("very_high", "Very high cost")];

/// The price chip's text; an unknown tier draws nothing.
pub fn price_label(tier: &str) -> Option<&'static str> {
    PRICES.iter().find(|(k, _)| *k == tier).map(|(_, v)| *v)
}

fn capitalised(text: &str) -> String {
    let mut c = text.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

/// `itemFor`: one model's row. `starred` is `Some` only with a harness.
pub fn model_item(m: &ModelOption, starred: Option<bool>) -> Item {
    Item {
        id: m.selection_key.clone(),
        label: m.name.clone(),
        description: model_description(&m.id, &m.description).to_string(),
        meta: m.provider.clone(),
        price: m.price_tier.as_deref().and_then(price_label).map(str::to_string),
        price_note: m.capability.as_deref().filter(|c| !c.is_empty()).map(capitalised),
        starred,
        context_size: m.context_window.filter(|w| *w > 0).map(|w| format_tokens(Some(w as f64))),
    }
}

/// The Default row: its description names HydraFusion's cost when Default resolves to it.
pub fn default_item(label: &str, description: &str, model: bool) -> Item {
    let description = if model { default_model_description(label, description) } else { description };
    Item { label: label.into(), description: description.into(), ..Item::default() }
}

/// `AgentSelector`: Default, then each agent.
pub fn agent_items(default: Item, agents: &[AgentOption]) -> Vec<Item> {
    std::iter::once(default)
        .chain(agents.iter().map(|a| Item {
            id: a.id.clone(),
            label: a.name.clone(),
            description: a.description.clone(),
            ..Item::default()
        }))
        .collect()
}

/// `ModelSelector`: the flat list, and with a harness the sections from `groupModels` (Default on its own above them).
pub fn model_items(
    default: Item,
    models: &[ModelOption],
    harness: Option<&str>,
    picker: &ModelPicker,
    show_recent: bool,
) -> (Vec<Item>, Option<Vec<Section>>) {
    let star = |m: &ModelOption| harness.map(|h| picker.is_favourite(&model_picker_key(h, &m.provider_id, &m.id)));
    let items: Vec<Item> = std::iter::once(default.clone()).chain(models.iter().map(|m| model_item(m, star(m)))).collect();
    let Some(harness) = harness else { return (items, None) };
    let groups: Vec<ModelSection> = group_models(models, harness, &picker.favourites, &picker.recent, show_recent);
    let sections = std::iter::once(Section { items: vec![default], ..Section::default() })
        .chain(groups.into_iter().map(|g| Section {
            title: g.title,
            provider: g.level == Level::Provider,
            items: g.items.iter().map(|m| model_item(m, star(m))).collect(),
        }))
        .collect();
    (items, Some(sections))
}

/// The model a star on `selection_key` is for, as the preference's key.
pub fn star_key(models: &[ModelOption], harness: &str, selection_key: &str) -> Option<String> {
    models.iter().find(|m| m.selection_key == selection_key).map(|m| model_picker_key(harness, &m.provider_id, &m.id))
}
