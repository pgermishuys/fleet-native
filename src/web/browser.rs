//! The browser tab's own state and rules (component 14, piece H; rows 14.51 to 14.58 and 14.60). Ports the script of
//! `components/canvas/BrowserCanvas.vue`: show an address through Fleet's preview, follow what the page's bridge says, answer
//! the bar. Plain Rust with no window, so every rule is tested headless; the host (`platform`) only loads `frame_src`.
use super::preview_bridge::{BridgeMessage, NavAction, PreviewHmr, app_address, read_bridge_message};
use crate::State;
use crate::fleet::canvases::{CanvasKind, server_tab_id};
use crate::fleet::http;
use crate::fleet::settings::HOME_MACHINE_KEY;
use serde_json::{Value, json};
use url::Url;

/// What one browser tab knows, kept for as long as the tab is open (Fleet keeps the component alive).
#[derive(Clone, Debug, Default)]
pub struct BrowserUi {
    /// The address bar's text.
    pub address: rune::TextEdit,
    /// The address field has the keyboard: what the page reports no longer overwrites it.
    pub editing: bool,
    /// The field had the keyboard at the last look, so gaining it (not having it) starts an edit and Enter ends one: Fleet's
    /// `@focus` and `@blur`, with the field still focused after Enter.
    pub focused: bool,
    /// Where this machine loads the preview, once Fleet gave it (`proxyOrigin`).
    pub proxy_origin: Option<String>,
    /// The page shown, as the app's own address (`target`).
    pub target: Option<Url>,
    /// What the web view loads: the preview's origin and the target's path, query and hash.
    pub frame_src: Option<String>,
    /// The `url` or the app's page last handed to `show` (`watch(pageUrl)`).
    pub wanted: Option<String>,
    pub error: Option<String>,
    /// A hello nobody here asked for, for the page that was already shown, is the page reloading itself.
    pub expecting_load: bool,
    pub last_href: Option<String>,
    /// From the page's hello; none until a page with the bridge has loaded.
    pub hmr: Option<PreviewHmr>,
    /// Reload with no bridge, and a page asked for again: part of the load key.
    pub loads: u32,
    /// Which question to Fleet is the latest, so an old answer lands on nothing.
    pub ask: u64,
    /// The app this tab shows, kept from its canvas (`appId`).
    pub app_id: Option<String>,
    /// What the app's panel, strip and output keep (piece I).
    pub app: super::browser_app::AppUi,
    /// The user chose Agent's view (`view === 'agent'`); it only counts while the agent has a tab on this page (piece J).
    pub agent: bool,
}

/// What the host is to do to a tab's web view besides loading it: run this in the page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NavRequest {
    pub tab: String,
    pub script: &'static str,
}

impl BrowserUi {
    /// `submitAddress`: digits are a port on this machine, a missing scheme is `http://`.
    pub fn typed_address(typed: &str) -> String {
        let value = typed.trim();
        if !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()) {
            return format!("http://localhost:{value}/");
        }
        let lower = value.to_ascii_lowercase();
        if lower.starts_with("http://") || lower.starts_with("https://") { value.to_string() } else { format!("http://{value}") }
    }

    pub fn text(&self) -> String {
        self.address.text.clone()
    }

    pub fn set_address(&mut self, text: &str) {
        self.address = rune::TextEdit::new(text);
    }

    fn origin(&self) -> Option<String> {
        self.target.as_ref().map(|t| t.origin().ascii_serialization())
    }
}

/// The preview's start for the web view: `origin + pathname + search + hash`.
fn frame_of(proxy_origin: &str, page: &Url) -> String {
    let mut out = format!("{proxy_origin}{}", page.path());
    if let Some(q) = page.query() {
        out.push('?');
        out.push_str(q);
    }
    if let Some(f) = page.fragment() {
        out.push('#');
        out.push_str(f);
    }
    out
}

/// What `openOutside` opens: the preview's address of the page in the bar (the app's own `localhost` is only right on Fleet's
/// machine), or the bar's text as it is when there is no preview yet.
pub fn outside_address(address: &str, proxy_origin: Option<&str>) -> String {
    match proxy_origin {
        Some(origin) => match Url::parse(address) {
            Ok(page) => frame_of(origin, &page),
            Err(_) => format!("{origin}/"),
        },
        None => address.to_string(),
    }
}

impl State {
    pub fn browser_ui(&self, tab: &str) -> Option<&BrowserUi> {
        self.web.browsers.get(tab)
    }

    fn browser_ui_mut(&mut self, tab: &str) -> &mut BrowserUi {
        self.web.browsers.entry(tab.to_string()).or_default()
    }

    /// The session is on a machine other than home: its preview needs that machine's own sign-in, so the tab says where the app
    /// opens instead (`elsewhere`).
    pub fn browser_elsewhere(&self) -> bool {
        self.machine_key_of(&self.canvas_session()) != HOME_MACHINE_KEY
    }

    /// The machine's name, and where its own web app shows this session (none when it has no web app).
    pub fn browser_machine(&self) -> (String, Option<String>) {
        let session = self.canvas_session();
        let key = self.machine_key_of(&session).to_string();
        let name = self.machines.machine(&key).map_or_else(|| "that machine".to_string(), |m| m.name.clone());
        let web_app =
            self.machines.entries().into_iter().find(|e| e.key == key).and_then(|e| e.info).and_then(|i| i.web_app) != Some(false);
        let there = self.machines.machine(&key).filter(|_| web_app).map(|m| format!("{}/sessions/{session}", m.base_url));
        (name, there)
    }

    /// The page a browser tab shows: its own `url`, else the page its app serves (`pageUrl`).
    pub fn browser_page_of(&self, page: &crate::fleet::browser_tab::BrowserPage) -> String {
        if !page.url.is_empty() {
            return page.url.clone();
        }
        page.app_id.as_deref().and_then(|id| self.apps.app(id)).and_then(|a| a.url.clone()).unwrap_or_default()
    }

    /// From `poll`: the browser tab on screen shows what it should (`onMounted`, `watch(pageUrl)`), and the address field says
    /// whether it has the keyboard. True when something to draw changed.
    pub fn keep_browser(&mut self, tx: &mut rune::Tx) -> bool {
        let agent_changed = self.keep_browser_agent();
        let tabs = self.open_canvases();
        let active = tabs.active().filter(|c| c.kind == CanvasKind::Browser).and_then(|c| Some((c.id.clone(), c.browser()?)));
        let on_screen = active.filter(|_| self.rail == crate::Rail::Sessions && self.content_open && !self.browser_elsewhere());
        // A tab that isn't on screen counts as gone when it comes back (`onActivated`).
        for (id, ui) in &mut self.web.browsers {
            if on_screen.as_ref().is_none_or(|(t, _)| t != id) {
                ui.app.visible = false;
                ui.app.out_key = None;
            }
        }
        let Some((tab, page)) = on_screen else { return agent_changed };
        let focused = tx.focused() == Some(ADDRESS_KEY);
        let mut changed = agent_changed;
        let want = self.browser_page_of(&page);
        let ui = self.browser_ui_mut(&tab);
        match (focused, ui.focused) {
            (true, false) => ui.editing = true,
            (false, _) => ui.editing = false,
            _ => {}
        }
        ui.focused = focused;
        if ui.wanted.as_ref() != Some(&want) {
            ui.wanted = Some(want.clone());
            if ui.text().is_empty() && !want.is_empty() {
                ui.set_address(&want);
            }
            self.browser_show(&tab, &want);
            changed = true;
        }
        changed | self.keep_browser_app(&tab, &page.url, page.app_id.as_deref(), tx)
    }

    /// `show`: the page through Fleet's preview. The question to Fleet is asked once per origin.
    pub fn browser_show(&mut self, tab: &str, page: &str) {
        let session = self.canvas_session();
        if page.is_empty() {
            let ui = self.browser_ui_mut(tab);
            (ui.error, ui.target, ui.frame_src) = (None, None, None);
            return;
        }
        let Ok(parsed) = Url::parse(page).ok().filter(|u| matches!(u.scheme(), "http" | "https") && u.has_host()).ok_or(()) else {
            self.browser_ui_mut(tab).error = Some(format!("{page} isn't an address."));
            return;
        };
        let link = self.link_for(&session);
        let ui = self.browser_ui_mut(tab);
        ui.error = None;
        let asked = ui.proxy_origin.is_some() && ui.origin().as_deref() == Some(&parsed.origin().ascii_serialization());
        if asked {
            let origin = ui.proxy_origin.clone().unwrap_or_default();
            self.browser_shown(tab, parsed, &origin);
            return;
        }
        let Some(link) = link else { return };
        ui.ask += 1;
        let ask = ui.ask;
        let (tab, path, body) = (tab.to_string(), format!("/api/sessions/{session}/browser/proxy"), json!({"url": parsed.to_string()}));
        let origin = parsed.origin().ascii_serialization();
        link.off_thread(
            move |base| http::text_with("POST", base, &path, Some(&body)),
            move |s, _, result| {
                let Some(ui) = s.web.browsers.get_mut(&tab).filter(|u| u.ask == ask) else { return };
                let failure = |words: String| format!("Fleet couldn't show {origin}: {words}");
                match result {
                    Err(e) => ui.error = Some(failure(e)),
                    Ok((status, text)) if !(200..300).contains(&status) => {
                        let said = serde_json::from_str::<Value>(&text).ok().and_then(|b| b.get("error")?.as_str().map(str::to_string));
                        ui.error = Some(failure(said.unwrap_or_else(|| format!("HTTP {status}"))));
                    }
                    Ok((_, text)) => {
                        match serde_json::from_str::<Value>(&text).ok().and_then(|b| b.get("origin")?.as_str().map(str::to_string)) {
                            Some(proxy) => {
                                ui.proxy_origin = Some(proxy.clone());
                                s.browser_shown(&tab, parsed, &proxy);
                            }
                            None => ui.error = Some(failure("its answer had no address".to_string())),
                        }
                    }
                }
            },
        );
    }

    fn browser_shown(&mut self, tab: &str, page: Url, proxy: &str) {
        let ui = self.browser_ui_mut(tab);
        if !ui.editing {
            ui.set_address(page.as_str());
        }
        ui.expecting_load = true;
        ui.hmr = None;
        ui.frame_src = Some(frame_of(proxy, &page));
        ui.target = Some(page);
    }

    /// Enter in the address field (`submitAddress`).
    pub fn browser_submit(&mut self, tab: &str) {
        let ui = self.browser_ui_mut(tab);
        ui.editing = false;
        let value = BrowserUi::typed_address(&ui.text());
        self.browser_show(tab, &value);
    }

    /// Back, Forward and Reload (`navigate`): the page's own history when it has the bridge, else Reload sets the address
    /// again. A web view has the page's history whether or not its bridge spoke, so it is asked directly.
    pub fn browser_navigate(&mut self, tab: &str, action: NavAction) {
        let ui = self.browser_ui_mut(tab);
        // Back and forward land on another address, which never counts as the page reloading itself.
        if action == NavAction::Reload {
            ui.expecting_load = true;
        }
        let live = self.web.can_show() && self.web.applied.as_ref().and_then(super::Wanted::tab) == Some(tab);
        if live {
            self.web.nav.push(NavRequest {
                tab: tab.to_string(),
                script: match action {
                    NavAction::Back => "history.back()",
                    NavAction::Forward => "history.forward()",
                    NavAction::Reload => "location.reload()",
                },
            });
        } else if action == NavAction::Reload {
            self.browser_ui_mut(tab).loads += 1;
        }
    }

    /// The preview's address for the page in the bar, for the system's browser (`openOutside`).
    pub fn browser_outside_url(&self, tab: &str) -> Option<String> {
        let ui = self.browser_ui(tab)?;
        Some(outside_address(&ui.text(), ui.proxy_origin.as_deref()))
    }

    /// A message from the preview bridge of the page in `tab` (`onMessage`). The host hears only the tab's own web view, so
    /// there is no origin to compare; an old or foreign message is ignored by `read_bridge_message`.
    pub fn hear_preview(&mut self, tab: &str, data: &Value) {
        let Some(message) = read_bridge_message(data) else { return };
        let session = self.canvas_session();
        let now = self.clock;
        let Some(ui) = self.web.browsers.get_mut(tab) else { return };
        let (Some(proxy), Some(target)) = (ui.proxy_origin.clone(), ui.target.clone()) else { return };
        let report = |ui: &mut BrowserUi, href: &str| {
            ui.last_href = Some(href.to_string());
            if !ui.editing {
                let shown = app_address(href, &proxy, &target.origin().ascii_serialization());
                ui.set_address(&shown);
            }
        };
        match message {
            BridgeMessage::Hello { hmr, href, .. } => {
                ui.hmr = Some(hmr);
                let reloaded = !ui.expecting_load && ui.last_href.as_deref() == Some(&href);
                ui.expecting_load = false;
                report(ui, &href);
                if reloaded {
                    self.canvases.mark_updated(tab, now);
                }
            }
            BridgeMessage::Location { href, .. } => report(ui, &href),
            BridgeMessage::Update => self.canvases.mark_updated(tab, now),
            BridgeMessage::Open { href, new_tab } => self.browser_open_link(tab, &session, &href, new_tab),
        }
    }

    /// `openLink`: another address on Fleet's machine opens through Fleet in a new tab, or here.
    fn browser_open_link(&mut self, tab: &str, session: &str, href: &str, new_tab: bool) {
        if !new_tab {
            self.browser_show(tab, href);
            return;
        }
        let (tab, href_again, asked) = (tab.to_string(), href.to_string(), session.to_string());
        self.open_address(session, href, move |s, result| match result {
            Ok(canvas) => s.load_server_canvases_then(&asked, Some(server_tab_id(&canvas))),
            // Fleet wouldn't open a tab for it: this one still can.
            Err(_) => s.browser_show(&tab, &href_again),
        });
    }
}

/// The address field's key.
pub const ADDRESS_KEY: &str = "browser-address";
