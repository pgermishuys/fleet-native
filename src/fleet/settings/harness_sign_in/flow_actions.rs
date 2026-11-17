//! What the sign-in flow asks of a machine (`HarnessSignInFlow.vue`): sign in with a key, start a sign-in in the browser and follow it
//! every 2 s until it ends, finish it with a code or a pasted address, cancel it. Every request goes on a thread and its answer comes
//! back to the UI thread; nothing here waits.
use super::flow::{Attempt, Flow, Method, answers_to_send, code_to_enter, is_remote, methods_to_offer, missing_answer};
use super::{Provider, error_words};
use crate::fleet::http;
use crate::{State, Tx};
use serde_json::{Value, json};
use std::time::Duration;

/// How often a browser sign-in's status is looked at (`POLL_MS`).
const POLL_SECS: f64 = 2.0;
/// How long "Copied" stays on the Copy code button.
const COPIED_SECS: f64 = 1.5;

fn ok(status: u16) -> bool {
    (200..300).contains(&status)
}

impl State {
    /// The flow of `harness`'s panel, while a provider's Sign in is open.
    pub fn sign_in_flow(&self, harness: &str) -> Option<&Flow> {
        self.sign_in_panel(harness)?.flow.as_ref()
    }

    fn flow_mut(&mut self) -> Option<&mut Flow> {
        self.settings.harnesses.sign_in.flow.as_mut()
    }

    /// The box a typed field's `on_edit` reaches; a spare one if the flow went while a frame was drawn.
    pub fn flow_edit_mut(&mut self, index: usize) -> &mut rune::TextEdit {
        let panel = &mut self.settings.harnesses.sign_in;
        match panel.flow.as_mut().and_then(|f| f.edits.get_mut(index)) {
            Some((_, edit)) => edit,
            None => &mut panel.spare,
        }
    }

    pub fn flow_key_mut(&mut self) -> &mut rune::TextEdit {
        let panel = &mut self.settings.harnesses.sign_in;
        match panel.flow.as_mut() {
            Some(f) => &mut f.key,
            None => &mut panel.spare,
        }
    }

    pub fn flow_code_mut(&mut self) -> &mut rune::TextEdit {
        let panel = &mut self.settings.harnesses.sign_in;
        match panel.flow.as_mut() {
            Some(f) => &mut f.code,
            None => &mut panel.spare,
        }
    }

    pub fn flow_landed_mut(&mut self) -> &mut rune::TextEdit {
        let panel = &mut self.settings.harnesses.sign_in;
        match panel.flow.as_mut() {
            Some(f) => &mut f.landed,
            None => &mut panel.spare,
        }
    }

    /// The command that signs in from a terminal (`signInCommand`), for the command method's advice.
    pub fn sign_in_command(&self, harness: &str) -> Option<String> {
        self.harness_list().all.iter().find(|h| h.kind == harness)?.setup.sign_in_command.clone()
    }

    /// The method the flow shows, with its provider.
    pub fn flow_method(&self) -> Option<Method> {
        let flow = self.settings.harnesses.sign_in.flow.as_ref()?;
        let provider = self.sign_ins(&flow.harness)?.providers.iter().find(|p| p.id == flow.provider)?;
        methods_to_offer(provider).into_iter().find(|m| m.key() == flow.method)
    }

    /// A provider's Sign in opened: its flow, on the provider's first method.
    pub(super) fn open_flow(&mut self, harness: &str, provider: &Provider) {
        let name = self.harness_list().all.iter().find(|h| h.kind == harness).map(|h| h.display_name.clone()).unwrap_or_default();
        let panel = &mut self.settings.harnesses.sign_in;
        let mut flow = Flow::new(harness, &name, provider);
        flow.remote = is_remote(&panel.key.0);
        panel.flow = Some(flow);
    }

    /// The flow ends (closed, another provider opened, the page left): a sign-in in the browser is cancelled and the key is wiped.
    /// Errors from the cancel are ignored.
    pub fn end_flow(&mut self) {
        let panel = &mut self.settings.harnesses.sign_in;
        let Some(flow) = panel.flow.take() else { return };
        let Some(attempt) = flow.attempt.clone() else { return };
        let machine = panel.key.0.clone();
        self.feed_call(
            &machine,
            move |base| http::sign_in_attempt("DELETE", base, &flow.harness, &flow.provider, &attempt.id, "", None),
            |_, _, _| {},
        );
    }

    /// The Close or Cancel of a flow: it ends and the provider's article closes.
    pub fn close_sign_in_flow(&mut self) {
        self.settings.harnesses.sign_in.open = None;
        self.end_flow();
    }

    /// A method's pill pressed.
    pub fn flow_pick_method(&mut self, key: &str) {
        let Some(method) = self.flow_method_by_key(key) else { return };
        if let Some(flow) = self.flow_mut() {
            flow.pick(&method);
        }
    }

    fn flow_method_by_key(&self, key: &str) -> Option<Method> {
        let flow = self.settings.harnesses.sign_in.flow.as_ref()?;
        let provider = self.sign_ins(&flow.harness)?.providers.iter().find(|p| p.id == flow.provider)?;
        methods_to_offer(provider).into_iter().find(|m| m.key() == key)
    }

    /// A choice picked in a field's list, or a tick set.
    pub fn flow_set_answer(&mut self, field: &str, value: Value) {
        if let Some(flow) = self.flow_mut() {
            flow.answers.insert(field.to_string(), value);
            flow.choosing = None;
        }
    }

    pub fn flow_toggle_choice(&mut self, field: &str, value: &str, on: bool) {
        if let Some(flow) = self.flow_mut() {
            flow.toggle_choice(field, value, on);
        }
    }

    /// `checkAnswers`: the first needed field without an answer is named, or the error clears.
    fn check_answers(&mut self, method: &Method) -> bool {
        let Some(flow) = self.flow_mut() else { return false };
        let missing = missing_answer(&method.fields, &flow.current());
        flow.error = missing.as_ref().map(|name| format!("Fill in \u{201c}{name}\u{201d}."));
        missing.is_none()
    }

    /// The form's button: sign in with the key, or start the sign-in in the browser.
    pub fn flow_submit(&mut self, tx: &mut Tx) {
        let Some(method) = self.flow_method() else { return };
        match method.kind.as_str() {
            "key" => self.sign_in_with_key(&method),
            "oauth" => self.start_browser_sign_in(&method, tx),
            _ => {}
        }
    }

    fn sign_in_with_key(&mut self, method: &Method) {
        if self.flow_mut().is_none_or(|f| f.busy) || !self.check_answers(method) {
            return;
        }
        let machine = self.settings.harnesses.sign_in.key.0.clone();
        let panel_key = self.settings.harnesses.sign_in.key.clone();
        let Some(flow) = self.flow_mut() else { return };
        if flow.key.text.trim().is_empty() {
            flow.error = Some("Enter the key.".into());
            return;
        }
        flow.busy = true;
        let (harness, name, provider, provider_name) =
            (flow.harness.clone(), flow.harness_name.clone(), flow.provider.clone(), flow.provider_name.clone());
        // The key is in this one request and nowhere else once it is sent.
        let key = flow.key.text.clone();
        let answers = Value::Object(answers_to_send(&method.fields, &flow.current()));
        let (kind, id) = (harness.clone(), provider.clone());
        let sent = self.feed_call(
            &machine,
            move |base| http::sign_in_key(base, &kind, &id, &key, &answers),
            move |s, _, answer| {
                if s.settings.harnesses.sign_in.key != panel_key {
                    return;
                }
                let here = s.flow_mut().filter(|f| f.provider == provider);
                match answer {
                    Ok((status, _)) if ok(status) => s.sign_in_done(&harness, &name, &format!("Signed in to {provider_name}.")),
                    Ok((status, body)) => {
                        if let Some(f) = here {
                            f.busy = false;
                            f.error = Some(error_words(status, &body));
                        }
                    }
                    Err(_) => {
                        if let Some(f) = here {
                            f.busy = false;
                            f.error = Some("Couldn't sign in.".into());
                        }
                    }
                }
            },
        );
        if !sent && let Some(f) = self.flow_mut() {
            f.busy = false;
            f.error = Some("Couldn't sign in.".into());
        }
    }

    fn start_browser_sign_in(&mut self, method: &Method, tx: &mut Tx) {
        let _ = tx;
        let Some(method_id) = method.id.clone() else { return };
        if self.flow_mut().is_none_or(|f| f.busy) || !self.check_answers(method) {
            return;
        }
        let panel_key = self.settings.harnesses.sign_in.key.clone();
        let machine = panel_key.0.clone();
        let Some(flow) = self.flow_mut() else { return };
        flow.busy = true;
        let answers = Value::Object(answers_to_send(&method.fields, &flow.current()));
        let (kind, id) = (flow.harness.clone(), flow.provider.clone());
        let sent = self.feed_call(
            &machine,
            move |base| http::sign_in_start(base, &kind, &id, &method_id, &answers),
            move |s, tx, answer| {
                let clock = s.clock;
                if s.settings.harnesses.sign_in.key != panel_key {
                    return;
                }
                let Some(f) = s.flow_mut() else { return };
                f.busy = false;
                match answer {
                    Ok((status, body)) if ok(status) => match Attempt::parse(&body) {
                        Some(attempt) => {
                            f.paste_open = f.remote;
                            // The desktop app opens the page in the system browser; the link under it is there if that fails.
                            let _ = tx.open_url(&attempt.url);
                            f.attempt = Some(attempt);
                            f.poll_due = Some(clock + POLL_SECS);
                            tx.wake_after(Duration::from_secs_f64(POLL_SECS));
                        }
                        None => f.error = Some("Couldn't start the sign-in.".into()),
                    },
                    Ok((status, body)) => f.error = Some(error_words(status, &body)),
                    Err(_) => f.error = Some("Couldn't start the sign-in.".into()),
                }
            },
        );
        if !sent && let Some(f) = self.flow_mut() {
            f.busy = false;
            f.error = Some("Couldn't start the sign-in.".into());
        }
    }

    /// `checkAttempt`: looks at the attempt once. Pending, or a look that missed, tries again in 2 s; any other answer ends it.
    pub fn check_attempt(&mut self) {
        let panel_key = self.settings.harnesses.sign_in.key.clone();
        let machine = panel_key.0.clone();
        let Some(flow) = self.flow_mut() else { return };
        let Some(attempt) = flow.attempt.clone() else { return };
        flow.checking = true;
        flow.poll_due = None;
        let (harness, name, provider, provider_name) =
            (flow.harness.clone(), flow.harness_name.clone(), flow.provider.clone(), flow.provider_name.clone());
        let (kind, id, attempt_id) = (harness.clone(), provider.clone(), attempt.id.clone());
        let sent = self.feed_call(
            &machine,
            move |base| http::sign_in_attempt("GET", base, &kind, &id, &attempt_id, "", None),
            move |s, tx, answer| {
                let clock = s.clock;
                if s.settings.harnesses.sign_in.key != panel_key {
                    return;
                }
                let Some(f) = s.flow_mut().filter(|f| f.attempt.as_ref().is_some_and(|a| a.id == attempt.id)) else { return };
                f.checking = false;
                let status = match answer {
                    Ok((code, body)) if ok(code) => serde_json::from_str::<Value>(&body).ok(),
                    _ => None,
                };
                let Some(status) = status else {
                    // A missed look isn't the end of the sign-in.
                    f.poll_due = Some(clock + POLL_SECS);
                    tx.wake_after(Duration::from_secs_f64(POLL_SECS));
                    return;
                };
                match status.get("status").and_then(Value::as_str) {
                    Some("pending") => {
                        f.poll_due = Some(clock + POLL_SECS);
                        tx.wake_after(Duration::from_secs_f64(POLL_SECS));
                    }
                    Some("complete") => {
                        f.attempt = None;
                        s.sign_in_done(&harness, &name, &format!("Signed in to {provider_name}."));
                    }
                    Some("failed") => {
                        f.attempt = None;
                        let why =
                            status.get("message").and_then(Value::as_str).filter(|m| !m.is_empty()).unwrap_or("the provider refused it");
                        f.error = Some(format!("The sign-in didn't work: {why}."));
                    }
                    _ => {
                        f.attempt = None;
                        f.error = Some("The sign-in ran out of time. Start it again.".into());
                    }
                }
                let _ = &provider;
            },
        );
        let clock = self.clock;
        if !sent && let Some(f) = self.flow_mut() {
            f.checking = false;
            f.poll_due = Some(clock + POLL_SECS);
        }
    }

    /// The Finish of a code box: sends the code, then looks at the attempt.
    pub fn flow_submit_code(&mut self) {
        let panel_key = self.settings.harnesses.sign_in.key.clone();
        let machine = panel_key.0.clone();
        let Some(flow) = self.flow_mut() else { return };
        let Some(attempt) = flow.attempt.clone() else { return };
        if flow.busy {
            return;
        }
        if flow.code.text.trim().is_empty() {
            flow.error = Some("Paste the code the provider showed you.".into());
            return;
        }
        flow.busy = true;
        flow.error = None;
        let body = json!({ "code": flow.code.text.clone() });
        let (kind, id) = (flow.harness.clone(), flow.provider.clone());
        self.send_attempt_step("/code", body, kind, id, attempt.id, panel_key, machine, |f| f.code.replace_text(String::new()));
    }

    /// The Finish sign-in of the paste box: the address the browser landed on goes to the listener on Fleet's computer.
    pub fn flow_submit_landed(&mut self) {
        let panel_key = self.settings.harnesses.sign_in.key.clone();
        let machine = panel_key.0.clone();
        let Some(flow) = self.flow_mut() else { return };
        let Some(attempt) = flow.attempt.clone() else { return };
        if flow.busy || flow.landed.text.trim().is_empty() {
            return;
        }
        flow.busy = true;
        flow.error = None;
        let body = json!({ "address": flow.landed.text.clone() });
        let (kind, id) = (flow.harness.clone(), flow.provider.clone());
        self.send_attempt_step("/callback", body, kind, id, attempt.id, panel_key, machine, |f| f.landed.replace_text(String::new()));
    }

    #[allow(clippy::too_many_arguments)]
    fn send_attempt_step(
        &mut self,
        step: &'static str,
        body: Value,
        kind: String,
        provider: String,
        attempt: String,
        panel_key: (String, String),
        machine: String,
        wipe: fn(&mut Flow),
    ) {
        let sent = self.feed_call(
            &machine,
            move |base| http::sign_in_attempt("POST", base, &kind, &provider, &attempt, step, Some(&body)),
            move |s, _, answer| {
                if s.settings.harnesses.sign_in.key != panel_key {
                    return;
                }
                let Some(f) = s.flow_mut() else { return };
                f.busy = false;
                match answer {
                    Ok((status, _)) if ok(status) => {
                        wipe(f);
                        s.check_attempt();
                    }
                    Ok((status, body)) => f.error = Some(error_words(status, &body)),
                    Err(_) => f.error = Some("Couldn't finish the sign-in.".into()),
                }
            },
        );
        if !sent && let Some(f) = self.flow_mut() {
            f.busy = false;
            f.error = Some("Couldn't finish the sign-in.".into());
        }
    }

    /// The Cancel of a running attempt: it is cancelled and the flow closes.
    pub fn flow_cancel(&mut self) {
        self.close_sign_in_flow();
    }

    /// `copyCode`: the code of a device sign-in on the clipboard, and "Copied" for 1.5 s.
    pub fn flow_copy_code(&mut self, tx: &mut Tx) {
        let clock = self.clock;
        let Some(flow) = self.flow_mut() else { return };
        let Some(code) = flow.attempt.as_ref().and_then(|a| code_to_enter(&a.instructions)) else { return };
        tx.copy(code);
        flow.copied_until = Some(clock + COPIED_SECS);
        tx.wake_after(Duration::from_millis(100));
    }

    /// The flow's timers, from `poll`: "Copied" ends, and a running attempt is looked at every 2 s. True when something drawn changed.
    pub fn keep_sign_in_flow(&mut self, now: f64, tx: &mut Tx) -> bool {
        let Some(flow) = self.flow_mut() else { return false };
        let mut changed = false;
        match flow.copied_until {
            Some(until) if now >= until => {
                flow.copied_until = None;
                changed = true;
            }
            Some(_) => tx.wake_after(Duration::from_millis(100)),
            None => {}
        }
        if flow.attempt.is_some() && !flow.checking {
            match flow.poll_due {
                Some(at) if now >= at => self.check_attempt(),
                Some(at) => tx.wake_after(Duration::from_secs_f64((at - now).max(0.01))),
                None => {}
            }
        }
        changed
    }
}
