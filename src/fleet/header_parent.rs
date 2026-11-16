//! The session a header's "Forked from" pill and "Delegated subagent" banner point to: its title, and opening it
//! (`useSessionTitle`, `lineageParentTitle`, the route's `handleBackToParent`).
use crate::State;
use rune::Tx;
use serde_json::Value;

impl State {
    /// The parent's title: from the list, else the one read for it, else nothing yet.
    pub fn parent_title(&self, id: &str) -> Option<String> {
        let listed = self.store.session_by_id(&self.projects, id).map(|s| s.title.trim().to_string());
        listed.filter(|t| !t.is_empty()).or_else(|| self.parent_titles.get(id).cloned())
    }

    /// Whether the list (or a kept session of another machine) holds the parent.
    pub fn parent_listed(&self, id: &str) -> bool {
        self.store.session_by_id(&self.projects, id).is_some()
    }

    /// Reads the title of a parent the list doesn't hold, once, off the UI thread. The session's own machine answers.
    pub fn load_parent_title(&mut self, parent: &str) {
        if self.parent_listed(parent) || self.parent_titles.contains_key(parent) {
            return;
        }
        let Some(link) = self.link_for(parent) else { return };
        let id = parent.to_string();
        let path = format!("/api/sessions/{}", crate::conv::image_sniff::encode_component(parent));
        link.off_thread(
            move |base| super::http::get(base, &path),
            move |s, _, result| {
                let title = result.ok().and_then(|v: Value| v.get("title").and_then(Value::as_str).map(|t| t.trim().to_string()));
                // A parent the machine can't give leaves the pill on "another session".
                if let Some(title) = title.filter(|t| !t.is_empty()) {
                    s.parent_titles.insert(id, title);
                }
            },
        );
    }

    /// Opens the parent in place when the list holds it (a plain click). One the list doesn't hold isn't opened here.
    pub fn open_parent(&mut self, id: &str, tx: &mut Tx) {
        let at = self.projects.iter().enumerate().find_map(|(p, project)| project.sessions.iter().position(|s| s.id == id).map(|i| (p, i)));
        if let Some((p, i)) = at {
            self.open_session(p, i, tx);
        }
    }
}
