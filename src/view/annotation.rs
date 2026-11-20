//! The annotation popover (`components/annotations/AnnotationPopover.vue`): the selected text quoted, a field for the question, Cancel
//! and Send. Fixed where the selection ended, kept 16 points inside the window; no scrim, as in Fleet, so the page behind stays
//! usable. What it does is in `fleet::annotation`.
use crate::fleet::annotation::{FIELD, WIDTH, clamp, preview};
use crate::theme::{Theme, sans};
use crate::ui::button::{Size, Variant, button};
use crate::ui::text_menu::text_input;
use crate::{State, Tx};
use rune::{El, Key, KeyEvent, Shadow, col, row, text};

/// The card, or nothing while no annotation is open.
pub fn overlay(s: &State, t: &Theme) -> Option<El> {
    let open = s.annotation.open.as_ref()?;
    let (x, y) = clamp(open.at, [s.ui.sel.window.0, s.ui.sel.window.1]);
    // `border-l-2 border-primary/40 bg-secondary px-3 py-2 text-sm italic text-muted-foreground`.
    let quote = col()
        .key("annotation-quote")
        .w_full()
        .px(12.0)
        .py(8.0)
        .fill(t.secondary())
        .border_sides([0.0, 0.0, 0.0, 2.0], t.accent.alpha(0.4))
        .child(text(format!("\"{}\"", preview(&open.anchor_text))).font(sans(14.0, 400.0, 1.43).italic()).color(t.muted).wrap());
    // The Textarea: `min-h-24 border-input px-3 py-2 text-sm`, a tint of the text colour behind it in the dark theme (`dark:bg-input/30`).
    let field = text_input(&s.annotation.field, sans(14.0, 400.0, 1.43))
        .key(FIELD)
        .label("Add your annotation…")
        .placeholder("Add your annotation…", t.muted)
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .w_full()
        .min_h(84.0)
        .px(12.0)
        .py(8.0)
        .fill(if t.dark { t.wash(5.4) } else { rune::Color::TRANSPARENT })
        .border(1.0, t.input())
        .border_layout()
        .focus_ring(3.0, 0.0, t.accent.alpha(0.5))
        .on_edit(|s: &mut State| &mut s.annotation.field)
        .on_input_key(|s: &mut State, ev: &KeyEvent, tx: &mut Tx| {
            if ev.key == Key::Escape {
                s.close_annotation();
                return true;
            }
            if ev.key == Key::Enter && (ev.mods.cmd || ev.mods.ctrl) {
                s.send_annotation(tx);
                return true;
            }
            false
        });
    let cancel =
        button(t, Variant::Outline, Size::Sm, None, "Cancel").key("annotation-cancel").on_press(|s: &mut State| s.close_annotation());
    let mut send = button(t, Variant::Default, Size::Sm, None, "Send").key("annotation-send");
    if s.annotation.can_send() {
        send = send.on_press_tx(|s: &mut State, tx: &mut Tx| s.send_annotation(tx));
    } else {
        send = crate::ui::button::disabled(send);
    }
    let actions = row().w_full().gap(8.0).items_center().justify_end().child(cancel).child(send);
    // `border border-border bg-card p-4 shadow-xl shadow-black/50 ring-1 ring-white/[0.08]`, square.
    let card = col()
        .key("annotation-popover")
        .role("dialog")
        .label("Annotation")
        .absolute(x, y)
        .w(WIDTH)
        .padding(16.0)
        .gap(12.0)
        .items_stretch()
        .fill(t.card_bg)
        .border(1.0, t.border)
        .border_layout()
        .box_shadow(Shadow { x: 0.0, y: 20.0, blur: 25.0, spread: -5.0, color: rune::Color::BLACK.alpha(0.5), inset: false })
        .box_shadow(Shadow { x: 0.0, y: 8.0, blur: 10.0, spread: -6.0, color: rune::Color::BLACK.alpha(0.5), inset: false })
        .child(quote)
        .child(field)
        .child(actions);
    Some(card)
}
