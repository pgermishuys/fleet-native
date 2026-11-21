//! An item's page (`pages/GitHubWorkItemDetailPage.vue`, rows 10.x and 11.x of docs/parity/21-github.md): what it has loaded, the
//! words of its by-line and conversation, what the split button says and does, and what sending the failing checks to the session
//! sends. The page that draws it is `view::item`. Pure but for the `State` methods, which `keep` and the buttons call.
use super::item::Asked;
use super::items::{ItemDetail, ItemSummary, TimelineEntry, TimelineKind, item_pr_facts, item_resource_id};
use super::sessions::{ItemSession, ensure_loaded, sessions_for};
use super::{ID, Kind, Where, state, state_ref};
use crate::fleet::pr_state::{PrState, pr_state, pr_state_label, pr_words};
use crate::fleet::send_to_agent::FIX;
use crate::fleet::smart_links::{CheckRun, CheckState, ReviewThread, SmartLink, check_state, ci_status, format_fix_prompt, review_threads};
use crate::{Rail, State};

const FAILED: &str = "GitHub didn't answer.";

/// The page asked for: owner, repository, kind and number.
pub type Key = (String, String, Kind, u64);

/// What the page holds for the item it is on.
#[derive(Default)]
pub struct Page {
    pub asked: Option<Key>,
    pub detail: Option<ItemDetail>,
    pub error: Option<String>,
    pub loading: bool,
    pub refreshing: bool,
    /// The menu behind the split button's chevron is open.
    pub menu: bool,
    /// Each request takes the next number; an answer for an older one is dropped (`AbortController`).
    generation: u64,
}

impl Page {
    /// For a scene: the page as it looks with `detail` loaded.
    pub fn show(key: Key, detail: Option<ItemDetail>, loading: bool, error: Option<&str>) -> Page {
        Page { asked: Some(key), detail, error: error.map(str::to_string), loading, ..Page::default() }
    }
}

/// "pull request" or "issue".
pub fn noun(kind: Kind) -> &'static str {
    if kind == Kind::Pull { "pull request" } else { "issue" }
}

/// The state the pill shows (`state`).
pub fn item_state(summary: &ItemSummary, detail: Option<&ItemDetail>) -> PrState {
    if summary.kind != Kind::Pull {
        return if summary.state == "closed" { PrState::Closed } else { PrState::Open };
    }
    pr_state(&item_pr_facts(summary, detail))
}

/// The pill's words (`stateLabel`).
pub fn state_label(summary: &ItemSummary, detail: Option<&ItemDetail>) -> String {
    if summary.kind != Kind::Pull {
        return if summary.state == "closed" { "Closed" } else { "Open" }.into();
    }
    let state = item_state(summary, detail);
    if matches!(state, PrState::Open | PrState::Blocked) { pr_words(&item_pr_facts(summary, detail)) } else { pr_state_label(state).into() }
}

/// `timelineWords`.
pub fn timeline_words(entry: &TimelineEntry) -> &'static str {
    match entry.kind {
        TimelineKind::Comment => "commented",
        TimelineKind::Review => match entry.state.as_deref() {
            Some("APPROVED") => "approved",
            Some("CHANGES_REQUESTED") => "requested changes",
            Some("DISMISSED") => "had a review dismissed",
            _ => "reviewed",
        },
        TimelineKind::Merged => "merged this",
        TimelineKind::Closed => "closed this",
        TimelineKind::Reopened => "reopened this",
        TimelineKind::Ready => "marked this ready for review",
        TimelineKind::Draft => "marked this as a draft",
        TimelineKind::Referenced => "mentioned this in",
    }
}

/// `timelineTone`: the colour of the entry's dot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dot {
    Plain,
    Approved,
    Changes,
    Merged,
    Closed,
}

pub fn timeline_tone(entry: &TimelineEntry) -> Dot {
    match entry.kind {
        TimelineKind::Merged => Dot::Merged,
        TimelineKind::Closed => Dot::Closed,
        TimelineKind::Review => match entry.state.as_deref() {
            Some("APPROVED") => Dot::Approved,
            Some("CHANGES_REQUESTED") => Dot::Changes,
            _ => Dot::Plain,
        },
        _ => Dot::Plain,
    }
}

/// The state a mention's icon draws (`entry.state === 'merged' ? 'merged' : … 'closed' : 'open'`).
pub fn reference_state(entry: &TimelineEntry) -> PrState {
    match entry.state.as_deref() {
        Some("merged") => PrState::Merged,
        Some("closed") => PrState::Closed,
        _ => PrState::Open,
    }
}

/// The mention's address inside Fleet, `None` for another repository's (`openReference`: that one opens on GitHub).
pub fn reference_place(entry: &TimelineEntry, owner: &str, repo: &str) -> Option<Where> {
    let r = entry.reference.as_ref()?;
    if !r.repository.eq_ignore_ascii_case(&format!("{owner}/{repo}")) {
        return None;
    }
    Some(Where::Item { owner: owner.into(), repo: repo.into(), kind: r.kind, number: r.number })
}

/// The pull requests that mention an issue or pull request (`linkedPullRequests`).
pub fn linked_pull_requests(detail: &ItemDetail) -> Vec<&TimelineEntry> {
    detail
        .timeline
        .iter()
        .filter(|e| e.kind == TimelineKind::Referenced && e.reference.as_ref().is_some_and(|r| r.kind == Kind::Pull))
        .collect()
}

/// "{author} wants to merge" or "{author} merged" for a pull request with a branch, else "{author} opened this".
pub fn by_words(summary: &ItemSummary) -> String {
    let author = summary.author.clone().unwrap_or_default();
    if summary.kind == Kind::Pull && summary.head_ref.is_some() {
        format!("{author} {}", if summary.state == "merged" { "merged" } else { "wants to merge" })
    } else {
        format!("{author} opened this")
    }
}

/// "in 5 files": what follows the diff, none without a count.
pub fn files_words(detail: Option<&ItemDetail>) -> Option<String> {
    let n = detail?.changed_files.filter(|n| *n != 0)?;
    Some(format!("in {n} file{}", if n == 1 { "" } else { "s" }))
}

/// What the split button does when pressed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Run {
    Fix,
    Open,
    Start,
}

/// `primary`: the button's words and what it does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Primary {
    pub label: String,
    pub run: Run,
}

/// `fixLabel`: what there is to send, for an open pull request.
pub fn fix_label(summary: &ItemSummary, runs: usize, threads: usize) -> Option<&'static str> {
    if summary.kind != Kind::Pull || summary.state != "open" {
        return None;
    }
    match (runs > 0, threads > 0) {
        (true, true) => Some("Fix checks and address review"),
        (true, false) => Some("Fix failing checks"),
        (false, true) => Some("Address review"),
        (false, false) => None,
    }
}

pub fn primary(summary: &ItemSummary, has_session: bool, fix: Option<&'static str>, sent: bool) -> Primary {
    match (has_session, fix) {
        (true, Some(words)) => Primary { label: if sent { "Sent to the session" } else { words }.into(), run: Run::Fix },
        (true, None) => Primary { label: "Open session".into(), run: Run::Open },
        (false, _) => Primary {
            label: if summary.kind == Kind::Pull && summary.state == "open" { "Start a session on this branch" } else { "Start a session" }
                .into(),
            run: Run::Start,
        },
    }
}

/// What a session has to fix on a pull request: its link, the failing runs and the open threads.
pub struct Fix {
    pub link: SmartLink,
    pub runs: Vec<CheckRun>,
    pub threads: Vec<ReviewThread>,
}

/// `sessionLink`, `failingRuns` and `threads`: the first session's link to this item carries the failing runs and the threads.
pub fn fix_of(s: &State, session: &str, resource_id: &str) -> Option<Fix> {
    let links = &s.smart_links;
    let list = links.by_session.get(session).or_else(|| links.header_by_session.get(session))?;
    let link = list.iter().find(|l| l.resource_id.eq_ignore_ascii_case(resource_id))?.clone();
    let runs =
        ci_status(&link).map(|c| c.check_runs).unwrap_or_default().into_iter().filter(|r| check_state(r) == CheckState::Failing).collect();
    let threads = review_threads(&link);
    Some(Fix { link, runs, threads })
}

impl State {
    /// The item the page shows, when the GitHub rail is on one.
    pub fn github_item_shown(&self) -> Option<Key> {
        match (&state_ref(&self.places).at, self.active_rail()) {
            (Where::Item { owner, repo, kind, number }, Some(Rail::Plugin(ID))) => Some((owner.clone(), repo.clone(), *kind, *number)),
            _ => None,
        }
    }

    /// The sessions on the item on screen.
    pub fn github_item_sessions(&self) -> Vec<ItemSession> {
        match state_ref(&self.places).item.detail.as_ref() {
            Some(d) => sessions_for(self, &item_resource_id(&d.summary)),
            None => Vec::new(),
        }
    }

    /// `load`: asks for the item; the answer of an older ask is dropped. `refresh` keeps what is drawn and spins the button.
    pub fn github_item_load(&mut self, refresh: bool) {
        let Some((owner, repo, kind, number)) = state_ref(&self.places).item.asked.clone() else { return };
        let p = &mut state(&mut self.places).item;
        p.generation += 1;
        let generation = p.generation;
        p.error = None;
        if refresh {
            p.refreshing = true;
        } else {
            p.loading = true;
            p.detail = None;
        }
        if number == 0 {
            p.loading = false;
            p.refreshing = false;
            p.detail = None;
            p.error = Some(format!("That isn't a GitHub {} link.", noun(kind)));
            return;
        }
        let asked = self.github_item(&owner, &repo, number, move |s, _, answer| {
            let p = &mut state(&mut s.places).item;
            if p.generation != generation {
                return;
            }
            p.loading = false;
            p.refreshing = false;
            match answer {
                Ok(detail) => p.detail = Some(detail),
                Err(why) => {
                    p.detail = None;
                    p.error = Some(why);
                }
            }
        });
        if asked == Asked::Unreachable {
            let p = &mut state(&mut self.places).item;
            p.loading = false;
            p.refreshing = false;
            p.detail = None;
            p.error = Some(FAILED.into());
        }
    }

    /// `refresh` (the Refresh button), and Try again from the error.
    pub fn github_item_refresh(&mut self) {
        let failed = state_ref(&self.places).item.detail.is_none();
        self.github_item_load(!failed);
    }

    /// From the plugin's `keep`: a new owner, repository or number loads again (the `watch`). True when something drawn changed.
    pub(super) fn keep_github_item(&mut self) -> bool {
        let shown = self.github_item_shown();
        if shown == state_ref(&self.places).item.asked {
            return false;
        }
        let g = state(&mut self.places);
        // A page that goes leaves its answer behind: a later ask is for the new item, whose own answer replaces this.
        g.item = Page { asked: shown.clone(), generation: g.item.generation + 1, ..Page::default() };
        if shown.is_some() {
            ensure_loaded(self);
            self.github_item_load(false);
        }
        true
    }

    /// The split button's press.
    pub fn github_item_primary(&mut self, tx: &mut crate::Tx) {
        let Some(detail) = state_ref(&self.places).item.detail.clone() else { return };
        let sessions = self.github_item_sessions();
        let id = item_resource_id(&detail.summary);
        let fix = sessions.first().and_then(|first| fix_of(self, &first.id, &id));
        let label = fix.as_ref().and_then(|f| fix_label(&detail.summary, f.runs.len(), f.threads.len()));
        let sent = sessions.first().is_some_and(|f| self.agent_sends.is_sent(&f.id, FIX));
        match primary(&detail.summary, !sessions.is_empty(), label, sent).run {
            Run::Fix => self.github_item_fix(),
            Run::Open => {
                if let Some(first) = sessions.first() {
                    super::sessions::open_session(self, first, tx);
                }
            }
            Run::Start => self.github_item_start(),
        }
    }

    /// `sendFix`: the failing runs and open threads, to the first session, as `fix`.
    pub fn github_item_fix(&mut self) {
        let Some(detail) = state_ref(&self.places).item.detail.clone() else { return };
        let Some(first) = self.github_item_sessions().into_iter().next() else { return };
        let Some(fix) = fix_of(self, &first.id, &item_resource_id(&detail.summary)) else { return };
        let text = format_fix_prompt(&fix.link, &fix.runs, &fix.threads);
        self.send_to_session(&first.id, FIX, text);
    }

    /// `start`: the New session page with the item attached, its body with it.
    pub fn github_item_start(&mut self) {
        let Some(detail) = state_ref(&self.places).item.detail.clone() else { return };
        super::sessions::start_session(self, &detail.summary, detail.body.as_deref());
    }

    /// A press on a mention: its page in Fleet when it is in this repository, else GitHub.
    pub fn github_item_open_reference(&mut self, entry: &TimelineEntry, tx: &mut crate::Tx) {
        let Some((owner, repo, _, _)) = state_ref(&self.places).item.asked.clone() else { return };
        match reference_place(entry, &owner, &repo) {
            Some(at) => state(&mut self.places).at = at,
            None => {
                if let Some(url) = &entry.url {
                    let _ = tx.open_url(url);
                }
            }
        }
    }
}
