//! `AnalyticsFilters.vue`'s two date fields, as text. Chrome's `<input type="date">` shows `MM/DD/YYYY` (en-US), takes
//! typing a segment at a time and fires `change` with a whole date (`YYYY-MM-DD`) or with nothing once it is emptied. Here the
//! field is plain text: a whole date typed is sent, an emptied field clears that end, and anything in between is left
//! alone until the field is left, when it goes back to what was last sent.
use super::filters::{Filters, day_number};
use crate::fleet::summary::iso_day;
use rune::TextEdit;

/// What the field holds right now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Typed {
    /// A whole date, as the server reads it ("2026-11-20").
    Whole(String),
    /// Nothing: the end is cleared.
    Empty,
    /// Some of a date: nothing is sent.
    Partial,
}

/// "11/20/2026" for "2026-11-20"; a value that isn't a date shows as an empty field, as the browser's does.
pub fn show(day: &str) -> String {
    match day_number(day).filter(|n| iso_day(*n) == day) {
        Some(_) => format!("{}/{}/{}", &day[5..7], &day[8..10], &day[0..4]),
        None => String::new(),
    }
}

/// Reads what was typed: exactly two digits, two digits and four digits between slashes, and a day that exists.
pub fn parse(text: &str) -> Typed {
    if text.is_empty() {
        return Typed::Empty;
    }
    let b = text.as_bytes();
    let shaped = b.len() == 10 && b[2] == b'/' && b[5] == b'/' && b.iter().enumerate().all(|(i, c)| i == 2 || i == 5 || c.is_ascii_digit());
    if !shaped {
        return Typed::Partial;
    }
    let iso = format!("{}-{}-{}", &text[6..10], &text[0..2], &text[3..5]);
    match day_number(&iso) {
        Some(n) if iso_day(n) == iso && &iso[0..4] != "0000" => Typed::Whole(iso),
        _ => Typed::Partial,
    }
}

/// What a look at the fields found.
#[derive(Default)]
pub struct Step {
    /// The filters with a typed end applied.
    pub next: Option<Filters>,
    /// A field's text changed, so the page draws again.
    pub redraw: bool,
}

/// The two fields' text and what each was last seen holding.
#[derive(Default)]
pub struct Fields {
    pub from: TextEdit,
    pub to: TextEdit,
    /// The end each field last agreed with (what the filters held, or what it sent).
    last: [String; 2],
    /// The text last looked at, so only a change is read.
    seen: [String; 2],
}

fn put(edit: &mut TextEdit, seen: &mut String, words: String) {
    let focused = edit.focused;
    *edit = TextEdit::new(words.clone());
    edit.focused = focused;
    *seen = words;
}

impl Fields {
    /// Looks at both fields against the filters: an end changed from outside (Reset, a restored range) is shown; a whole date
    /// typed, or an emptied field, becomes the next filters; a field that was left half-typed goes back.
    pub fn step(&mut self, filters: &Filters) -> Step {
        let mut step = Step::default();
        let mut next = filters.clone();
        for (i, (edit, end)) in [(&mut self.from, &mut next.from), (&mut self.to, &mut next.to)].into_iter().enumerate() {
            let shown = show(end);
            if *end != self.last[i] {
                self.last[i] = end.clone();
                if edit.text != shown {
                    put(edit, &mut self.seen[i], shown);
                    step.redraw = true;
                }
                self.seen[i] = edit.text.clone();
            } else if edit.text != self.seen[i] {
                self.seen[i] = edit.text.clone();
                let typed = match parse(&edit.text) {
                    Typed::Whole(day) => Some(day),
                    Typed::Empty => Some(String::new()),
                    Typed::Partial => None,
                };
                if let Some(day) = typed.filter(|d| d != end) {
                    self.last[i] = day.clone();
                    *end = day;
                }
            } else if !edit.focused && edit.text != shown {
                put(edit, &mut self.seen[i], shown);
                step.redraw = true;
            }
        }
        if next != *filters {
            step.next = Some(next);
        }
        step
    }
}
