//! Settings → Updates & about (`UpdatesSection.vue`): one column of groups, `grid gap-6` (21). Fleet (with the app's own row), Harnesses
//! Machines and What's new are drawn.
//! The little pieces the groups share (the pill buttons of `SystemSection`, its status icon and its underlined links) are here.
use crate::State;
use crate::fleet::settings::updates_page::Tone;
use crate::fleet::settings_nav::Page;
use crate::icons::{CIRCLE_ALERT, CIRCLE_CHECK, DOWNLOAD, REFRESH_CW};
use crate::theme::{RADIUS_BTN, Theme, sans};
use crate::ui::button::disabled;
use crate::ui::confirm::spinner;
use crate::ui::rem;
use rune::{Cursor, El, Span, col, el, icon, rich, row, text};

pub fn content(s: &State, t: &Theme) -> El {
    col()
        .key("updates-section")
        .w_full()
        .gap(21.0)
        .child(super::super::settings::page_header(s, t, &Page::Updates))
        .child(super::updates_fleet::group(s, t))
        .child(super::updates_harnesses::group(s, t))
        .child(super::updates_machines::group(s, t))
        .child(super::updates_whats_new::group(s, t))
}

/// The ink of a status line (`text-text`, `text-accent-text`, `text-idle-text`, `text-error-text`).
pub fn ink(t: &Theme, tone: Tone) -> rune::Color {
    match tone {
        Tone::Plain | Tone::Ok => t.text,
        Tone::Info => t.accent_text,
        // Fleet's `text-idle-text` is missing from its stylesheet too: the ready line is plain.
        Tone::Warn => t.text,
        // Fleet's `text-error-text` class isn't in its stylesheet, so its failed line draws in the plain text colour; so does this.
        Tone::Error => t.text,
    }
}

/// The status icon, 15 px: a spinner while busy, else the tone's (`CheckCircle2`, `Download`, `AlertCircle`); none for plain.
pub fn status_icon(t: &Theme, tone: Tone, busy: bool) -> Option<El> {
    if busy {
        return Some(spinner(t.muted));
    }
    Some(match tone {
        Tone::Ok => icon(&CIRCLE_CHECK, 15.0).color(t.running_text),
        Tone::Warn => icon(&DOWNLOAD, 15.0).color(t.text),
        Tone::Error => icon(&CIRCLE_ALERT, 15.0).color(t.text),
        Tone::Info => icon(&DOWNLOAD, 15.0).color(t.accent_text),
        Tone::Plain => return None,
    })
}

/// `rounded-btn border border-border bg-card-bg px-3 py-1.5 text-xs font-medium`, hover `bg-main-bg`; `lead` is the 12 px icon
/// or spinner; `accent` is the filled variant (`border-accent bg-accent text-primary-foreground`).
pub fn pill(t: &Theme, key: &str, label: &str, lead: Option<El>, accent: bool) -> El {
    let ink = if accent { t.primary_fg } else { t.text };
    let mut b = row()
        .key(key.to_string())
        .role("button")
        .label(label.to_string())
        .center_y()
        .gap(5.25)
        .px(10.5)
        .py(5.25)
        .radius(RADIUS_BTN)
        .border(1.0, if accent { t.accent } else { t.border })
        .border_layout()
        .fill(if accent { t.accent } else { t.card_bg })
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 2.0, t.accent);
    if !accent {
        b = b.hover(t.main_bg);
    }
    if let Some(lead) = lead {
        b = b.child(lead);
    }
    b.child(text(label.to_string()).font(sans(rem(0.75), 500.0, 1.33)).color(ink))
}

/// "Check for updates" with its 12 px refresh icon, or a spinner while it waits. `press` is left off while `waiting`.
pub fn check_button(t: &Theme, key: &str, waiting: bool, press: impl Fn(&mut State) + 'static) -> El {
    let lead = if waiting { spinner(t.muted) } else { icon(&REFRESH_CW, 12.0).color(t.text) };
    let b = pill(t, key, "Check for updates", Some(lead), false);
    if waiting { disabled(b) } else { b.on_press(move |s: &mut State| press(s)) }
}

/// A link drawn muted and underlined (`text-xs text-muted underline underline-offset-2`).
pub fn link(t: &Theme, key: &str, words: &str, press: impl Fn(&mut State) + 'static) -> El {
    link_in(t, key, words, sans(rem(0.75), 400.0, 1.33), press)
}

/// `link` in another face (the version line's is mono).
pub fn link_in(t: &Theme, key: &str, words: &str, font: rune::tok::TextStyle, press: impl Fn(&mut State) + 'static) -> El {
    row()
        .key(key.to_string())
        .role("button")
        .label(words.to_string())
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 0.0, t.accent)
        .child(rich(words.to_string(), vec![Span::new(0, words.len()).underline(t.muted)]).font(font).color(t.muted))
        .on_press(move |s: &mut State| press(s))
}

/// The download bar: `h-1.5` (6), `--border` track, `--accent` fill; `percent` None is a full bar (the size isn't known).
pub fn bar(t: &Theme, key: &str, percent: Option<u32>) -> El {
    let done = percent.map_or(100.0, |p| p.min(100) as f32);
    let mut b = row().key(key.to_string()).w_full().h(6.0).radius(3.0).fill(t.border).clip_rounded(3.0);
    if done > 0.0 {
        b = b.child(el().h(6.0).w(0.0).grow(done).fill(t.accent));
    }
    if done < 100.0 {
        b = b.child(el().h(6.0).w(0.0).grow(100.0 - done));
    }
    b
}

pub fn note(t: &Theme, key: &str, words: &str) -> El {
    text(words.to_string()).key(key.to_string()).font(sans(rem(0.75), 400.0, 1.33)).color(t.muted).wrap().w_full()
}

/// An error under a row (`ErrorNote`, drawn in the text colour as Fleet's is): its own row with the card's padding.
pub fn error_row(t: &Theme, key: &str, words: &str) -> El {
    col().w_full().px(14.0).py(10.5).child(note(t, key, words).color(t.text))
}
