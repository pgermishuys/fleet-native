//! Component 13, piece A, the parts that need no server: `terminal-theme.test.ts` under its names, the screen read back row by
//! row from recorded streams, the fit, the keys, a query answered, and an unchanged row handed back as the same `Rc`.
use crate::term::fit::{fit, fit_box};
use crate::term::keys::encode;
use crate::term::palette::{Palette, is_light, mix, terminal_theme, with_alpha};
use crate::term::screen::Screen;
use crate::term::{Terminal, text_of};
use crate::theme::Theme;
use rune::{Key, KeyEvent, Mods};
use std::collections::HashMap;
use std::rc::Rc;

fn reader(values: &[(&str, &str)]) -> impl Fn(&str) -> String {
    let map: HashMap<String, String> = values.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    move |name| map.get(name).cloned().unwrap_or_default()
}

// ---- terminal-theme.test.ts -------------------------------------------------------------------------------------------------

// terminalTheme: "takes text, selection and status colours from the Fleet theme"
#[test]
fn takes_text_selection_and_status_colours_from_the_fleet_theme() {
    let theme = terminal_theme(&reader(&[
        ("--panel-bg", "#141418"),
        ("--text", "#e8e8ec"),
        ("--accent-dim", "rgba(99, 102, 241, 0.15)"),
        ("--error", "#ef4444"),
        ("--running", "#22c55e"),
        ("--idle", "#f59e0b"),
        ("--complete", "#3b82f6"),
        ("--queued", "#8b5cf6"),
    ]));
    assert_eq!(theme.foreground, "#e8e8ec");
    assert_eq!(theme.cursor, "#e8e8ec");
    assert_eq!(theme.cursor_accent, "#141418");
    // 28% of the accent over the panel, opaque.
    assert_eq!(Some(theme.selection_background.clone()), mix("#141418", "#6366f1", 0.28));
    assert!(theme.selection_background.starts_with('#') && theme.selection_background.len() == 7);
    assert_eq!(theme.ansi[1..6], ["#ef4444", "#22c55e", "#f59e0b", "#3b82f6", "#8b5cf6"]);
    assert_eq!(theme.scrollbar_slider_background, "rgba(232, 232, 236, 0.16)");
}

// terminalTheme: "uses a light palette for the colours a light theme doesn't set"
#[test]
fn uses_a_light_palette_for_the_colours_a_light_theme_doesnt_set() {
    let light = terminal_theme(&reader(&[("--panel-bg", "#FFFFFF"), ("--text", "#1A1918")]));
    let dark = terminal_theme(&reader(&[("--panel-bg", "#0A0A0A"), ("--text", "#FAFAFA")]));
    assert_eq!(light.ansi[6], "#0e7490");
    assert_eq!(dark.ansi[6], "#67e8f9");
}

// terminalTheme: "falls back when the theme sets nothing" (the native theme always sets them: these are the palette's fallbacks)
#[test]
fn falls_back_when_the_theme_sets_nothing() {
    let theme = terminal_theme(&|_| String::new());
    assert!(!theme.foreground.is_empty());
    assert!(!theme.ansi[1].is_empty());
}

// isLight
#[test]
fn is_light_reads_the_colours_fleet_writes() {
    for (color, expected) in [
        ("#ffffff", true),
        ("#F3F2EF", true),
        ("#fff", true),
        ("rgb(250, 250, 250)", true),
        ("#141418", false),
        ("#1E293B", false),
        ("rgba(0, 0, 0, 0.5)", false),
        ("not a colour", false),
    ] {
        assert_eq!(is_light(color), expected, "{color}");
    }
}

// mix: "paints one colour over another, opaque"
#[test]
fn mix_paints_one_colour_over_another_opaque() {
    assert_eq!(mix("#000000", "#ffffff", 0.5).as_deref(), Some("#808080"));
    assert_eq!(mix("#ffffff", "#5b6ec7", 0.28).as_deref(), Some("#d1d6ef"));
}

// mix: "gives up on a colour it can't read"
#[test]
fn mix_gives_up_on_a_colour_it_cant_read() {
    assert_eq!(mix("var(--x)", "#fff", 0.5), None);
}

// withAlpha: "turns a hex colour into rgba"
#[test]
fn with_alpha_turns_a_hex_colour_into_rgba() {
    assert_eq!(with_alpha("#1A1918", 0.28), "rgba(26, 25, 24, 0.28)");
}

// withAlpha: "leaves a colour it can't read alone"
#[test]
fn with_alpha_leaves_a_colour_it_cant_read_alone() {
    assert_eq!(with_alpha("hsl(0 0% 50%)", 0.5), "hsl(0 0% 50%)");
}

#[test]
fn both_native_themes_get_the_palette_their_brightness_picks() {
    // The dark theme's blue is its own `--complete`; the colours it doesn't set come from the dark palette (cyan).
    let (dark, light) = (Palette::new(&Theme::of(true)), Palette::new(&Theme::of(false)));
    assert_ne!(dark.ansi(6), light.ansi(6));
    assert_eq!(dark.ansi(1), Theme::of(true).error);
    assert_eq!(light.ansi(2), Theme::of(false).running);
}

// ---- the screen -------------------------------------------------------------------------------------------------------------

fn stream(name: &str) -> &'static [u8] {
    match name {
        "git-log" => include_bytes!("../tests/fixtures/terminal/git-log.bin"),
        "ls" => include_bytes!("../tests/fixtures/terminal/ls.bin"),
        "unicode" => include_bytes!("../tests/fixtures/terminal/unicode.bin"),
        "vim" => include_bytes!("../tests/fixtures/terminal/vim.bin"),
        _ => panic!("no stream {name}"),
    }
}

#[test]
fn a_recorded_stream_reads_back_row_by_row() {
    let t = Terminal::new(100, 30);
    t.feed(stream("git-log"));
    let rows = text_of(&t);
    assert!(rows.iter().any(|r| r.contains("Release 0.2") && r.contains("Ada Fenwick")), "{rows:#?}");
    assert!(rows.iter().any(|r| r.contains("A wick shorter than 2 mm goes out")), "{rows:#?}");
    // The graph's drawing characters came through whole.
    assert!(rows.iter().any(|r| r.starts_with("*   ") || r.starts_with("* ")), "{rows:#?}");
}

#[test]
fn vim_draws_its_screen_in_the_alternate_buffer() {
    let t = Terminal::new(100, 30);
    t.feed(stream("vim"));
    let rows = text_of(&t);
    assert!(rows.iter().any(|r| r.contains("pub struct Wick")), "{rows:#?}");
    assert!(rows.iter().any(|r| r.contains("README.md")), "{rows:#?}");
}

#[test]
fn wide_and_combining_characters_keep_their_columns() {
    let t = Terminal::new(100, 30);
    t.feed(stream("unicode"));
    let rows = text_of(&t);
    assert!(rows.iter().any(|r| r.contains("日本語のテキスト")), "{rows:#?}");
    assert!(rows.iter().any(|r| r.contains("e\u{301} a\u{308} n\u{303}")), "{rows:#?}");
}

fn read(t: &Terminal, screen: &mut Screen) {
    let pal = Palette::new(&Theme::of(true));
    t.read(|term| screen.read(term, &pal, Theme::of(true).terminal_bg(), 1));
}

// Row 13.17: a row that didn't change is the same `Rc`, so Rune keeps its shaped line.
#[test]
fn an_unchanged_row_is_handed_back_as_the_same_rc() {
    let t = Terminal::new(40, 6);
    t.feed(b"first\r\nsecond\r\nthird");
    let mut screen = Screen::default();
    read(&t, &mut screen);
    assert_eq!(screen.built, 6, "the first read builds every row");
    let before: Vec<_> = screen.rows.clone();
    t.feed(b" line");
    read(&t, &mut screen);
    assert_eq!(screen.built, 1, "only the row that changed is built");
    assert!(Rc::ptr_eq(&before[0], &screen.rows[0]) && Rc::ptr_eq(&before[1], &screen.rows[1]));
    assert!(!Rc::ptr_eq(&before[2], &screen.rows[2]));
    // Another theme builds them all again.
    let pal = Palette::new(&Theme::of(false));
    t.read(|term| screen.read(term, &pal, Theme::of(false).terminal_bg(), 2));
    assert_eq!(screen.built, 6);
}

#[test]
fn the_cursor_follows_the_output() {
    let t = Terminal::new(40, 6);
    t.feed(b"demo $ ");
    let mut screen = Screen::default();
    read(&t, &mut screen);
    let c = screen.cursor.as_ref().expect("a cursor");
    assert_eq!((c.col, c.line), (7, 0));
    t.feed(b"\x1b[?25l");
    read(&t, &mut screen);
    assert!(screen.cursor.is_none(), "DECTCEM hides it");
}

// ---- the fit (13.2) ---------------------------------------------------------------------------------------------------------

#[test]
fn a_508_by_225_box_is_70_by_11() {
    assert_eq!(fit(508.0, 225.0), (70, 11));
    // The same box with its padding (12 left, 4 right, 6 top, 4 bottom) put back.
    assert_eq!(fit_box(508.0 + 16.0, 225.0 + 10.0), (70, 11));
}

#[test]
fn a_box_with_no_room_is_at_least_two_columns_and_a_row() {
    assert_eq!(fit(0.0, 0.0), (2, 1));
}

// ---- the keys (13.61) -------------------------------------------------------------------------------------------------------

fn key(key: Key, mods: Mods, app_cursor: bool) -> Vec<u8> {
    encode(&KeyEvent { key, mods }, app_cursor).expect("bytes")
}

#[test]
fn the_basic_keys_make_the_bytes_xterm_makes() {
    let none = Mods::default();
    assert_eq!(key(Key::Text("a".into()), none, false), b"a");
    assert_eq!(key(Key::Enter, none, false), b"\r");
    assert_eq!(key(Key::Backspace, none, false), [0x7f]);
    assert_eq!(key(Key::Tab, none, false), b"\t");
    assert_eq!(key(Key::Escape, none, false), [0x1b]);
    // Ctrl+C: off a Mac the shell sees it as `Command('c')`, on a Mac as Ctrl with text.
    let ctrl = Mods { ctrl: true, cmd: !cfg!(target_os = "macos"), ..none };
    let c = if cfg!(target_os = "macos") { Key::Text("c".into()) } else { Key::Command('c') };
    assert_eq!(key(c, ctrl, false), [3]);
}

#[test]
fn arrows_are_csi_in_normal_mode_and_ss3_in_application_mode() {
    let none = Mods::default();
    assert_eq!(key(Key::Up, none, false), b"\x1b[A");
    assert_eq!(key(Key::Up, none, true), b"\x1bOA");
}

#[test]
fn a_program_that_asks_for_application_cursor_keys_is_told_so() {
    let t = Terminal::new(40, 6);
    assert!(!t.mode().contains(crate::term::APP_CURSOR));
    t.feed(b"\x1b[?1h");
    assert!(t.mode().contains(crate::term::APP_CURSOR));
}

// ---- parser answers (13.16) -------------------------------------------------------------------------------------------------

#[test]
fn a_cursor_position_query_is_answered_with_the_cursor_position() {
    let t = Terminal::new(40, 6);
    t.feed(b"one\r\ntwo\x1b[6n");
    assert_eq!(t.take_answers(), b"\x1b[2;4R");
    assert!(t.take_answers().is_empty(), "taken once");
}

#[test]
fn a_device_attributes_query_is_answered() {
    let t = Terminal::new(40, 6);
    t.feed(b"\x1b[c");
    assert!(t.take_answers().starts_with(b"\x1b[?"));
}

#[test]
fn a_reset_starts_from_a_blank_screen_of_the_same_size() {
    let t = Terminal::new(40, 6);
    t.feed(b"old");
    t.reset();
    assert_eq!(t.size(), (40, 6));
    assert!(text_of(&t).iter().all(String::is_empty));
}

#[test]
fn a_long_line_wraps_and_reflows_on_resize() {
    let t = Terminal::new(10, 4);
    t.feed(b"0123456789abcdef");
    assert_eq!(text_of(&t)[..2], ["0123456789", "abcdef"]);
    t.resize(20, 4);
    assert_eq!(text_of(&t)[0], "0123456789abcdef");
}
