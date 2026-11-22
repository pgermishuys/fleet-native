//! The right panel's tab model: which tabs each session has, in what order, which is active, and whether Changes is open to be
//! reviewed. Plain data and functions with no view (component 10, piece A). Ports `stores/canvases.ts` (everything but the
//! server canvases, file tabs and mod panes' bodies), `lib/canvas-registry.ts` (kinds, labels, icons, titles) and
//! `lib/panel-content.ts`.
//!
//! Widen is kept in `RightPanel::widened` (6Q) and saved in the settings file, not here: it is one flag for every session.
use crate::icons;
use rune::Icon;
use std::collections::HashMap;

/// What a tab shows. The order is the registry's (`canvasTypes`), which is also the order the kinds are listed in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CanvasKind {
    Changes,
    Files,
    Context,
    Progress,
    Turns,
    Agents,
    Visual,
    Browser,
    Page,
    File,
    Mod,
}

/// A kind nobody registered (`canvasType` throws a `TypeError` for it).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnknownKind(pub String);

impl CanvasKind {
    pub const ALL: [CanvasKind; 11] = [
        Self::Changes,
        Self::Files,
        Self::Context,
        Self::Progress,
        Self::Turns,
        Self::Agents,
        Self::Visual,
        Self::Browser,
        Self::Page,
        Self::File,
        Self::Mod,
    ];

    /// The kind's id, which is also the id of its tab when it is a built-in.
    pub fn slug(self) -> &'static str {
        match self {
            Self::Changes => "changes",
            Self::Files => "files",
            Self::Context => "context",
            Self::Progress => "progress",
            Self::Turns => "turns",
            Self::Agents => "agents",
            Self::Visual => "visual",
            Self::Browser => "browser",
            Self::Page => "page",
            Self::File => "file",
            Self::Mod => "mod",
        }
    }

    pub fn parse(slug: &str) -> Result<Self, UnknownKind> {
        Self::ALL.into_iter().find(|k| k.slug() == slug).ok_or_else(|| UnknownKind(slug.to_string()))
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Changes => "Changes",
            Self::Files => "Files",
            Self::Context => "Context",
            Self::Progress => "Progress",
            Self::Turns => "Turns",
            Self::Agents => "Agents",
            Self::Visual => "Document",
            Self::Browser => "Browser",
            Self::Page => "Page",
            Self::File => "File",
            Self::Mod => "Mod",
        }
    }

    pub fn icon(self) -> &'static Icon {
        match self {
            Self::Changes => &icons::GIT_COMPARE,
            Self::Files => &icons::FOLDER_TREE,
            Self::Context => &icons::PAPERCLIP,
            Self::Progress => &icons::LIST_CHECKS,
            Self::Turns => &icons::HISTORY,
            Self::Agents => &icons::NETWORK,
            Self::Visual => &icons::FILE_TEXT,
            Self::Browser => &icons::GLOBE,
            Self::Page => &icons::PANELS_TOP_LEFT,
            Self::File => &icons::FILE,
            Self::Mod => &icons::PUZZLE,
        }
    }

    /// One tab per session that opens from the + menu or on its own (`BuiltInCanvasKind`).
    pub fn is_built_in(self) -> bool {
        !matches!(self, Self::Visual | Self::Browser | Self::Page | Self::File | Self::Mod)
    }
}

/// The built-in tabs the + menu offers, in its order (`PICKABLE_CANVAS_KINDS`).
pub const PICKABLE: [CanvasKind; 6] =
    [CanvasKind::Context, CanvasKind::Progress, CanvasKind::Changes, CanvasKind::Files, CanvasKind::Turns, CanvasKind::Agents];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VisualType {
    Markdown,
    Html,
}

/// A document the agent rendered (`VisualPayload`); component 14 fills the content.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VisualPayload {
    pub kind: VisualType,
    pub title: Option<String>,
    pub source_path: Option<String>,
    pub content: String,
}

/// One open tab (`CanvasInstance`). What a kind needs beyond its id is in the field for it; the server's part comes with
/// piece C and the file tab's view with component 11.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanvasInstance {
    pub id: String,
    pub kind: CanvasKind,
    pub payload: Option<VisualPayload>,
    /// The title of a browser, page or mod pane tab, already without terminal codes.
    pub title: Option<String>,
    /// A file tab's path, whether it is a preview and how it shows the file.
    pub file: Option<FileTab>,
    /// A browser or page tab's canvas on the server: what 14's body reads.
    pub server: Option<ServerTab>,
    /// A mod pane tab's mod and pane (`CanvasInstance.modPane`); its title is in `title`.
    pub mod_pane: Option<ModPaneRef>,
}

/// Which pane of which mod a `mod` tab draws.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModPaneRef {
    pub mod_name: String,
    pub pane_id: String,
}

/// A pane id belongs to its mod: two mods may use the same one (`modPaneCanvasId`).
pub fn mod_pane_canvas_id(mod_name: &str, pane_id: &str) -> String {
    format!("mod:{mod_name}:{pane_id}")
}

/// How a file tab shows its file (`FileView`). Markdown and HTML also have Rendered, and their Edit is called Source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileView {
    Rendered,
    Edit,
    Diff,
}

/// A file tab's own part (`FileTab`). A preview tab (italic) is replaced by the next file opened with a single click unless
/// it has unsaved changes; a kept (pinned) tab isn't.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileTab {
    pub path: String,
    pub preview: bool,
    pub view: FileView,
}

/// What a server canvas's tab keeps of it: the id the server knows it by, its version (an older one never replaces a newer
/// one) and its state, which the body reads (`CanvasInstance.server`, `.browser`, `.page`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerTab {
    pub canvas_id: String,
    pub version: i64,
    pub state: serde_json::Value,
}

/// A canvas the server keeps for a session (`ServerCanvasSnapshot`). Only a browser or a page is shown.
#[derive(Clone, Debug, PartialEq)]
pub struct ServerCanvas {
    pub id: String,
    pub kind: CanvasKind,
    pub title: String,
    pub version: i64,
    pub state: serde_json::Value,
}

/// `GET .../canvases` items and `canvas.updated` payloads share these fields. A kind this client doesn't show (the retired
/// `diagram` and `sequence`, an unknown one) is `None`, so it is left out of the list and of events (`isServerCanvasKind`).
pub fn read_server_canvas(item: &serde_json::Value) -> Option<ServerCanvas> {
    let kind = match item.get("kind").and_then(serde_json::Value::as_str)? {
        "browser" => CanvasKind::Browser,
        "page" => CanvasKind::Page,
        _ => return None,
    };
    let id = item.get("canvasId").and_then(serde_json::Value::as_str).filter(|s| !s.is_empty())?;
    Some(ServerCanvas {
        id: id.to_string(),
        kind,
        title: item.get("title").and_then(serde_json::Value::as_str).unwrap_or_default().to_string(),
        version: item.get("version").and_then(serde_json::Value::as_i64).unwrap_or(0),
        state: item.get("state").cloned().unwrap_or(serde_json::Value::Null),
    })
}

/// The tab's id (`serverCanvasTabId`).
pub fn server_tab_id(canvas_id: &str) -> String {
    format!("canvas:{canvas_id}")
}

impl ServerCanvas {
    fn instance(&self) -> CanvasInstance {
        CanvasInstance {
            id: server_tab_id(&self.id),
            kind: self.kind,
            payload: None,
            title: Some(plain_terminal_text(&self.title)),
            file: None,
            server: Some(ServerTab { canvas_id: self.id.clone(), version: self.version, state: self.state.clone() }),
            mod_pane: None,
        }
    }
}

/// A server canvas event on a session's topic (`CanvasEvent`).
#[derive(Clone, Debug, PartialEq)]
pub enum CanvasEvent {
    Updated(ServerCanvas),
    Focused(String),
    Closed(String),
}

impl CanvasInstance {
    pub fn built_in(kind: CanvasKind) -> Self {
        Self { id: kind.slug().to_string(), kind, payload: None, title: None, file: None, server: None, mod_pane: None }
    }

    /// Changes is the one tab that can't be closed (`FIXED_CANVAS_IDS`).
    pub fn closable(&self) -> bool {
        self.id != "changes"
    }

    pub fn title(&self) -> String {
        canvas_title(self)
    }

    pub fn icon(&self) -> &'static Icon {
        canvas_icon(self)
    }
}

pub use crate::conv::output::plain_terminal_text;

/// The file name: what follows the last `/`, or the whole path when nothing does (`fileName`).
pub fn file_name(path: &str) -> &str {
    let name = path.rsplit('/').next().unwrap_or(path);
    if name.is_empty() { path } else { name }
}

pub fn file_icon(path: &str) -> &'static Icon {
    let ext = path.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    // Lucide renamed `FileJson` to `FileBraces`; it is the same picture.
    match ext.as_str() {
        "json" | "jsonc" | "json5" => &icons::FILE_BRACES,
        "md" | "markdown" | "mdx" | "txt" | "rst" => &icons::FILE_TEXT,
        "js" | "mjs" | "cjs" | "jsx" | "ts" | "mts" | "cts" | "tsx" | "vue" | "svelte" | "cs" | "fs" | "go" | "rs" | "py" | "rb"
        | "java" | "kt" | "swift" | "c" | "cc" | "cpp" | "h" | "hpp" | "php" | "sh" | "ps1" | "sql" | "css" | "scss" | "less" | "html"
        | "htm" | "xml" | "csproj" | "props" | "targets" | "slnx" | "yml" | "yaml" | "toml" => &icons::FILE_CODE,
        _ => &icons::FILE,
    }
}

/// A visual's title: its own, else the name of the file it came from, else "Document" (`visualCanvasTitle`).
pub fn visual_title(payload: &VisualPayload) -> String {
    if let Some(title) = payload.title.as_deref().map(plain_terminal_text).map(|t| t.trim().to_string()).filter(|t| !t.is_empty()) {
        return title;
    }
    if let Some(path) = payload.source_path.as_deref().map(str::trim).filter(|p| !p.is_empty()) {
        return file_name(path).to_string();
    }
    "Document".to_string()
}

pub fn visual_id(payload: &VisualPayload) -> String {
    format!("visual:{}", visual_title(payload))
}

pub fn file_id(path: &str) -> String {
    format!("file:{path}")
}

fn ends_with_any(path: &str, extensions: &[&str]) -> bool {
    let lower = path.to_ascii_lowercase();
    extensions.iter().any(|e| lower.ends_with(&format!(".{e}")))
}

/// Markdown and HTML open rendered; images are only ever shown; everything else opens in the editor (`hasRenderedView`).
pub fn has_rendered_view(path: &str) -> bool {
    ends_with_any(path, &["md", "markdown", "mdx", "htm", "html"])
}

/// The image types the server's `/files/image` serves (`isImagePath`).
pub fn is_image_path(path: &str) -> bool {
    ends_with_any(path, &["png", "jpg", "jpeg", "gif", "webp", "avif", "bmp", "ico", "svg"])
}

pub fn default_file_view(path: &str) -> FileView {
    if has_rendered_view(path) { FileView::Rendered } else { FileView::Edit }
}

/// What `open_file` asks of the caller.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OpenFile {
    /// Keep the tab instead of opening it as a preview.
    pub keep: bool,
    pub view: Option<FileView>,
}

/// A file tab opened or unpinned: its id, and the path of a preview it replaced, whose buffer the caller drops.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Opened {
    pub id: String,
    pub dropped: Option<String>,
}

/// A tab's title (`canvasTitle`): the file, the browser, page or pane's own title, the visual's, else the kind's label.
pub fn canvas_title(canvas: &CanvasInstance) -> String {
    if let Some(file) = &canvas.file {
        return file_name(&file.path).to_string();
    }
    if let Some(title) = &canvas.title {
        return title.clone();
    }
    canvas.payload.as_ref().map_or_else(|| canvas.kind.label().to_string(), visual_title)
}

pub fn canvas_icon(canvas: &CanvasInstance) -> &'static Icon {
    if let Some(file) = &canvas.file {
        return file_icon(&file.path);
    }
    match canvas.payload.as_ref().map(|p| p.kind) {
        Some(VisualType::Markdown) => &icons::FILE_TEXT,
        Some(VisualType::Html) => &icons::GLOBE,
        None => canvas.kind.icon(),
    }
}

/// One session's tabs (`SessionCanvases`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionCanvases {
    pub canvases: Vec<CanvasInstance>,
    pub active_id: String,
    /// The conversation's visuals, oldest first.
    pub known_visuals: Vec<VisualPayload>,
    /// Built-ins already introduced, so closing one keeps it closed.
    pub introduced: Vec<CanvasKind>,
    /// Changes was opened to be reviewed: the panel makes room. Another tab ends it; dragging the gutter wins over it.
    pub review: bool,
    /// Changes scrolls to this file; `seq` makes asking for one file twice work.
    pub review_focus: Option<ReviewFocus>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewFocus {
    pub path: String,
    pub seq: u32,
}

impl Default for SessionCanvases {
    /// A session never seen starts with Changes and Files, Changes active.
    fn default() -> Self {
        Self {
            canvases: vec![CanvasInstance::built_in(CanvasKind::Changes), CanvasInstance::built_in(CanvasKind::Files)],
            active_id: "changes".to_string(),
            known_visuals: Vec::new(),
            introduced: Vec::new(),
            review: false,
            review_focus: None,
        }
    }
}

impl SessionCanvases {
    pub fn ids(&self) -> Vec<&str> {
        self.canvases.iter().map(|c| c.id.as_str()).collect()
    }

    pub fn active(&self) -> Option<&CanvasInstance> {
        self.canvases.iter().find(|c| c.id == self.active_id)
    }

    /// Changes is the active tab and was opened to be reviewed (`state.review && activeId === "changes"`).
    pub fn reviewing(&self) -> bool {
        self.review && self.active_id == "changes"
    }

    fn mark_introduced(&mut self, kind: CanvasKind) {
        if !self.introduced.contains(&kind) {
            self.introduced.push(kind);
        }
    }

    fn has(&self, id: &str) -> bool {
        self.canvases.iter().any(|c| c.id == id)
    }
}

/// Every session's tabs, and when each tab's content last updated itself.
#[derive(Clone, Debug, Default)]
pub struct Canvases {
    by_session: HashMap<String, SessionCanvases>,
    /// Tab id → the clock when its content last updated itself (a page Fleet reloaded), for the pulse.
    updated_at: HashMap<String, f64>,
    /// Agents running or waiting, for the Agents tab's badge. Piece L fills it from the lineage; until then only scenes do.
    pub agents_running: usize,
    /// The session, active tab and tab count the strip was last scrolled for, and whether a scroll is due (`keep_strip`).
    pub strip_seen: Option<(String, String, usize)>,
    pub strip_due: bool,
    /// The tab ids the strip last saw, so a new one can be told from a reordering (`keep_strip`).
    pub strip_ids: Vec<String>,
    /// The tab that just arrived and the clock it did, for the slide-in.
    pub entering: Option<(String, f64)>,
}

impl Canvases {
    #[cfg(test)]
    pub(crate) fn by_session_for_test(&mut self, session: &str, tabs: SessionCanvases) {
        self.by_session.insert(session.to_string(), tabs);
    }

    /// Sets a session's tabs outright (scenes; piece C's list loads).
    pub fn replace(&mut self, session: &str, tabs: SessionCanvases) {
        self.by_session.insert(session.to_string(), tabs);
    }

    /// The session's tabs; a session never seen has the default pair.
    pub fn session(&self, session: &str) -> SessionCanvases {
        self.by_session.get(session).cloned().unwrap_or_default()
    }

    /// The id of a session's active tab, without copying its tabs.
    pub fn session_active_id(&self, session: &str) -> Option<String> {
        self.by_session.get(session).map(|s| s.active_id.clone())
    }

    fn edit(&mut self, session: &str, change: impl FnOnce(&mut SessionCanvases)) {
        change(self.by_session.entry(session.to_string()).or_default());
    }

    /// Only if the tab exists. Changes is for review when it was reached from another tab, and stays as it was when it is
    /// pressed again; any other tab ends the review (`activate`).
    pub fn activate(&mut self, session: &str, id: &str) {
        self.edit(session, |s| {
            if !s.has(id) {
                return;
            }
            s.review = id == "changes" && (s.active_id != "changes" || s.review);
            s.active_id = id.to_string();
        });
    }

    /// Changes opened to be reviewed, scrolled to one file when a path is given (`openChanges`).
    pub fn open_changes(&mut self, session: &str, path: Option<&str>) {
        self.edit(session, |s| {
            if !s.has("changes") {
                s.canvases.insert(0, CanvasInstance::built_in(CanvasKind::Changes));
            }
            s.active_id = "changes".to_string();
            s.review = true;
            if let Some(path) = path {
                let seq = s.review_focus.as_ref().map_or(0, |f| f.seq) + 1;
                s.review_focus = Some(ReviewFocus { path: path.to_string(), seq });
            }
        });
    }

    /// Opens (or focuses) a built-in at the end of the strip (`open`). Anything else has its own way in and is ignored.
    pub fn open(&mut self, session: &str, kind: CanvasKind) {
        if !kind.is_built_in() {
            return;
        }
        self.edit(session, |s| {
            if !s.has(kind.slug()) {
                s.canvases.push(CanvasInstance::built_in(kind));
            }
            s.active_id = kind.slug().to_string();
            s.review = kind == CanvasKind::Changes;
            s.mark_introduced(kind);
        });
    }

    /// Adds a built-in as the first tab without taking focus, the first time the session has something for it, even if
    /// it was closed later (`introduce`).
    pub fn introduce(&mut self, session: &str, kind: CanvasKind) {
        if !kind.is_built_in() {
            return;
        }
        self.edit(session, |s| {
            if s.introduced.contains(&kind) {
                return;
            }
            if !s.has(kind.slug()) {
                s.canvases.insert(0, CanvasInstance::built_in(kind));
            }
            s.mark_introduced(kind);
        });
    }

    /// Opens (or brings forward) the tab of a mod's pane. A pane id has one tab per mod: opening it again replaces the title, adds
    /// no tab and makes it active (`openModPane`).
    pub fn open_mod_pane(&mut self, session: &str, pane: ModPaneRef, title: &str) -> String {
        let id = mod_pane_canvas_id(&pane.mod_name, &pane.pane_id);
        let tab = CanvasInstance {
            title: Some(plain_terminal_text(title)),
            mod_pane: Some(pane),
            ..CanvasInstance { id: id.clone(), ..CanvasInstance::built_in(CanvasKind::Mod) }
        };
        self.edit(session, |s| {
            match s.canvases.iter_mut().find(|c| c.id == id) {
                Some(open) => *open = tab,
                None => s.canvases.push(tab),
            }
            s.review = false;
            s.active_id = id.clone();
        });
        id
    }

    /// Replaces an open pane's title without opening it or changing the active tab; a pane that isn't open is left alone
    /// (`updateModPane`). True when the tab was there.
    pub fn update_mod_pane(&mut self, session: &str, pane: &ModPaneRef, title: &str) -> bool {
        let id = mod_pane_canvas_id(&pane.mod_name, &pane.pane_id);
        if !self.session(session).canvases.iter().any(|c| c.id == id) {
            return false;
        }
        self.edit(session, |s| {
            if let Some(tab) = s.canvases.iter_mut().find(|c| c.id == id) {
                tab.title = Some(plain_terminal_text(title));
            }
        });
        true
    }

    /// Opens a document as a tab; one with the same title is replaced in place, so a revised diagram takes the old one's
    /// place. Returns the tab's id (`openVisual`).
    pub fn open_visual(&mut self, session: &str, payload: VisualPayload, activate: bool) -> String {
        let id = visual_id(&payload);
        let tab =
            CanvasInstance { payload: Some(payload), ..CanvasInstance { id: id.clone(), ..CanvasInstance::built_in(CanvasKind::Visual) } };
        self.edit(session, |s| {
            match s.canvases.iter_mut().find(|c| c.id == id) {
                Some(open) => *open = tab,
                None => s.canvases.push(tab),
            }
            if activate {
                s.active_id = id.clone();
            }
        });
        id
    }

    /// Opens a file in its own tab, or focuses it when it is open already. Without `keep` it opens as the preview, which takes
    /// the place of the previous preview unless that one `is_dirty` (then that one is kept and the new tab goes at the end).
    /// `dropped` is the path whose buffer is no longer needed (`openFile`).
    pub fn open_file(&mut self, session: &str, path: &str, options: OpenFile, is_dirty: &dyn Fn(&str) -> bool) -> Opened {
        let id = file_id(path);
        let mut dropped = None;
        self.edit(session, |s| {
            if let Some(file) = s.canvases.iter_mut().find(|c| c.id == id).and_then(|c| c.file.as_mut()) {
                file.preview = file.preview && !options.keep;
                file.view = options.view.unwrap_or(file.view);
                s.active_id = id.clone();
                return;
            }
            let tab = CanvasInstance {
                file: Some(FileTab {
                    path: path.to_string(),
                    preview: !options.keep,
                    view: options.view.unwrap_or_else(|| default_file_view(path)),
                }),
                ..CanvasInstance { id: id.clone(), ..CanvasInstance::built_in(CanvasKind::File) }
            };
            let at = s.canvases.iter().position(|c| c.file.as_ref().is_some_and(|f| f.preview));
            if let (false, Some(at)) = (options.keep, at)
                && let Some(old) = s.canvases[at].file.as_ref().map(|f| f.path.clone())
                && !is_dirty(&old)
            {
                dropped = Some(old);
                s.canvases[at] = tab;
            } else {
                // An unsaved preview stays open as a kept tab.
                if !options.keep {
                    for f in s.canvases.iter_mut().filter_map(|c| c.file.as_mut()) {
                        f.preview = false;
                    }
                }
                s.canvases.push(tab);
            }
            s.active_id = id.clone();
        });
        Opened { id, dropped }
    }

    /// Keeps (pins) a preview tab: its pin, typing in it, a double-click on it (`keepFile`).
    pub fn keep_file(&mut self, session: &str, path: &str) {
        self.with_file(session, path, |f| f.preview = false);
    }

    /// Shows a file tab's file another way (`setFileView`).
    pub fn set_file_view(&mut self, session: &str, path: &str, view: FileView) {
        self.with_file(session, path, |f| f.view = view);
    }

    fn with_file(&mut self, session: &str, path: &str, change: impl FnOnce(&mut FileTab)) {
        let id = file_id(path);
        if self.session(session).canvases.iter().any(|c| c.id == id && c.file.is_some()) {
            self.edit(session, |s| {
                if let Some(f) = s.canvases.iter_mut().find(|c| c.id == id).and_then(|c| c.file.as_mut()) {
                    change(f);
                }
            });
        }
    }

    /// Unpins a kept tab: it becomes the preview, and the previous preview closes unless it `is_dirty` (then it is kept). The
    /// active tab stays unless it closed. Unpinning a preview does nothing; the path in `dropped` loses its buffer (`unpinFile`).
    pub fn unpin_file(&mut self, session: &str, path: &str, is_dirty: &dyn Fn(&str) -> bool) -> Option<String> {
        let id = file_id(path);
        if !self.session(session).canvases.iter().any(|c| c.id == id && c.file.as_ref().is_some_and(|f| !f.preview)) {
            return None;
        }
        let mut dropped = None;
        self.edit(session, |s| {
            let old = s
                .canvases
                .iter()
                .find(|c| c.file.as_ref().is_some_and(|f| f.preview))
                .map(|c| (c.id.clone(), c.file.as_ref().unwrap().path.clone()));
            for c in &mut s.canvases {
                if c.id == id
                    && let Some(f) = c.file.as_mut()
                {
                    f.preview = true;
                }
            }
            if let Some((old_id, old_path)) = old {
                if is_dirty(&old_path) {
                    for c in s.canvases.iter_mut().filter(|c| c.id == old_id) {
                        if let Some(f) = c.file.as_mut() {
                            f.preview = false;
                        }
                    }
                } else {
                    s.canvases.retain(|c| c.id != old_id);
                    dropped = Some(old_path);
                }
            }
            if !s.canvases.iter().any(|c| c.id == s.active_id) {
                s.active_id = id.clone();
            }
        });
        dropped
    }

    /// Closes a tab, never Changes. The closed tab's left neighbour becomes active, else the one now in its place, else the
    /// first. Returns the tab so a file tab's buffer or a mod pane's view can be dropped (`close`).
    pub fn close(&mut self, session: &str, id: &str) -> Option<CanvasInstance> {
        if id == "changes" {
            return None;
        }
        let mut closed = None;
        self.edit(session, |s| {
            let Some(at) = s.canvases.iter().position(|c| c.id == id) else { return };
            closed = Some(s.canvases.remove(at));
            if s.active_id == id {
                let next = at.checked_sub(1).and_then(|i| s.canvases.get(i)).or_else(|| s.canvases.get(at)).or_else(|| s.canvases.first());
                if let Some(next) = next {
                    s.active_id = next.id.clone();
                }
            }
        });
        closed
    }

    /// Replaces the session's server canvases with a freshly loaded list, oldest first. Other tabs keep their place; each server
    /// tab already open is redrawn in place or removed when the list lacks it; new ones go at the end. The active tab stays
    /// if it is still there, else the first one is (`setServerCanvases`).
    pub fn set_server_canvases(&mut self, session: &str, list: Vec<ServerCanvas>) {
        let mut loaded: Vec<CanvasInstance> = list.iter().map(ServerCanvas::instance).collect();
        self.edit(session, |s| {
            let mut tabs = Vec::new();
            for tab in std::mem::take(&mut s.canvases) {
                if tab.server.is_none() {
                    tabs.push(tab);
                } else if let Some(at) = loaded.iter().position(|l| l.id == tab.id) {
                    tabs.push(loaded.remove(at));
                }
            }
            tabs.extend(loaded);
            if !tabs.iter().any(|c| c.id == s.active_id)
                && let Some(first) = tabs.first()
            {
                s.active_id = first.id.clone();
            }
            s.canvases = tabs;
        });
    }

    /// `canvas.updated` adds the tab at the end without taking focus, or redraws it in place; `canvas.focused` brings it
    /// forward; `canvas.closed` removes it, and a second close is nothing (`applyCanvasEvent`).
    pub fn apply_canvas_event(&mut self, session: &str, event: CanvasEvent) {
        match event {
            CanvasEvent::Updated(canvas) => {
                let next = canvas.instance();
                self.edit(session, |s| match s.canvases.iter().position(|c| c.id == next.id) {
                    None => s.canvases.push(next),
                    // Events can overtake a list loaded earlier; never go back a version.
                    Some(at) if s.canvases[at].server.as_ref().is_some_and(|k| k.version > canvas.version) => {}
                    Some(at) => s.canvases[at] = next,
                });
            }
            CanvasEvent::Focused(id) => self.activate(session, &server_tab_id(&id)),
            CanvasEvent::Closed(id) => {
                self.close(session, &server_tab_id(&id));
            }
        }
    }

    /// The conversation's visuals, oldest first (`setKnownVisuals`).
    pub fn set_known_visuals(&mut self, session: &str, visuals: Vec<VisualPayload>) {
        self.edit(session, |s| s.known_visuals = visuals);
    }

    /// A tab's content updated itself, at clock `now` (`markUpdated`).
    pub fn mark_updated(&mut self, tab: &str, now: f64) {
        self.updated_at.insert(tab.to_string(), now);
    }

    pub fn updated_at(&self, tab: &str) -> Option<f64> {
        self.updated_at.get(tab).copied()
    }
}

/// What decides whether the panel has something to show (`PanelContentState`).
#[derive(Clone, Copy, Debug, Default)]
pub struct PanelContent<'a> {
    pub changed_files: usize,
    pub canvases: &'a [CanvasInstance],
    pub progress: bool,
    pub context: bool,
    pub agents: bool,
    /// `/diffs` failed: Changes has an error and Try again to show.
    pub changes_failed: bool,
}

/// Anything but Changes and Files is content: a page, browser, visual, file or mod tab (`hasExtraTabs`).
pub fn has_extra_tabs(canvases: &[CanvasInstance]) -> bool {
    canvases.iter().any(|c| !matches!(c.kind, CanvasKind::Changes | CanvasKind::Files))
}

/// An empty session keeps the panel folded; anything the person should see opens it (`panelHasContent`).
pub fn panel_has_content(c: PanelContent) -> bool {
    c.changed_files > 0 || c.changes_failed || has_extra_tabs(c.canvases) || c.progress || c.context || c.agents
}

impl crate::State {
    /// The open session's tabs.
    pub fn open_canvases(&self) -> SessionCanvases {
        self.canvases.session(&self.canvas_session())
    }

    /// Hands the panel's width rules what the model says about review (`state.review && activeId === "changes"`).
    pub fn sync_review(&mut self) {
        self.right.review = self.open_canvases().reviewing();
    }

    /// Runs a change to the open session's tabs, then keeps the panel's review width in step.
    pub fn edit_canvases<R>(&mut self, change: impl FnOnce(&mut Canvases, &str) -> R) -> Option<R> {
        let id = self.canvas_session();
        let out = change(&mut self.canvases, &id);
        self.sync_review();
        Some(out)
    }

    /// Widen was pressed: the other way round from now (`toggleWidened`).
    pub fn toggle_widened(&mut self, tx: &mut rune::Tx) {
        self.set_widened(!self.right.widened, tx);
    }
}
