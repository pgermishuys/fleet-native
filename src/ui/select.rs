//! `select/`: a 31.5px trigger (`size="sm"` 28) with a chevron at half strength, and a content
//! panel at least as wide as the trigger. A highlighted row is a solid accent fill.
use super::overlay::{anchored, scrim};
use super::rem;
use crate::State;
use crate::theme::{Theme, sans};
use rune::{Align, Color, Cursor, El, Side, col, icon, row, text};
use std::rc::Rc;

pub type Choose = Rc<dyn Fn(&mut State, &str)>;

/// `SelectTrigger.vue`. The caller keys it (the panel anchors to that key) and opens it.
pub fn trigger(t: &Theme, label: &str, placeholder: bool, small: bool, width: f32) -> El {
    row()
        .w(width)
        .h(if small { rem(2.0) } else { rem(2.25) })
        .px(rem(0.75))
        .gap(rem(0.5))
        .center_y()
        .justify_between()
        .border(1.0, t.input())
        .hover(if t.dark { t.wash(4.0) } else { Color::TRANSPARENT })
        .cursor(Cursor::Pointer)
        .focus_ring(3.0, 0.0, t.accent.alpha(0.5))
        .role("combobox")
        .label(label.to_string())
        .child(
            text(label.to_string())
                .font(sans(rem(0.875), 400.0, 1.3))
                .color(if placeholder { t.muted } else { t.text })
                .truncate()
                .shrink(),
        )
        .child(icon(&crate::icons::CHEVRON_DOWN, rem(1.0)).color(t.muted).opacity(0.5))
}

/// The open list. `options` are `(value, label)`.
pub fn content(t: &Theme, key: &str, anchor: &str, width: f32, options: &[(&str, &str)], current: Option<&str>, choose: Choose) -> Vec<El> {
    let close: Rc<dyn Fn(&mut State)> = Rc::new(|s: &mut State| s.ui.open = None);
    content_in(t, key, anchor, width, options, current, choose, close, 0.0)
}

/// `content` for a list that sits in another popover: `close` ends only the list (the popover stays), and the panel has the
/// `radius` that popover gives it (`.ns-select-content`).
#[allow(clippy::too_many_arguments)]
pub fn content_in(
    t: &Theme,
    key: &str,
    anchor: &str,
    width: f32,
    options: &[(&str, &str)],
    current: Option<&str>,
    choose: Choose,
    close: Rc<dyn Fn(&mut State)>,
    radius: f32,
) -> Vec<El> {
    let mut list = col().key("rows").items_stretch();
    for (i, (value, label)) in options.iter().enumerate() {
        let on = current == Some(*value);
        let v = value.to_string();
        let choose = choose.clone();
        list = list.child(
            row()
                .key(format!("{key}-opt{i}"))
                .role("option")
                .label(label.to_string())
                .py(rem(0.375))
                .pl(rem(0.5))
                .pr(rem(0.5))
                .center_y()
                .radius(3.5)
                .cursor(Cursor::Default)
                .hover_group()
                .focus_highlight(t.accent)
                .hover(t.accent)
                .focusable(true)
                .focus_selected(on)
                .child(text(label.to_string()).font(sans(rem(0.875), 400.0, 1.3)).color(t.text).group_color(t.primary_fg).grow(1.0))
                .child(
                    icon(&crate::icons::CHECK, rem(1.0))
                        .color(t.text)
                        .group_color(t.primary_fg)
                        .opacity(if on { 1.0 } else { 0.0 })
                        .ml(rem(0.5)),
                )
                .on_press_tx({
                    let close = close.clone();
                    move |s: &mut State, _| {
                        close(s);
                        choose(s, &v);
                    }
                }),
        );
    }
    let mut p = col()
        .key(format!("{key}-menu"))
        .role("listbox")
        .focus_group()
        .min_w(width)
        .padding(rem(0.25))
        .fill(t.card_bg)
        .border(1.0, t.border)
        .items_stretch()
        .child(list)
        .radius(radius)
        .focus_return(anchor.to_string())
        .modal(true, {
            let close = close.clone();
            move |s: &mut State| close(s)
        });
    for s in t.menu_shadows() {
        p = p.box_shadow(s);
    }
    vec![scrim(&format!("{key}-scrim"), move |s: &mut State| close(s)), anchored(p, anchor, Side::Bottom, Align::Start, 4.0)]
}
