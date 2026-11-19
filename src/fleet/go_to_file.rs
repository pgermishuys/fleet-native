//! Go to file's store (weave-fleet `stores/go-to-file.ts`): which session's picker is open, and which file should take the
//! keyboard once it is drawn. A file opened from a message's link takes it the same way, at the line the message named.

use super::canvases::OpenFile;
use super::file_buffers::Status;
use super::finder::{FileSearch, Scope, Step, failure, parse_files};
use super::http;
use crate::State;
use rune::Tx;
use std::time::Duration;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GoToFile {
    /// The line a file was opened at, marked as the active line: the code view has no caret until editing (slice 2).
    pub shown_line: Option<(String, String, u32)>,
    /// The session whose picker is open.
    pub session: Option<String>,
    /// The file to give the keyboard to when its view opens, with the line to show.
    pub focus_on_open: Option<FocusOnOpen>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FocusOnOpen {
    pub session: String,
    pub path: String,
    pub line: Option<u32>,
}

impl GoToFile {
    pub fn show(&mut self, session: &str) {
        self.session = Some(session.to_string());
    }

    pub fn hide(&mut self) {
        self.session = None;
    }

    pub fn ask_focus(&mut self, session: &str, path: &str, line: Option<u32>) {
        self.focus_on_open = Some(FocusOnOpen { session: session.to_string(), path: path.to_string(), line });
    }

    /// The file's view calls this when it is drawn: the request once, for that session and path only, else `None`. The inner
    /// option is the line (`takeFocus`).
    pub fn take_focus(&mut self, session: &str, path: &str) -> Option<Option<u32>> {
        let pending = self.focus_on_open.as_ref()?;
        if pending.session != session || pending.path != path {
            return None;
        }
        self.focus_on_open.take().map(|p| p.line)
    }
}

/// What the dialog holds while it is open: the box, the active row, and the file-name search it asks the machine
/// (`GoToFileDialog.vue`'s `query`, `active` and `useFindFiles`). Its own search, so the `@` list and the Files tab never answer it.
#[derive(Debug, Default)]
pub struct GoToDialog {
    pub query: rune::TextEdit,
    pub active: usize,
    pub search: FileSearch,
    /// The rows last listed: a different list puts the active row back on the first (`watch(items)`).
    listed: Vec<String>,
}

impl GoToDialog {
    /// The files to list: what is open when nothing is typed, else the search's files, folders (they end in "/") left out.
    pub fn items(&self, open_files: Vec<String>) -> Vec<String> {
        let typed = self.query.text.trim();
        if typed.is_empty() {
            return open_files;
        }
        self.search.files(typed).into_iter().filter(|p| !p.ends_with('/')).collect()
    }

    /// The query the search follows: none while nothing is typed or the dialog is shut.
    pub fn wanted(&self, shown: bool) -> Option<&str> {
        (shown && !self.query.text.trim().is_empty()).then_some(self.query.text.as_str())
    }

    /// ↓ and ↑ move the active row and wrap at both ends (`move`).
    pub fn step(&mut self, by: isize, len: usize) {
        if len > 0 {
            self.active = (self.active as isize + by).rem_euclid(len as isize) as usize;
        }
    }

    /// A new list puts the active row first. `true` when it did.
    fn follow(&mut self, items: &[String]) -> bool {
        if self.listed == items {
            return false;
        }
        self.listed = items.to_vec();
        self.active = 0;
        true
    }
}

/// The folder of a path: everything before its last "/" (`folderOf`).
pub fn folder_of(path: &str) -> &str {
    path.rfind('/').map_or("", |i| &path[..i])
}

impl State {
    /// The files open in the session's tabs, in tab order.
    pub fn go_to_open_files(&self, session: &str) -> Vec<String> {
        self.canvases.session(session).canvases.iter().filter_map(|c| c.file.as_ref().map(|f| f.path.clone())).collect()
    }

    /// The rows the dialog lists now.
    pub fn go_to_items(&self) -> Vec<String> {
        let session = self.go_to_file.session.as_deref().unwrap_or_default();
        self.go_to.items(self.go_to_open_files(session))
    }

    /// The command and Ctrl/Cmd+P (`go-to-file`): the dialog for the open session, empty, the first row active, the box focused.
    pub fn go_to_file_open(&mut self, tx: &mut Tx) {
        let session = self.canvas_session();
        self.go_to_file.show(&session);
        self.go_to = GoToDialog::default();
        self.ui.open = Some(crate::ui::Open::GoToFile);
        tx.focus(crate::view::go_to_file::INPUT, false);
    }

    /// Esc, a click outside, or a file picked.
    pub fn go_to_file_close(&mut self) {
        self.go_to_file.hide();
        if self.ui.open == Some(crate::ui::Open::GoToFile) {
            self.ui.open = None;
        }
    }

    /// `pick`: the file opens as a kept tab and takes the keyboard, the right panel shows, the dialog closes.
    pub fn go_to_pick(&mut self, path: &str, tx: &mut Tx) {
        let Some(session) = self.go_to_file.session.clone() else { return };
        self.go_to_file.ask_focus(&session, path, None);
        self.open_file(&session, path, OpenFile { keep: true, view: None }, tx);
        self.set_content_open(true);
        self.go_to_file_close();
    }

    /// Enter: the active row.
    pub fn go_to_pick_active(&mut self, tx: &mut Tx) {
        if let Some(path) = self.go_to_items().get(self.go_to.active).cloned() {
            self.go_to_pick(&path, tx);
        }
    }

    /// From `poll`. The search follows the box (its 80 ms wait, one request, a newer one replacing an older), a new list puts the
    /// active row first, and a file asked to take the keyboard takes it once its code is drawn. `true` when something drawn changed.
    pub fn keep_go_to_file(&mut self, now: f64, tx: &mut Tx) -> bool {
        let shown = self.ui.open == Some(crate::ui::Open::GoToFile) && self.go_to_file.session.is_some();
        let mut changed = false;
        if shown || self.go_to.wanted(true).is_some() {
            let session = self.go_to_file.session.clone().unwrap_or_default();
            let scope = Scope::Session(session.clone());
            let wanted = self.go_to.wanted(shown).map(str::to_string);
            let scope = shown.then_some(&scope);
            let loading = (self.go_to.search.loading, self.go_to.search.error.is_some());
            match self.go_to.search.want(scope, wanted.as_deref(), now) {
                Step::Idle => {}
                Step::Wait(seconds) => tx.wake_after(Duration::from_secs_f64(seconds + 0.002)),
                Step::Send { generation, query } => self.send_go_to_search(&session, generation, query),
            }
            changed |= loading != (self.go_to.search.loading, self.go_to.search.error.is_some());
            if shown {
                let items = self.go_to_items();
                changed |= self.go_to.follow(&items);
            }
        }
        changed | self.take_file_focus(tx)
    }

    fn send_go_to_search(&mut self, session: &str, generation: u64, query: String) {
        let Some(link) = self.link_for(session) else {
            // An invented session: the invented project answers at once.
            let files = crate::mock::files_tab::find(&query);
            self.go_to.search.answered(generation, &query, Ok(files));
            return;
        };
        let (id, asked) = (session.to_string(), query.clone());
        link.off_thread(
            move |base| match http::find_files(base, &id, &asked) {
                Ok((status, body)) if (200..300).contains(&status) => Ok(parse_files(&body)),
                Ok((status, body)) => Err(failure(status, &body)),
                Err(why) => Err(why),
            },
            move |s, _, result| s.go_to.search.answered(generation, &query, result),
        );
    }

    /// The open file's code is drawn: the request for it is taken, once. It scrolls the line to the middle and marks it (slice 1 has
    /// no caret), and the code takes the keyboard. A file that can't be shown drops the request.
    fn take_file_focus(&mut self, tx: &mut Tx) -> bool {
        let session = self.canvas_session();
        let tabs = self.canvases.session(&session);
        let Some(file) = tabs.active().and_then(|c| c.file.clone()) else { return false };
        if self.go_to_file.focus_on_open.as_ref().is_none_or(|p| p.session != session || p.path != file.path) {
            return false;
        }
        let machine = self.machine_key_of(&session).to_string();
        let status = self.file_buffers.info(&machine, &session, &file.path).map(|i| i.status);
        let image = super::canvases::is_image_path(&file.path);
        if status == Some(Status::Loading) || (status.is_none() && !image) {
            return false;
        }
        let Some(line) = self.go_to_file.take_focus(&session, &file.path) else { return false };
        if status != Some(Status::Ready) || file.view == super::canvases::FileView::Rendered {
            return true;
        }
        let count = self.file_buffers.buffer(&machine, &session, &file.path).map_or(0, |b| b.lines().len());
        tx.focus(crate::view::code_view::CODE, false);
        self.go_to_file.shown_line = line.map(|l| (l as usize).clamp(1, count.max(1)) as u32).map(|l| (session, file.path.clone(), l));
        if let Some((_, _, l)) = &self.go_to_file.shown_line {
            tx.scroll_center_in_instant(
                crate::view::code_view::CODE,
                format!("code-line-{l}{}", if self.sidebar.file_wrap(&file.path) { "w" } else { "" }),
            );
        }
        true
    }
}
