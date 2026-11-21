//! The `#` picker's host (`composables/use-github-picker.ts`, `NewSessionComposer.vue`'s `# picker`): what GitHub offers for the
//! `#query` typed in the New session box, in the folder's repository. Core owns the trigger and the place (`ItemPicker`); this owns
//! the list, the three shapes of question (nothing typed, a number, words), the 200 ms pause, the answer a newer question drops, and
//! the keys. Every request goes on a thread; nothing here waits.
use super::items::{ItemPage, ItemSummary, error_words, item_session_preset};
use super::sessions::{open_session, sessions_of};
use super::{ID, state, state_ref};
use crate::conv::export_history::encode_component;
use crate::fleet::http;
use crate::fleet::new_session::places::{HashTrigger, ItemPicker};
use crate::fleet::new_session::request::Folder;
use crate::fleet::settings::HOME_MACHINE_KEY;
use crate::theme::Theme;
use crate::{State, Tx};
use rune::{El, Key, KeyEvent};
use std::sync::Arc;
use std::time::Duration;

/// `DEBOUNCE_MS`, in the app's clock seconds.
pub const DEBOUNCE: f64 = 0.2;
const FAILED: &str = "GitHub didn't answer.";

/// `PickerGroup`.
#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    pub label: &'static str,
    pub items: Vec<ItemSummary>,
}

/// What the picker is asking for, and the answers that are in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Part {
    Assigned,
    Recent,
    Exact,
    Matches,
}

/// The picker's own state, in the plugin's entry.
#[derive(Debug, Default)]
pub struct Picker {
    /// `owner`, `repo` and the query the list is for; `None` while the popup is shut or the folder has no GitHub repository.
    wanted: Option<(String, String, String)>,
    pub groups: Vec<Group>,
    pub loading: bool,
    pub error: Option<String>,
    /// The highlighted row, over the groups' items in order (`pickerIndex`).
    pub index: usize,
    /// Bumped by every question and by closing: an answer of another generation is dropped (Fleet aborts the request).
    generation: u64,
    /// When the pause after a typed query is over, on the app's clock.
    due: Option<f64>,
    answers: Vec<(Part, Result<Vec<ItemSummary>, String>)>,
    expect: usize,
}

impl Picker {
    /// A list as a scene draws it, with no request: the popup is for `owner/repo` and nothing typed after `#`.
    pub fn show(&mut self, groups: Vec<Group>, loading: bool) {
        *self = Picker { wanted: Some(("acme".into(), "rocket".into(), String::new())), groups, loading, ..Picker::default() };
    }

    /// Every item in the order the rows are drawn.
    pub fn items(&self) -> Vec<&ItemSummary> {
        self.groups.iter().flat_map(|g| g.items.iter()).collect()
    }
}

/// `parseGitHubRemote`: `owner/repo` from `git@github.com:owner/repo.git` or `https://github.com/owner/repo`.
pub fn parse_github_remote(url: &str) -> Option<(String, String)> {
    let url = url.trim();
    let lower = url.to_ascii_lowercase();
    let mut from = 0;
    while let Some(at) = lower[from..].find("github.com") {
        let after = from + at + "github.com".len();
        from = after;
        if !matches!(url.as_bytes().get(after), Some(b':' | b'/')) {
            continue;
        }
        let mut rest = &url[after + 1..];
        rest = rest.strip_suffix('/').unwrap_or(rest);
        rest = rest.strip_suffix(".git").filter(|r| !r.ends_with('/')).unwrap_or(rest);
        let Some((owner, repo)) = rest.split_once('/') else { continue };
        let owner_ok = !owner.is_empty() && owner.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
        let repo_ok = !repo.is_empty() && repo.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
        if owner_ok && repo_ok {
            return Some((owner.to_string(), repo.to_string()));
        }
    }
    None
}

/// `gitHubRepository`: the Folder chip's repository, read from its remotes (`origin` first; the server's own reading of a remote, else
/// its URL). `None` for a directory, a chat with no folder, remotes not in yet, or none on GitHub.
pub fn repository(s: &State) -> Option<(String, String)> {
    if !matches!(s.new_draft.draft()?.folder, Some(Folder::Repository(_))) {
        return None;
    }
    let mut remotes: Vec<_> = s.repository_detail()?.remotes.iter().collect();
    remotes.sort_by_key(|r| r.name != "origin");
    remotes.into_iter().find_map(|r| r.github.clone().or_else(|| parse_github_remote(&r.url)))
}

pub(super) fn search_path(q: &str, first: usize) -> String {
    format!("/api/integrations/github/search?q={}&first={first}", encode_component(q))
}

impl State {
    /// One question's request; its answer joins the others of the same generation.
    fn picker_search(&mut self, generation: u64, part: Part, q: String, first: usize) -> bool {
        self.feed_call(
            HOME_MACHINE_KEY,
            move |base| http::text_with("GET", base, &search_path(&q, first), None),
            move |s, _, answer| {
                let items = match answer {
                    Ok((status, body)) if (200..300).contains(&status) => {
                        Ok(ItemPage::read(&serde_json::from_str(&body).unwrap_or_default()).items)
                    }
                    Ok((status, body)) => Err(error_words(status, &body)),
                    Err(_) => Err(FAILED.into()),
                };
                s.picker_answered(generation, part, items);
            },
        )
    }

    /// `load`: the questions the typed query needs.
    fn picker_load(&mut self, owner: &str, repo: &str, typed: &str) {
        let p = &mut state(&mut self.places).picker;
        p.generation += 1;
        let generation = p.generation;
        p.loading = true;
        p.error = None;
        p.answers.clear();
        let scope = format!("repo:{owner}/{repo} is:open");
        let mut sent = true;
        if typed.is_empty() {
            p.expect = 2;
            sent &= self.picker_search(generation, Part::Assigned, format!("{scope} assignee:@me sort:updated-desc"), 5);
            sent &= self.picker_search(generation, Part::Recent, format!("{scope} sort:updated-desc"), 8);
        } else if typed.chars().all(|c| c.is_ascii_digit()) {
            p.expect = 2;
            let number: u64 = typed.parse().unwrap_or(0);
            let asked =
                self.github_items_by_number(owner, repo, &[number], move |s, _, items| s.picker_answered(generation, Part::Exact, items));
            sent &= asked == super::item::Asked::Sent;
            sent &= self.picker_search(generation, Part::Matches, format!("{scope} {typed} sort:updated-desc"), 8);
        } else {
            p.expect = 1;
            sent &= self.picker_search(generation, Part::Matches, format!("{scope} {typed} sort:updated-desc"), 10);
        }
        if !sent {
            self.picker_answered(generation, Part::Matches, Err(FAILED.into()));
        }
    }

    /// An answer is in: the list is made once every question has one; the first failure ends it (`Promise.all`).
    fn picker_answered(&mut self, generation: u64, part: Part, items: Result<Vec<ItemSummary>, String>) {
        let p = &mut state(&mut self.places).picker;
        if p.generation != generation {
            return;
        }
        match items {
            Err(why) => {
                p.generation += 1;
                p.groups.clear();
                p.loading = false;
                p.error = Some(why);
            }
            Ok(items) => {
                p.answers.push((part, Ok(items)));
                if p.answers.len() < p.expect {
                    return;
                }
                let of = |wanted: Part| -> Vec<ItemSummary> {
                    p.answers.iter().find(|(x, _)| *x == wanted).and_then(|(_, r)| r.clone().ok()).unwrap_or_default()
                };
                let groups = if p.answers.iter().any(|(x, _)| *x == Part::Assigned) {
                    let assigned = of(Part::Assigned);
                    let recent: Vec<_> = of(Part::Recent).into_iter().filter(|r| !assigned.iter().any(|a| a.number == r.number)).collect();
                    vec![Group { label: "Assigned to you", items: assigned }, Group { label: "Recently updated", items: recent }]
                } else {
                    let exact = of(Part::Exact);
                    let rest: Vec<_> = of(Part::Matches).into_iter().filter(|m| !exact.iter().any(|e| e.number == m.number)).collect();
                    vec![Group { label: "Matches", items: exact.into_iter().chain(rest).collect() }]
                };
                p.groups = groups.into_iter().filter(|g| !g.items.is_empty()).collect();
                p.loading = false;
                p.index = p.index.min(p.groups.iter().map(|g| g.items.len()).sum::<usize>().saturating_sub(1));
            }
        }
    }

    /// The watch on the repository and the query: opening asks at once, typing after a pause, closing drops what is on its way.
    fn picker_keep(&mut self, open: Option<&HashTrigger>, now: f64, tx: &mut Tx) -> bool {
        let repo = open.and_then(|_| repository(self));
        let next = repo.zip(open).map(|((o, r), t)| (o, r, t.query.clone()));
        let p = &mut state(&mut self.places).picker;
        let mut changed = false;
        if p.wanted != next {
            let previous = std::mem::replace(&mut p.wanted, next.clone());
            changed = true;
            p.due = None;
            match next {
                None => {
                    p.generation += 1;
                    p.groups.clear();
                    p.loading = false;
                    p.error = None;
                    p.answers.clear();
                }
                Some((owner, repo, query)) => {
                    if previous.as_ref().is_none_or(|(_, _, old)| *old != query) {
                        p.index = 0;
                    }
                    p.loading = true;
                    if previous.is_none() {
                        self.picker_load(&owner, &repo, &query);
                    } else {
                        p.due = Some(now + DEBOUNCE);
                        tx.wake_after(Duration::from_secs_f64(DEBOUNCE + 0.002));
                    }
                }
            }
        }
        let p = &mut state(&mut self.places).picker;
        if let (Some(at), Some((owner, repo, query))) = (p.due, p.wanted.clone())
            && at <= now
        {
            p.due = None;
            self.picker_load(&owner, &repo, &query);
            changed = true;
        }
        changed
    }

    /// The keys of the open popup (A6.6-A6.10).
    fn picker_key(&mut self, ev: &KeyEvent, tx: &mut Tx) -> bool {
        if self.new_box.edit.is_composing() {
            return false;
        }
        let p = &state_ref(&self.places).picker;
        let count = p.items().len();
        let index = p.index;
        match ev.key {
            Key::Down | Key::Up => {
                let next = if count == 0 {
                    0
                } else if ev.key == Key::Down {
                    (index + 1) % count
                } else {
                    (index + count - 1) % count
                };
                state(&mut self.places).picker.index = next;
                tx.scroll_into_view_in("github-picker-scroll", format!("github-item-picker-row-{next}"));
                true
            }
            Key::Enter | Key::Tab => {
                let item = p.items().get(index.min(count.saturating_sub(1))).copied().cloned();
                match item {
                    // Still asking GitHub: wait rather than send "Fix #" by accident. With nothing to pick, Enter sends.
                    None => p.loading,
                    Some(item) => {
                        self.picker_pick(&item, ev.mods.shift, tx);
                        true
                    }
                }
            }
            Key::Escape => {
                self.new_hash_dismiss();
                true
            }
            _ => false,
        }
    }

    /// `pickGitHubItem(item, openExisting)`: Shift opens the item's session when it has one, else the item starts one.
    pub fn picker_pick(&mut self, item: &ItemSummary, open_existing: bool, tx: &mut Tx) {
        if open_existing && let Some(session) = sessions_of(self, item).first().cloned() {
            open_session(self, &session, tx);
            return;
        }
        self.pick_item(&item_session_preset(item, None), tx);
    }

    /// The popup's keys of rows that already have a session, lower-cased (`withSessions`).
    pub fn picker_has_session(&self, item: &ItemSummary) -> bool {
        !sessions_of(self, item).is_empty()
    }
}

/// The host core registers for the `#` place.
pub struct Host;

pub fn host() -> Arc<dyn ItemPicker> {
    Arc::new(Host)
}

impl ItemPicker for Host {
    fn on(&self, s: &State) -> bool {
        s.places.entry(ID).is_some_and(|e| e.manifest.enabled(&s.pickers.prefs))
    }

    fn keep(&self, s: &mut State, open: Option<&HashTrigger>, now: f64, tx: &mut Tx) -> bool {
        s.picker_keep(open, now, tx)
    }

    fn key(&self, s: &mut State, ev: &KeyEvent, tx: &mut Tx) -> bool {
        s.picker_key(ev, tx)
    }

    fn view(&self, s: &State, t: &Theme) -> Option<El> {
        Some(super::view::picker::popup(s, t))
    }
}
