//! Open in (`OpenToolContextSubmenu.vue`): the rows of the submenu a session's right-click menu opens. Tools come in groups
//! (editors, terminals, file managers); an editor with key files is a submenu of its own. The data and the calls are
//! `fleet::open_in`; `ui::menu` draws the rows and the nested panels.
use crate::fleet::open_in::{Category, Tool, groups};
use crate::icons::{CODE, FILE_TEXT, FOLDER, FOLDER_OPEN, TERMINAL};
use crate::ui::menu::{Entry, Item};
use crate::{State, Tx};
use rune::Icon;

/// The hint on the trigger row.
pub const HINT: &str = "Opens the session's folder in an editor, a terminal or the file manager.";

/// `ToolIcon`'s fallbacks by category; Fleet's own marks for each program are brand logos we don't borrow (docs/PORT.md).
fn glyph(c: Category) -> &'static Icon {
    match c {
        Category::Editor => &CODE,
        Category::Terminal => &TERMINAL,
        Category::Explorer => &FOLDER_OPEN,
    }
}

fn direct(tool: &Tool, directory: &str) -> Entry {
    let (id, dir) = (tool.id.clone(), directory.to_string());
    Entry::Item(
        Item::new(tool.label.clone())
            .id(format!("open-in-{}", tool.id))
            .icon(glyph(tool.category))
            .on(move |s: &mut State, _: &mut Tx| s.open_in_directory(&dir, &id)),
    )
}

/// The submenu's rows for the folder `directory`.
pub fn entries(s: &State, directory: &str) -> Vec<Entry> {
    let o = &s.open_in;
    let tools = o.tools.clone().unwrap_or_default();
    let mut out = Vec::new();
    if tools.is_empty() {
        // Loading until the first answer (Fleet's `isLoading` starts true); an error leaves none, which reads as empty.
        if o.tools.is_none() && o.tools_error.is_none() {
            out.push(Entry::Item(Item::new("Detecting tools\u{2026}").id("open-in-loading").spinner().disabled(true)));
        } else {
            out.push(Entry::Item(Item::new("No tools detected").id("open-in-empty").disabled(true)));
        }
    }
    for (n, (category, items)) in groups(&tools).into_iter().enumerate() {
        if n > 0 {
            out.push(Entry::Separator);
        }
        for tool in items {
            let files = o.files.get(&tool.id).filter(|f| category == Category::Editor && !f.is_empty());
            let Some(files) = files else {
                out.push(direct(&tool, directory));
                continue;
            };
            let (id, dir) = (tool.id.clone(), directory.to_string());
            let mut rows = vec![
                Entry::Item(Item::new("Open directory").id(format!("open-in-{id}-dir")).icon(&FOLDER).on({
                    let (id, dir) = (id.clone(), dir.clone());
                    move |s: &mut State, _: &mut Tx| s.open_in_directory(&dir, &id)
                })),
                Entry::Separator,
            ];
            if o.files_loading {
                rows.push(Entry::Item(Item::new("Loading\u{2026}").id(format!("open-in-{id}-loading")).spinner().disabled(true)));
            }
            for (i, file) in files.iter().enumerate() {
                let (id, dir, f) = (id.clone(), dir.clone(), file.clone());
                rows.push(Entry::Item(
                    Item::new(file.clone())
                        .id(format!("open-in-{}-file-{i}", tool.id))
                        .icon(&FILE_TEXT)
                        .mono()
                        .on(move |s: &mut State, _: &mut Tx| s.open_in_file(&dir, &f, &id)),
                ));
            }
            out.push(Entry::Item(Item::new(tool.label.clone()).id(format!("open-in-{}", tool.id)).icon(glyph(category)).submenu_fit(rows)));
        }
    }
    out
}
