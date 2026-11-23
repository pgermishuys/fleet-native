//! Settings → Machines (`MachinesSection.vue`, inside `SettingsPage.vue`'s detail pane): the machines this app knows as cards, each
//! with Work here and Manage (the machine's own page, `machine_page.rs`), and "Phones and browsers" (piece I). `settings.rs` mounts
//! `content` as the Machines page. The pairing form is piece K's.
use super::machine_card::card;
use super::settings::kit::{self, Row};
use crate::icons::PLUS;
use crate::theme::{Theme, sans};
use crate::ui::button::{Size, Variant, button};
use crate::ui::rem;
use crate::{State, Tx};
use rune::{El, col, el, row, text};

/// "Phones and browsers": the devices with keys are piece I's. Until then the group says so.
fn phones_group(t: &Theme) -> El {
    let words = "Phones and browsers aren't in the native app yet. Their keys are on each machine's own page, under Manage.";
    kit::group(
        t,
        "phones",
        "Phones and browsers",
        Some("Every phone and browser that has keys, wherever it connected. Remove one and it loses them on every machine."),
        vec![
            row()
                .w_full()
                .px(16.0)
                .py(12.0)
                .child(text(words.to_string()).key("machine-phones-later").font(sans(rem(0.875), 400.0, 1.43)).color(t.muted).wrap()),
        ],
    )
}

/// The page's content: the title and its line, then the groups. The Settings shell (`settings.rs`) gives it the sheet and the scroll.
pub fn content(s: &State, t: &Theme) -> El {
    let add = button(t, Variant::Default, Size::Default, Some(&PLUS), "Add a machine")
        .key("machines-add")
        .on_press_tx(|s: &mut State, tx: &mut Tx| s.open_add_form(tx));
    let form = &s.machine_page.add;
    // The button is gone while the form is open (`v-if="!showAddForm"`).
    let control = if form.open { el().size(0.0, 0.0) } else { add };
    let mut rows = vec![Row::new("add-machine", "Add a machine")
        .summary(
        "Every machine runs its own Fleet and keeps its own sessions, repositories and sign-ins. A session stays on the machine it started on. The sessions list shows them all; the one you're working in is live.",
        )
        .control(control)
        .build(s, t)];
    if form.open {
        rows.push(super::add_machine::form(s, t));
    }
    if let Some(name) = &form.added {
        rows.push(super::add_machine::added(t, name));
    }
    rows.extend(s.machine_cards().iter().map(|c| card(s, t, c)));
    let header = super::settings::page_header(s, t, &crate::fleet::settings_nav::Page::Machines);
    col()
        .key("machines-page-body")
        .w_full()
        .gap(21.0)
        .child(header)
        .child(kit::group(t, "machines", "Your machines", None, rows))
        .child(phones_group(t))
}
