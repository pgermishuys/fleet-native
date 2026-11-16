//! The palette commands that act on the open session (`lib/session-commands.ts`, `ActivityStream.vue` 503-539): Copy session
//! ID, and Export raw messages (JSON), which writes the loaded messages to a file through the system's save dialog (the same
//! one Export as Markdown uses). The file's words are plain data; the write runs on a thread.
use super::model::{Msg, PartData};
use crate::view::conversation::Role;
use crate::{State, Tx};
use rune::FileDialogOptions;
use serde_json::{Map, Value, json};

/// `new Date(ms).toISOString()`.
pub fn iso_utc(ms: i64) -> String {
    let (secs, millis) = (ms.div_euclid(1000), ms.rem_euclid(1000));
    let (days, rest) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Howard Hinnant's civil-from-days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{millis:03}Z", rest / 3600, rest % 3600 / 60, rest % 60)
}

/// The download's name: the title with every run of characters outside `[a-z0-9-_]` turned into one `-`, trimmed of `-`,
/// else the session's id, then `-conversation.json`.
pub fn raw_file_name(title: &str, session: &str) -> String {
    let (mut stem, mut in_other) = (String::new(), false);
    for c in title.chars() {
        if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
            stem.push(c);
            in_other = false;
        } else if !in_other {
            stem.push('-');
            in_other = true;
        }
    }
    let stem = stem.trim_matches('-');
    format!("{}-conversation.json", if stem.is_empty() { session } else { stem })
}

fn role_word(role: Role) -> &'static str {
    match role {
        Role::User => "user",
        Role::Assistant => "assistant",
        Role::Shell => "shell",
    }
}

/// A part as the accumulated message holds it (`AccumulatedPart`).
fn part_json(part: &PartData) -> Value {
    match part {
        PartData::Text { id, text } => json!({ "partId": id, "type": "text", "text": text }),
        PartData::Reasoning { id, text, summary } => {
            let mut o = json!({ "partId": id, "type": "reasoning", "text": text });
            if let Some(s) = summary {
                o["summary"] = json!(s);
            }
            o
        }
        PartData::Tool { id, tool, call_id, state } => {
            json!({ "partId": id, "type": "tool", "tool": tool, "callId": call_id, "state": state })
        }
        PartData::File { id, mime, filename, url } => {
            let mut o = json!({ "partId": id, "type": "file", "mime": mime, "url": url });
            if let Some(f) = filename {
                o["filename"] = json!(f);
            }
            o
        }
        PartData::Compaction { id, trigger, tokens_before, tokens_after, summary } => {
            let mut o = Map::new();
            o.insert("partId".into(), json!(id));
            o.insert("type".into(), json!("compaction"));
            for (k, v) in [
                ("trigger", trigger.as_ref().map(|t| json!(t))),
                ("tokensBefore", tokens_before.map(|t| json!(t))),
                ("tokensAfter", tokens_after.map(|t| json!(t))),
                ("summary", summary.as_ref().map(|t| json!(t))),
            ] {
                if let Some(v) = v {
                    o.insert(k.into(), v);
                }
            }
            Value::Object(o)
        }
    }
}

/// An `AccumulatedMessage`: the keys Fleet's file has, those the message lacks left out (as `JSON.stringify` leaves out
/// `undefined`).
pub fn message_json(m: &Msg) -> Value {
    let mut o = Map::new();
    o.insert("messageId".into(), json!(m.id));
    o.insert("sessionId".into(), json!(m.session_id));
    o.insert("role".into(), json!(role_word(m.role)));
    o.insert("parts".into(), Value::Array(m.parts.iter().map(part_json).collect()));
    let optional: [(&str, Option<Value>); 8] = [
        ("cost", m.cost.map(|c| json!(c))),
        ("tokens", m.tokens.map(|t| json!({ "input": t.input, "output": t.output, "reasoning": t.reasoning }))),
        ("createdAt", m.created_at.map(|t| json!(t))),
        ("agent", m.agent.as_ref().map(|a| json!(a))),
        ("modelID", m.model_id.as_ref().map(|a| json!(a))),
        ("completedAt", m.completed_at.map(|t| json!(t))),
        ("parentID", m.parent_id.as_ref().map(|a| json!(a))),
        ("turnError", m.turn_error.clone()),
    ];
    for (k, v) in optional {
        if let Some(v) = v {
            o.insert(k.into(), v);
        }
    }
    if let Some(f) = &m.finish {
        o.insert("finish".into(), json!(f));
    }
    if m.steered {
        o.insert("steered".into(), json!(true));
    }
    if let Some(c) = &m.command {
        let mut command = Map::new();
        command.insert("name".into(), json!(c.name));
        if let Some(a) = &c.arguments {
            command.insert("arguments".into(), json!(a));
        }
        o.insert("command".into(), Value::Object(command));
    }
    if let Some(mark) = &m.mod_mark {
        let kind = match mark.kind {
            super::model::ModKind::Sent => "sent",
            super::model::ModKind::Rewritten => "rewritten",
        };
        o.insert("mod".into(), json!({ "kind": kind, "name": mark.name }));
    }
    if m.compaction_summary {
        o.insert("compactionSummary".into(), json!(true));
    }
    Value::Object(o)
}

/// The file's content: `{ sessionId, title, exportedAt, messages }`, indented by two spaces.
pub fn raw_export(session: &str, title: &str, exported_at: &str, messages: &[Msg]) -> String {
    // The keys in Fleet's order (serde_json's map sorts them), the messages indented one level in.
    let text = |v: &str| Value::String(v.to_string()).to_string();
    let list = serde_json::to_string_pretty(&messages.iter().map(message_json).collect::<Vec<_>>()).unwrap_or_default();
    format!(
        "{{\n  \"sessionId\": {},\n  \"title\": {},\n  \"exportedAt\": {},\n  \"messages\": {}\n}}",
        text(session),
        text(title),
        text(exported_at),
        list.replace('\n', "\n  ")
    )
}

impl State {
    /// `copy-session-id`: this session's id on the clipboard; Rune can't report a refusal, and Fleet ignores one.
    pub fn copy_session_id(&mut self, tx: &mut Tx) {
        if !self.has_session() {
            return;
        }
        tx.copy(self.session().id.clone());
    }

    /// `export-raw-messages`: the loaded messages (what the conversation holds now) through the system's save dialog. With
    /// no server the invented session has no wire messages, so the file holds an empty list.
    pub fn export_raw_messages(&mut self, tx: &mut Tx) {
        if !self.has_session() {
            return;
        }
        let session = self.session();
        let id = session.id.clone();
        let title = if session.title.is_empty() { id.clone() } else { session.title.clone() };
        let content = raw_export(
            &id,
            &title,
            &iso_utc(self.now_ms.unwrap_or_else(super::link::now_ms)),
            self.stream.as_ref().map_or(&[][..], |s| s.messages.as_slice()),
        );
        let name = raw_file_name(&title, &id);
        let options = FileDialogOptions { title: "Save raw messages".into(), file_name: Some(name.clone()), ..Default::default() }
            .filter("JSON", &["json"]);
        tx.save_file(options, move |s: &mut State, answer, tx| {
            if let Ok(Some(path)) = answer {
                s.save_text(name, path, content, tx);
            }
        });
    }
}
