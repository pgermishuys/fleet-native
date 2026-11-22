//! `ui/button/index.ts` (`buttonVariants`) and `Button.vue`: nine variants, eight sizes.
use super::rem;
use crate::theme::{RADIUS_BTN, Theme, sans};
use rune::{Color, Cursor, El, Icon, el, icon, row, text};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Variant {
    Default,
    Destructive,
    Outline,
    Secondary,
    Ghost,
    Link,
    ToolbarIcon,
    ToolbarIconDanger,
    Filter { active: bool },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Size {
    Default,
    Sm,
    Lg,
    Icon,
    IconSm,
    IconLg,
    Toolbar,
    ToolbarLg,
}

impl Size {
    /// `h-9 px-4`, `h-8 px-3`, `h-10 px-6` and the square sizes, in Fleet's 14px rem.
    fn dims(self, has_icon: bool) -> (f32, f32, f32) {
        match self {
            Size::Default => (rem(2.25), if has_icon { rem(0.75) } else { rem(1.0) }, rem(0.5)),
            Size::Sm => (rem(2.0), if has_icon { rem(0.625) } else { rem(0.75) }, rem(0.375)),
            Size::Lg => (rem(2.5), if has_icon { rem(1.0) } else { rem(1.5) }, rem(0.5)),
            Size::Icon => (rem(2.25), 0.0, 0.0),
            Size::IconSm => (rem(2.0), 0.0, 0.0),
            Size::IconLg => (rem(2.5), 0.0, 0.0),
            Size::Toolbar => (rem(1.75), 0.0, 0.0),
            Size::ToolbarLg => (rem(2.0), 0.0, 0.0),
        }
    }

    fn square(self) -> bool {
        matches!(self, Size::Icon | Size::IconSm | Size::IconLg | Size::Toolbar | Size::ToolbarLg)
    }
}

/// The label colour, fill and border of a variant at rest, and its hover wash.
struct Look {
    ink: Color,
    fill: Color,
    border: Color,
    hover_fill: Color,
    hover_ink: Option<Color>,
}

fn look(t: &Theme, v: Variant) -> Look {
    let none = Color::TRANSPARENT;
    let base = Look { ink: t.text, fill: none, border: none, hover_fill: none, hover_ink: None };
    match v {
        // `hover:bg-primary/90`: the page shows through a tenth.
        Variant::Default => Look { ink: t.primary_fg, fill: t.accent, hover_fill: t.panel_bg.alpha(0.1), ..base },
        // `dark:bg-destructive/60`.
        Variant::Destructive => Look {
            ink: Color::WHITE,
            fill: if t.dark { t.destructive.alpha(0.6) } else { t.destructive },
            hover_fill: t.panel_bg.alpha(0.1),
            ..base
        },
        Variant::Outline => Look { fill: t.panel_bg, border: t.border, hover_fill: t.accent.alpha(0.1), ..base },
        Variant::Secondary => Look { fill: t.secondary(), hover_fill: t.panel_bg.alpha(0.2), ..base },
        Variant::Ghost => Look { hover_fill: t.accent.alpha(0.1), ..base },
        Variant::Link => Look { ink: t.accent, ..base },
        Variant::ToolbarIcon => Look { ink: t.muted, hover_fill: t.wash(6.0), hover_ink: Some(t.text), ..base },
        Variant::ToolbarIconDanger => Look { ink: t.muted, hover_fill: t.error.alpha(0.1), hover_ink: Some(t.error_text), ..base },
        Variant::Filter { active } => Look {
            ink: if active { t.accent_text } else { t.muted },
            fill: if active { t.accent_dim } else { none },
            hover_fill: if active { none } else { t.main_bg },
            hover_ink: if active { None } else { Some(t.text) },
            ..base
        },
    }
}

/// A button with an optional icon and label. Chain `.key(..)` and `.on_press(..)` on the result.
pub fn button(t: &Theme, v: Variant, size: Size, glyph: Option<&'static Icon>, label: &str) -> El {
    let l = look(t, v);
    let glyph_size = if size.square() && matches!(v, Variant::ToolbarIcon | Variant::ToolbarIconDanger) { rem(0.9) } else { rem(1.0) };
    let lead = glyph.map(|g| {
        let i = icon(g, glyph_size).color(l.ink);
        match l.hover_ink {
            Some(c) => i.group_color(c),
            None => i,
        }
    });
    button_lead(t, v, size, lead, label)
}

/// `button` with its leading element made by the caller (a spinner), spaced as an icon is.
pub fn button_lead(t: &Theme, v: Variant, size: Size, lead: Option<El>, label: &str) -> El {
    button_ink(t, v, size, lead, label, None)
}

/// `button_lead` with the label in `ink` instead of the variant's (`class="text-error-text"` on a ghost button).
pub fn button_ink(t: &Theme, v: Variant, size: Size, lead: Option<El>, label: &str, ink: Option<Color>) -> El {
    let mut l = look(t, v);
    if let Some(ink) = ink {
        l.ink = ink;
    }
    let (h, px, gap) = size.dims(lead.is_some() && !label.is_empty());
    // `text-sm` is 0.875rem; the filter chip is `text-[11px]`.
    let font = if matches!(v, Variant::Filter { .. }) { sans(11.0, 500.0, 1.4) } else { sans(rem(0.875), 500.0, 1.4) };
    let mut e = row()
        .h(h)
        .px(px)
        .gap(gap)
        .center()
        .radius(RADIUS_BTN)
        .fill(l.fill)
        .border(1.0, l.border)
        .hover(l.hover_fill)
        .cursor(Cursor::Pointer)
        .label(label.to_string())
        .focus_ring(3.0, 0.0, t.accent.alpha(0.5));
    if size.square() {
        e = e.w(h);
    }
    if let Some(lead) = lead {
        e = e.child(lead);
    }
    if !label.is_empty() {
        let mut tx = text(label.to_string()).font(font).color(l.ink);
        if let Some(c) = l.hover_ink {
            tx = tx.group_color(c);
        }
        e = e.child(tx);
    }
    if l.hover_ink.is_some() { e.hover_group() } else { e }
}

/// A square icon button (`size="icon"`, `"toolbar"`, ...).
pub fn icon_button(t: &Theme, v: Variant, size: Size, glyph: &'static Icon, label: &str) -> El {
    button(t, v, size, Some(glyph), "").label(label.to_string()).help(label.to_string())
}

/// `disabled:pointer-events-none disabled:opacity-50`: the caller leaves the handler off.
pub fn disabled(e: El) -> El {
    e.opacity(0.5).disabled(true).cursor(Cursor::Default)
}

/// A bare element the size of a button's glyph, for places that build their own row.
pub fn glyph_box(size: f32) -> El {
    el().size(size, size)
}
