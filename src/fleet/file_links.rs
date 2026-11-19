//! The files a message names, as links (`stores/file-links.ts`, `MessageBubble.vue`'s `openFile`, `ActivityStream.vue`'s
//! `openReferencedFile`). The paths in a session's messages are asked of the server once, in batches
//! (`POST …/files/resolve`); those that are files in its folder draw as links and open a tab, at the line the text named.
use super::canvases::{FileView, OpenFile, has_rendered_view};
use crate::State;
use crate::conv::references::{markdown_file_paths, parse_file_reference};
use crate::view::conversation::Part;
use rune::Tx;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

/// The most paths one request asks about; the server looks at no more.
pub const MAX_PATHS_PER_REQUEST: usize = 200;
/// How long a failed request waits before the same paths are asked again (Fleet asks again on the next render).
const RETRY_AFTER: f64 = 5.0;
/// What a link's destination starts with, so a press knows it is one of ours (and not a web address).
pub const SCHEME: &str = "fleet-file:";

/// Path as the text named it → path from the session's folder, for the paths that are files there.
pub type Found = Rc<HashMap<String, String>>;

#[derive(Default)]
struct Session {
    found: Found,
    missing: HashSet<String>,
    asking: HashSet<String>,
    /// How much of each text part has been read for paths (part id → length), so a part is read again only as it grows.
    read: HashMap<String, usize>,
    /// `true` while the turn was running at the last look: when it stops, the paths that weren't files are asked again.
    working: bool,
    retry_at: f64,
}

#[derive(Default)]
pub struct FileLinks {
    sessions: HashMap<String, Session>,
}

impl FileLinks {
    /// What the session's messages name that is a file there; the view reads it.
    pub fn found(&self, session: &str) -> Found {
        self.sessions.get(session).map(|s| s.found.clone()).unwrap_or_default()
    }

    /// `forgetMissing`: the paths that weren't files are asked again; the files found stay links.
    pub fn forget_missing(&mut self, session: &str) {
        if let Some(s) = self.sessions.get_mut(session) {
            s.missing.clear();
            s.read.clear();
        }
    }

    /// An answer for `paths`: each is a file (with its path from the folder) or isn't one.
    pub fn answered(&mut self, session: &str, paths: &[String], files: &HashMap<String, String>) {
        let s = self.sessions.entry(session.to_string()).or_default();
        let mut found = (*s.found).clone();
        for path in paths {
            s.asking.remove(path);
            match files.get(path) {
                Some(relative) => {
                    s.missing.remove(path);
                    found.insert(path.clone(), relative.clone());
                }
                None => {
                    s.missing.insert(path.clone());
                }
            }
        }
        s.found = Rc::new(found);
    }

    /// The request failed: not known yet, so the paths are asked about again after a pause.
    pub fn failed(&mut self, session: &str, paths: &[String], now: f64) {
        let s = self.sessions.entry(session.to_string()).or_default();
        for path in paths {
            s.asking.remove(path);
        }
        s.read.clear();
        s.retry_at = now + RETRY_AFTER;
    }

    /// The paths in `texts` (part id, text) not known or being asked yet, in order, once each; marks them as being asked.
    pub fn wanted<'a>(&mut self, session: &str, texts: impl Iterator<Item = (&'a str, &'a str)>, now: f64) -> Vec<String> {
        let s = self.sessions.entry(session.to_string()).or_default();
        if now < s.retry_at {
            return Vec::new();
        }
        let mut wanted: Vec<String> = Vec::new();
        for (id, text) in texts {
            if s.read.get(id) == Some(&text.len()) {
                continue;
            }
            s.read.insert(id.to_string(), text.len());
            for path in markdown_file_paths(text) {
                if !s.found.contains_key(&path) && !s.missing.contains(&path) && !s.asking.contains(&path) && !wanted.contains(&path) {
                    wanted.push(path);
                }
            }
        }
        s.asking.extend(wanted.iter().cloned());
        wanted
    }

    /// The turn's end is seen once: the paths that weren't files are asked about again.
    pub fn turn(&mut self, session: &str, working: bool) {
        let s = self.sessions.entry(session.to_string()).or_default();
        let ended = s.working && !working;
        s.working = working;
        if ended {
            self.forget_missing(session);
        }
    }
}

/// The destination a resolved path's link carries: `fleet-file:` + the path from the folder, and `#L{line}` at a line.
pub fn destination(relative: &str, line: Option<u32>) -> String {
    match line {
        Some(n) => format!("{SCHEME}{relative}#L{n}"),
        None => format!("{SCHEME}{relative}"),
    }
}

/// A destination of ours back into the path and the line.
fn from_destination(url: &str) -> Option<(String, Option<u32>)> {
    let rest = url.strip_prefix(SCHEME)?;
    match rest.rsplit_once("#L") {
        Some((path, n)) if n.parse::<u32>().is_ok() => Some((path.to_string(), n.parse().ok())),
        _ => Some((rest.to_string(), None)),
    }
}

/// What a text names, as a link's destination: inline code that reads as a file and that the server said is one
/// (`code_link`), and the URL of a Markdown link to such a file.
pub fn link_for(found: &Found, text: &str) -> Option<String> {
    let reference = parse_file_reference(text)?;
    found.get(&reference.path).map(|relative| destination(relative, reference.line))
}

/// The tooltip (`title` of `.file-ref`): "Open {path}[ at line N] · Ctrl-click keeps the tab", ⌘-click on macOS.
pub fn help(destination: &str) -> Option<String> {
    let (path, line) = from_destination(destination)?;
    let at = line.map(|n| format!(" at line {n}")).unwrap_or_default();
    let key = if cfg!(target_os = "macos") { "⌘" } else { "Ctrl" };
    Some(format!("Open {path}{at} · {key}-click keeps the tab"))
}

impl State {
    /// From `poll`: reads the open session's text for the files it names and asks the server about the new ones, one request
    /// per 200 paths; sees a turn end. Invented sessions are answered from the invented project at once.
    pub fn keep_file_links(&mut self, now: f64) {
        let session = self.canvas_session();
        let working = self.session().status == crate::Status::Working;
        self.file_links.turn(&session, working);
        let mut links = std::mem::take(&mut self.file_links);
        let texts = self.session().messages.iter().flat_map(|m| m.parts.iter()).filter_map(|p| match p {
            Part::Text { id, text } => Some((id.as_str(), text.as_str())),
            _ => None,
        });
        let wanted = links.wanted(&session, texts, now);
        self.file_links = links;
        if wanted.is_empty() {
            return;
        }
        for batch in wanted.chunks(MAX_PATHS_PER_REQUEST) {
            self.resolve_files(&session, batch.to_vec(), now);
        }
    }

    fn resolve_files(&mut self, session: &str, paths: Vec<String>, now: f64) {
        let Some(link) = self.link_for(session) else {
            let files = paths.iter().filter(|p| crate::mock::files_tab::is_file(p)).map(|p| (p.clone(), p.clone())).collect();
            self.file_links.answered(session, &paths, &files);
            return;
        };
        let (id, asked) = (session.to_string(), paths.clone());
        link.off_thread(
            move |base| match super::http::files_resolve(base, &id, &asked) {
                Ok((status, body)) if (200..300).contains(&status) => Ok(parse_resolved(&body)),
                Ok((status, _)) => Err(format!("Failed to resolve files: {status}")),
                Err(why) => Err(why),
            },
            {
                let session = session.to_string();
                move |s, _, result| match result {
                    Ok(files) => s.file_links.answered(&session, &paths, &files),
                    Err(_) => s.file_links.failed(&session, &paths, now),
                }
            },
        );
    }

    /// A press on a link of ours (`openFile` of the bubble): a click is a preview, Ctrl/Cmd-click keeps the tab; Shift or Alt
    /// opens nothing. A web address opens in the browser; a path the server didn't call a file opens nothing.
    pub fn open_file_link(&mut self, url: &str, mods: rune::Mods, tx: &mut Tx) {
        if url.contains("://") || url.starts_with("mailto:") {
            let _ = tx.open_url(url);
            return;
        }
        if mods.shift || mods.alt {
            return;
        }
        let session = self.canvas_session();
        let target = match from_destination(url) {
            Some(t) => Some(t),
            // A Markdown link's own address: `[tax.ts](src/billing/tax.ts#L18)`.
            None => link_for(&self.file_links.found(&session), url).and_then(|d| from_destination(&d)),
        };
        if let Some((path, line)) = target {
            self.open_file_at(&session, &path, line, mods.ctrl || mods.cmd, tx);
        }
    }

    /// A file named in a message or a find-in-files hit opens in a tab beside the conversation and the panel shows. At a line the
    /// view scrolls to it and takes the keyboard; the line is in the source, so Markdown and HTML show their source, not the page.
    pub fn open_file_at(&mut self, session: &str, path: &str, line: Option<u32>, keep: bool, tx: &mut Tx) {
        if let Some(line) = line {
            self.go_to_file.ask_focus(session, path, Some(line));
        }
        let view = (line.is_some() && has_rendered_view(path)).then_some(FileView::Edit);
        self.open_file(session, path, OpenFile { keep, view }, tx);
        self.set_content_open(true);
    }
}

/// `{ files: [{ path, relativePath }] }`: each path as it was asked, with its path from the folder.
pub fn parse_resolved(body: &str) -> HashMap<String, String> {
    let value: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    value
        .get("files")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|f| Some((f.get("path")?.as_str()?.to_string(), f.get("relativePath")?.as_str()?.to_string())))
        .collect()
}
