//! The small controls: `badge/`, `input/`, `textarea/`, `switch/`, `separator/`, `tabs/`,
//! `progress/`, `card/`, `avatar/`. Each is a function returning an element the caller keys.
use super::rem;
use crate::theme::{RADIUS_CARD, Theme, sans};
use crate::ui::text_menu::text_input;
use rune::{Color, Cursor, El, Icon, TextEdit, auto_grow, col, el, icon, row, text};

// ---- badge -----------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BadgeVariant {
    Default,
    Secondary,
    Destructive,
    Outline,
    /// `border-current bg-transparent`: the border takes the colour of its text.
    Status(StatusInk),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StatusInk {
    Running,
    Idle,
    Error,
    Muted,
}

/// `badgeVariants`: square (`rounded-none`), `px-2 py-0.5 text-xs`.
pub fn badge(t: &Theme, v: BadgeVariant, label: &str) -> El {
    let (ink, fill, border) = match v {
        BadgeVariant::Default => (t.primary_fg, t.accent, Color::TRANSPARENT),
        BadgeVariant::Secondary => (t.text, t.secondary(), Color::TRANSPARENT),
        BadgeVariant::Destructive => (Color::WHITE, if t.dark { t.destructive.alpha(0.6) } else { t.destructive }, Color::TRANSPARENT),
        BadgeVariant::Outline => (t.text, Color::TRANSPARENT, t.border),
        BadgeVariant::Status(c) => {
            let c = match c {
                StatusInk::Running => t.running,
                StatusInk::Idle => t.idle,
                StatusInk::Error => t.error,
                StatusInk::Muted => t.muted,
            };
            (c, Color::TRANSPARENT, c)
        }
    };
    row()
        .px(rem(0.5))
        .py(rem(0.125))
        .gap(rem(0.25))
        .center()
        .fill(fill)
        .border(1.0, border)
        .child(text(label.to_string()).font(sans(rem(0.75), 500.0, 1.33)).color(ink))
}

// ---- input and textarea ----------------------------------------------------------------------

/// `Input.vue`: 31.5px high, a 1px `--input` border, no radius (`rounded-none`), 12.25px text,
/// a 3px accent ring while focused.
pub fn input(t: &Theme, edit: &TextEdit, placeholder: &str) -> El {
    text_input(edit, sans(rem(0.875), 400.0, 1.3))
        .single_line()
        .placeholder(placeholder.to_string(), t.muted)
        .label(placeholder.to_string())
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .w_full()
        .h(rem(2.25))
        .px(rem(0.75))
        .py((rem(2.25) - rem(0.875) * 1.3) / 2.0 - 1.0)
        .border(1.0, t.input())
        .focus_ring(3.0, 0.0, t.accent.alpha(0.5))
}

/// `Textarea.vue`: at least 64px (`min-h-16` is 56px at 14px rem), grows with its text.
pub fn textarea(t: &Theme, edit: &TextEdit, placeholder: &str, max_lines: usize) -> El {
    grow(textarea_field(t, edit, placeholder), max_lines)
}

/// A textarea's field before it is wrapped to grow: a caller that keys it and attaches handlers does that first, then `grow`s it
/// (the wrapper takes the key `scroll`, so a key set on the wrapper would replace it).
pub fn textarea_field(t: &Theme, edit: &TextEdit, placeholder: &str) -> El {
    text_input(edit, sans(rem(0.875), 400.0, 1.4))
        .placeholder(placeholder.to_string(), t.muted)
        .label(placeholder.to_string())
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .w_full()
        .px(rem(0.75))
        .py(rem(0.5))
        .border(1.0, t.input())
        .min_h(rem(4.0))
        .focus_ring(3.0, 0.0, t.accent.alpha(0.5))
}

/// Makes a `textarea_field` fit its text, up to `max_lines`.
pub fn grow(field: El, max_lines: usize) -> El {
    auto_grow(field, 3, max_lines)
}

// ---- switch ----------------------------------------------------------------------------------

/// `Switch.vue`: a 28 × 16.1 track, a 14px thumb that slides `100% - 2px`. The caller adds the
/// key and the toggle handler.
pub fn switch(t: &Theme, on: bool) -> El {
    let off = if t.dark { t.input().alpha(0.18 * 0.8) } else { t.input() };
    switch_with(t, on, off)
}

/// `switch` with its own unchecked track (Automations draws "off" in 22 % text so it shows against the bar).
pub fn switch_with(t: &Theme, on: bool, off: Color) -> El {
    let (w, h, thumb) = (rem(2.0), rem(1.15), rem(1.0));
    let track = if on { t.accent } else { off };
    // The unchecked track is `bg-input` (`dark:bg-input/80`); the thumb is `--background` or, in
    // dark, the text colour when off and the primary foreground when on.
    let knob = if t.dark { if on { t.primary_fg } else { t.text } } else { t.panel_bg };
    row()
        .w(w)
        .h(h)
        .center_y()
        .radius(h / 2.0)
        .fill(track)
        .border(1.0, Color::TRANSPARENT)
        .cursor(Cursor::Pointer)
        .focus_ring(3.0, 0.0, t.accent.alpha(0.5))
        .role("switch")
        .checked(on)
        .child(el().size(thumb, thumb).radius(thumb / 2.0).fill(knob).ml(if on { w - thumb - 2.0 } else { 0.0 }))
}

// ---- separator -------------------------------------------------------------------------------

pub fn separator(t: &Theme) -> El {
    el().w_full().h(1.0).fill(t.border)
}

pub fn separator_v(t: &Theme) -> El {
    el().w(1.0).h_full().fill(t.border)
}

// ---- tabs ------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TabsVariant {
    /// The segmented track (`bg-secondary h-9 p-[3px]`).
    Default,
    /// Text with a 2px accent underline (`variant="underline"`).
    Underline,
}

/// One tab: `(key, label, active)`. The caller adds `.on_press` to each returned element by
/// building the list with `tab` and passing them to `tabs`.
pub fn tab(t: &Theme, v: TabsVariant, key: &str, label: &str, active: bool) -> El {
    match v {
        TabsVariant::Default => row()
            .key(key.to_string())
            .h_full()
            .px(rem(0.5))
            .py(rem(0.25))
            .center()
            .grow(1.0)
            .fill(if active { if t.dark { t.wash(5.4) } else { t.panel_bg } } else { Color::TRANSPARENT })
            .border(1.0, if active && t.dark { t.input() } else { Color::TRANSPARENT })
            .cursor(Cursor::Pointer)
            .focus_ring(3.0, 0.0, t.accent.alpha(0.5))
            .role("tab")
            .child(text(label.to_string()).font(sans(rem(0.875), 500.0, 1.4)).color(if t.dark && !active { t.muted } else { t.text })),
        TabsVariant::Underline => col()
            .key(key.to_string())
            .py(8.0)
            .px(2.0)
            .mb(-1.0)
            .cursor(Cursor::Pointer)
            .border_sides([0.0, 0.0, 2.0, 0.0], if active { t.accent } else { Color::TRANSPARENT })
            .focus_ring(3.0, 0.0, t.accent.alpha(0.5))
            .role("tab")
            .child(text(label.to_string()).font(sans(13.0, 500.0, 1.4)).color(if active { t.text } else { t.muted }).group_color(t.text))
            .hover_group(),
    }
}

pub fn tabs(t: &Theme, v: TabsVariant, items: Vec<El>) -> El {
    match v {
        TabsVariant::Default => row().h(rem(2.25)).padding(3.0).fill(t.secondary()).items_stretch().role("tablist").children(items),
        TabsVariant::Underline => {
            row().h(rem(2.5)).gap(rem(1.0)).border_sides([0.0, 0.0, 1.0, 0.0], t.border).center_y().role("tablist").children(items)
        }
    }
}

// ---- progress --------------------------------------------------------------------------------

/// `Progress.vue`: 7px high, a primary 20% track and a primary fill, `value` in 0..=100.
pub fn progress(t: &Theme, value: f32, width: f32) -> El {
    let h = rem(0.5);
    row()
        .w(width)
        .h(h)
        .radius(h / 2.0)
        .fill(t.accent.alpha(0.2))
        .clip_rounded(h / 2.0)
        .child(el().h(h).w(width * value.clamp(0.0, 100.0) / 100.0).fill(t.accent))
}

// ---- card ------------------------------------------------------------------------------------

/// `Card.vue`: `rounded-card border py-6 shadow-sm`, 21px between its parts.
pub fn card(t: &Theme) -> El {
    col().radius(RADIUS_CARD).fill(t.card_bg).border(1.0, t.border).py(rem(1.5)).gap(rem(1.5))
}

pub fn card_header(title: &str, description: Option<&str>, t: &Theme) -> El {
    let mut h = col().px(rem(1.5)).gap(rem(0.375)).child(text(title.to_string()).font(sans(14.0, 600.0, 1.0)).color(t.text));
    if let Some(d) = description {
        h = h.child(text(d.to_string()).font(sans(rem(0.875), 400.0, 1.4)).color(t.muted));
    }
    h
}

pub fn card_content() -> El {
    col().px(rem(1.5))
}

pub fn card_footer() -> El {
    row().center_y().px(rem(1.5))
}

// ---- avatar ----------------------------------------------------------------------------------

/// `Avatar.vue` with its fallback: a 28px circle on `--muted`, the first letter centred.
pub fn avatar(t: &Theme, letter: &str) -> El {
    let s = rem(2.0);
    el().size(s, s)
        .radius(s / 2.0)
        .fill(t.muted.alpha(0.25))
        .center()
        .child(text(letter.to_string()).font(sans(rem(0.875), 500.0, 1.0)).color(t.text))
}

// ---- collapsible -----------------------------------------------------------------------------

/// `Collapsible`: a trigger row with a chevron; the caller shows the content when `open`.
pub fn collapsible_trigger(t: &Theme, label: &str, open: bool, chevron: &'static Icon, chevron_open: &'static Icon) -> El {
    row()
        .gap(rem(0.5))
        .center_y()
        .cursor(Cursor::Pointer)
        .focus_ring(3.0, 0.0, t.accent.alpha(0.5))
        .role("button")
        .expanded(open)
        .child(icon(if open { chevron_open } else { chevron }, rem(1.0)).color(t.muted))
        .child(text(label.to_string()).font(sans(rem(0.875), 500.0, 1.4)).color(t.text))
}
