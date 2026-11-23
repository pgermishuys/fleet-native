//! The phones and browsers that have keys (`DevicesList.vue`), under Settings → Machines → "Phones and browsers": each with how
//! it connected, when it was added and last seen, and **Remove**; then the machines that didn't answer, and what a refused
//! Remove said. `machines_page.rs` draws it with `hide_this_machine`, as `MachinesSection.vue` does.
use crate::conv::time::local_offset_secs;
use crate::fleet::machines::screens::Screen;
use crate::fleet::new_session::machines::format_date_time;
use crate::fleet::row_status::{format_date, to_timestamp};
use crate::fleet::settings::harnesses::relative as format_relative;
use crate::theme::{RADIUS_CARD, Theme, sans};
use crate::ui::button::{Size, Variant, button, disabled};
use crate::ui::confirm::error_note;
use crate::ui::rem;
use crate::view::message::now_ms;
use crate::{State, Tx};
use rune::{El, col, row, text};

/// What the list is given (`DevicesList`'s props).
pub struct Props<'a> {
    pub devices: &'a [Screen],
    /// How this computer is signed in, for its row.
    pub this_computer: &'a str,
    /// Phones and browsers only: leaves out the row for the machine itself.
    pub hide_this_machine: bool,
    /// Your Fleet: machines that didn't answer, so what they hold isn't in the list.
    pub not_answering: &'a [String],
}

/// The times a row says, on the page's clock.
struct Clock {
    now: i64,
    offset: i64,
}

impl Clock {
    fn at(&self, text: &str) -> Option<i64> {
        to_timestamp(text, self.offset)
    }

    fn date(&self, text: &str) -> String {
        self.at(text).map(|at| format_date(at, self.now, self.offset)).unwrap_or_default()
    }

    fn date_time(&self, text: &str) -> String {
        self.at(text).map(|at| format_date_time(at, self.now, self.offset)).unwrap_or_default()
    }

    fn relative(&self, text: &str) -> String {
        self.at(text).map(|at| format_relative(at, self.now, self.offset)).unwrap_or_default()
    }
}

/// `added`: how it came to have a key, and when.
fn added(device: &Screen, clock: &Clock) -> String {
    let date = clock.date(&device.created_at);
    if device.is_machine {
        return format!("Machine, paired {date}");
    }
    // The Fleet-wide list says which machine it connected through.
    if device.fleet_wide {
        return match &device.through_name {
            Some(through) => format!("Connected through {through}, added {date}"),
            None => format!("Added {date}"),
        };
    }
    if device.paired_via.is_some() { format!("Added {date} from another machine") } else { format!("Added {date}") }
}

/// The second line of a row: `<added> · last seen <relative>`.
pub fn line(device: &Screen, now: i64, offset: i64) -> String {
    let clock = Clock { now, offset };
    format!("{} \u{b7} last seen {}", added(device, &clock), clock.relative(&device.last_used_at))
}

/// "atlas isn't answering, so what it holds isn't shown." / "atlas, workbench aren't answering, so what they hold isn't shown."
pub fn not_answering_words(names: &[String]) -> String {
    let one = names.len() == 1;
    format!(
        "{} {} answering, so what {} isn't shown.",
        names.join(", "),
        if one { "isn't" } else { "aren't" },
        if one { "it holds" } else { "they hold" }
    )
}

fn small(t: &Theme, words: String) -> El {
    text(words).font(sans(rem(0.75), 400.0, 1.33)).color(t.muted).wrap()
}

/// `.devices__row`: flex, centred, gap 12, at least 52 tall, padding 8/14.
fn frame(inner: El, trailing: Option<El>) -> El {
    let mut r = row().w_full().center_y().gap(12.0).min_h(52.0).px(14.0).py(8.0).child(inner);
    if let Some(trailing) = trailing {
        r = r.child(trailing);
    }
    r
}

fn device_row(s: &State, t: &Theme, device: &Screen, clock: &Clock) -> El {
    let title = format!("Added {}, last seen {}", clock.date_time(&device.created_at), clock.date_time(&device.last_used_at));
    let words = col()
        .grow(1.0)
        .min_w(0.0)
        .child(text(device.name.clone()).font(sans(rem(0.875), 400.0, 1.43)).color(t.text).truncate())
        .child(small(t, line(device, clock.now, clock.offset)).help(title));
    let remove = button(t, Variant::Outline, Size::Sm, None, "Remove").key(format!("remove-device-{}", device.id));
    let remove = if s.machine_data.removing.as_deref() == Some(device.id.as_str()) {
        disabled(remove)
    } else {
        let id = device.id.clone();
        remove.on_press_tx(move |s: &mut State, tx: &mut Tx| s.ask_remove_screen(&id, tx))
    };
    frame(words, Some(remove)).key(format!("device-row-{}", device.id))
}

/// The list: "Devices with access" and this machine's row unless hidden, a row per device in one bordered card, then who didn't
/// answer and a refused Remove.
pub fn list(s: &State, t: &Theme, p: &Props) -> El {
    let clock = Clock { now: now_ms(s), offset: local_offset_secs() };
    let mut rows = Vec::new();
    if !p.hide_this_machine {
        let this = col()
            .grow(1.0)
            .min_w(0.0)
            .child(text("This machine").font(sans(rem(0.875), 400.0, 1.43)).color(t.text))
            .child(small(t, p.this_computer.to_string()));
        rows.push(frame(this, None).key("device-row-this-machine"));
    }
    rows.extend(p.devices.iter().map(|d| device_row(s, t, d, &clock)));
    let mut card =
        col().key("devices").w_full().radius(RADIUS_CARD).border(1.0, t.border).border_layout().fill(t.card_bg).clip_rounded(RADIUS_CARD);
    for (at, r) in rows.into_iter().enumerate() {
        card = card.child(if at == 0 { r } else { col().w_full().border_sides([1.0, 0.0, 0.0, 0.0], t.border).child(r) });
    }
    let mut e = col().key("devices-with-access").w_full().mt(rem(1.25)).gap(rem(0.5));
    if !p.hide_this_machine {
        e = e.child(text("DEVICES WITH ACCESS").font(sans(rem(0.75), 500.0, 1.33).tracking(0.6)).color(t.muted));
    }
    e = e.child(card);
    if !p.not_answering.is_empty() {
        e = e.child(small(t, not_answering_words(p.not_answering)).key("devices-not-answering"));
    }
    if let Some(why) = &s.machine_data.screen_error {
        e = e.child(error_note(t, "devices-error", why, None));
    }
    e
}
