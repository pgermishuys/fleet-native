//! Fleet's side of a GitHub page (`composables/use-github-sessions.ts`): the sessions working on each pull request or issue (read
//! from the sessions' header links), opening one, and starting a new one from an item.
use super::items::{ItemSummary, item_resource_id, item_session_preset};
use crate::fleet::row_status::{self, Glyph, RowItem, Tone};
use crate::mock::Session;
use crate::view::session_row::RowClock;
use crate::{Rail, State, Tx};

/// One session working on an item, with what its chip says (`GitHubSessionChip.vue`).
#[derive(Clone, Debug, PartialEq)]
pub struct ItemSession {
    pub id: String,
    /// The machine it is on; `None` for this one.
    pub machine: Option<String>,
    /// "Untitled session" for a session with no title.
    pub title: String,
    /// The word the chip shows: "Needs you", what the session is doing, or "Idle".
    pub words: String,
    pub tone: Tone,
    /// Something is going on (`isSessionLive`): the chip draws the session's glyph, else a message icon.
    pub live: bool,
    pub glyph: Glyph,
    /// The state in words, for the glyph's tooltip.
    pub description: String,
    pub waiting: bool,
    updated: i64,
}

impl ItemSession {
    fn of(session: &Session, machine: Option<&str>, clock: &RowClock) -> ItemSession {
        let item = RowItem::of(session, clock.now);
        let status = row_status::row_status(&item, clock.now, clock.offset_secs, true);
        let waiting = item.status == "waiting_input";
        let words = if waiting {
            "Needs you".to_string()
        } else if status.tone == Tone::Working {
            status.description.clone()
        } else {
            "Idle".to_string()
        };
        let title = session.title.trim();
        ItemSession {
            id: session.id.clone(),
            machine: machine.map(str::to_string),
            title: if title.is_empty() { "Untitled session".into() } else { title.to_string() },
            words,
            tone: status.tone,
            live: row_status::is_live(item.status),
            glyph: row_status::glyph(&item),
            description: status.description,
            waiting,
            updated: if session.row.updated > 0 { session.row.updated } else { session.row.created },
        }
    }
}

/// `sessionsFor`: the sessions whose header links point at `owner/repo#n` (ignoring case), not archived, most recently active
/// first. The sessions are the ones this app lists, here and on the other machines.
pub fn sessions_for(s: &State, resource_id: &str) -> Vec<ItemSession> {
    let ids = s.smart_links.sessions_for(resource_id);
    if ids.is_empty() {
        return Vec::new();
    }
    let clock = RowClock::now(s);
    let mut found: Vec<ItemSession> = Vec::new();
    let mut take = |session: &Session, machine: Option<&str>| {
        if ids.contains(&session.id) && !session.row.archived && !found.iter().any(|f| f.id == session.id) {
            found.push(ItemSession::of(session, machine, &clock));
        }
    };
    for session in s.projects.iter().flat_map(|p| p.sessions.iter()) {
        take(session, None);
    }
    for (machine, other) in &s.machines.others {
        for session in &other.sessions {
            take(session, Some(machine));
        }
    }
    found.sort_by(|a, b| b.updated.cmp(&a.updated).then_with(|| a.id.cmp(&b.id)));
    found
}

/// The sessions working on an item.
pub fn sessions_of(s: &State, item: &ItemSummary) -> Vec<ItemSession> {
    sessions_for(s, &item_resource_id(item))
}

/// The pages load the header links when they open (the sessions list is already kept by the app). Asked once; pushed updates
/// keep them current.
pub fn ensure_loaded(s: &mut State) {
    s.ensure_header_links_loaded();
}

/// `openSession`: the session's page, on its machine.
pub fn open_session(s: &mut State, session: &ItemSession, tx: &mut Tx) {
    s.rail = Rail::Sessions;
    match &session.machine {
        Some(machine) => s.open_machine_session(machine, &session.id, tx),
        None => {
            let at = s
                .projects
                .iter()
                .enumerate()
                .find_map(|(p, project)| project.sessions.iter().position(|x| x.id == session.id).map(|i| (p, i)));
            if let Some((p, i)) = at {
                s.open_session(p, i, tx);
            }
        }
    }
}

/// `startSession`: the sessions panel opens and the rail goes to Sessions, and only then does the item become the New session page's
/// initial source and the page open (so the page that takes it is the one on screen). The body is passed only from an item's page.
pub fn start_session(s: &mut State, item: &ItemSummary, body: Option<&str>) {
    s.sidebar_open = true;
    s.rail = Rail::Sessions;
    s.new_draft.set_initial_source(Some(item_session_preset(item, body)));
    s.open_new_session_page(None);
}
