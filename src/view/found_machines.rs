//! Add a machine → Found (`FoundMachinesCard.vue`, with `EmptyState`, `ErrorNote`, `MachineDiagnosisPanel` and `MachineAddNote`):
//! machines waiting to be added, seen on Tailscale and the home network. Fleet shows the first half of the code the terminal
//! shows; adding one needs the rest, typed from that terminal, so the machine added is the one in front of you.
use super::add_machine::{add_note, copied, error_note, spinner, with_code};
use crate::fleet::machines::add::AddMode;
use crate::fleet::machines::found::FoundMachine;
use crate::icons::RADAR;
use crate::theme::{RADIUS_BTN, RADIUS_CARD, Theme, mono, sans};
use crate::ui::button::{Size, Variant, button, button_lead};
use crate::ui::rem;
use crate::ui::text_menu::text_input;
use crate::{State, Tx};
use rune::{El, Key, KeyEvent, col, icon, row, text};

fn xs(t: &Theme, words: &str) -> El {
    text(words.to_string()).font(sans(rem(0.75), 400.0, 1.33)).color(t.muted).wrap()
}

/// `EmptyState` bordered and compact: the radar, "No machine is waiting", the line, and the way to the pasted link.
fn empty(t: &Theme) -> El {
    let paste = button(t, Variant::Link, Size::Sm, None, "Paste a pairing link instead")
        .key("machines-found-paste")
        .on_press_tx(|s: &mut State, tx: &mut Tx| s.pick_add_mode(AddMode::Link, tx));
    col()
        .key("machines-found-empty")
        .w_full()
        .center()
        .gap(4.0)
        .px(rem(0.75))
        .py(rem(1.0))
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .border_dash(3.0, 3.0)
        .child(icon(&RADAR, 20.0).color(t.muted).mb(rem(0.5)))
        .child(text("No machine is waiting").font(sans(rem(0.875), 500.0, 1.43)).color(t.text))
        .child(xs(t, "Run fleet node up on the other machine. This list updates by itself."))
        .child(row().mt(rem(0.75)).child(paste))
}

/// The box the hint and the typed rest share: `rounded-btn border bg-main-bg px-3 py-2 font-mono text-sm tracking-widest`.
fn code_box(t: &Theme, e: El) -> El {
    e.px(rem(0.75)).py(rem(0.5)).radius(RADIUS_BTN).border(1.0, t.border).border_layout().fill(t.main_bg)
}

fn code_font() -> rune::TextStyle {
    mono(rem(0.875), 400.0, 1.43).tracking(1.4)
}

fn machine_row(s: &State, t: &Theme, found: &FoundMachine) -> El {
    let tab = &s.machine_page.add.waiting;
    let id = found.machine_id.clone();
    let busy = tab.busy.as_deref() == Some(id.as_str());
    let edit = tab.rest.get(&id).cloned().unwrap_or_default();
    let (on_edit, on_key) = (id.clone(), id.clone());
    let field = code_box(
        t,
        text_input(&edit, code_font())
            .key(format!("machines-found-code-{id}"))
            .label(format!("The last four characters of the code {} shows", found.name))
            .single_line()
            .placeholder("____", t.muted)
            .color(t.text)
            .caret(t.text, t.accent.alpha(0.35))
            .w(rem(7.0))
            .focus_ring(1.0, 0.0, t.accent)
            .on_edit(move |s: &mut State| s.machine_page.add.waiting.rest.entry(on_edit.clone()).or_default())
            .on_input_key(move |s: &mut State, ev: &KeyEvent, _: &mut Tx| match ev.key {
                Key::Enter => {
                    s.add_found(&on_key);
                    true
                }
                Key::Escape => {
                    s.close_add_form();
                    true
                }
                _ => false,
            }),
    );
    let field = if busy { super::machine_card::faded(field) } else { field };
    let can = tab.busy.is_none() && found.code_for(&edit.text).is_some();
    let add = if busy {
        button_lead(t, Variant::Default, Size::Default, Some(spinner(t.primary_fg, 16.0)), "Checking…")
    } else {
        button(t, Variant::Default, Size::Default, None, &format!("Add {}", found.name))
    };
    let add_id = id.clone();
    let add = if can { add.on_press(move |s: &mut State| s.add_found(&add_id)) } else { super::machine_card::faded(add) };
    let mut e = col()
        .key(format!("machines-found-row-{id}"))
        .w_full()
        .gap(12.0)
        .padding(12.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.card_bg)
        .child(
            col()
                .gap(2.0)
                .child(text(found.title()).key(format!("machines-found-name-{id}")).font(sans(rem(0.875), 500.0, 1.43)).color(t.text))
                .child(xs(t, &format!("{}. Waiting to join.", found.reach_label()))),
        )
        .child(
            row()
                .flex_wrap()
                .items_end()
                .gap(8.0)
                .child(
                    col()
                        .gap(4.0)
                        .child(
                            text(format!("Check the code in {}'s terminal", found.name)).font(sans(rem(0.75), 500.0, 1.33)).color(t.muted),
                        )
                        .child(
                            row()
                                .gap(8.0)
                                .center_y()
                                .child(code_box(
                                    t,
                                    text(format!("{}-", found.hint))
                                        .key(format!("machines-found-hint-{id}"))
                                        .font(code_font())
                                        .color(t.text),
                                ))
                                .child(field),
                        ),
                )
                .child(add.key(format!("machines-found-add-{id}"))),
        );
    if let Some(why) = tab.failure.get(&id) {
        let key = format!("machines-found-error-{id}");
        let shown = copied(s, &key);
        e = e.child(error_note(t, &key, why, false, shown));
    }
    e.child(add_note(t, &found.name))
}

/// The Found tab.
pub fn card(s: &State, t: &Theme) -> El {
    let e = col().key("machines-found").w_full().gap(12.0).child(with_code(
        t,
        "machines-found-intro",
        "On the other machine, run `fleet node up` (or `fleet node pair`). While it waits, it shows up here. Nothing is pasted: type \
         the rest of the code its terminal shows.",
        rem(0.75),
        t.muted,
    ));
    match &s.machine_data.found {
        None => e.child(
            row()
                .key("machines-found-looking")
                .gap(8.0)
                .center_y()
                .child(spinner(t.muted, 14.0))
                .child(xs(t, "Looking on Tailscale and your home network…")),
        ),
        Some(list) if list.is_empty() => e.child(empty(t)),
        Some(list) => e.child(
            col()
                .key("machines-found-list")
                .label("Machines waiting to be added")
                .w_full()
                .gap(12.0)
                .children(list.iter().map(|found| machine_row(s, t, found)).collect::<Vec<_>>()),
        ),
    }
}
