//! Find in files, the logic (component 12, piece O; weave-fleet `lib/find-text.ts`, `composables/use-find-text.ts` and
//! `stores/find-in-files.ts`, rows 14.3, 14.4, 14.5 and 14.13 of docs/parity/12-changes-and-review.md): the request
//! `GET /api/sessions/{id}/find/text` on the session's machine, 250 ms after typing pauses, what its answer holds, why it
//! can fail, and the Ctrl+Shift+F request the Files tab takes once it is on screen. No view in here; the results list and the
//! contents half of the Files tab are piece P's.
//!
//! Fleet aborts the older request when a newer query, a switch or another session replaces it; here a run number does that:
//! an answer for an older run is dropped when it arrives.
use super::http;
use crate::State;
use rune::Tx;
use serde_json::Value;
use std::time::{Duration, Instant};

/// Typing waits this long, so a word is searched once, not once per letter (`FIND_TEXT_DEBOUNCE_MS`), in seconds.
pub const DEBOUNCE: f64 = 0.25;
/// Shorter than this searches nothing: one letter matches almost every line (`FIND_TEXT_MIN_CHARS`).
pub const MIN_CHARS: usize = 2;
/// Matches shown per file before "Show N more in this file".
pub const MATCHES_PER_FILE: usize = 5;

/// A marked stretch of a match's text. Offsets are UTF-16 code units, as the .NET server counts them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextRange {
    pub start: usize,
    pub end: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextMatch {
    pub line: u32,
    pub column: u32,
    /// The line, cut around the match when it is long; `ranges` index into this.
    pub text: String,
    pub ranges: Vec<TextRange>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextFile {
    pub path: String,
    /// Every matching line found in the file, which can be more than `matches` holds.
    pub total: u32,
    pub matches: Vec<TextMatch>,
}

/// Why a search ended early.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stopped {
    Limit,
    Deadline,
}

/// How the server found the lines: `git grep`, or a walk of a folder that isn't a repository.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Engine {
    Git,
    Walk,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextSearchResult {
    pub files: Vec<TextFile>,
    pub matches: u32,
    pub stopped: Option<Stopped>,
    pub engine: Engine,
}

/// What `find/text` answered, read leniently (a field the server leaves out is empty). `None` when it isn't an object.
pub fn parse_result(body: &Value) -> Option<TextSearchResult> {
    let number = |v: &Value, key: &str| v.get(key).and_then(Value::as_u64).unwrap_or(0) as u32;
    let files = body
        .get("files")
        .and_then(Value::as_array)?
        .iter()
        .map(|f| TextFile {
            path: f.get("path").and_then(Value::as_str).unwrap_or_default().to_string(),
            total: number(f, "total"),
            matches: f
                .get("matches")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(|m| TextMatch {
                    line: number(m, "line"),
                    column: number(m, "column"),
                    text: m.get("text").and_then(Value::as_str).unwrap_or_default().to_string(),
                    ranges: m
                        .get("ranges")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .map(|r| TextRange { start: number(r, "start") as usize, end: number(r, "end") as usize })
                        .collect(),
                })
                .collect(),
        })
        .collect();
    Some(TextSearchResult {
        files,
        matches: number(body, "matches"),
        stopped: match body.get("stopped").and_then(Value::as_str) {
            Some("limit") => Some(Stopped::Limit),
            Some("deadline") => Some(Stopped::Deadline),
            _ => None,
        },
        engine: if body.get("engine").and_then(Value::as_str) == Some("walk") { Engine::Walk } else { Engine::Git },
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextSegment {
    pub text: String,
    pub marked: bool,
}

/// The line cut into plain and marked pieces, so it can be drawn without markup (`segmentsOf`). A range outside the line is
/// ignored, and one that overlaps an earlier one only adds what that left unmarked.
pub fn segments_of(text: &str, ranges: &[TextRange]) -> Vec<TextSegment> {
    let units: Vec<u16> = text.encode_utf16().collect();
    let piece = |from: usize, to: usize| String::from_utf16_lossy(&units[from..to]);
    let mut sorted = ranges.to_vec();
    sorted.sort_by_key(|r| r.start);
    let mut segments = Vec::new();
    let mut at = 0;
    for range in sorted {
        let start = range.start.max(at);
        let end = range.end.min(units.len());
        if end <= start {
            continue;
        }
        if start > at {
            segments.push(TextSegment { text: piece(at, start), marked: false });
        }
        segments.push(TextSegment { text: piece(start, end), marked: true });
        at = end;
    }
    if at < units.len() {
        segments.push(TextSegment { text: piece(at, units.len()), marked: false });
    }
    segments
}

/// A matching line as the list shows it, in the order the keys step through them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VisibleHit {
    pub path: String,
    pub line: u32,
}

/// The lines on screen: the first few per file, or all of them for a file that was expanded (`visibleHits`).
pub fn visible_hits(files: &[TextFile], expanded: &[String]) -> Vec<VisibleHit> {
    files
        .iter()
        .flat_map(|file| {
            let shown = if expanded.contains(&file.path) { file.matches.len() } else { MATCHES_PER_FILE.min(file.matches.len()) };
            file.matches[..shown].iter().map(|m| VisibleHit { path: file.path.clone(), line: m.line })
        })
        .collect()
}

/// Why a search gave no answer. Never shown as "no matches".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailureKind {
    /// The regex is bad (400).
    Pattern,
    /// That machine's Fleet is too old to search inside files (404).
    Unsupported,
    /// Anything else.
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FindTextFailure {
    pub kind: FailureKind,
    pub message: String,
}

fn failure(kind: FailureKind, message: impl Into<String>) -> FindTextFailure {
    FindTextFailure { kind, message: message.into() }
}

/// What a finished request is: the answer, or the failure it names (`search` and its `catch`). `Err` is a request that never
/// got an answer (offline, timed out), which carries its own words.
pub fn classify(answer: Result<(u16, Value), String>) -> Result<TextSearchResult, FindTextFailure> {
    let (status, body) = match answer {
        Ok(a) => a,
        Err(why) => return Err(failure(FailureKind::Failed, why)),
    };
    let reason = body.get("error").and_then(Value::as_str);
    match status {
        404 => Err(failure(FailureKind::Unsupported, "This machine can't search inside files yet.")),
        400 => Err(failure(FailureKind::Pattern, reason.unwrap_or("That isn't a valid pattern."))),
        s if !(200..300).contains(&s) => {
            Err(failure(FailureKind::Failed, reason.map_or_else(|| format!("The search failed (HTTP {s})."), str::to_string)))
        }
        _ => parse_result(&body).ok_or_else(|| failure(FailureKind::Failed, "The search failed.")),
    }
}

/// What `FindText::sync` says to do.
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    Idle,
    /// Look again in this many seconds: the request is waiting out the debounce.
    Wait(f64),
    /// Ask now; hand the answer back with this number.
    Send {
        run: u64,
        session: String,
        query: String,
        regex: bool,
        match_case: bool,
    },
}

/// `useFindText`'s refs: the last answer stays in view until the next one comes.
#[derive(Debug, Default)]
pub struct FindText {
    pub result: Option<TextSearchResult>,
    pub failure: Option<FindTextFailure>,
    pub loading: bool,
    /// How long the last answer took, in milliseconds.
    pub took_ms: f64,
    /// The "Regular expression" switch.
    pub regex: bool,
    /// The "Match case" switch.
    pub match_case: bool,
    /// A Ctrl+Shift+F request was taken: the field is to take the keyboard, its text selected (piece P does that).
    pub focus_field: bool,
    /// Files whose every match is shown, not the first few.
    pub expanded: Vec<String>,
    /// The line the arrow keys are on.
    pub active: Option<VisibleHit>,
    run: u64,
    due: Option<f64>,
    /// What the last `sync` looked at: when it changes the search starts again (the `watch` sources).
    watched: Option<(String, String, bool, bool)>,
}

impl FindText {
    /// `isActive`: the query is long enough to search.
    pub fn is_active(query: &str) -> bool {
        query.trim().chars().count() >= MIN_CHARS
    }

    /// The watch on `[session, query, regex, matchCase]`: any change drops the request in flight and starts again. Another
    /// session drops the old answer; a short query clears everything and asks nothing.
    pub fn sync(&mut self, session: &str, query: &str, now: f64) -> Step {
        let query = query.trim();
        let watched = (session.to_string(), query.to_string(), self.regex, self.match_case);
        if self.watched.as_ref() != Some(&watched) {
            if self.watched.as_ref().is_some_and(|w| w.0 != session) {
                self.result = None;
            }
            self.watched = Some(watched);
            self.run += 1;
            self.due = None;
            if !Self::is_active(query) {
                self.result = None;
                self.failure = None;
                self.loading = false;
            } else {
                self.loading = true;
                self.due = Some(now + DEBOUNCE);
            }
        }
        match self.due {
            Some(due) if now >= due => {
                self.due = None;
                Step::Send { run: self.run, session: session.into(), query: query.into(), regex: self.regex, match_case: self.match_case }
            }
            Some(due) => Step::Wait(due - now),
            None => Step::Idle,
        }
    }

    /// The answer to the search `run` asked: an older run's is dropped (`signal.aborted`).
    pub fn answered(&mut self, run: u64, answer: Result<TextSearchResult, FindTextFailure>, took_ms: f64) {
        if run != self.run {
            return;
        }
        self.loading = false;
        match answer {
            Ok(found) => {
                self.failure = None;
                // A new answer starts over: every file folded, the first line chosen.
                self.expanded.clear();
                self.active =
                    found.files.first().and_then(|f| f.matches.first().map(|m| VisibleHit { path: f.path.clone(), line: m.line }));
                self.result = Some(found);
                self.took_ms = took_ms;
            }
            Err(why) => {
                self.failure = Some(why);
                self.result = None;
            }
        }
    }
}

impl FindText {
    /// What a scene draws: `answer` for `query`, as if it had been asked and had come back `took_ms` later. Nothing is sent.
    pub fn show(&mut self, session: &str, query: &str, answer: Result<TextSearchResult, FindTextFailure>, took_ms: f64) {
        self.watched = Some((session.to_string(), query.trim().to_string(), self.regex, self.match_case));
        self.due = None;
        self.answered(self.run, answer, took_ms);
        self.loading = false;
    }

    /// "Show N more in this file".
    pub fn expand(&mut self, path: &str) {
        if !self.expanded.iter().any(|p| p == path) {
            self.expanded.push(path.to_string());
        }
    }

    /// The lines on screen, in the order the arrow keys step through them.
    pub fn hits(&self) -> Vec<VisibleHit> {
        visible_hits(self.result.as_ref().map_or(&[], |r| &r.files), &self.expanded)
    }

    /// ArrowDown and ArrowUp (`moveActive`): one line on, stopping at either end.
    pub fn move_active(&mut self, step: i32) {
        let hits = self.hits();
        if hits.is_empty() {
            return;
        }
        let at = hits.iter().position(|h| Some(h) == self.active.as_ref()).map_or(-1, |i| i as i32);
        let next = (at + step).clamp(0, hits.len() as i32 - 1) as usize;
        self.active = Some(hits[next].clone());
    }

    /// Enter: the active line, or the first one.
    pub fn enter_hit(&self) -> Option<VisibleHit> {
        let hits = self.hits();
        hits.iter().find(|h| Some(*h) == self.active.as_ref()).or(hits.first()).cloned()
    }
}

/// `stores/find-in-files.ts`: which session's Files tab was asked to search inside files.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FindInFiles {
    request: Option<String>,
}

impl FindInFiles {
    pub fn ask(&mut self, session: &str) {
        self.request = Some(session.to_string());
    }

    /// True once, for the session that was asked.
    pub fn take(&mut self, session: &str) -> bool {
        if self.request.as_deref() != Some(session) {
            return false;
        }
        self.request = None;
        true
    }
}

impl State {
    /// A line picked in the results (`openHit`): its file opens in a preview tab at that line, on its source when it has a
    /// rendered view, and the right panel shows.
    pub fn open_hit(&mut self, path: &str, line: u32, tx: &mut Tx) {
        let session = self.files_session();
        self.go_to_file.ask_focus(&session, path, Some(line));
        let view = super::canvases::has_rendered_view(path).then_some(super::canvases::FileView::Edit);
        self.open_file(&session, path, super::canvases::OpenFile { keep: false, view }, tx);
        self.set_content_open(true);
    }

    /// The palette's "Find in files…" and Ctrl/Cmd+Shift+F: the Files tab of the open session, the right panel shown, and the
    /// request left for the tab to take once it is on screen.
    pub fn find_in_files_ask(&mut self) {
        let Some(session) = self.open_id() else { return };
        self.edit_canvases(|c, id| c.open(id, super::canvases::CanvasKind::Files));
        self.set_content_open(true);
        self.find_in_files.ask(&session);
    }

    /// Called every frame. The Files tab on screen takes a pending request: contents mode, and the field to take the keyboard.
    /// Then the search follows the box: the machine is asked when its debounce is up. True when something drawn changed.
    pub fn keep_find_text(&mut self, now: f64, tx: &mut Tx) -> bool {
        let session = self.files_session();
        let mut changed = false;
        let on_screen = self.open_canvases().active().is_some_and(|c| c.kind == super::canvases::CanvasKind::Files);
        if on_screen && self.find_in_files.take(&session) {
            self.file_browser.search_in = super::file_browser::SearchIn::Contents;
            self.find_text.focus_field = true;
            tx.focus(crate::view::files_tab::FIELD, false);
            tx.select_all_text(crate::view::files_tab::FIELD);
            changed = true;
        }
        let contents = self.file_browser.search_in == super::file_browser::SearchIn::Contents;
        let query = if contents { self.file_browser.field.text.clone() } else { String::new() };
        let before = (self.find_text.loading, self.find_text.result.is_some(), self.find_text.failure.is_some());
        let step = self.find_text.sync(&session, &query, now);
        changed |= before != (self.find_text.loading, self.find_text.result.is_some(), self.find_text.failure.is_some());
        match step {
            Step::Idle => {}
            Step::Wait(seconds) => tx.wake_after(Duration::from_secs_f64(seconds)),
            Step::Send { run, session, query, regex, match_case } => {
                if let Some(link) = self.link_for(&session) {
                    link.off_thread(
                        move |base| {
                            let started = Instant::now();
                            let answer = classify(http::find_text(base, &session, &query, regex, match_case));
                            (answer, started.elapsed().as_secs_f64() * 1000.0)
                        },
                        move |s, _, (answer, took)| s.find_text.answered(run, answer, took),
                    );
                } else {
                    self.find_text.answered(run, Err(failure(FailureKind::Failed, "The search failed.")), 0.0);
                }
                changed = true;
            }
        }
        changed
    }
}
