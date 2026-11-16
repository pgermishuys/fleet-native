//! The row menu's Open in submenu (`OpenToolContextSubmenu.vue` and the composables behind it, `use-available-tools.ts`,
//! `use-key-files.ts`, `use-open-directory.ts`, `use-open-file.ts`): which tools this machine can open a folder with, which
//! files each editor offers to open straight away, and the two calls that open a folder or a file. The menu is drawn by
//! `view::open_in`. Every request runs on a thread; with no server (scenes, the invented list) a small invented set of tools
//! answers at once and an open is only written down, so nothing here needs a server or starts a program.
use super::http;
use crate::{State, Tx};
use serde_json::{Value, json};
use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Category {
    Editor,
    Terminal,
    Explorer,
}

/// One tool the machine found (`AvailableTool`). `icon_name` is read for completeness; the menu's icon comes from the category.
#[derive(Clone, PartialEq, Debug)]
pub struct Tool {
    pub id: String,
    pub label: String,
    pub category: Category,
}

/// `GET /api/available-tools`: `{ tools: [...] }` or a bare array. A tool with no id or an unknown category is dropped (Fleet's
/// menu has no group to put it in).
pub fn parse_tools(v: &Value) -> Vec<Tool> {
    let list = v.as_array().or_else(|| v.get("tools").and_then(Value::as_array));
    list.map(|a| {
        a.iter()
            .filter_map(|t| {
                let id = t.get("id")?.as_str()?.to_string();
                let category = match t.get("category")?.as_str()? {
                    "editor" => Category::Editor,
                    "terminal" => Category::Terminal,
                    "explorer" => Category::Explorer,
                    _ => return None,
                };
                let label = t.get("label").and_then(Value::as_str).unwrap_or(&id).to_string();
                Some(Tool { id, label, category })
            })
            .collect()
    })
    .unwrap_or_default()
}

/// `GET /api/key-files`: `{ filesByTool: { <tool>: [relative path, ...] } }`.
pub fn parse_key_files(v: &Value) -> HashMap<String, Vec<String>> {
    v.get("filesByTool")
        .and_then(Value::as_object)
        .map(|o| {
            o.iter()
                .map(|(k, files)| {
                    (
                        k.clone(),
                        files.as_array().map(|a| a.iter().filter_map(|f| f.as_str().map(str::to_string)).collect()).unwrap_or_default(),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

/// `groups`: editors, then terminals, then file explorers; a group with no tool is left out.
pub fn groups(tools: &[Tool]) -> Vec<(Category, Vec<Tool>)> {
    [Category::Editor, Category::Terminal, Category::Explorer]
        .into_iter()
        .filter_map(|c| {
            let items: Vec<Tool> = tools.iter().filter(|t| t.category == c).cloned().collect();
            (!items.is_empty()).then_some((c, items))
        })
        .collect()
}

/// `handleOpenFile`: the folder and the key file joined with a slash, each `//` made one.
pub fn absolute(directory: &str, file: &str) -> String {
    format!("{directory}/{file}").replace("//", "/")
}

pub const KEY_FILES_PATH: &str = "/api/key-files";

/// `GET /api/key-files?directory=<dir>`.
pub fn key_files_path(directory: &str) -> String {
    format!("{KEY_FILES_PATH}?directory={}", crate::conv::image_sniff::encode_component(directory))
}

/// `POST /api/open-directory` body.
pub fn open_directory_body(directory: &str, tool: &str) -> Value {
    json!({ "directory": directory, "tool": tool })
}

/// `POST /api/open-file` body.
pub fn open_file_body(file_path: &str, tool: &str) -> Value {
    json!({ "filePath": file_path, "tool": tool })
}

/// What an open was asked to do (kept only when no server is given, so a test can read it).
#[derive(Clone, PartialEq, Debug)]
pub enum Opened {
    Directory { directory: String, tool: String },
    File { path: String, tool: String },
}

/// The menu's data: the tools the machine offers and the key files of the folder the menu is on.
#[derive(Default)]
pub struct OpenIn {
    /// `None` until an answer came (or one is cached from an earlier look).
    pub tools: Option<Vec<Tool>>,
    pub tools_loading: bool,
    pub tools_error: Option<String>,
    /// The key files for `directory`, by tool id.
    pub files: HashMap<String, Vec<String>>,
    pub files_loading: bool,
    pub directory: String,
    /// Opens asked of the invented machine.
    pub opened: Vec<Opened>,
}

/// The tools the invented machine offers, and the key files its editor shows.
pub fn mock_tools() -> Vec<Tool> {
    let t = |id: &str, label: &str, category| Tool { id: id.into(), label: label.into(), category };
    vec![
        t("code", "Code editor", Category::Editor),
        t("notes", "Notes editor", Category::Editor),
        t("term", "Terminal", Category::Terminal),
        t("files", "File manager", Category::Explorer),
    ]
}

pub fn mock_key_files() -> HashMap<String, Vec<String>> {
    HashMap::from([("code".to_string(), vec!["README.md".to_string(), "docs/notes.md".to_string()])])
}

impl State {
    /// The menu opened on a row of this machine: read the tools again (Fleet does on mount, showing the cached list meanwhile)
    /// and the key files of its folder.
    pub fn open_in_prepare(&mut self, directory: &str, _tx: &mut Tx) {
        self.open_in.directory = directory.to_string();
        // The submenu starts with no key files each time it is mounted.
        self.open_in.files.clear();
        let Some(link) = self.link.clone() else {
            self.open_in.tools = Some(mock_tools());
            self.open_in.files = mock_key_files();
            self.open_in.tools_loading = false;
            self.open_in.files_loading = false;
            return;
        };
        self.open_in.tools_loading = self.open_in.tools.is_none();
        self.open_in.tools_error = None;
        link.off_thread(
            |base| http::get(base, "/api/available-tools"),
            |s, _, result| {
                s.open_in.tools_loading = false;
                match result {
                    Ok(v) => s.open_in.tools = Some(parse_tools(&v)),
                    // An error keeps what was known and says nothing (4.45).
                    Err(why) => s.open_in.tools_error = Some(why),
                }
            },
        );
        if directory.is_empty() {
            return;
        }
        self.open_in.files_loading = true;
        let (path, asked) = (key_files_path(directory), directory.to_string());
        link.off_thread(
            move |base| http::get(base, &path),
            move |s, _, result| {
                // A later menu on another folder owns the answer now.
                if s.open_in.directory != asked {
                    return;
                }
                s.open_in.files_loading = false;
                if let Ok(v) = result {
                    s.open_in.files = parse_key_files(&v);
                }
            },
        );
    }

    /// A terminal, a file manager, or an editor's Open directory (`useOpenDirectory`). A failure shows nothing (4.47).
    pub fn open_in_directory(&mut self, directory: &str, tool: &str) {
        let Some(link) = self.link.clone() else {
            self.open_in.opened.push(Opened::Directory { directory: directory.into(), tool: tool.into() });
            return;
        };
        let body = open_directory_body(directory, tool);
        link.off_thread(move |base| http::post(base, "/api/open-directory", &body), |_, _, _| {});
    }

    /// A key file of an editor (`useOpenFile`).
    pub fn open_in_file(&mut self, directory: &str, file: &str, tool: &str) {
        let path = absolute(directory, file);
        let Some(link) = self.link.clone() else {
            self.open_in.opened.push(Opened::File { path, tool: tool.into() });
            return;
        };
        let body = open_file_body(&path, tool);
        link.off_thread(move |base| http::post(base, "/api/open-file", &body), |_, _, _| {});
    }
}
