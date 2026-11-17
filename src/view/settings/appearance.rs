//! Settings → Appearance (`AppearanceSection.vue`): Look (theme, code theme, font, font size) and Conversation (Thinking).
//! Nothing here waits on a server: every choice is this app's own and is kept at once (`fleet::settings::appearance`).
use super::appearance_cards::{self as cards, CardState, Pick, card, code_preview, colours, joined, label_row};
use super::kit::{self, Row};
use crate::State;
use crate::conv::gist::Thinking;
use crate::fleet::settings::appearance::{FontSize, ThemeChoice, parse_thinking, thinking_key};
use crate::fleet::settings::code_themes::{
    CODE_THEMES, CodeThemeSelection as Sel, is_available, label, readable_palette, resolve_id, resolve_palette, theme,
};
use crate::theme::{Theme, sans};
use crate::ui::rem;
use rune::{Color, El, col, row, text};
use std::rc::Rc;
use std::sync::OnceLock;

/// Fleet's breakpoints, in the window's pixels: `sm:grid-cols-2 md:grid-cols-3 lg:grid-cols-4`.
fn columns(s: &State) -> usize {
    // The window's width in points is what the zoomed view sees; Fleet's breakpoints are of the unzoomed window.
    let w = s.ui.sel.window.0 * s.sidebar.appearance().font_size.px() / FontSize::M.px();
    match w {
        w if w <= 0.0 || w >= 1024.0 => 4,
        w if w >= 768.0 => 3,
        w if w >= 640.0 => 2,
        _ => 1,
    }
}

/// `mt-3` then `grid gap-3`: the cards in rows of `columns`, an unfilled last row padded so the cards keep their width.
fn grid(s: &State, key: &str, cards: Vec<El>) -> El {
    let n = columns(s);
    let mut grid = col().key(key.to_string()).w_full().mt(10.5).gap(10.5);
    let mut cards = cards.into_iter();
    loop {
        let mut line = row().w_full().gap(10.5).items_stretch();
        let mut filled = 0;
        for c in cards.by_ref().take(n) {
            line = line.child(c);
            filled += 1;
        }
        if filled == 0 {
            return grid;
        }
        for _ in filled..n {
            line = line.child(rune::el().grow(1.0).w(0.0));
        }
        grid = grid.child(line);
    }
}

fn theme_label(dark: bool) -> &'static str {
    if dark { "Default Dark" } else { "Light" }
}

fn theme_cards(s: &State, t: &Theme) -> El {
    let chosen = s.sidebar.appearance().theme;
    let one = |choice: ThemeChoice, key: &str, name: &str, swatch: El| {
        let picked = chosen == choice;
        card(
            t,
            format!("theme-card-{key}"),
            name,
            CardState { picked, available: true },
            vec![swatch, label_row(t, name, None, picked)],
            move |s: &mut State| s.set_theme(choice),
        )
    };
    let list = vec![
        one(ThemeChoice::System, "system", "System", cards::system_swatch(t)),
        one(ThemeChoice::Dark, "dark", theme_label(true), cards::theme_swatch(t, true)),
        one(ThemeChoice::Light, "light", theme_label(false), cards::theme_swatch(t, false)),
    ];
    grid(s, "theme-cards", list)
}

/// Each card's colours as lifted for the window's scheme, worked out once per scheme: the lift steps through shades.
type Shown = Vec<(Sel, [Color; 8])>;

fn shown(dark: bool) -> &'static [(Sel, [Color; 8])] {
    static CACHE: [OnceLock<Shown>; 2] = [OnceLock::new(), OnceLock::new()];
    CACHE[usize::from(dark)].get_or_init(|| {
        let grounds = Theme::of(dark).code_grounds();
        let id = if dark { "dark" } else { "light" };
        std::iter::once(Sel::Match)
            .chain(CODE_THEMES.iter().map(|t| Sel::Scheme(t.id)))
            .map(|sel| (sel, colours(&readable_palette(resolve_palette(sel, id, dark), &grounds))))
            .collect()
    })
}

/// The card in use: a dark-only scheme saved on a dark window shows as Match theme on a light one.
pub fn active_code_theme(s: &State, dark: bool) -> Sel {
    match s.sidebar.appearance().code_theme {
        Sel::Scheme(id) if !is_available(id, dark) => Sel::Match,
        other => other,
    }
}

fn code_theme_cards(s: &State, t: &Theme) -> El {
    let dark = t.dark;
    let active = active_code_theme(s, dark);
    let theme_id = if dark { "dark" } else { "light" };
    let one = |sel: Sel, name: &str, note: Option<&str>, available: bool, palette: [Color; 8]| {
        let picked = active == sel;
        card(
            t,
            format!("code-theme-{}", sel.key()),
            name,
            CardState { picked, available },
            vec![code_preview(t, &format!("code-theme-{}", sel.key()), &palette), label_row(t, name, note, picked)],
            move |s: &mut State| s.set_code_theme(sel),
        )
    };
    let matched = theme(resolve_id(Sel::Match, theme_id, dark));
    let mut list = Vec::new();
    for (sel, palette) in shown(dark) {
        match sel {
            Sel::Match => list.push(one(*sel, "Match theme", Some(label(matched, dark)), true, *palette)),
            Sel::Scheme(id) => {
                let c = theme(*id);
                let available = is_available(*id, dark);
                // An unavailable card still previews the scheme's own colours, so you can see why it is for dark windows.
                let colours_shown = if available { *palette } else { colours(&c.dark) };
                list.push(one(*sel, label(c, dark), (!available).then_some("Dark themes only"), available, colours_shown));
            }
        }
    }
    grid(s, "code-theme-cards", list)
}

const FONT_SIZES: [(&str, &str); 4] = [("s", "S"), ("m", "M"), ("l", "L"), ("xl", "XL")];

const THINKING: [(Thinking, &str, &str); 3] = [
    (Thinking::Folded, "Folded", "One line per block, with its gist. Click a line to open it."),
    (Thinking::Open, "Open", "Every block in full."),
    (Thinking::Hidden, "Hidden", "Not shown. The harness and the history still keep it."),
];

/// The one font card: Inter, always the one in use.
fn font_card(t: &Theme) -> El {
    let picked = true;
    row()
        .key("font-card-inter")
        .role("button")
        .label("Inter")
        .checked(picked)
        .items_center()
        .justify_between()
        .gap(10.5)
        .padding(10.5)
        .radius(crate::theme::RADIUS_CARD)
        .border(1.0, t.accent)
        .border_layout()
        .fill(t.accent_dim)
        .child(text("Inter").font(sans(rem(0.875), 500.0, 1.43)).color(t.text))
        .child(rune::icon(&crate::icons::CHECK, 14.0).color(t.accent_text))
}

fn look(s: &State, t: &Theme) -> El {
    let a = s.sidebar.appearance();
    let rows = vec![
        Row::new("theme", "Theme")
            .summary(&format!("Active: {}. System follows your computer.", theme_label(t.dark)))
            .below(theme_cards(s, t))
            .build(s, t),
        Row::new("code-theme", "Code theme")
            .summary(
                "Syntax colours for files and code blocks. Match theme uses your theme's own scheme where it has one, otherwise One \
                 Dark or One Light. Darcula, Monokai and Nord are for dark themes.",
            )
            .below(code_theme_cards(s, t))
            .build(s, t),
        Row::new("font", "Font")
            .summary("The interface font.")
            // Two columns from 640px, so the one card is half the row there.
            .control(font_card(t))
            .build(s, t),
        Row::new("font-size", "Font size")
            .summary("The base size of the interface.")
            .control(joined(
                t,
                "font-size",
                "Font size",
                &FONT_SIZES,
                a.font_size.key(),
                Rc::new(|s: &mut State, v: &str| s.set_font_size(FontSize::parse(v))),
            ))
            .build(s, t),
    ];
    kit::group(t, "look", "Look", None, rows)
}

fn conversation(s: &State, t: &Theme) -> El {
    let chosen = s.sidebar.appearance().thinking;
    let help = THINKING.iter().find(|(id, ..)| *id == chosen).map_or(THINKING[0].2, |(.., h)| h);
    let options: Vec<(&str, &str)> = THINKING.iter().map(|(id, name, _)| (thinking_key(*id), *name)).collect();
    let pick: Pick = Rc::new(|s: &mut State, v: &str| s.set_thinking(parse_thinking(v)));
    let rows = vec![
        Row::new("thinking", "Thinking")
            .summary(help)
            .control(joined(t, "thinking", "Thinking", &options, thinking_key(chosen), pick))
            .build(s, t),
    ];
    kit::group(t, "conversation", "Conversation", None, rows)
}

pub fn content(s: &State, t: &Theme) -> El {
    col()
        .key("appearance-section")
        .w_full()
        .gap(21.0)
        .child(super::super::settings::page_header(s, t, &crate::fleet::settings_nav::Page::Appearance))
        .child(look(s, t))
        .child(conversation(s, t))
}
