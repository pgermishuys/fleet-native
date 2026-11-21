//! What one machine's feed reads (component 23, piece C; `lib/phone/inbox-feed.ts`, `lib/mission-control-requests.ts`
//! `fetchNow`): the session list, then what each waiting session waits on, and what each session is doing this second. The
//! parts that decide are plain functions; `Reach` is how a call goes out, and every call here blocks, so the caller runs
//! them on a thread.
use super::{Ask, NowKind, SessionNow};
use crate::asks::permission::to_permission_ask;
use crate::asks::question::{QuestionPart, QuestionStatus};
use crate::conv::export_history::convert_fleet_message;
use crate::fleet::groups::Listed;
use crate::fleet::http;
use crate::fleet::machine::Machine;
use crate::fleet::machine_routes;
use crate::fleet::machines::Answer;
use crate::fleet::model::PartData;
use crate::fleet::sessions::parse_sessions_for;
use crate::fleet::store::Retention;
use crate::mock::Session;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

/// How many sessions a read lists (`SESSION_PAGE`).
pub const SESSION_PAGE: usize = 100;
/// How many waiting sessions a read looks at the asks of.
pub const MAX_ASKS: usize = 12;
/// How many of a session's newest messages are searched for a question that waits (`messages?limit=6`).
pub const QUESTION_WINDOW: usize = 6;

/// Where a read goes: home's own server, or another machine at its picked address (with the token held for it, and a
/// second address tried when the first doesn't answer).
#[derive(Clone)]
pub enum Reach {
    Home(String),
    Machine(Box<Machine>),
}

impl Reach {
    /// The address calls go to now.
    pub fn base(&self) -> String {
        match self {
            Reach::Home(base) => base.clone(),
            Reach::Machine(m) => m.base().to_string(),
        }
    }

    pub fn get(&self, path: &str) -> Answer {
        match self {
            Reach::Home(base) => http::get_page(base, path),
            Reach::Machine(m) => machine_routes::get_on(m, path),
        }
    }
}

/// What a read of one machine brought: its sessions and the asks of the waiting ones, by session id.
#[derive(Default)]
pub struct Inbox {
    pub sessions: Vec<Session>,
    pub asks: HashMap<String, Ask>,
}

/// A read that failed: `Some(words)` when the machine answered and said no (`FeedError`), `None` when it didn't answer at
/// all or said something that isn't a list.
pub type Failed = Option<String>;

/// `readInbox`'s refusal: a 401 says the key stopped working, any other status its number.
pub fn refusal(status: u16) -> String {
    if status == 401 { "This phone's key for this machine stopped working.".to_string() } else { format!("It answered {status}.") }
}

/// The list answer, as sessions.
pub fn parse_list(answer: Answer, now_ms: i64) -> Result<Vec<Session>, Failed> {
    let (status, body) = answer.map_err(|_| None)?;
    if !(200..300).contains(&status) {
        return Err(Some(refusal(status)));
    }
    let rows: Value = serde_json::from_str(&body).map_err(|_| None)?;
    Ok(parse_sessions_for(&rows, now_ms, Retention::All))
}

/// `readInbox`: the sessions, then each waiting one's ask (at most `MAX_ASKS`, each on a thread: one that can't be read
/// has no ask, and doesn't fail the read).
pub fn read_inbox(reach: &Reach, now_ms: i64) -> Result<Inbox, Failed> {
    let sessions = parse_list(reach.get(&format!("/api/sessions?limit={SESSION_PAGE}&offset=0")), now_ms)?;
    let waiting: Vec<&str> = sessions.iter().filter(|s| s.status_word() == "waiting_input").take(MAX_ASKS).map(|s| s.id.as_str()).collect();
    let asks = std::thread::scope(|scope| {
        let reads: Vec<_> = waiting.iter().map(|id| (*id, scope.spawn(|| read_ask(reach, id)))).collect();
        reads.into_iter().filter_map(|(id, read)| Some((id.to_string(), read.join().ok()??))).collect()
    });
    Ok(Inbox { sessions, asks })
}

/// `readAsk`: the first permission the session asks, else a question it waits on in its last few messages.
fn read_ask(reach: &Reach, session: &str) -> Option<Ask> {
    let id = crate::conv::export_history::encode_component(session);
    if let Ok((status, body)) = reach.get(&format!("/api/sessions/{id}/permissions"))
        && (200..300).contains(&status)
        && let Ok(Value::Array(list)) = serde_json::from_str::<Value>(&body)
        && let Some(first) = list.iter().find_map(to_permission_ask)
    {
        return Some(Ask::Permission(Box::new(first)));
    }
    let (status, body) = reach.get(&format!("/api/sessions/{id}/messages?limit={QUESTION_WINDOW}")).ok()?;
    if !(200..300).contains(&status) {
        return None;
    }
    find_pending_question(&serde_json::from_str(&body).ok()?)
}

/// `findPendingQuestion`: the newest question tool call that still waits, from a page of messages as the server writes it.
/// A call answered anywhere counts as answered, because a snapshot and a live update can carry one call as two parts.
pub fn find_pending_question(page: &Value) -> Option<Ask> {
    // The messages route writes Fleet's own flat shape (`FleetMessage`), not the hub's: `findPendingQuestion` converts it with
    // `convertFleetMessageToAccumulated`.
    let wire = page.get("messages").and_then(Value::as_array).map_or(&[][..], Vec::as_slice);
    let messages: Vec<_> = wire.iter().filter_map(|m| convert_fleet_message(m, "")).collect();
    let calls = |part: &PartData| match part {
        PartData::Tool { id, tool, call_id, state } => {
            // History written by older servers has no call id; the part's id stands in for it.
            QuestionPart::read(tool, state).map(|q| (if call_id.is_empty() { id.clone() } else { call_id.clone() }, q))
        }
        _ => None,
    };
    let settled: HashSet<String> = messages
        .iter()
        .flat_map(|m| &m.parts)
        .filter_map(calls)
        .filter(|(_, q)| matches!(q.status, QuestionStatus::Completed | QuestionStatus::Error))
        .map(|(call, _)| call)
        .collect();
    messages.iter().rev().flat_map(|m| m.parts.iter().rev()).filter_map(calls).find_map(|(call, q)| {
        if settled.contains(&call) || !q.status.is_active() {
            return None;
        }
        let input = q.input.filter(|i| !i.questions.is_empty())?;
        let more = input.questions.len() - 1;
        Some(Ask::Question { request_id: call, question: input.questions.into_iter().next()?, more })
    })
}

/// `fetchNow`: what each session on the machine is doing this second. The first call turns the machine's bookkeeping on, so
/// it answers empty once. A machine too old to know it says 404, and one that doesn't answer says nothing: no lines, no error.
pub fn read_now(reach: &Reach) -> HashMap<String, SessionNow> {
    match reach.get("/api/sessions/now") {
        Ok((status, body)) if (200..300).contains(&status) => parse_now(&serde_json::from_str(&body).unwrap_or(Value::Null)),
        _ => HashMap::new(),
    }
}

pub fn parse_now(body: &Value) -> HashMap<String, SessionNow> {
    let Some(lines) = body.as_object() else { return HashMap::new() };
    lines
        .iter()
        .filter_map(|(id, line)| {
            let kind = match line.get("kind")?.as_str()? {
                "tool" => NowKind::Tool,
                "thinking" => NowKind::Thinking,
                "reply" => NowKind::Reply,
                _ => return None,
            };
            let text = line.get("text")?.as_str()?.to_string();
            let at = line.get("at").and_then(Value::as_str).unwrap_or_default().to_string();
            Some((id.clone(), SessionNow { text, kind, at }))
        })
        .collect()
}
