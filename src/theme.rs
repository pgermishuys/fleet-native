//! Fleet's design tokens, copied from `client/src/assets/main.css` (`:root` is dark,
//! `:root[data-theme="light"]` is light). Rune's windows blend in sRGB as CSS does, so the
//! `rgba()` and `color-mix()` values carry over as they are.
use rune::tok::TextStyle;
use rune::{Color, Shadow};

fn c(s: &str) -> Color {
    rune::color::hex(s).expect("a colour literal")
}

#[derive(Clone, Copy)]
pub struct Theme {
    pub dark: bool,
    /// `--main-bg`: the window behind the rail, the session list and the status bar.
    pub main_bg: Color,
    /// `--panel-bg`: the workspace sheet.
    pub panel_bg: Color,
    /// `--card-bg`: the composer and the New session button.
    pub card_bg: Color,
    pub border: Color,
    pub text: Color,
    pub muted: Color,
    pub accent: Color,
    /// `--accent-text`: accent ink that reads on the page (the older-messages spinner).
    pub accent_text: Color,
    pub running: Color,
    pub idle: Color,
    /// `--idle-text`: amber ink that reads on the page (the same as `--idle` in the dark theme).
    pub idle_text: Color,
    /// `--status-waiting-text` (`var(--idle-text)`): the usage bars from 80%.
    pub status_waiting_text: Color,
    /// `#1a1400`, the ink on the current find match (a literal in `main.css`, beside `--status-waiting`).
    pub find_ink: Color,
    pub error: Color,
    /// `--running-text` and `--error-text`: green and red ink that read on the page (the `+N −N` counts).
    pub running_text: Color,
    pub error_text: Color,
    /// `--primary-foreground`: ink on a solid accent fill.
    pub primary_fg: Color,
    /// `--destructive` (and `--destructive-foreground`, white in both themes).
    pub destructive: Color,
    /// `--accent-dim`: the active filter chip's fill.
    pub accent_dim: Color,
    /// `--complete` and `--queued`.
    pub complete: Color,
    pub queued: Color,
    /// `--complete-text` and `--queued-text`: blue and violet ink that read on the page (terminal colours).
    pub complete_text: Color,
    pub queued_text: Color,
    /// How much of `--text` a quiet session row keeps after a day (`--row-dim-1`) and after three (`--row-dim-2`).
    pub row_dim: [f32; 2],
    /// `--row-meta`: the share of `--text` a row's age and labels take.
    pub row_meta: f32,
    /// `--row-error`, `--row-waiting`, `--row-accent`, `--row-running`, `--row-queued`: how much of itself a status
    /// colour keeps as row text; the rest comes from `--text`.
    pub row_error: f32,
    pub row_waiting: f32,
    pub row_accent: f32,
    pub row_running: f32,
    pub row_queued: f32,
}

impl Theme {
    pub fn of(dark: bool) -> Theme {
        if dark {
            Theme {
                dark,
                main_bg: c("#0d0d10"),
                panel_bg: c("#141418"),
                card_bg: c("#1b1b20"),
                border: Color::WHITE.alpha(0.075),
                text: c("#e8e8ec"),
                muted: c("#8e8e9a"),
                accent: c("#6366f1"),
                accent_text: c("#8284F4"),
                running: c("#22c55e"),
                idle: c("#f59e0b"),
                idle_text: c("#f59e0b"),
                status_waiting_text: c("#f59e0b"),
                find_ink: c("#1a1400"),
                error: c("#ef4444"),
                running_text: c("#22c55e"),
                error_text: c("#EF5E5F"),
                primary_fg: c("#ffffff"),
                destructive: c("#dc2626"),
                accent_dim: c("#6366f1").alpha(0.15),
                complete: c("#3b82f6"),
                queued: c("#8b5cf6"),
                complete_text: c("#4F8FF7"),
                queued_text: c("#9F78F8"),
                row_dim: [0.66, 0.51],
                row_meta: 0.53,
                row_error: 0.71,
                row_waiting: 1.0,
                row_accent: 0.67,
                row_running: 1.0,
                row_queued: 0.69,
            }
        } else {
            Theme {
                dark,
                main_bg: c("#F3F2EF"),
                panel_bg: c("#FFFFFF"),
                card_bg: c("#FFFFFF"),
                border: c("#1A1918").alpha(0.09),
                text: c("#1A1918"),
                muted: c("#6E6A65"),
                accent: c("#5B6EC7"),
                accent_text: c("#4D5EA9"),
                running: c("#16a34a"),
                idle: c("#d97706"),
                idle_text: c("#975304"),
                status_waiting_text: c("#975304"),
                find_ink: c("#1a1400"),
                error: c("#dc2626"),
                running_text: c("#107435"),
                error_text: c("#B92323"),
                primary_fg: c("#ffffff"),
                destructive: c("#dc2626"),
                accent_dim: c("#5B6EC7").alpha(0.12),
                complete: c("#2563eb"),
                queued: c("#7c3aed"),
                complete_text: c("#2159D4"),
                queued_text: c("#7537DF"),
                row_dim: [0.77, 0.62],
                row_meta: 0.63,
                row_error: 0.71,
                row_waiting: 0.55,
                row_accent: 0.69,
                row_running: 0.56,
                row_queued: 0.80,
            }
        }
    }

    /// `color-mix(in srgb, var(--text) N%, transparent)`: Fleet's hover and selection washes.
    pub fn wash(&self, percent: f32) -> Color {
        self.text.alpha(percent / 100.0)
    }

    /// `color-mix(in srgb, <color> share, var(--text))`: a status colour as a row's text (`--row-error` and the rest).
    pub fn text_mix(&self, color: Color, share: f32) -> Color {
        // CSS mixes the sRGB numbers; Rune's colours are linear, so decode, mix and encode back.
        let mix =
            |a: f32, b: f32| rune::color::srgb_decode(share * rune::color::srgb_encode(a) + (1.0 - share) * rune::color::srgb_encode(b));
        Color { r: mix(color.r, self.text.r), g: mix(color.g, self.text.g), b: mix(color.b, self.text.b), a: 1.0 }
    }

    /// The row text `--row-meta` of `--text`: a session's age, labels and progress count.
    pub fn row_meta_ink(&self) -> Color {
        self.text.alpha(self.row_meta)
    }

    /// `color-mix(in srgb, var(--accent) P%, var(--border))`: the frame's border while it has focus (55) or is in
    /// shell or side mode (70). `--border` is translucent, so the mix is premultiplied as CSS does it; Rune's
    /// colours are linear, so the blend is a little off the browser's, which mixes the sRGB numbers.
    pub fn accent_border(&self, percent: f32) -> Color {
        self.border_mix(self.accent, percent)
    }

    /// `color-mix(in srgb, <color> P%, var(--border))`: a card's border tinted toward `color` (the failure card's
    /// `--error` at 35%). Premultiplied, as `accent_border` explains.
    pub fn border_mix(&self, color: Color, percent: f32) -> Color {
        let (p, b, a) = (percent / 100.0, self.border, color);
        let alpha = p * a.a + (1.0 - p) * b.a;
        let m = |x: f32, y: f32| (p * x * a.a + (1.0 - p) * y * b.a) / alpha;
        Color { r: m(a.r, b.r), g: m(a.g, b.g), b: m(a.b, b.b), a: alpha }
    }

    /// `color-mix(in srgb, <color> P%, var(--card-bg))`: the composer's frame while files are dragged over it.
    pub fn card_mix(&self, color: Color, percent: f32) -> Color {
        let (p, b) = (percent / 100.0, self.card_bg);
        Color { r: p * color.r + (1.0 - p) * b.r, g: p * color.g + (1.0 - p) * b.g, b: p * color.b + (1.0 - p) * b.b, a: b.a }
    }

    /// `--md-marker`: `color-mix(in srgb, var(--accent) 55%, var(--muted))`, a list's bullets and numbers. Rune mixes in
    /// linear light, so it lands a little off the browser's.
    pub fn md_marker(&self) -> Color {
        self.muted.mix(self.accent, 0.55)
    }

    /// The eight `--syntax-*` tokens of `markdown.css` (One Dark and One Light, Fleet's no-JS defaults), in the order
    /// keyword, string, number, comment, function, attr, type, literal. A chosen code theme overrides them in Fleet;
    /// the code themes land with Settings.
    pub fn syntax(&self) -> [Color; 8] {
        let h: [&str; 8] = if self.dark {
            ["#c678dd", "#98c379", "#d19a66", "#8b949e", "#61afef", "#e5c07b", "#e06c75", "#56b6c2"]
        } else {
            ["#a626a4", "#50a14f", "#986801", "#a0a1a7", "#4078f2", "#c18401", "#e45649", "#0184bc"]
        };
        h.map(c)
    }

    /// `--color-secondary`: `color-mix(in srgb, var(--text) 8%, transparent)`, the segmented tabs' track.
    pub fn secondary(&self) -> Color {
        self.wash(8.0)
    }

    /// `--color-input`: `color-mix(in srgb, var(--text) 18%, transparent)`, field borders and the
    /// Switch's off track.
    pub fn input(&self) -> Color {
        self.wash(18.0)
    }

    /// `ImageLightbox.vue`: `rgba(0,0,0,.8)` over the whole window, a literal in Fleet too (the same in both themes).
    pub fn scrim(&self) -> Color {
        Color::BLACK.alpha(0.8)
    }

    /// `.attachment-chip__remove`: `rgba(0,0,0,.6)`, and `rgba(220,38,38,.8)` under the pointer (literals in Fleet too).
    pub fn chip_remove(&self) -> Color {
        Color::BLACK.alpha(0.6)
    }

    pub fn chip_remove_hover(&self) -> Color {
        rune::color::hex("#DC2626").expect("a colour literal").alpha(0.8)
    }

    /// The lightbox picture's `0 8px 32px rgba(0,0,0,.5)`.
    pub fn lightbox_shadow(&self) -> Shadow {
        Shadow { x: 0.0, y: 8.0, blur: 32.0, spread: 0.0, color: Color::BLACK.alpha(0.5), inset: false }
    }

    /// `--menu-shadow`: the raised look of menus, popovers and dialogs' lift.
    pub fn menu_shadows(&self) -> Vec<Shadow> {
        let (a, b) = if self.dark {
            (c("#000000").alpha(0.55), c("#000000").alpha(0.25))
        } else {
            (c("#1A1918").alpha(0.22), c("#1A1918").alpha(0.06))
        };
        vec![
            Shadow { x: 0.0, y: 16.0, blur: 40.0, spread: -12.0, color: a, inset: false },
            Shadow { x: 0.0, y: 2.0, blur: 6.0, spread: 0.0, color: b, inset: false },
        ]
    }

    /// `--sheet-shadow`: none in dark, two soft layers in light.
    pub fn sheet_shadows(&self) -> Vec<Shadow> {
        if self.dark {
            return Vec::new();
        }
        let ink = c("#1A1918");
        vec![
            Shadow { x: 0.0, y: 1.0, blur: 2.0, spread: 0.0, color: ink.alpha(0.04), inset: false },
            Shadow { x: 0.0, y: 12.0, blur: 32.0, spread: -18.0, color: ink.alpha(0.18), inset: false },
        ]
    }
}

/// `--radius-card`, `--radius-btn` and `--radius-panel`.
pub const RADIUS_CARD: f32 = 10.0;
pub const RADIUS_BTN: f32 = 8.0;
pub const RADIUS_PANEL: f32 = 12.0;

/// Inter at `size`/`weight` with a CSS line-height multiplier.
pub const fn sans(size: f32, weight: f32, line: f32) -> TextStyle {
    TextStyle::new(size, weight).lh(size * line)
}

/// JetBrains Mono at `size` with a CSS line-height multiplier.
pub const fn mono(size: f32, weight: f32, line: f32) -> TextStyle {
    TextStyle::new(size, weight).mono().lh(size * line)
}
