//! The Install panel of Settings → Harnesses (`HarnessInstallPanel.vue`): how a harness is installed here, the places it can go,
//! the folders, what to know, the command to copy, and the sign-in command. Its own scoped CSS, so sizes are literal px.
//! Also the boxes and buttons the update strip and the path field share.
use crate::State;
use crate::Tx;
use crate::fleet::harness_setup_rows::{Choice, Folder};
use crate::fleet::new_session::harness_choice::HarnessEntry;
use crate::fleet::settings::harness_install::{self as rules, Panel};
use crate::icons::{CHECK, COPY, DOWNLOAD, EXTERNAL_LINK};
use crate::theme::{RADIUS_BTN, RADIUS_CARD, Theme, mono, sans};
use rune::{Cursor, El, Icon, col, el, icon, row, text};

/// `.harness-install__btn` (padding 5/10, 12px/500) and `.harness-update__btn` (6/11, 12.5px) in the card colour; Rune has no
/// hover border, so the faint accent wash stands in (docs/RUNE.md).
pub fn button(t: &Theme, glyph: Option<&'static Icon>, label: &str, roomy: bool) -> El {
    let (px, py, size) = if roomy { (11.0, 6.0, 12.5) } else { (10.0, 5.0, 12.0) };
    let mut b = row()
        .role("button")
        .label(label.to_string())
        .center_y()
        .gap(6.0)
        .px(px)
        .py(py)
        .radius(RADIUS_BTN)
        .fill(t.card_bg)
        .border(1.0, t.border)
        .border_layout()
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 2.0, t.accent)
        .hover(t.accent.alpha(0.08));
    if let Some(g) = glyph {
        b = b.child(icon(g, 14.0).color(t.text));
    }
    b.child(text(label.to_string()).font(sans(size, 500.0, 1.45)).color(t.text))
}

/// `code`: a command in the mono face, in a box (`1px --border`, radius 6, padding 5/8, `--card-bg`), wrapping anywhere.
pub fn code(t: &Theme, key: &str, command: &str) -> El {
    row()
        .shrink()
        .min_w(0.0)
        .px(8.0)
        .py(5.0)
        .radius(6.0)
        .fill(t.card_bg)
        .border(1.0, t.border)
        .border_layout()
        .child(text(command.to_string()).key(key.to_string()).font(mono(11.5, 400.0, 1.5)).color(t.text).wrap().shrink().min_w(0.0))
}

/// The command and its Copy button, which says Copied for 1.5 s.
pub fn copy_row(s: &State, t: &Theme, key: &str, command: &str) -> El {
    let copied = s.harness_copied(command);
    let owned = command.to_string();
    row().w_full().flex_wrap().center_y().gap(8.0).child(code(t, key, command)).child(
        button(t, Some(if copied { &CHECK } else { &COPY }), if copied { "Copied" } else { "Copy" }, false)
            .key(format!("{key}-copy"))
            .on_press_tx(move |s: &mut State, tx: &mut Tx| s.copy_harness_command(&owned, tx)),
    )
}

fn muted(t: &Theme, words: impl Into<String>) -> El {
    text(words.into()).font(sans(12.5, 400.0, 1.5)).color(t.muted).wrap().w_full()
}

fn chip(t: &Theme, key: &str, words: &str) -> El {
    row()
        .px(8.0)
        .py(1.0)
        .radius(999.0)
        .fill(t.accent.alpha(0.1))
        .border(1.0, t.accent.alpha(0.3))
        .border_layout()
        .child(text(words.to_string()).key(key.to_string()).font(sans(11.0, 500.0, 1.45)).color(t.accent_text))
}

/// `dl`: `fit-content(15em) 1fr`. Rune has no grid, so the label column is as wide as the longest label, at most 15em.
fn folders(t: &Theme, list: &[Folder]) -> El {
    let longest = list.iter().map(|f| f.label.chars().count()).max().unwrap_or(0) as f32;
    let width = (longest * 6.7).min(12.5 * 15.0);
    col().w_full().gap(4.0).children(list.iter().map(|f| {
        row()
            .w_full()
            .items_start()
            .gap(14.0)
            .child(text(f.label.clone()).font(sans(12.5, 400.0, 1.5)).color(t.muted).w(width).wrap())
            .child(text(f.path.clone()).font(mono(11.5, 400.0, 1.5)).color(t.text).wrap().grow(1.0).min_w(0.0))
    }))
}

fn choice_card(s: &State, t: &Theme, c: &Choice) -> El {
    let mut label = row().flex_wrap().center_y().gap(8.0).child(text(c.label.clone()).font(sans(13.0, 600.0, 1.45)).color(t.text));
    if c.recommended {
        label = label.child(chip(t, &format!("harness-install-recommended-{}", c.id), "Recommended"));
    }
    let mut card = col()
        .key(format!("harness-install-choice-{}", c.id))
        .w_full()
        .gap(8.0)
        .px(12.0)
        .py(10.0)
        .radius(RADIUS_BTN)
        .fill(t.card_bg)
        .border(1.0, t.border)
        .border_layout()
        .child(label)
        .child(muted(t, c.description.clone()));
    if !c.folders.is_empty() {
        card = card.child(folders(t, &c.folders));
    }
    card.child(copy_row(s, t, &format!("harness-install-choice-command-{}", c.id), &c.command))
}

fn notes(t: &Theme, list: &[String]) -> El {
    col().w_full().gap(4.0).children(list.iter().map(|n| {
        row()
            .items_start()
            .child(row().w(18.0).h(18.75).center_y().child(el().size(4.0, 4.0).radius(2.0).fill(t.muted)))
            .child(text(n.clone()).font(sans(12.5, 400.0, 1.5)).color(t.muted).wrap().grow(1.0).min_w(0.0))
    }))
}

fn terminal_line(s: &State, t: &Theme, lead: &str, key: &str, command: &str) -> El {
    col().w_full().gap(6.0).child(muted(t, lead)).child(copy_row(s, t, key, command))
}

fn body(s: &State, t: &Theme, h: &HarnessEntry, p: &Panel) -> El {
    let mut head = row().w_full().flex_wrap().center_y().gap(8.0).child(text("Install").font(sans(13.0, 600.0, 1.45)).color(t.text));
    if let Some(mode) = &p.mode {
        head = head.child(chip(t, "harness-install-mode", mode));
    }
    if let Some(v) = &p.version {
        head = head.child(text(v.clone()).key("harness-install-version").font(mono(11.5, 400.0, 1.5)).color(t.muted));
    }
    let mut c = col()
        .key("harness-install")
        .w_full()
        .gap(12.0)
        .px(16.0)
        .py(14.0)
        .radius(RADIUS_CARD)
        .fill(t.main_bg)
        .border(1.0, t.border)
        .border_layout()
        .child(head);
    if !p.choices.is_empty() {
        let mut list = col()
            .key("harness-install-choices")
            .w_full()
            .gap(10.0)
            .child(muted(t, "It can go in one of two places. Pick one when you install it:"));
        for choice in &p.choices {
            list = list.child(choice_card(s, t, choice));
        }
        c = c.child(list);
    } else if !p.folders.is_empty() {
        c = c.child(folders(t, &p.folders));
    }
    if !p.notes.is_empty() {
        c = c.child(notes(t, &p.notes));
    }
    if let Some(cmd) = &p.install_command {
        c = c.child(terminal_line(s, t, "Install it in a terminal:", "harness-install-command", cmd));
    }
    if p.set_up {
        c = c.child(
            row().child(
                button(t, Some(&DOWNLOAD), &format!("Install {} here", h.display_name), false)
                    .key("harness-install-set-up")
                    .on_press(|s: &mut State| s.open_harness_setup()),
            ),
        );
    }
    if let Some(cmd) = &p.sign_in {
        c = c.child(terminal_line(s, t, p.sign_in_lead, "harness-install-sign-in", cmd));
    }
    if let Some(url) = &p.download {
        let url = url.clone();
        c = c.child(
            row().child(
                row()
                    .key("harness-install-download")
                    .role("link")
                    .label(format!("Download {}", h.display_name))
                    .center_y()
                    .gap(4.0)
                    .cursor(Cursor::Pointer)
                    .focusable(true)
                    .focus_ring(2.0, 2.0, t.accent)
                    .child(text(format!("Download {}", h.display_name)).font(sans(12.5, 500.0, 1.45)).color(t.accent_text))
                    .child(icon(&EXTERNAL_LINK, 12.0).color(t.accent_text))
                    .on_press_tx(move |_: &mut State, tx: &mut Tx| {
                        let _ = tx.open_url(&url);
                    }),
            ),
        );
    }
    c
}

/// The panel for a harness that describes its install; none for the rest.
pub fn panel(s: &State, t: &Theme, h: &HarnessEntry) -> Option<El> {
    let live = !s.settings.choice.is_other(&s.machines);
    let p = rules::panel(h, live, rules::offers_sign_in(h))?;
    Some(body(s, t, h, &p))
}
