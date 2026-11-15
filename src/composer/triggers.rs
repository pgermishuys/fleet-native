//! The autocomplete popup's logic (`use-autocomplete.ts`), with nothing of the lists' loading in it: which of the
//! three triggers (`/`, `@`, `$`) is open at the caret, the items each lists, the keys, and what picking one does to
//! the text. The lists themselves (commands, agents, files, sessions) are handed in as plain data, `Lists`; piece K
//! fills them from threads and draws the popup.
//!
//! A change to the text, the caret or the lists is the caller's to make; it then asks again. `poll` is what Fleet's
//! `watch([items, isLoading])` is: call it after any of them changes.
use super::files::file_row;
use super::refs::*;
use std::collections::HashMap;

/// How many sessions the `@` list offers: a few for a bare `@`, which is mostly for files, more once you type.
pub const SESSIONS_FOR_BARE_MENTION: usize = 3;
pub const SESSIONS_FOR_QUERY: usize = 5;
const SESSION_VALUE_PREFIX: &str = "session:";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Command,
    Agent,
    File,
    Session,
    Skill,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
    pub group: Group,
    pub value: String,
    /// An agent's colour, or "dir" for a folder.
    pub meta: Option<String>,
    pub session: Option<ReferableSession>,
    /// Why it can't be picked here; the row is greyed out.
    pub disabled_reason: Option<String>,
    /// For a skill: a warning to show while it's selected (it goes as a note, not with the message).
    pub warning: Option<String>,
}

impl Item {
    fn new(id: String, label: String, description: Option<String>, group: Group, value: String) -> Item {
        Item { id, label, description, group, value, meta: None, session: None, disabled_reason: None, warning: None }
    }

    pub fn is_directory(&self) -> bool {
        self.meta.as_deref() == Some("dir")
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Agent {
    pub name: String,
    pub description: Option<String>,
    pub mode: Option<String>,
    pub color: Option<String>,
}

/// What the popup lists from, as the finder has it now.
#[derive(Debug, Clone, Default)]
pub struct Lists {
    /// The session the composer writes to; none on the New Session page (no `/` list, no `$`, no agents).
    pub session_id: Option<String>,
    /// Whose picks these are: the session's id, or the New Session draft's row key.
    pub draft_key: Option<String>,
    pub harness_type: Option<String>,
    /// A harness's name by its type, for the sessions' rows and the skill warning.
    pub harness_names: HashMap<String, String>,
    /// Fleet's own commands (`/btw`), listed before the harness's; a harness command of the same name is left out.
    pub fleet_commands: Vec<Command>,
    pub commands: Vec<Command>,
    pub commands_loading: bool,
    pub commands_error: Option<String>,
    pub agents: Vec<Agent>,
    pub agents_loading: bool,
    pub agents_error: Option<String>,
    pub files: Vec<String>,
    pub files_loading: bool,
    pub files_error: Option<String>,
    /// The sessions the `@` list can offer (the store's merged with the fetched ones).
    pub sessions: Vec<SessionItem>,
    pub(super) skills: Vec<SessionSkill>,
}

impl Lists {
    /// The session's commands arrived: they and the skills among them replace the old ones.
    pub fn set_commands(&mut self, commands: Vec<Command>) {
        self.skills = skills_among(&commands);
        self.commands = commands;
    }

    /// The session's skills; none until its commands have loaded, and none without a session.
    pub fn skills(&self) -> &[SessionSkill] {
        if self.session_id.is_some() { &self.skills } else { &[] }
    }

    fn skill_names(&self) -> Vec<&str> {
        self.skills().iter().map(|s| s.name.as_str()).collect()
    }

    fn harness_name(&self, kind: &str) -> Option<String> {
        self.harness_names.get(kind).cloned()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerKind {
    Slash,
    Mention,
    Skill,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Trigger {
    pub kind: TriggerKind,
    pub start: usize,
}

/// The text and caret, with the draft's picks, which picking one changes.
pub struct Ctx<'a> {
    pub text: &'a str,
    pub caret: usize,
    pub lists: &'a Lists,
    pub picks: &'a mut Picks,
}

/// A change to the text a pick makes: the whole new text, and where the caret goes (set at once, and again on the
/// next tick, as Fleet does).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    pub text: String,
    pub caret: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Down,
    Up,
    Enter,
    Tab,
    Escape,
    /// Any other key.
    Other,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KeyOutcome {
    /// The popup took the key (`preventDefault`).
    pub consumed: bool,
    /// The key must not go on to anything else, an interrupt on Esc (`stopPropagation`).
    pub stop: bool,
    pub edit: Option<Edit>,
    /// The popup was open for this key, so Enter, Tab and Esc don't reach the composer (Enter doesn't send). A `$`
    /// list with no items that isn't loading counts as closed here: "costs $5" still sends.
    pub swallowed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Pending {
    draft: String,
    key: Key,
}

#[derive(Debug, Clone, Default)]
pub struct Autocomplete {
    selected: usize,
    /// Esc closed the popup at this text; it stays closed until the text differs.
    suppressed: Option<String>,
    /// Enter or Tab pressed before anything matched what's typed: the draft then, and the key.
    pending: Option<Pending>,
}

impl Autocomplete {
    pub fn new() -> Autocomplete {
        Autocomplete::default()
    }

    /// Which list is open at the caret, if any (`computedTrigger`).
    pub fn trigger(&self, ctx: &Ctx) -> Option<Trigger> {
        let (text, caret) = (ctx.text, ctx.caret);
        if text.is_empty() {
            return None;
        }
        // A `$` the caret is typing, anywhere (after a `/command` too), when the session has skills to offer.
        if let Some((start, _)) = skill_trigger(text, caret)
            && (!ctx.lists.skills().is_empty() || ctx.lists.commands_loading)
        {
            return Some(Trigger { kind: TriggerKind::Skill, start });
        }
        if text.starts_with('/') && caret >= 1 && ctx.lists.session_id.is_some() {
            return if text[1..caret].contains(' ') { None } else { Some(Trigger { kind: TriggerKind::Slash, start: 0 }) };
        }
        let before = &text[..caret];
        let at = before.rfind('@')?;
        if before[..at].chars().next_back().is_some_and(|c| !c.is_whitespace()) {
            return None;
        }
        let between = &before[at + 1..];
        if between.contains(' ') {
            return None;
        }
        // The caret at the end of a reference picked from this list: it's done, not being typed.
        if ctx.lists.draft_key.is_some() {
            let token = format!("@{between}");
            if ctx.picks.is_picked_file(&token) || ctx.picks.is_remembered_session(&token) {
                return None;
            }
        }
        Some(Trigger { kind: TriggerKind::Mention, start: at })
    }

    /// What's typed after the `@`, `/` or `$`, for the popup to mark in the rows.
    pub fn query<'a>(&self, ctx: &'a Ctx) -> &'a str {
        match self.trigger(ctx) {
            Some(t) => &ctx.text[t.start + 1..ctx.caret],
            None => "",
        }
    }

    /// What the file search is asked: what's typed after an `@`, and none for any other trigger (`mentionQuery`).
    pub fn file_query<'a>(&self, ctx: &'a Ctx) -> Option<&'a str> {
        self.trigger(ctx).filter(|t| t.kind == TriggerKind::Mention).map(|t| &ctx.text[t.start + 1..ctx.caret])
    }

    fn is_suppressed(&self, ctx: &Ctx) -> bool {
        self.suppressed.as_deref() == Some(ctx.text)
    }

    pub fn is_open(&self, ctx: &Ctx) -> bool {
        self.trigger(ctx).is_some() && !self.is_suppressed(ctx)
    }

    pub fn is_mention_open(&self, ctx: &Ctx) -> bool {
        self.is_open(ctx) && self.trigger(ctx).is_some_and(|t| t.kind == TriggerKind::Mention)
    }

    pub fn is_skill_open(&self, ctx: &Ctx) -> bool {
        self.is_open(ctx) && self.trigger(ctx).is_some_and(|t| t.kind == TriggerKind::Skill)
    }

    pub fn is_loading(&self, ctx: &Ctx) -> bool {
        match self.trigger(ctx).map(|t| t.kind) {
            Some(TriggerKind::Slash | TriggerKind::Skill) => ctx.lists.commands_loading,
            Some(TriggerKind::Mention) => ctx.lists.agents_loading || ctx.lists.files_loading,
            None => false,
        }
    }

    pub fn error(&self, ctx: &Ctx) -> Option<String> {
        match self.trigger(ctx).map(|t| t.kind) {
            Some(TriggerKind::Slash | TriggerKind::Skill) => ctx.lists.commands_error.clone(),
            Some(TriggerKind::Mention) => ctx.lists.agents_error.clone().or_else(|| ctx.lists.files_error.clone()),
            None => None,
        }
    }

    pub fn items(&self, ctx: &Ctx) -> Vec<Item> {
        let Some(trigger) = self.trigger(ctx) else { return Vec::new() };
        if self.is_suppressed(ctx) {
            return Vec::new();
        }
        let lists = ctx.lists;
        let filter_text = &ctx.text[trigger.start + 1..ctx.caret];
        let filter = filter_text.to_lowercase();

        match trigger.kind {
            TriggerKind::Slash => {
                let own: Vec<&Command> = lists.fleet_commands.iter().collect();
                let rest = lists.commands.iter().filter(|c| !lists.fleet_commands.iter().any(|o| o.name == c.name));
                own.into_iter()
                    .chain(rest)
                    .filter(|c| c.name.to_lowercase().starts_with(&filter))
                    .map(|c| {
                        Item::new(
                            format!("command:{}", c.name),
                            format!("/{}", c.name),
                            c.description.clone(),
                            Group::Command,
                            format!("/{} ", c.name),
                        )
                    })
                    .collect()
            }
            TriggerKind::Skill => self.skill_items(ctx, trigger.start, &filter),
            TriggerKind::Mention => {
                let agents = lists.agents.iter().filter(|a| {
                    filter.is_empty()
                        || a.name.to_lowercase().contains(&filter)
                        || a.description.as_ref().is_some_and(|d| d.to_lowercase().contains(&filter))
                });
                let agent_items = agents.map(|a| {
                    let mut item = Item::new(
                        format!("agent:{}", a.name),
                        format!("@{}", a.name),
                        a.description.clone().or_else(|| a.mode.clone()),
                        Group::Agent,
                        format!("@{} ", a.name),
                    );
                    item.meta = a.color.clone();
                    item
                });
                let file_items = lists.files.iter().map(|path| {
                    let row = file_row(path);
                    let mut item = Item::new(format!("file:{path}"), row.label, row.description, Group::File, format!("@{path} "));
                    item.meta = row.is_directory.then(|| "dir".to_string());
                    item
                });
                let limit = if filter.is_empty() { SESSIONS_FOR_BARE_MENTION } else { SESSIONS_FOR_QUERY };
                let names = |kind: &str| lists.harness_name(kind);
                let session_items = match_referable_sessions(&lists.sessions, filter_text, lists.session_id.as_deref(), limit, &names)
                    .into_iter()
                    .map(|s| {
                        let mut item = Item::new(
                            format!("session:{}", s.id),
                            s.title.clone(),
                            Some(s.description.clone()),
                            Group::Session,
                            format!("{SESSION_VALUE_PREFIX}{}", s.id),
                        );
                        item.session = Some(s);
                        item
                    });
                // Sessions, then files and folders: a session is the rarer pick, so the few that match go on top.
                session_items.chain(file_items).chain(agent_items).collect()
            }
        }
    }

    /// The session's skills matching what's typed after `$`, names starting with it first. On Claude Code, which
    /// expands only the skill a message starts with, a skill picked after another goes as a note, and one the model
    /// can't load can't go at all.
    fn skill_items(&self, ctx: &Ctx, start: usize, filter: &str) -> Vec<Item> {
        let lists = ctx.lists;
        let others = format!("{}{}", &ctx.text[..start], &ctx.text[ctx.caret..]);
        let first = if lists.harness_type.as_deref() == Some("claude-code") {
            skill_names_in(&others, &lists.skill_names()).into_iter().next()
        } else {
            None
        };
        let harness = lists.harness_name("claude-code").unwrap_or_else(|| "Claude Code".into());

        let mut skills: Vec<&SessionSkill> = lists.skills().iter().filter(|s| s.name.to_lowercase().contains(filter)).collect();
        skills.sort_by_key(|s| !s.name.to_lowercase().starts_with(filter));
        skills
            .into_iter()
            .map(|skill| {
                let mut item = Item::new(
                    format!("skill:{}", skill.name),
                    skill.name.clone(),
                    skill.description.clone(),
                    Group::Skill,
                    format!("${} ", skill.name),
                );
                if let Some(first) = first.as_ref().filter(|f| **f != skill.name) {
                    if skill.user_only {
                        item.disabled_reason = Some(format!(
                            "The model can't load {}, so it only runs first in a message, and this one already runs {first}.",
                            skill.name
                        ));
                    } else {
                        item.warning = Some(format!(
                            "This message already runs {first}. {harness} runs one skill per message, so this one goes as a note."
                        ));
                    }
                }
                item
            })
            .collect()
    }

    /// The highlighted row: the selection, held inside the list.
    pub fn selected_index(&self, ctx: &Ctx) -> usize {
        let len = self.items(ctx).len();
        if len == 0 { 0 } else { self.selected.min(len - 1) }
    }

    pub fn selected_value(&self, ctx: &Ctx) -> Option<String> {
        self.items(ctx).get(self.selected_index(ctx)).map(|i| i.value.clone())
    }

    /// Whether Enter or Tab waits for an answer (`pendingPick`).
    pub fn has_pending(&self) -> bool {
        self.pending.is_some()
    }

    /// Picks a row by its value (a click, or a key): returns the edit, or none when nothing is open or a session
    /// can't be told. Remembers a picked file or session in the draft's picks. `onSelect`.
    pub fn on_select(&mut self, ctx: &mut Ctx, selected: &str) -> Option<Edit> {
        let trigger = self.trigger(ctx)?;
        let draft_key = ctx.lists.draft_key.is_some();
        let mut item_value = selected.to_string();
        if let Some(id) = selected.strip_prefix(SESSION_VALUE_PREFIX) {
            // A session goes in as a token made from its title; the composer remembers which session the token names.
            let session = self.items(ctx).into_iter().find(|i| i.value == selected).and_then(|i| i.session);
            let session = session.filter(|_| draft_key)?;
            debug_assert_eq!(session.id, id);
            let token = ctx.picks.session_token(&session.id, &session.title);
            ctx.picks.remember_session(SessionReference { token: token.clone(), session_id: session.id, title: session.title });
            item_value = format!("{token} ");
        } else if draft_key && self.items(ctx).iter().any(|i| i.value == selected && i.group == Group::File) {
            // A file or folder picked (not a folder opened to look inside): the composer edits it as one piece.
            ctx.picks.remember_file(selected.trim_end());
        }

        let edit = if trigger.kind == TriggerKind::Slash {
            Edit { caret: item_value.len(), text: item_value }
        } else {
            let before = &ctx.text[..trigger.start];
            let after = &ctx.text[ctx.caret..];
            Edit { caret: before.len() + item_value.len(), text: format!("{before}{item_value}{after}") }
        };
        self.selected = 0;
        self.suppressed = None;
        Some(edit)
    }

    /// Opens a folder in the popup: the text becomes `@folder/` and the folder's contents are listed; the folder
    /// isn't picked.
    pub fn on_open_folder(&mut self, ctx: &mut Ctx, value: &str) -> Option<Edit> {
        // Trimmed, the value matches no row, so the folder isn't remembered as a pick.
        self.on_select(ctx, value.trim_end())
    }

    /// Esc: close it, and keep it closed until the text differs.
    pub fn on_close(&mut self, ctx: &Ctx) {
        self.suppressed = Some(ctx.text.to_string());
        self.selected = 0;
    }

    fn pick(&mut self, ctx: &mut Ctx, item: &Item, key: Key) -> Option<Edit> {
        if item.disabled_reason.is_some() {
            return None;
        }
        if key == Key::Tab && item.is_directory() {
            self.on_open_folder(ctx, &item.value.clone())
        } else {
            self.on_select(ctx, &item.value.clone())
        }
    }

    /// `onKeyDown`. A key that isn't a pick or a move goes on to the composer's own handling unless `swallowed`.
    pub fn on_key(&mut self, ctx: &mut Ctx, key: Key) -> KeyOutcome {
        let was_open = self.is_open(ctx) && !(self.is_skill_open(ctx) && self.items(ctx).is_empty() && !self.is_loading(ctx));
        let mut out = self.handle_key(ctx, key);
        out.swallowed = was_open && matches!(key, Key::Enter | Key::Tab | Key::Escape);
        out
    }

    fn handle_key(&mut self, ctx: &mut Ctx, key: Key) -> KeyOutcome {
        let mut out = KeyOutcome::default();
        if !self.is_open(ctx) {
            return out;
        }
        let items = self.items(ctx);
        match key {
            Key::Down => {
                out.consumed = true;
                self.selected = if items.is_empty() { 0 } else { (self.selected + 1) % items.len() };
            }
            Key::Up => {
                out.consumed = true;
                self.selected = if items.is_empty() { 0 } else { (self.selected + items.len() - 1) % items.len() };
            }
            Key::Enter | Key::Tab => {
                let at = self.selected_index(ctx);
                match items.get(at) {
                    Some(item) if item.disabled_reason.is_some() => out.consumed = true,
                    Some(item) => {
                        out.consumed = true;
                        out.edit = self.pick(ctx, item, key);
                    }
                    None if self.is_loading(ctx) => {
                        out.consumed = true;
                        self.pending = Some(Pending { draft: ctx.text.to_string(), key });
                    }
                    None => {}
                }
            }
            Key::Escape => {
                out.consumed = true;
                out.stop = true;
                self.on_close(ctx);
            }
            Key::Other => {
                self.selected = 0;
                if self.suppressed.as_deref().is_some_and(|s| s != ctx.text) {
                    self.suppressed = None;
                }
            }
        }
        out
    }

    /// The lists, the text or the caret changed: a held Enter or Tab picks the first match once the answer is in, unless
    /// the draft changed or the popup closed meanwhile (`watch([items, isLoading])`).
    pub fn poll(&mut self, ctx: &mut Ctx) -> Option<Edit> {
        let pending = self.pending.clone()?;
        if ctx.text != pending.draft || !self.is_open(ctx) {
            self.pending = None;
            return None;
        }
        if self.is_loading(ctx) {
            return None;
        }
        self.pending = None;
        let item = self.items(ctx).into_iter().next()?;
        self.pick(ctx, &item, pending.key)
    }
}
