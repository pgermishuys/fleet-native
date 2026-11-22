//! Fleet's design tokens, copied from `client/src/assets/main.css` (`:root` is dark,
//! `:root[data-theme="light"]` is light). Rune's windows blend in sRGB as CSS does, so the
//! `rgba()` and `color-mix()` values carry over as they are.
use crate::fleet::pr_state::PrState;
use rune::tok::TextStyle;
use rune::{Color, Shadow};

fn c(s: &str) -> Color {
    rune::color::hex(s).expect("a colour literal")
}

/// One Dark and One Light, Fleet's no-JS defaults (`markdown.css`), before a code theme is chosen.
fn default_syntax(dark: bool) -> [Color; 8] {
    let h: [&str; 8] = if dark {
        ["#c678dd", "#98c379", "#d19a66", "#8b949e", "#61afef", "#e5c07b", "#e06c75", "#56b6c2"]
    } else {
        ["#a626a4", "#50a14f", "#986801", "#a0a1a7", "#4078f2", "#c18401", "#e45649", "#0184bc"]
    };
    h.map(c)
}

#[derive(Clone, Copy)]
pub struct Theme {
    pub dark: bool,
    /// The eight `--syntax-*` colours code is drawn in: One Dark or One Light until the app picks the code theme chosen
    /// on Appearance (`with_syntax`).
    syntax: [Color; 8],
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
    /// `--coral` and `--coral-text`: a machine's colour, apart from every status colour (the machine heading).
    pub coral: Color,
    pub coral_text: Color,
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
                syntax: default_syntax(dark),
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
                coral: c("#D95A3A"),
                coral_text: c("#DE6E52"),
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
                syntax: default_syntax(dark),
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
                coral: c("#D95A3A"),
                coral_text: c("#A7452D"),
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

    /// `--status-waiting` (`var(--idle)`): what the cards that wait on you are tinted with.
    pub fn status_waiting(&self) -> Color {
        self.idle
    }

    /// The Board's Idle and Waiting status colours (`boardStatusOptions` in `stores/board.ts`): literals `#71717a` and
    /// `#f59e0b` in Fleet, not tokens, so the same in both themes.
    pub fn board_status_idle(&self) -> Color {
        c("#71717a")
    }

    pub fn board_status_waiting(&self) -> Color {
        c("#f59e0b")
    }

    /// The Files tree's change badges (`.file-browser-tree-node__badge--green|orange|red`): fixed hex in Fleet, not tokens, so the
    /// same in both themes. Added, modified, deleted.
    pub fn badge_added(&self) -> Color {
        c("#10b981")
    }

    pub fn badge_modified(&self) -> Color {
        c("#f59e0b")
    }

    pub fn badge_deleted(&self) -> Color {
        c("#ef4444")
    }

    /// `.page-canvas__frame { background: #fff }`: a page's frame before the page draws, white in both themes (a literal in
    /// Fleet too: pages without a background of their own expect a white one).
    pub fn page_frame(&self) -> Color {
        Color::WHITE
    }

    /// `.agent-tab__caption`'s fill (`rgb(20 20 24 / 0.86)`) and ink (`#f2f2f5`): literals in Fleet, the same in both themes, since
    /// the caption sits on a picture of someone else's page.
    pub fn agent_caption_bg(&self) -> Color {
        c("#141418").alpha(0.86)
    }

    pub fn agent_caption_ink(&self) -> Color {
        c("#f2f2f5")
    }

    /// `.agent-tab__caption--failed`: `color-mix(in srgb, var(--error) 80%, black)`.
    pub fn agent_caption_failed_bg(&self) -> Color {
        self.mix(self.error, Color::BLACK, 0.8)
    }

    /// `var(--warning, #b45309)`: weave-fleet's main.css defines no `--warning`, so Fleet always draws the fallback. It tints
    /// an Allowed rule that covers every use of a tool.
    pub fn warning(&self) -> Color {
        c("#b45309")
    }

    /// `--diff-add` (`var(--running)`) and `--diff-del` (`var(--error)`): the fill of an added and a removed line.
    pub fn diff_add(&self) -> Color {
        self.running
    }

    pub fn diff_del(&self) -> Color {
        self.error
    }

    /// `--diff-add-text` (`var(--running-text)`) and `--diff-del-text` (`var(--error-text)`): the `+N` and `−N` counts.
    pub fn diff_add_text(&self) -> Color {
        self.running_text
    }

    pub fn diff_del_text(&self) -> Color {
        self.error_text
    }

    /// `--pr-open`, `--pr-draft`, `--pr-blocked`, `--pr-merged` and `--pr-closed`: a pull request's colour by state.
    pub fn pr_tone(&self, state: PrState) -> Color {
        match state {
            PrState::Open => self.running,
            PrState::Draft => self.muted,
            PrState::Blocked => self.idle,
            PrState::Merged => self.queued,
            PrState::Closed => self.error,
        }
    }

    /// `--pr-open-text` and the rest: the same colours as ink that reads on the page (a draft stays `--pr-draft`).
    pub fn pr_tone_text(&self, state: PrState) -> Color {
        match state {
            PrState::Open => self.running_text,
            PrState::Draft => self.muted,
            PrState::Blocked => self.status_waiting_text,
            PrState::Merged => self.queued_text,
            PrState::Closed => self.error_text,
        }
    }

    /// `--check-pass` (`var(--running)`), `--check-fail` (`var(--error)`) and `--check-pending` (`var(--status-waiting)`).
    pub fn check_pass(&self) -> Color {
        self.running
    }

    pub fn check_fail(&self) -> Color {
        self.error
    }

    pub fn check_pending(&self) -> Color {
        self.idle
    }

    /// `color-mix(in srgb, var(--text) N%, transparent)`: Fleet's hover and selection washes.
    pub fn wash(&self, percent: f32) -> Color {
        self.text.alpha(percent / 100.0)
    }

    /// The bar over a folded run in a file's Diff is `@codemirror/merge`'s own `.cm-collapsedLines` (`#f3f3f3` fading in and out, `#888`
    /// ink), which Fleet's theme doesn't restyle: the same in both themes, so a white band on dark. Ported as it is.
    pub fn merge_fold(&self) -> (Color, Color) {
        (c("#f3f3f3"), c("#888888"))
    }

    /// `rgba(239, 68, 68, a)`: the red of the list's "showing cached sessions" banner. Fleet writes it as a literal, so
    /// it is the same in both themes (`--error` is `#ef4444` dark and `#dc2626` light).
    pub fn feedback_red(&self, alpha: f32) -> Color {
        c("#ef4444").alpha(alpha)
    }

    /// `color-mix(in srgb, <color> share, var(--text))`: a status colour as a row's text (`--row-error` and the rest).
    pub fn text_mix(&self, color: Color, share: f32) -> Color {
        self.mix(color, self.text, share)
    }

    /// `--terminal-bg` (`TerminalDrawer.vue`): `color-mix(in srgb, var(--panel-bg) 96%, var(--text))`, the terminal drawer's
    /// ground, a step off the panel.
    pub fn terminal_bg(&self) -> Color {
        self.mix(self.panel_bg, self.text, 0.96)
    }

    /// `color-mix(in srgb, a share%, b)`.
    pub fn mix(&self, a: Color, b: Color, share: f32) -> Color {
        // CSS mixes the sRGB numbers; Rune's colours are linear, so decode, mix and encode back.
        let m =
            |x: f32, y: f32| rune::color::srgb_decode(share * rune::color::srgb_encode(x) + (1.0 - share) * rune::color::srgb_encode(y));
        Color { r: m(a.r, b.r), g: m(a.g, b.g), b: m(a.b, b.b), a: 1.0 }
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

    /// The eight `--syntax-*` tokens, in the order keyword, string, number, comment, function, attr, type, literal: the code
    /// theme chosen on Appearance (`with_syntax`), One Dark or One Light before any is.
    pub fn syntax(&self) -> [Color; 8] {
        self.syntax
    }

    /// The same theme with the colours of a code theme (`applyCodeTheme`).
    pub fn with_syntax(mut self, hex: &[impl AsRef<str>; 8]) -> Theme {
        self.syntax = std::array::from_fn(|i| c(hex[i].as_ref()));
        self
    }

    /// `--text`, `--main-bg`, `--panel-bg` and `--card-bg` as Fleet's CSS has them: the surfaces code sits on, which a code
    /// theme's colours are lifted to read on (`documentCodeGrounds`).
    pub fn code_grounds(&self) -> crate::fleet::settings::code_themes::Grounds {
        let (text, surfaces) =
            if self.dark { ("#e8e8ec", ["#0d0d10", "#141418", "#1b1b20"]) } else { ("#1a1918", ["#f3f2ef", "#ffffff", "#ffffff"]) };
        crate::fleet::settings::code_themes::Grounds { text: text.into(), surfaces: surfaces.map(String::from).to_vec() }
    }

    /// The four colours of a theme card's picture on Appearance: window, card, accent, text (`themes[].swatches` in
    /// `stores/theme.ts`; literals there, not tokens, so the card shows the theme whichever one is on).
    pub fn swatches(dark: bool) -> [Color; 4] {
        let h = if dark { ["#0d0d10", "#1b1b20", "#6366f1", "#e8e8ec"] } else { ["#F3F2EF", "#ffffff", "#5B6EC7", "#1A1918"] };
        h.map(c)
    }

    /// The System card's picture: `from-[#0a0a0b] via-[#0a0a0b] to-[#f1f5f9]`, dark on the left and light on the right.
    pub fn system_swatch() -> [Color; 2] {
        [c("#0a0a0b"), c("#f1f5f9")]
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

    /// `.selection-bar`'s `box-shadow: 0 12px 32px -12px rgba(0, 0, 0, 0.35)`, a literal in `SessionsPanel.vue` (both themes).
    pub fn selection_shadow(&self) -> Shadow {
        Shadow { x: 0.0, y: 12.0, blur: 32.0, spread: -12.0, color: Color::BLACK.alpha(0.35), inset: false }
    }

    /// `.ns-pop`'s `box-shadow: 0 16px 40px -12px rgba(0, 0, 0, 0.35)`, a literal in `new-session.css` (the same in both themes).
    pub fn pop_shadow(&self) -> Shadow {
        Shadow { x: 0.0, y: 16.0, blur: 40.0, spread: -12.0, color: Color::BLACK.alpha(0.35), inset: false }
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

/// Chart.js's own defaults on the Overview's chart, which Fleet doesn't set (so they are not CSS variables): the tooltip's
/// fill (`rgba(0, 0, 0, 0.8)`) and its ink (white), and the right axis' tick marks, whose grid has no colour of Fleet's
/// (`Chart.defaults.borderColor`, `rgba(0, 0, 0, 0.1)`).
pub fn chart_tip_bg() -> Color {
    Color::BLACK.alpha(0.8)
}

pub fn chart_tip_ink() -> Color {
    Color::WHITE
}

pub fn chart_default_grid() -> Color {
    Color::BLACK.alpha(0.1)
}

/// Inter at `size`/`weight` with a CSS line-height multiplier.
pub const fn sans(size: f32, weight: f32, line: f32) -> TextStyle {
    TextStyle::new(size, weight).lh(size * line)
}

/// JetBrains Mono at `size` with a CSS line-height multiplier.
pub const fn mono(size: f32, weight: f32, line: f32) -> TextStyle {
    TextStyle::new(size, weight).mono().lh(size * line)
}

impl Theme {
    /// `.usage-pop__tick`'s `#fff`: the pace mark on a usage bar, white in both themes.
    pub fn usage_tick(&self) -> Color {
        c("#ffffff")
    }

    /// `color-mix(in srgb, #000 45%, transparent)`: the 1 px ring that keeps the tick visible on the filled and the empty part.
    pub fn usage_tick_ring(&self) -> Color {
        c("#000000").alpha(0.45)
    }
}
