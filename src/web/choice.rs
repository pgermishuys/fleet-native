//! The page tab's pick (component 14, piece C; rows 14.36 to 14.38): what the page proposed, why Fleet refused what it sent,
//! and the send or skip that answers the agent. Plain Rust on `State`, so it is tested with no window. Ports the script of
//! `components/canvas/PageCanvas.vue` (`proposal`, `refusal`, `sending`, `sendError`, `answerChoice`).
use crate::State;
use crate::fleet::http;
use crate::fleet::pages::{ChoiceStatus, PageChoice, is_choice_live};
use serde_json::{Value, json};

/// What the bar holds for one page's wait. A new `callId` starts clean; a change of the canvas's status ends sending.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChoiceUi {
    pub call_id: String,
    /// What the page proposed: the value to send and the words to show.
    pub proposal: Option<(String, String)>,
    /// Why Fleet refused what the page last sent as its pick (too long, empty).
    pub refusal: String,
    /// The status the canvas had when the request went out; it is over when the canvas says anything else.
    pub sending: Option<ChoiceStatus>,
    pub error: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer {
    Send,
    Skip,
}

pub const SEND_FAILED: &str = "Couldn't send that. Try again.";

/// `POST /api/sessions/{id}/page-choices/{callId}/{send|skip}`.
pub fn answer_path(session: &str, call_id: &str, answer: Answer) -> String {
    let action = if answer == Answer::Send { "send" } else { "skip" };
    format!("/api/sessions/{}/page-choices/{}/{action}", http::escape(session), http::escape(call_id))
}

/// Fleet's reason for a refused request (`body.error`), else the sentence the bar always has.
pub fn failure_words(body: &str) -> String {
    serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|b| b.get("error").and_then(Value::as_str).map(str::to_string))
        .unwrap_or_else(|| SEND_FAILED.to_string())
}

impl State {
    /// The shown page's choice, when the tab is the active page tab.
    fn tab_choice(&self, tab: &str) -> Option<PageChoice> {
        let (shown, page) = super::wanted::page_tab(self)?;
        if shown != tab {
            return None;
        }
        page.choice
    }

    /// A page's wait is live while its session is in a turn (`isChoiceLive` with the open session's `activityStatus`).
    pub fn choice_is_live(&self, choice: &PageChoice) -> bool {
        is_choice_live(Some(choice), Some(self.open_activity()))
    }

    /// What the bar holds for `tab`'s wait `choice`: nothing from an earlier wait.
    pub fn choice_ui(&self, tab: &str, choice: &PageChoice) -> ChoiceUi {
        match self.web.choices.get(tab) {
            Some(ui) if ui.call_id == choice.call_id => ui.clone(),
            _ => ChoiceUi { call_id: choice.call_id.clone(), ..Default::default() },
        }
    }

    /// `sending`, as the bar reads it: still on while the canvas says what it said when the request went out.
    pub fn choice_sending(&self, tab: &str, choice: &PageChoice) -> bool {
        self.choice_ui(tab, choice).sending == Some(choice.status)
    }

    fn choice_ui_mut(&mut self, tab: &str, choice: &PageChoice) -> &mut ChoiceUi {
        let fresh = self.choice_ui(tab, choice);
        let slot = self.web.choices.entry(tab.to_string()).or_default();
        *slot = fresh;
        slot
    }

    /// `fleet:page-choice`: taken only while the agent waits, and only the first (Change clears it, so a page can't swap it
    /// under the user).
    pub(crate) fn page_proposes(&mut self, tab: &str, value: String, summary: String) {
        let Some(choice) = self.tab_choice(tab).filter(|c| self.choice_is_live(c)) else { return };
        let ui = self.choice_ui_mut(tab, &choice);
        if ui.proposal.is_none() {
            ui.proposal = Some((value, summary));
            ui.refusal.clear();
        }
    }

    /// `fleet:page-choice-refused`: likewise only while live and none is held.
    pub(crate) fn page_refused(&mut self, tab: &str, reason: String) {
        let Some(choice) = self.tab_choice(tab).filter(|c| self.choice_is_live(c)) else { return };
        let ui = self.choice_ui_mut(tab, &choice);
        if ui.proposal.is_none() {
            ui.refusal = reason;
        }
    }

    /// Change: the proposal goes, so the page can propose again.
    pub fn change_choice(&mut self, tab: &str) {
        if let Some(choice) = self.tab_choice(tab) {
            self.choice_ui_mut(tab, &choice).proposal = None;
        }
    }

    /// Send to agent or Skip. The request goes on a thread; a failure puts Fleet's words in the bar and lets the user try
    /// again. A success needs nothing here: the canvas's new status (an event) changes the bar.
    pub fn answer_choice(&mut self, tab: &str, answer: Answer) {
        let Some(choice) = self.tab_choice(tab) else { return };
        let ui = self.choice_ui(tab, &choice);
        if ui.sending == Some(choice.status) {
            return;
        }
        if answer == Answer::Send && ui.proposal.is_none() {
            return;
        }
        let session = self.canvas_session();
        let Some(link) = self.link_for(&session) else { return };
        let body = match (answer, &ui.proposal) {
            (Answer::Send, Some((value, summary))) => json!({ "value": value, "summary": summary }),
            _ => json!({}),
        };
        let slot = self.choice_ui_mut(tab, &choice);
        slot.sending = Some(choice.status);
        slot.error.clear();
        let (path, tab, call_id) = (answer_path(&session, &choice.call_id, answer), tab.to_string(), choice.call_id.clone());
        link.off_thread(
            move |base| match http::text_with("POST", base, &path, Some(&body)) {
                Ok((status, _)) if (200..300).contains(&status) => None,
                Ok((_, text)) => Some(failure_words(&text)),
                Err(_) => Some(SEND_FAILED.to_string()),
            },
            move |s, _, failed| {
                let Some(words) = failed else { return };
                let Some(ui) = s.web.choices.get_mut(&tab).filter(|ui| ui.call_id == call_id) else { return };
                ui.sending = None;
                ui.error = words;
            },
        );
    }

    /// `openOutside`: the address of the page in `tab`, for the system's browser.
    pub fn page_outside_url(&self, tab: &str) -> Option<String> {
        let (shown, page) = super::wanted::page_tab(self)?;
        if shown != tab {
            return None;
        }
        crate::fleet::web_page::page_url(&super::wanted::page_base(self)?, &page)
    }

    /// Reload: the page loads again (`reloads++` in the frame's key).
    pub fn reload_page(&mut self, tab: &str) {
        *self.web.reloads.entry(tab.to_string()).or_default() += 1;
    }
}
