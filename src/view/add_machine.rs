//! The Add a machine form on Settings → Machines (`MachinesSection.vue`'s `machines-add-form`, `MachineAddNote.vue`): the tab strip
//! "How to add the machine" and the Paste a pairing link tab (the pairing link, an address with its code, or under Advanced the
//! machine's token; the machine found behind it; the button). Found is `found_machines.rs`, On a new server `new_server.rs`; the
//! "Set it up from here" tab (SSH) is not built (owner decision 5).
use super::machine_card::{faded, plain_button};
use crate::fleet::machine_diagnosis::{Diagnosis, StepState};
use crate::fleet::machines::add::AddMode;
use crate::fleet::pairing::{CODE_FIELD, Failure, LINK_FIELD, TOKEN_FIELD};
use crate::icons::{CHECK, CIRCLE, CIRCLE_ALERT, LOADER_CIRCLE, X};
use crate::theme::{RADIUS_BTN, RADIUS_CARD, Theme, mono, sans};
use crate::ui::button::{Size, Variant, button, button_lead};
use crate::ui::rem;
use crate::ui::text_menu::text_input;
use crate::{State, Tx};
use rune::{Cursor, Cycle, El, Key, KeyEvent, Span, TextEdit, col, icon, row, text};

/// `inputClass`: full width, 1px border, the page's colour, `text-sm`, an accent border while focused.
pub(super) fn input(t: &Theme, edit: &TextEdit, key: &str, label: &str, placeholder: &str, font: rune::TextStyle, locked: bool) -> El {
    let e = text_input(edit, font)
        .key(key.to_string())
        .label(label.to_string())
        .single_line()
        .placeholder(placeholder.to_string(), t.muted)
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .w_full()
        .px(rem(0.75))
        .py(rem(0.5))
        .radius(RADIUS_BTN)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.main_bg)
        .focus_ring(1.0, 0.0, t.accent)
        .on_input_key(|s: &mut State, ev: &KeyEvent, _: &mut Tx| match ev.key {
            Key::Enter => {
                s.submit_add();
                true
            }
            Key::Escape => {
                s.close_add_form();
                true
            }
            _ => false,
        });
    if locked { faded(e) } else { e }
}

/// A label over its field: `text-xs font-medium uppercase tracking-wide text-muted`.
fn labelled(t: &Theme, words: &str, field: El, hint: Option<&str>) -> El {
    let mut e = col()
        .w_full()
        .gap(4.0)
        .child(text(words.to_uppercase()).font(sans(rem(0.75), 500.0, 1.33).tracking(0.6)).color(t.muted))
        .child(field);
    if let Some(hint) = hint {
        e = e.child(text(hint.to_string()).font(sans(rem(0.75), 400.0, 1.33)).color(t.muted).wrap().w_full());
    }
    e
}

pub(super) fn copied(s: &State, what: &str) -> bool {
    s.machine_page.copied.as_ref().is_some_and(|(w, _)| w == what)
}

pub(super) fn spinner(ink: rune::Color, size: f32) -> El {
    static FADE: [(f32, f32); 3] = [(0.0, 1.0), (0.5, 0.35), (1.0, 1.0)];
    icon(&LOADER_CIRCLE, size).color(ink).cycle(Cycle { period: 1.2, phase: 0.0, stops: &FADE, still: 1.0, fps: 30.0 })
}

/// `ErrorNote`: the red box. With a diagnosis its headline is the message and the steps follow it (`addError`).
pub(super) fn error_note(t: &Theme, key: &str, why: &Failure, compact: bool, copied: bool) -> El {
    let shown = why.diagnosis.as_ref().map_or(why.message.as_str(), |d| d.headline.as_str());
    let mut body =
        col().shrink().min_w(0.0).gap(4.0).child(
            text(shown.to_string()).font(sans(rem(if compact { 0.75 } else { 0.875 }), 400.0, 1.4)).color(t.error_text).wrap().w_full(),
        );
    if let Some(d) = &why.diagnosis {
        body = body.child(diagnosis(t, key, d, copied));
    }
    let e = row().key(key.to_string()).role("alert").w_full().gap(8.0).items_start();
    if compact {
        return e.child(icon(&CIRCLE_ALERT, 14.0).color(t.error_text).mt(1.0)).child(body);
    }
    e.px(12.0)
        .py(8.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.error.alpha(0.3))
        .border_layout()
        .fill(t.error.alpha(0.1))
        .child(icon(&CIRCLE_ALERT, 16.0).color(t.error_text).mt(1.0))
        .child(body)
}

/// `MachineDiagnosisPanel`: the steps ticked up to the one that broke, what to try, and Copy details.
fn diagnosis(t: &Theme, key: &str, d: &Diagnosis, copied: bool) -> El {
    let small = |w: f32| sans(rem(0.75), w, 1.33);
    let mut steps = col().gap(4.0);
    for step in &d.steps {
        let (glyph, ink) = match step.state {
            StepState::Ok => (&CHECK, t.running),
            StepState::Failed => (&X, t.error_text),
            StepState::NotChecked => (&CIRCLE, t.muted),
        };
        let muted = step.state == StepState::NotChecked;
        let label = if muted { format!("{} · not checked", step.label) } else { step.label.clone() };
        let mut words =
            col().shrink().min_w(0.0).child(text(label).font(small(400.0)).color(if muted { t.muted } else { t.error_text }).wrap());
        if let Some(detail) = &step.detail {
            words = words.child(text(detail.clone()).font(small(400.0)).color(t.muted).wrap().w_full());
        }
        steps = steps.child(row().gap(8.0).items_start().child(icon(glyph, 14.0).color(ink).mt(2.0)).child(words));
    }
    let (details, what) = (d.text(), key.to_string());
    let copy = plain_button(t, format!("{key}-copy"), if copied { "Copied" } else { "Copy details" })
        .on_press_tx(move |s: &mut State, tx: &mut Tx| s.copy_on_page(&what, details.clone(), tx));
    col()
        .key(format!("{key}-diagnosis"))
        .mt(8.0)
        .gap(8.0)
        .child(steps)
        .child(
            row()
                .key(format!("{key}-try"))
                .gap(4.0)
                .child(text("Try").font(small(600.0)).color(t.error_text))
                .child(text(d.next.clone()).font(small(400.0)).color(t.error_text).wrap().shrink().min_w(0.0)),
        )
        .child(row().child(copy))
}

/// `MachineAddNote`: what adding a machine means, in three points.
pub(super) fn add_note(t: &Theme, name: &str) -> El {
    let points = [
        format!("Sessions you start on {name} run there, with {name}'s sign-ins, folders and agents."),
        format!("This Fleet can run agents, change files and open terminals on {name}."),
        format!("{name} gives this Fleet a key of its own. Forget {name} here to stop."),
    ];
    let mut e = col().key("machines-add-note").w_full().gap(4.0);
    for p in points {
        e = e.child(
            row()
                .gap(8.0)
                .items_start()
                .child(text("•").font(sans(rem(0.75), 400.0, 1.33)).color(t.muted))
                .child(text(p).font(sans(rem(0.75), 400.0, 1.33)).color(t.muted).wrap().shrink().min_w(0.0)),
        );
    }
    e
}

fn found_card(s: &State, t: &Theme) -> Option<El> {
    let found = s.machine_page.add.found.as_ref().filter(|_| !s.machine_page.add.uses_token())?;
    Some(
        col()
            .key("machines-add-found")
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
                    .child(
                        row().gap(8.0).center_y().child(icon(&CHECK, 16.0).color(t.running)).child(
                            text(found.line()).key("machines-add-found-line").font(sans(rem(0.875), 500.0, 1.43)).color(t.text).wrap(),
                        ),
                    )
                    .child(
                        text(found.reach())
                            .key("machines-add-found-reach")
                            .font(sans(rem(0.75), 400.0, 1.33))
                            .color(t.muted)
                            .wrap()
                            .ml(24.0),
                    ),
            )
            .child(add_note(t, &found.name)),
    )
}
/// Words with commands in them: each `` `…` `` is drawn as `.machine-code` (mono at 0.85em on `--accent-dim`, 1px 5px padding, 4px corners).
/// Words with commands in them: each `` `…` `` is drawn as `.machine-code` (mono at 0.85em on `--accent-dim`).
pub(super) fn with_code(t: &Theme, key: &str, words: &str, size: f32, ink: rune::Color) -> El {
    let (mut content, mut spans) = (String::new(), Vec::new());
    for (i, part) in words.split('`').enumerate() {
        if i % 2 == 1 {
            spans.push(Span::new(content.len(), content.len() + part.len()).mono().size(size * 0.85).color(t.text).bg(t.accent_dim, 5.0));
        }
        content.push_str(part);
    }
    text(content).key(key.to_string()).font(sans(size, 400.0, 1.33)).color(ink).wrap().w_full().spans(spans).span_radius(4.0)
}

/// The tab strip, "How to add the machine": `flex gap-1 rounded-btn border p-1`, each tab `flex-1 px-3 py-1.5 font-medium`, the
/// picked one on `bg-accent/15`.
fn strip(s: &State, t: &Theme) -> El {
    let picked = s.machine_page.add.mode;
    let tabs = AddMode::ALL.iter().map(|&mode| {
        let on = mode == picked;
        row()
            .key(format!("machines-add-mode-{}", mode.id()))
            .role("tab")
            .label(mode.label().to_string())
            .grow(1.0)
            .min_w(0.0)
            .px(rem(0.75))
            .py(rem(0.375))
            .center()
            .radius(RADIUS_BTN)
            .fill(if on { t.accent.alpha(0.15) } else { rune::Color::TRANSPARENT })
            .cursor(Cursor::Pointer)
            .focusable(true)
            .focus_ring(3.0, 0.0, t.accent.alpha(0.5))
            .hover_group()
            .on_press_tx(move |s: &mut State, tx: &mut Tx| s.pick_add_mode(mode, tx))
            .child(
                text(mode.label().to_string())
                    .font(sans(rem(0.875), 500.0, 1.43))
                    .color(if on { t.text } else { t.muted })
                    .group_color(t.text),
            )
    });
    row()
        .key("machines-add-modes")
        .role("tablist")
        .label("How to add the machine")
        .w_full()
        .gap(4.0)
        .padding(4.0)
        .radius(RADIUS_BTN)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.main_bg)
        .children(tabs.collect::<Vec<_>>())
}

/// The form, under the "Add a machine" row: the strip, the picked tab's card, the buttons.
pub fn form(s: &State, t: &Theme) -> El {
    let f = &s.machine_page.add;
    let e = col()
        .key("machines-add-form")
        .w_full()
        .gap(12.0)
        .padding(16.0)
        .fill(t.main_bg)
        .child(text("Add a machine").font(sans(rem(0.875), 600.0, 1.43)).color(t.text))
        .child(strip(s, t));
    let mut e = match f.mode {
        AddMode::Found => e.child(super::found_machines::card(s, t)),
        AddMode::Server => e.child(super::new_server::card(s, t)),
        AddMode::Link => link_tab(s, t, e),
    };
    if let Some(why) = &f.error {
        e = e.child(error_note(t, "machines-add-error", why, false, copied(s, "machines-add-error")));
    }
    let cancel = plain_button(t, "machines-add-cancel".into(), "Cancel");
    let cancel = if f.adding { faded(cancel) } else { cancel.on_press(|s: &mut State| s.close_add_form()) };
    let mut buttons = row().flex_wrap().gap(8.0);
    if f.mode == AddMode::Link {
        let submit = if f.adding {
            faded(button_lead(t, Variant::Default, Size::Default, Some(spinner(t.primary_fg, 16.0)), "Checking…"))
        } else if f.can_add() {
            button(t, Variant::Default, Size::Default, None, &f.label()).on_press(|s: &mut State| s.submit_add())
        } else {
            faded(button(t, Variant::Default, Size::Default, None, &f.label()))
        };
        buttons = buttons.child(submit.key("machines-add-submit"));
    }
    e.child(buttons.child(cancel))
}

/// Paste a pairing link: the hint when a machine can't be set up from here, the help, the link and its code or token, the
/// machine found behind it.
fn link_tab(s: &State, t: &Theme, mut e: El) -> El {
    let f = &s.machine_page.add;
    let pasted = f.pasted();
    if let Some(name) = &f.link_hint {
        e = e.child(with_code(
            t,
            "machines-add-link-hint",
            &format!("Fleet can't set up {name} from here. On {name}, run `fleet node up` and paste the link it prints."),
            rem(0.75),
            t.text,
        ));
    }
    let mut e = e
        .child(with_code(
            t,
            "machines-add-link-help",
            "On the other machine, run `fleet node pair`. It prints a pairing link that works once for 10 minutes. Paste it here.",
            rem(0.75),
            t.muted,
        ))
        .child(labelled(
            t,
            "Pairing link",
            input(
                t,
                &f.link,
                LINK_FIELD,
                "Pairing link",
                "http://100.64.90.72:6262/add-machine#ABCD-EFGH",
                sans(rem(0.875), 400.0, 1.4),
                f.adding,
            )
            .on_edit(|s: &mut State| &mut s.machine_page.add.link),
            Some("Or just the machine's address, then its code."),
        ));
    match &pasted {
        Some(crate::fleet::machine::PairingInput::Invalid(why)) => {
            e = e.child(error_note(t, "machines-add-link-invalid", &Failure::said(why), true, false));
        }
        Some(crate::fleet::machine::PairingInput::Address { .. }) => {
            if !f.uses_token() {
                e = e.child(labelled(
                    t,
                    "Pairing code",
                    input(t, &f.code, CODE_FIELD, "Pairing code", "ABCD-EFGH", mono(rem(0.875), 400.0, 1.4).tracking(1.5), f.adding)
                        .on_edit(|s: &mut State| &mut s.machine_page.add.code),
                    None,
                ));
            }
            if f.advanced {
                e = e.child(labelled(
                    t,
                    "Access token",
                    input(t, &f.token, TOKEN_FIELD, "Access token", "Paste the machine's token", sans(rem(0.875), 400.0, 1.4), f.adding)
                        .on_edit(|s: &mut State| crate::fleet::machine_actions::masked(&mut s.machine_page.add.token)),
                    Some("This is the machine's full-access token. A pairing link is safer."),
                ));
            } else {
                e = e.child(
                    text("Advanced: use the machine's access token")
                        .key("machines-add-advanced")
                        .role("button")
                        .font(sans(rem(0.75), 400.0, 1.33))
                        .color(t.muted)
                        .cursor(Cursor::Pointer)
                        .hover(t.text.alpha(0.0))
                        .focusable(true)
                        .on_press_tx(|s: &mut State, tx: &mut Tx| s.show_add_advanced(tx)),
                );
            }
        }
        _ => {}
    }
    if f.looking {
        e = e.child(
            row()
                .key("machines-add-looking")
                .gap(8.0)
                .center_y()
                .child(spinner(t.muted, 14.0))
                .child(text("Looking for the machine…").font(sans(rem(0.75), 400.0, 1.33)).color(t.muted)),
        );
    }
    if let Some(why) = &f.preview_error {
        e = e.child(error_note(t, "machines-add-preview-error", why, false, copied(s, "machines-add-preview-error")));
    }
    if let Some(card) = found_card(s, t) {
        e = e.child(card);
    }
    e
}

/// "Added <name>. Its sessions are in the sessions list." (`justAdded`).
pub fn added(t: &Theme, name: &str) -> El {
    row()
        .key("machines-added")
        .role("status")
        .w_full()
        .gap(8.0)
        .center_y()
        .px(16.0)
        .py(12.0)
        .child(icon(&CHECK, 16.0).color(t.running))
        .child(
            text(format!("Added {name}. Its sessions are in the sessions list.")).font(sans(rem(0.875), 400.0, 1.43)).color(t.text).wrap(),
        )
}
