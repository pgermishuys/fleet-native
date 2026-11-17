//! The tool registry: what the client knows about each tool, in one place (`lib/tools/registry.ts`,
//! `input.ts`, `text.ts`), and how a call's row is worked out (`lib/tool-row.ts`).
//!
//! One descriptor per canonical tool name carries its category, label, icon and heading. Aliases and
//! case are handled once, by [`get_tool`]: `Read`, `read` and `mcp__fleet__fleet_page_show` all
//! resolve here. Icons are named by [`ToolIcon`] so this file holds no drawing code.
use serde_json::{Map, Value};

// ── text.ts ───────────────────────────────────────────────────────────────────

/// Shorten a file path to `…/parentDir/filename` when it exceeds 50 characters.
pub fn shorten_path(path: &str) -> String {
    if path.chars().count() <= 50 {
        return path.to_string();
    }
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() <= 2 { path.to_string() } else { format!("…/{}", parts[parts.len() - 2..].join("/")) }
}

/// Truncate to `max` characters, appending "…" if trimmed. (Fleet counts UTF-16 units; the two differ
/// only for characters outside the basic plane.)
pub fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max { value.to_string() } else { format!("{}…", value.chars().take(max).collect::<String>()) }
}

/// `read` → `Read`.
pub fn capitalize(value: &str) -> String {
    let mut chars = value.chars();
    chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

// ── input.ts ──────────────────────────────────────────────────────────────────

/// The first of `keys` that holds a non-blank string, as written (not trimmed).
pub fn string_field(input: Option<&Map<String, Value>>, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|k| input?.get(*k)?.as_str().filter(|s| !s.trim().is_empty()).map(str::to_string))
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct ToolQuestion {
    pub question: Option<String>,
    pub header: Option<String>,
}

/// The fields the common tools carry, whichever harness sent them. Harnesses disagree on the names
/// (OpenCode says `filePath`, OpenCode 2 `path`, Claude Code `file_path`); every field is optional.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ToolInput {
    pub file_path: Option<String>,
    pub command: Option<String>,
    pub description: Option<String>,
    pub pattern: Option<String>,
    pub url: Option<String>,
    pub query: Option<String>,
    /// A skill's name: OpenCode says `name`, OpenCode 2 `id`.
    pub skill: Option<String>,
    pub agent: Option<String>,
    pub code: Option<String>,
    pub content: Option<String>,
    pub old_string: Option<String>,
    pub new_string: Option<String>,
    pub questions: Vec<ToolQuestion>,
    /// Whatever else the call sent, for tools that have their own fields.
    pub raw: Map<String, Value>,
}

pub fn as_record(value: &Value) -> Option<&Map<String, Value>> {
    value.as_object()
}

pub fn parse_tool_input(input: &Value) -> ToolInput {
    let record = as_record(input);
    let field = |keys: &[&str]| string_field(record, keys);
    let questions = record
        .and_then(|r| r.get("questions")?.as_array())
        .map(|list| {
            list.iter()
                .filter_map(as_record)
                .map(|q| ToolQuestion { question: string_field(Some(q), &["question"]), header: string_field(Some(q), &["header"]) })
                .collect()
        })
        .unwrap_or_default();
    ToolInput {
        file_path: field(&["filePath", "path", "file_path", "file", "notebook_path"]),
        command: field(&["command"]),
        description: field(&["description"]),
        pattern: field(&["pattern"]),
        url: field(&["url"]),
        query: field(&["query"]),
        skill: field(&["name", "id"]),
        agent: field(&["agent"]),
        code: record.and_then(|r| r.get("code")?.as_str()).map(str::to_string),
        content: field(&["content", "text", "newContent", "new_content"]),
        old_string: field(&["oldString", "old_string"]),
        new_string: field(&["newString", "new_string"]),
        questions,
        raw: record.cloned().unwrap_or_default(),
    }
}

// ── registry.ts ───────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    Read,
    Search,
    Edit,
    Shell,
    Web,
    Subagent,
    Question,
    Plan,
    Skill,
    Fleet,
    Other,
}

/// The server's word for the same call (`Permissions.Classify`); it decides what an ask says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PermissionKind {
    Read,
    Edit,
    Shell,
    Web,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileWrite {
    Create,
    Modify,
}

/// The lucide icon a tool is drawn with; `view::conversation` maps each to its `src/icons.rs` constant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolIcon {
    AppWindow,
    BookOpen,
    Camera,
    CircleCheck,
    Code,
    FileText,
    Folder,
    GitBranch,
    Globe,
    Layers,
    Lightbulb,
    LightbulbOff,
    ListChecks,
    ListTodo,
    MessageCircleQuestion,
    MessagesSquare,
    MonitorUp,
    PackageCheck,
    PanelsTopLeft,
    Pencil,
    RefreshCw,
    Search,
    Send,
    Server,
    Terminal,
    Wrench,
}

type Label = fn(&ToolInput, &str) -> String;

struct Descriptor {
    /// Canonical name: lowercase, as the server sends it.
    name: &'static str,
    /// Other names the same tool goes by, any case. Punctuation is ignored (`apply_patch` is `applypatch`).
    aliases: &'static [&'static str],
    category: Category,
    permission: PermissionKind,
    heading: &'static str,
    icon: ToolIcon,
    label: Option<Label>,
    file_write: Option<FileWrite>,
    /// The card's title is Fleet's answer ("Messaged Update documentation"), when Fleet gave one.
    titled_from_answer: bool,
    /// The call drives the browser, so its row offers browser steps.
    uses_browser: bool,
    /// The label is a search pattern, drawn as a pill.
    pattern_label: bool,
}

const fn d(name: &'static str, category: Category, permission: PermissionKind, heading: &'static str, icon: ToolIcon) -> Descriptor {
    Descriptor {
        name,
        aliases: &[],
        category,
        permission,
        heading,
        icon,
        label: None,
        file_write: None,
        titled_from_answer: false,
        uses_browser: false,
        pattern_label: false,
    }
}

impl Descriptor {
    const fn label(mut self, label: Label) -> Self {
        self.label = Some(label);
        self
    }
    const fn aka(mut self, aliases: &'static [&'static str]) -> Self {
        self.aliases = aliases;
        self
    }
    const fn writes(mut self, kind: FileWrite) -> Self {
        self.file_write = Some(kind);
        self
    }
    const fn answered(mut self) -> Self {
        self.titled_from_answer = true;
        self
    }
    const fn browser(mut self) -> Self {
        self.uses_browser = true;
        self
    }
    const fn pattern(mut self) -> Self {
        self.pattern_label = true;
        self
    }
}

// ── Label builders ────────────────────────────────────────────────────────────

fn text(input: &ToolInput, key: &str) -> Option<String> {
    string_field(Some(&input.raw), &[key])
}

fn shell_label(input: &ToolInput, fallback: &str) -> String {
    if let Some(description) = &input.description {
        return description.clone();
    }
    input.command.as_deref().map_or_else(|| fallback.to_string(), |c| truncate(c, 60))
}

fn file_label(input: &ToolInput, fallback: &str) -> String {
    input.file_path.as_deref().map_or_else(|| fallback.to_string(), shorten_path)
}

fn pattern_label(input: &ToolInput, fallback: &str) -> String {
    input.pattern.clone().unwrap_or_else(|| fallback.to_string())
}

/// JS `Number(value)` for the cases a tool input holds: a number, or a numeric string.
fn number(input: &ToolInput, key: &str) -> Option<f64> {
    match input.raw.get(key)? {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.trim().parse().ok(),
        _ => None,
    }
}

fn credits(input: &ToolInput) -> Option<String> {
    number(input, "credits").filter(|n| *n > 0.0).map(|n| format!("{n:.2} credits"))
}

/// Joins the facts that are present with " · ", capitalised; the fallback when none is.
fn facts(parts: Vec<Option<String>>, fallback: &str) -> String {
    let label = parts.into_iter().flatten().collect::<Vec<_>>().join(" · ");
    if label.is_empty() { fallback.to_string() } else { capitalize(&label) }
}

/// A HydraFusion pass: "Critic · model · needs repair · 43.0 s · 0.20 credits".
fn pass_label(input: &ToolInput, fallback: &str) -> String {
    facts(
        vec![
            text(input, "role"),
            text(input, "model"),
            text(input, "verdict"),
            number(input, "durationMs").filter(|n| *n > 0.0).map(|n| format!("{:.1} s", n / 1000.0)),
            credits(input),
            text(input, "reason").map(|r| truncate(&r, 60)),
        ],
        fallback,
    )
}

/// The head of a turn: "Cascade · 4.07 credits · 4 requests".
fn turn_label(input: &ToolInput, fallback: &str) -> String {
    let requests = number(input, "requests").filter(|n| *n > 0.0).map(|n| format!("{n} {}", if n == 1.0 { "request" } else { "requests" }));
    facts(vec![text(input, "pattern"), credits(input), requests, text(input, "degraded").map(|_| "degraded".into())], fallback)
}

fn open_label(input: &ToolInput, fallback: &str) -> String {
    let what = input.command.as_ref().or(input.url.as_ref());
    let title = text(input, "title").unwrap_or_default();
    match what {
        Some(what) if title.is_empty() => truncate(what, 60),
        Some(what) => format!("{title} · {}", truncate(what, 60)),
        None if title.is_empty() => fallback.to_string(),
        None => title,
    }
}

fn page_label(input: &ToolInput, fallback: &str) -> String {
    let title = text(input, "title").unwrap_or_default();
    match text(input, "path") {
        Some(path) if title.is_empty() => truncate(&shorten_path(&path), 60),
        Some(path) => format!("{title} · {}", truncate(&shorten_path(&path), 60)),
        None if title.is_empty() => fallback.to_string(),
        None => title,
    }
}

fn screenshot_label(input: &ToolInput, _: &str) -> String {
    let viewport = text(input, "viewport").unwrap_or_else(|| "desktop".into());
    let path = text(input, "path").map(|p| format!(" {}", truncate(&p, 40))).unwrap_or_default();
    format!("screenshot{path} ({viewport})")
}

fn browser_read_label(input: &ToolInput, _: &str) -> String {
    let what = text(input, "what").unwrap_or_else(|| "page".into());
    match text(input, "text") {
        Some(quoted) => format!("{what} “{}”", truncate(&quoted, 40)),
        None => what,
    }
}

fn browser_act_label(input: &ToolInput, _: &str) -> String {
    let action = text(input, "action").unwrap_or_else(|| "act".into());
    match ["url", "ref", "text"].iter().find_map(|k| text(input, k)) {
        Some(target) => format!("{action} {}", truncate(&target, 60)),
        None => action,
    }
}

fn subagent_label(input: &ToolInput, fallback: &str) -> String {
    let (agent, description) = (input.agent.as_deref().unwrap_or(""), input.description.as_deref().unwrap_or(""));
    if !agent.is_empty() && !description.is_empty() {
        return format!("{agent} · {}", truncate(description, 60));
    }
    [description, agent, fallback].into_iter().find(|s| !s.is_empty()).unwrap_or("").to_string()
}

fn question_label(input: &ToolInput, fallback: &str) -> String {
    let first = input.questions.first();
    match first.and_then(|q| q.question.as_ref().or(q.header.as_ref())) {
        Some(heading) => truncate(heading, 60),
        None => fallback.to_string(),
    }
}

fn code_label(input: &ToolInput, fallback: &str) -> String {
    match input.code.as_deref().unwrap_or("").lines().find(|l| !l.trim().is_empty()) {
        Some(line) => truncate(line.trim(), 60),
        None => fallback.to_string(),
    }
}

fn todo_label(input: &ToolInput, fallback: &str) -> String {
    match input.raw.get("todos").and_then(Value::as_array) {
        Some(todos) if !todos.is_empty() => format!("{} {}", todos.len(), if todos.len() == 1 { "todo" } else { "todos" }),
        _ => fallback.to_string(),
    }
}

fn named(key: &'static str, default: &'static str, limit: Option<usize>) -> impl Fn(&ToolInput) -> String {
    move |input| match text(input, key) {
        Some(v) => limit.map_or(v.clone(), |n| truncate(&v, n)),
        None => default.to_string(),
    }
}

fn remember_label(input: &ToolInput, _: &str) -> String {
    named("text", "remember", Some(80))(input)
}
fn search_notes_label(input: &ToolInput, _: &str) -> String {
    named("query", "search notes", Some(80))(input)
}
fn forget_label(input: &ToolInput, _: &str) -> String {
    named("id", "forget note", None)(input)
}
fn write_mod_label(input: &ToolInput, _: &str) -> String {
    named("name", "write mod", None)(input)
}
fn check_mod_label(input: &ToolInput, _: &str) -> String {
    named("name", "check mod", None)(input)
}
fn reload_mod_label(input: &ToolInput, _: &str) -> String {
    named("name", "reload mod", None)(input)
}
fn keep_mod_label(input: &ToolInput, _: &str) -> String {
    named("name", "keep mod", None)(input)
}
fn title_label(input: &ToolInput, fallback: &str) -> String {
    text(input, "title").unwrap_or_else(|| fallback.to_string())
}
fn url_label(input: &ToolInput, fallback: &str) -> String {
    input.url.clone().unwrap_or_else(|| fallback.to_string())
}
fn query_label(input: &ToolInput, fallback: &str) -> String {
    input.query.as_deref().map_or_else(|| fallback.to_string(), |q| truncate(q, 60))
}
fn skill_label(input: &ToolInput, fallback: &str) -> String {
    input.skill.clone().unwrap_or_else(|| fallback.to_string())
}

// ── The descriptors ───────────────────────────────────────────────────────────

use Category as C;
use FileWrite::{Create, Modify};
use PermissionKind as P;
use ToolIcon as I;

static DESCRIPTORS: &[Descriptor] = &[
    // Reading and searching
    d("read", C::Read, P::Read, "Read", I::FileText).label(file_label),
    d("list", C::Read, P::Read, "List", I::Folder).aka(&["ls"]).label(file_label),
    d("lsp", C::Read, P::Read, "Lsp", I::Wrench).label(file_label),
    d("glob", C::Search, P::Read, "Glob", I::Search).label(pattern_label).pattern(),
    d("grep", C::Search, P::Read, "Grep", I::Search).label(pattern_label).pattern(),
    d("codesearch", C::Search, P::Read, "Codesearch", I::Search).aka(&["search"]),
    // Writing
    d("edit", C::Edit, P::Edit, "Edit", I::Pencil).aka(&["multiedit", "strreplaceeditor"]).label(file_label).writes(Modify),
    d("write", C::Edit, P::Edit, "Write", I::Pencil).label(file_label).writes(Create),
    d("notebookedit", C::Edit, P::Edit, "Notebook Edit", I::Pencil).label(file_label).writes(Create),
    d("patch", C::Edit, P::Edit, "Patch", I::Pencil).aka(&["apply_patch"]).label(file_label).writes(Modify),
    // Running
    d("bash", C::Shell, P::Shell, "Bash", I::Terminal).label(shell_label),
    d("shell", C::Shell, P::Shell, "Shell", I::Terminal).aka(&["terminal"]).label(shell_label),
    d("execute", C::Shell, P::Shell, "Code", I::Code).label(code_label).browser(),
    // The web
    d("webfetch", C::Web, P::Web, "Web Fetch", I::Globe).aka(&["fetch"]).label(url_label),
    d("websearch", C::Search, P::Web, "Web Search", I::Search).label(query_label),
    // The agent's own tools
    d("task", C::Subagent, P::Read, "Task", I::GitBranch).aka(&["agent"]),
    d("subagent", C::Subagent, P::Read, "Subagent", I::GitBranch).label(subagent_label),
    d("hydrafusion_pass", C::Subagent, P::Read, "HydraFusion pass", I::Layers).label(pass_label),
    d("copilot_turn", C::Subagent, P::Read, "Turn", I::Layers).label(turn_label),
    d("skill", C::Skill, P::Read, "Skill", I::Layers).label(skill_label),
    d("question", C::Question, P::Read, "Question", I::MessageCircleQuestion).label(question_label),
    d("todowrite", C::Plan, P::Read, "Update todos", I::ListTodo).aka(&["todo"]).label(todo_label),
    d("todoread", C::Plan, P::Read, "Read todos", I::ListChecks),
    // Fleet's own tools
    d("fleet_app_start", C::Fleet, P::Read, "Run app", I::AppWindow).label(open_label).answered(),
    d("fleet_browser_open", C::Fleet, P::Read, "Open page", I::Globe).label(open_label).answered(),
    d("fleet_browser_screenshot", C::Fleet, P::Read, "Screenshot", I::Camera).label(screenshot_label).answered(),
    d("fleet_browser_read", C::Fleet, P::Read, "Read page", I::Globe).label(browser_read_label).browser(),
    d("fleet_browser_act", C::Fleet, P::Read, "Use page", I::Globe).label(browser_act_label).browser(),
    d("fleet_page_show", C::Fleet, P::Read, "Show page", I::PanelsTopLeft).label(page_label).answered(),
    d("fleet_walkthrough_show", C::Fleet, P::Read, "Walkthrough", I::BookOpen).label(title_label).answered(),
    d("fleet_message", C::Fleet, P::Read, "Message session", I::Send).answered(),
    d("fleet_session_read", C::Fleet, P::Read, "Read session", I::MessagesSquare).answered(),
    d("fleet_machine_list", C::Fleet, P::Read, "List machines", I::Server).answered(),
    d("fleet_session_start", C::Fleet, P::Read, "Start session on a machine", I::MonitorUp).answered(),
    d("fleet_memory_save", C::Fleet, P::Read, "Remember", I::Lightbulb).label(remember_label),
    d("fleet_memory_search", C::Fleet, P::Read, "Search notes", I::Search).label(search_notes_label),
    d("fleet_memory_forget", C::Fleet, P::Read, "Forget note", I::LightbulbOff).label(forget_label),
    d("fleet_mod_write", C::Fleet, P::Read, "Write mod", I::Pencil).label(write_mod_label),
    d("fleet_mod_check", C::Fleet, P::Read, "Check mod", I::CircleCheck).label(check_mod_label),
    d("fleet_mod_reload", C::Fleet, P::Read, "Reload mod", I::RefreshCw).label(reload_mod_label),
    d("fleet_mod_keep", C::Fleet, P::Read, "Keep mod", I::PackageCheck).label(keep_mod_label),
    d("fleet_step_done", C::Fleet, P::Read, "Step done", I::CircleCheck),
];

/// Tools the server classifies for permission asks that the UI has nothing else to say about, lowercased.
fn server_only_kind(lowered: &str) -> Option<PermissionKind> {
    Some(match lowered {
        "notebookread"
        | "exitplanmode"
        | "listmcpresourcestool"
        | "readmcpresourcetool"
        | "opencode_list_mcp_resources"
        | "opencode_read_mcp_resource" => P::Read,
        "move" => P::Edit,
        "bashoutput" | "killshell" | "killbash" => P::Shell,
        _ => return None,
    })
}

/// `Apply_Patch` → `applypatch`: case and punctuation do not tell tools apart.
fn compact(name: &str) -> String {
    name.to_lowercase().chars().filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit()).collect()
}

fn find_descriptor(name: &str) -> Option<&'static Descriptor> {
    let key = compact(name);
    DESCRIPTORS.iter().find(|d| compact(d.name) == key || d.aliases.iter().any(|a| compact(a) == key))
}

/// `mcp__fleet__fleet_page_show` → its server and tool; any other name → `None`.
pub fn parse_mcp_name(name: &str) -> Option<(String, String)> {
    let rest = name.strip_prefix("mcp__")?;
    let split = rest.find("__")?;
    (split > 0 && split + 2 < rest.len()).then(|| (rest[..split].to_string(), rest[split + 2..].to_string()))
}

/// A tool, resolved: everything the client knows about a name, with fallbacks for names it does not know.
pub struct Tool {
    /// The name as it arrived.
    pub raw: String,
    /// The canonical name when the tool is known; otherwise the lowercased name without any `mcp__server__` prefix.
    pub name: String,
    pub known: bool,
    pub mcp: Option<(String, String)>,
    pub category: Category,
    pub permission: PermissionKind,
    pub icon: ToolIcon,
    /// The name as a header: "Web Fetch"; for an unknown tool its name with a capital.
    pub heading: String,
    /// The tool part of an MCP name, capitalised ("Lookup_order").
    pub display_name: String,
    pub file_write: Option<FileWrite>,
    pub titled_from_answer: bool,
    pub uses_browser: bool,
    pub pattern_label: bool,
    descriptor: Option<&'static Descriptor>,
}

impl Tool {
    /// A one-line summary of the call; the tool's name as it arrived when the input has nothing to say.
    pub fn label(&self, input: &Value) -> String {
        match self.descriptor.and_then(|d| d.label) {
            Some(build) => build(&parse_tool_input(input), &self.raw),
            None => self.raw.clone(),
        }
    }
}

/// Everything the client knows about a tool name, in any case and under any alias. Never fails.
pub fn get_tool(name: &str) -> Tool {
    let mcp = parse_mcp_name(name);
    // Fleet's own MCP server is Fleet's tools under another name; any other server's tools are nobody's we know.
    let lookup = match &mcp {
        Some((server, tool)) if server == "fleet" => Some(tool.as_str()),
        Some(_) => None,
        None => Some(name),
    };
    let descriptor = lookup.and_then(find_descriptor);
    let lowered = mcp.as_ref().map_or(name, |(_, tool)| tool.as_str()).to_lowercase();
    let (mut permission, mut category) = (P::Other, C::Other);
    if let Some(d) = descriptor {
        (permission, category) = (d.permission, d.category);
    } else if mcp.is_none() {
        if lowered.starts_with("fleet_") {
            (permission, category) = (P::Read, C::Fleet);
        } else if let Some(kind) = server_only_kind(&lowered) {
            permission = kind;
            category = match kind {
                P::Read => C::Read,
                P::Edit => C::Edit,
                P::Shell => C::Shell,
                P::Web => C::Web,
                P::Other => C::Other,
            };
        }
    }
    Tool {
        raw: name.to_string(),
        name: descriptor.map_or(lowered, |d| d.name.to_string()),
        known: descriptor.is_some(),
        category,
        permission,
        icon: descriptor.map_or(ToolIcon::Wrench, |d| d.icon),
        heading: descriptor.map_or_else(|| capitalize(name), |d| d.heading.to_string()),
        display_name: capitalize(mcp.as_ref().map_or(name, |(_, tool)| tool.as_str())),
        file_write: descriptor.and_then(|d| d.file_write),
        titled_from_answer: descriptor.is_some_and(|d| d.titled_from_answer),
        uses_browser: descriptor.is_some_and(|d| d.uses_browser),
        pattern_label: descriptor.is_some_and(|d| d.pattern_label),
        mcp,
        descriptor,
    }
}

/// Every canonical name with its aliases, for the test that each name is claimed once.
pub fn all_names() -> Vec<&'static str> {
    DESCRIPTORS.iter().flat_map(|d| std::iter::once(d.name).chain(d.aliases.iter().copied())).collect()
}

/// The canonical names of the tools that satisfy `flag`, in table order.
pub fn names_where(flag: impl Fn(&Tool) -> bool) -> Vec<&'static str> {
    DESCRIPTORS.iter().filter(|d| flag(&get_tool(d.name))).map(|d| d.name).collect()
}

// ── tool-row.ts: status, duration, grouping ───────────────────────────────────

/// The harness's status word, capitalised as Fleet's `formatToolStatus` does ("Running"); the card keys on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolStatus {
    Pending,
    Running,
    /// The call returned while its work runs on.
    Background,
    Completed,
    Cancelled,
    /// Work Fleet's Stop ended: said as the strip says it, not as a failure.
    Stopped,
    Error,
    /// Any other word the harness sent.
    Other,
}

/// The glyph a status is drawn with (`TOOL_STATUS_TO_GLYPH`); `waiting_input` is drawn instead while a page call waits for a pick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glyph {
    Idle,
    Running,
    Completed,
    Error,
    WaitingInput,
}

impl ToolStatus {
    /// `formatToolStatus`: a missing word is Pending.
    pub fn from_word(word: Option<&str>) -> ToolStatus {
        match word.filter(|w| !w.is_empty()).map(capitalize).as_deref() {
            None | Some("Pending") => ToolStatus::Pending,
            Some("Running") => ToolStatus::Running,
            Some("Background") => ToolStatus::Background,
            Some("Completed") => ToolStatus::Completed,
            Some("Cancelled") => ToolStatus::Cancelled,
            Some("Stopped") => ToolStatus::Stopped,
            Some("Error") => ToolStatus::Error,
            Some(_) => ToolStatus::Other,
        }
    }

    pub fn glyph(self) -> Glyph {
        match self {
            ToolStatus::Running | ToolStatus::Background => Glyph::Running,
            ToolStatus::Completed => Glyph::Completed,
            ToolStatus::Error => Glyph::Error,
            _ => Glyph::Idle,
        }
    }

    /// What the right-hand end of the row shows: only a running, background or failed call has a glyph; Completed shows
    /// nothing (the turn's fold says it), and so do Pending, Cancelled and any other word.
    pub fn shows_glyph(self) -> bool {
        matches!(self, ToolStatus::Running | ToolStatus::Background | ToolStatus::Error)
    }
}

/// A call's run time as a row says it: "0.1s" under a second's tenth, "34s", "2m 5s". Empty when nothing was timed.
pub fn format_call_duration(ms: Option<f64>) -> String {
    let Some(ms) = ms.filter(|m| m.is_finite() && *m >= 0.0) else { return String::new() };
    if ms < 1000.0 {
        return format!("{:.1}s", (ms / 100.0).round().max(1.0) / 10.0);
    }
    let seconds = (ms / 1000.0).round() as u64;
    if seconds < 60 { format!("{seconds}s") } else { format!("{}m {}s", seconds / 60, seconds % 60) }
}

/// How long a call ran, from the start and end times the harness kept on its state (ms).
pub fn tool_duration_ms(state: &Value) -> Option<f64> {
    let time = state.get("time")?;
    let (start, end) = (time.get("start")?.as_f64()?, time.get("end")?.as_f64()?);
    (end >= start).then_some(end - start)
}

/// The shell header: "bun test · exit 0 · 0.1s", leaving out what the harness doesn't report.
pub fn shell_header(input: &Value, fallback: &str, exit: Option<i64>, duration_ms: Option<f64>) -> String {
    let command = parse_tool_input(input).command.and_then(|c| c.lines().next().map(|l| l.trim().to_string())).filter(|c| !c.is_empty());
    let mut parts = vec![command.unwrap_or_else(|| fallback.to_string())];
    if let Some(code) = exit {
        parts.push(format!("exit {code}"));
    }
    let took = format_call_duration(duration_ms);
    if !took.is_empty() {
        parts.push(took);
    }
    parts.join(" · ")
}

/// A read: a file or a folder looked at, which the conversation dims and collapses with the reads beside it.
pub fn is_read_call(kind: &str) -> bool {
    get_tool(kind).category == Category::Read
}

/// The call drew a diagram or sequence canvas, which Fleet no longer keeps; any harness's name for it (`isRetiredCanvasTool`).
pub fn is_retired_canvas_tool(kind: &str) -> bool {
    matches!(get_tool(kind).name.as_str(), "fleet_canvas_open" | "fleet_canvas_patch")
}

pub fn is_write_call(kind: &str) -> bool {
    get_tool(kind).file_write.is_some()
}

/// A shell row: category shell, except Code Mode's `execute`, which has no command to show.
pub fn is_shell_row(kind: &str) -> bool {
    let tool = get_tool(kind);
    tool.category == Category::Shell && tool.name != "execute"
}

/// What the grouping needs to know of a call.
pub trait ToolRowItem {
    fn id(&self) -> &str;
    fn kind(&self) -> &str;
    fn status(&self) -> ToolStatus;
    /// The call started a sub-agent: it has its own row.
    fn delegated(&self) -> bool {
        false
    }
}

impl<T: ToolRowItem> ToolRowItem for &T {
    fn id(&self) -> &str {
        (**self).id()
    }
    fn kind(&self) -> &str {
        (**self).kind()
    }
    fn status(&self) -> ToolStatus {
        (**self).status()
    }
    fn delegated(&self) -> bool {
        (**self).delegated()
    }
}

/// A read that collapses into a "Read N files" row: done, or still on its way (its path may not have arrived yet, so
/// alone it would read as a bare "Read"). A failed or stopped read keeps its own row: a failure is never folded away.
pub fn is_groupable_read(tool: &impl ToolRowItem) -> bool {
    !tool.delegated()
        && is_read_call(tool.kind())
        && matches!(tool.status(), ToolStatus::Completed | ToolStatus::Running | ToolStatus::Pending)
}

#[derive(Debug, PartialEq)]
pub enum ToolRowEntry<T> {
    Tool(T),
    /// Two or more reads in a row; `id` is `reads-{first id}`.
    Reads {
        tools: Vec<T>,
        id: String,
    },
}

/// The rows a message draws: two or more reads in a row (done or still streaming) become one "Read N files" entry.
pub fn group_reads<T: ToolRowItem>(tools: impl IntoIterator<Item = T>) -> Vec<ToolRowEntry<T>> {
    let mut entries = Vec::new();
    let mut run: Vec<T> = Vec::new();
    let flush = |run: &mut Vec<T>, entries: &mut Vec<ToolRowEntry<T>>| {
        if run.len() >= 2 {
            let id = format!("reads-{}", run[0].id());
            entries.push(ToolRowEntry::Reads { tools: std::mem::take(run), id });
        } else {
            entries.extend(run.drain(..).map(ToolRowEntry::Tool));
        }
    };
    for tool in tools {
        if is_groupable_read(&tool) {
            run.push(tool);
        } else {
            flush(&mut run, &mut entries);
            entries.push(ToolRowEntry::Tool(tool));
        }
    }
    flush(&mut run, &mut entries);
    entries
}
