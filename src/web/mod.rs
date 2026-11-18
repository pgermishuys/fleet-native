//! The web view host (component 14): the system's own web view (WebView2, WKWebView, WebKitGTK, through `wry`) as a child
//! of the window, laid over a page tab's body (`docs/PORT.md` decision 1, `docs/research/web-content.md`).
//!
//! A native view is on top of everything Rune draws, so the state decides where it is (`wanted`) and the host only follows:
//! every frame `State::keep_web` compares what is wanted with what was last done, and moves, loads or hides the views when
//! they differ. Headless runs, scenes and builds without the `web` feature never get a window handle, so they draw the
//! fallback panel instead (decision 9).
pub mod body;
pub mod choice;
pub mod cover;
pub mod inject;
pub mod page_bridge;
pub mod page_theme;
pub mod wanted;

#[cfg(all(feature = "web", not(target_arch = "wasm32")))]
pub mod demo;
#[cfg(all(feature = "web", not(target_arch = "wasm32")))]
pub mod platform;

pub use body::{html_body, html_document};
pub use wanted::{Placement, Wanted, wanted};

/// What the state knows of the web views. The views themselves are native and live on the UI thread (`platform`).
#[derive(Debug, Default)]
pub struct Host {
    /// The page area's rectangle in window points, from its `on_layout`.
    pub rect: Option<[f32; 4]>,
    /// The same in screen points (the CI walk clicks at it).
    pub screen: Option<[f32; 4]>,
    /// The window handed over its native handle and the system's side started (GTK, COM).
    pub available: bool,
    /// Why no web view could be made (a Wayland window, no WebView2 runtime); the fallback shows from then on.
    pub failed: Option<String>,
    /// With no server: where the invented pages are served (the CI walk's local server).
    pub mock_base: Option<String>,
    /// A click in the page gave it the keyboard (it told us over IPC); Rune has none until one of its elements takes it.
    pub page_has_keys: bool,
    /// What pages saved with `fleet:page-state`, kept for the app's run (row 14.31).
    pub kept: page_bridge::KeptStates,
    /// The theme the pages were last given, so a change is sent once (row 14.28).
    pub theme: Option<page_theme::PageTheme>,
    /// The pick each page tab holds while its agent waits (row 14.38), by tab.
    pub choices: std::collections::HashMap<String, choice::ChoiceUi>,
    /// How many times each page tab's Reload was pressed; part of the load key (row 14.27).
    pub reloads: std::collections::HashMap<String, u32>,
    /// What the overlay layer covers (row 14.4).
    pub cover: cover::Cover,
    /// When the panel finishes easing to its new width (Widen), on the poll clock: the page waits out of the window until then.
    pub settle_until: f64,
    /// What `keep_web` last did, so an unchanged frame touches nothing native.
    pub applied: Option<Wanted>,
    /// The CI walk, while it runs (`demo`).
    #[cfg(all(feature = "web", not(target_arch = "wasm32")))]
    pub walk: Option<demo::Walk>,
}

impl Host {
    /// A web view can be shown here: a real window on a system that hosts one.
    pub fn can_show(&self) -> bool {
        self.available && self.failed.is_none()
    }
}

impl crate::State {
    /// From `poll`, every frame: the native views follow `wanted`. True when the view must draw again (a web view
    /// couldn't be made, so the fallback shows).
    pub fn keep_web(&mut self, tx: &mut rune::Tx) -> bool {
        #[cfg(all(feature = "web", not(target_arch = "wasm32")))]
        return platform::keep(self, tx);
        #[cfg(not(all(feature = "web", not(target_arch = "wasm32"))))]
        {
            let _ = tx;
            false
        }
    }
}

impl crate::State {
    /// The theme pages are given: the colours being drawn now and the root font size (`usePageTheme`).
    pub fn page_theme(&self) -> page_theme::PageTheme {
        page_theme::read_page_theme(&crate::theme::Theme::of(self.drawn_dark.get()), self.sidebar.appearance().font_size.px())
    }

    /// A Fleet message from the page of tab `tab` (rows 14.30 to 14.33): what to hand back to that page, if anything. Only
    /// the page tab on screen is heard, so a page that has gone, or another one, changes nothing (`event.source` in Fleet).
    pub fn hear_page(&mut self, tab: &str, data: &serde_json::Value, tx: &mut rune::Tx) -> Option<serde_json::Value> {
        let (shown, page) = wanted::page_tab(self)?;
        if shown != tab {
            return None;
        }
        match page_bridge::read_page_message(data)? {
            page_bridge::PageMessage::Hello => Some(self.web.kept.state_message(&page.page_id).to_json()),
            page_bridge::PageMessage::State { state } => {
                self.web.kept.keep(&page.page_id, state);
                None
            }
            // Nothing is sent: the text goes under what's typed and the user sends it.
            page_bridge::PageMessage::Reply { text } => {
                let session = self.canvas_session();
                self.append_draft_text(&session, &text);
                tx.focus("composer", false);
                None
            }
            page_bridge::PageMessage::Choice { value, summary } => {
                self.page_proposes(tab, value, summary);
                None
            }
            page_bridge::PageMessage::ChoiceRefused { reason } => {
                self.page_refused(tab, reason);
                None
            }
        }
    }
}
