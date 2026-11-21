//! Ports `AutomationWhenPicker.vue`: the When chip under the composer's box and the menu above it (rows 8.1-8.9). The chip
//! says the schedule (Clock, or Zap for an event); the menu offers the message's words back, Repeat, Once with its date and
//! time, a cron, and the events. Rune has no date or time field, so those are text boxes taking `YYYY-MM-DD` and `HH:MM`.
use crate::fleet::automations::schedule::{When, day_name, hm, when_chip};
use crate::fleet::automations::submit::time_zone;
use crate::fleet::automations::when::{Repeat, WHEN_CHIP, time_of, weekly_day};
use crate::fleet::automations::words::describe_event_type;
use crate::icons::{CALENDAR_CLOCK, CLOCK, MESSAGE_SQUARE_TEXT, REPEAT, ZAP};
use crate::theme::{Theme, mono, sans};
use crate::ui::chip::{Chip, Option_, option, pop, pop_field, pop_field_box, pop_label, pop_note, pop_separator};
use crate::ui::text_menu::text_input;
use crate::view::automations::composer::MESSAGE;
use crate::{State, Tx};
use rune::{Align, El, Key, KeyEvent, ScrollSpec, Side, TextEdit, col, text};

/// `.automation-when`: 300 wide, at most min(560, 70 % of the window) tall. Fleet's scoped rule never reaches the popover
/// (row 8.2), so its menu grows past the window; this is the height it means.
const WIDTH: f32 = 300.0;
const MAX_H: f32 = 560.0;

pub const DATE: &str = "automation-when-date";
pub const TIME: &str = "automation-when-time";
pub const CRON: &str = "automation-when-cron";

/// What the chip and the menu read off the composer.
pub struct Shown {
    pub when: Option<When>,
    /// The schedule came from the words in the message.
    pub parsed: bool,
    /// The message has schedule words a choice here overrides.
    pub message_has_schedule: bool,
}

/// The chip (row 8.1): Clock (Zap for an event), the label, a chevron; the attention look with no schedule, and the label in
/// `--accent-text` while it came from the message.
pub fn chip(s: &State, t: &Theme, shown: &Shown, busy: bool) -> El {
    let from = crate::view::message::now_ms(s);
    let local = |ms: i64| crate::conv::time::local_offset_at(ms);
    let label = when_chip(shown.when.as_ref(), from, &local);
    let glyph = if matches!(shown.when, Some(When::Event { .. })) { &ZAP } else { &CLOCK };
    let mut chip = Chip::new(WHEN_CHIP, label).glyph(glyph).attention(shown.when.is_none()).open(s.when_menu_open()).disabled(busy);
    if shown.parsed {
        chip = chip.ink(t.accent_text);
    }
    chip.view(t, |s: &mut State, tx: &mut Tx| s.open_when_menu(tx))
}

/// The menu (rows 8.2-8.8), 6 px above the chip, its left edge on the chip's.
pub fn overlay(s: &State, t: &Theme) -> Vec<El> {
    let editing = s.composer_editing();
    let shown = s.with_composer(editing, |c, _| Shown {
        when: c.when(),
        parsed: c.active_hit().is_some(),
        message_has_schedule: c.hit().is_some(),
    });
    let when = shown.when.as_ref();
    let today = crate::fleet::automations::schedule::weekday(crate::view::message::now_ms(s), &|ms| crate::conv::time::local_offset_at(ms));
    let at = hm(time_of(when));
    let mut body = Vec::new();
    if shown.parsed {
        body.push(pop_note(t, "Picked up from your message. Choosing here takes over."));
    } else if shown.message_has_schedule {
        let spec =
            Option_ { icon: Some(&MESSAGE_SQUARE_TEXT), ..Option_::new("automation-when-use-message", "Use the words in the message") };
        body.push(option(t, spec, None, |s, _| s.choose_when(None)));
    }
    body.push(pop_label(t, "Repeat"));
    for which in Repeat::ALL {
        let (title, detail) = match which {
            Repeat::Daily => ("Every day".to_string(), format!("at {at}")),
            Repeat::Weekdays => ("Weekdays".to_string(), format!("Mon–Fri at {at}")),
            Repeat::Weekly => (format!("{}s", day_name(weekly_day(when, today))), format!("every week at {at}")),
            Repeat::Hourly => ("Every hour".to_string(), "on the hour".to_string()),
        };
        let spec = Option_ { icon: Some(&REPEAT), detail: Some(detail), checked: which.is(when), ..Option_::new(which.key(), title) };
        body.push(option(t, spec, None, move |s, _| s.when_repeat(which)));
    }
    body.push(pop_separator(t));
    let once = Option_ {
        icon: Some(&CALENDAR_CLOCK),
        detail: Some("on a date you pick, then it switches off".into()),
        checked: matches!(when, Some(When::Once { .. })),
        ..Option_::new("automation-when-once", "Once")
    };
    body.push(option(t, once, None, |s, _| s.when_once()));
    // `.automation-when__fields`: the date for a one-off, the time for anything with a time of day, 2 px apart.
    let mut fields = col().key("automation-when-fields").w_full().gap(2.0);
    let m = &s.automations_nav.when;
    if matches!(when, Some(When::Once { .. })) {
        let input = field_input(t, &m.date, DATE, "Date", "YYYY-MM-DD", false, |s| &mut s.automations_nav.when.date)
            .on_input_change(|s: &mut State, _: &mut Tx| s.when_date_changed());
        fields = fields.child(pop_field(t, "Date", pop_field_box(t, input)));
    }
    if !matches!(when, Some(When::Event { .. } | When::Hours { .. } | When::Minutes { .. } | When::Cron { .. })) {
        let label = format!("Time ({})", time_zone());
        let input = field_input(t, &m.time, TIME, &label, "HH:MM", false, |s| &mut s.automations_nav.when.time)
            .on_input_change(|s: &mut State, _: &mut Tx| s.when_time_changed());
        fields = fields.child(pop_field(t, &label, pop_field_box(t, input)));
    }
    body.push(fields);
    body.push(pop_separator(t));
    body.push(cron(s, t));
    body.push(pop_separator(t));
    body.push(pop_label(t, "When something happens"));
    for event_type in s.when_event_types() {
        let spec = Option_ {
            icon: Some(&ZAP),
            checked: matches!(when, Some(When::Event { event_type: e }) if *e == event_type),
            ..Option_::new(format!("automation-when-event-{event_type}"), describe_event_type(&event_type))
        };
        body.push(option(t, spec, None, move |s, _| s.choose_when(Some(When::Event { event_type: event_type.clone() }))));
    }
    let window_h = s.ui.sel.window.1;
    let max_h = if window_h > 0.0 { MAX_H.min(window_h * 0.7) } else { MAX_H };
    let panel = pop(t, "automation-when-pop", WIDTH, body)
        .scroll(ScrollSpec { stick_bottom: false })
        .max_h(max_h)
        .focus_return(MESSAGE.to_string())
        .modal(true, |s: &mut State| s.close_when_menu());
    vec![
        crate::ui::overlay::scrim("automation-when-scrim", |s: &mut State| s.close_when_menu()),
        crate::ui::overlay::anchored(panel, WHEN_CHIP, Side::Top, Align::Start, 6.0),
    ]
}

/// "Custom (cron, in <zone> time)": mono, filled with the schedule's cron; Enter applies it, and so does leaving it changed
/// (`keep_when_menu`); the error under it in 11.5 px.
fn cron(s: &State, t: &Theme) -> El {
    let m = &s.automations_nav.when;
    let label = format!("Custom (cron, in {} time)", time_zone());
    let input = field_input(t, &m.cron, CRON, &label, "0 9 * * 1", true, |s| &mut s.automations_nav.when.cron).on_input_key(
        |s: &mut State, ev: &KeyEvent, _: &mut Tx| {
            let enter = ev.key == Key::Enter;
            if enter {
                s.when_apply_cron();
            }
            enter
        },
    );
    let mut field = pop_field(t, &label, pop_field_box(t, input));
    if let Some(error) = &m.cron_error {
        field = field
            .child(text(error.clone()).key("automation-when-cron-error").font(sans(11.5, 400.0, 1.3)).color(t.error_text).wrap().w_full());
    }
    field
}

/// `.ns-field__input`'s text: 13 px (mono 12 px), one line.
fn field_input(
    t: &Theme,
    edit: &TextEdit,
    key: &str,
    label: &str,
    placeholder: &str,
    monospace: bool,
    pick: fn(&mut State) -> &mut TextEdit,
) -> El {
    let font = if monospace { mono(12.0, 400.0, 1.3) } else { sans(13.0, 400.0, 1.3) };
    text_input(edit, font)
        .key(key.to_string())
        .label(label.to_string())
        .placeholder(placeholder.to_string(), t.muted)
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .single_line()
        .w_full()
        .on_edit(pick)
}
