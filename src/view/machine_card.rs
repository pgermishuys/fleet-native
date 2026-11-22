//! One machine's card on Settings → Machines (`MachinesSection.vue`, the `<li class="machine-card">` and its scoped CSS) and the
//! Details panel under it (`MachineDiagnosisPanel.vue`). Home's card has Rename only; another machine's also has Work here, New
//! token and Forget, and the "Agents here can send it work" setting. Every word comes from `fleet::machine_card`.
use super::settings::kit;
use crate::fleet::machine_actions::{RENAME_FIELD, TOKEN_FIELD, parse_agent_work};
use crate::fleet::machine_card::{Card, Tone};
use crate::fleet::machine_diagnosis::{Diagnosis, StepState};
use crate::fleet::machines::AgentWork;
use crate::icons::{CHECK, CHEVRON_DOWN, CHEVRON_RIGHT, CIRCLE, CIRCLE_ALERT, X};
use crate::theme::{RADIUS_BTN, Theme, mono, sans};
use crate::ui::rem;
use crate::ui::text_menu::text_input;
use crate::{State, Tx};
use rune::{Cursor, El, Key, KeyEvent, col, el, icon, row, text};
use std::rc::Rc;

/// `buttonSecondaryClass`: bordered, on the page's colour, 12.25px medium; 30px high.
pub fn plain_button(t: &Theme, key: String, label: &str) -> El {
    row()
        .key(key)
        .role("button")
        .label(label.to_string())
        .h(rem(2.14))
        .px(rem(0.75))
        .center()
        .radius(RADIUS_BTN)
        .fill(t.main_bg)
        .border(1.0, t.border)
        .border_layout()
        .hover(t.accent.alpha(0.1))
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(3.0, 0.0, t.accent.alpha(0.5))
        .child(text(label.to_string()).font(sans(rem(0.875), 500.0, 1.4)).color(t.text))
}

/// A disabled button: half faded and not pressable.
pub fn faded(e: El) -> El {
    e.opacity(0.6).disabled(true).cursor(Cursor::Default)
}

/// `inputClass` at `py-1`: a 1px border, the page's colour, 12.25px text, an accent border while focused.
fn field(t: &Theme, edit: &rune::TextEdit, key: &str, label: &str, placeholder: &str, width: f32) -> El {
    text_input(edit, sans(rem(0.875), 400.0, 1.4))
        .key(key.to_string())
        .label(label.to_string())
        .single_line()
        .placeholder(placeholder.to_string(), t.muted)
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .w(width)
        .px(rem(0.75))
        .py(rem(0.25))
        .radius(RADIUS_BTN)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.main_bg)
        .focus_ring(1.0, 0.0, t.accent)
}

fn tone_ink(t: &Theme, tone: Tone) -> rune::Color {
    match tone {
        Tone::Ok => t.running_text,
        Tone::Bad => t.error_text,
        Tone::Muted => t.muted,
    }
}

fn os_chip(t: &Theme, os: &str) -> El {
    row()
        .px(5.0)
        .radius(4.0)
        .border(1.0, t.border)
        .border_layout()
        .child(text(os.to_uppercase()).font(mono(10.0, 600.0, 1.4).tracking(0.5)).color(t.muted))
}

fn live_chip(t: &Theme) -> El {
    row()
        .px(5.0)
        .py(1.0)
        .radius(4.0)
        .fill(t.coral.alpha(0.14))
        .child(text("LIVE").key("machine-live").font(sans(9.5, 700.0, 1.4).tracking(0.855)).color(t.coral_text))
}

/// The Details disclosure and, open, the six steps (`MachineDiagnosisPanel`).
fn details(s: &State, t: &Theme, card: &Card, why: &Diagnosis) -> El {
    let open = s.machine_page.details.contains(&card.key);
    let key = card.key.clone();
    let summary = row()
        .key(format!("machine-details-{}", card.key))
        .role("button")
        .label("Details")
        .center_y()
        .gap(2.0)
        .cursor(Cursor::Pointer)
        .focusable(true)
        .on_press(move |s: &mut State| s.toggle_machine_details(&key))
        .child(icon(if open { &CHEVRON_DOWN } else { &CHEVRON_RIGHT }, 12.0).color(t.muted))
        .child(text("Details").font(sans(rem(0.75), 400.0, 1.33)).color(t.muted));
    let mut e = col().mt(4.0).child(summary);
    if open {
        e = e.child(panel(s, t, card, why));
    }
    e
}

fn panel(s: &State, t: &Theme, card: &Card, why: &Diagnosis) -> El {
    let small = |w: f32| sans(rem(0.75), w, 1.33);
    let mut steps = col().gap(4.0);
    for step in &why.steps {
        let (glyph, ink) = match step.state {
            StepState::Ok => (&CHECK, t.running),
            StepState::Failed => (&X, t.error_text),
            StepState::NotChecked => (&CIRCLE, t.muted),
        };
        let muted = step.state == StepState::NotChecked;
        let label = if muted { format!("{} · not checked", step.label) } else { step.label.clone() };
        let mut words = col().shrink().min_w(0.0).child(text(label).font(small(400.0)).color(if muted { t.muted } else { t.text }).wrap());
        if let Some(detail) = &step.detail {
            words = words.child(text(detail.clone()).font(small(400.0)).color(t.muted).wrap().w_full());
        }
        steps = steps.child(row().gap(8.0).items_start().child(icon(glyph, 14.0).color(ink).mt(2.0)).child(words));
    }
    let (key, copy_key) = (card.key.clone(), card.key.clone());
    let copied = s.machine_page.copied.as_ref().is_some_and(|(what, _)| *what == format!("diagnosis-{}", card.key));
    let text_to_copy = why.text();
    let buttons = row()
        .gap(8.0)
        .child(
            plain_button(t, format!("machine-diagnosis-retry-{}", card.key), "Try again now")
                .on_press(move |s: &mut State| s.retry_machine(&key)),
        )
        .child(
            plain_button(t, format!("machine-diagnosis-copy-{}", card.key), if copied { "Copied" } else { "Copy details" })
                .on_press_tx(move |s: &mut State, tx: &mut Tx| s.copy_on_page(&format!("diagnosis-{copy_key}"), text_to_copy.clone(), tx)),
        );
    col()
        .key(format!("machine-diagnosis-{}", card.key))
        .mt(8.0)
        .gap(8.0)
        .child(steps)
        .child(
            row()
                .key(format!("machine-diagnosis-try-{}", card.key))
                .gap(4.0)
                .child(text("Try").font(small(600.0)).color(t.text))
                .child(text(why.next.clone()).font(small(400.0)).color(t.text).wrap().shrink().min_w(0.0)),
        )
        .child(buttons)
}

/// "Agents here can send it work": Ask / Always / Never and the level the work runs under.
fn agent_work(s: &State, t: &Theme, card: &Card, current: AgentWork, level: &str) -> El {
    let busy = s.machine_page.busy.as_deref() == Some(card.key.as_str());
    let machine = card.key.clone();
    let segments = kit::segments(
        t,
        &format!("machine-agent-work-{}", card.key),
        "Agents here can send it work",
        &[("ask", "Ask"), ("always", "Always"), ("never", "Never")],
        match current {
            AgentWork::Ask => "ask",
            AgentWork::Always => "always",
            AgentWork::Never => "never",
        },
        busy,
        Rc::new(move |s: &mut State, word: &str| s.set_agent_work(&machine, parse_agent_work(word).unwrap_or(AgentWork::Ask))),
    );
    col()
        .key(format!("machine-agent-work-{}", card.key))
        .mt(6.0)
        .gap(4.0)
        .child(
            row()
                .flex_wrap()
                .center_y()
                .gap(10.0)
                .child(text("Agents here can send it work").font(sans(12.5, 400.0, 1.4)).color(t.text))
                .child(segments),
        )
        .child(
            text(format!("Work there runs under {}'s level: {level}.", card.name))
                .key(format!("machine-agent-work-level-{}", card.key))
                .font(sans(12.0, 400.0, 1.4))
                .color(t.muted)
                .wrap()
                .w_full(),
        )
}

/// The name field while renaming, with Save and Cancel (Enter saves, Escape cancels).
fn renaming(s: &State, t: &Theme) -> El {
    let page = &s.machine_page;
    let busy = page.renaming.as_deref().is_some_and(|k| page.busy.as_deref() == Some(k));
    let input = field(t, &page.rename, RENAME_FIELD, "New name", "", 240.0)
        .on_edit(|s: &mut State| &mut s.machine_page.rename)
        .on_input_key(|s: &mut State, ev: &KeyEvent, tx: &mut Tx| match ev.key {
            Key::Enter => {
                s.save_card_rename(tx);
                true
            }
            Key::Escape => {
                s.cancel_card_rename();
                true
            }
            _ => false,
        });
    let save = plain_button(t, "machine-rename-save".into(), "Save");
    let save = if busy { faded(save) } else { save.on_press_tx(|s: &mut State, tx: &mut Tx| s.save_card_rename(tx)) };
    let cancel = plain_button(t, "machine-rename-cancel".into(), "Cancel").on_press(|s: &mut State| s.cancel_card_rename());
    row().flex_wrap().center_y().gap(8.0).child(input).child(save).child(cancel)
}

/// The new-token field, saved with Enter or Save.
fn token_form(s: &State, t: &Theme, name: &str) -> El {
    let page = &s.machine_page;
    let busy = page.token_for.as_deref().is_some_and(|k| page.busy.as_deref() == Some(k));
    let input = field(t, &page.token, TOKEN_FIELD, &format!("New token for {name}"), "Paste the new token", 320.0)
        .on_edit(|s: &mut State| &mut s.machine_page.token)
        .on_input_key(|s: &mut State, ev: &KeyEvent, _: &mut Tx| match ev.key {
            Key::Enter => {
                s.save_machine_token();
                true
            }
            Key::Escape => {
                s.cancel_machine_token();
                true
            }
            _ => false,
        });
    let empty = page.token.text.trim().is_empty();
    let save = plain_button(t, "machine-token-save".into(), "Save");
    let save = if busy || empty { faded(save) } else { save.on_press(|s: &mut State| s.save_machine_token()) };
    let cancel = plain_button(t, "machine-token-cancel".into(), "Cancel").on_press(|s: &mut State| s.cancel_machine_token());
    row().mt(8.0).flex_wrap().center_y().gap(8.0).child(input).child(save).child(cancel)
}

/// `ErrorNote` (`compact`): one line under the card's fields.
fn error_line(t: &Theme, key: String, why: &str) -> El {
    row()
        .key(key)
        .role("alert")
        .mt(4.0)
        .gap(8.0)
        .items_start()
        .child(icon(&CIRCLE_ALERT, 14.0).color(t.error_text).mt(1.0))
        .child(text(why.to_string()).font(sans(rem(0.75), 400.0, 1.33)).color(t.error_text).wrap().shrink().min_w(0.0))
}

pub fn card(s: &State, t: &Theme, c: &Card) -> El {
    let page = &s.machine_page;
    let dot = el().key(format!("machine-dot-{}", c.key)).size(8.0, 8.0).mt(6.0).radius(4.0).fill(match c.tone {
        Tone::Ok => t.running,
        Tone::Bad => t.error,
        Tone::Muted => t.muted,
    });
    let is_renaming = page.renaming.as_deref() == Some(c.key.as_str());
    let mut names = col().grow(1.0).shrink().min_w(0.0).gap(2.0);
    names = if is_renaming {
        names.child(renaming(s, t))
    } else {
        let mut line = row()
            .flex_wrap()
            .center_y()
            .gap(7.0)
            .child(text(c.name.clone()).key(format!("machine-name-{}", c.key)).font(mono(13.5, 600.0, 1.4)).color(t.text).wrap());
        if let Some(os) = &c.os {
            line = line.child(os_chip(t, os));
        }
        if c.live {
            line = line.child(live_chip(t));
        }
        names.child(line)
    };
    let small = mono(11.5, 400.0, 1.4);
    names = names
        .child(text(c.location.clone()).key(format!("machine-location-{}", c.key)).font(small).color(t.muted).wrap().w_full())
        .child(text(c.state.clone()).key(format!("machine-state-{}", c.key)).font(small).color(tone_ink(t, c.tone)).wrap().w_full());
    if let Some(r) = &c.route {
        names = names.child(super::machine_addresses::route(t, &c.key, r));
    }
    if let Some(list) = super::machine_addresses::list(s, t, c) {
        names = names.child(list);
    }
    if let Some(line) = &c.capabilities {
        let mut e = text(line.clone()).key(format!("machine-capabilities-{}", c.key)).font(small).color(t.muted).wrap().w_full();
        if let Some(versions) = &c.versions {
            e = e.help(versions.clone());
        }
        names = names.child(e);
    }
    if let Some((work, level)) = &c.agent_work {
        names = names.child(agent_work(s, t, c, *work, level));
    }
    if let Some(seats) = super::machine_job_seats::view(s, t, &c.key) {
        names = names.child(seats);
    }
    if page.token_for.as_deref() == Some(c.key.as_str()) {
        names = names.child(token_form(s, t, &c.name));
    }
    if let Some(why) = &c.diagnosis {
        names = names.child(details(s, t, c, why));
    }
    if let Some(why) = page.errors.get(&c.key) {
        names = names.child(error_line(t, format!("machine-error-{}", c.key), why));
    }

    let mut actions = row().flex_wrap().gap(6.0).items_start().justify_end();
    if !is_renaming {
        let (open, rename, token, forget) = (c.key.clone(), c.key.clone(), c.key.clone(), c.key.clone());
        if !c.live {
            actions = actions
                .child(plain_button(t, format!("machine-open-{}", c.key), "Work here").on_press(move |s: &mut State| s.work_here(&open)));
        }
        actions = actions.child(
            plain_button(t, format!("machine-rename-{}", c.key), "Rename")
                .on_press_tx(move |s: &mut State, tx: &mut Tx| s.start_card_rename(&rename, tx)),
        );
        if !c.is_home {
            actions = actions
                .child(
                    plain_button(t, format!("machine-token-{}", c.key), "New token")
                        .on_press_tx(move |s: &mut State, tx: &mut Tx| s.start_machine_token(&token, tx)),
                )
                .child(
                    plain_button(t, format!("machine-forget-{}", c.key), "Forget")
                        .on_press_tx(move |s: &mut State, tx: &mut Tx| s.ask_forget_machine(&forget, tx)),
                );
        }
    }
    row()
        .key(format!("machine-card-{}", c.key))
        .w_full()
        .flex_wrap()
        .items_start()
        .gap(12.0)
        .px(16.0)
        .py(12.0)
        .child(dot)
        .child(names)
        .child(actions)
}
