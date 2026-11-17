//! Dreaming, as plain data (`lib/dreaming.ts`): what a dream is, the calls Settings → Memory's Dreaming panel makes under
//! `/api/memory/dreams`, how their answers are read, and the words the panel uses (kind labels, the summary, the harness it
//! dreams on). No Rune and no I/O: `dreaming_actions.rs` sends the calls and keeps the panel's state, [`DreamingPage`].
use super::memory::{List, Request};
use crate::fleet::http;
use rune::TextEdit;
use serde_json::{Value, json};
use std::collections::BTreeSet;

/// How often a dream that is reading or running is asked about, in seconds (`DREAM_POLL_MS`).
pub const POLL_SECS: f64 = 3.0;

/// `maxlength` of the text area that edits a flagged note.
pub const MAX_FLAG_CHARS: usize = 300;

/// A session a dream read or points at (`DreamSessionRef`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionRef {
    pub id: String,
    pub title: String,
    pub harness_type: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Added,
    Rewritten,
    Confirmed,
    Merged,
    Retired,
    Kept,
    Left,
    Skipped,
    Flagged,
}

impl Kind {
    fn parse(word: &str) -> Option<Kind> {
        Some(match word {
            "added" => Kind::Added,
            "rewritten" => Kind::Rewritten,
            "confirmed" => Kind::Confirmed,
            "merged" => Kind::Merged,
            "retired" => Kind::Retired,
            "kept" => Kind::Kept,
            "left" => Kind::Left,
            "skipped" => Kind::Skipped,
            "flagged" => Kind::Flagged,
            _ => return None,
        })
    }

    /// `dreamKindLabel`.
    pub fn label(self) -> &'static str {
        match self {
            Kind::Added => "Added",
            Kind::Rewritten => "Rewritten",
            Kind::Confirmed => "Confirmed",
            Kind::Merged => "Merged",
            Kind::Retired => "Retired",
            Kind::Kept => "Kept for good",
            Kind::Left => "Left as is",
            Kind::Skipped => "Not changed",
            Kind::Flagged => "Looks out of date",
        }
    }

    /// The words `describeDreamChanges` counts a kind under.
    fn count_word(self) -> &'static str {
        match self {
            Kind::Added => "added",
            Kind::Rewritten => "rewritten",
            Kind::Confirmed => "confirmed",
            Kind::Merged => "merged",
            Kind::Retired => "retired",
            _ => "kept for good",
        }
    }
}

/// A note a change replaced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Replaced {
    pub id: String,
    pub text: String,
}

/// One thing a dream did, or decided not to do (`DreamChangeView`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub id: String,
    pub kind: Kind,
    pub list: Option<List>,
    pub note_id: Option<String>,
    pub text: Option<String>,
    pub replaced: Vec<Replaced>,
    pub why: String,
    pub sessions: Vec<SessionRef>,
    pub of_sessions: i64,
    pub quote: Option<String>,
    pub reason: Option<String>,
    /// Undone; for a flag, dismissed with Keep.
    pub undone: bool,
    /// What the dream read to check the note: the file or commit, and what it showed.
    pub evidence: Option<String>,
    /// For a confirmed note: how it is now known to hold.
    pub how: Option<String>,
}

impl Change {
    /// The Why button, the evidence under it and the flag's evidence block show only with something to say.
    pub fn has_evidence(&self) -> bool {
        !self.sessions.is_empty()
            || self.quote.as_deref().is_some_and(|q| !q.is_empty())
            || self.evidence.as_deref().is_some_and(|e| !e.is_empty())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Reading,
    Running,
    Done,
    Failed,
}

/// One dream (`DreamView`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dream {
    pub id: String,
    pub repository: String,
    pub repository_name: String,
    pub session_id: Option<String>,
    pub harness_type: Option<String>,
    pub status: Status,
    pub error: Option<String>,
    pub started: String,
    pub finished: Option<String>,
    pub read_sessions: Vec<SessionRef>,
    pub changes: Vec<Change>,
    /// The night that started it; none for Dream now.
    pub run_id: Option<String>,
    pub tokens: Option<i64>,
    pub since: Option<String>,
}

// ─── Reading answers ────────────────────────────────────────────────────────

fn text(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}

fn session_refs(v: &Value, key: &str) -> Vec<SessionRef> {
    v.get(key)
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|s| {
                    Some(SessionRef {
                        id: text(s, "id")?,
                        title: text(s, "title").unwrap_or_default(),
                        harness_type: text(s, "harnessType"),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_change(v: &Value) -> Option<Change> {
    Some(Change {
        id: text(v, "id").filter(|id| !id.is_empty())?,
        kind: Kind::parse(v.get("kind")?.as_str()?)?,
        list: match v.get("list").and_then(Value::as_str) {
            Some("machine") => Some(List::Machine),
            Some(_) => Some(List::Repository),
            None => None,
        },
        note_id: text(v, "noteId").filter(|id| !id.is_empty()),
        text: text(v, "text"),
        replaced: v
            .get("replaced")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter().map(|r| Replaced { id: text(r, "id").unwrap_or_default(), text: text(r, "text").unwrap_or_default() }).collect()
            })
            .unwrap_or_default(),
        why: text(v, "why").unwrap_or_default(),
        sessions: session_refs(v, "sessions"),
        of_sessions: v.get("ofSessions").and_then(Value::as_f64).map_or(0, |n| n as i64),
        quote: text(v, "quote"),
        reason: text(v, "reason"),
        undone: v.get("undone").and_then(Value::as_bool) == Some(true),
        evidence: text(v, "evidence"),
        how: text(v, "how"),
    })
}

pub fn parse_dream(v: &Value) -> Option<Dream> {
    Some(Dream {
        id: text(v, "id").filter(|id| !id.is_empty())?,
        repository: text(v, "repository").unwrap_or_default(),
        repository_name: text(v, "repositoryName").unwrap_or_default(),
        session_id: text(v, "sessionId").filter(|id| !id.is_empty()),
        harness_type: text(v, "harnessType"),
        status: match v.get("status").and_then(Value::as_str)? {
            "reading" => Status::Reading,
            "running" => Status::Running,
            "failed" => Status::Failed,
            _ => Status::Done,
        },
        error: text(v, "error"),
        started: text(v, "started").unwrap_or_default(),
        finished: text(v, "finished").filter(|f| !f.is_empty()),
        read_sessions: session_refs(v, "readSessions"),
        changes: v.get("changes").and_then(Value::as_array).map(|a| a.iter().filter_map(parse_change).collect()).unwrap_or_default(),
        run_id: text(v, "runId").filter(|r| !r.is_empty()),
        tokens: v.get("tokens").and_then(Value::as_f64).map(|n| n as i64).filter(|n| *n != 0),
        since: text(v, "since"),
    })
}

/// `GET /api/memory/dreams` → `{dreams: DreamView[]}`, newest first.
pub fn parse_dreams(v: &Value) -> Option<Vec<Dream>> {
    Some(v.get("dreams")?.as_array()?.iter().filter_map(parse_dream).collect())
}

// ─── Requests ───────────────────────────────────────────────────────────────

const DREAMS_PATH: &str = "/api/memory/dreams";

pub fn list_dreams(repository: &str) -> Request {
    Request {
        method: "GET",
        path: format!("{DREAMS_PATH}?repository={}", http::escape(repository)),
        body: None,
        fallback: "Couldn't load the dreams.".into(),
    }
}

/// `harness` is left out of the body when there is none (`harnessType ? { harnessType } : {}`).
pub fn start_dream(repository: &str, harness: Option<&str>) -> Request {
    let mut body = json!({ "repository": repository });
    if let Some(h) = harness.filter(|h| !h.is_empty()) {
        body["harnessType"] = json!(h);
    }
    Request { method: "POST", path: DREAMS_PATH.into(), body: Some(body), fallback: "Couldn't start the dream.".into() }
}

/// Puts a change back, or for a flag marks it dealt with.
pub fn undo_change(dream_id: &str, change_id: &str) -> Request {
    Request {
        method: "POST",
        path: format!("{DREAMS_PATH}/{}/changes/{}/undo", http::escape(dream_id), http::escape(change_id)),
        body: None,
        fallback: "Couldn't undo the change.".into(),
    }
}

// ─── Words and groups ───────────────────────────────────────────────────────

pub fn is_busy(dream: Option<&Dream>) -> bool {
    dream.is_some_and(|d| matches!(d.status, Status::Reading | Status::Running))
}

/// `dreamListLabel`.
pub fn list_label(list: Option<List>) -> &'static str {
    if list == Some(List::Machine) { "This machine" } else { "Repository" }
}

/// `groupDreamChanges`: what was done first, then what it left as is, then what it saw in one session only.
#[derive(Default)]
pub struct Groups<'a> {
    pub applied: Vec<&'a Change>,
    pub flagged: Vec<&'a Change>,
    pub left: Vec<&'a Change>,
    pub skipped: Vec<&'a Change>,
}

pub fn group_changes(changes: &[Change]) -> Groups<'_> {
    let mut g = Groups::default();
    for c in changes {
        match c.kind {
            Kind::Flagged => g.flagged.push(c),
            Kind::Left => g.left.push(c),
            Kind::Skipped => g.skipped.push(c),
            _ => g.applied.push(c),
        }
    }
    g
}

impl Dream {
    pub fn groups(&self) -> Groups<'_> {
        group_changes(&self.changes)
    }

    /// Changes that are still in place (`appliedDreamChanges`).
    pub fn applied(&self) -> Vec<&Change> {
        self.groups().applied.into_iter().filter(|c| !c.undone).collect()
    }

    /// The notes a flag points at that nobody has dealt with (`openDreamFlags`).
    pub fn open_flags(&self) -> Vec<&Change> {
        self.groups().flagged.into_iter().filter(|c| !c.undone).collect()
    }

    /// Applied changes that altered a note: a confirmed note holds as it was, so it isn't counted (`changedDreamNotes`).
    pub fn changed(&self) -> usize {
        self.applied().iter().filter(|c| c.kind != Kind::Confirmed).count()
    }

    pub fn confirmed(&self) -> usize {
        self.applied().iter().filter(|c| c.kind == Kind::Confirmed).count()
    }

    /// "3 notes changed, 1 confirmed".
    pub fn changed_summary(&self) -> String {
        let (changed, confirmed) = (self.changed(), self.confirmed());
        let mut line = format!("{changed} {} changed", plural(changed, "note", "notes"));
        if confirmed > 0 {
            line += &format!(", {confirmed} confirmed");
        }
        line
    }

    /// The first line of the summary: "Last night" for a night's dream, else "Last dream".
    pub fn heading(&self) -> String {
        format!(
            "{} · {} · {}",
            if self.run_id.is_some() { "Last night" } else { "Last dream" },
            self.repository_name,
            self.changed_summary()
        )
    }

    /// `describeDreamChanges`: "3 sessions read overnight · 1 added, 2 rewritten, 1 looks out of date", the kinds in the
    /// order each first appears.
    pub fn describe(&self) -> String {
        let mut counts: Vec<(&'static str, usize)> = vec![];
        for change in self.applied() {
            let word = change.kind.count_word();
            match counts.iter_mut().find(|(w, _)| *w == word) {
                Some((_, n)) => *n += 1,
                None => counts.push((word, 1)),
            }
        }
        let mut parts: Vec<String> = counts.iter().map(|(word, n)| format!("{n} {word}")).collect();
        let flags = self.open_flags().len();
        if flags > 0 {
            parts.push(format!("{flags} looks out of date"));
        }
        let read = format!("{} {} read overnight", self.read_sessions.len(), plural(self.read_sessions.len(), "session", "sessions"));
        if parts.is_empty() { read } else { format!("{read} · {}", parts.join(", ")) }
    }
}

pub fn plural(n: usize, one: &'static str, many: &'static str) -> &'static str {
    if n == 1 { one } else { many }
}

/// "Title · Harness name" when the session names a harness, else its title (`sessionLabel`). `names` are `(type, display)`.
pub fn session_label(session: &SessionRef, names: &[(String, String)]) -> String {
    match session.harness_type.as_deref().filter(|h| !h.is_empty()) {
        Some(h) => {
            let name = names.iter().find(|(kind, _)| kind == h).map_or(h, |(_, display)| display.as_str());
            format!("{} · {name}", session.title)
        }
        None => session.title.clone(),
    }
}

/// The harness "Dream now" uses (`harnessType`): the one picked here, then the one nightly dreaming uses, each only while it is
/// enabled; else `opencode2`, else the first enabled one, else none. `enabled` are the enabled harness types in order.
pub fn harness_choice(chosen: Option<&str>, nightly: Option<&str>, enabled: &[String]) -> String {
    for wanted in [chosen, nightly].into_iter().flatten() {
        if !wanted.is_empty() && enabled.iter().any(|e| e == wanted) {
            return wanted.to_string();
        }
    }
    enabled.iter().find(|e| *e == "opencode2").or(enabled.first()).cloned().unwrap_or_default()
}

// ─── The panel's state ──────────────────────────────────────────────────────

/// What the Dreaming panel holds while Settings → Memory is open (`DreamingPanel.vue`'s refs). It lives inside the page's
/// state, so it goes with it when the page is left or the machine changes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DreamingPage {
    /// The newest dream of the repository shown.
    pub dream: Option<Dream>,
    pub error: Option<String>,
    /// The words of the last failed read, so the next good read clears only its own error (`loadError`).
    pub load_error: Option<String>,
    /// "Dream now" is on its way (`isStarting`).
    pub starting: bool,
    /// The change an undo, Keep, Forget or Save is on its way for (`undoing`); one at a time.
    pub undoing: Option<String>,
    /// Changes whose Why is open (`open`).
    pub open: BTreeSet<String>,
    pub show_skipped: bool,
    /// The flag being edited and what its text area holds (`editing`).
    pub editing: Option<String>,
    pub edit: TextEdit,
    /// The harness nightly dreaming uses, once piece S reads it (`nightlyHarness`).
    pub nightly_harness: Option<String>,
    /// Which read of the page and which repository the panel last asked for.
    pub asked: Option<(u64, Option<String>)>,
    /// A read is on its way, so a poll doesn't pile another on it.
    pub reading: bool,
    /// When a busy dream is next asked about.
    pub next_poll: Option<f64>,
    /// Whether the dream was busy when last looked at: the notes are read again when it stops.
    pub was_busy: bool,
}

// A text edit is equal to itself; `Eq` lets the page hold one.
impl Eq for DreamingPage {}

impl DreamingPage {
    pub fn busy(&self) -> bool {
        is_busy(self.dream.as_ref())
    }

    /// Takes a newer view of the dream, and says whether it has just stopped being busy (the notes are read again then).
    pub fn set_dream(&mut self, dream: Option<Dream>) -> bool {
        self.dream = dream;
        let busy = self.busy();
        let stopped = self.was_busy && !busy;
        self.was_busy = busy;
        if !busy {
            self.next_poll = None;
        }
        stopped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn change(id: &str, kind: &str) -> Value {
        json!({ "id": id, "kind": kind, "list": "repository", "noteId": format!("n-{id}"), "text": "A note.", "replaced": [], "why": "Seen twice.",
                "sessions": [], "ofSessions": 0, "quote": null, "reason": null, "undone": false })
    }

    fn dream(changes: Vec<Value>) -> Dream {
        parse_dream(
            &json!({ "id": "d1", "repository": "/work/lantern", "repositoryName": "lantern", "sessionId": null, "harnessType": "opencode2",
            "status": "done", "error": null, "started": "2026-10-20T03:00:00Z", "finished": "2026-10-20T03:09:00Z",
            "readSessions": [{ "id": "s1", "title": "One", "harnessType": null }, { "id": "s2", "title": "Two", "harnessType": null }],
            "changes": changes, "runId": "r1", "tokens": 18400, "since": null }),
        )
        .expect("a dream")
    }

    #[test]
    fn counts_changed_apart_from_confirmed() {
        let mut undone = change("c4", "added");
        undone["undone"] = json!(true);
        let d = dream(vec![change("c1", "added"), change("c2", "rewritten"), change("c3", "confirmed"), undone, change("c5", "flagged")]);
        assert_eq!((d.changed(), d.confirmed()), (2, 1));
        assert_eq!(d.changed_summary(), "2 notes changed, 1 confirmed");
        assert_eq!(d.heading(), "Last night · lantern · 2 notes changed, 1 confirmed");
        let none = dream(vec![]);
        assert_eq!(none.changed_summary(), "0 notes changed");
        assert_eq!(dream(vec![change("c1", "added")]).changed_summary(), "1 note changed");
    }

    #[test]
    fn groups_applied_then_flagged_left_and_skipped() {
        let d =
            dream(vec![change("a", "skipped"), change("b", "added"), change("c", "left"), change("d", "flagged"), change("e", "merged")]);
        let g = d.groups();
        let ids = |v: &[&Change]| v.iter().map(|c| c.id.clone()).collect::<Vec<_>>();
        assert_eq!(ids(&g.applied), ["b", "e"]);
        assert_eq!((ids(&g.flagged), ids(&g.left), ids(&g.skipped)), (vec!["d".to_string()], vec!["c".to_string()], vec!["a".to_string()]));
    }

    #[test]
    fn describes_what_a_night_did_in_the_order_it_first_happened() {
        let d = dream(vec![
            change("a", "rewritten"),
            change("b", "added"),
            change("c", "rewritten"),
            change("d", "kept"),
            change("e", "flagged"),
        ]);
        assert_eq!(d.describe(), "2 sessions read overnight · 2 rewritten, 1 added, 1 kept for good, 1 looks out of date");
        assert_eq!(dream(vec![]).describe(), "2 sessions read overnight");
        let mut one = dream(vec![]);
        one.read_sessions.truncate(1);
        assert_eq!(one.describe(), "1 session read overnight");
    }

    #[test]
    fn labels_every_kind_and_both_lists() {
        let kinds = ["added", "rewritten", "confirmed", "merged", "retired", "kept", "left", "skipped", "flagged"];
        let labels: Vec<&str> = kinds.iter().map(|k| Kind::parse(k).unwrap().label()).collect();
        assert_eq!(
            labels,
            ["Added", "Rewritten", "Confirmed", "Merged", "Retired", "Kept for good", "Left as is", "Not changed", "Looks out of date"]
        );
        assert_eq!(list_label(Some(List::Machine)), "This machine");
        assert_eq!(list_label(Some(List::Repository)), "Repository");
        assert!(Kind::parse("surprise").is_none());
    }

    #[test]
    fn picks_the_harness_to_dream_on() {
        let enabled = vec!["claude-code".to_string(), "opencode2".to_string()];
        assert_eq!(harness_choice(Some("claude-code"), Some("opencode2"), &enabled), "claude-code");
        assert_eq!(harness_choice(Some("gone"), Some("claude-code"), &enabled), "claude-code");
        assert_eq!(harness_choice(None, None, &enabled), "opencode2");
        assert_eq!(harness_choice(Some("gone"), None, &["pi".to_string()]), "pi");
        assert_eq!(harness_choice(None, None, &[]), "");
    }

    #[test]
    fn busy_dreams_are_reading_or_running() {
        let mut d = dream(vec![]);
        assert!(!is_busy(Some(&d)));
        d.status = Status::Running;
        assert!(is_busy(Some(&d)));
        d.status = Status::Reading;
        assert!(is_busy(Some(&d)));
        assert!(!is_busy(None));
    }

    #[test]
    fn requests_carry_the_repository_and_leave_out_an_empty_harness() {
        assert_eq!(
            start_dream("/work/lantern", Some("claude-code")).body,
            Some(json!({ "repository": "/work/lantern", "harnessType": "claude-code" }))
        );
        assert_eq!(start_dream("/work/lantern", Some("")).body, Some(json!({ "repository": "/work/lantern" })));
        let list = list_dreams("/work/lantern");
        assert_eq!((list.method, list.fallback.as_str()), ("GET", "Couldn't load the dreams."));
        assert!(list.path.starts_with("/api/memory/dreams?repository=") && !list.path.contains("/work/"));
        let undo = undo_change("d 1", "c1");
        assert_eq!((undo.method, undo.path.as_str()), ("POST", "/api/memory/dreams/d%201/changes/c1/undo"));
        assert_eq!(undo.fallback, "Couldn't undo the change.");
        assert_eq!(start_dream("/r", None).fallback, "Couldn't start the dream.");
    }

    #[test]
    fn reads_a_dream_and_skips_what_it_cannot() {
        let d = dream(vec![change("a", "added"), json!({ "id": "x", "kind": "mystery" }), json!({ "kind": "added" })]);
        assert_eq!(d.changes.len(), 1);
        assert_eq!(d.tokens, Some(18400));
        assert_eq!(d.run_id.as_deref(), Some("r1"));
        assert!(parse_dreams(&json!({ "dreams": [{ "id": "d1", "status": "done" }, { "nope": 1 }] })).is_some_and(|v| v.len() == 1));
        assert!(parse_dreams(&json!({})).is_none());
    }

    #[test]
    fn a_session_says_its_harness_by_display_name() {
        let names = vec![("claude-code".to_string(), "Claude Code".to_string())];
        let with = SessionRef { id: "s".into(), title: "Fix it".into(), harness_type: Some("claude-code".into()) };
        assert_eq!(session_label(&with, &names), "Fix it · Claude Code");
        let unknown = SessionRef { harness_type: Some("pi".into()), ..with.clone() };
        assert_eq!(session_label(&unknown, &names), "Fix it · pi");
        assert_eq!(session_label(&SessionRef { harness_type: None, ..with }, &names), "Fix it");
    }

    #[test]
    fn a_dream_that_stops_being_busy_says_so_once() {
        let mut page = DreamingPage::default();
        let mut running = dream(vec![]);
        running.status = Status::Running;
        assert!(!page.set_dream(Some(running)));
        page.next_poll = Some(3.0);
        assert!(page.set_dream(Some(dream(vec![]))));
        assert!(page.next_poll.is_none());
        assert!(!page.set_dream(Some(dream(vec![]))));
    }
}
