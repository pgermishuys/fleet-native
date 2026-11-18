//! The + menu's Browser entry (component 14, piece J; rows 14.70 to 14.72): `BrowserOpenDialog.vue`. A command to run in the
//! session's folder, or an address on Fleet's machine, in a new browser tab. The dialog is drawn in `view::browser_open`.
use crate::State;
use crate::conv::browser::{BrowserTarget, browser_target};
use crate::fleet::app_runs::SessionApps;
use crate::fleet::canvases::server_tab_id;
use rune::{TextEdit, Tx};

/// The field's key.
pub const FIELD: &str = "browser-open-input";

/// The dialog while it is open.
#[derive(Debug)]
pub struct BrowserOpen {
    pub session: String,
    pub value: TextEdit,
    /// "Used here before": the project's last working command, then the apps' commands newest first, no repeats.
    pub suggestions: Vec<String>,
    /// The user typed or picked: the suggestion that arrives later no longer fills the field.
    pub touched: bool,
    pub busy: bool,
    pub error: Option<String>,
}

impl BrowserOpen {
    pub fn target(&self) -> Option<BrowserTarget> {
        browser_target(&self.value.text)
    }
}

/// The hint under the field.
pub fn hint(target: Option<&BrowserTarget>) -> String {
    match target {
        None => "A command, like npm run dev, or an address on Fleet's machine, like localhost:5173.".into(),
        Some(BrowserTarget::Address(url)) => format!("Shows {url} through Fleet."),
        Some(BrowserTarget::Command(_)) => "Runs it in this session's folder and shows the page it serves.".into(),
    }
}

/// The submit button's word.
pub fn submit_word(target: Option<&BrowserTarget>) -> &'static str {
    if matches!(target, Some(BrowserTarget::Address(_))) { "Open" } else { "Run" }
}

/// The suggestions: the project's command, then the apps' commands newest first; blanks and repeats left out.
pub fn suggestions(known: &SessionApps) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let commands = known.preview_command.iter().cloned().chain(known.apps.iter().rev().map(|a| a.command.clone()));
    for command in commands.filter(|c| !c.is_empty()) {
        if !out.contains(&command) {
            out.push(command);
        }
    }
    out
}

impl State {
    /// The + menu's Browser entry. The form starts over each time.
    pub fn open_browser_dialog(&mut self, tx: &mut Tx) {
        let session = self.canvas_session();
        self.web.open = Some(BrowserOpen {
            session: session.clone(),
            value: TextEdit::new(""),
            suggestions: Vec::new(),
            touched: false,
            busy: false,
            error: None,
        });
        tx.focus(FIELD, false);
        let asked = session.clone();
        self.list_session_apps(&session, move |s, result| {
            let Ok(known) = result else { return };
            // Nothing to offer on a failure; the field still works.
            let Some(d) = s.web.open.as_mut().filter(|d| d.session == asked) else { return };
            d.suggestions = suggestions(&known);
            if let (false, Some(command)) = (d.touched, known.preview_command) {
                d.value = TextEdit::new(command);
            }
        });
    }

    pub fn close_browser_dialog(&mut self) {
        self.web.open = None;
    }

    /// A key in the field: typing is the user's own, and takes an earlier error away.
    pub fn browser_open_typed(&mut self) {
        if let Some(d) = self.web.open.as_mut() {
            d.touched = true;
            d.error = None;
        }
    }

    pub fn browser_open_pick(&mut self, command: &str) {
        if let Some(d) = self.web.open.as_mut() {
            d.touched = true;
            d.value = TextEdit::new(command);
            d.error = None;
        }
    }

    /// Run or Open (Enter does the same): nothing without a target or while busy. Fleet answers with the new tab's canvas; the
    /// canvases are loaded again, that tab comes forward and the dialog closes. A refusal stays in the dialog.
    pub fn browser_open_submit(&mut self) {
        let Some(d) = self.web.open.as_ref() else { return };
        let Some(target) = d.target() else { return };
        if d.busy {
            return;
        }
        let session = d.session.clone();
        let connected = self.link_for(&session).is_some();
        let Some(d) = self.web.open.as_mut() else { return };
        if !connected {
            d.error = Some("Fleet isn't connected.".into());
            return;
        }
        d.busy = true;
        d.error = None;
        let asked = session.clone();
        let done = move |s: &mut State, result: Result<String, String>| match result {
            Ok(canvas) => {
                if s.web.open.as_ref().is_some_and(|d| d.session == asked) {
                    s.close_browser_dialog();
                }
                s.load_server_canvases_then(&asked, Some(server_tab_id(&canvas)));
            }
            Err(why) => {
                if let Some(d) = s.web.open.as_mut().filter(|d| d.session == asked) {
                    d.busy = false;
                    d.error = Some(why);
                }
            }
        };
        match target {
            BrowserTarget::Command(command) => self.start_command(&session, &command, done),
            BrowserTarget::Address(url) => self.open_address(&session, &url, done),
        }
    }
}
