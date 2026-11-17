//! A harness's update strip (`HarnessUpdateStrip.vue`): that a newer version is out (or that Fleet needs one), the button, and
//! the update Fleet is running. Exported for Updates & about and the setup rows. Its own scoped CSS, so sizes are literal px.
use super::harness_install::{button, code};
use crate::State;
use crate::fleet::new_session::harness_choice::HarnessEntry;
use crate::fleet::settings::harness_update::{Piece, Strip, Tone, strip};
use crate::icons::{CHECK, CIRCLE_ARROW_UP, COPY};
use crate::theme::{RADIUS_BTN, Theme, mono, sans};
use crate::ui::confirm::spinner;
use rune::{Cursor, El, ScrollSpec, col, row, text};

fn tones(t: &Theme, tone: Tone) -> (rune::Color, rune::Color) {
    match tone {
        Tone::Plain => (t.border, t.main_bg),
        Tone::Warn => (t.idle.alpha(0.4), t.idle.alpha(0.1)),
        Tone::Ok => (t.running.alpha(0.35), t.running.alpha(0.1)),
        Tone::Error => (t.error.alpha(0.35), t.error.alpha(0.08)),
    }
}

fn message(t: &Theme, st: &Strip) -> El {
    let mut c = col().grow(1.0).min_w(260.0).shrink();
    if !st.lead.is_empty() {
        let mut lead = row()
            .w_full()
            .flex_wrap()
            .gap(4.0)
            .child(text(st.lead.clone()).font(sans(13.0, 600.0, 1.5)).color(t.text).wrap().shrink().min_w(0.0));
        if let Some(more) = &st.after_lead {
            lead = lead.child(text(more.clone()).font(sans(13.0, 400.0, 1.5)).color(t.text).shrink().min_w(0.0));
        }
        c = c.child(lead);
    }
    if !st.rest.is_empty() {
        let mut rest = row().w_full().flex_wrap().center_y().gap(4.0);
        for piece in &st.rest {
            rest = rest.child(match piece {
                Piece::Words(w) => text(w.clone()).font(sans(12.0, 400.0, 1.5)).color(t.muted).wrap().shrink().min_w(0.0),
                Piece::Code(cmd) => text(cmd.clone()).font(mono(11.5, 400.0, 1.5)).color(t.muted).wrap().shrink().min_w(0.0),
            });
        }
        c = c.child(rest);
    }
    c
}

/// The strip for `h`, on `machine`, or none when there is nothing to say. `key` keeps a page's strips apart.
pub fn view(s: &State, t: &Theme, h: &HarnessEntry, machine: &str) -> Option<El> {
    let error = s.settings.harnesses.strips.error.as_ref().filter(|(k, _)| *k == h.kind).map(|(_, e)| e.clone());
    let st = match strip(h) {
        Some(st) => st,
        None if error.is_some() => Strip::empty(),
        None => return None,
    };
    let busy = s.settings.harnesses.strips.busy;
    let (edge, fill) = tones(t, st.tone);
    let kind = h.kind.clone();
    let key = |name: &str| format!("harness-update-{name}-{kind}");
    let send = |method: &'static str| {
        let (machine, kind) = (machine.to_string(), kind.clone());
        move |s: &mut State| s.send_harness_update(&machine, &kind, method)
    };
    let off = |b: El| if busy { crate::ui::button::disabled(b) } else { b };
    let mut actions = row().center_y().gap(8.0);
    if st.busy {
        actions = actions.child(spinner(t.accent_text));
    }
    if st.cancel {
        actions = actions.child(off(button(t, None, "Cancel", true).key(key("cancel")).on_press(send("DELETE"))));
    }
    if let Some(label) = st.dismiss {
        actions = actions.child(off(button(t, None, label, true).key(key("dismiss")).on_press(send("DELETE"))));
    }
    if let Some(label) = &st.primary {
        let b = row()
            .key(key("start"))
            .role("button")
            .label(label.clone())
            .center_y()
            .gap(6.0)
            .px(11.0)
            .py(6.0)
            .radius(RADIUS_BTN)
            .fill(t.accent)
            .border(1.0, t.accent)
            .border_layout()
            .cursor(Cursor::Pointer)
            .focusable(true)
            .focus_ring(2.0, 2.0, t.accent)
            .hover(t.accent.alpha(0.9))
            .child(rune::icon(&CIRCLE_ARROW_UP, 14.0).color(t.primary_fg))
            .child(text(label.clone()).font(sans(12.5, 500.0, 1.45)).color(t.primary_fg))
            .on_press(send("POST"));
        actions = actions.child(off(b));
    }
    let mut c = col()
        .key(format!("harness-update-{}", h.kind))
        .w_full()
        .gap(10.0)
        .px(14.0)
        .py(12.0)
        .radius(RADIUS_BTN)
        .fill(fill)
        .border(1.0, edge)
        .border_layout()
        .child(row().w_full().flex_wrap().center_y().gap(10.0).child(message(t, &st)).child(actions));
    if let Some(why) = error {
        c = c.child(text(why.clone()).key(key("error")).role("alert").font(sans(12.5, 400.0, 1.5)).color(t.error_text).wrap().w_full());
    }
    if let Some(cmd) = &st.command {
        let copied = s.harness_copied(cmd);
        let owned = cmd.clone();
        c = c.child(
            row().w_full().flex_wrap().center_y().gap(8.0).child(code(t, &key("command"), cmd)).child(
                button(t, Some(if copied { &CHECK } else { &COPY }), if copied { "Copied" } else { "Copy" }, true)
                    .key(key("copy"))
                    .on_press_tx(move |s: &mut State, tx: &mut crate::Tx| s.copy_harness_command(&owned, tx)),
            ),
        );
    }
    if let Some(out) = &st.output {
        c = c.child(
            col()
                .key(key("output"))
                .w_full()
                .px(10.0)
                .py(8.0)
                .radius(6.0)
                .fill(t.card_bg)
                .border(1.0, t.border)
                .border_layout()
                .child(text(out.clone()).font(mono(11.5, 400.0, 1.5)).color(t.muted).wrap().w_full())
                .scroll(ScrollSpec { stick_bottom: false })
                .max_h(140.0),
        );
    }
    Some(c)
}
