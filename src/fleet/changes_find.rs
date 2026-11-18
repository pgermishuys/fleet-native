//! Find in Changes, the state behind the bar (`composables/use-changes-find.ts` and the find half of `ChangesReview.vue`, rows 11.2,
//! 11.3, 11.5, 11.7, 11.9 and 11.10 of docs/parity/12-changes-and-review.md): whether it is open, the query, the matches over the
//! diff data, stepping, which files have to load for the search, and the machine's search that spares loading all of them.
//! No view in here; the bar, the marks and the keys are piece K's.
//!
//! The machine is asked which changed files hold the text (`GET /diffs/search?q=`, weave-fleet #941), 120 ms after the last
//! keystroke, and only those load. A machine that can't say (an older Fleet, git failing), or a query git's `-i` can't fold (it
//! folds ASCII letters only), loads every file instead. A newer query replaces the one in flight: its answer is dropped by number
//! where Fleet aborts the request.
use super::diff_contents::Entry;
use super::diffs::FileDiff;
use super::http;
use super::review::{starts_collapsed, visible};
use crate::State;
use crate::conv::changes_find::{ChangesHit, FindFile, Reveal, Section, find_changes_hits, find_lines, reveal_plan, selected_query};
use crate::conv::review::ReviewItem;
use rune::Tx;
use serde_json::Value;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::time::Duration;

/// How long after the last keystroke the machine is asked (`setTimeout(…, 120)`), in seconds.
pub const SEARCH_DEBOUNCE: f64 = 0.12;

/// What `ChangesFind::sync` says to do about the machine's search.
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    Idle,
    /// Look again in this many seconds: the request is waiting out the debounce.
    Wait(f64),
    /// Ask now; hand the answer back with this number.
    Send {
        run: u64,
        query: String,
    },
}

/// What the machine said of a query: the changed files that hold it, with how many lines do.
pub type Found = HashMap<String, u64>;

#[derive(Debug, Default)]
pub struct ChangesFind {
    pub open: bool,
    pub query: String,
    pub current: usize,
    /// `serverMatches`: `None` nothing asked (bar shut, nothing typed, or the answer is on its way); `Some(None)` the machine can't say.
    server: Option<Option<Found>>,
    /// `searchPending`: the answer is on its way, or the debounce is running.
    pending: bool,
    run: u64,
    due: Option<f64>,
    /// What the last `sync` looked at: when it changes the search starts again.
    watched: Option<(bool, String, u64)>,
    session: String,
    hits: Vec<ChangesHit>,
    /// What `hits` were worked out from.
    hits_stamp: Option<u64>,
    /// The query and whether it had matches when the last jump was made: either changing goes to the first match.
    jumped: (String, bool),
}

/// `/[^\u0000-\u007f]/`.
fn not_ascii(query: &str) -> bool {
    !query.is_ascii()
}

impl ChangesFind {
    /// `open`: the selection (a one-line, short one inside Changes) becomes the query.
    pub fn open(&mut self, selection: Option<&str>) {
        let selected = selected_query(selection);
        if !selected.is_empty() {
            self.set_query(&selected);
        }
        self.open = true;
    }

    /// `close`: the matches go.
    pub fn close(&mut self) {
        self.open = false;
    }

    /// Typing: the search starts again at the first match.
    pub fn set_query(&mut self, query: &str) {
        if self.query != query {
            self.query = query.to_string();
            self.current = 0;
        }
    }

    pub fn hits(&self) -> &[ChangesHit] {
        &self.hits
    }

    pub fn current_hit(&self) -> Option<&ChangesHit> {
        self.hits.get(self.current)
    }

    /// `step`: next (1) or previous (-1) match, wrapping at both ends. False with no matches.
    pub fn step(&mut self, direction: i32) -> bool {
        let total = self.hits.len() as i64;
        if total == 0 {
            return false;
        }
        self.current = (self.current as i64 + i64::from(direction)).rem_euclid(total) as usize;
        true
    }

    /// The bar's count (`ChangesFindBar` `count`): nothing for a blank query, "Searching…" or "No matches", else "2 of 7".
    pub fn count_label(&self, searching: bool) -> String {
        if self.query.trim().is_empty() {
            return String::new();
        }
        match self.hits.len() {
            0 if searching => "Searching…".into(),
            0 => "No matches".into(),
            n => format!("{} of {n}", self.current + 1),
        }
    }

    /// `findWants`: whether find needs this file loaded. It holds the text, or the machine can't say; lockfiles and very large diffs
    /// stay shut until opened.
    pub fn wants(&self, file: &FileDiff) -> bool {
        !starts_collapsed(file)
            && match &self.server {
                None => false,
                Some(None) => true,
                Some(Some(found)) => found.contains_key(&file.file),
            }
    }

    /// `findUnsearched`: files that start shut (lockfiles, very large diffs), are shut, aren't loaded, and that the machine says hold
    /// the text (all of them when it can't say). `is_open` and `is_ready` say what the review shows of a path.
    pub fn unsearched(&self, files: &[FileDiff], is_open: impl Fn(&str) -> bool, is_ready: impl Fn(&str) -> bool) -> usize {
        files
            .iter()
            .filter(|f| {
                starts_collapsed(f)
                    && !is_open(&f.file)
                    && !is_ready(&f.file)
                    && match &self.server {
                        Some(Some(found)) => found.contains_key(&f.file),
                        _ => true,
                    }
            })
            .count()
    }

    /// `findSearching`: the machine's answer is on its way, or a file the search wants is loading.
    pub fn searching(&self, files: &[FileDiff], is_loading: impl Fn(&str) -> bool) -> bool {
        self.pending || files.iter().any(|f| self.wants(f) && is_loading(&f.file))
    }

    /// The watch on `[isOpen, query, files]`: any change drops the search in flight and starts again. A blank query asks nothing; one
    /// git can't fold is searched by loading every file.
    pub fn sync(&mut self, files: &[FileDiff], now: f64) -> Step {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for f in files {
            (&f.file, super::diff_contents::version_of(f)).hash(&mut h);
        }
        let watched = (self.open, self.query.trim().to_string(), h.finish());
        if self.watched.as_ref() != Some(&watched) {
            self.watched = Some(watched.clone());
            self.run += 1;
            self.due = None;
            let (open, query, _) = watched;
            if !open || query.is_empty() {
                self.server = None;
                self.pending = false;
            } else if not_ascii(&query) {
                self.server = Some(None);
                self.pending = false;
            } else {
                self.pending = true;
                self.due = Some(now + SEARCH_DEBOUNCE);
            }
        }
        match self.due {
            Some(due) if now >= due => {
                self.due = None;
                Step::Send { run: self.run, query: self.query.trim().to_string() }
            }
            Some(due) => Step::Wait(due - now),
            None => Step::Idle,
        }
    }

    /// The machine's answer to the search `run` asked: an older run's is dropped. Any failure or non-OK status is "can't say".
    pub fn answered(&mut self, run: u64, answer: Result<(u16, Value), String>) {
        if run != self.run {
            return;
        }
        self.server = Some(match answer {
            Ok((status, body)) if (200..300).contains(&status) => Some(parse_found(&body)),
            _ => None,
        });
        self.pending = false;
    }
}

fn parse_found(body: &Value) -> Found {
    body.get("files")
        .and_then(Value::as_array)
        .map(|files| {
            files
                .iter()
                .filter_map(|f| Some((f.get("file")?.as_str()?.to_string(), f.get("count").and_then(Value::as_u64).unwrap_or(0))))
                .collect()
        })
        .unwrap_or_default()
}

impl State {
    /// A file's review items when its contents are in and it has lines to show (`!view.placeholder`).
    fn find_items(&self, file: &FileDiff) -> Option<std::rc::Rc<Vec<ReviewItem>>> {
        match self.diff_contents.entry(&file.file) {
            Some(Entry::Ready(item)) if !item.binary && !item.truncated => Some(crate::view::review::review_items(item)),
            _ => None,
        }
    }

    /// `findFiles`: every loaded file's searchable lines, in the order the review draws them.
    pub fn changes_find_files(&self) -> Vec<FindFile> {
        self.review_files()
            .iter()
            .filter_map(|f| self.find_items(f).map(|items| FindFile { path: f.file.clone(), lines: find_lines(&items) }))
            .collect()
    }

    /// What the matches are worked out from: the query and what each file holds. Equal means the same matches.
    fn changes_find_stamp(&self, files: &[FileDiff]) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        (self.changes_find.open, &self.changes_find.query).hash(&mut h);
        for f in files {
            match self.diff_contents.entry(&f.file) {
                Some(Entry::Ready(item)) => (&f.file, item.before.len(), item.after.len(), item.binary || item.truncated).hash(&mut h),
                _ => (&f.file, 0usize, 0usize, true).hash(&mut h),
            }
        }
        h.finish()
    }

    /// `Ctrl+F` or the header's Find button. `selection` is the text selected inside Changes, if any.
    pub fn changes_find_open(&mut self, selection: Option<&str>) {
        self.changes_find.open(selection);
    }

    pub fn changes_find_close(&mut self) {
        self.changes_find.close();
    }

    pub fn changes_find_set_query(&mut self, query: &str) {
        self.changes_find.set_query(query);
    }

    /// Next (1) or previous (-1) match: its line is drawn at once.
    pub fn changes_find_step(&mut self, direction: i32) {
        if self.changes_find.step(direction) {
            self.changes_find_reveal();
        }
    }

    pub fn changes_find_count_label(&self) -> String {
        let files = self.review_files();
        let loading = |p: &str| matches!(self.diff_contents.entry(p), Some(Entry::Loading));
        self.changes_find.count_label(self.changes_find.searching(&files, loading))
    }

    /// `findUnsearched` over what the review shows.
    pub fn changes_find_unsearched(&self) -> usize {
        let files = self.review_files();
        let ready = |p: &str| matches!(self.diff_contents.entry(p), Some(Entry::Ready(_)));
        self.changes_find.unsearched(&files, |p| self.review.is_open(p), ready)
    }

    /// What drawing the current match takes (`revealHit`), for a view or a test to look at before it is done.
    pub fn changes_find_plan(&self, hit: &ChangesHit) -> Reveal {
        let Some(items) = self.diffs.by_file(&hit.path).and_then(|f| self.find_items(f)) else {
            return Reveal::default();
        };
        let drawn = visible(&items, self.review.is_drawn_all(&hit.path)).iter().any(|s| {
            s.item.id() == hit.item_id
                && match s.item {
                    ReviewItem::Fold(_) => true,
                    ReviewItem::Hunk(h) => h.lines.iter().take(s.take).any(|l| l.index == hit.line),
                }
        });
        let section = Section {
            open: self.review.is_open(&hit.path),
            // Rendered is component 12's piece M: until it exists no file is viewed as Rendered.
            rendered: false,
            fold_open: self.review.fold_open(&hit.path, &hit.item_id),
            drawn,
        };
        reveal_plan(hit, section)
    }

    /// `revealHit`: opens the match's file, marks it near, opens its folded run, draws the rest of a long diff.
    pub fn changes_find_reveal(&mut self) {
        let Some(hit) = self.changes_find.current_hit().cloned() else { return };
        let plan = self.changes_find_plan(&hit);
        if plan.open_file {
            self.review.toggle(&hit.path);
        }
        self.review.mark_near([hit.path.clone()]);
        if plan.open_fold {
            self.review.open_fold(&hit.path, &hit.item_id);
        }
        if plan.show_all {
            self.review.show_all(&hit.path);
        }
        self.review_sync();
    }

    /// From `poll`: starts the machine's search when its debounce is up, loads the files the search wants, works the matches out
    /// again when what they come from changed, and draws the first match when a new query or the first matches arrive.
    /// True when the bar must redraw.
    pub fn keep_changes_find(&mut self, now: f64, tx: &mut Tx) -> bool {
        let session = self.diffs.session().to_string();
        let mut changed = false;
        if self.changes_find.session != session {
            self.changes_find = ChangesFind { session: session.clone(), ..Default::default() };
            changed = true;
        }
        if session.is_empty() {
            return changed;
        }
        let files = self.review_files();
        match self.changes_find.sync(&files, now) {
            Step::Idle => {}
            Step::Wait(seconds) => tx.wake_after(Duration::from_secs_f64(seconds)),
            Step::Send { run, query } => {
                if let Some(link) = self.link_for(&session) {
                    let session = session.clone();
                    link.off_thread(
                        move |base| http::diff_search(base, &session, &query),
                        move |s, _, answer| s.changes_find.answered(run, answer),
                    );
                } else {
                    self.changes_find.answered(run, Err("no Fleet".into()));
                }
                changed = true;
            }
        }
        if self.changes_find.open {
            let wanted: Vec<&FileDiff> = files.iter().filter(|f| self.changes_find.wants(f)).collect();
            for file in wanted {
                self.load_diff_file(&session, file);
            }
        }
        changed | self.refresh_changes_hits(&files)
    }

    fn refresh_changes_hits(&mut self, files: &[FileDiff]) -> bool {
        let stamp = self.changes_find_stamp(files);
        let mut changed = false;
        if self.changes_find.hits_stamp != Some(stamp) {
            self.changes_find.hits_stamp = Some(stamp);
            let hits =
                if self.changes_find.open { find_changes_hits(&self.changes_find_files(), &self.changes_find.query) } else { Vec::new() };
            changed = hits != self.changes_find.hits;
            self.changes_find.hits = hits;
            if self.changes_find.current >= self.changes_find.hits.len() {
                self.changes_find.current = 0;
            }
        }
        let key = (self.changes_find.query.clone(), !self.changes_find.hits.is_empty());
        if self.changes_find.jumped != key {
            self.changes_find.jumped = key;
            self.changes_find_reveal();
            changed = true;
        }
        changed
    }
}
