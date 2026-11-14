//! SignalR's JSON protocol, the part Fleet's hub uses (`docs/FLEET.md`, "The hub"): after the
//! WebSocket opens, every message is a JSON object ending in the byte 0x1e.
use serde_json::{Value, json};

const SEP: char = '\u{1e}';

/// The first thing a client sends.
pub fn handshake() -> String {
    format!("{{\"protocol\":\"json\",\"version\":1}}{SEP}")
}

/// A call to a hub method.
pub fn invocation(id: u32, target: &str, args: &[Value]) -> String {
    format!("{}{SEP}", json!({"type": 1, "invocationId": id.to_string(), "target": target, "arguments": args}))
}

pub fn pong() -> String {
    format!("{{\"type\":6}}{SEP}")
}

#[derive(Debug, PartialEq)]
pub enum Frame {
    /// The server's answer to the handshake; the error is its reason for refusing.
    Handshake(Option<String>),
    /// `Event(topic, eventId, data)`, the one method the server calls. `event_id` is the middle
    /// argument: null for a live event of the hub's own, else a number (negative for some).
    Event {
        topic: String,
        event_id: Option<i64>,
        data: Value,
    },
    /// The result of an invocation of ours.
    Completion {
        id: String,
        result: Value,
        error: Option<String>,
    },
    Ping,
    Close(Option<String>),
    Other,
}

/// Every frame in one WebSocket message.
pub fn parse(message: &str) -> Vec<Frame> {
    message.split(SEP).filter(|f| !f.is_empty()).map(frame).collect()
}

fn frame(text: &str) -> Frame {
    let Ok(v) = serde_json::from_str::<Value>(text) else { return Frame::Other };
    let s = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
    match v.get("type").and_then(Value::as_i64) {
        None => Frame::Handshake(s("error")),
        Some(1) if s("target").as_deref() == Some("Event") => {
            let args = v.get("arguments").and_then(Value::as_array);
            let event_id = args.and_then(|a| a.get(1)).and_then(Value::as_i64);
            match (args.and_then(|a| a.first()).and_then(Value::as_str), args.and_then(|a| a.get(2))) {
                (Some(topic), Some(data)) => Frame::Event { topic: topic.to_string(), event_id, data: data.clone() },
                _ => Frame::Other,
            }
        }
        Some(3) => Frame::Completion {
            id: s("invocationId").unwrap_or_default(),
            result: v.get("result").cloned().unwrap_or(Value::Null),
            error: s("error"),
        },
        Some(6) => Frame::Ping,
        Some(7) => Frame::Close(s("error")),
        _ => Frame::Other,
    }
}
