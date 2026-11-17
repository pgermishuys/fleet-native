//! Settings → Folders → Worktrees as plain data (`WorktreeNamingSection.vue`, `stores/worktree-naming.ts`): the five fields
//! you edit, the sample the preview names, the token chips, the body that is saved and the preview itself. No Rune widgets and
//! no I/O here but [`load`] and [`save`], which a thread calls. What touches the app is `naming_actions.rs`.
use crate::fleet::http;
use crate::fleet::new_session::feeds::extract_error;
use crate::fleet::new_session::naming::{Context, Name, Templates, resolve_worktree_name};
use rune::TextEdit;
use serde_json::{Value, json};

pub const LOAD_FALLBACK: &str = "Couldn't load worktree naming.";
pub const SAVE_FALLBACK: &str = "Couldn't save worktree naming.";
/// The sentence the preview starts from.
pub const SAMPLE: &str = "PLAT-1841 add rate limiting to the session create endpoint";
/// The id the preview names its session with.
pub const SHORT_ID: &str = "a1b2c3d4";
/// How long "Saved" stays, in seconds.
pub const SAVED_SECS: f64 = 2.0;

/// The five things you can set, in the order the fields are drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Prefix,
    Branch,
    Root,
    Folder,
    Ticket,
}

/// A token chip: the name inserted and the hint after it.
pub struct Token {
    pub name: &'static str,
    pub hint: &'static str,
}

const fn token(name: &'static str, hint: &'static str) -> Token {
    Token { name, hint }
}

impl Field {
    pub const ALL: [Field; 5] = [Field::Prefix, Field::Branch, Field::Root, Field::Folder, Field::Ticket];

    pub fn key(self) -> &'static str {
        match self {
            Field::Prefix => "prefix",
            Field::Branch => "branch",
            Field::Root => "root",
            Field::Folder => "folder",
            Field::Ticket => "ticket",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Field::Prefix => "Branch prefix",
            Field::Branch => "Branch name",
            Field::Root => "Worktree root",
            Field::Folder => "Folder name",
            Field::Ticket => "Capture: ticket",
        }
    }

    /// The chips under the field (`tokensFor`); the prefix and the capture have none.
    pub fn tokens(self) -> &'static [Token] {
        const BRANCH: [Token; 7] = [
            token("slug", "from your message"),
            token("user", "your username"),
            token("prefix", "set above"),
            token("ticket", "captured"),
            token("date", "2026-09-21"),
            token("repo", "repository name"),
            token("shortid", "random"),
        ];
        const ROOT: [Token; 4] = [
            token("repoParent", "beside the repository"),
            token("repo", "repository name"),
            token("home", "your home folder"),
            token("user", "your username"),
        ];
        const FOLDER: [Token; 5] = [
            token("branch", "the resolved branch"),
            token("slug", "from your message"),
            token("ticket", "captured"),
            token("repo", "repository name"),
            token("shortid", "random"),
        ];
        match self {
            Field::Branch => &BRANCH,
            Field::Root => &ROOT,
            Field::Folder => &FOLDER,
            _ => &[],
        }
    }
}

/// The user's own layer as the server holds it (`WorktreeNamingLayerTemplates`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Layer {
    pub branch: Option<String>,
    pub root: Option<String>,
    pub folder: Option<String>,
    pub ticket: Option<String>,
    pub prefix: Option<String>,
}

/// What a read or a save answers: the user's layer and Fleet's defaults.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Loaded {
    pub user: Layer,
    pub defaults: Templates,
}

/// What the Worktrees card keeps while Folders is open.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NamingPage {
    /// The machine key it was read from; leaving the page or moving the switch reads again.
    pub loaded_for: Option<String>,
    pub request: u64,
    pub loading: bool,
    pub saving: bool,
    pub error: Option<String>,
    pub user: Layer,
    pub defaults: Templates,
    pub prefix: TextEdit,
    pub branch: TextEdit,
    pub root: TextEdit,
    pub folder: TextEdit,
    pub ticket: TextEdit,
    pub sample: TextEdit,
    /// When "Saved" ends, on the app's clock; `None` while it isn't shown.
    pub saved_until: Option<f64>,
}

impl Eq for NamingPage {}

fn own(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}

fn read_templates(v: Option<&Value>) -> Templates {
    let d = Templates::default();
    let get = |k: &str, fallback: &str| v.and_then(|v| own(v, k)).unwrap_or_else(|| fallback.to_string());
    Templates {
        branch: get("branch", &d.branch),
        root: get("root", &d.root),
        folder: get("folder", &d.folder),
        capture: Vec::new(),
        prefix: get("prefix", &d.prefix),
    }
}

/// `apply`: `{effective, user, defaults, layers}`; only the last two are kept. A field the server leaves out is Fleet's own.
pub fn parse(status: u16, body: &str, fallback: &str) -> Result<Loaded, String> {
    if !(200..300).contains(&status) {
        return Err(extract_error(body, fallback));
    }
    let v: Value = serde_json::from_str(body).ok().filter(Value::is_object).ok_or_else(|| fallback.to_string())?;
    let user = v.get("user").cloned().unwrap_or(Value::Null);
    let ticket = user.get("capture").and_then(|c| own(c, "ticket"));
    Ok(Loaded {
        user: Layer {
            branch: own(&user, "branch"),
            root: own(&user, "root"),
            folder: own(&user, "folder"),
            ticket,
            prefix: own(&user, "prefix"),
        },
        defaults: read_templates(v.get("defaults")),
    })
}

/// `store.load()` with no repository: the user's own layer.
pub fn load(base: &str) -> Result<Loaded, String> {
    match http::worktree_naming(base, None) {
        Ok((status, body)) => parse(status, &body, LOAD_FALLBACK),
        Err(_) => Err(LOAD_FALLBACK.to_string()),
    }
}

/// `store.save()`: the body goes to the machine; a template the server can't name a worktree from is refused in its words.
pub fn save(base: &str, body: &Value) -> Result<Loaded, String> {
    match http::put_worktree_naming(base, body) {
        Ok((status, text)) => parse(status, &text, SAVE_FALLBACK),
        Err(_) => Err(SAVE_FALLBACK.to_string()),
    }
}

impl NamingPage {
    pub fn edit(&self, field: Field) -> &TextEdit {
        match field {
            Field::Prefix => &self.prefix,
            Field::Branch => &self.branch,
            Field::Root => &self.root,
            Field::Folder => &self.folder,
            Field::Ticket => &self.ticket,
        }
    }

    pub fn edit_mut(&mut self, field: Field) -> &mut TextEdit {
        match field {
            Field::Prefix => &mut self.prefix,
            Field::Branch => &mut self.branch,
            Field::Root => &mut self.root,
            Field::Folder => &mut self.folder,
            Field::Ticket => &mut self.ticket,
        }
    }

    /// `syncDraft`: the fields hold the user's own layer, and an empty field means "use the default".
    pub fn sync_draft(&mut self) {
        let u = self.user.clone();
        self.prefix.replace_text(u.prefix.unwrap_or_default());
        self.branch.replace_text(u.branch.unwrap_or_default());
        self.root.replace_text(u.root.unwrap_or_default());
        self.folder.replace_text(u.folder.unwrap_or_default());
        self.ticket.replace_text(u.ticket.unwrap_or_default());
    }

    /// An answer replaced the layers: they are kept and the fields follow.
    pub fn adopt(&mut self, got: Loaded) {
        self.user = got.user;
        self.defaults = got.defaults;
        self.sync_draft();
    }

    /// A chip appends `{name}` to the end of its field.
    pub fn insert_token(&mut self, field: Field, name: &str) {
        let edit = self.edit_mut(field);
        let text = format!("{}{{{name}}}", edit.text);
        edit.replace_text(text);
    }

    /// "Use Fleet's defaults" empties every field before it saves.
    pub fn clear_draft(&mut self) {
        for f in Field::ALL {
            self.edit_mut(f).clear();
        }
    }

    /// What is saved: an empty field goes as `null`, the ticket pattern inside `capture`.
    pub fn save_body(&self) -> Value {
        let some = |e: &TextEdit| if e.text.is_empty() { Value::Null } else { Value::String(e.text.clone()) };
        let capture = if self.ticket.text.is_empty() { Value::Null } else { json!({ "ticket": self.ticket.text }) };
        json!({ "branch": some(&self.branch), "root": some(&self.root), "folder": some(&self.folder), "capture": capture, "prefix": some(&self.prefix) })
    }

    /// The templates the preview names with: the draft, with Fleet's defaults where a field is empty.
    pub fn preview_templates(&self) -> Templates {
        let pick = |e: &TextEdit, d: &str| if e.text.is_empty() { d.to_string() } else { e.text.clone() };
        let d = &self.defaults;
        Templates {
            branch: pick(&self.branch, &d.branch),
            root: pick(&self.root, &d.root),
            folder: pick(&self.folder, &d.folder),
            capture: if self.ticket.text.is_empty() { Vec::new() } else { vec![("ticket".into(), self.ticket.text.clone())] },
            prefix: pick(&self.prefix, &d.prefix),
        }
    }

    /// The preview's name, for an invented repository on an invented computer. `today` is `2026-09-21`.
    pub fn preview(&self, today: &str) -> Preview {
        let templates = self.preview_templates();
        let context = Context {
            repository_path: "/home/you/source/lantern".into(),
            user: "you".into(),
            date: today.into(),
            short_id: SHORT_ID.into(),
            home: "/home/you".into(),
        };
        let name = resolve_worktree_name(&templates, &context, &self.sample.text, None);
        let session_id_stands_in =
            templates.branch.contains("{slug}") && name.branch.as_deref().is_some_and(|b| b.contains(&format!("session-{SHORT_ID}")));
        Preview { name, session_id_stands_in }
    }
}

/// What the "What you'd get" box shows.
pub struct Preview {
    pub name: Name,
    /// The sample gave no slug, so the session id stands in for it (the note under the preview).
    pub session_id_stands_in: bool,
}

impl Preview {
    pub fn branch_line(&self) -> String {
        self.name.branch.clone().unwrap_or_else(|| format!("weave-session-{SHORT_ID} (named by the server)"))
    }

    pub fn worktree_line(&self) -> String {
        let folder = if self.name.folder.is_empty() { format!("weave-session-{SHORT_ID}") } else { self.name.folder.clone() };
        format!("{}/{folder}", self.name.root)
    }
}
