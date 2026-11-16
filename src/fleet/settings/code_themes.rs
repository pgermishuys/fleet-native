//! The code colour schemes (`lib/code-themes.ts`): six schemes of eight colours, which one "Match theme" picks, which can be
//! used on a light window, and the lift that makes every colour read on the surfaces code sits on. Plain Rust; the colours
//! stay hex strings, as in Fleet, so a test reads them the way Fleet's does.

/// keyword, string, number, comment, function, attr, type, literal: `syntaxTokens`, in Fleet's order.
pub const TOKENS: usize = 8;
pub type Palette = [&'static str; TOKENS];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CodeThemeId {
    Darcula,
    Monokai,
    Github,
    OneDark,
    Solarized,
    Nord,
}

pub struct CodeTheme {
    pub id: CodeThemeId,
    pub key: &'static str,
    pub label: &'static str,
    /// The name when the light palette is in use, where it has its own (One Dark's is One Light).
    pub light_label: Option<&'static str>,
    pub dark: Palette,
    /// Only schemes with a real light variant have one; the rest are for dark windows only.
    pub light: Option<Palette>,
}

/// The order of the cards on the page.
pub const CODE_THEMES: [CodeTheme; 6] = [
    CodeTheme {
        id: CodeThemeId::Darcula,
        key: "darcula",
        label: "Darcula",
        light_label: None,
        dark: ["#cc7832", "#6a8759", "#6897bb", "#808080", "#ffc66d", "#9876aa", "#a9b7c6", "#cc7832"],
        light: None,
    },
    CodeTheme {
        id: CodeThemeId::Monokai,
        key: "monokai",
        label: "Monokai",
        light_label: None,
        dark: ["#f92672", "#e6db74", "#ae81ff", "#75715e", "#a6e22e", "#f92672", "#66d9ef", "#ae81ff"],
        light: None,
    },
    CodeTheme {
        id: CodeThemeId::Github,
        key: "github",
        label: "GitHub",
        light_label: None,
        dark: ["#ff7b72", "#a5d6ff", "#79c0ff", "#8b949e", "#d2a8ff", "#7ee787", "#ffa657", "#79c0ff"],
        light: Some(["#cf222e", "#0a3069", "#0550ae", "#6e7781", "#8250df", "#116329", "#953800", "#0550ae"]),
    },
    CodeTheme {
        id: CodeThemeId::OneDark,
        key: "one-dark",
        label: "One Dark",
        light_label: Some("One Light"),
        dark: ["#c678dd", "#98c379", "#d19a66", "#8b949e", "#61afef", "#e5c07b", "#e06c75", "#56b6c2"],
        light: Some(["#a626a4", "#50a14f", "#986801", "#a0a1a7", "#4078f2", "#c18401", "#e45649", "#0184bc"]),
    },
    CodeTheme {
        id: CodeThemeId::Solarized,
        key: "solarized",
        label: "Solarized",
        light_label: None,
        dark: ["#859900", "#2aa198", "#d33682", "#586e75", "#268bd2", "#93a1a1", "#b58900", "#cb4b16"],
        light: Some(["#859900", "#2aa198", "#d33682", "#93a1a1", "#268bd2", "#586e75", "#b58900", "#cb4b16"]),
    },
    // Nord's comment is lighter than Nord's own #616e88 (2.3:1 on a Nord code block); this one reads at about 4:1.
    CodeTheme {
        id: CodeThemeId::Nord,
        key: "nord",
        label: "Nord",
        light_label: None,
        dark: ["#81a1c1", "#a3be8c", "#b48ead", "#909aae", "#88c0d0", "#8fbcbb", "#8fbcbb", "#d08770"],
        light: None,
    },
];

/// What is chosen: a scheme, or "match", which follows the app theme.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CodeThemeSelection {
    #[default]
    Match,
    Scheme(CodeThemeId),
}

impl CodeThemeSelection {
    pub fn key(self) -> &'static str {
        match self {
            CodeThemeSelection::Match => "match",
            CodeThemeSelection::Scheme(id) => theme(id).key,
        }
    }

    /// `isValidCodeTheme`: a stored value that isn't a scheme or "match" is not a choice.
    pub fn parse(value: &str) -> Option<CodeThemeSelection> {
        if value == "match" {
            return Some(CodeThemeSelection::Match);
        }
        CODE_THEMES.iter().find(|t| t.key == value).map(|t| CodeThemeSelection::Scheme(t.id))
    }
}

pub fn theme(id: CodeThemeId) -> &'static CodeTheme {
    CODE_THEMES.iter().find(|t| t.id == id).unwrap_or(&CODE_THEMES[0])
}

/// The scheme an app theme brings with it under "match"; the others get One Dark / One Light. The native app has the
/// default dark and light themes only (`docs/PORT.md`), so only those ids are asked, but the table is Fleet's.
fn theme_scheme(theme_id: &str) -> Option<CodeThemeId> {
    match theme_id {
        "nord" => Some(CodeThemeId::Nord),
        "monokai" => Some(CodeThemeId::Monokai),
        "github-dark" => Some(CodeThemeId::Github),
        "solarized-dark" | "solarized-light" => Some(CodeThemeId::Solarized),
        _ => None,
    }
}

/// Whether a scheme can be used on this window: Darcula, Monokai and Nord have no light colours, and their dark ones
/// are unreadable on a light background.
pub fn is_available(id: CodeThemeId, dark: bool) -> bool {
    dark || theme(id).light.is_some()
}

/// The scheme that is drawn: a dark-only scheme picked on a dark window falls back to "match" on a light one, and comes
/// back on a dark one.
pub fn resolve_id(selection: CodeThemeSelection, theme_id: &str, dark: bool) -> CodeThemeId {
    match selection {
        CodeThemeSelection::Scheme(id) if is_available(id, dark) => id,
        _ => theme_scheme(theme_id).unwrap_or(CodeThemeId::OneDark),
    }
}

/// The scheme's name as shown: One Dark reads One Light on a light window.
pub fn label(t: &CodeTheme, dark: bool) -> &'static str {
    if !dark && t.light.is_some() { t.light_label.unwrap_or(t.label) } else { t.label }
}

pub fn resolve_palette(selection: CodeThemeSelection, theme_id: &str, dark: bool) -> Palette {
    let t = theme(resolve_id(selection, theme_id, dark));
    if dark { t.dark } else { t.light.unwrap_or(t.dark) }
}

// ── readable on the app theme ──────────────────────────────────────────────────

/// The surfaces code is drawn on: the ink, and the three surfaces under it, as `#rrggbb`.
pub struct Grounds {
    pub text: String,
    pub surfaces: Vec<String>,
}

fn channels(hex: &str) -> [f64; 3] {
    let h = hex.trim_start_matches('#');
    [0, 2, 4].map(|i| u8::from_str_radix(h.get(i..i + 2).unwrap_or("00"), 16).unwrap_or(0) as f64)
}

fn to_hex(c: [f64; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0].round() as u8, c[1].round() as u8, c[2].round() as u8)
}

fn is_hex(value: &str) -> bool {
    let v = value.trim();
    v.len() == 7 && v.starts_with('#') && v[1..].bytes().all(|b| b.is_ascii_hexdigit())
}

/// A CSS colour as `#rrggbb`: six digits, three digits, or `rgb()` / `rgba()` with full alpha; else none.
pub fn normalize_hex(value: &str) -> Option<String> {
    let v = value.trim().to_lowercase();
    let digits = v.strip_prefix('#');
    if let Some(d) = digits.filter(|d| d.bytes().all(|b| b.is_ascii_hexdigit())) {
        return match d.len() {
            6 => Some(v),
            3 => Some(format!("#{}", d.chars().flat_map(|c| [c, c]).collect::<String>())),
            _ => None,
        };
    }
    let inner = v.strip_prefix("rgba(").or_else(|| v.strip_prefix("rgb("))?.strip_suffix(')')?;
    let parts: Vec<&str> = inner.split(|c: char| c == ',' || c == '/' || c.is_whitespace()).filter(|p| !p.is_empty()).collect();
    let nums: Vec<u8> = parts.iter().take(3).map(|p| p.parse::<u8>().ok()).collect::<Option<_>>()?;
    let alpha_ok = match parts.get(3) {
        None => parts.len() == 3,
        Some(a) => parts.len() == 4 && a.parse::<f64>().is_ok_and(|a| a == 1.0),
    };
    (nums.len() == 3 && alpha_ok).then(|| to_hex([nums[0] as f64, nums[1] as f64, nums[2] as f64]))
}

fn luminance(hex: &str) -> f64 {
    let f = |c: f64| if c <= 0.03928 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) };
    let [r, g, b] = channels(hex).map(|c| f(c / 255.0));
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

pub fn contrast_ratio(a: &str, b: &str) -> f64 {
    let (x, y) = (luminance(a), luminance(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

/// `a` at `percent` over `b`, as `color-mix(in srgb, a p%, b)` draws it.
pub fn mix_hex(a: &str, percent: f64, b: &str) -> String {
    let (x, y) = (channels(a), channels(b));
    to_hex([0, 1, 2].map(|i| x[i] * percent / 100.0 + y[i] * (1.0 - percent / 100.0)))
}

/// Every background code can sit on: the bare surfaces (the Files editor) and the code block's 4% ink tint over each.
pub fn code_backgrounds(g: &Grounds) -> Vec<String> {
    g.surfaces.iter().flat_map(|s| [s.clone(), mix_hex(&g.text, 4.0, s)]).collect()
}

/// The palette with every colour that reads below 4.5:1 on one of those backgrounds moved toward black (on a light
/// window) or white (a dark one), keeping its hue, just until it reads. Colours that already read stay as they are.
pub fn readable_palette(palette: Palette, g: &Grounds) -> [String; TOKENS] {
    let mut out = palette.map(String::from);
    if !std::iter::once(&g.text).chain(&g.surfaces).all(|c| is_hex(c)) {
        return out;
    }
    let backgrounds = code_backgrounds(g);
    let toward = if luminance(&g.text) < 0.5 { "#000000" } else { "#ffffff" };
    let reads = |c: &str| backgrounds.iter().all(|bg| contrast_ratio(c, bg) >= 4.5);
    for colour in &mut out {
        if !is_hex(colour) || reads(colour) {
            continue;
        }
        let base = colour.clone();
        let mut k = 1.0;
        while k < 100.0 && !reads(&mix_hex(toward, k, &base)) {
            k += 1.0;
        }
        *colour = mix_hex(toward, k, &base);
    }
    out
}
