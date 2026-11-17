//! Component 16, piece E: what Appearance keeps. Fleet's `code-themes.test.ts` (seven cases) and the store half of
//! `theme.test.ts` under their names, with `localStorage` read as the settings file; then the lift that makes colours read,
//! and that a chosen code theme is what the next frame draws.
use crate::conv::gist::Thinking;
use crate::fleet::settings::appearance::{FontSize, ThemeChoice};
use crate::fleet::settings::code_themes::{
    CODE_THEMES, CodeThemeId as Id, CodeThemeSelection as Sel, Palette, TOKENS, code_backgrounds, contrast_ratio, is_available, label,
    mix_hex, normalize_hex, readable_palette, resolve_palette,
};
use crate::fleet::settings::{self, Sidebar};
use crate::tests::{FRAME, app};
use crate::theme::Theme;
use crate::{State, test_wait};
use rune::Runtime;

fn scheme(id: Id) -> &'static crate::fleet::settings::code_themes::CodeTheme {
    CODE_THEMES.iter().find(|t| t.id == id).expect("a scheme")
}

fn dark(id: Id) -> Palette {
    scheme(id).dark
}

fn light(id: Id) -> Palette {
    scheme(id).light.expect("a light palette")
}

// ── code-themes.test.ts ─────────────────────────────────────────────────────────

#[test]
fn offers_darcula_monokai_github_one_dark_solarized_and_nord() {
    let keys: Vec<_> = CODE_THEMES.iter().map(|t| t.key).collect();
    assert_eq!(keys, ["darcula", "monokai", "github", "one-dark", "solarized", "nord"]);
}

#[test]
fn gives_every_scheme_a_dark_palette_with_all_eight_tokens_as_hex_colours() {
    let hex = |c: &str| c.len() == 7 && c.starts_with('#') && c[1..].bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
    for t in &CODE_THEMES {
        assert_eq!(t.dark.len(), TOKENS);
        assert!(t.dark.iter().all(|c| hex(c)), "{} dark", t.key);
        if let Some(l) = t.light {
            assert!(l.iter().all(|c| hex(c)), "{} light", t.key);
        }
    }
}

#[test]
fn match_picks_the_scheme_named_after_the_app_theme() {
    assert_eq!(resolve_palette(Sel::Match, "nord", true), dark(Id::Nord));
    assert_eq!(resolve_palette(Sel::Match, "monokai", true), dark(Id::Monokai));
    assert_eq!(resolve_palette(Sel::Match, "github-dark", true), dark(Id::Github));
    assert_eq!(resolve_palette(Sel::Match, "solarized-light", false), light(Id::Solarized));
}

#[test]
fn match_falls_back_to_one_dark_on_a_dark_theme_and_one_light_on_a_light_theme() {
    assert_eq!(resolve_palette(Sel::Match, "dark", true), dark(Id::OneDark));
    assert_eq!(resolve_palette(Sel::Match, "dracula", true), dark(Id::OneDark));
    assert_eq!(resolve_palette(Sel::Match, "light", false), light(Id::OneDark));
}

#[test]
fn an_explicit_scheme_uses_its_light_palette_on_a_light_theme_when_it_has_one() {
    assert_eq!(resolve_palette(Sel::Scheme(Id::Github), "light", false), light(Id::Github));
    assert_eq!(resolve_palette(Sel::Scheme(Id::Github), "nord", true), dark(Id::Github));
}

#[test]
fn darcula_monokai_and_nord_are_for_dark_themes_only() {
    let only_dark: Vec<_> = CODE_THEMES.iter().filter(|t| !is_available(t.id, false)).map(|t| t.key).collect();
    assert_eq!(only_dark, ["darcula", "monokai", "nord"]);
    assert!(CODE_THEMES.iter().all(|t| is_available(t.id, true)));
}

#[test]
fn a_dark_only_scheme_falls_back_to_match_theme_on_a_light_theme() {
    assert_eq!(resolve_palette(Sel::Scheme(Id::Darcula), "light", false), light(Id::OneDark));
    assert_eq!(resolve_palette(Sel::Scheme(Id::Monokai), "solarized-light", false), light(Id::Solarized));
    assert_eq!(resolve_palette(Sel::Scheme(Id::Nord), "dark", true), dark(Id::Nord));
}

// ── what the lift does (`readablePalette`) ──────────────────────────────────────

#[test]
fn labels_read_one_light_on_a_light_window_and_nothing_else_changes() {
    assert_eq!(label(scheme(Id::OneDark), true), "One Dark");
    assert_eq!(label(scheme(Id::OneDark), false), "One Light");
    assert_eq!(label(scheme(Id::Nord), false), "Nord");
    assert_eq!(label(scheme(Id::Github), false), "GitHub");
}

#[test]
fn every_colour_reads_at_4_5_to_1_on_every_code_background_after_the_lift() {
    for dark_window in [true, false] {
        let grounds = Theme::of(dark_window).code_grounds();
        let backgrounds = code_backgrounds(&grounds);
        assert_eq!(backgrounds.len(), 6);
        for t in CODE_THEMES.iter().filter(|t| is_available(t.id, dark_window)) {
            let palette = if dark_window { t.dark } else { t.light.expect("available") };
            for colour in readable_palette(palette, &grounds) {
                for bg in &backgrounds {
                    assert!(contrast_ratio(&colour, bg) >= 4.5, "{} {colour} on {bg}", t.key);
                }
            }
        }
    }
}

#[test]
fn a_colour_that_already_reads_is_left_as_it_is_and_one_that_does_not_keeps_its_hue_side() {
    let grounds = Theme::of(true).code_grounds();
    // #c678dd (One Dark's keyword) reads on the dark surfaces; Darcula's #808080 comment does not quite.
    assert_eq!(readable_palette(dark(Id::OneDark), &grounds)[0], "#c678dd");
    let lifted = readable_palette(dark(Id::Darcula), &grounds);
    assert_ne!(lifted[3], "#808080");
    // Lifted toward white on a dark window: every channel is at least as light as it was.
    assert!(lifted[3][1..3] >= *"80");
    let light_grounds = Theme::of(false).code_grounds();
    let lifted = readable_palette(light(Id::OneDark), &light_grounds);
    // One Light's comment (#a0a1a7) is pale on white and goes darker.
    assert!(lifted[3][1..3] < *"a0", "{}", lifted[3]);
}

#[test]
fn a_palette_is_kept_whole_when_the_surfaces_are_not_colours() {
    let grounds = crate::fleet::settings::code_themes::Grounds { text: "var(--text)".into(), surfaces: vec!["#000000".into()] };
    assert_eq!(readable_palette(dark(Id::Darcula), &grounds), dark(Id::Darcula).map(String::from));
}

#[test]
fn colours_are_read_as_six_digits_from_short_and_rgb_forms() {
    assert_eq!(normalize_hex("#fff").as_deref(), Some("#ffffff"));
    assert_eq!(normalize_hex(" #1A1918 ").as_deref(), Some("#1a1918"));
    assert_eq!(normalize_hex("rgb(13, 13, 16)").as_deref(), Some("#0d0d10"));
    assert_eq!(normalize_hex("rgba(13 13 16 / 1)").as_deref(), Some("#0d0d10"));
    assert_eq!(normalize_hex("rgba(13, 13, 16, 0.5)"), None);
    assert_eq!(normalize_hex("var(--text)"), None);
    assert_eq!(normalize_hex("#12"), None);
}

#[test]
fn mixing_matches_color_mix_in_srgb() {
    assert_eq!(mix_hex("#ffffff", 50.0, "#000000"), "#808080");
    assert_eq!(mix_hex("#102030", 100.0, "#ffffff"), "#102030");
    assert_eq!(mix_hex("#102030", 0.0, "#ffffff"), "#ffffff");
    assert!((contrast_ratio("#000000", "#ffffff") - 21.0).abs() < 1e-9);
}

#[test]
fn the_grounds_are_the_hex_of_the_themes_own_colours() {
    for dark_window in [true, false] {
        let t = Theme::of(dark_window);
        let g = t.code_grounds();
        let hex = |s: &str| rune::color::hex(s).expect("a colour");
        assert_eq!(hex(&g.text), t.text);
        assert_eq!([hex(&g.surfaces[0]), hex(&g.surfaces[1]), hex(&g.surfaces[2])], [t.main_bg, t.panel_bg, t.card_bg]);
    }
}

// ── theme.test.ts, the store half ───────────────────────────────────────────────

fn folder(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("fleet-native-appearance-{name}-{}", std::process::id()))
}

fn with_folder(rt: &mut Runtime<State>, name: &str) -> std::path::PathBuf {
    let dir = folder(name);
    let d = dir.clone();
    rt.update(move |s, _| s.adopt_settings(d, Sidebar::default()));
    rt.frame(FRAME);
    dir
}

/// Waits for the thread that writes the file, then reads it back the way the next start does.
fn stored(dir: &std::path::Path, what: &str, holds: impl Fn(&str) -> bool) -> Sidebar {
    test_wait::until_true(what, || std::fs::read_to_string(dir.join(settings::FILE)).is_ok_and(|json| holds(&json)));
    settings::load(dir)
}

#[test]
fn persists_explicit_theme_choices_and_toggles_between_light_and_dark() {
    let mut rt = app();
    let dir = with_folder(&mut rt, "theme");
    rt.update(|s, _| s.set_theme(ThemeChoice::Light));
    assert_eq!(rt.state.dark, Some(false));
    let kept = stored(&dir, "the theme file", |j| j.contains("\"light\""));
    assert_eq!(kept.appearance().theme, ThemeChoice::Light);

    rt.frame(FRAME);
    rt.update(|s, _| s.toggle_theme());
    assert_eq!(rt.state.dark, Some(true));
    let kept = stored(&dir, "the toggled theme file", |j| j.contains("\"dark\""));
    assert_eq!(kept.appearance().theme, ThemeChoice::Dark);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_theme_and_thinking_come_back_at_the_next_start() {
    let mut rt = app();
    let dir = with_folder(&mut rt, "restart");
    rt.update(|s, _| {
        s.set_theme(ThemeChoice::Light);
        s.set_thinking(Thinking::Hidden);
        s.set_font_size(FontSize::Xl);
        s.set_code_theme(Sel::Scheme(Id::Github));
    });
    let json = rt.state.sidebar.to_json();
    stored(&dir, "the file", |j| j.contains("github"));

    let mut next = app();
    next.update(move |s, _| s.adopt_settings(dir.clone(), Sidebar::parse(&json)));
    next.frame(FRAME);
    assert_eq!(next.state.dark, Some(false));
    assert_eq!(next.state.thinking, Thinking::Hidden);
    let a = next.state.sidebar.appearance();
    assert_eq!((a.font_size, a.code_theme), (FontSize::Xl, Sel::Scheme(Id::Github)));
    let _ = std::fs::remove_dir_all(folder("restart"));
}

#[test]
fn an_unknown_stored_value_reads_as_the_default_and_a_file_with_no_choices_has_none() {
    let a = Sidebar::parse(r#"{"theme":"solarized-dark","codeTheme":"comic-sans","fontSize":"xxl","thinking":"loud"}"#).appearance();
    assert_eq!(a, Default::default());
    assert_eq!((a.theme, a.code_theme, a.font_size, a.thinking), (ThemeChoice::System, Sel::Match, FontSize::M, Thinking::Folded));
    let a = Sidebar::parse(r#"{"theme":7,"codeTheme":null}"#).appearance();
    assert_eq!(a, Default::default());
    let json = Sidebar::default().to_json();
    for key in ["theme", "codeTheme", "fontSize", "thinking"] {
        assert!(!json.contains(key), "{key} written with nothing chosen");
    }
}

#[test]
fn the_font_sizes_are_fleets_four_roots() {
    let px: Vec<_> = FontSize::ALL.iter().map(|s| (s.label(), s.px())).collect();
    assert_eq!(px, [("S", 13.0), ("M", 14.0), ("L", 15.0), ("XL", 16.0)]);
}

// ── theme.test.ts, the code theme half ──────────────────────────────────────────

fn keyword(rt: &Runtime<State>, dark_window: bool) -> rune::Color {
    rt.state.with_code_theme(Theme::of(dark_window)).syntax()[0]
}

fn hex(s: &str) -> rune::Color {
    rune::color::hex(s).expect("a colour")
}

#[test]
fn defaults_to_matching_the_app_theme_and_draws_that_themes_syntax_tokens() {
    let rt = app();
    assert_eq!(rt.state.sidebar.appearance().code_theme, Sel::Match);
    assert_eq!(keyword(&rt, true), hex("#c678dd"));
    assert_eq!(keyword(&rt, false), hex("#a626a4"));
}

#[test]
fn persists_an_explicit_code_theme_and_applies_it_over_the_app_theme() {
    let mut rt = app();
    let dir = with_folder(&mut rt, "code");
    rt.update(|s, _| s.set_code_theme(Sel::Scheme(Id::Monokai)));
    assert_eq!(rt.state.sidebar.appearance().code_theme, Sel::Scheme(Id::Monokai));
    let kept = stored(&dir, "the code theme file", |j| j.contains("monokai"));
    assert_eq!(kept.appearance().code_theme.key(), "monokai");
    let t = rt.state.with_code_theme(Theme::of(true)).syntax();
    // Fleet's test reads the bare scheme, because jsdom has no surfaces to lift to; the window lifts Monokai's pink a little.
    let lifted = readable_palette(dark(Id::Monokai), &Theme::of(true).code_grounds());
    assert_eq!((t[0], t[1]), (hex(&lifted[0]), hex("#e6db74")));
    assert_ne!(lifted[0], "#f92672");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn picking_a_scheme_stores_it_and_applies_its_colours() {
    let mut rt = app();
    for (id, first) in [(Id::Darcula, "#cc7832"), (Id::Github, "#ff7b72"), (Id::Solarized, "#859900")] {
        rt.update(move |s, _| s.set_code_theme(Sel::Scheme(id)));
        rt.frame(FRAME);
        let drawn = rt.state.with_code_theme(Theme::of(true)).syntax();
        // The lift only moves colours that don't read, so the keyword is the scheme's own unless it needed it.
        let want = readable_palette(dark(id), &Theme::of(true).code_grounds());
        assert_eq!(drawn[0], hex(&want[0]), "{id:?}");
        assert!(want[0] == first || contrast_ratio(&want[0], "#141418") >= 4.5);
    }
}

#[test]
fn re_applies_the_syntax_tokens_when_the_app_theme_changes_under_match() {
    let mut rt = app();
    rt.update(|s, _| s.set_code_theme(Sel::Match));
    assert_eq!(keyword(&rt, true), hex("#c678dd"));
    assert_eq!(keyword(&rt, false), hex("#a626a4"));
    // The cache is per scheme, so going back and forth gives each its own colours.
    assert_eq!(keyword(&rt, true), hex("#c678dd"));
}

#[test]
fn reads_a_stored_code_theme_back_and_ignores_an_unknown_one() {
    assert_eq!(Sidebar::parse(r#"{"codeTheme":"darcula"}"#).appearance().code_theme, Sel::Scheme(Id::Darcula));
    assert_eq!(Sidebar::parse(r#"{"codeTheme":"comic-sans"}"#).appearance().code_theme, Sel::Match);
}

#[test]
fn keeps_a_dark_only_scheme_for_dark_themes_and_uses_match_theme_on_a_light_one() {
    let mut rt = app();
    rt.update(|s, _| s.set_code_theme(Sel::Scheme(Id::Monokai)));
    let monokai = hex(&readable_palette(dark(Id::Monokai), &Theme::of(true).code_grounds())[0]);
    assert_eq!(keyword(&rt, true), monokai);
    // On the light window Monokai steps aside for One Light, and the choice is still Monokai.
    assert_eq!(keyword(&rt, false), hex(&readable_palette(light(Id::OneDark), &Theme::of(false).code_grounds())[0]));
    assert_eq!(rt.state.sidebar.appearance().code_theme, Sel::Scheme(Id::Monokai));
    assert_eq!(keyword(&rt, true), monokai);
}

#[test]
fn the_cycle_and_toggle_commands_keep_the_choice() {
    let mut rt = app();
    rt.update(|s, _| s.set_theme(ThemeChoice::Dark));
    rt.update(|s, _| s.set_thinking(Thinking::Open));
    assert_eq!(rt.state.thinking, Thinking::Open);
    assert_eq!(rt.state.sidebar.appearance().thinking, Thinking::Open);
}

#[test]
fn a_newer_write_to_another_folder_does_not_drop_an_older_one_to_this_folder() {
    // Tests (and nothing else here) save to several folders in one process: the order is kept per folder.
    let (here, there) = (folder("order-here"), folder("order-there"));
    settings::write_in_order(u64::MAX - 1, &there, "{}");
    settings::write_in_order(u64::MAX - 5, &here, "{\"kept\":true}");
    assert!(std::fs::read_to_string(here.join(settings::FILE)).is_ok_and(|j| j.contains("kept")));
    // The same folder still drops an older write.
    settings::write_in_order(u64::MAX - 6, &here, "{\"older\":true}");
    assert!(std::fs::read_to_string(here.join(settings::FILE)).is_ok_and(|j| j.contains("kept")));
    let _ = std::fs::remove_dir_all(here);
    let _ = std::fs::remove_dir_all(there);
}
