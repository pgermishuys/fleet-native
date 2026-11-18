//! The terminal's colours (`client/src/lib/terminal-theme.ts`): text, cursor and selection from Fleet's tokens, red, green,
//! yellow, blue and magenta from its status colours, the other ANSI colours from a light or dark palette picked by how bright
//! the panel is. Ported on CSS strings as Fleet does it, so its tests carry over by name; `Palette::new` turns the result
//! into Rune's colours once per theme.
use crate::theme::Theme;
use rune::Color;

const DARK: [&str; 16] = [
    "#3a3a44", "#f87171", "#4ade80", "#fbbf24", "#a2a7fa", "#c4a5fa", "#67e8f9", "#d6d6dd", "#6b6b76", "#fca5a5", "#86efac", "#fde68a",
    "#c7d2fe", "#ddd6fe", "#a5f3fc", "#f5f5f7",
];

const LIGHT: [&str; 16] = [
    "#1a1918", "#c4302b", "#1a7f37", "#9a6700", "#4557b5", "#8250df", "#0e7490", "#8a857f", "#6e6a65", "#dc2626", "#16a34a", "#b45309",
    "#2563eb", "#7c3aed", "#0891b2", "#34322f",
];

/// xterm's `ITheme`, the fields the terminal reads (colours as CSS strings).
#[derive(Debug, Clone, PartialEq)]
pub struct XtermTheme {
    pub ansi: [String; 16],
    pub foreground: String,
    pub cursor: String,
    pub cursor_accent: String,
    pub selection_background: String,
    pub selection_inactive_background: String,
    pub scrollbar_slider_background: String,
}

/// `terminalTheme(read)`: `read` gives a CSS variable's value, empty when a theme doesn't set it.
pub fn terminal_theme(read: &dyn Fn(&str) -> String) -> XtermTheme {
    let get = |name: &str, fallback: &str| {
        let v = read(name);
        if v.trim().is_empty() { fallback.to_string() } else { v.trim().to_string() }
    };
    let panel = get("--panel-bg", "#141418");
    let palette = if is_light(&panel) { &LIGHT } else { &DARK };
    let text = get("--text", palette[15]);
    let mut ansi = palette.map(str::to_string);
    for (slot, name) in [(1, "--error"), (2, "--running"), (3, "--idle"), (4, "--complete"), (5, "--queued")] {
        ansi[slot] = get(name, palette[slot]);
    }
    XtermTheme {
        ansi,
        foreground: text.clone(),
        cursor: text.clone(),
        cursor_accent: panel.clone(),
        // Opaque on purpose: xterm blends a translucent selection over its own (transparent) background.
        selection_background: mix(&panel, &get("--accent", "#6366f1"), 0.28)
            .unwrap_or_else(|| get("--accent-dim", "rgba(99, 102, 241, 0.25)")),
        selection_inactive_background: mix(&panel, &text, 0.14).unwrap_or_else(|| get("--accent-dim", "rgba(99, 102, 241, 0.15)")),
        scrollbar_slider_background: with_alpha(&text, 0.16),
    }
}

/// `amount` of `over` painted on `base`, as an opaque `#rrggbb`; None when either colour can't be read.
pub fn mix(base: &str, over: &str, amount: f32) -> Option<String> {
    let (a, b) = (parse_color(base)?, parse_color(over)?);
    let channel = |i: usize| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * amount).round() as u8;
    Some(format!("#{:02x}{:02x}{:02x}", channel(0), channel(1), channel(2)))
}

/// The colour at the given opacity, as `rgba()`; a colour it can't read comes back unchanged.
pub fn with_alpha(color: &str, alpha: f32) -> String {
    match parse_color(color) {
        Some([r, g, b]) => format!("rgba({r}, {g}, {b}, {alpha})"),
        None => color.to_string(),
    }
}

/// True for a colour brighter than mid-grey. Understands `#rgb`, `#rrggbb` and `rgb()`/`rgba()`.
pub fn is_light(color: &str) -> bool {
    let Some(rgb) = parse_color(color) else { return false };
    let [r, g, b] = rgb.map(|c| {
        let v = c as f32 / 255.0;
        if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    });
    0.2126 * r + 0.7152 * g + 0.0722 * b > 0.4
}

fn parse_color(color: &str) -> Option<[u8; 3]> {
    let v = color.trim();
    if let Some(h) = v.strip_prefix('#').filter(|h| h.chars().all(|c| c.is_ascii_hexdigit())) {
        let digits: String = match h.len() {
            3 => h.chars().flat_map(|c| [c, c]).collect(),
            6 => h.to_string(),
            _ => return None,
        };
        let at = |i: usize| u8::from_str_radix(&digits[i..i + 2], 16).ok();
        return Some([at(0)?, at(2)?, at(4)?]);
    }
    let inner = v.strip_prefix("rgba(").or_else(|| v.strip_prefix("rgb("))?;
    let mut n =
        inner.split(|c: char| c == ',' || c.is_whitespace() || c == '/' || c == ')').filter(|p| !p.is_empty()).map(|p| p.parse::<f32>());
    let mut next = || n.next()?.ok().map(|v| v.clamp(0.0, 255.0).round() as u8);
    Some([next()?, next()?, next()?])
}

fn rgb(r: u8, g: u8, b: u8) -> Color {
    let d = |v: u8| rune::color::srgb_decode(v as f32 / 255.0);
    Color { r: d(r), g: d(g), b: d(b), a: 1.0 }
}

fn to_color(css: &str) -> Color {
    let [r, g, b] = parse_color(css).unwrap_or([0, 0, 0]);
    rgb(r, g, b)
}

fn hex(c: Color) -> String {
    let e = |v: f32| (rune::color::srgb_encode(v) * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}", e(c.r), e(c.g), e(c.b))
}

/// What the screen is drawn with, for one Fleet theme.
#[derive(Clone)]
pub struct Palette {
    pub fg: Color,
    /// `--panel-bg`, the colour on the cursor's character.
    pub cursor_ink: Color,
    pub selection: Color,
    pub selection_inactive: Color,
    ansi: [Color; 16],
}

impl Palette {
    /// `terminalTheme()` read from Fleet's tokens (the tokens are opaque here, so each becomes `#rrggbb`).
    pub fn new(t: &Theme) -> Palette {
        let tokens = |name: &str| match name {
            "--panel-bg" => hex(t.panel_bg),
            "--text" => hex(t.text),
            "--accent" => hex(t.accent),
            "--error" => hex(t.error),
            "--running" => hex(t.running),
            "--idle" => hex(t.idle),
            "--complete" => hex(t.complete),
            "--queued" => hex(t.queued),
            _ => String::new(),
        };
        let x = terminal_theme(&tokens);
        Palette {
            fg: to_color(&x.foreground),
            cursor_ink: to_color(&x.cursor_accent),
            selection: to_color(&x.selection_background),
            selection_inactive: to_color(&x.selection_inactive_background),
            ansi: x.ansi.map(|c| to_color(&c)),
        }
    }

    pub fn ansi(&self, i: usize) -> Color {
        self.ansi[i]
    }

    /// The 256-colour palette: the 16 above, xterm's 6×6×6 cube, then the grey ramp.
    pub fn indexed(&self, i: u8) -> Color {
        match i {
            0..16 => self.ansi[i as usize],
            16..232 => {
                let v = |n: u8| if n == 0 { 0 } else { 55 + n * 40 };
                let n = i - 16;
                rgb(v(n / 36), v(n / 6 % 6), v(n % 6))
            }
            _ => {
                let g = 8 + (i - 232) * 10;
                rgb(g, g, g)
            }
        }
    }

    pub fn truecolor(&self, r: u8, g: u8, b: u8) -> Color {
        rgb(r, g, b)
    }
}
