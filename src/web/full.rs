//! A conversation page open full size (component 14, piece F; row 14.94): Fleet's `.conversation-page__overlay`. The state of the
//! dialog (`view::page_card::dialog` draws it) and the rule that its web view is the one on screen (`wanted`).
use super::Placement;
use super::page_theme::PagePlace;
use super::wanted::page_base;
use crate::State;
use rune::Tx;

/// The page that is open full size. Fleet's dialog is part of the page's component, so it goes with the conversation: another
/// session shows no dialog.
#[derive(Clone, Debug, PartialEq)]
pub struct Full {
    /// The call that showed the page; names the web view and what loads it.
    pub call: String,
    pub session: String,
    pub title: String,
    /// Fleet's copy of the page on the server (`ToolCardPage.path`).
    pub path: String,
    /// The element that had the keyboard when it opened, which gets it back (`returnFocusTo`).
    pub return_to: Option<String>,
}

pub const CLOSE_KEY: &str = "page-full-close";

impl State {
    /// "Open full size": the dialog opens with focus on Close.
    pub fn open_page_full(&mut self, call: &str, title: &str, path: &str, tx: &mut Tx) {
        let return_to = tx.focused().map(str::to_string);
        self.web.full = Some(Full { call: call.into(), session: self.canvas_session(), title: title.into(), path: path.into(), return_to });
        self.web.full_rect = None;
        tx.focus(CLOSE_KEY, false);
    }

    /// Escape, the scrim and Close: it goes, and the keyboard goes back where it was.
    pub fn close_page_full(&mut self, tx: &mut Tx) {
        let Some(full) = self.web.full.take() else { return };
        self.web.full_rect = None;
        match full.return_to {
            Some(key) => tx.focus(key, false),
            None => tx.blur(),
        }
    }

    /// The dialog that is up, for the session on screen.
    pub fn page_full(&self) -> Option<&Full> {
        self.web.full.as_ref().filter(|f| f.session == self.canvas_session())
    }

    /// The page of the dialog on Fleet's server (the session's machine), or the invented one with no server.
    pub fn page_full_url(&self, full: &Full) -> Option<String> {
        Some(format!("{}{}", page_base(self)?.trim_end_matches('/'), full.path))
    }

    /// The address "Open in a new tab" opens for a call's page.
    pub fn page_url_of(&self, path: &str) -> Option<String> {
        Some(format!("{}{path}", page_base(self)?.trim_end_matches('/')))
    }
}

/// The dialog's web view, which takes every other one's place while the dialog is up: it is under the scrim for nothing else.
pub fn wanted_full(s: &State) -> Option<Placement> {
    let full = s.page_full()?;
    let url = s.page_full_url(full)?;
    let rect = s.web.full_rect.filter(|r| r[2] >= 1.0 && r[3] >= 1.0)?;
    Some(Placement {
        tab: format!("full:{}", full.call),
        url,
        load_key: full.call.clone(),
        rect,
        html: None,
        place: PagePlace::Conversation,
        browser: false,
    })
}
