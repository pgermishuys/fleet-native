//! Notices: Fleet's quiet, one-at-a-time cards (`stores/notices.ts`, and the "quiet moment" of `NoticeCard.vue`).
//!
//! A notice opens as a small card once, then settles into a chip in the status bar that stays until whatever posted it
//! removes it. Which ids have been shown is remembered in the settings file, so the same notice never interrupts twice,
//! even across restarts. Plain Rust: every time here is seconds on the app's clock (`State::clock`), passed in, so a test
//! moves time by handing in a later number. The card's look is piece L, the chips' M.
//!
//! Fleet's notices carry closures (`run`, `onChipClick`, `onEscape`). Here each is a name the notice's source hears
//! when it runs, so a notice stays plain data.

/// How long a card stays before it settles into its chip (`NOTICE_HOLD_MS`).
pub const HOLD_SECS: f64 = 8.0;
/// A card waits this long after the last keystroke in a text field (`NOTICE_TYPING_QUIET_MS`).
pub const TYPING_QUIET_SECS: f64 = 2.5;
/// And this long after Fleet opens, so it never lands on top of the first paint (`NOTICE_STARTUP_QUIET_MS`).
pub const STARTUP_QUIET_SECS: f64 = 3.0;
/// How often a waiting notice looks for its quiet moment (`WAIT_POLL_MS`).
pub const WAIT_POLL_SECS: f64 = 0.5;
/// The settings file keeps this many shown ids (`SHOWN_LIMIT`).
pub const SHOWN_LIMIT: usize = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tone {
    #[default]
    Accent,
    /// Tints the icon amber, for a notice that asks before something stops work.
    Warn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ActionTone {
    Primary,
    /// A red wash, for an action that stops work.
    Danger,
    #[default]
    Quiet,
}

/// An icon from `src/icons.rs`, compared by which one it is.
#[derive(Debug, Clone, Copy)]
pub struct IconRef(pub &'static rune::Icon);

impl PartialEq for IconRef {
    fn eq(&self, other: &IconRef) -> bool {
        std::ptr::eq(self.0, other.0)
    }
}

impl Eq for IconRef {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Action {
    pub label: String,
    pub icon: Option<IconRef>,
    pub tone: ActionTone,
    /// What the notice's source hears when it is pressed.
    pub run: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    Highlight,
    New,
    /// Marks the line green; the others use the accent.
    Fixed,
}

/// One line of a card's list (What's new).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub text: String,
    /// A quiet label before the text: the part of Fleet it's about.
    pub label: Option<String>,
    pub kind: Option<ItemKind>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub label: String,
    pub href: String,
    /// With a name, the link is handled in Fleet and the card settles.
    pub run: Option<String>,
}

/// What a notice's card shows (`NoticeContent`), then what the store does with it (`Notice`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Notice {
    /// Stable key. A notice whose id has been shown before comes back settled, as its chip, never as a card.
    pub id: String,
    pub title: String,
    pub body: Option<String>,
    /// The card's default is the circle-arrow-up.
    pub icon: Option<IconRef>,
    pub tone: Tone,
    pub actions: Vec<Action>,
    pub items: Vec<Item>,
    /// A line under the list ("+2 more fixes").
    pub more: Option<String>,
    pub link: Option<Link>,
    /// What the status-bar chip says once the card settles. Without one, the notice goes away when it settles.
    pub chip: Option<String>,
    /// Only ever a chip: it never opens as a card by itself.
    pub quiet: bool,
    /// Goes away this long after it was posted.
    pub expires_secs: Option<f64>,
    /// Clicking the chip tells the source this instead of opening the card.
    pub on_chip_click: Option<String>,
    /// How long the card stays before it settles, instead of [`HOLD_SECS`].
    pub hold_secs: Option<f64>,
    /// Esc on the card tells the source this before the card settles.
    pub on_escape: Option<String>,
    /// Shows the time left as a bar that drains, for a card whose action (Undo) only lasts that long.
    pub countdown: bool,
}

impl Notice {
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Notice {
        Notice { id: id.into(), title: title.into(), ..Notice::default() }
    }

    pub fn chip(mut self, text: impl Into<String>) -> Notice {
        self.chip = Some(text.into());
        self
    }

    pub fn quiet(mut self) -> Notice {
        self.quiet = true;
        self
    }

    pub fn expires(mut self, secs: f64) -> Notice {
        self.expires_secs = Some(secs);
        self
    }
}

#[derive(Debug, Default, Clone)]
pub struct Notices {
    notices: Vec<Notice>,
    /// When each notice goes away, on the app clock, by id.
    expiry: Vec<(String, f64)>,
    /// Notices waiting to open as a card, oldest first.
    waiting: Vec<String>,
    open: Option<String>,
    /// The open card stays until it's closed: the user opened it from its chip, or it's asking them something.
    pinned: bool,
    /// Ids that have been a card, oldest first, at most [`SHOWN_LIMIT`].
    shown: Vec<String>,
    shown_changed: bool,
    hold: Hold,
    /// The pointer is over the open card.
    hovered: bool,
    /// What the card's buttons, link and Esc told their source, oldest first, until the source takes it.
    heard: Vec<String>,
}

/// How long the open card has left before it settles (`startHold`, `pauseHold`, `resumeHold`).
#[derive(Debug, Default, Clone)]
struct Hold {
    id: Option<String>,
    total: f64,
    /// Seconds left as of `since`; while paused, as of the pause.
    left: f64,
    since: f64,
    held: bool,
}

impl Notices {
    pub fn notices(&self) -> &[Notice] {
        &self.notices
    }

    pub fn get(&self, id: &str) -> Option<&Notice> {
        self.notices.iter().find(|n| n.id == id)
    }

    pub fn has(&self, id: &str) -> bool {
        self.get(id).is_some()
    }

    /// The open card.
    pub fn open(&self) -> Option<&Notice> {
        self.open.as_deref().and_then(|id| self.get(id))
    }

    pub fn pinned(&self) -> bool {
        self.pinned
    }

    /// The oldest notice waiting for its card.
    pub fn next_waiting(&self) -> Option<&str> {
        self.waiting.first().map(String::as_str)
    }

    /// The chips: a notice with a chip text that isn't the open card and isn't still waiting for its card (it arrives as
    /// a card first).
    pub fn chips(&self) -> Vec<&Notice> {
        self.notices
            .iter()
            .filter(|n| n.chip.is_some() && self.open.as_deref() != Some(n.id.as_str()) && !self.waiting.contains(&n.id))
            .collect()
    }

    // ── what is remembered between runs ──────────────────────────────────────

    /// Takes the ids read from the settings file (`readShown`). Anything shown before the file came back stays.
    pub fn adopt_shown(&mut self, stored: &[String]) {
        let mut all: Vec<String> = stored.iter().filter(|id| !self.shown.contains(id)).cloned().collect();
        all.append(&mut self.shown);
        self.shown = all;
        self.trim_shown();
        // A notice posted before the file was read and already waiting has been shown on an earlier run.
        let shown = &self.shown;
        let quiet: Vec<String> = self.waiting.iter().filter(|id| shown.contains(id)).cloned().collect();
        self.waiting.retain(|id| !quiet.contains(id));
    }

    pub fn shown(&self) -> &[String] {
        &self.shown
    }

    /// The ids to write when they changed since the last call.
    pub fn take_shown_save(&mut self) -> Option<Vec<String>> {
        std::mem::take(&mut self.shown_changed).then(|| self.shown.clone())
    }

    fn mark_shown(&mut self, id: &str) {
        if self.shown.iter().any(|s| s == id) {
            return;
        }
        self.shown.push(id.to_string());
        self.trim_shown();
        self.shown_changed = true;
    }

    fn trim_shown(&mut self) {
        if self.shown.len() > SHOWN_LIMIT {
            self.shown.drain(..self.shown.len() - SHOWN_LIMIT);
        }
    }

    // ── the store ────────────────────────────────────────────────────────────

    /// Adds a notice, or replaces the one with the same id in place. A new one waits for its card, unless it is quiet or
    /// its id was shown before: those go straight to a chip.
    pub fn post(&mut self, notice: Notice, now: f64) {
        if let Some(existing) = self.notices.iter_mut().find(|n| n.id == notice.id) {
            *existing = notice;
            return;
        }
        if let Some(secs) = notice.expires_secs {
            self.expiry.push((notice.id.clone(), now + secs));
        }
        if !notice.quiet && !self.shown.contains(&notice.id) {
            self.waiting.push(notice.id.clone());
        }
        self.notices.push(notice);
    }

    /// Changes what a card says without closing it (a confirmation step, for one). An open card then stays open.
    pub fn update(&mut self, id: &str, change: impl FnOnce(&mut Notice)) {
        if let Some(n) = self.notices.iter_mut().find(|n| n.id == id) {
            change(n);
        }
        if self.open.as_deref() == Some(id) {
            self.pinned = true;
        }
    }

    /// Opens the next waiting card. The host calls this at a quiet moment.
    pub fn open_next(&mut self) {
        if self.open.is_some() || self.waiting.is_empty() {
            return;
        }
        let id = self.waiting.remove(0);
        self.mark_shown(&id);
        self.open = Some(id);
        self.pinned = false;
    }

    /// Opens a notice's card again, from its chip or the app menu. It stays open until closed.
    pub fn reopen(&mut self, id: &str) {
        if !self.has(id) {
            return;
        }
        self.waiting.retain(|w| w != id);
        self.pinned = true;
        self.open = Some(id.to_string());
        self.mark_shown(id);
    }

    /// Closes the open card: it becomes its chip, or goes away if it has none.
    pub fn settle(&mut self) {
        let Some(id) = self.open.take() else { return };
        self.pinned = false;
        if self.get(&id).is_some_and(|n| n.chip.is_none()) {
            self.remove(&id);
        }
    }

    pub fn remove(&mut self, id: &str) {
        self.notices.retain(|n| n.id != id);
        self.expiry.retain(|(e, _)| e != id);
        self.waiting.retain(|w| w != id);
        if self.open.as_deref() == Some(id) {
            self.open = None;
            self.pinned = false;
        }
    }

    /// Removes every notice whose id starts with `prefix`, except `keep`.
    pub fn remove_where(&mut self, prefix: &str, keep: Option<&str>) {
        let gone: Vec<String> =
            self.notices.iter().filter(|n| n.id.starts_with(prefix) && Some(n.id.as_str()) != keep).map(|n| n.id.clone()).collect();
        for id in gone {
            self.remove(&id);
        }
    }

    // ── the open card: its hold, and what it tells its source ────────────────

    /// The pointer came onto or left the open card. While it is over it, the card doesn't settle by itself.
    pub fn set_hover(&mut self, inside: bool) {
        self.hovered = inside;
    }

    /// Settles a card that has been open its hold, unless it is pinned or held (hovered, or focus is inside it). True
    /// when it settled. A card that opens starts its hold now; a pinned card has none (`NoticeCard.vue`, "Hold").
    pub fn keep_hold(&mut self, now: f64, focus_inside: bool) -> bool {
        let Some((id, total)) = self.open().map(|n| (n.id.clone(), n.hold_secs.unwrap_or(HOLD_SECS))) else {
            self.hold = Hold::default();
            return false;
        };
        if self.hold.id.as_deref() != Some(id.as_str()) {
            self.hold = Hold { id: Some(id), total, left: total, since: now, held: false };
            self.hovered = false;
        }
        if self.pinned {
            return false;
        }
        let want = self.hovered || focus_inside;
        if want && !self.hold.held {
            // At least a second is left after a pause, so a card you glanced at doesn't vanish as you let go.
            self.hold.left = (self.hold.left - (now - self.hold.since)).max(1.0);
            self.hold.held = true;
        } else if !want && self.hold.held {
            self.hold.held = false;
            self.hold.since = now;
        }
        if !self.hold.held && now - self.hold.since >= self.hold.left {
            self.settle();
            return true;
        }
        false
    }

    /// Seconds until the open card settles by itself, for the clock to wake at; none when it is pinned or held.
    pub fn hold_left(&self, now: f64) -> Option<f64> {
        (self.open().is_some() && !self.pinned && !self.hold.held && self.hold.id == self.open)
            .then_some(self.hold.left - (now - self.hold.since))
    }

    /// The share of the countdown bar still showing, for a card that has one and isn't pinned.
    pub fn countdown(&self, now: f64) -> Option<f32> {
        let n = self.open()?;
        if !n.countdown || self.pinned {
            return None;
        }
        if self.hold.id.as_deref() != Some(n.id.as_str()) {
            return Some(1.0);
        }
        let left = if self.hold.held { self.hold.left } else { self.hold.left - (now - self.hold.since) };
        Some((left / self.hold.total.max(f64::EPSILON)).clamp(0.0, 1.0) as f32)
    }

    /// Whether the countdown is held, so it stops drawing.
    pub fn held(&self) -> bool {
        self.hold.held
    }

    /// A button, link or Esc on the card: its source hears `run`.
    pub fn tell(&mut self, run: &str) {
        self.heard.push(run.to_string());
    }

    /// What the source has been told since it last asked.
    pub fn take_heard(&mut self) -> Vec<String> {
        std::mem::take(&mut self.heard)
    }

    /// Esc on the card: the source hears `on_escape`, then the card settles (`onCardKeydown`).
    pub fn escape(&mut self) {
        if let Some(run) = self.open().and_then(|n| n.on_escape.clone()) {
            self.tell(&run);
        }
        self.settle();
    }

    /// Takes away notices whose time has come; true when any went.
    pub fn expire(&mut self, now: f64) -> bool {
        let due: Vec<String> = self.expiry.iter().filter(|(_, at)| now >= *at).map(|(id, _)| id.clone()).collect();
        for id in &due {
            self.remove(id);
        }
        !due.is_empty()
    }

    /// The soonest a notice expires, for the clock to wake at.
    pub fn next_expiry(&self) -> Option<f64> {
        self.expiry.iter().map(|(_, at)| *at).reduce(f64::min)
    }
}

/// Whether this is a good moment for a card to open: not in the first seconds, not while typing, and only while someone
/// is looking at Fleet (`isQuietMoment`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quiet {
    /// When Fleet opened, on the app clock.
    pub started: f64,
    /// The last key typed into a text field.
    pub last_typed: Option<f64>,
    pub focused: bool,
    pub visible: bool,
}

impl Default for Quiet {
    fn default() -> Quiet {
        Quiet { started: 0.0, last_typed: None, focused: true, visible: true }
    }
}

impl Quiet {
    pub fn typed(&mut self, now: f64) {
        self.last_typed = Some(now);
    }

    pub fn is_quiet(&self, now: f64) -> bool {
        now - self.started >= STARTUP_QUIET_SECS
            && self.last_typed.is_none_or(|t| now - t >= TYPING_QUIET_SECS)
            && self.focused
            && self.visible
    }
}
