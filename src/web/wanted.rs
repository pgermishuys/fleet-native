//! Which page is on screen, and where, read from the state alone (component 14, piece A; rows 14.3 and 14.4). Plain Rust, so
//! the rule is tested with no window; `web::Host::apply` does what it says to the native web views.
use crate::State;
use crate::fleet::canvases::CanvasKind;
use crate::fleet::web_page::{ShownPage, page_url, shown_page};

/// What the window should show of the web views.
#[derive(Clone, Debug, PartialEq)]
pub enum Wanted {
    /// No page tab is shown: another route, the panel collapsed, no page asked for, or it has no address.
    Nothing,
    /// A page tab is shown but a Rune overlay is open, which a native view would cover (decision 7): the page keeps its state
    /// out of sight.
    Hidden {
        tab: String,
    },
    Shown(Placement),
}

/// A page on screen: its tab, its address, what makes it load again, and its rectangle in window points.
#[derive(Clone, Debug, PartialEq)]
pub struct Placement {
    pub tab: String,
    pub url: String,
    /// `frameKey`'s `{pageId}:{shownAt}`: a new value loads the page again (row 14.27).
    pub load_key: String,
    pub rect: [f32; 4],
}

impl Wanted {
    pub fn tab(&self) -> Option<&str> {
        match self {
            Self::Nothing => None,
            Self::Hidden { tab } => Some(tab),
            Self::Shown(p) => Some(&p.tab),
        }
    }
}

/// The Fleet a session's page is served from. Another machine's base is piece C's (row 14.16); until then, home's. With no
/// server, the address the invented pages are served at (`web::Host::mock_base`).
pub fn page_base(s: &State) -> Option<String> {
    s.link.as_ref().map(|l| l.base.clone()).or_else(|| s.web.mock_base.clone())
}

/// The open session's active tab when it is a page tab: its id, and the page its server canvas shows (component 10's
/// `CanvasInstance::server` carries the canvas's state).
pub fn page_tab(s: &State) -> Option<(String, ShownPage)> {
    let tabs = s.open_canvases();
    let active = tabs.active().filter(|c| c.kind == CanvasKind::Page)?;
    let server = active.server.as_ref()?;
    Some((active.id.clone(), shown_page(&server.state)))
}

/// The page canvas the content panel shows, with its address, while the open session is on screen with its panel open.
pub fn shown_tab(s: &State) -> Option<(String, String, String)> {
    if s.rail != crate::Rail::Sessions || !s.panel_body_shown(CanvasKind::Page) {
        return None;
    }
    let (tab, page) = page_tab(s)?;
    let url = page_url(&page_base(s)?, &page)?;
    Some((tab, url, format!("{}:{}", page.page_id, page.shown_at)))
}

/// `wanted`: the rule the host follows every frame.
pub fn wanted(s: &State) -> Wanted {
    let Some((tab, url, load_key)) = shown_tab(s) else { return Wanted::Nothing };
    let Some(rect) = s.web.rect.filter(|r| r[2] >= 1.0 && r[3] >= 1.0) else { return Wanted::Nothing };
    if s.ui.open.is_some() {
        return Wanted::Hidden { tab };
    }
    Wanted::Shown(Placement { tab, url, load_key, rect })
}
