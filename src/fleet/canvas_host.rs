//! What the panel's tab strip needs from the model (`CanvasHost.vue`, `SessionsV2RightPanel.vue`): the badges on the tabs,
//! the tab that is showing, and the handlers behind a tab, its close button and the `+` menu. Plain logic; `view::canvas_host`
//! draws it.
use super::canvases::{CanvasKind, PICKABLE};
use super::smart_links::needs_attention;
use crate::{State, Tx};

/// The one mark a tab can carry after its title (`CanvasTabBadge`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Badge {
    /// A 7 px `--error` dot with its accessible label.
    Attention(String),
    /// A count, or `done/total`, with its accessible label.
    Count(String, String),
}

/// The id of the key a tab's element carries, `canvas-tab-{id}`.
pub fn tab_key(id: &str) -> String {
    format!("canvas-tab-{id}")
}

/// The key of a tab's close button.
pub fn close_key(id: &str) -> String {
    format!("canvas-tab-close-{id}")
}

/// The strip's scroll box, which the active tab is brought into.
pub const STRIP: &str = "canvas-tabs";

impl State {
    /// The session whose tabs the panel shows: the open one on a server, the shown one in the invented data.
    pub fn canvas_session(&self) -> String {
        self.open_id().unwrap_or_else(|| self.session().id.clone())
    }

    /// The id of the active tab (`activeCanvas`).
    pub fn active_canvas(&self) -> String {
        self.open_canvases().active_id
    }

    /// Whether the active tab's own body is drawn, so another component's view may take its place: the panel is open and
    /// that kind is active (`panel_body_shown`; component 14 asks it of a page or browser tab).
    pub fn panel_body_shown(&self, kind: CanvasKind) -> bool {
        self.content_open && self.open_canvases().active().is_some_and(|c| c.kind == kind)
    }

    /// The badge for each tab (`tabBadges`): Context counts the linked items, or flags one that needs attention; Progress
    /// shows `done/total`; Agents shows what is running or waiting.
    pub fn tab_badge(&self, id: &str) -> Option<Badge> {
        match id {
            "context" => {
                let links = self.context_links();
                if links.iter().any(|l| needs_attention(l)) {
                    return Some(Badge::Attention("A linked pull request needs attention".into()));
                }
                (!links.is_empty()).then(|| Badge::Count(links.len().to_string(), format!("{} linked", links.len())))
            }
            "progress" => {
                let p = self.session().row.progress.as_ref()?;
                (p.total > 0).then(|| Badge::Count(format!("{}/{}", p.done, p.total), format!("{} of {} done", p.done, p.total)))
            }
            "agents" => {
                let n = self.canvases.agents_running;
                (n > 0).then(|| Badge::Count(n.to_string(), format!("{n} running or waiting")))
            }
            _ => None,
        }
    }

    /// A tab was clicked or reached by key: it becomes active and the strip scrolls to it.
    pub fn activate_tab(&mut self, id: &str, tx: &mut Tx) {
        let id = id.to_string();
        self.edit_canvases(|c, session| c.activate(session, &id));
        self.reveal_active(tx);
    }

    /// Closes a tab; the one that takes its place shows (`close`). Changes can't be closed.
    pub fn close_tab(&mut self, id: &str, tx: &mut Tx) {
        let id = id.to_string();
        self.edit_canvases(|c, session| c.close(session, &id));
        self.reveal_active(tx);
    }

    /// A built-in picked in the `+` menu: added if missing and made active.
    pub fn open_built_in(&mut self, kind: CanvasKind, tx: &mut Tx) {
        self.edit_canvases(|c, session| c.open(session, kind));
        self.reveal_active(tx);
    }

    /// The active tab is scrolled into view whenever the active id or the number of tabs changes, a tab the agent just opened
    /// included (`watch([activeId, canvases.length])`). The scroll waits one frame, so a new tab has been laid out.
    pub fn keep_strip(&mut self, tx: &mut Tx) -> bool {
        let tabs = self.open_canvases();
        let now = (self.canvas_session(), tabs.active_id.clone(), tabs.canvases.len());
        if self.canvases.strip_seen.as_ref() != Some(&now) {
            self.canvases.strip_seen = Some(now);
            self.canvases.strip_due = true;
            return true;
        }
        if self.canvases.strip_due && self.content_open {
            self.canvases.strip_due = false;
            self.reveal_active(tx);
        }
        false
    }

    /// Scrolls the strip to the active tab (`scrollIntoView({inline: "nearest"})`), after the tabs are drawn.
    pub fn reveal_active(&self, tx: &mut Tx) {
        tx.scroll_into_view_in(STRIP, tab_key(&self.active_canvas()));
    }

    /// The tab `step` places from the active one, wrapping (`onTabKeydown`).
    pub fn neighbour_tab(&self, step: isize) -> Option<String> {
        let tabs = self.open_canvases();
        let at = tabs.canvases.iter().position(|c| c.id == tabs.active_id)? as isize;
        let n = tabs.canvases.len() as isize;
        Some(tabs.canvases[(at + step).rem_euclid(n) as usize].id.clone())
    }

    /// Home and End.
    pub fn edge_tab(&self, last: bool) -> Option<String> {
        let tabs = self.open_canvases();
        let tab = if last { tabs.canvases.last() } else { tabs.canvases.first() };
        tab.map(|c| c.id.clone())
    }

    /// The kinds the `+` menu offers, each with whether it is open already.
    pub fn picker_rows(&self) -> Vec<(CanvasKind, bool)> {
        let open = self.open_canvases();
        PICKABLE.iter().map(|k| (*k, open.canvases.iter().any(|c| c.id == k.slug()))).collect()
    }
}
