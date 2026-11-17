//! The parts of Settings → Appearance (`AppearanceSection.vue`): a card (a button with a picture over a label and a check),
//! a theme's swatch, the code preview with its coloured tokens, and the joined buttons Font size and Thinking use.
use crate::State;
use crate::icons::{CHECK, MONITOR};
use crate::theme::{RADIUS_BTN, RADIUS_CARD, Theme, mono, sans};
use crate::ui::rem;
use rune::{Color, Cursor, El, col, el, icon, rich, row, text};
use rune::{Span, color};
use std::f32::consts::FRAC_PI_2;
use std::rc::Rc;

/// What a card is doing. A card is `p-3` with a 1px border; picked it takes the accent border and its dim fill.
#[derive(Clone, Copy)]
pub struct CardState {
    pub picked: bool,
    /// An unavailable card is disabled and half faded (`cursor-not-allowed ... opacity-50`).
    pub available: bool,
}

/// `flex flex-col gap-3 rounded-card border p-3`: the picture, then the label row.
pub fn card(t: &Theme, key: String, label: &str, state: CardState, body: Vec<El>, press: impl Fn(&mut State) + 'static) -> El {
    let mut c = col()
        .key(key)
        .role("button")
        .label(label.to_string())
        .checked(state.picked)
        .grow(1.0)
        .w(0.0)
        .gap(10.5)
        .padding(10.5)
        .radius(RADIUS_CARD)
        .border(1.0, if state.picked { t.accent } else { t.border })
        .border_layout()
        .focusable(true)
        .focus_ring(2.0, 2.0, t.accent)
        .children(body);
    if state.picked {
        c = c.fill(t.accent_dim);
    }
    if state.available { c.cursor(Cursor::Pointer).on_press(press) } else { c.opacity(0.5).disabled(true).cursor(Cursor::NotAllowed) }
}

/// `flex items-center justify-between gap-2`: the label (with a muted note after it) and the check when picked.
pub fn label_row(t: &Theme, label: &str, note: Option<&str>, picked: bool) -> El {
    let font = sans(rem(0.75), 500.0, 1.33);
    let words = match note {
        Some(n) => rich(format!("{label} \u{b7} {n}"), vec![Span::new(label.len(), label.len() + 3 + n.len()).color(t.muted)]),
        None => rich(label.to_string(), vec![]),
    };
    let mut r = row().w_full().items_center().justify_between().gap(7.0).child(words.font(font).color(t.text).wrap().shrink().min_w(0.0));
    if picked {
        r = r.child(icon(&CHECK, 14.0).color(t.accent_text));
    }
    r
}

/// The picture frame both swatches share: `h-10 rounded-btn border border-border/60`.
fn swatch_frame(t: &Theme) -> El {
    row().w_full().h(35.0).radius(RADIUS_BTN).border(1.0, t.border.alpha(t.border.a * 0.6)).border_layout().clip_rounded(RADIUS_BTN)
}

/// The System card's picture: dark running into light, with a monitor on it.
pub fn system_swatch(t: &Theme) -> El {
    let [dark, light] = Theme::system_swatch();
    swatch_frame(t)
        .center()
        .linear_gradient(FRAC_PI_2, [(0.0, dark), (0.5, dark), (1.0, light)])
        .child(icon(&MONITOR, 14.0).color(Color::WHITE.alpha(0.7)))
}

/// A theme card's picture: its window colour with three bars (card, accent, text) standing on the bottom.
pub fn theme_swatch(t: &Theme, dark: bool) -> El {
    let [window, card, accent, ink] = Theme::swatches(dark);
    // `h-10` less the border and the padding, which the bars take their heights from.
    let inner = 35.0 - 2.0 - 10.5;
    swatch_frame(t)
        .items_end()
        .gap(1.75)
        .padding(5.25)
        .fill(window)
        .child(el().grow(1.0).h(inner).radius(2.0).fill(card))
        .child(el().w(7.0).h(inner * 0.75).radius(2.0).fill(accent))
        .child(el().w(7.0).h(inner * 0.5).radius(2.0).fill(ink).opacity(0.6))
}

/// The eight colours of a code theme, in `syntaxTokens` order.
pub const KEYWORD: usize = 0;
pub const STRING: usize = 1;
pub const NUMBER: usize = 2;
pub const COMMENT: usize = 3;
pub const FUNCTION: usize = 4;
pub const ATTR: usize = 5;
pub const TYPE: usize = 6;
pub const LITERAL: usize = 7;

/// The preview's three lines: each piece is its words and the token it is coloured as, `None` for the plain text.
type Piece = (&'static str, Option<(usize, &'static str)>);
const PREVIEW: [&[Piece]; 3] = [
    &[
        ("const", Some((KEYWORD, "keyword"))),
        (" ", None),
        ("run", Some((FUNCTION, "function"))),
        (" = (", None),
        ("name", Some((ATTR, "attr"))),
        (") => ", None),
        ("true", Some((LITERAL, "literal"))),
        (";", None),
    ],
    &[
        ("\"nord\"", Some((STRING, "string"))),
        (": ", None),
        ("42", Some((NUMBER, "number"))),
        (" ", None),
        ("// ok", Some((COMMENT, "comment"))),
    ],
    &[("Fleet", Some((TYPE, "type"))), (".", None), ("start", Some((FUNCTION, "function"))), ("()", None)],
];

/// The code preview (`pre.code-theme-preview`): mono 10 over 15, the card's own ground, tokens in the scheme's colours.
/// Each token is keyed `<key>-<name>` so a test can read the colour it was drawn in.
pub fn code_preview(t: &Theme, key: &str, palette: &[Color; 8]) -> El {
    let font = mono(10.0, 400.0, 1.5);
    let mut pre = col()
        .w_full()
        .px(7.0)
        .py(5.25)
        .radius(RADIUS_BTN)
        .border(1.0, t.border.alpha(t.border.a * 0.6))
        .border_layout()
        .fill(t.card_bg)
        .clip_rounded(RADIUS_BTN);
    for line in PREVIEW {
        let mut r = row().w_full();
        for (at, (words, token)) in line.iter().enumerate() {
            let mut piece = text((*words).to_string()).font(font);
            piece = match token {
                Some((i, name)) => piece.key(format!("{key}-{name}-{at}")).color(palette[*i]),
                None => piece.color(t.text),
            };
            r = r.child(piece);
        }
        pre = pre.child(r);
    }
    pre
}

/// A palette's hex colours as colours.
pub fn colours(hex: &[impl AsRef<str>; 8]) -> [Color; 8] {
    std::array::from_fn(|i| color::hex(hex[i].as_ref()).unwrap_or(Color::TRANSPARENT))
}

pub type Pick = Rc<dyn Fn(&mut State, &str)>;

/// `inline-flex rounded-card border`: the choices side by side, joined by a rule, the picked one filled with the accent.
/// A choice is keyed `<key>-<value>`; the group `<key>-group`.
pub fn joined(t: &Theme, key: &str, label: &str, options: &[(&str, &str)], value: &str, pick: Pick) -> El {
    let mut group = row()
        .key(format!("{key}-group"))
        .role("group")
        .label(label.to_string())
        .self_start()
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .clip_rounded(RADIUS_CARD);
    for (at, (v, words)) in options.iter().enumerate() {
        let on = *v == value;
        let (v, p) = ((*v).to_string(), pick.clone());
        let mut b = row()
            .key(format!("{key}-{v}"))
            .role("button")
            .label((*words).to_string())
            .checked(on)
            .center()
            .px(14.0)
            .py(7.0)
            .cursor(Cursor::Pointer)
            .focusable(true)
            .focus_ring(2.0, -2.0, t.accent)
            .child(text((*words).to_string()).font(sans(rem(0.875), 500.0, 1.43)).color(if on { t.primary_fg } else { t.muted }));
        b = if on { b.fill(t.accent) } else { b.hover(t.card_bg) };
        if at > 0 {
            b = b.border_sides([0.0, 0.0, 0.0, 1.0], t.border);
        }
        group = group.child(b.on_press(move |s: &mut State| p(s, &v)));
    }
    group
}
