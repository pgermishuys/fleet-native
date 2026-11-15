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
            }
        }
    }

    /// `color-mix(in srgb, var(--text) N%, transparent)`: Fleet's hover and selection washes.
    pub fn wash(&self, percent: f32) -> Color {
        self.text.alpha(percent / 100.0)
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
