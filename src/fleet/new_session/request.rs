//! What the New session page sends to start a session, and the list row it puts in at once
//! (`lib/new-session-request.ts`: `buildCreateSessionRequest`, `buildCreatedSessionRow`, `slugForBranch`,
//! `titleFromMessage`; rows A13.2, A13.7, A13.10, A13.11, A20.1-A20.9). Pure: no Rune, no I/O.
//!
//! The GitHub preset is plain data here (`GitHubPreset`), as the boundary in `docs/parity/07-…` asks: core builds the
//! request a GitHub item needs, and component 21 is what fills the preset in.
use serde_json::{Map, Value, json};
use std::sync::Arc;

/// Where the session runs: a scanned repository, any folder, or nowhere (a quick chat).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Folder {
    Repository(String),
    Directory(String),
    None,
}

/// How a repository session gets its checkout. Only used when the folder is a repository.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Workspace {
    Current,
    New,
    Existing(String),
}

/// What a GitHub item attaches to the page (`GitHubSessionSourcePreset`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitHubPreset {
    /// `github-issue` or `github-pull-request`.
    pub source_type: String,
    pub owner: String,
    pub repo: String,
    pub number: u64,
    pub title: String,
    pub body: Option<String>,
    pub html_url: String,
    pub repo_full_name: String,
    pub suggested_branch: Option<String>,
}

/// A picture that goes with the first message (`ImageAttachment`): its type, name and base64 text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    pub mime: String,
    pub filename: Option<String>,
    pub data: Arc<str>,
}

/// The page's choices when Enter is pressed (`NewSessionState`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewSessionState {
    pub folder: Folder,
    pub workspace: Workspace,
    pub message: String,
    pub title: Option<String>,
    pub project_id: Option<String>,
    pub tags: Vec<String>,
    pub harness_type: Option<String>,
    /// The profile to start with, or the "no profile" pick; `None` lets the server use the harness's default.
    pub harness_profile_id: Option<String>,
    pub github_preset: Option<GitHubPreset>,
    /// Branch for a new worktree, instead of the one generated from the message.
    pub branch: Option<String>,
    /// Where a new worktree starts (`origin/<name>` or a local branch); `None` for the default.
    pub base_branch: Option<String>,
    /// Fetch an `origin/…` base first (default true).
    pub fetch_origin: bool,
    /// The agent to start with; `None` for the harness's default.
    pub agent: Option<String>,
    /// Provider id and model id; `None` for the agent's default.
    pub model: Option<(String, String)>,
    /// Sessions picked from the `@` list whose tokens are in the message, as (token, session id).
    pub session_references: Vec<(String, String)>,
    /// Images that go with the message; left out when there's no message.
    pub attachments: Vec<Image>,
}

impl NewSessionState {
    /// A page with a folder and workspace chosen and nothing else.
    pub fn new(folder: Folder, workspace: Workspace) -> Self {
        NewSessionState {
            folder,
            workspace,
            message: String::new(),
            title: None,
            project_id: None,
            tags: Vec::new(),
            harness_type: None,
            harness_profile_id: None,
            github_preset: None,
            branch: None,
            base_branch: None,
            fetch_origin: true,
            agent: None,
            model: None,
            session_references: Vec::new(),
            attachments: Vec::new(),
        }
    }
}

/// `CreateSessionOptions` as the page builds them. `to_json` is the sparse form Fleet's tests compare; `body` is what is sent.
#[derive(Clone, Debug, PartialEq)]
pub struct CreateOptions {
    /// A `SessionSourceSelection`: `{key: {providerId, sourceType, actionId, contractVersion}, input}`.
    pub source: Value,
    /// `worktree` or `existing`.
    pub isolation_strategy: &'static str,
    pub branch: Option<String>,
    pub title: Option<String>,
    pub initial_prompt: Option<String>,
    pub harness_type: Option<String>,
    pub harness_profile_id: Option<String>,
    pub project_id: Option<String>,
    pub tags: Vec<String>,
    pub agent: Option<String>,
    pub model: Option<(String, String)>,
    pub session_references: Vec<(String, String)>,
    pub attachments: Vec<Image>,
}

impl CreateOptions {
    /// The source's provider (`builtin.repository`, `builtin.github`, …).
    pub fn provider_id(&self) -> Option<&str> {
        self.source.pointer("/key/providerId").and_then(Value::as_str)
    }

    /// Only the fields that are set, under Fleet's names: the object Fleet's tests compare with `toEqual`.
    pub fn to_json(&self) -> Value {
        let mut o = Map::new();
        o.insert("source".into(), self.source.clone());
        o.insert("isolationStrategy".into(), json!(self.isolation_strategy));
        let mut set = |key: &str, value: Option<Value>| {
            if let Some(v) = value {
                o.insert(key.into(), v);
            }
        };
        set("branch", self.branch.as_ref().map(|b| json!(b)));
        set("title", self.title.as_ref().map(|t| json!(t)));
        set("initialPrompt", self.initial_prompt.as_ref().map(|p| json!(p)));
        set("harnessType", self.harness_type.as_ref().map(|h| json!(h)));
        set("harnessProfileId", self.harness_profile_id.as_ref().map(|p| json!(p)));
        set("projectId", self.project_id.as_ref().map(|p| json!(p)));
        set("tags", (!self.tags.is_empty()).then(|| json!(self.tags)));
        set("agent", self.agent.as_ref().map(|a| json!(a)));
        set("model", self.model.as_ref().map(|(p, m)| json!({ "providerID": p, "modelID": m })));
        set(
            "attachments",
            (!self.attachments.is_empty()).then(|| {
                Value::Array(self.attachments.iter().map(|a| json!({ "mime": a.mime, "filename": a.filename, "data": &*a.data })).collect())
            }),
        );
        set(
            "sessionReferences",
            (!self.session_references.is_empty())
                .then(|| Value::Array(self.session_references.iter().map(|(t, s)| json!({ "token": t, "sessionId": s })).collect())),
        );
        Value::Object(o)
    }
}

/// `{directory, options}`: what `createSession(directory, options)` takes.
#[derive(Clone, Debug, PartialEq)]
pub struct NewSessionRequest {
    /// `None` for a quick chat.
    pub directory: Option<String>,
    pub options: CreateOptions,
}

impl NewSessionRequest {
    /// The body of `POST /api/sessions` (`CreateSessionApiRequest`, A20.1): the first group is always there, with `null`
    /// for what isn't set; `agent`, `model`, `sessionReferences` and `attachments` are spread only when set.
    pub fn body(&self) -> Value {
        let o = &self.options;
        let null_or = |v: &Option<String>| v.as_ref().map_or(Value::Null, |s| json!(s));
        let mut body = json!({
            "directory": null_or(&self.directory),
            "title": null_or(&o.title),
            "isolationStrategy": o.isolation_strategy,
            "branch": null_or(&o.branch),
            "source": o.source,
            "harnessType": null_or(&o.harness_type),
            "harnessProfileId": null_or(&o.harness_profile_id),
            "initialPrompt": null_or(&o.initial_prompt),
            "onComplete": null,
            "projectId": null_or(&o.project_id),
            "tags": if o.tags.is_empty() { Value::Null } else { json!(o.tags) },
        });
        let sparse = o.to_json();
        for key in ["agent", "model", "sessionReferences", "attachments"] {
            if let Some(v) = sparse.get(key) {
                body[key] = v.clone();
            }
        }
        body
    }
}

/// The words of a refused or failed build: shown in the red box, and nothing is sent.
pub type BuildResult = Result<NewSessionRequest, String>;

const MAX_SLUG_LENGTH: usize = 40;
const MAX_TITLE_LENGTH: usize = 60;

const STOP_WORDS: &[&str] = &[
    "a", "an", "the", "and", "or", "but", "so", "to", "of", "in", "on", "at", "for", "with", "from", "by", "into", "about", "as", "is",
    "are", "be", "it", "its", "this", "that", "these", "those", "i", "me", "my", "we", "us", "our", "you", "your", "please", "can",
    "could", "would", "should", "will", "lets", "let", "just", "some", "do", "does",
];

/// `text` as JavaScript's `normalize("NFKD")` + dropping the combining marks would leave it, for the letters a
/// message is likely to hold: Latin-1 and Latin Extended-A letters lose their accents, full-width letters become plain
/// ones, and the `ﬁ`-style ligatures spell out. Anything else is left as it is (it becomes a word break later).
/// Rust's standard library has no normalisation tables and `unicode-normalization` would be a new crate; a letter this
/// misses is a word break, which for a branch-name preview is the harmless side to fail on. Letters that don't
/// decompose (`ß æ ø đ ł`) are word breaks in Fleet too.
fn fold_letter(c: char, out: &mut String) {
    match c {
        '\u{0300}'..='\u{036f}' => {}
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => out.push('a'),
        'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => out.push('c'),
        'ď' => out.push('d'),
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => out.push('e'),
        'ĝ' | 'ğ' | 'ġ' | 'ģ' => out.push('g'),
        'ĥ' => out.push('h'),
        'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' => out.push('i'),
        'ĳ' => out.push_str("ij"),
        'ĵ' => out.push('j'),
        'ķ' => out.push('k'),
        'ĺ' | 'ļ' | 'ľ' => out.push('l'),
        'ñ' | 'ń' | 'ņ' | 'ň' => out.push('n'),
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ō' | 'ŏ' | 'ő' => out.push('o'),
        'ŕ' | 'ŗ' | 'ř' => out.push('r'),
        'ś' | 'ŝ' | 'ş' | 'š' | 'ſ' => out.push('s'),
        'ţ' | 'ť' => out.push('t'),
        'ù' | 'ú' | 'û' | 'ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => out.push('u'),
        'ŵ' => out.push('w'),
        'ý' | 'ÿ' | 'ŷ' => out.push('y'),
        'ź' | 'ż' | 'ž' => out.push('z'),
        'ﬀ' => out.push_str("ff"),
        'ﬁ' => out.push_str("fi"),
        'ﬂ' => out.push_str("fl"),
        'ﬃ' => out.push_str("ffi"),
        'ﬄ' => out.push_str("ffl"),
        // Full-width ASCII (`！` to `～`) folds to the ASCII it stands for.
        '\u{ff01}'..='\u{ff5e}' => out.push(char::from_u32(c as u32 - 0xfee0).unwrap_or(' ')),
        c => out.push(c),
    }
}

fn slug_words(text: &str) -> Vec<String> {
    let mut folded = String::with_capacity(text.len());
    for c in text.chars().flat_map(char::to_lowercase) {
        fold_letter(c, &mut folded);
    }
    folded
        .replace(['\'', '\u{2019}'], "")
        .split(|c: char| !(c.is_ascii_lowercase() || c.is_ascii_digit()))
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

/// The first line of `text` that has something on it (JavaScript's `trim`, which is Unicode white space).
fn first_line(text: &str) -> &str {
    text.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l)).find(|l| !l.trim().is_empty()).unwrap_or("")
}

/// A branch-name slug for a message: its first line, lowercase words joined by hyphens, filler words dropped, cut at a
/// word boundary to 40 characters. Empty when nothing usable is left (`slugForBranch`).
pub fn slug_for_branch(text: &str) -> String {
    let words = slug_words(first_line(text));
    let meaningful: Vec<&String> = words.iter().filter(|w| !STOP_WORDS.contains(&w.as_str())).collect();
    let chosen: Vec<&String> = if meaningful.is_empty() { words.iter().collect() } else { meaningful };

    let mut slug = String::new();
    for word in chosen {
        let next = if slug.is_empty() { word.clone() } else { format!("{slug}-{word}") };
        if next.len() > MAX_SLUG_LENGTH {
            // A single word longer than the limit is cut; otherwise stop at the last whole word.
            return if slug.is_empty() { word[..MAX_SLUG_LENGTH].to_string() } else { slug };
        }
        slug = next;
    }
    slug
}

/// A session title for a message: its first line, spaces tidied, cut at a word boundary to 60 characters
/// (`titleFromMessage`). Fleet keeps the title it's given, so this is what the session is called until someone renames it.
pub fn title_from_message(text: &str) -> String {
    let line = first_line(text).split_whitespace().collect::<Vec<_>>().join(" ");
    // JavaScript counts UTF-16 units; counting characters differs only for emoji and the like, and only by cutting
    // a little later, never mid-character.
    let chars: Vec<char> = line.chars().collect();
    if chars.len() <= MAX_TITLE_LENGTH {
        return line;
    }
    let cut = &chars[..MAX_TITLE_LENGTH - 1];
    let kept = match cut.iter().rposition(|&c| c == ' ') {
        Some(space) if space >= MAX_TITLE_LENGTH / 2 => &cut[..space],
        _ => cut,
    };
    let kept: String = kept.iter().collect();
    format!("{}…", kept.trim_end())
}

/// A branch someone chose for this worktree: typed here, or suggested by the GitHub source. When neither did, there
/// is none and the request carries no branch: the server names the worktree from the message and the naming templates,
/// so automations and API callers name it the same way (`resolveNewWorktreeBranch`).
pub fn resolve_new_worktree_branch(branch: Option<&str>, preset: Option<&GitHubPreset>) -> Option<String> {
    let typed = branch.map(str::trim).filter(|b| !b.is_empty());
    let suggested = preset.and_then(|p| p.suggested_branch.as_deref()).map(str::trim).filter(|b| !b.is_empty());
    typed.or(suggested).map(str::to_string)
}

fn source_key(provider: &str, source_type: &str) -> Value {
    json!({ "providerId": provider, "sourceType": source_type, "actionId": "start-session", "contractVersion": 1 })
}

/// The base fields for a new worktree: only what differs from the server's defaults.
fn worktree_base(state: &NewSessionState) -> Map<String, Value> {
    let mut base = Map::new();
    if let Some(b) = state.base_branch.as_ref().filter(|b| !b.is_empty()) {
        base.insert("baseBranch".into(), json!(b));
    }
    if !state.fetch_origin {
        base.insert("fetchOrigin".into(), json!(false));
    }
    base
}

fn repository_source(path: &str, workspace: &Workspace, branch: Option<&str>, base: Map<String, Value>) -> Value {
    let mut input = Map::new();
    input.insert("repositoryPath".into(), json!(path));
    input.insert("isolationStrategy".into(), json!(if *workspace == Workspace::Current { "existing" } else { "worktree" }));
    match workspace {
        Workspace::Existing(worktree) => {
            input.insert("existingWorktreePath".into(), json!(worktree));
        }
        Workspace::New => {
            if let Some(b) = branch {
                input.insert("branch".into(), json!(b));
            }
            input.extend(base);
        }
        Workspace::Current => {}
    }
    json!({ "key": source_key("builtin.repository", "repository"), "input": input })
}

fn directory_source(path: &str) -> Value {
    json!({ "key": source_key("builtin.local", "directory"), "input": { "directory": path, "isolationStrategy": "existing" } })
}

fn github_source(
    preset: &GitHubPreset,
    repository: &str,
    isolation: &str,
    branch: Option<&str>,
    existing_worktree: Option<&str>,
    base: Map<String, Value>,
) -> Value {
    let mut input = Map::new();
    input.insert("owner".into(), json!(preset.owner));
    input.insert("repo".into(), json!(preset.repo));
    input.insert("number".into(), json!(preset.number));
    input.insert("repositoryPath".into(), json!(repository));
    input.insert("isolationStrategy".into(), json!(isolation));
    // A worktree that exists has its branch already, and nothing to base: only a new one takes either.
    match (existing_worktree, branch) {
        (Some(path), _) => {
            input.insert("existingWorktreePath".into(), json!(path));
        }
        (None, branch) => {
            if let Some(b) = branch {
                input.insert("branch".into(), json!(b));
            }
            input.extend(base);
        }
    }
    json!({ "key": source_key("builtin.github", &preset.source_type), "input": input })
}

/// Turns the page's choices into the arguments for `createSession` (`buildCreateSessionRequest`). `Err` is the words
/// for the red box.
pub fn build_create_session_request(state: &NewSessionState) -> BuildResult {
    let message = state.message.trim();
    let preset = state.github_preset.as_ref();

    if let Some(p) = preset
        && !matches!(state.folder, Folder::Repository(_))
    {
        return Err(format!("Pick the repository for {} #{}.", p.repo_full_name, p.number));
    }
    if let Folder::Repository(path) | Folder::Directory(path) = &state.folder
        && path.trim().is_empty()
    {
        return Err("Pick a folder.".to_string());
    }

    let is_repository = matches!(state.folder, Folder::Repository(_));
    let is_worktree = is_repository && state.workspace != Workspace::Current;
    let is_new_worktree = is_repository && state.workspace == Workspace::New;
    let branch = if is_new_worktree { resolve_new_worktree_branch(state.branch.as_deref(), preset) } else { None };
    let base = if is_new_worktree { worktree_base(state) } else { Map::new() };

    let (directory, source) = match &state.folder {
        Folder::None => (
            None,
            json!({ "key": {
            "providerId": "builtin.quickchat", "sourceType": "quick-chat", "actionId": "start-session", "contractVersion": 1
        }, "input": {} }),
        ),
        Folder::Directory(path) => {
            let path = path.trim();
            (Some(path.to_string()), directory_source(path))
        }
        Folder::Repository(path) => {
            let source = match preset {
                Some(p) => github_source(
                    p,
                    path,
                    if is_worktree { "worktree" } else { "existing" },
                    branch.as_deref(),
                    match &state.workspace {
                        Workspace::Existing(worktree) => Some(worktree.as_str()),
                        _ => None,
                    },
                    base,
                ),
                None => repository_source(path, &state.workspace, branch.as_deref(), base),
            };
            (Some(path.clone()), source)
        }
    };

    let tags: Vec<String> = state.tags.iter().map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect();
    let typed_title = state.title.as_deref().map(str::trim).filter(|t| !t.is_empty());
    let preset_title = preset.map(|p| p.title.trim()).filter(|t| !t.is_empty());
    let title = typed_title
        .map(str::to_string)
        .or(preset_title.map(str::to_string))
        .or(Some(title_from_message(message)).filter(|t| !t.is_empty()));
    let some = |v: &Option<String>| v.clone().filter(|s| !s.is_empty());

    Ok(NewSessionRequest {
        directory,
        options: CreateOptions {
            source,
            isolation_strategy: if is_worktree { "worktree" } else { "existing" },
            branch,
            title,
            initial_prompt: Some(message.to_string()).filter(|m| !m.is_empty()),
            harness_type: some(&state.harness_type),
            harness_profile_id: some(&state.harness_profile_id),
            project_id: some(&state.project_id),
            tags,
            agent: some(&state.agent),
            model: state.model.clone(),
            session_references: if message.is_empty() { Vec::new() } else { state.session_references.clone() },
            attachments: if message.is_empty() { Vec::new() } else { state.attachments.clone() },
        },
    })
}

/// What `POST /api/sessions` answers (`CreateSessionResponse`), as far as the page reads it.
#[derive(Clone, Debug, PartialEq)]
pub struct Created {
    pub instance_id: String,
    pub workspace_id: String,
    /// The `session` object whole (id, title, time, tags), which goes into the row as it came.
    pub session: Value,
    pub branch: Option<String>,
    pub harness_type: Option<String>,
}

impl Created {
    /// `None` when the answer lacks a session id: nothing to open.
    pub fn from_wire(answer: &Value) -> Option<Created> {
        let text = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
        let session = answer.get("session")?.clone();
        session.get("id").and_then(Value::as_str)?;
        Some(Created {
            instance_id: text(answer, "instanceId")?,
            workspace_id: text(answer, "workspaceId").unwrap_or_default(),
            session,
            branch: text(answer, "branch"),
            harness_type: text(answer, "harnessType"),
        })
    }

    pub fn session_id(&self) -> &str {
        self.session.get("id").and_then(Value::as_str).unwrap_or_default()
    }
}

/// The list row for a session the page just created, before the session list has it, in the shape of a
/// `GET /api/sessions` item so the list's own parser reads it (`buildCreatedSessionRow`). It lands where the draft row
/// was: the chosen project, or Scratch (`project` is its id and name).
pub fn build_created_session_row(created: &Created, request: &NewSessionRequest, project: Option<(&str, &str)>) -> Value {
    let o = &request.options;
    let is_working = o.initial_prompt.is_some() || o.provider_id() == Some("builtin.github");
    let directory = request.directory.as_deref();
    json!({
        "instanceId": created.instance_id,
        "workspaceId": created.workspace_id,
        "workspaceDirectory": directory.unwrap_or(""),
        "workspaceDisplayName": null,
        "isolationStrategy": o.isolation_strategy,
        "sessionStatus": if is_working { "active" } else { "idle" },
        "session": created.session,
        "instanceStatus": "running",
        "parentSessionId": null,
        "sourceDirectory": directory,
        // The server names a new worktree, so its answer is the branch, not the request.
        "branch": created.branch.as_ref().or(o.branch.as_ref()),
        "activityStatus": if is_working { "busy" } else { "idle" },
        "lifecycleStatus": "running",
        "retentionStatus": "active",
        "archivedAt": null,
        "typedInstanceStatus": "running",
        "isHidden": false,
        "projectId": project.map(|(id, _)| id),
        "projectName": project.map(|(_, name)| name),
        "harnessType": created.harness_type,
        "tags": created.session.get("tags").cloned().unwrap_or_else(|| json!([])),
        "selectedAgent": o.agent,
        "selectedModel": o.model.as_ref().map(|(p, m)| json!({ "providerID": p, "modelID": m })),
    })
}
