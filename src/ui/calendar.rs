//! A month calendar for a popover under a date field: the month and year between previous and next, the weekday initials, six
//! weeks of days (Sunday first, as an en-US browser's date picker draws them), today ringed, the picked day filled and the days
//! outside `min`/`max` dimmed and inert. Analytics' two date fields use it; Automations' date field may too. Days are
//! `YYYY-MM-DD` text, compared as text (four-digit years), the form the filters already hold.
use super::overlay::{anchored, scrim};
use crate::State;
use crate::fleet::analytics::filters::day_number;
use crate::fleet::summary::iso_day;
use crate::theme::{RADIUS_BTN, Theme, sans};
use rune::{Align, Cursor, El, Icon, Side, col, icon, row, text};
use std::rc::Rc;

const MONTHS: [&str; 12] =
    ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
const WEEKDAYS: [&str; 7] = ["S", "M", "T", "W", "T", "F", "S"];
const CELL: f32 = 30.0;
const PAD: f32 = 8.0;

/// A month on show.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Month {
    pub year: i32,
    /// 1 to 12.
    pub month: u32,
}

impl Month {
    /// The month holding `day` ("2026-11-20"), or `None` for text that isn't a day.
    pub fn of(day: &str) -> Option<Month> {
        day_number(day).filter(|n| iso_day(*n) == day)?;
        Some(Month { year: day[0..4].parse().ok()?, month: day[5..7].parse().ok()? })
    }

    /// `by` months later (earlier when negative).
    pub fn shifted(self, by: i32) -> Month {
        let index = self.year * 12 + self.month as i32 - 1 + by;
        Month { year: index.div_euclid(12), month: index.rem_euclid(12) as u32 + 1 }
    }

    pub fn title(self) -> String {
        format!("{} {}", MONTHS[self.month as usize - 1], self.year)
    }

    /// The 42 days the grid shows: whole weeks from the Sunday on or before the 1st.
    pub fn cells(self) -> Vec<String> {
        let first = day_number(&format!("{:04}-{:02}-01", self.year, self.month)).unwrap_or(0);
        // 1 January 1970 was a Thursday.
        let weekday = (first + 4).rem_euclid(7);
        (0..42).map(|i| iso_day(first - weekday + i)).collect()
    }
}

/// What the calendar draws.
pub struct Spec<'a> {
    /// The key prefix of every element in it.
    pub key: &'a str,
    pub month: Month,
    /// The day the field holds, or "".
    pub picked: &'a str,
    pub today: &'a str,
    /// The earliest and latest day that can be picked ("" for no limit).
    pub min: &'a str,
    pub max: &'a str,
}

type Step = Rc<dyn Fn(&mut State)>;
/// A day picked, as `YYYY-MM-DD`.
pub type Pick = Rc<dyn Fn(&mut State, &str)>;

fn step(t: &Theme, key: String, glyph: &'static Icon, label: &'static str, press: Step) -> El {
    row()
        .key(key)
        .role("button")
        .label(label)
        .w(CELL)
        .h(CELL)
        .center()
        .radius(RADIUS_BTN)
        .cursor(Cursor::Pointer)
        .hover(t.wash(6.0))
        .focus_ring(2.0, 0.0, t.accent)
        .child(icon(glyph, 14.0).color(t.muted))
        .on_press(move |s: &mut State| press(s))
}

fn day(t: &Theme, c: &Spec, iso: &str, in_month: bool, pick: &Pick) -> El {
    let allowed = (c.min.is_empty() || iso >= c.min) && (c.max.is_empty() || iso <= c.max);
    let on = iso == c.picked;
    let ink = if on {
        t.primary_fg
    } else if in_month {
        t.text
    } else {
        t.muted
    };
    let number = text(iso[8..10].trim_start_matches('0').to_string()).font(sans(12.5, 400.0, 1.0).features("'tnum' 1")).color(ink);
    let mut cell =
        row().key(format!("{}-day-{iso}", c.key)).role("button").label(iso.to_string()).w(CELL).h(CELL).center().radius(RADIUS_BTN);
    if on {
        cell = cell.fill(t.accent);
    } else if iso == c.today {
        cell = cell.border(1.0, t.accent).border_layout();
    }
    cell = cell.child(number);
    if !allowed {
        return cell.opacity(0.3);
    }
    let iso = iso.to_string();
    let pick = pick.clone();
    let cell = cell.cursor(Cursor::Pointer).focus_ring(2.0, 0.0, t.accent).on_press(move |s: &mut State| pick(s, &iso));
    if on { cell } else { cell.hover(t.wash(10.0)) }
}

/// The popover's panel: header, weekday initials, the weeks.
pub fn view(t: &Theme, c: &Spec, prev: Step, next: Step, pick: Pick) -> El {
    let mut header = row().w_full().h(CELL).center_y().justify_between().child(step(
        t,
        format!("{}-prev", c.key),
        &crate::icons::CHEVRON_LEFT,
        "Previous month",
        prev,
    ));
    header = header.child(text(c.month.title()).key(format!("{}-title", c.key)).font(sans(12.5, 600.0, 1.3)).color(t.text)).child(step(
        t,
        format!("{}-next", c.key),
        &crate::icons::CHEVRON_RIGHT,
        "Next month",
        next,
    ));
    let mut initials = row().w_full();
    for d in WEEKDAYS {
        initials = initials.child(row().w(CELL).h(24.0).center().child(text(d).font(sans(11.0, 500.0, 1.0)).color(t.muted)));
    }
    let mut weeks = col().w_full();
    for week in c.month.cells().chunks(7) {
        let mut line = row().w_full();
        for iso in week {
            let in_month = Month::of(iso) == Some(c.month);
            line = line.child(day(t, c, iso, in_month, &pick));
        }
        weeks = weeks.child(line);
    }
    col()
        .key(format!("{}-menu", c.key))
        .role("dialog")
        .label("Calendar")
        .w(CELL * 7.0 + PAD * 2.0 + 2.0)
        .padding(PAD)
        .gap(2.0)
        .fill(t.card_bg)
        .border(1.0, t.border)
        .border_layout()
        .radius(RADIUS_BTN)
        .child(header)
        .child(initials)
        .child(weeks)
}

/// The panel under the element keyed `anchor`, over a scrim: a press outside, or Escape, runs `close`.
pub fn popover(t: &Theme, panel: El, anchor: &str, scrim_key: &str, close: Step) -> Vec<El> {
    let mut p = panel.focus_return(anchor.to_string()).modal(true, {
        let close = close.clone();
        move |s: &mut State| close(s)
    });
    for shadow in t.menu_shadows() {
        p = p.box_shadow(shadow);
    }
    vec![scrim(scrim_key, move |s: &mut State| close(s)), anchored(p, anchor, Side::Bottom, Align::Start, 4.0)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_month_is_six_whole_weeks_from_the_sunday_on_or_before_the_first() {
        let cells = Month { year: 2026, month: 11 }.cells();
        assert_eq!(cells.len(), 42);
        // 1 November 2026 is a Sunday.
        assert_eq!((cells[0].as_str(), cells[41].as_str()), ("2026-11-01", "2026-12-12"));
        let cells = Month { year: 2026, month: 10 }.cells();
        assert_eq!(cells[0], "2026-09-27", "1 October 2026 is a Thursday");
    }

    #[test]
    fn months_step_across_a_year() {
        let nov = Month { year: 2026, month: 11 };
        assert_eq!(nov.shifted(2), Month { year: 2027, month: 1 });
        assert_eq!(Month { year: 2027, month: 1 }.shifted(-1), Month { year: 2026, month: 12 });
        assert_eq!(Month { year: 2026, month: 1 }.shifted(-1), Month { year: 2025, month: 12 });
        assert_eq!(Month::of("2026-11-20"), Some(nov));
        assert_eq!(Month::of("soon"), None);
        assert_eq!(nov.title(), "November 2026");
    }
}
