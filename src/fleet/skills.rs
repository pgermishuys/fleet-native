//! Fleet's built-in skills and the user's versions of them, for Improve a skill (`lib/skill-versions.ts`,
//! `stores/built-in-skills.ts`): which skills are built in (a finished skill call on one offers Improve), the requests
//! that ask the model for a better one and keep or switch a version, and what of a turn Improve can send along. The
//! dialog is `view::improve_skill` (piece U); the diff it shows is `view::skill_diff`, which Settings' skill history
//! reuses. Every request runs on a thread. With no server (scenes, tests) a small invented set of skills answers.
use super::http;
use super::machines::read_error;
use crate::State;
use crate::view::conversation::{Message, Part, Role};
use serde_json::{Value, json};

/// A built-in skill as Settings lists it (`BuiltInSkill`).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct BuiltInSkill {
    pub name: String,
    pub description: String,
    pub enabled: bool,
    /// The user's version sessions get; `None` for Fleet's.
    pub version: Option<u32>,
    pub fleet_changed: bool,
    pub version_count: u32,
    /// No switch of its own: it is on while the Mods feature is on.
    pub with_mods: bool,
}

/// `GET /api/skills/built-in`: a bare array. Anything else is no skills; an entry with no name is dropped.
pub fn parse_skills(v: &Value) -> Vec<BuiltInSkill> {
    let Some(list) = v.as_array() else { return Vec::new() };
    list.iter()
        .filter_map(|s| {
            let name = s.get("name")?.as_str()?.to_string();
            Some(BuiltInSkill {
                name,
                description: s.get("description").and_then(Value::as_str).unwrap_or_default().to_string(),
                enabled: s.get("enabled").and_then(Value::as_bool).unwrap_or(false),
                version: s.get("version").and_then(Value::as_u64).map(|n| n as u32),
                fleet_changed: s.get("fleetChanged").and_then(Value::as_bool).unwrap_or(false),
                version_count: s.get("versionCount").and_then(Value::as_u64).unwrap_or(0) as u32,
                with_mods: s.get("withMods").and_then(Value::as_bool).unwrap_or(false),
            })
        })
        .collect()
}

/// What the model proposed. Nothing is saved until the user keeps it (`SkillProposal`).
#[derive(Clone, PartialEq, Debug)]
pub struct SkillProposal {
    pub name: String,
    /// The text the change was made to.
    pub base: String,
    /// The user's version it was made to; `None` for Fleet's.
    pub base_version: Option<u32>,
    pub content: String,
    pub asks: u32,
    pub tokens: Option<Tokens>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Tokens {
    pub total: u64,
    pub from_cache: u64,
}

/// A proposal from the server's answer; one with no text is not a proposal.
pub fn parse_proposal(v: &Value) -> Option<SkillProposal> {
    let text = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
    Some(SkillProposal {
        name: text("name")?,
        base: text("base").unwrap_or_default(),
        base_version: v.get("baseVersion").and_then(Value::as_u64).map(|n| n as u32),
        content: text("content")?,
        asks: v.get("asks").and_then(Value::as_u64).unwrap_or(1) as u32,
        tokens: v.get("tokens").filter(|t| t.is_object()).map(|t| Tokens {
            total: t.get("total").and_then(Value::as_u64).unwrap_or(0),
            from_cache: t.get("fromCache").and_then(Value::as_u64).unwrap_or(0),
        }),
    })
}

/// What Improve is asked: the note, and what of the session goes with it (`ImproveRequest`).
#[derive(Clone, PartialEq, Debug, Default)]
pub struct ImproveRequest {
    pub session_id: String,
    pub note: String,
    /// The parts of the session the user chose, as text; `None` sends none.
    pub context: Option<String>,
    /// Read the whole conversation in a fork of the session instead of sending `context`.
    pub whole_conversation: bool,
}

const PATH: &str = "/api/skills/built-in";

/// `/api/skills/built-in/<name>`, the name escaped as a path part (`encodeURIComponent`).
pub fn at(name: &str) -> String {
    format!("{PATH}/{}", crate::conv::image_sniff::encode_component(name))
}

/// `POST …/improve`. Fleet's `JSON.stringify` leaves out what is `undefined`, so a request without context sends no key.
pub fn improve_body(r: &ImproveRequest) -> Value {
    let mut body = json!({"sessionId": r.session_id, "note": r.note});
    if let Some(context) = &r.context {
        body["context"] = json!(context);
    }
    if r.whole_conversation {
        body["wholeConversation"] = json!(true);
    }
    body
}

/// `POST …/versions`: `note` and `sessionId` are sent as null when there are none, as Fleet does.
pub fn save_body(content: &str, note: Option<&str>, session_id: Option<&str>) -> Value {
    json!({"content": content, "note": note, "sessionId": session_id})
}

/// `PUT …/active`: a version number, or null for Fleet's own.
pub fn switch_body(version: Option<u32>) -> Value {
    json!({"version": version})
}

/// "Fleet's" or "Yours · v3": which copy sessions get (`versionLabel`).
pub fn version_label(version: Option<u32>) -> String {
    match version {
        Some(n) if n > 0 => format!("Yours \u{b7} v{n}"),
        _ => "Fleet's".to_string(),
    }
}

/// A rough count of tokens, for showing what a question costs before it is asked (`estimateTokens`): a quarter of the
/// text's UTF-16 length, rounded up, as the browser counts it.
pub fn estimate_tokens(text: &str) -> u64 {
    (text.encode_utf16().count() as u64).div_ceil(4)
}

// ── What Improve can include from the session ───────────────────────────────────────────────────────────────────────

/// How much of each tool call's output goes in: enough to see what happened, not whole files.
const TOOL_OUTPUT_LIMIT: usize = 1500;

/// One tool call of a message, as Improve reads it.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct ImproveTool {
    pub title: String,
    pub kind: Option<String>,
    pub status: Option<String>,
    pub output: Option<String>,
}

/// A message, as Improve reads it (`ImproveMessage`).
#[derive(Clone, PartialEq, Debug, Default)]
pub struct ImproveMessage {
    pub id: String,
    pub role: String,
    pub body: String,
    pub tools: Vec<ImproveTool>,
}

/// The conversation the view holds, as Improve reads it: the text of each message and its tool calls.
pub fn improve_messages(messages: &[Message]) -> Vec<ImproveMessage> {
    messages
        .iter()
        .map(|m| {
            let mut out = ImproveMessage {
                id: m.id.clone(),
                role: match m.role {
                    Role::User => "user",
                    _ => "assistant",
                }
                .to_string(),
                ..Default::default()
            };
            let mut texts = Vec::new();
            for part in &m.parts {
                match part {
                    Part::Text { text, .. } => texts.push(text.as_str()),
                    Part::Tool { tool, detail, status, body, .. } => out.tools.push(ImproveTool {
                        title: detail.clone(),
                        kind: Some(tool.clone()),
                        status: Some(status_word(*status).to_string()),
                        output: Some(body.output.clone()).filter(|o| !o.is_empty()),
                    }),
                    _ => {}
                }
            }
            out.body = texts.join("\n\n");
            out
        })
        .collect()
}

/// The word Fleet's status carries ("Completed" is left unsaid).
fn status_word(status: crate::conv::tools::ToolStatus) -> &'static str {
    use crate::conv::tools::ToolStatus as S;
    match status {
        S::Completed => "Completed",
        S::Error => "Error",
        S::Running => "Running",
        S::Pending => "Pending",
        S::Background => "Background",
        S::Stopped => "Stopped",
        S::Cancelled => "Cancelled",
        S::Other => "Other",
    }
}

/// The turn a skill was used in, split into the parts the user can include; empty where there is nothing.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct ImproveTurn {
    /// What the user asked, and what the agent wrote back in that turn.
    pub exchange: String,
    /// The turn's tool calls, each with the start of its output.
    pub tool_calls: String,
}

/// The turn around `message_id`: back to the prompt that started it and on to the next prompt (`improveTurn`).
pub fn improve_turn(messages: &[ImproveMessage], message_id: &str) -> ImproveTurn {
    let Some(index) = messages.iter().position(|m| m.id == message_id) else { return ImproveTurn::default() };
    let mut start = index;
    while start > 0 && messages[start].role != "user" {
        start -= 1;
    }
    let mut end = index + 1;
    while end < messages.len() && messages[end].role != "user" {
        end += 1;
    }
    let request = if messages[start].role == "user" { messages[start].body.trim() } else { "" };
    let replies: Vec<&ImproveMessage> = messages[start..end].iter().filter(|m| m.role != "user").collect();
    let reply = replies.iter().map(|m| m.body.trim()).filter(|b| !b.is_empty()).collect::<Vec<_>>().join("\n\n");
    let exchange = [
        (!request.is_empty()).then(|| format!("The user asked:\n{request}")),
        (!reply.is_empty()).then(|| format!("The agent answered:\n{reply}")),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join("\n\n");

    let calls: Vec<String> = replies
        .iter()
        .flat_map(|m| m.tools.iter())
        .map(|tool| {
            let mut head = format!("- {}: {}", tool.kind.as_deref().unwrap_or("tool"), tool.title);
            if let Some(status) = tool.status.as_deref().filter(|s| *s != "Completed") {
                head.push_str(&format!(" ({status})"));
            }
            let Some(output) = tool.output.as_deref().map(str::trim).filter(|o| !o.is_empty()) else { return head };
            let shown = if output.chars().count() > TOOL_OUTPUT_LIMIT {
                format!("{}\n\u{2026}", output.chars().take(TOOL_OUTPUT_LIMIT).collect::<String>())
            } else {
                output.to_string()
            };
            let indented = shown.split('\n').map(|l| format!("  {l}")).collect::<Vec<_>>().join("\n");
            format!("{head}\n{indented}")
        })
        .collect();
    let tool_calls = if calls.is_empty() { String::new() } else { format!("The tool calls in that turn:\n{}", calls.join("\n")) };
    ImproveTurn { exchange, tool_calls }
}

// ── What the app keeps ──────────────────────────────────────────────────────────────────────────────────────────────

/// Which skill's Improve the user asked for, from which call's card (`ToolCard` `improve`: the skill and the tool id).
#[derive(Clone, PartialEq, Debug)]
pub struct Improving {
    pub skill: String,
    pub tool_id: String,
}

#[derive(Default)]
pub struct Skills {
    /// `None` until the list came (or no server was given).
    pub list: Option<Vec<BuiltInSkill>>,
    pub loading: bool,
    pub error: Option<String>,
    /// The press on a card's Improve; the dialog (piece U) reads it.
    pub improving: Option<Improving>,
    /// A question is out to the model.
    pub asking: bool,
    /// The last answer to a question: the proposal, or the server's words.
    pub proposal: Option<Result<SkillProposal, String>>,
    /// The last keep or switch: the skill's new state, or the server's words.
    pub saved: Option<Result<BuiltInSkill, String>>,
    /// Scene only: draw the skill diff in place of a Settings page.
    pub demo_diff: bool,
    /// The Improve dialog (piece U).
    pub dialog: super::improve::ImproveDialog,
}

impl Skills {
    pub fn is_built_in(&self, name: &str) -> bool {
        self.list.as_ref().is_some_and(|l| l.iter().any(|s| s.name == name))
    }
}

/// The skills the invented machine has, for scenes and for a run with no server.
pub fn mock_skills() -> Vec<BuiltInSkill> {
    let skill = |name: &str, description: &str| BuiltInSkill {
        name: name.into(),
        description: description.into(),
        enabled: true,
        ..Default::default()
    };
    vec![skill("tidy-docs", "Keep the docs in step with the code"), skill("release-notes", "Draft release notes from merged changes")]
}

/// A skill's answer to a keep or a switch (`BuiltInSkillDetail`), as the list shows it.
pub fn parse_saved(v: &Value) -> Option<BuiltInSkill> {
    let mut skill = parse_skills(&json!([v])).pop()?;
    skill.version_count = v.get("versions").and_then(Value::as_array).map_or(skill.version_count, |a| a.len() as u32);
    Some(skill)
}

pub(super) fn answer(fallback: &str, got: Result<(u16, String), String>) -> Result<Value, String> {
    let (status, body) = got.map_err(|_| fallback.to_string())?;
    if !(200..300).contains(&status) {
        return Err(read_error(&body, fallback.to_string()));
    }
    serde_json::from_str(&body).map_err(|_| fallback.to_string())
}

impl State {
    /// Loads the list once; later calls reuse it (`ensureLoaded`).
    pub fn skills_ensure_loaded(&mut self) {
        if self.skills.list.is_some() || self.skills.loading {
            return;
        }
        let Some(link) = self.link.clone() else {
            self.skills.list = Some(mock_skills());
            return;
        };
        self.skills.loading = true;
        link.off_thread(
            |base| http::get(base, PATH),
            |s, _, result| {
                s.skills.loading = false;
                match result {
                    Ok(v) => {
                        s.skills.list = Some(parse_skills(&v));
                        s.skills.error = None;
                    }
                    Err(why) => s.skills.error = Some(why),
                }
            },
        );
    }

    /// The Improve button on a skill call's card was pressed.
    pub fn skill_improve_open(&mut self, skill: &str, tool_id: &str) {
        self.skills.improving = Some(Improving { skill: skill.into(), tool_id: tool_id.into() });
        self.skills.proposal = None;
        self.improve_open(skill, tool_id);
    }

    /// Asks the model for a better skill (`improveSkill`). A refusal comes back with the server's words.
    pub fn skill_ask(&mut self, name: &str, request: &ImproveRequest) {
        self.skill_ask_for(name, request, 0);
    }

    /// The same for the dialog's ticket, which hears the answer (`improve_asked`); ticket 0 is nobody's.
    pub(super) fn skill_ask_for(&mut self, name: &str, request: &ImproveRequest, ticket: u32) {
        let fallback = format!("Couldn't ask for a better {name}.");
        self.skills.proposal = None;
        let Some(link) = self.link.clone() else {
            self.skills.proposal = Some(Err(fallback));
            self.improve_asked(ticket);
            return;
        };
        self.skills.asking = true;
        let (path, body) = (format!("{}/improve", at(name)), improve_body(request));
        link.off_thread(
            move |base| answer(&fallback, http::text_within("POST", base, &path, Some(&body), 300)),
            move |s, _, result| {
                s.skills.asking = false;
                s.skills.proposal =
                    Some(result.and_then(|v| parse_proposal(&v).ok_or_else(|| "The model's answer had no skill in it.".to_string())));
                s.improve_asked(ticket);
            },
        );
    }

    /// Keeps a proposal as the user's next version (`saveSkillVersion`).
    pub fn skill_keep(&mut self, name: &str, content: &str, note: Option<&str>, session_id: Option<&str>) {
        self.skill_keep_for(name, content, note, session_id, 0, None);
    }

    /// The same for the dialog's ticket, which hears the answer (`improve_kept`); ticket 0 is nobody's.
    pub(super) fn skill_keep_for(
        &mut self,
        name: &str,
        content: &str,
        note: Option<&str>,
        session_id: Option<&str>,
        ticket: u32,
        previous: Option<u32>,
    ) {
        let fallback = format!("Couldn't save your version of {name}.");
        let Some(link) = self.link.clone() else {
            self.skills.saved = Some(Err(fallback));
            self.improve_kept(ticket, previous);
            return;
        };
        let (path, body) = (format!("{}/versions", at(name)), save_body(content, note, session_id));
        link.off_thread(
            move |base| answer(&fallback, http::text_with("POST", base, &path, Some(&body))),
            move |s, _, result| {
                s.skills.saved = Some(result.and_then(|v| parse_saved(&v).ok_or_else(|| "No skill came back.".to_string())));
                s.improve_kept(ticket, previous);
            },
        );
    }

    /// Makes one of the user's versions the one sessions get, or Fleet's for `None` (`switchSkillVersion`; Undo).
    pub fn skill_switch(&mut self, name: &str, version: Option<u32>) {
        let fallback = format!("Couldn't switch {name}.");
        let Some(link) = self.link.clone() else {
            self.skills.saved = Some(Err(fallback));
            return;
        };
        let (path, body) = (format!("{}/active", at(name)), switch_body(version));
        link.off_thread(
            move |base| answer(&fallback, http::text_with("PUT", base, &path, Some(&body))),
            |s, _, result| s.skills.saved = Some(result.and_then(|v| parse_saved(&v).ok_or_else(|| "No skill came back.".to_string()))),
        );
    }
}
