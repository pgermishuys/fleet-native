//! The side conversation's own live stream (`/btw`): its session's snapshot and events, kept apart from the open session's, drawn
//! in the panel. Ports what `SideConversationPanel.vue` leaves to `ActivityStream.vue` mounted on the side session: the messages
//! after the boundary (`messagesAfter`), the question shown at once, the fold plan, and the progress report (`progress`) that
//! says whether it is working and what its newest answer is, which the tab reports (piece D).
//! The panel watches the side session's topic while its parent is open and stops when the parent goes or the side conversation
//! does; the link asks for the snapshot again after a reconnect (`Link::watch_side`).
use super::events::Event;
use super::model::{Msg, StreamState};
use super::prompts::{Confirm, PromptStatus};
use super::reducer;
use super::side::{Progress, messages_after};
use super::state::Topic;
use super::{sessions, view};
use crate::State;
use crate::conv::background::Finished;
use crate::conv::compaction::fold_compaction_summaries;
use crate::conv::fold::FoldOptions;
use crate::conv::fold_plan::FoldPlan;
use crate::conv::message_list::{ListContext, render_message_body};
use crate::mock::Session;
use crate::view::conversation::{Message, Role};
use rune::Tx;
use std::collections::HashMap;

/// One side session's stream.
#[derive(Default)]
pub struct SideStream {
    /// The snapshot, once it has arrived and with its events applied. Nothing is reported before it: an empty stream that
    /// hasn't loaded isn't "no answer".
    state: Option<StreamState>,
    /// Events that came before the snapshot; they follow it.
    pending: Vec<Event>,
    /// What it carried after the boundary, as the reducer holds it (what the working line reads).
    shown: Vec<Msg>,
    /// A turn was seen running here: the newest turn stays unfolded after it ends.
    watched: bool,
    /// The last progress told, so the same one isn't told twice.
    told: Option<(bool, Option<String>, Option<String>)>,
}

/// The panel's side of the link: which side session is listened to, and every one's stream.
#[derive(Default)]
pub struct SideStreams {
    streams: HashMap<String, SideStream>,
    /// The parent and the side session the link listens to now.
    watching: Option<(String, String)>,
}

/// The newest assistant message with something to say: its id and its trimmed words (`latestAnswerMessage`).
fn latest_answer(shown: &[Msg]) -> Option<(String, String)> {
    shown.iter().rev().filter(|m| m.role == Role::Assistant).find_map(|m| {
        let body = render_message_body(&m.parts);
        let body = body.trim();
        (!body.is_empty()).then(|| (m.id.clone(), body.to_string()))
    })
}

impl SideStreams {
    /// The parent and the side session the link listens to now.
    pub fn watching(&self) -> Option<&(String, String)> {
        self.watching.as_ref()
    }
}

impl State {
    /// The side session the panel is on: the open session's side conversation, if it has one.
    pub fn side_session_id(&self) -> Option<String> {
        self.side()?.side.as_ref().map(|s| s.session_id.clone())
    }

    /// The side conversation's messages after the boundary (what its working line reads).
    pub(crate) fn side_stream_messages(&self) -> &[Msg] {
        let id = self.side_session_id();
        id.and_then(|id| self.side_ui.live.streams.get(&id)).map_or(&[], |s| s.shown.as_slice())
    }

    /// Keeps the link listening to the right side session (each frame): the open session's, while it has one. Leaving the session
    /// or ending the side conversation stops it and forgets what it held. True when something changed.
    pub fn keep_side_stream(&mut self) -> bool {
        let want = self.draft_owner().zip(self.side_session_id()).filter(|(owner, _)| self.link_for(owner).is_some());
        if self.side_ui.live.watching == want {
            return false;
        }
        if let Some((owner, id)) = self.side_ui.live.watching.take() {
            if let Some(link) = self.link_for(&owner) {
                link.unwatch_side(&id);
            }
            self.side_ui.live.streams.remove(&id);
            self.side_ui.conv = None;
            self.side_ui.plan = FoldPlan::default();
        }
        if let Some((owner, id)) = &want {
            if let Some(link) = self.link_for(owner) {
                link.watch_side(id);
            }
            // The side session's own permission asks wait here too.
            self.load_asks(id);
            self.side_ui.live.streams.entry(id.clone()).or_default();
            self.sync_side(id);
        }
        self.side_ui.live.watching = want;
        true
    }

    /// The side session's snapshot arrived, or arrived again after a reconnect.
    pub fn on_side_snapshot(&mut self, id: &str, mut state: StreamState, _tx: &mut Tx) {
        if self.side_session_id().as_deref() != Some(id) {
            return;
        }
        let stream = self.side_ui.live.streams.entry(id.to_string()).or_default();
        for event in std::mem::take(&mut stream.pending) {
            reducer::apply(&mut state, &event);
        }
        if state.session_status.working() {
            stream.watched = true;
        }
        self.prompts.on_messages(id, &state.messages);
        stream.state = Some(state);
        self.load_asks(id);
        self.sync_side(id);
    }

    /// Events of the side session: its permission asks, the confirmation of the question, and the conversation itself.
    pub fn on_side_events(&mut self, id: &str, events: Vec<Event>, _tx: &mut Tx) {
        if self.side_session_id().as_deref() != Some(id) {
            return;
        }
        let topic = Topic::Session(id.to_string());
        for event in &events {
            self.on_ask_event(&topic, event);
            if let Event::PromptCommitted { life, correlation_id, event_id } = event {
                let confirm = Confirm {
                    correlation_id: correlation_id.clone(),
                    event_id: *event_id,
                    server_message_id: life.info.get("id").and_then(|i| i.as_str()).map(str::to_string),
                };
                self.prompts.confirm(id, &confirm);
            }
        }
        let stream = self.side_ui.live.streams.entry(id.to_string()).or_default();
        let Some(state) = stream.state.as_mut() else {
            stream.pending.extend(events);
            self.sync_side(id);
            return;
        };
        let was_working = state.session_status.working();
        for event in &events {
            reducer::apply(state, event);
        }
        if !was_working && state.session_status.working() {
            stream.watched = true;
        }
        self.prompts.on_messages(id, &state.messages);
        self.sync_side(id);
    }

    /// Redraws the panel's conversation from the stream and the sent prompts, and reports progress once the snapshot is in.
    /// A side conversation is small, so it is rebuilt whole.
    pub(crate) fn sync_side(&mut self, id: &str) {
        let Some(side) = self.side().and_then(|st| st.side.clone()).filter(|s| s.session_id == id) else { return };
        let Some(stream) = self.side_ui.live.streams.get_mut(id) else { return };
        let empty;
        let state = match &stream.state {
            Some(state) => state,
            None => {
                empty = reducer::create_state(&serde_json::json!({ "messages": [] }));
                &empty
            }
        };
        let folded = fold_compaction_summaries(&state.messages);
        let kept: Vec<&Msg> = messages_after(&state.messages, side.boundary_message_id.as_deref(), |m| m.id.as_str());
        let shown: Vec<Msg> = kept.into_iter().filter(|m| !folded.hidden.contains(&m.id)).cloned().collect();
        let finished = Finished::new();
        let sent = self.prompts.of(id);
        let optimistic: Vec<Message> = sent
            .iter()
            .filter(|prompt| !shown.iter().any(|m| m.id == prompt.id))
            .map(|prompt| {
                let mut m = Message::user(format!("optimistic-{}", prompt.correlation_id), prompt.body.clone());
                m.needs_retry = prompt.status == PromptStatus::NeedsRetry;
                m
            })
            .collect();
        let messages = super::prompts::merge_by_timestamp(
            shown.iter().map(|m| view::message(m, folded.summaries.get(&m.id).map(String::as_str), &finished)).collect(),
            optimistic,
        );
        let running = state.session_status.working();
        let context = ListContext {
            delegations: &state.delegations,
            finished: &finished,
            running: &[],
            older: &[],
            models: &[],
            thinking: self.thinking,
        };
        let options = FoldOptions { running, keep_last: stream.watched, directory: None, open: &self.turns_open };
        let mut plan = FoldPlan::build(&shown, sent, &context, &options);
        plan.place(messages.iter().map(|m| m.id.as_str()));
        let status = sessions::stream_status_of(state.session_status);
        let progress = (running, latest_answer(&shown));
        let loaded = stream.state.is_some();
        let told = (progress.0, progress.1.as_ref().map(|a| a.1.clone()), progress.1.as_ref().map(|a| a.0.clone()));
        let report = loaded && stream.told.as_ref() != Some(&told);
        if report {
            stream.told = Some(told.clone());
        }
        stream.shown = shown;
        self.side_ui.conv = Some(Session {
            id: id.to_string(),
            title: side.title,
            status,
            ago_secs: None,
            branch: String::new(),
            messages,
            tokens: 0,
            row: Default::default(),
            listing: Default::default(),
        });
        self.side_ui.conv_of = Some(id.to_string());
        self.side_ui.plan = plan;
        let shown = self.side_ui.live.streams.get(id).map(|s| s.shown.clone()).unwrap_or_default();
        self.sync_side_questions(id, &shown);
        if let (true, Some(owner)) = (report, self.draft_owner()) {
            self.report_side_progress(&owner, Progress { working: told.0, latest_answer: told.1, latest_answer_id: told.2 });
        }
    }
}
