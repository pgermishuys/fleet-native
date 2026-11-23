//! Settings → Machines (`MachinesSection.vue`, inside `SettingsPage.vue`'s detail pane): the machines this app knows as cards, each
//! with Work here and Manage (the machine's own page, `machine_page.rs`), and "Phones and browsers" (`add_phone.rs`, `devices_list.rs`). `settings.rs` mounts
//! `content` as the Machines page. The pairing form is piece K's.
use super::machine_card::card;
use super::settings::kit::{self, Row};
use super::{add_phone, devices_list};
use crate::fleet::machine_actions::AccessState;
use crate::icons::PLUS;
use crate::theme::{Theme, sans};
use crate::ui::button::{Size, Variant, button};
use crate::ui::confirm::error_note;
use crate::ui::rem;
use crate::{State, Tx};
use rune::{El, col, el, row, text};

/// "Phones and browsers" (`machine-phones-group`): Add your phone and the phones and browsers that have keys, once this machine's
/// access is read; else why there is nothing to show.
fn phones_group(s: &State, t: &Theme) -> El {
    let body = match &s.machine_page.access {
        AccessState::Loaded(access) => {
            let name = s.machines.home.as_ref().map(|h| h.name.clone()).filter(|n| !n.is_empty()).unwrap_or_else(|| "This machine".into());
            let this_computer = if access.requires_token { "Signed in with the machine token" } else { "Signed in on loopback" };
            let data = &s.machine_data;
            let not_answering: Vec<String> = data.screens.not_answering.iter().map(|m| m.name.clone()).collect();
            let props = devices_list::Props {
                devices: &data.screens.screens,
                this_computer,
                hide_this_machine: true,
                not_answering: &not_answering,
            };
            col()
                .key("machine-phones")
                .w_full()
                .px(16.0)
                .py(12.0)
                .child(add_phone::panel(s, t, &name))
                .child(devices_list::list(s, t, &props))
        }
        AccessState::Failed(why) => col().w_full().px(16.0).py(12.0).child(error_note(t, "machine-phones-error", why, None)),
        other => {
            let words = match other {
                AccessState::Device => "This app is signed in as a paired device. Adding phones happens on the machine itself.",
                AccessState::Unavailable => {
                    "This Fleet doesn't hand out access tokens: it signs people in with an identity provider, or it's too old for machines."
                }
                _ => "Loading…",
            };
            row()
                .w_full()
                .px(16.0)
                .py(12.0)
                .child(text(words.to_string()).key("machine-phones-note").font(sans(rem(0.875), 400.0, 1.43)).color(t.muted).wrap())
        }
    };
    let group = kit::group(
        t,
        "phones",
        "Phones and browsers",
        Some("Every phone and browser that has keys, wherever it connected. Remove one and it loses them on every machine."),
        vec![body],
    );
    // The group keeps its own `settings-group-phones` key for anchors; the wrapper carries Fleet's test id.
    col().key("machine-phones-group").w_full().child(group)
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
        .child(phones_group(s, t))
}
