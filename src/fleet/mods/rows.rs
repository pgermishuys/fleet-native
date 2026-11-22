//! What a tool row reads of its mods when it draws (component 22, piece E): the checked tree for its line and its body, found by
//! the open session and the call's id (not by the tool), the keys the trees' elements and controls are known by, and the routing
//! of a used control to the call it is in. The asking is `renders.rs`; the drawing is `view::mods::row`.
use super::points::{ToolRowSite, ToolRowView, tool_row_view_id};
use super::resolve::Resolved;
use super::types::{Site, Surface};
use crate::State;
use serde_json::Value;
use std::sync::Arc;

pub fn site_of(site: ToolRowSite) -> Site {
    match site {
        ToolRowSite::ToolUse => Site::ToolUse,
        ToolRowSite::ToolResult => Site::ToolResult,
    }
}

/// The element key a row's tree is drawn under (`<key>.0`, `<key>.0.1`…), which also names its controls' state.
pub fn tree_key(site: ToolRowSite, session: &str, call: &str) -> String {
    format!("mod-{}:{session}:{call}", if site == ToolRowSite::ToolUse { "line" } else { "body" })
}

/// Whether `handle` is one of the callbacks a tree holds.
pub(super) fn holds(node: &Value, handle: &str) -> bool {
    let Some(map) = node.as_object() else { return false };
    map.get("handles").and_then(Value::as_object).is_some_and(|h| h.values().any(|v| v.as_str() == Some(handle)))
        || map.get("children").and_then(Value::as_array).is_some_and(|kids| kids.iter().any(|k| holds(k, handle)))
}

impl State {
    /// The session whose rows are on screen: the open one (a scene's invented session has no server id, so its own).
    pub fn mod_row_session(&self) -> String {
        self.open_id().unwrap_or_else(|| self.session().id.clone())
    }

    /// What a mod draws at `site` of this call's row: the tree, checked and cut (a `null` tree is `Value::Null`, which draws
    /// nothing). `None` is Fleet's own drawing: no view, a view of no mod, or a tree that broke the contract (reported once).
    pub fn mod_row_tree(&self, site: ToolRowSite, call: &str) -> Option<Arc<Value>> {
        if call.is_empty() || self.mod_renders.views.is_empty() {
            return None;
        }
        let view = self.mod_renders.view(site, &self.mod_row_session(), call)?;
        match self.mod_resolver.resolve(Some(&view), site_of(site)) {
            Resolved::Draws { tree, .. } => Some(tree),
            Resolved::NothingDrawn => None,
        }
    }

    /// Changes whenever something a call's row draws from its mods changes: its two views (each its own contribution) and what
    /// its body's controls show. Nothing of another row moves it. Zero while no mod has drawn anything.
    pub fn mod_row_stamp(&self, call: &str) -> (u64, u64, u64, u32) {
        if call.is_empty() || self.mod_renders.views.is_empty() {
            return (0, 0, 0, 0);
        }
        let session = self.mod_row_session();
        let stamp = |site| self.mod_renders.views.stamp(&tool_row_view_id(site, &session, call));
        let (line, body) = (stamp(ToolRowSite::ToolUse), stamp(ToolRowSite::ToolResult));
        let controls = if body == 0 { 0 } else { self.mods_ui.stamp(&tree_key(ToolRowSite::ToolResult, &session, call)) };
        // The line's cap and the title's floor are shares of the row's measured width.
        (line, body, controls, if line == 0 { 0 } else { self.mod_line_w.to_bits() })
    }

    /// The controls of each row's body know their tree (`ModControls::sync`): a new Input starts, a changed `value` replaces its
    /// text, and one whose view went is let go. A tree already synced is skipped, so the cost is a look at each view. True when
    /// anything moved.
    pub fn sync_row_controls(&mut self) -> bool {
        if self.mod_renders.views.is_empty() && self.mods_ui.row_seen.is_empty() {
            return false;
        }
        let mut now = Vec::new();
        for view in self.mod_renders.views.items() {
            if view.site != ToolRowSite::ToolResult {
                continue;
            }
            if let Resolved::Draws { tree, .. } = self.mod_resolver.resolve(Some(&view), Site::ToolResult) {
                now.push((tree_key(view.site, &view.session_id, &view.call_id), tree));
            }
        }
        let gone: Vec<String> = self.mods_ui.row_seen.keys().filter(|k| !now.iter().any(|(n, _)| n == *k)).cloned().collect();
        let mut moved = !gone.is_empty();
        for key in gone {
            self.mods_ui.clear(&key);
            self.mods_ui.row_seen.remove(&key);
        }
        for (key, tree) in now {
            if self.mods_ui.row_seen.get(&key).is_some_and(|seen| Arc::ptr_eq(seen, &tree)) {
                continue;
            }
            self.mods_ui.sync(&key, &tree);
            self.mods_ui.row_seen.insert(key, tree);
            moved = true;
        }
        moved
    }

    /// The row a handle belongs to: the session and the call of the view whose tree holds it.
    fn mod_row_of_handle(&self, handle: &str) -> Option<ToolRowView> {
        self.mod_renders.views.items().into_iter().find(|v| holds(&v.view.tree, handle)).map(|v| (*v).clone())
    }

    /// Each control used since the last frame goes to its mod as `desktop` (`ToolCard`'s `onAction`): `POST …/mods/action` with the
    /// session's machine, and the call's row draws again when the server doesn't take it. A handle no row holds any more is dropped.
    pub fn deliver_mod_actions(&mut self) -> bool {
        if self.mods_ui.pending.is_empty() {
            return false;
        }
        for action in std::mem::take(&mut self.mods_ui.pending) {
            if let Some(row) = self.mod_row_of_handle(&action.handle) {
                self.mod_action(&row.session_id, &row.call_id, action, Surface::Desktop);
            } else if let Some(pane) = self.mod_pane_of_handle(&action.handle) {
                self.mod_pane_action(&pane, action);
            }
        }
        true
    }
}
