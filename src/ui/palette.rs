//! The command palette: `CommandPalette.vue` over `ui/command/` (a `CommandDialog`). Ctrl K
//! (⌘K on macOS) opens it from anywhere; Up/Down move the highlight and wrap, Enter runs,
//! Backspace on an empty search leaves a level, Escape closes.
use super::commands::{Category, Command, Run, all, score};
use super::overlay::centred;
use super::rem;
use crate::theme::{Theme, mono, sans};
use crate::ui::text_menu::text_input;
use crate::{State, Tx};
use rune::{Color, Cursor, El, Key, KeyEvent, ScrollSpec, TextEdit, col, el, icon, row, text};

pub struct Palette {
    pub query: TextEdit,
    /// The highlighted row, an index into [`rows`].
    pub hl: usize,
    /// The parent command's id while a level is open.
    pub level: Option<String>,
}

impl Default for Palette {
    fn default() -> Self {
        Palette { query: TextEdit::new(""), hl: 0, level: None }
    }
}

pub enum Row {
    Back,
    Cmd(Command),
}

/// The commands at this level, filtered and grouped in Fleet's order.
pub fn rows(s: &State) -> Vec<(Option<Category>, Row)> {
    let mut commands = all(s);
    if let Some(parent) = &s.ui.palette.level {
        commands = match commands.into_iter().find(|c| &c.id == parent).map(|c| c.run) {
            Some(Run::Level(items)) => items,
            _ => Vec::new(),
        };
    }
    let q = s.ui.palette.query.text.clone();
    let mut out = Vec::new();
    if s.ui.palette.level.is_some() {
        out.push((None, Row::Back));
    }
    for cat in Category::ORDER {
        let mut group: Vec<(u32, Command)> =
            commands.iter().filter(|c| c.category == cat).filter_map(|c| score(c, &q).map(|n| (n, c.clone()))).collect();
        // Fleet lists an unfiltered group alphabetically; a search ranks by score.
        group.sort_by(|(a, x), (b, y)| b.cmp(a).then_with(|| x.label.to_lowercase().cmp(&y.label.to_lowercase())));
        out.extend(group.into_iter().map(|(_, c)| (Some(cat), Row::Cmd(c))));
    }
    out
}

pub fn open(s: &mut State, tx: &mut Tx) {
    s.ui.palette = Palette::default();
    s.ui.open = Some(super::Open::Palette);
    tx.focus("palette-input", false);
}

pub fn close(s: &mut State) {
    s.ui.open = None;
    s.ui.palette = Palette::default();
}

pub fn toggle(s: &mut State, tx: &mut Tx) {
    if s.ui.open == Some(super::Open::Palette) {
        close(s);
    } else {
        open(s, tx);
    }
}

/// Runs the highlighted row: a command, a level, or Back.
pub fn choose(s: &mut State, index: usize, tx: &mut Tx) {
    let rows = rows(s);
    match rows.into_iter().nth(index).map(|(_, r)| r) {
        Some(Row::Back) => back(s),
        Some(Row::Cmd(c)) if c.disabled => {}
        Some(Row::Cmd(c)) => match c.run {
            Run::Level(_) => {
                s.ui.palette.level = Some(c.id);
                s.ui.palette.query.clear();
                s.ui.palette.hl = 0;
            }
            Run::Now(f) => {
                close(s);
                f(s, tx);
            }
            Run::Waiting(n) => {
                close(s);
                s.toast(format!("{} isn't in the native app yet (component {n} of the port).", c.label), None, 4.0, tx);
            }
        },
        None => {}
    }
}

fn back(s: &mut State) {
    s.ui.palette.level = None;
    s.ui.palette.query.clear();
    s.ui.palette.hl = 0;
}

fn on_key(s: &mut State, ev: &KeyEvent, tx: &mut Tx) -> bool {
    let n = rows(s).len();
    match ev.key {
        Key::Down | Key::Up if n > 0 => {
            let hl = &mut s.ui.palette.hl;
            *hl = if ev.key == Key::Down { (*hl + 1) % n } else { (*hl + n - 1) % n };
            tx.scroll_into_view(format!("palette-row{}", s.ui.palette.hl));
            true
        }
        Key::Enter => {
            let i = s.ui.palette.hl;
            choose(s, i, tx);
            true
        }
        Key::Backspace if s.ui.palette.query.text.is_empty() && s.ui.palette.level.is_some() => {
            back(s);
            true
        }
        _ if ev.is_command('k') => {
            close(s);
            true
        }
        _ => false,
    }
}

fn kbd(t: &Theme, label: &str) -> El {
    text(label.to_string()).font(mono(10.5, 400.0, 1.2)).color(t.muted)
}

/// The dialog and its overlay.
pub fn view(s: &State, t: &Theme) -> Vec<El> {
    let p = &s.ui.palette;
    let level_label = p.level.as_ref().and_then(|id| all(s).into_iter().find(|c| &c.id == id)).map(|c| c.label);
    let placeholder = level_label.map_or("Type a command or search…".to_string(), |l| format!("{l}…"));
    let input = row()
        .h(rem(2.25))
        .px(rem(0.75))
        .gap(rem(0.5))
        .center_y()
        .border_sides([0.0, 0.0, 1.0, 0.0], t.border)
        .child(icon(&crate::icons::SEARCH, rem(1.0)).color(t.text).opacity(0.5))
        .child(
            text_input(&p.query, sans(rem(0.875), 400.0, 1.3))
                .key("palette-input")
                .label("Command palette search")
                .single_line()
                .placeholder(placeholder, t.muted)
                .color(t.text)
                .caret(t.text, t.accent.alpha(0.35))
                .grow(1.0)
                .h(rem(2.25))
                .py((rem(2.25) - rem(0.875) * 1.3) / 2.0 - 1.0)
                .on_edit(|s: &mut State| &mut s.ui.palette.query)
                .on_input_change(|s: &mut State, _: &mut Tx| s.ui.palette.hl = 0)
                .on_input_key(on_key),
        );

    let items = rows(s);
    let mut list = col().w_full().padding(rem(0.25));
    if items.is_empty() {
        list = list
            .child(el().w_full().py(rem(1.5)).center().child(text("No commands found.").font(sans(rem(0.875), 400.0, 1.4)).color(t.text)));
    }
    let mut last: Option<Option<Category>> = None;
    for (i, (cat, r)) in items.into_iter().enumerate() {
        if let (Some(c), true) = (cat, last != Some(cat)) {
            list = list
                .child(el().w_full().px(rem(0.5)).py(rem(0.375)).child(text(c.name()).font(sans(rem(0.75), 500.0, 1.3)).color(t.muted)));
        }
        last = Some(cat);
        list = list.child(item(s, t, i, r));
    }
    let list = list.w_full().scroll(ScrollSpec { stick_bottom: false }).key("palette-list").max_h(300.0);

    let hint = |keys: &str, what: &str| {
        row().gap(rem(0.25)).center_y().child(kbd(t, keys)).child(text(what.to_string()).font(sans(rem(0.75), 400.0, 1.3)).color(t.muted))
    };
    let mut hints = row().gap(rem(0.75)).center_y().child(hint("↑↓", "Navigate")).child(hint("Enter", "Select"));
    if p.level.is_some() {
        hints = hints.child(hint("Backspace", "Back"));
    }
    hints = hints.child(hint("Esc", "Close"));
    let toggle = if cfg!(target_os = "macos") { "⌘K" } else { "Ctrl+K" };
    let footer = row()
        .w_full()
        .px(rem(0.75))
        .py(rem(0.5))
        .center_y()
        .justify_between()
        .border_sides([1.0, 0.0, 0.0, 0.0], t.border)
        .child(hints)
        .child(
            row().gap(rem(0.25)).center_y().child(kbd(t, toggle)).child(text("toggle").font(sans(rem(0.75), 400.0, 1.3)).color(t.muted)),
        );

    let mut panel = col()
        .key("palette-dialog")
        .role("dialog")
        .label("Command palette")
        .w(rem(32.0))
        .fill(t.card_bg)
        .border(1.0, t.border)
        .clip_rounded(0.0)
        .child(input)
        .child(list)
        .child(footer)
        .modal(true, |s: &mut State| close(s));
    for sh in t.menu_shadows() {
        panel = panel.box_shadow(sh);
    }
    vec![
        el().key("palette-overlay").role("none").absolute(0.0, 0.0).w_full().h_full().fill(Color::BLACK.alpha(0.8)).a11y_hidden(),
        centred("palette-frame", panel),
    ]
}

fn item(s: &State, t: &Theme, i: usize, r: Row) -> El {
    let hl = s.ui.palette.hl == i;
    let (glyph, label, desc, tail, disabled) = match &r {
        Row::Back => (&crate::icons::CHEVRON_LEFT, "Back".to_string(), String::new(), Tail::None, false),
        Row::Cmd(c) => {
            let tail = match (&c.run, &c.shortcut) {
                (Run::Level(_), _) => Tail::Level,
                (Run::Waiting(n), _) => Tail::Waiting(*n),
                (_, Some(k)) => Tail::Keys(k.clone()),
                _ => Tail::None,
            };
            (c.icon, c.label.clone(), c.description.clone(), tail, c.disabled)
        }
    };
    let (ink, soft) = if hl { (t.primary_fg, t.primary_fg) } else { (t.text, t.muted) };
    let mut e = row()
        .key(format!("palette-row{i}"))
        .role("option")
        .label(label.clone())
        .w_full()
        .px(rem(0.5))
        .py(rem(0.375))
        .gap(rem(0.5))
        .center_y()
        .radius(3.5)
        .cursor(Cursor::Default)
        .fill(if hl { t.accent } else { Color::TRANSPARENT })
        .child(icon(glyph, rem(1.0)).color(soft))
        .child(text(label).font(sans(rem(0.875), 400.0, 1.43)).color(ink).grow(1.0));
    if !desc.is_empty() {
        e = e.child(text(desc).font(sans(rem(0.75), 400.0, 1.3)).color(soft).truncate().shrink().mr(rem(0.5)));
    }
    e = match tail {
        Tail::Level => e.child(text("›").font(sans(rem(0.75), 400.0, 1.3)).color(soft)),
        Tail::Keys(k) => e.child(
            row()
                .h(rem(1.25))
                .px(rem(0.375))
                .center()
                .radius(4.0)
                .border(1.0, t.border)
                .child(text(k).font(mono(10.0, 500.0, 1.0)).color(soft)),
        ),
        Tail::Waiting(n) => e.child(text(format!("waiting on {n}")).font(mono(10.0, 500.0, 1.0)).color(soft)),
        Tail::None => e,
    };
    if disabled {
        e = e.opacity(0.5);
    } else {
        e = e.hover(t.wash(7.0)).on_press_tx(move |s: &mut State, tx: &mut Tx| {
            s.ui.palette.hl = i;
            choose(s, i, tx);
        });
    }
    e
}

enum Tail {
    None,
    Level,
    Keys(String),
    Waiting(u8),
}
