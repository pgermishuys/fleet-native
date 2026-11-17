//! Component 16, piece F: the Appearance page. Fleet's `AppearanceSection.test.ts` cases (the first, third and fourth; the
//! second is piece E's) under their names, then the theme cards, the font size and Thinking, which Fleet's tests don't reach.
use crate::conv::gist::Thinking;
use crate::fleet::settings::appearance::{FontSize, ThemeChoice};
use crate::fleet::settings::code_themes::{CODE_THEMES, CodeThemeSelection as Sel, code_backgrounds, contrast_ratio};
use crate::tests::{FRAME, app};
use crate::theme::Theme;
use crate::{Rail, State};
use rune::Runtime;

/// A click, then enough frames for the layout to settle (a font size is a zoom, which moves everything).
fn click(rt: &mut Runtime<State>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no element keyed {key:?}"));
    rt.pointer_move(x, y);
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    for _ in 0..30 {
        rt.frame(FRAME);
    }
}

fn page(dark: bool) -> Runtime<State> {
    let mut rt = app();
    // Tall enough for the whole page, so a click needs no scrolling.
    rt.set_viewport((1280.0, 1500.0), 1.0);
    rt.update(move |s, _| {
        s.dark = Some(dark);
        s.rail = Rail::Settings;
        s.open_settings("appearance", None);
    });
    for _ in 0..30 {
        rt.frame(FRAME);
    }
    rt
}

fn find<'a>(all: &'a [rune::ElementInfo], key: &str) -> &'a rune::ElementInfo {
    all.iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting).unwrap_or_else(|| panic!("no element keyed {key:?}"))
}

fn has(rt: &Runtime<State>, key: &str) -> bool {
    rt.elements().iter().any(|e| e.key.as_deref() == Some(key))
}

/// A picked card or button is the one with a fill (the accent's dim wash or the accent itself).
fn checked(rt: &Runtime<State>, key: &str) -> bool {
    find(&rt.elements(), key).fill.a > 0.0
}

/// Every word drawn inside the element at `key`, in order.
fn words(rt: &Runtime<State>, key: &str) -> String {
    let all = rt.elements();
    let at = all.iter().position(|e| e.key.as_deref() == Some(key)).unwrap_or_else(|| panic!("no element keyed {key:?}"));
    let mut out = String::new();
    let mut stack = vec![at];
    while let Some(i) = stack.pop() {
        if let Some(t) = &all[i].text {
            out.push_str(t);
        }
        let mut kids: Vec<_> = (0..all.len()).filter(|&k| all[k].parent == Some(i)).collect();
        kids.reverse();
        stack.extend(kids);
    }
    out
}

const KEYS: [&str; 7] = ["match", "darcula", "monokai", "github", "one-dark", "solarized", "nord"];

// ── AppearanceSection.test.ts ───────────────────────────────────────────────────

#[test]
fn lists_match_theme_and_the_six_schemes_with_the_current_one_checked() {
    let rt = page(true);
    let cards: Vec<_> = rt
        .elements()
        .iter()
        .filter_map(|e| e.key.clone())
        .filter(|k| k.matches('-').count() <= 3 && KEYS.iter().any(|c| *k == format!("code-theme-{c}")))
        .collect();
    assert_eq!(cards, KEYS.map(|k| format!("code-theme-{k}")));
    for (key, name) in KEYS.iter().zip(["Match theme", "Darcula", "Monokai", "GitHub", "One Dark", "Solarized", "Nord"]) {
        assert!(words(&rt, &format!("code-theme-{key}")).contains(name), "{key}: {}", words(&rt, &format!("code-theme-{key}")));
    }
    assert!(checked(&rt, "code-theme-match"));
    assert!(!checked(&rt, "code-theme-nord"));
}

#[test]
fn each_card_previews_code_in_that_schemes_own_colours() {
    let rt = page(true);
    let all = rt.elements();
    // Darcula's keyword is #cc7832 (rgb(204, 120, 50)); on the dark window it already reads, so the lift leaves it.
    let keyword = all
        .iter()
        .find(|e| e.key.as_deref().is_some_and(|k| k.starts_with("code-theme-darcula-keyword")))
        .expect("the keyword token of the Darcula card");
    assert_eq!(keyword.text_color, Some(rune::color::hex("#cc7832").expect("a colour")));
}

#[test]
fn on_a_light_theme_the_dark_only_schemes_cant_be_picked_and_match_theme_shows_as_in_use() {
    let mut rt = page(true);
    rt.update(|s, _| s.sidebar.update_appearance(|a| a.code_theme = Sel::parse("monokai").expect("a scheme")));
    rt.update(|s, _| s.dark = Some(false));
    rt.frame(FRAME);
    rt.frame(FRAME);
    for id in ["darcula", "monokai", "nord"] {
        let key = format!("code-theme-{id}");
        assert_eq!(find(&rt.elements(), &key).opacity, 0.5, "{id} is faded");
        assert!(words(&rt, &key).contains("Dark themes only"), "{id}: {}", words(&rt, &key));
    }
    assert_eq!(find(&rt.elements(), "code-theme-github").opacity, 1.0);
    assert!(checked(&rt, "code-theme-match"));
    assert!(words(&rt, "code-theme-match").contains("Match theme \u{b7} One Light"), "{}", words(&rt, "code-theme-match"));
    assert!(words(&rt, "code-theme-one-dark").contains("One Light"));
    assert!(!checked(&rt, "code-theme-monokai"));

    click(&mut rt, "code-theme-nord");
    assert_eq!(rt.state.sidebar.appearance().code_theme, Sel::parse("monokai").expect("a scheme"), "a disabled card doesn't pick");
}

#[test]
fn picking_a_card_keeps_it_and_checks_it() {
    let mut rt = page(true);
    click(&mut rt, "code-theme-monokai");
    rt.frame(FRAME);
    assert_eq!(rt.state.sidebar.appearance().code_theme, Sel::parse("monokai").expect("a scheme"));
    assert!(checked(&rt, "code-theme-monokai"));
    assert!(!checked(&rt, "code-theme-match"));
}

// ── Theme, Font, Font size, Thinking ────────────────────────────────────────────

#[test]
fn the_theme_row_says_which_theme_is_in_use_and_checks_the_choice() {
    let mut rt = page(true);
    assert!(words(&rt, "settings-row-theme").contains("Active: Default Dark. System follows your computer."));
    assert!(checked(&rt, "theme-card-system"), "System is the default");
    click(&mut rt, "theme-card-light");
    rt.frame(FRAME);
    assert_eq!(rt.state.sidebar.appearance().theme, ThemeChoice::Light);
    assert_eq!(rt.state.dark, Some(false));
    assert!(checked(&rt, "theme-card-light") && !checked(&rt, "theme-card-system"));
    assert!(words(&rt, "settings-row-theme").contains("Active: Light."));
    click(&mut rt, "theme-card-system");
    assert_eq!(rt.state.dark, None, "System follows the computer again");
}

#[test]
fn the_page_has_fleets_two_groups_and_five_rows_in_order() {
    let rt = page(true);
    for key in ["settings-group-look", "settings-group-conversation"] {
        assert!(has(&rt, key), "{key}");
    }
    let all = rt.elements();
    let rows: Vec<f32> =
        ["theme", "code-theme", "font", "font-size", "thinking"].iter().map(|r| find(&all, &format!("settings-row-{r}")).rect[1]).collect();
    assert!(rows.windows(2).all(|w| w[0] < w[1]), "{rows:?}");
}

#[test]
fn the_font_row_has_inter_alone_and_checked() {
    let rt = page(true);
    assert!(checked(&rt, "font-card-inter"));
    assert!(words(&rt, "font-card-inter").contains("Inter"));
}

#[test]
fn font_size_picks_one_of_four_and_keeps_it() {
    let mut rt = page(true);
    assert!(checked(&rt, "font-size-m"), "M is the default");
    for (key, size) in [("s", FontSize::S), ("l", FontSize::L), ("xl", FontSize::Xl), ("m", FontSize::M)] {
        click(&mut rt, &format!("font-size-{key}"));
        rt.frame(FRAME);
        assert_eq!(rt.state.sidebar.appearance().font_size, size);
        assert!(checked(&rt, &format!("font-size-{key}")));
    }
}

#[test]
fn the_window_follows_the_font_size_as_a_page_zoom() {
    let mut rt = page(true);
    assert_eq!(rt.zoom(), 1.0);
    click(&mut rt, "font-size-xl");
    rt.frame(FRAME);
    assert!((rt.zoom() - 16.0 / 14.0).abs() < 1e-4, "{}", rt.zoom());
    click(&mut rt, "font-size-s");
    rt.frame(FRAME);
    assert!((rt.zoom() - 13.0 / 14.0).abs() < 1e-4, "{}", rt.zoom());
}

#[test]
fn the_thinking_buttons_change_the_choice_and_the_summary_with_it() {
    let mut rt = page(true);

    assert!(checked(&rt, "thinking-folded"));
    assert!(words(&rt, "settings-row-thinking").contains("One line per block, with its gist. Click a line to open it."));
    click(&mut rt, "thinking-open");
    rt.frame(FRAME);
    assert_eq!(rt.state.thinking, Thinking::Open);
    assert!(checked(&rt, "thinking-open") && !checked(&rt, "thinking-folded"));
    assert!(words(&rt, "settings-row-thinking").contains("Every block in full."));
    click(&mut rt, "thinking-hidden");
    rt.frame(FRAME);
    assert_eq!(rt.state.thinking, Thinking::Hidden);
    assert!(words(&rt, "settings-row-thinking").contains("Not shown. The harness and the history still keep it."));
}

#[test]
fn every_row_says_what_fleet_says() {
    let rt = page(true);
    assert!(words(&rt, "settings-row-code-theme").contains(
        "Syntax colours for files and code blocks. Match theme uses your theme's own scheme where it has one, otherwise One Dark or \
         One Light. Darcula, Monokai and Nord are for dark themes."
    ));
    assert!(words(&rt, "settings-row-font").contains("The interface font."));
    assert!(words(&rt, "settings-row-font-size").contains("The base size of the interface."));
}

#[test]
fn the_cards_keep_their_columns_by_the_windows_width() {
    let mut rt = page(true);
    let columns = |rt: &Runtime<State>| {
        let all = rt.elements();
        let first = find(&all, "code-theme-match").rect[1];
        KEYS.iter().filter(|k| (find(&all, &format!("code-theme-{k}")).rect[1] - first).abs() < 1.0).count()
    };
    for (width, want) in [(1280.0, 4), (900.0, 3), (700.0, 2)] {
        rt.update(move |s, _| s.ui.sel.window = (width, 760.0));
        rt.frame(FRAME);
        rt.frame(FRAME);
        assert_eq!(columns(&rt), want, "window {width}");
    }
}

#[test]
fn every_preview_token_is_drawn_in_a_colour_that_reads_on_the_cards_ground() {
    for dark in [true, false] {
        let rt = page(dark);
        let t = Theme::of(dark);
        let backgrounds = code_backgrounds(&t.code_grounds());
        for c in CODE_THEMES.iter().filter(|c| dark || c.light.is_some()) {
            let prefix = format!("code-theme-{}-", c.key);
            let tokens: Vec<_> =
                rt.elements().iter().filter(|e| e.key.as_deref().is_some_and(|k| k.starts_with(&prefix))).cloned().collect();
            // keyword, function, attr, literal, string, number, comment, type, function again: nine pieces of three lines.
            assert_eq!(tokens.len(), 9, "{}", c.key);
            for e in tokens {
                let got = e.text_color.expect("a colour").hex();
                for bg in &backgrounds {
                    assert!(contrast_ratio(&got, bg) >= 4.4, "{} {got} on {bg} (dark {dark})", e.key.unwrap_or_default());
                }
            }
        }
    }
}
