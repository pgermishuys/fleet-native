//! What Appearance keeps (`stores/theme.ts`): the theme (System, Dark or Light), the code theme, the font size and how
//! thinking is drawn. Fleet keeps each in `localStorage` (`weave:theme`, `weave:code-theme`, `weave:font-size`,
//! `weave:thinking`); here they are four keys of the app's own settings file. A stored value that isn't one of the choices
//! reads as the default, as in Fleet. Plain Rust; the file is read and written off the UI thread (`State::save_settings`).
//!
//! The font family has one choice ("inter") that is always the one in use, so nothing is kept for it. The theme choices
//! other than Dark and Light are out of scope (`docs/PORT.md`), so are not read.
use super::code_themes::{CodeThemeSelection, readable_palette, resolve_palette};
use crate::State;
use crate::conv::gist::Thinking;
use serde_json::{Value, json};

/// The code theme's colours as last worked out, for which choice and window scheme (`State::with_code_theme`).
pub type PaletteCache = std::cell::RefCell<Option<((CodeThemeSelection, bool), [String; 8])>>;

/// `ThemeSelection`, as far as the native app has themes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThemeChoice {
    #[default]
    System,
    Dark,
    Light,
}

impl ThemeChoice {
    pub fn key(self) -> &'static str {
        match self {
            ThemeChoice::System => "system",
            ThemeChoice::Dark => "dark",
            ThemeChoice::Light => "light",
        }
    }

    /// An unknown value, or a theme this app doesn't have, reads as System (`readStoredTheme`).
    pub fn parse(value: &str) -> ThemeChoice {
        match value {
            "dark" => ThemeChoice::Dark,
            "light" => ThemeChoice::Light,
            _ => ThemeChoice::System,
        }
    }

    /// What `State.dark` holds for it: `None` follows the computer.
    pub fn dark(self) -> Option<bool> {
        match self {
            ThemeChoice::System => None,
            ThemeChoice::Dark => Some(true),
            ThemeChoice::Light => Some(false),
        }
    }
}

/// `fontSizes`: the interface's base size in pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FontSize {
    S,
    #[default]
    M,
    L,
    Xl,
}

impl FontSize {
    pub const ALL: [FontSize; 4] = [FontSize::S, FontSize::M, FontSize::L, FontSize::Xl];

    pub fn key(self) -> &'static str {
        match self {
            FontSize::S => "s",
            FontSize::M => "m",
            FontSize::L => "l",
            FontSize::Xl => "xl",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            FontSize::S => "S",
            FontSize::M => "M",
            FontSize::L => "L",
            FontSize::Xl => "XL",
        }
    }

    /// The root font size Fleet gives the page; `1rem` is this many pixels.
    pub fn px(self) -> f32 {
        match self {
            FontSize::S => 13.0,
            FontSize::M => 14.0,
            FontSize::L => 15.0,
            FontSize::Xl => 16.0,
        }
    }

    /// An unknown value reads as "m" (`readStoredFontSize`).
    pub fn parse(value: &str) -> FontSize {
        FontSize::ALL.into_iter().find(|s| s.key() == value).unwrap_or_default()
    }
}

pub fn thinking_key(t: Thinking) -> &'static str {
    match t {
        Thinking::Folded => "folded",
        Thinking::Open => "open",
        Thinking::Hidden => "hidden",
    }
}

/// An unknown value reads as "folded" (`readStoredThinking`).
pub fn parse_thinking(value: &str) -> Thinking {
    [Thinking::Folded, Thinking::Open, Thinking::Hidden].into_iter().find(|t| thinking_key(*t) == value).unwrap_or_default()
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Appearance {
    pub theme: ThemeChoice,
    pub code_theme: CodeThemeSelection,
    pub font_size: FontSize,
    pub thinking: Thinking,
}

impl Appearance {
    /// Reads the keys of the settings file's root; a missing or wrongly typed one is its default.
    pub fn parse(root: &serde_json::Map<String, Value>) -> Appearance {
        let text = |k: &str| root.get(k).and_then(Value::as_str);
        Appearance {
            theme: text("theme").map(ThemeChoice::parse).unwrap_or_default(),
            code_theme: text("codeTheme").and_then(CodeThemeSelection::parse).unwrap_or_default(),
            font_size: text("fontSize").map(FontSize::parse).unwrap_or_default(),
            thinking: text("thinking").map(parse_thinking).unwrap_or_default(),
        }
    }

    /// Only what differs from the defaults is written, so a file with no choices has none of these keys.
    pub fn write(&self, root: &mut Value) {
        let d = Appearance::default();
        if self.theme != d.theme {
            root["theme"] = json!(self.theme.key());
        }
        if self.code_theme != d.code_theme {
            root["codeTheme"] = json!(self.code_theme.key());
        }
        if self.font_size != d.font_size {
            root["fontSize"] = json!(self.font_size.key());
        }
        if self.thinking != d.thinking {
            root["thinking"] = json!(thinking_key(self.thinking));
        }
    }
}

impl State {
    /// A scene: a reply with a code block, drawn with this code theme on a dark or light window.
    pub fn show_code_theme(&mut self, selection: CodeThemeSelection, dark: bool) {
        self.show_conversation(crate::mock::find::of_marks());
        crate::conv::fold::toggle_turn(&mut self.turns_open, "u1");
        self.refold();
        self.sidebar.update_appearance(|a| a.code_theme = selection);
        self.dark = Some(dark);
    }

    /// The theme with the chosen code theme's colours, each lifted to read on the window's surfaces (`applyCodeTheme`).
    /// The lift is worked out again only when the choice or the window's scheme changes.
    pub fn with_code_theme(&self, t: crate::theme::Theme) -> crate::theme::Theme {
        let selection = self.sidebar.appearance().code_theme;
        let mut cache = self.code_palette.borrow_mut();
        if cache.as_ref().is_none_or(|(key, _)| *key != (selection, t.dark)) {
            let theme_id = if t.dark { "dark" } else { "light" };
            let palette = readable_palette(resolve_palette(selection, theme_id, t.dark), &t.code_grounds());
            *cache = Some(((selection, t.dark), palette));
        }
        t.with_syntax(&cache.as_ref().expect("just set").1)
    }

    /// The stored choices take effect: the window's theme and how thinking is drawn.
    pub(crate) fn apply_appearance(&mut self) {
        let a = self.sidebar.appearance();
        self.dark = a.theme.dark();
        self.thinking = a.thinking;
    }

    /// A scene's choices are drawn, but the window's theme stays what the scene set.
    pub(crate) fn apply_appearance_scene(&mut self) {
        self.thinking = self.sidebar.appearance().thinking;
    }

    fn change_appearance(&mut self, change: impl FnOnce(&mut Appearance)) {
        self.sidebar.update_appearance(change);
        self.apply_appearance();
        self.save_settings();
    }

    /// A theme card was picked (`setTheme`).
    pub fn set_theme(&mut self, choice: ThemeChoice) {
        self.change_appearance(|a| a.theme = choice);
    }

    /// Light for dark and dark for light, by what is drawn now, and the choice is kept (`toggleTheme`).
    pub fn toggle_theme(&mut self) {
        let choice = if self.drawn_dark.get() { ThemeChoice::Light } else { ThemeChoice::Dark };
        self.set_theme(choice);
    }

    /// A code theme card was picked (`setCodeTheme`). The colours follow at the next frame.
    pub fn set_code_theme(&mut self, selection: CodeThemeSelection) {
        self.change_appearance(|a| a.code_theme = selection);
    }

    /// The size is kept; the window doesn't scale with it yet (`docs/parity/16-settings.md`, row 3.20).
    pub fn set_font_size(&mut self, size: FontSize) {
        self.change_appearance(|a| a.font_size = size);
    }

    /// A Thinking option was picked (`setThinking`).
    pub fn set_thinking(&mut self, display: Thinking) {
        self.change_appearance(|a| a.thinking = display);
    }
}
