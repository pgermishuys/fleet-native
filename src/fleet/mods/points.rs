//! Where mods' trees reach Fleet's screens (port of `lib/mods/points.ts` and the part of `lib/contributions.ts` it
//! uses): one `Views` per kind of site, ids as the TypeScript builds them. A site with no view draws exactly what it
//! drew before mods. A row asks "did my view change?" with a `ViewWatch`, so one call's new view redraws that row alone.
use super::types::{Action, Author, Surface};
use serde_json::Value;
use std::sync::Arc;

/// A control in the tree was used, on this surface.
pub type OnAction = Arc<dyn Fn(&Action, Surface) + Send + Sync>;

/// What every site's view carries.
#[derive(Clone)]
pub struct ModTreeView {
    /// `Null` with mods draws nothing (at `ToolResult`, an empty body). No mods means "no mod draws here": remove the view.
    pub tree: Arc<Value>,
    /// The mods that drew it, outermost first.
    pub mods: Vec<Author>,
    pub on_action: Option<OnAction>,
}

impl ModTreeView {
    pub fn new(tree: Value, mods: Vec<Author>) -> Self {
        ModTreeView { tree: Arc::new(tree), mods, on_action: None }
    }
}

/// A view that holds a tree.
pub trait HasTree {
    fn base(&self) -> &ModTreeView;
}

impl HasTree for ModTreeView {
    fn base(&self) -> &ModTreeView {
        self
    }
}

/// A view a `Views` can hold: it knows its own id, and the label to sort by.
pub trait Keyed: HasTree {
    fn id(&self) -> String;
    fn label(&self) -> Option<&str> {
        None
    }
}

/// A tool row's line (`ToolUse`) or opened body (`ToolResult`), for one call.
#[derive(Clone)]
pub struct ToolRowView {
    pub site: ToolRowSite,
    /// The session the call is in. A row finds its view by this, the call id and the site, not by the tool.
    pub session_id: String,
    pub call_id: String,
    /// The canonical name from Fleet's tool registry. Data for the mod; a row doesn't match on it.
    pub tool: String,
    pub view: ModTreeView,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolRowSite {
    ToolUse,
    ToolResult,
}

pub fn tool_row_view_id(site: ToolRowSite, session_id: &str, call_id: &str) -> String {
    let site = match site {
        ToolRowSite::ToolUse => "ToolUse",
        ToolRowSite::ToolResult => "ToolResult",
    };
    format!("{site}:{session_id}:{call_id}")
}

impl HasTree for ToolRowView {
    fn base(&self) -> &ModTreeView {
        &self.view
    }
}

impl Keyed for ToolRowView {
    fn id(&self) -> String {
        tool_row_view_id(self.site, &self.session_id, &self.call_id)
    }
}

/// The band above a session's composer.
#[derive(Clone)]
pub struct ComposerBandView {
    pub session_id: String,
    pub view: ModTreeView,
}

impl HasTree for ComposerBandView {
    fn base(&self) -> &ModTreeView {
        &self.view
    }
}

impl Keyed for ComposerBandView {
    fn id(&self) -> String {
        self.session_id.clone()
    }
}

/// A chip at the end of the status bar, shown only while its session is on screen.
#[derive(Clone)]
pub struct StatusChipView {
    pub session_id: String,
    pub view: ModTreeView,
}

impl HasTree for StatusChipView {
    fn base(&self) -> &ModTreeView {
        &self.view
    }
}

impl Keyed for StatusChipView {
    fn id(&self) -> String {
        self.session_id.clone()
    }
}

/// A pane a mod opened in a session: a `mod` tab on desktop.
#[derive(Clone)]
pub struct ModPaneView {
    pub session_id: String,
    pub pane_id: String,
    pub title: String,
    /// The mod that opened it.
    pub mod_id: String,
    pub view: ModTreeView,
}

/// A pane id belongs to its mod: two mods may use the same one.
pub fn mod_pane_view_id(session_id: &str, mod_id: &str, pane_id: &str) -> String {
    format!("{session_id}:{mod_id}:{pane_id}")
}

/// Who contributes a pane's view: one owner per pane, so closing or redrawing it removes just that one.
pub fn mod_pane_owner(session_id: &str, mod_id: &str, pane_id: &str) -> String {
    format!("mod-pane:{}", mod_pane_view_id(session_id, mod_id, pane_id))
}

impl HasTree for ModPaneView {
    fn base(&self) -> &ModTreeView {
        &self.view
    }
}

impl Keyed for ModPaneView {
    fn id(&self) -> String {
        mod_pane_view_id(&self.session_id, &self.mod_id, &self.pane_id)
    }
    fn label(&self) -> Option<&str> {
        Some(&self.title)
    }
}

/// A tree from a draft is marked Draft wherever it shows (band, chip, pane tab).
pub fn is_draft_view(view: &ModTreeView) -> bool {
    view.mods.iter().any(|m| m.draft)
}

struct Entry<T> {
    owner: String,
    token: u64,
    view: Arc<T>,
}

/// One kind of site's views: unique by id, a later view with an id replaces the earlier one (keeping its place), each
/// owned so a mod can leave without anyone tracking what it added (`defineContributionPoint`).
pub struct Views<T: Keyed> {
    entries: Vec<(String, Entry<T>)>,
    next_token: u64,
}

impl<T: Keyed> Default for Views<T> {
    fn default() -> Self {
        Views { entries: Vec::new(), next_token: 0 }
    }
}

impl<T: Keyed> Views<T> {
    /// Adds views for an owner. Returns a token for `dispose`.
    pub fn contribute(&mut self, owner: &str, added: Vec<T>) -> u64 {
        self.next_token += 1;
        let token = self.next_token;
        for view in added {
            let id = view.id();
            let entry = Entry { owner: owner.to_string(), token, view: Arc::new(view) };
            match self.entries.iter_mut().find(|(i, _)| *i == id) {
                Some((_, slot)) => *slot = entry,
                None => self.entries.push((id, entry)),
            }
        }
        token
    }

    /// Removes the views that are still this `contribute` call's.
    pub fn dispose(&mut self, token: u64) {
        self.entries.retain(|(_, e)| e.token != token);
    }

    pub fn remove_by_owner(&mut self, owner: &str) {
        self.entries.retain(|(_, e)| e.owner != owner);
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn get(&self, id: &str) -> Option<Arc<T>> {
        self.entries.iter().find(|(i, _)| i == id).map(|(_, e)| e.view.clone())
    }

    /// Which contribution this id's view came from: it moves whenever the view is replaced and is never reused, so a row can
    /// tell "my view changed" with one number (0 when there is none). A pointer would do until a freed view's address came back.
    pub fn stamp(&self, id: &str) -> u64 {
        self.entries.iter().find(|(i, _)| i == id).map_or(0, |(_, e)| e.token)
    }

    pub fn owner_of(&self, id: &str) -> Option<&str> {
        self.entries.iter().find(|(i, _)| i == id).map(|(_, e)| e.owner.as_str())
    }

    /// Every view, by label (a point without labels keeps the order the views came in).
    pub fn items(&self) -> Vec<Arc<T>> {
        let mut items: Vec<Arc<T>> = self.entries.iter().map(|(_, e)| e.view.clone()).collect();
        items.sort_by_key(|v| v.label().unwrap_or_default().to_lowercase());
        items
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// One id's view, tracked on its own (`useModView`): `changed` is true only when the view is a different one from the
/// last it was given, so a change to another id's view doesn't redraw this row.
pub struct ViewWatch<T> {
    last: Option<Arc<T>>,
}

impl<T> Default for ViewWatch<T> {
    fn default() -> Self {
        ViewWatch { last: None }
    }
}

impl<T> ViewWatch<T> {
    pub fn changed(&mut self, now: Option<Arc<T>>) -> bool {
        let same = match (&self.last, &now) {
            (None, None) => true,
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            _ => false,
        };
        self.last = now;
        !same
    }
}
