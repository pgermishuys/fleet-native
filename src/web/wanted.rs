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
    /// A Document tab's HTML (already with its policy, `html_document`): loaded as it is, and `url` is empty.
    pub html: Option<String>,
    /// Where the page believes it sits, which its theme is named for (`pageFrameName`).
    pub place: super::page_theme::PagePlace,
    /// A browser tab's preview: it gets the preview bridge in place of the page bridge, and no theme (row 14.54).
    pub browser: bool,
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

/// The Fleet a session's page is served from: the machine the session is on (`apiUrlOn(machine.connection, …)`, row 14.16;
/// `/pages` is anonymous, so no token goes with it). With no server, the address the invented pages are served at
/// (`web::Host::mock_base`).
pub fn page_base(s: &State) -> Option<String> {
    s.target_of(&s.canvas_session()).map(|t| t.base).or_else(|| s.web.mock_base.clone())
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
    let reloads = s.web.reloads.get(&tab).copied().unwrap_or(0);
    Some((tab, url, format!("{}:{}:{reloads}", page.page_id, page.shown_at)))
}

/// The browser tab the content panel shows, with the preview address its web view loads and what makes it load again. Not while
/// the tab says an error, waits for an address, has its app's panel up, or the session is on another machine (rows 14.53 and 14.60).
pub fn shown_browser(s: &State) -> Option<(String, String, String)> {
    if s.rail != crate::Rail::Sessions || !s.panel_body_shown(CanvasKind::Browser) || s.agent_view.target.is_some() || s.browser_elsewhere()
    {
        return None;
    }
    let tab = s.active_canvas();
    let ui = s.browser_ui(&tab)?;
    // An app that is starting, exited or stopped has a panel where the page would be (row 14.61).
    let src = ui.frame_src.clone().filter(|_| ui.error.is_none() && s.browser_panel(&tab).is_none())?;
    let key = format!("{src}:{}", ui.loads);
    Some((tab, src, key))
}

/// The Document tab of an HTML payload the content panel shows: its tab, the document to load (scripts refused) and what makes it
/// load again, which is its content (`openVisual` replaces a revised one in its own tab).
pub fn shown_document(s: &State) -> Option<(String, String, String)> {
    use crate::fleet::canvases::VisualType;
    if s.rail != crate::Rail::Sessions || !s.panel_body_shown(CanvasKind::Visual) {
        return None;
    }
    let tabs = s.open_canvases();
    let active = tabs.active()?;
    let payload = active.payload.as_ref().filter(|p| p.kind == VisualType::Html)?;
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    std::hash::Hash::hash(&payload.content, &mut hash);
    let key = format!("{}:{}:{:x}", active.id, payload.content.len(), std::hash::Hasher::finish(&hash));
    Some((active.id.clone(), super::html_document(&payload.content), key))
}

/// `wanted`: the rule the host follows every frame.
pub fn wanted(s: &State) -> Wanted {
    // Full size covers the whole window, so it is the only web view on screen while it is up.
    if s.page_full().is_some() {
        // A toast or notice floating over it hides it only there, as for a tab's page.
        return match super::full::wanted_full(s) {
            None => Wanted::Nothing,
            Some(p) if s.web.cover.over(p.rect) => Wanted::Hidden { tab: p.tab },
            Some(p) => Wanted::Shown(p),
        };
    }
    let browser = shown_browser(s);
    let (tab, url, html, load_key) = match (shown_tab(s), browser.clone(), shown_document(s)) {
        (Some((tab, url, key)), ..) => (tab, url, None, key),
        (None, Some((tab, url, key)), _) => (tab, url, None, key),
        (None, None, Some((tab, html, key))) => (tab, String::new(), Some(html), key),
        (None, None, None) => return Wanted::Nothing,
    };
    let Some(rect) = s.web.rect.filter(|r| r[2] >= 1.0 && r[3] >= 1.0) else { return Wanted::Nothing };
    // A native view is over everything Rune draws: out of the way for a menu or a dialog, and while the panel eases to its
    // width (Rune reports where an eased element comes to rest, so the page would be there at once); under a toast or a tooltip
    // only where they are.
    if s.ui.open.is_some() || s.web.cover.whole() || s.web.cover.over(rect) || s.clock < s.web.settle_until {
        return Wanted::Hidden { tab };
    }
    Wanted::Shown(Placement {
        browser: browser.is_some_and(|(t, ..)| t == tab),
        tab,
        url,
        load_key,
        rect,
        html,
        place: super::page_theme::PagePlace::Tab,
    })
}
