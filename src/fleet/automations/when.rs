//! The When menu's state and what its rows do (`AutomationWhenPicker.vue`, `setWhen` of `AutomationComposer.vue`; rows
//! 8.2-8.9): the menu open or not, the date, time and cron boxes, the cron's error, and the event types it offers (the
//! server's catalog once it answers, the built-in list before that or when it fails). Every choice goes through
//! [`super::composer::ComposerState::set_when`], which takes over from the words and drops "Just once".
use super::composer::ComposerState;
use super::schedule::{Hm, Tz, When, cron_of, hm, local_date, next_run, tomorrow, weekday, with_time};
use crate::{State, Tx};
use rune::TextEdit;

/// `Open::Popover` name of the menu.
pub const WHEN_POPOVER: &str = "automation-when";
/// The chip's key (`automation-when-chip`).
pub const WHEN_CHIP: &str = "automation-when-chip";
pub const CRON_ERROR: &str = "A cron has five parts: minute hour day month weekday.";

/// The time a schedule with none starts at.
const NINE: Hm = (9, 0);

/// The menu's boxes and the cron's error. The boxes are filled from the schedule when the menu opens.
#[derive(Debug, Default)]
pub struct WhenMenu {
    pub date: TextEdit,
    pub time: TextEdit,
    /// `cronDraft`.
    pub cron: TextEdit,
    /// `cronError`.
    pub cron_error: Option<String>,
    /// The cron box has had the keys since the menu opened; losing them then is the blur that applies it.
    cron_seen_focus: bool,
}

/// The four Repeat rows (`chooseRepeat`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Repeat {
    Daily,
    Weekdays,
    Weekly,
    Hourly,
}

impl Repeat {
    pub const ALL: [Repeat; 4] = [Repeat::Daily, Repeat::Weekdays, Repeat::Weekly, Repeat::Hourly];

    pub fn key(self) -> &'static str {
        match self {
            Repeat::Daily => "automation-when-daily",
            Repeat::Weekdays => "automation-when-weekdays",
            Repeat::Weekly => "automation-when-weekly",
            Repeat::Hourly => "automation-when-hourly",
        }
    }

    /// The schedule this row makes from the one there is: the time kept, the weekly one on its own day (else today).
    pub fn make(self, when: Option<&When>, today: u8) -> When {
        let t = time_of(when);
        match self {
            Repeat::Daily => When::Weekly { days: vec![0, 1, 2, 3, 4, 5, 6], t, ambiguous: false },
            Repeat::Weekdays => When::Weekly { days: vec![1, 2, 3, 4, 5], t, ambiguous: false },
            Repeat::Weekly => When::Weekly { days: vec![weekly_day(when, today)], t, ambiguous: false },
            Repeat::Hourly => When::Hours { n: 1 },
        }
    }

    /// The row's check: the schedule is this kind.
    pub fn is(self, when: Option<&When>) -> bool {
        match (self, when) {
            (Repeat::Daily, Some(When::Weekly { days, .. })) => days.len() == 7,
            (Repeat::Weekdays, Some(When::Weekly { days, .. })) => days.as_slice() == [1, 2, 3, 4, 5],
            (Repeat::Weekly, Some(When::Weekly { days, .. })) => days.len() == 1,
            (Repeat::Hourly, Some(When::Hours { .. })) => true,
            _ => false,
        }
    }
}

/// `time`: the schedule's time of day, else 09:00.
pub fn time_of(when: Option<&When>) -> Hm {
    match when {
        Some(When::Once { t, .. } | When::Weekly { t, .. } | When::Monthly { t, .. }) => *t,
        _ => NINE,
    }
}

/// `weeklyDay`: the weekly row's day is the schedule's own when it has one day, otherwise today.
pub fn weekly_day(when: Option<&When>, today: u8) -> u8 {
    match when {
        Some(When::Weekly { days, .. }) if days.len() == 1 => days[0],
        _ => today,
    }
}

/// `onceDate`: the date a one-off runs on, empty for anything else.
pub fn once_date(when: Option<&When>, from: i64, tz: Tz) -> String {
    match when {
        Some(w @ When::Once { .. }) => next_run(w, from, tz).map(|d| local_date(d, tz)).unwrap_or_default(),
        _ => String::new(),
    }
}

/// `chooseOnce`: a one-off on the date it had, else tomorrow, at the time.
pub fn once(when: Option<&When>, from: i64, tz: Tz) -> When {
    let date = Some(once_date(when, from, tz)).filter(|d| !d.is_empty()).unwrap_or_else(|| tomorrow(from, tz));
    When::Once { t: time_of(when), day: None, rel: None, date: Some(date) }
}

/// `HH:MM` (one or two digits for the hour), as a time field gives it; None for anything else.
pub fn parse_hm(typed: &str) -> Option<Hm> {
    let (h, m) = typed.trim().split_once(':')?;
    let ok = |s: &str, max: i64| {
        (!s.is_empty() && s.len() <= 2 && s.bytes().all(|b| b.is_ascii_digit())).then(|| s.parse().ok()).flatten().filter(|n| *n < max)
    };
    Some((ok(h, 24)?, ok(m, 60).filter(|_| m.len() == 2)?))
}

/// `setTime`: the same schedule at the time typed (a weekly one stops asking "every week or once?"); with no schedule that
/// has a time, a weekly one on today. None while the box doesn't hold a time.
pub fn set_time(when: Option<&When>, typed: &str, today: u8) -> Option<When> {
    let t = parse_hm(typed)?;
    let base = match when {
        Some(w @ (When::Once { .. } | When::Weekly { .. } | When::Monthly { .. })) => w.clone(),
        _ => When::Weekly { days: vec![today], t: NINE, ambiguous: false },
    };
    Some(with_time(&base, t))
}

/// `setDate`: a one-off on the date typed (`YYYY-MM-DD`), at the time. None while the box doesn't hold one.
pub fn set_date(when: Option<&When>, typed: &str) -> Option<When> {
    let typed = typed.trim();
    let b = typed.as_bytes();
    let shaped = b.len() == 10 && b[4] == b'-' && b[7] == b'-' && b.iter().enumerate().all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit());
    shaped.then(|| When::Once { t: time_of(when), day: None, rel: None, date: Some(typed.to_string()) })
}

/// `applyCron`: spaces folded; nothing for an empty box; five parts or the error.
pub fn apply_cron(typed: &str) -> Result<Option<When>, &'static str> {
    let expr = typed.split_whitespace().collect::<Vec<_>>().join(" ");
    if expr.is_empty() {
        return Ok(None);
    }
    if expr.split(' ').count() != 5 {
        return Err(CRON_ERROR);
    }
    Ok(Some(When::Cron { expr }))
}

impl State {
    pub fn when_menu_open(&self) -> bool {
        self.ui.open == Some(crate::ui::Open::Popover(WHEN_POPOVER))
    }

    /// The composer the menu works on.
    fn when_composer(&mut self) -> &mut ComposerState {
        let editing = self.composer_editing();
        let nav = &mut self.automations_nav;
        if editing { &mut nav.edit } else { &mut nav.draft }
    }

    fn when_now(&self) -> Option<When> {
        self.with_composer(self.composer_editing(), |c, _| c.when())
    }

    fn today(&self) -> u8 {
        weekday(crate::view::message::now_ms(self), &super::submit::local)
    }

    /// The chip: the menu opens with the cron box holding the schedule's cron, no error, and the date and time boxes the
    /// schedule's (`watch(open)`); the first row takes the keys, as the popover's content does.
    pub fn open_when_menu(&mut self, tx: &mut Tx) {
        self.show_when_menu();
        tx.focus(Repeat::Daily.key(), false);
    }

    /// The menu open and filled, the keys left where they are (a scene).
    pub fn show_when_menu(&mut self) {
        let when = self.when_now();
        let from = crate::view::message::now_ms(self);
        let m = &mut self.automations_nav.when;
        m.cron.replace_text(when.as_ref().and_then(cron_of).unwrap_or_default());
        m.cron_error = None;
        m.cron_seen_focus = false;
        self.sync_when_fields(when.as_ref(), from, true, true);
        self.ui.open = Some(crate::ui::Open::Popover(WHEN_POPOVER));
    }

    fn sync_when_fields(&mut self, when: Option<&When>, from: i64, date: bool, time: bool) {
        let m = &mut self.automations_nav.when;
        if date {
            m.date.replace_text(once_date(when, from, &super::submit::local));
        }
        if time {
            m.time.replace_text(hm(time_of(when)));
        }
    }

    /// Closing: focus goes back to the box (the panel's `focus_return`).
    pub fn close_when_menu(&mut self) {
        if self.when_menu_open() {
            self.ui.open = None;
        }
    }

    /// `choose`: the schedule chosen here (None gives it back to the words), and the menu closes.
    pub fn choose_when(&mut self, when: Option<When>) {
        self.when_composer().set_when(when);
        self.close_when_menu();
    }

    pub fn when_repeat(&mut self, which: Repeat) {
        let today = self.today();
        let when = which.make(self.when_now().as_ref(), today);
        self.choose_when(Some(when));
    }

    /// Once stays open, now showing the Date box.
    pub fn when_once(&mut self) {
        let from = crate::view::message::now_ms(self);
        let when = once(self.when_now().as_ref(), from, &super::submit::local);
        self.when_composer().set_when(Some(when.clone()));
        self.sync_when_fields(Some(&when), from, true, false);
    }

    /// The time box changed: a whole time is the schedule's.
    pub fn when_time_changed(&mut self) {
        let typed = self.automations_nav.when.time.text.clone();
        let Some(when) = set_time(self.when_now().as_ref(), &typed, self.today()) else { return };
        self.when_composer().set_when(Some(when.clone()));
        // A weekly-or-once question goes, but the date is the same; a one-off's date box follows a time past midnight.
        self.sync_when_fields(Some(&when), crate::view::message::now_ms(self), true, false);
    }

    /// The date box changed: a whole date makes it a one-off on that day.
    pub fn when_date_changed(&mut self) {
        let typed = self.automations_nav.when.date.text.clone();
        let Some(when) = set_date(self.when_now().as_ref(), &typed) else { return };
        self.when_composer().set_when(Some(when));
    }

    /// Enter in the cron box, or leaving it changed: five parts are the schedule, anything else says so.
    pub fn when_apply_cron(&mut self) {
        let typed = self.automations_nav.when.cron.text.clone();
        match apply_cron(&typed) {
            Ok(Some(when)) => self.choose_when(Some(when)),
            Ok(None) => {}
            Err(words) => self.automations_nav.when.cron_error = Some(words.to_string()),
        }
    }

    /// Polled while the page is open: the cron box losing the keys with something new in it applies it (its `@blur`).
    pub(crate) fn keep_when_menu(&mut self) -> bool {
        if !self.when_menu_open() {
            self.automations_nav.when.cron_seen_focus = false;
            return false;
        }
        let m = &mut self.automations_nav.when;
        if m.cron.focused {
            m.cron_seen_focus = true;
            return false;
        }
        if !std::mem::take(&mut m.cron_seen_focus) {
            return false;
        }
        let typed = m.cron.text.trim().to_string();
        let saved = self.when_now().as_ref().and_then(cron_of).unwrap_or_default();
        if !typed.is_empty() && typed != saved {
            self.when_apply_cron();
        }
        true
    }

    /// The event types the menu offers: the machine's catalog once read, else the built-in list.
    pub fn when_event_types(&self) -> Vec<String> {
        match &self.automations_nav.composer.events {
            Some(list) => list.clone(),
            None => super::words::event_types().into_iter().map(str::to_string).collect(),
        }
    }
}
