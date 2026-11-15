//! `@` sessions and `$` skills as plain logic: `session-references.ts` (the token made from a title, `textHasToken`,
//! what a draft remembers, which picks still go with the text, the sessions the `@` list offers) and
//! `skill-mentions.ts` (the `$` trigger, which skills a text names, how a harness delivers them, the list's hint).
//! The chips drawn in a rendered message and the block a sent message carries are component 3's.
//!
//! Positions are byte offsets on character boundaries, where Fleet's are UTF-16 indices; the carets of the text
//! edit are the same kind.

/// A reference to a session, as the draft remembers it: the token in the text (`@` included), the session and its title.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionReference {
    pub token: String,
    pub session_id: String,
    pub title: String,
}

/// The most references one composer remembers; the oldest go first.
pub const MAX_REMEMBERED: usize = 50;
const MAX_SLUG_LENGTH: usize = 40;

/// The Latin letter under an accent, for the letters NFKD would split (Latin-1 and Latin Extended-A, the ones titles
/// have). Fleet's `normalize("NFKD")` plus removing the combining marks does this and more; a crate for the rest
/// isn't worth its build time on three systems, and a letter this misses stays itself, which still makes a token.
fn base_letter(c: char) -> char {
    let table = |from: u32, letters: &str| letters.chars().nth((c as u32).wrapping_sub(from) as usize);
    let folded = match c as u32 {
        0xC0..=0xC5 => Some('A'),
        0xC7 => Some('C'),
        0xC8..=0xCB => Some('E'),
        0xCC..=0xCF => Some('I'),
        0xD1 => Some('N'),
        0xD2..=0xD6 => Some('O'),
        0xD9..=0xDC => Some('U'),
        0xDD => Some('Y'),
        0xE0..=0xE5 => Some('a'),
        0xE7 => Some('c'),
        0xE8..=0xEB => Some('e'),
        0xEC..=0xEF => Some('i'),
        0xF1 => Some('n'),
        0xF2..=0xF6 => Some('o'),
        0xF9..=0xFC => Some('u'),
        0xFD | 0xFF => Some('y'),
        // Latin Extended-A, 0x100 on; letters with no decomposition (Đ, Ħ, ı, ĸ, Ł, Ŋ, Œ, Ŧ) stay themselves.
        0x100..=0x17F => table(
            0x100,
            "AaAaAaCcCcCcCcDdĐđEeEeEeEeEeGgGgGgGgHhĦħIiIiIiIiIiJjJjKkĸLlLlLlLlŁłNnNnNnnŊŋOoOoOoŒœRrRrRrSsSsSsSsTtTtŦŧUuUuUuUuUuUuWwYyYZzZzZzs",
        ),
        _ => None,
    };
    folded.unwrap_or(c)
}

/// A session's title as a token: lower case, words joined by dashes, `session` when nothing is left
/// (`sessionReferenceSlug`).
pub fn session_reference_slug(title: &str) -> String {
    let folded: String = title.chars().filter(|c| !('\u{300}'..='\u{36F}').contains(c)).map(base_letter).collect::<String>().to_lowercase();
    let mut slug = String::new();
    for word in folded.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()) {
        let next = if slug.is_empty() { word.to_string() } else { format!("{slug}-{word}") };
        if next.chars().count() > MAX_SLUG_LENGTH {
            if slug.is_empty() {
                slug = word.chars().take(MAX_SLUG_LENGTH).collect();
            }
            break;
        }
        slug = next;
    }
    if slug.is_empty() { "session".into() } else { slug }
}

/// Where `token` stands on its own in `text` (the start of the token), or none: not after a letter or digit, and not
/// followed by a letter, digit, `_`, `-`, `/`, `@` or a "." plus a letter or digit. `@notes` isn't in `@notes-2`
/// or `@notes.md`, but is in `@notes.` (`tokenPattern`).
pub fn find_token(text: &str, token: &str) -> Option<usize> {
    if token.is_empty() {
        return None;
    }
    let mut from = 0;
    while let Some(found) = text[from..].find(token) {
        let at = from + found;
        let before_ok = text[..at].chars().next_back().is_none_or(|c| c.is_whitespace() || "([\"'".contains(c));
        let mut after = text[at + token.len()..].chars();
        let after_ok = match after.next() {
            None => true,
            Some(c) if c.is_alphanumeric() || "_-/@".contains(c) => false,
            Some('.') => !after.next().is_some_and(|n| n.is_alphanumeric()),
            Some(_) => true,
        };
        if before_ok && after_ok {
            return Some(at);
        }
        from = at + text[at..].chars().next().map_or(1, char::len_utf8);
    }
    None
}

pub fn text_has_token(text: &str, token: &str) -> bool {
    find_token(text, token).is_some()
}

/// What a composer remembers it picked from the `@` list, by the draft it belongs to (the caller keeps one per draft
/// key: the session's id, or the New Session draft's row). Kept with the draft by component 5 piece L.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Picks {
    files: Vec<String>,
    sessions: Vec<SessionReference>,
}

impl Picks {
    pub fn from_parts(files: Vec<String>, sessions: Vec<SessionReference>) -> Picks {
        Picks { files, sessions }
    }

    pub fn files(&self) -> &[String] {
        &self.files
    }

    pub fn sessions(&self) -> &[SessionReference] {
        &self.sessions
    }

    /// The token for `session`: its slug, or the slug with `-2`, `-3`… when another session here already has it. A
    /// session picked before gets its token back (`sessionReferenceToken`).
    pub fn session_token(&self, session_id: &str, title: &str) -> String {
        let base = format!("@{}", session_reference_slug(title));
        for attempt in 1.. {
            let token = if attempt == 1 { base.clone() } else { format!("{base}-{attempt}") };
            match self.sessions.iter().find(|p| p.token == token) {
                Some(holder) if holder.session_id != session_id => {}
                _ => return token,
            }
        }
        unreachable!("an open range")
    }

    /// Remembers that `reference.token` names `reference.session_id`; the newest 50 stay.
    pub fn remember_session(&mut self, reference: SessionReference) {
        self.sessions.retain(|p| p.token != reference.token);
        self.sessions.push(reference);
        let extra = self.sessions.len().saturating_sub(MAX_REMEMBERED);
        self.sessions.drain(..extra);
    }

    /// The picks whose token is still in `text`, in the order they appear: what goes with the message.
    pub fn session_references_in(&self, text: &str) -> Vec<SessionReference> {
        let mut found: Vec<(usize, &SessionReference)> =
            self.sessions.iter().filter_map(|p| find_token(text, &p.token).map(|at| (at, p))).collect();
        found.sort_by_key(|(at, _)| *at);
        found.into_iter().map(|(_, p)| p.clone()).collect()
    }

    pub fn remembered_session_tokens(&self) -> Vec<&str> {
        self.sessions.iter().map(|p| p.token.as_str()).collect()
    }

    /// Forgets every session picked: a New Session draft that's started, or a fresh one.
    pub fn forget_sessions(&mut self) {
        self.sessions.clear();
    }

    /// Remembers that `token` (`@src/app.ts`, `@src/components/`) was picked from the `@` list.
    pub fn remember_file(&mut self, token: &str) {
        if !self.files.iter().any(|t| t == token) {
            self.files.push(token.to_string());
        }
    }

    /// Forgets the picked files and folders no longer in `text`: typed again, they're plain text.
    pub fn forget_files_not_in(&mut self, text: &str) {
        self.files.retain(|t| text_has_token(text, t));
    }

    /// Forgets `tokens`, or every pick when left out.
    pub fn forget_files(&mut self, tokens: Option<&[&str]>) {
        match tokens {
            Some(tokens) => self.files.retain(|t| !tokens.contains(&t.as_str())),
            None => self.files.clear(),
        }
    }

    pub fn is_picked_file(&self, token: &str) -> bool {
        self.files.iter().any(|t| t == token)
    }

    pub fn is_remembered_session(&self, token: &str) -> bool {
        self.sessions.iter().any(|p| p.token == token)
    }
}

// ── The block in a sent message ──────────────────────────────────────────

const BLOCK_OPEN: &str = "<fleet-session-references>\n";
const BLOCK_CLOSE: &str = "</fleet-session-references>";

// Fleet HTML-encodes the attributes.
fn decode(value: &str) -> String {
    [("&quot;", "\""), ("&#39;", "'"), ("&lt;", "<"), ("&gt;", ">"), ("&amp;", "&")]
        .iter()
        .fold(value.to_string(), |text, (entity, plain)| text.replace(entity, plain))
}

fn attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let open = format!("{name}=\"");
    let from = tag.find(&open)? + open.len();
    Some(&tag[from..from + tag[from..].find('"')?])
}

/// A sent message's text without the block Fleet added, and the sessions the block names
/// (`parseSessionReferences`). The block must close the message.
pub fn parse_session_references(body: &str) -> (String, Vec<SessionReference>) {
    let trimmed = body.trim_end();
    let block = trimmed.strip_suffix(BLOCK_CLOSE).and_then(|rest| rest.rfind(BLOCK_OPEN).map(|at| (at, &rest[at..])));
    let Some((at, inside)) = block else { return (body.to_string(), Vec::new()) };
    let references = inside
        .split("<session ")
        .skip(1)
        .filter_map(|tag| {
            Some(SessionReference {
                token: decode(attribute(tag, "ref")?),
                session_id: decode(attribute(tag, "id")?),
                title: decode(attribute(tag, "title")?),
            })
        })
        .collect();
    (body[..at].trim_end().to_string(), references)
}

// ── The @ list's sessions ────────────────────────────────────────────────

/// What `matchReferableSessions` reads of a session in the list (Fleet's `SessionListItem`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SessionItem {
    pub id: String,
    pub title: String,
    pub status: String,
    pub activity: Option<String>,
    pub harness_type: String,
    pub project_name: Option<String>,
    pub workspace_display_name: Option<String>,
    pub workspace_directory: String,
    pub parent_session_id: Option<String>,
    pub archived: bool,
    /// Milliseconds since the epoch: the session's `updated`, else its `created`, else 0.
    pub updated: f64,
}

/// A session offered in the `@` list.
#[derive(Debug, Clone, PartialEq)]
pub struct ReferableSession {
    pub id: String,
    pub title: String,
    pub status: String,
    pub activity: Option<String>,
    /// Where it is and what runs it: "weave-fleet · OpenCode 2 · archived", or "subagent of …".
    pub description: String,
    pub updated_at: f64,
}

fn folder_name(directory: &str) -> &str {
    let trimmed = directory.trim_end_matches(['/', '\\']);
    trimmed.rsplit(['/', '\\']).next().unwrap_or(directory)
}

/// The sessions to offer for `query`, best first: titles that start with it, then titles with a word that does, then
/// any title containing it, newest first within each. An empty query offers the newest. Leaves out `exclude_id`
/// (the session being written to). `harness_name` names a harness's type; none leaves the type itself.
pub fn match_referable_sessions(
    items: &[SessionItem],
    query: &str,
    exclude_id: Option<&str>,
    limit: usize,
    harness_name: &dyn Fn(&str) -> Option<String>,
) -> Vec<ReferableSession> {
    let needle = query.trim().to_lowercase();
    let mut seen = std::collections::HashSet::new();
    let mut ranked: Vec<(&SessionItem, u8)> = Vec::new();
    for item in items {
        if Some(item.id.as_str()) == exclude_id || !seen.insert(item.id.as_str()) {
            continue;
        }
        let title = item.title.to_lowercase();
        let rank = if needle.is_empty() || title.starts_with(&needle) {
            0
        } else if title.split(|c: char| !c.is_alphanumeric()).any(|w| w.starts_with(&needle)) {
            1
        } else if title.contains(&needle) {
            2
        } else {
            continue;
        };
        ranked.push((item, rank));
    }
    ranked.sort_by(|a, b| a.1.cmp(&b.1).then(b.0.updated.total_cmp(&a.0.updated)));

    ranked
        .into_iter()
        .take(limit)
        .map(|(item, _)| {
            let parent = item.parent_session_id.as_deref().and_then(|p| items.iter().find(|i| i.id == p));
            let harness = harness_name(&item.harness_type).unwrap_or_else(|| item.harness_type.clone());
            let place = item
                .project_name
                .clone()
                .or_else(|| item.workspace_display_name.clone())
                .unwrap_or_else(|| folder_name(&item.workspace_directory).into());
            let mut parts = match parent {
                Some(parent) => vec![format!("subagent of {}", parent.title)],
                None => vec![place, harness],
            };
            if item.archived {
                parts.push("archived".into());
            }
            ReferableSession {
                id: item.id.clone(),
                title: item.title.clone(),
                status: item.status.clone(),
                activity: item.activity.clone(),
                description: parts.into_iter().filter(|p| !p.is_empty()).collect::<Vec<_>>().join(" · "),
                updated_at: item.updated,
            }
        })
        .collect()
}

// ── `$` skills ───────────────────────────────────────────────────────────

/// A command as `GET /api/sessions/{id}/commands` lists it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Command {
    pub name: String,
    pub description: Option<String>,
    /// "command", "skill" or none (a harness that can't say).
    pub kind: Option<String>,
    pub user_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSkill {
    pub name: String,
    pub description: Option<String>,
    /// The model can't load it (Claude Code's `disable-model-invocation`): it only runs from the front of a message.
    pub user_only: bool,
}

/// The skills among a session's commands: a command the harness says isn't a skill is left out
/// (`rememberSessionSkills`).
pub fn skills_among(commands: &[Command]) -> Vec<SessionSkill> {
    commands
        .iter()
        .filter(|c| c.kind.as_deref() != Some("command"))
        .map(|c| SessionSkill { name: c.name.clone(), description: c.description.clone(), user_only: c.user_only })
        .collect()
}

/// How a skill reaches the agent: with the message, or as a note naming it that the agent loads it from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillDelivery {
    Attached,
    Note,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillMention {
    pub name: String,
    pub delivery: SkillDelivery,
}

/// Whether `index` is inside backticks: code, where a `$` is the code's own.
pub fn inside_backticks(text: &str, index: usize) -> bool {
    text[..index].matches('`').count() % 2 == 1
}

fn trim_trailing_punctuation(token: &str) -> &str {
    token.trim_end_matches(['.', ',', ';', ':', '!', '?', ')', ']', '}', '\'', '"'])
}

/// Each `$name` token in `text` that names one of `skills`: its name and where it starts and ends, `$` included.
/// A `$` begins one at the start or after whitespace, with a character that is neither whitespace nor `$` next, and
/// the token runs to the next whitespace less sentence punctuation (`skillTokens`).
pub fn skill_tokens(text: &str, skills: &[&str]) -> Vec<(String, usize, usize)> {
    let mut tokens = Vec::new();
    if skills.is_empty() {
        return tokens;
    }
    let mut prev_is_space = true;
    let mut chars = text.char_indices().peekable();
    while let Some((at, c)) = chars.next() {
        if c == '$' && prev_is_space && chars.peek().is_some_and(|(_, n)| !n.is_whitespace() && *n != '$') {
            let end = text[at..].find(char::is_whitespace).map_or(text.len(), |e| at + e);
            let token = trim_trailing_punctuation(&text[at..end]);
            let name = &token[1..];
            if skills.contains(&name) && !inside_backticks(text, at) {
                tokens.push((name.to_string(), at, at + token.len()));
            }
            // The match consumed the token; the next one needs whitespace of its own.
            while chars.peek().is_some_and(|(i, _)| *i < end) {
                chars.next();
            }
            prev_is_space = false;
            continue;
        }
        prev_is_space = c.is_whitespace();
    }
    tokens
}

/// The skills `text` names, each once, in the order they first appear: what goes to the server with the message.
pub fn skill_names_in(text: &str, skills: &[&str]) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for (name, _, _) in skill_tokens(text, skills) {
        if !names.contains(&name) {
            names.push(name);
        }
    }
    names
}

/// How each skill reaches the agent on a harness. OpenCode 2 takes skills with the message. Claude Code expands only
/// the skill a message starts with, so Fleet moves the first one there and names the rest in a note. Every other
/// harness gets the note.
pub fn skill_deliveries(harness_type: Option<&str>, names: &[String]) -> Vec<SkillMention> {
    names
        .iter()
        .enumerate()
        .map(|(index, name)| SkillMention {
            name: name.clone(),
            delivery: if harness_type == Some("opencode2") || (harness_type == Some("claude-code") && index == 0) {
                SkillDelivery::Attached
            } else {
                SkillDelivery::Note
            },
        })
        .collect()
}

/// What the `$` list says about a skill picked on this harness, for its footer.
pub fn skill_list_hint(harness_type: Option<&str>, harness_name: &str) -> String {
    match harness_type {
        Some("opencode2") => format!("{harness_name} loads it with the message"),
        Some("claude-code") => format!("{harness_name} runs the first skill in a message"),
        _ => format!("{harness_name} gets a note naming it, then loads it"),
    }
}

/// Where a `$` the caret is typing starts, and what's typed after it; none when the caret isn't in one.
pub fn skill_trigger(text: &str, caret: usize) -> Option<(usize, String)> {
    let before = &text[..caret];
    let start = before.rfind('$')?;
    if before[..start].chars().next_back().is_some_and(|c| !c.is_whitespace()) {
        return None;
    }
    let query = &before[start + 1..];
    if query.contains(|c: char| c.is_whitespace() || c == '$') || inside_backticks(text, start) {
        return None;
    }
    Some((start, query.to_string()))
}
