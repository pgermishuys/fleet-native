//! What the buttons on a background row do (`BackgroundWorkRow.vue`, `WorkOutput.vue`, `use-running-work.ts`): Stop,
//! Output, Open, Details and Events. The requests run on a thread (`Link::off_thread`); every answer comes back through
//! a method here, which tests call directly.
use super::http;
use super::work::{RunningWorkItem, apply_item, is_running, to_item};
use crate::State;
use crate::conv::work_output::{Next, Page, REFRESH_MS, Reader};
use crate::view::message::now_ms;
use rune::Tx;
use serde_json::Value;
use std::time::Duration;

/// `errorMessage`: the reason in an error body, Fleet's `{error}` or a problem's `detail` or `title`.
pub fn error_message(body: &Value, fallback: String) -> String {
    ["error", "detail", "title"]
        .iter()
        .find_map(|k| body.get(k).and_then(Value::as_str).filter(|s| !s.trim().is_empty()))
        .map_or(fallback, str::to_string)
}

fn parse_page(body: &Value) -> Option<Page> {
    Some(Page {
        output: body.get("output").and_then(Value::as_str).unwrap_or("").to_string(),
        next_offset: body.get("nextOffset").and_then(Value::as_u64)?,
        size: body.get("size").and_then(Value::as_u64)?,
        truncated: body.get("truncated") == Some(&Value::Bool(true)),
    })
}

impl State {
    fn work_item(&self, id: &str) -> Option<&RunningWorkItem> {
        self.running_work().iter().find(|i| i.id == id)
    }

    /// Stop (`stopWork`): a second press while one is out is refused. The answer is the item ended; it is applied at
    /// once, before its event.
    pub fn stop_work(&mut self, id: &str) {
        self.work.stop_errors.remove(id);
        let Some(item) = self.work_item(id) else { return };
        if self.work.stopping.contains(id) {
            self.work.stop_errors.insert(id.to_string(), "Already stopping.".into());
            return;
        }
        let Some(link) = &self.link else { return };
        let (session, item_id) = (item.session_id.clone(), id.to_string());
        self.work.stopping.insert(item_id.clone());
        let asked = item_id.clone();
        link.off_thread(move |base| http::work_stop(base, &session, &asked), move |s, _, result| s.on_work_stopped(&item_id, result));
    }

    pub fn on_work_stopped(&mut self, id: &str, result: Result<(u16, Value), String>) {
        self.work.stopping.remove(id);
        match result {
            Err(why) => {
                self.work.stop_errors.insert(id.to_string(), why);
            }
            Ok((status, body)) if !(200..300).contains(&status) => {
                self.work.stop_errors.insert(id.to_string(), error_message(&body, format!("It couldn't be stopped (HTTP {status}).")));
            }
            Ok((_, body)) => {
                if let (Some(item), Some(stream)) = (to_item(&body), self.stream.as_mut()) {
                    apply_item(&mut stream.running_work, item);
                }
            }
        }
    }

    /// Output toggles the panel under the row; closing it ends its reading (the answer to a request still out is dropped).
    pub fn toggle_work_output(&mut self, id: &str) {
        if self.work.outputs.remove(id).is_some() {
            return;
        }
        let Some(item) = self.work_item(id) else { return };
        let running = is_running(item);
        self.work.next_epoch += 1;
        self.work.outputs.insert(id.to_string(), Reader::new(self.work.next_epoch, running));
        self.read_work_output(id);
    }

    /// Details toggles its panel; it needs nothing from the server.
    pub fn toggle_work_details(&mut self, id: &str) {
        if !self.work.details.remove(id) {
            self.work.details.insert(id.to_string());
        }
    }

    /// Starts a read of an open output, from where the last one stopped.
    fn read_work_output(&mut self, id: &str) {
        if self.link.is_none() {
            return;
        }
        let Some(reader) = self.work.outputs.get_mut(id) else { return };
        let offset = reader.begin();
        self.request_work_page(id, offset);
    }

    fn request_work_page(&self, id: &str, offset: u64) {
        let (Some(link), Some(item), Some(reader)) = (&self.link, self.work_item(id), self.work.outputs.get(id)) else { return };
        let (session, item_id, done_id, epoch) = (item.session_id.clone(), id.to_string(), id.to_string(), reader.epoch);
        link.off_thread(
            move |base| http::work_output(base, &session, &item_id, offset),
            move |s, tx, result| s.on_work_page(&done_id, epoch, result, tx),
        );
    }

    /// One answer of the output (`read`). An answer for a panel closed since is dropped.
    pub fn on_work_page(&mut self, id: &str, epoch: u64, result: Result<(u16, Value), String>, tx: &mut Tx) {
        let page = match result {
            Err(why) => Err(why),
            Ok((status, body)) => match parse_page(&body).filter(|_| (200..300).contains(&status)) {
                Some(page) => Ok(page),
                None => Err(error_message(&body, format!("The output couldn't be read (HTTP {status})."))),
            },
        };
        let running = self.work_item(id).is_some_and(is_running);
        let now = now_ms(self);
        let Some(reader) = self.work.outputs.get_mut(id).filter(|r| r.epoch == epoch) else { return };
        match reader.page(page) {
            Next::Fetch(offset) => self.request_work_page(id, offset),
            Next::Done => {
                reader.was_running |= running;
                if running {
                    reader.next_at = Some(now + REFRESH_MS);
                    tx.wake_after(Duration::from_millis(REFRESH_MS as u64));
                }
            }
        }
    }

    /// The refresh of every open output, from `poll`: every 2 s while its work runs, and one last read when it ends.
    /// An output whose item left the list is closed.
    pub fn poll_work_outputs(&mut self, tx: &mut Tx) {
        let now = now_ms(self);
        let ids: Vec<String> = self.work.outputs.keys().cloned().collect();
        for id in ids {
            let running = self.work_item(&id).map(is_running);
            let Some(reader) = self.work.outputs.get_mut(&id) else { continue };
            let Some(running) = running else {
                self.work.outputs.remove(&id);
                continue;
            };
            if reader.in_flight {
                continue;
            }
            let due = if running {
                reader.was_running = true;
                reader.next_at.is_some_and(|at| at <= now)
            } else {
                std::mem::take(&mut reader.was_running)
            };
            if due {
                self.read_work_output(&id);
            } else if let Some(at) = self.work.outputs.get(&id).and_then(|r| r.next_at) {
                tx.wake_after(Duration::from_millis((at - now).max(50) as u64));
            }
        }
    }

    /// Open (`openChild`): the child's session, with this one remembered as its parent. Children aren't in the session
    /// list yet (component 6), so one that isn't listed can't be shown; returns whether it was.
    pub fn open_child(&mut self, parent: &str, child: &str, tx: &mut Tx) -> bool {
        let found = self.projects.iter().enumerate().find_map(|(p, pr)| pr.sessions.iter().position(|s| s.id == child).map(|i| (p, i)));
        let Some((p, i)) = found else { return false };
        self.open_session(p, i, tx);
        self.work.opened_from = Some(parent.to_string());
        true
    }

    /// Events: scroll the conversation to the call that set a monitor going.
    pub fn show_work_events(&mut self, call: &str, tx: &mut Tx) {
        self.show_message(None, Some(call), tx);
    }
}
