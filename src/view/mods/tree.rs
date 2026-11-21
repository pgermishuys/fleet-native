//! A tree a mod drew, drawn with Fleet's own look (`components/mods/ModTree.vue`, `ModNode.vue`, `ModBox.vue`,
//! `ModText.vue`; component 22, piece B). The tree is checked first (`fleet::mods::validate`); an invalid one draws as if
//! the mod weren't there (the site's own drawing alone) and a `Fleet` node draws that drawing in place. At the inline sites
//! (a tool row's line, the status bar) the root is one compact row and every row Box in it wraps.
//!
//! Pill and Icon are in `leaves`, Button, Input and Select in `controls`, Markdown and Code in `rich`; Page is piece J's and draws nothing yet.
use super::leaves::{self, flag, role_color, space_px, str_prop};
use super::{controls, rich};
use crate::State;
use crate::fleet::mods::types::Site;
use crate::fleet::mods::validate::{Check, validate_tree};
use crate::theme::{RADIUS_BTN, RADIUS_CARD, Theme, mono, sans};
use rune::{El, Span, col, el, row, text};
use serde_json::{Map, Value};

/// The type a site's text inherits: the size and line height of the place the tree sits in. A tool row, the status bar and
/// the pane each set their own; these are the defaults until their pieces say.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub size: f32,
    pub line: f32,
}

impl Look {
    pub fn of(site: Site) -> Look {
        match site {
            Site::Pane => Look { size: 14.0, line: 1.5 },
            Site::StatusChip => Look { size: 11.0, line: 1.3 },
            Site::ToolUse | Site::ToolResult | Site::ComposerBand => Look { size: 12.0, line: 1.4 },
        }
    }
}

/// `align-items` / `justify-content`'s words (`SIDES`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Start,
    Center,
    End,
    Stretch,
    Baseline,
    SpaceBetween,
}

impl Side {
    pub fn of(word: &str) -> Option<Side> {
        Some(match word {
            "start" => Side::Start,
            "center" => Side::Center,
            "end" => Side::End,
            "stretch" => Side::Stretch,
            "baseline" => Side::Baseline,
            "space-between" => Side::SpaceBetween,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Border {
    Round,
    Single,
    Dashed,
    Quote,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Background {
    Subtle,
    Tint,
}

/// What a Box's props say, before anything is drawn (`ModBox`'s `style`, `border` and `background`).
#[derive(Clone, Debug, PartialEq)]
pub struct BoxLook {
    /// A Box is a row unless it says `column`.
    pub column: bool,
    pub gap: f32,
    /// Top, right, bottom, left. `padding` is the base; `paddingX` and `paddingY` replace their sides only.
    pub padding: [f32; 4],
    pub align: Option<Side>,
    pub justify: Option<Side>,
    pub wrap: bool,
    pub grow: bool,
    /// A share of the parent, 0 to 1 (`"40%"` is 0.4). Never a basis: in a column parent that would be the height.
    pub width: Option<f32>,
    pub border: Option<Border>,
    pub edge: Option<String>,
    pub background: Option<Background>,
}

pub fn box_look(props: &Map<String, Value>) -> BoxLook {
    let space = |name: &str| props.get(name).and_then(space_px);
    let base = space("padding");
    let (x, y) = (space("paddingX").or(base).unwrap_or(0.0), space("paddingY").or(base).unwrap_or(0.0));
    BoxLook {
        column: str_prop(props, "flexDirection") == Some("column"),
        gap: space("gap").unwrap_or(0.0),
        padding: [y, x, y, x],
        align: str_prop(props, "alignItems").and_then(Side::of),
        justify: str_prop(props, "justifyContent").and_then(Side::of),
        wrap: str_prop(props, "flexWrap") == Some("wrap"),
        grow: props.get("flexGrow").and_then(Value::as_f64) == Some(1.0),
        width: str_prop(props, "width").and_then(share),
        border: match str_prop(props, "borderStyle") {
            Some("round") => Some(Border::Round),
            Some("single") => Some(Border::Single),
            Some("dashed") => Some(Border::Dashed),
            Some("quote") => Some(Border::Quote),
            _ => None,
        },
        edge: str_prop(props, "borderColor").filter(|r| role_color(&Theme::of(true), r).is_some()).map(str::to_string),
        background: match str_prop(props, "background") {
            Some("subtle") => Some(Background::Subtle),
            Some("tint") => Some(Background::Tint),
            _ => None,
        },
    }
}

/// `"<n>%"` as a fraction (`/^\d+(\.\d+)?%$/`).
fn share(width: &str) -> Option<f32> {
    let number = width.strip_suffix('%')?;
    let (whole, part) = number.split_once('.').unwrap_or((number, "0"));
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    (digits(whole) && digits(part)).then(|| number.parse::<f32>().ok()).flatten().map(|n| n / 100.0)
}

/// What a Text's props say (`ModText`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Ink {
    pub color: Option<InkColor>,
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub code: bool,
}

/// A role colour by its word, kept as the role so a theme can resolve it later.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InkColor(pub &'static str);

const ROLES: [&str; 6] = ["text", "muted", "accent", "good", "warn", "bad"];

fn role_word(word: &str) -> Option<InkColor> {
    ROLES.iter().find(|r| **r == word).map(|r| InkColor(r))
}

impl Ink {
    /// A Text inside this one: what it says replaces, what it doesn't say is kept. An explicit colour beats `dimColor`.
    pub fn nested(self, props: &Map<String, Value>) -> Ink {
        let own = str_prop(props, "color").and_then(role_word).or_else(|| flag(props, "dimColor").then_some(InkColor("muted")));
        Ink {
            color: own.or(self.color),
            bold: self.bold || flag(props, "bold"),
            italic: self.italic || flag(props, "italic"),
            strike: self.strike || flag(props, "strikethrough"),
            code: self.code || flag(props, "code"),
        }
    }
}

struct Ctx<'a> {
    s: &'a State,
    t: &'a Theme,
    look: Look,
    inline: bool,
    fleet: Option<El>,
}

/// `ModTree`: the tree drawn for `site`, or `fleet` (the site's own drawing, if it has one) when the tree breaks the contract.
/// A `null` tree draws nothing. Every element is keyed `<key>.<path>`, so a test and a press can name one.
pub fn tree(s: &State, t: &Theme, tree: &Value, site: Site, fleet: Option<El>) -> El {
    tree_keyed(s, t, "mod-tree", tree, site, Look::of(site), fleet)
}

pub fn tree_keyed(s: &State, t: &Theme, key: &str, tree: &Value, site: Site, look: Look, fleet: Option<El>) -> El {
    let inline = site.inline();
    // `inline-flex` at an inline site: as wide as what it holds. Elsewhere a block: a Box fills it, a lone Pill, Icon or Button is
    // as wide as it is (`inline-flex` in a block).
    let lone_leaf = matches!(tree.get("type").and_then(Value::as_str), Some("Pill" | "Icon" | "Button" | "Fleet"));
    let root = if inline {
        row().center_y().gap(6.0).self_start()
    } else if lone_leaf {
        col().items_start().w_full()
    } else {
        col().items_stretch().w_full()
    };
    let root = root.key(key.to_string()).min_w(0.0);
    match to_draw(tree, site) {
        Draw::Fleet(_) => match fleet {
            Some(own) => root.child(own),
            None => root,
        },
        Draw::Nothing => root,
        Draw::Tree(tree) => {
            let mut cx = Ctx { s, t, look, inline, fleet };
            root.child(node(&mut cx, &format!("{key}.0"), &tree, true))
        }
    }
}

/// What the root draws for a tree at a site (`ModTree`'s `check`).
#[derive(Debug, PartialEq)]
pub enum Draw<'a> {
    /// A `null` tree: the root is there and empty.
    Nothing,
    /// The tree broke the contract (the reason): only the site's own drawing.
    Fleet(String),
    /// The tree to draw: the validator's, with any text cut, not the one it was given.
    Tree(std::borrow::Cow<'a, Value>),
}

pub fn to_draw(tree: &Value, site: Site) -> Draw<'_> {
    match validate_tree(tree, site) {
        Check::Invalid { reason } => Draw::Fleet(reason),
        Check::Ok { tree, .. } if tree.is_null() => Draw::Nothing,
        Check::Ok { tree, .. } => Draw::Tree(tree),
    }
}

/// `ModNode`: a string is text, an element goes to what draws it. `lone` is whether it is its parent's only child.
fn node(cx: &mut Ctx, key: &str, value: &Value, lone: bool) -> El {
    let Some(map) = value.as_object() else {
        return plain(cx, key, value.as_str().unwrap_or_default());
    };
    let props = map.get("props").and_then(Value::as_object).cloned().unwrap_or_default();
    match map.get("type").and_then(Value::as_str) {
        Some("Box") => boxed(cx, key, &props, map.get("children")),
        Some("Text") => styled(cx, key, &props, map.get("children")),
        Some("Pill") => leaves::pill(cx.t, key, &props, !cx.inline || lone),
        Some("Icon") => leaves::icon_el(cx.t, key, &props, cx.t.text).unwrap_or_else(el),
        Some("Fleet") => cx.fleet.take().unwrap_or_else(el),
        Some("Button") => controls::button(cx.t, key, &props, &handles_of(map), !cx.inline || lone),
        Some("Input") => controls::input(cx.s, cx.t, key, &props, &handles_of(map), cx.look.size),
        Some("Select") => controls::select_control(cx.s, cx.t, key, &props, &handles_of(map)),
        Some("Markdown") => rich::markdown(cx.t, key, &props, cx.look.size),
        Some("Code") => rich::code(cx.s, cx.t, key, &props),
        // Page is piece J's.
        _ => el(),
    }
}

fn handles_of(node: &Map<String, Value>) -> Map<String, Value> {
    node.get("handles").and_then(Value::as_object).cloned().unwrap_or_default()
}

fn plain(cx: &Ctx, key: &str, words: &str) -> El {
    text(words.to_string())
        .key(key.to_string())
        .font(sans(cx.look.size, 400.0, cx.look.line))
        .color(cx.t.text)
        .wrap()
        .wrap_anywhere()
        .shrink()
        .min_w(0.0)
}

fn boxed(cx: &mut Ctx, key: &str, props: &Map<String, Value>, children: Option<&Value>) -> El {
    let look = box_look(props);
    let t = cx.t;
    let [top, right, bottom, left] = look.padding;
    let mut e = if look.column { col() } else { row() };
    e = e.key(key.to_string()).min_w(0.0).shrink().gap(look.gap).pt(top).pr(right).pb(bottom).pl(left);
    e = match look.align {
        Some(Side::Center) => e.items_center(),
        Some(Side::End) => e.items_end(),
        // Rune has no baseline alignment: a row of different sizes lines up at the top (docs/RUNE.md).
        Some(Side::Start | Side::Baseline) => e.items_start(),
        Some(Side::Stretch | Side::SpaceBetween) | None => e.items_stretch(),
    };
    e = match look.justify {
        Some(Side::Center) => e.justify_center(),
        Some(Side::End) => e.justify_end(),
        Some(Side::SpaceBetween) => e.justify_between(),
        _ => e.justify_start(),
    };
    // At an inline site every row Box wraps, so a row of items runs onto the next line instead of squeezing each one.
    if look.wrap || (cx.inline && !look.column) {
        e = e.flex_wrap();
    }
    if look.grow {
        e = e.grow(1.0);
    }
    if let Some(fraction) = look.width {
        e = e.w_frac(fraction);
    }
    let edge = look.edge.as_deref().and_then(|r| role_color(t, r));
    let line = edge.unwrap_or(t.border);
    e = match look.border {
        Some(Border::Round) => e.border(1.0, line).radius(RADIUS_CARD).border_layout(),
        Some(Border::Single) => e.border(1.0, line).border_layout(),
        Some(Border::Dashed) => e.border(1.0, line).border_dash(3.0, 3.0).radius(RADIUS_BTN).border_layout(),
        Some(Border::Quote) => e.border_sides([0.0, 0.0, 0.0, 3.0], line).border_layout(),
        None => e,
    };
    e = match look.background {
        Some(Background::Subtle) => e.fill(t.wash(4.0)),
        Some(Background::Tint) => e.fill(edge.unwrap_or(t.muted).alpha(0.12)),
        None => e,
    };
    let kids = children.and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default();
    for (i, child) in kids.iter().enumerate() {
        e = e.child(node(cx, &format!("{key}.{i}"), child, kids.len() == 1));
    }
    e
}

/// What a run of a Text says, in the order it is read: the words and the ink they are in.
struct Run {
    words: String,
    ink: Ink,
}

fn collect(children: Option<&Value>, ink: Ink, out: &mut Vec<Run>) {
    for child in children.and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default() {
        match child {
            Value::String(words) => out.push(Run { words: words.clone(), ink }),
            Value::Object(map) => {
                let props = map.get("props").and_then(Value::as_object).cloned().unwrap_or_default();
                collect(map.get("children"), ink.nested(&props), out);
            }
            _ => {}
        }
    }
}

/// `ModText`: one line of words in role colours. A nested Text is a span of the same run, so the words flow and wrap as one
/// paragraph. It wraps anywhere by default; `wrap: "truncate"` is one line with an ellipsis.
fn styled(cx: &Ctx, key: &str, props: &Map<String, Value>, children: Option<&Value>) -> El {
    let base = Ink::default().nested(props);
    let mut runs = Vec::new();
    collect(children, base, &mut runs);
    let (t, size, line) = (cx.t, cx.look.size, cx.look.line);
    let ink = |i: Ink| i.color.and_then(|c| role_color(t, c.0)).unwrap_or(t.text);
    let font = |i: Ink| {
        let f = if i.code {
            mono(size * 0.92, if i.bold { 600.0 } else { 400.0 }, line)
        } else {
            sans(size, if i.bold { 600.0 } else { 400.0 }, line)
        };
        if i.italic { f.italic() } else { f }
    };
    let words: String = runs.iter().map(|r| r.words.as_str()).collect();
    let mut e = text(words.clone()).key(key.to_string()).font(font(base)).color(ink(base)).min_w(0.0).shrink();
    let mut spans = Vec::new();
    let mut at = 0;
    for run in &runs {
        let end = at + run.words.len();
        if run.ink != base && !run.words.is_empty() {
            let mut span = Span::new(at, end).color(ink(run.ink));
            if run.ink.bold != base.bold {
                span = span.weight(if run.ink.bold { 600.0 } else { 400.0 });
            }
            if run.ink.code && !base.code {
                span = span.mono().size(size * 0.92);
            }
            if run.ink.italic && !base.italic {
                span = span.italic();
            }
            spans.push(span);
        }
        if run.ink.strike && !run.words.is_empty() {
            e = e.strike(at, end, None);
        }
        at = end;
    }
    if !spans.is_empty() {
        e = e.spans(spans);
    }
    if str_prop(props, "wrap") == Some("truncate") { e.truncate() } else { e.wrap().wrap_anywhere() }
}
