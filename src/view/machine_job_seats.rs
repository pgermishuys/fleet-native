//! A machine's seats and labels for Jobs, under its card in Settings → Machines (`MachineJobSeats.vue`). Only drawn with Jobs
//! on, and only for a machine the home Fleet lists. Each field saves when it is left or Enter is pressed (`@change`).
use crate::fleet::jobs::new_job::{SeatField, labels_key, machine_of, seats_key};
use crate::theme::{RADIUS_BTN, Theme, sans};
use crate::ui::text_menu::text_input;
use crate::ui::{error_note, rem};
use crate::{State, Tx};
use rune::{El, Key, KeyEvent, TextEdit, row, text};

fn field(t: &Theme, edit: &TextEdit, key: String, label: &str, placeholder: &str, width: f32) -> El {
    text_input(edit, sans(rem(0.875), 400.0, 1.43))
        .key(key)
        .label(label.to_string())
        .single_line()
        .placeholder(placeholder.to_string(), t.muted)
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .w(width)
        .px(rem(0.5))
        .py(rem(0.25))
        .radius(RADIUS_BTN)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.main_bg)
        .focus_ring(1.0, 0.0, t.accent)
}

/// `None` when Jobs is off or the home Fleet doesn't list this machine.
pub fn view(s: &State, t: &Theme, entry_key: &str) -> Option<El> {
    if s.labs_feature(crate::fleet::labs::Feature::Jobs) != Some(true) {
        return None;
    }
    let m = machine_of(&s.jobs.machines, entry_key)?;
    let fields = s.jobs.seat_fields.get(&m.id)?;
    let enter = |id: String, which: SeatField| {
        move |s: &mut State, ev: &KeyEvent, _: &mut Tx| {
            if ev.key == Key::Enter {
                s.job_seats_commit(&id, which);
                return true;
            }
            false
        }
    };
    let (seats_id, labels_id) = (m.id.clone(), m.id.clone());
    let seats = field(t, &fields.seats, seats_key(&m.id), "Seats for jobs", "", rem(4.0))
        .on_edit({
            let id = m.id.clone();
            move |s: &mut State| &mut s.jobs.seat_fields.get_mut(&id).expect("filled").seats
        })
        .on_input_key(enter(seats_id, SeatField::Seats));
    let labels = field(t, &fields.labels, labels_key(&m.id), "Labels", "none", rem(10.0))
        .on_edit({
            let id = m.id.clone();
            move |s: &mut State| &mut s.jobs.seat_fields.get_mut(&id).expect("filled").labels
        })
        .on_input_key(enter(labels_id, SeatField::Labels));
    let name = |words: &str| text(words.to_string()).font(sans(rem(0.875), 400.0, 1.43)).color(t.muted);
    let mut line = row()
        .key(format!("machine-job-seats-{}", m.id))
        .mt(rem(0.5))
        .flex_wrap()
        .center_y()
        .gap(rem(0.75))
        .child(row().center_y().gap(rem(0.5)).child(name("Seats for jobs")).child(seats))
        .child(row().center_y().gap(rem(0.5)).child(name("Labels")).child(labels));
    if let Some(why) = s.jobs.save.error.as_ref().filter(|_| s.jobs.saving.as_deref() == Some(m.id.as_str())) {
        line = line.child(error_note::compact(t, &format!("machine-job-seats-error-{}", m.id), why));
    }
    Some(line)
}
