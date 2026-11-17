//! The theme as the pages agents show see it (component 14, piece B; rows 14.28 and 14.29). Ports `lib/page-theme-core.ts`
//! and `readPageTheme`: the `--fleet-*` variables are read from `Theme` rather than from CSS. Plain Rust.
use crate::theme::{RADIUS_CARD, Theme};
use rune::Color;
use serde_json::{Map, Value, json};

/// Where a page sits: a conversation page is sized to fit and drawn on the conversation's own background.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PagePlace {
    Conversation,
    Tab,
}

impl PagePlace {
    fn key(self) -> &'static str {
        match self {
            Self::Conversation => "conversation",
            Self::Tab => "tab",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PageTheme {
    pub dark: bool,
    pub variables: Map<String, Value>,
}

/// `--font-sans-stack` and `--font-mono-stack` of `main.css`.
pub const FONT_SANS: &str = "\"Inter Variable\", \"Inter\", system-ui, -apple-system, sans-serif";
pub const FONT_MONO: &str = "\"JetBrains Mono Variable\", \"JetBrains Mono\", ui-monospace, monospace";

/// Chart series, in order: one set for dark grounds and one for light (`CHART_SERIES`).
pub const CHART_SERIES: [[&str; 5]; 2] =
    [["#6366f1", "#d95926", "#199e70", "#c98500", "#d55181"], ["#5B6EC7", "#d95926", "#199e70", "#c98500", "#d55181"]];

pub fn chart_series(dark: bool) -> &'static [&'static str; 5] {
    &CHART_SERIES[usize::from(!dark)]
}

/// A colour as CSS writes it: `#rrggbb`, with the alpha pair only when it isn't opaque.
pub fn css(c: Color) -> String {
    let hex = c.hex();
    if hex.ends_with("ff") { hex[..7].to_string() } else { hex }
}

/// `readPageTheme`: each page variable from the app's own. `font_px` is the root font size (Appearance: 13 to 16).
pub fn read_page_theme(t: &Theme, font_px: f32) -> PageTheme {
    let colours: [(&str, Color); 11] = [
        ("--fleet-bg", t.panel_bg),
        ("--fleet-surface", t.card_bg),
        ("--fleet-text", t.text),
        ("--fleet-muted", t.muted),
        ("--fleet-border", t.border),
        ("--fleet-accent", t.accent),
        ("--fleet-accent-surface", t.accent_dim),
        ("--fleet-accent-text", t.primary_fg),
        ("--fleet-success", t.running),
        ("--fleet-warning", t.idle),
        ("--fleet-danger", t.error),
    ];
    let mut variables = Map::new();
    for (name, colour) in colours {
        variables.insert(name.into(), json!(css(colour)));
    }
    variables.insert("--fleet-info".into(), json!(css(t.complete)));
    variables.insert("--fleet-radius".into(), json!(format!("{RADIUS_CARD}px")));
    variables.insert("--fleet-font-sans".into(), json!(FONT_SANS));
    variables.insert("--fleet-font-mono".into(), json!(FONT_MONO));
    for (i, colour) in chart_series(t.dark).iter().enumerate() {
        variables.insert(format!("--fleet-chart-{}", i + 1), json!(colour));
    }
    variables.insert("--fleet-font-size".into(), json!(format!("{font_px}px")));
    PageTheme { dark: t.dark, variables }
}

impl PageTheme {
    fn appearance(&self) -> &'static str {
        if self.dark { "dark" } else { "light" }
    }

    /// `pageFrameName`: `fleet:` and the place and theme as JSON, which the page's script reads before it draws.
    pub fn frame_name(&self, place: PagePlace) -> String {
        format!("fleet:{}", json!({ "place": place.key(), "appearance": self.appearance(), "variables": self.variables }))
    }

    /// `pageThemeMessage`: what a page is sent when the theme changes while it's open.
    pub fn message(&self) -> Value {
        json!({ "type": "fleet:theme", "appearance": self.appearance(), "variables": self.variables })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_the_apps_own_colours_and_fonts_as_the_pages_variables() {
        let t = Theme::of(true);
        let theme = read_page_theme(&t, 14.0);
        assert!(theme.dark);
        let v = |name: &str| theme.variables[name].as_str().unwrap().to_string();
        assert_eq!(v("--fleet-bg"), "#141418");
        assert_eq!(v("--fleet-surface"), "#1b1b20");
        assert_eq!(v("--fleet-text"), "#e8e8ec");
        assert_eq!(v("--fleet-accent"), "#6366f1");
        assert_eq!(v("--fleet-success"), "#22c55e");
        assert_eq!(v("--fleet-warning"), "#f59e0b");
        assert_eq!(v("--fleet-danger"), "#ef4444");
        assert_eq!(v("--fleet-info"), "#3b82f6");
        assert_eq!(v("--fleet-accent-text"), "#ffffff");
        assert_eq!(v("--fleet-radius"), "10px");
        assert_eq!(v("--fleet-font-sans"), FONT_SANS);
        assert_eq!(v("--fleet-font-mono"), FONT_MONO);
        assert_eq!(v("--fleet-font-size"), "14px");
        // The border and the accent wash are translucent, so they keep their alpha.
        assert_eq!(v("--fleet-border").len(), 9);
        assert_eq!(v("--fleet-accent-surface").len(), 9);
    }

    #[test]
    fn gives_a_light_theme_the_chart_series_for_light_grounds() {
        let light = read_page_theme(&Theme::of(false), 14.0);
        assert!(!light.dark);
        let series: Vec<&str> = (1..=5).map(|i| light.variables[&format!("--fleet-chart-{i}")].as_str().unwrap()).collect();
        assert_eq!(series, CHART_SERIES[1]);
        assert_eq!(series[0], "#5B6EC7");
        let dark = read_page_theme(&Theme::of(true), 14.0);
        assert_eq!(dark.variables["--fleet-chart-1"], "#6366f1");
        assert_eq!(light.variables["--fleet-bg"], "#ffffff");
    }

    #[test]
    fn the_font_size_follows_appearance() {
        assert_eq!(read_page_theme(&Theme::of(true), 16.0).variables["--fleet-font-size"], "16px");
    }

    #[test]
    fn frame_name_is_fleet_and_the_place_and_theme_as_json_which_the_pages_script_reads() {
        let theme = read_page_theme(&Theme::of(false), 14.0);
        let name = theme.frame_name(PagePlace::Conversation);
        let json: Value = serde_json::from_str(name.strip_prefix("fleet:").expect("fleet: first")).unwrap();
        assert_eq!(json, json!({ "place": "conversation", "appearance": "light", "variables": theme.variables }));
        assert_eq!(theme.message(), json!({ "type": "fleet:theme", "appearance": "light", "variables": theme.variables }));
        assert!(theme.frame_name(PagePlace::Tab).contains("\"place\":\"tab\""));
    }
}
