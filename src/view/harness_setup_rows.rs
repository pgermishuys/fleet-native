//! `HarnessSetupRows.vue` (`components/harness-setup`): the rows of the setup wizard's harnesses step. Each has the harness's icon,
//! name, state chip and one button (Install, Update, Sign in, or Download); under it the install places, the notes, the setup
//! terminal with the found banner, and a failure with the command to copy. The setup terminal's widget and the update strip and
//! sign-in panel (`HarnessUpdateStrip.vue`, `HarnessSignInPanel.vue`) are components 13's and 16's: the words and boxes around
//! them are drawn, their buttons say so. Sizes are the component's own scoped CSS (literal px), as in `harness_setup.rs`.
use crate::fleet::harness_setup_rows::{Action, Choice, rows};
use crate::fleet::new_session::harness_choice::{HarnessEntry, Harnesses};
use crate::icons::{
    CABLE, CHECK, CIRCLE_CHECK, CIRCLE_DASHED, COPY, DOWNLOAD, EXTERNAL_LINK, HEXAGON, INFINITY, KEY_ROUND, SQUARE_TERMINAL,
    TRIANGLE_ALERT, X,
};
use crate::theme::{RADIUS_BTN, RADIUS_CARD, Theme, mono, sans};
use crate::view::folder_menu::spinner;
use crate::{State, Tx};
use rune::{Cursor, El, Icon, col, el, icon, row, text};

/// The container, one row per harness Fleet can set up. With none to list it is the empty 2px box Fleet draws (C13.4).
pub fn list(s: &State, t: &Theme) -> El {
    let harnesses = s.setup_harnesses();
    let mut c = col().key("harness-setup-rows").w_full().border(1.0, t.border).border_layout().radius(RADIUS_CARD).fill(t.card_bg);
    for (i, h) in rows(&harnesses).into_iter().enumerate() {
        if i > 0 {
            c = c.child(el().h(1.0).w_full().fill(t.border));
        }
        c = c.child(harness_row(s, t, &harnesses, h));
    }
    c
}

fn harness_row(s: &State, t: &Theme, all: &Harnesses, h: &HarnessEntry) -> El {
    let setup = &s.harness_setup.rows;
    let mut r = col().key(format!("harness-setup-row-{}", h.kind)).w_full().gap(12.0).px(16.0).py(14.0).child(head(s, t, all, h));
    if !h.choices().is_empty() {
        r = r.child(choices(s, t, h));
    }
    if h.fleet_installs() {
        r = r.child(update_strip(t, h));
        if h.setup_state() == "sign-in-required" {
            r = r.child(sign_in_panel(t, h));
        }
    }
    if !h.notes().is_empty() {
        r = r.child(notes(t, h));
    }
    if setup.is_active(&h.kind) {
        r = r.child(terminal(s, t, all));
    }
    if let Some(f) = setup.failure.as_ref().filter(|f| f.kind == h.kind) {
        r = r.child(failure(s, t, &f.message, &f.command));
    }
    r
}

// ─── the head line ──────────────────────────────────────────────────────────────

fn glyph_for(h: &HarnessEntry) -> &'static Icon {
    match h.setup.icon.as_deref() {
        Some("terminal") => &SQUARE_TERMINAL,
        Some("hexagon") => &HEXAGON,
        Some("infinity") => &INFINITY,
        _ => &CABLE,
    }
}

fn head(s: &State, t: &Theme, all: &Harnesses, h: &HarnessEntry) -> El {
    let tile = el()
        .size(34.0, 34.0)
        .radius(RADIUS_BTN)
        .fill(t.main_bg)
        .border(1.0, t.border)
        .border_layout()
        .center()
        .child(icon(glyph_for(h), 17.0).color(t.text));
    let ready = h.setup_state() == "ready";
    let mut name = row().center_y().gap(6.0).child(text(h.display_name.clone()).font(sans(14.0, 600.0, 1.4)).color(t.text));
    if h.kind == all.default_type() {
        name = name.child(text("DEFAULT").font(sans(10.5, 600.0, 1.4).tracking(0.05 * 10.5)).color(t.accent_text));
    }
    let sub_font = if ready { mono(11.5, 400.0, 1.4) } else { sans(12.5, 400.0, 1.4) };
    let words = col()
        .grow(1.0)
        .min_w(0.0)
        .child(name)
        .child(text(h.subtitle()).key(format!("harness-setup-sub-{}", h.kind)).font(sub_font).color(t.muted).wrap().w_full().mt(2.0));
    let mut actions = row().center_y().gap(8.0).child(chip(t, h.setup_state()));
    if let Some(url) = h.download() {
        let url = url.to_string();
        let b = button(t, h.kind == all.default_type(), Some(&EXTERNAL_LINK), &format!("Download {}", h.display_name));
        actions = actions.child(b.key(format!("harness-setup-download-{}", h.kind)).role("link").on_press_tx(
            move |_: &mut State, tx: &mut Tx| {
                let _ = tx.open_url(&url);
            },
        ));
    } else if let (Some(action), false) = (h.setup_action(), h.fleet_installs()) {
        let primary = h.kind == all.default_type() || action == Action::SignIn;
        let glyph = if action == Action::SignIn { &KEY_ROUND } else { &DOWNLOAD };
        let kind = if action == Action::SignIn { "sign-in" } else { "install" };
        let busy = s.harness_setup.rows.is_active(&h.kind) && s.harness_setup.rows.found(all).is_none();
        let id = h.kind.clone();
        let b = button(t, primary, Some(glyph), &h.action_label()).key(format!("harness-setup-{kind}-{}", h.kind));
        actions = actions.child(if busy {
            b.opacity(0.5).disabled(true).cursor(Cursor::NotAllowed)
        } else {
            b.on_press(move |s: &mut State| s.setup_rows_next(&id))
        });
    }
    // Fleet's head is a wrapping row whose name block is `flex: 1 1 220px`: the chip and button drop to their own line, at the right,
    // when tile + 220 + them don't fit the dialog's row (508px). Rune sizes a wrapping row by its text's longest line, so the same
    // test is made here from the labels' lengths (about 6.6px a character at 12.5px, 5.9 at 11px).
    let chars = |n: usize, per: f32| n as f32 * per;
    let button_w = h
        .download()
        .map(|_| h.display_name.len() + 9)
        .or_else(|| (h.setup_action().is_some() && !h.fleet_installs()).then(|| h.action_label().len()));
    let actions_w =
        14.0 + chars(chip_label(h.setup_state()).len(), 5.9) + 3.5 + 14.0 + button_w.map_or(0.0, |n| 8.0 + 22.0 + 20.0 + chars(n, 6.6));
    let wraps = 34.0 + 12.0 + 220.0 + 12.0 + actions_w > 508.0;
    if wraps {
        col()
            .w_full()
            .gap(12.0)
            .child(row().w_full().center_y().gap(12.0).child(tile).child(words))
            .child(row().w_full().justify_end().child(actions))
    } else {
        row().w_full().center_y().gap(12.0).child(tile).child(words).child(actions)
    }
}

fn chip_label(state: &str) -> &'static str {
    match state {
        "ready" => "Ready",
        "sign-in-required" => "Sign-in needed",
        "update-needed" => "Update needed",
        "not-working" => "Not working",
        _ => "Not installed",
    }
}

/// The state chip: icon and label in the state's colours (`harnessStatusClasses`).
fn chip(t: &Theme, state: &str) -> El {
    let label = chip_label(state);
    let (glyph, edge, fill, ink) = match state {
        "ready" => (&CIRCLE_CHECK, t.running.alpha(0.3), t.running.alpha(0.1), t.running_text),
        "sign-in-required" => (&TRIANGLE_ALERT, t.idle.alpha(0.3), t.idle.alpha(0.1), t.idle_text),
        "update-needed" => (&TRIANGLE_ALERT, t.idle.alpha(0.3), t.idle.alpha(0.1), t.idle_text),
        "not-working" => (&TRIANGLE_ALERT, t.error.alpha(0.3), t.error.alpha(0.1), t.error_text),
        _ => (&CIRCLE_DASHED, t.border, t.main_bg, t.muted),
    };
    row()
        .key("harness-setup-chip")
        .center_y()
        .gap(3.5)
        .px(7.0)
        .py(1.75)
        .radius(999.0)
        .fill(fill)
        .border(1.0, edge)
        .border_layout()
        .child(icon(glyph, 11.0).color(ink))
        .child(text(label).font(sans(11.0, 500.0, 1.45)).color(ink))
}

/// `.harness-setup-rows__btn`, in its two looks. The caller adds the key and what it does.
fn button(t: &Theme, primary: bool, glyph: Option<&'static Icon>, label: &str) -> El {
    let (fill, edge, ink) = if primary { (t.accent, t.accent, t.primary_fg) } else { (t.main_bg, t.border, t.text) };
    let mut b = row()
        .role("button")
        .label(label.to_string())
        .center_y()
        .gap(6.0)
        .px(11.0)
        .py(6.0)
        .radius(RADIUS_BTN)
        .fill(fill)
        .border(1.0, edge)
        .border_layout()
        .cursor(Cursor::Pointer)
        .focus_ring(2.0, 2.0, t.accent);
    // Rune can't change a border on hover (docs/RUNE.md): the plain button gets a faint accent wash instead.
    b = if primary { b.hover(t.accent.alpha(0.9)) } else { b.hover(t.accent.alpha(0.08)) };
    if let Some(g) = glyph {
        b = b.child(icon(g, 14.0).color(ink));
    }
    b.child(text(label.to_string()).font(sans(12.5, 500.0, 1.45)).color(ink))
}

// ─── install places, notes ──────────────────────────────────────────────────────

/// A native radio, 13px, in the accent colour.
fn radio(t: &Theme, on: bool, off: bool) -> El {
    let ink = if on { t.accent } else { t.muted };
    let b = el().size(13.0, 13.0).mt(2.0).border(1.0, ink).border_layout().radius(6.5).opacity(if off { 0.5 } else { 1.0 });
    if on { b.center().child(el().size(5.0, 5.0).radius(2.5).fill(ink)) } else { b }
}

fn choices(s: &State, t: &Theme, h: &HarnessEntry) -> El {
    let rows = &s.harness_setup.rows;
    let picked = rows.picked_choice(h).map(|c| c.id.clone());
    let locked = rows.is_active(&h.kind);
    let mut c = col()
        .key(format!("harness-setup-choices-{}", h.kind))
        .role("radiogroup")
        .label(format!("Where to install {}", h.display_name))
        .w_full()
        .gap(6.0)
        .pl(46.0)
        .child(text("Where to install it").font(sans(12.0, 600.0, 1.4)).color(t.text));
    for choice in h.choices() {
        c = c.child(choice_card(t, h, choice, picked.as_deref() == Some(choice.id.as_str()), locked));
    }
    c
}

fn choice_card(t: &Theme, h: &HarnessEntry, choice: &Choice, on: bool, locked: bool) -> El {
    let mut label = row().flex_wrap().center_y().gap(6.0).child(text(choice.label.clone()).font(sans(12.5, 600.0, 1.4)).color(t.text));
    if choice.recommended {
        label = label.child(
            el().px(7.0)
                .radius(999.0)
                .fill(t.accent.alpha(0.1))
                .border(1.0, t.accent.alpha(0.3))
                .border_layout()
                .child(text("Recommended").font(sans(10.5, 500.0, 1.5)).color(t.accent_text)),
        );
    }
    let (kind, id) = (h.kind.clone(), choice.id.clone());
    let card = row()
        .key(format!("harness-setup-choice-{}-{}", h.kind, choice.id))
        .role("radio")
        .label(choice.label.clone())
        .checked(on)
        .w_full()
        .items_start()
        .gap(10.0)
        .px(11.0)
        .py(9.0)
        .radius(RADIUS_BTN)
        .fill(t.main_bg)
        .border(1.0, if on { t.accent.alpha(0.55) } else { t.border })
        .border_layout()
        .focus_ring(2.0, 2.0, t.accent)
        .child(radio(t, on, locked))
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .gap(2.0)
                .child(label)
                .child(text(choice.description.clone()).font(sans(12.0, 400.0, 1.4)).color(t.muted).wrap().w_full()),
        );
    if locked {
        card
    } else {
        card.hover(t.accent.alpha(0.05)).cursor(Cursor::Pointer).on_press(move |s: &mut State| s.harness_setup.rows.pick(&kind, &id))
    }
}

fn notes(t: &Theme, h: &HarnessEntry) -> El {
    col().key(format!("harness-setup-notes-{}", h.kind)).w_full().gap(3.0).pl(46.0).children(h.notes().iter().map(|n| {
        row()
            .items_start()
            .child(row().w(16.0).h(16.8).justify_end().center_y().pr(5.0).child(el().size(4.0, 4.0).radius(2.0).fill(t.muted)))
            .child(text(n.clone()).font(sans(12.0, 400.0, 1.4)).color(t.muted).wrap().grow(1.0).min_w(0.0))
    }))
}

// ─── Fleet's own download (GitHub Copilot): the strip and sign-in panel are Settings' ────────────────────────

fn hand_off(what: &'static str) -> impl Fn(&mut State, &mut Tx) + 'static {
    move |s: &mut State, tx: &mut Tx| {
        s.toast(format!("{what} comes with Settings, which isn't in the native app yet (component 16)."), None, 4.0, tx)
    }
}

fn update_strip(t: &Theme, h: &HarnessEntry) -> El {
    let working = matches!(h.setup.update_phase.as_deref(), Some("waiting" | "running"));
    let head = if working { format!("Updating {}…", h.display_name) } else { format!("{} isn't installed.", h.display_name) };
    let plan = h.setup.install_plan.clone().unwrap_or_default();
    let mut words = col().grow(1.0).min_w(0.0).gap(2.0).child(text(head).font(sans(13.0, 600.0, 1.4)).color(t.text));
    if !working {
        words = words.child(text(plan).font(sans(13.0, 400.0, 1.4)).color(t.muted).wrap().w_full());
    }
    let mut r = row().w_full().items_center().gap(12.0).child(words);
    r = r.child(if working {
        spinner(t)
    } else {
        button(t, true, Some(&DOWNLOAD), "Install")
            .key(format!("harness-setup-update-install-{}", h.kind))
            .on_press_tx(hand_off("Installing it"))
    });
    el_box(t, 8.0, 14.0, 12.0).key(format!("harness-update-{}", h.kind)).child(r)
}

fn sign_in_panel(t: &Theme, h: &HarnessEntry) -> El {
    let head = row()
        .center_y()
        .gap(8.0)
        .child(icon(&KEY_ROUND, 14.0).color(t.text))
        .child(text("Providers").font(sans(13.0, 600.0, 1.4)).color(t.text));
    let line = text(format!("Sign in to the providers {} uses, with a key or in the browser.", h.display_name))
        .font(sans(12.5, 400.0, 1.4))
        .color(t.muted)
        .wrap()
        .w_full();
    let show = button(t, false, None, "Show providers")
        .key(format!("harness-setup-providers-{}", h.kind))
        .on_press_tx(hand_off("The providers list"));
    el_box(t, RADIUS_CARD, 16.0, 14.0).key(format!("harness-sign-in-{}", h.kind)).gap(8.0).child(head).child(line).child(show)
}

/// A bordered box on the main background (the strip's and the panel's frame).
fn el_box(t: &Theme, radius: f32, px: f32, py: f32) -> El {
    col().w_full().px(px).py(py).radius(radius).fill(t.main_bg).border(1.0, t.border).border_layout()
}

// ─── the setup terminal, the found banner, a failure ───────────────────────────────────────────────────────

fn terminal(s: &State, t: &Theme, all: &Harnesses) -> El {
    let rows = &s.harness_setup.rows;
    let Some(active) = rows.active.as_ref() else { return el() };
    let name = all.all.iter().find(|h| h.kind == active.kind).map(|h| h.display_name.as_str()).unwrap_or_default();
    let verb = if active.action == Action::SignIn { "Sign in to" } else { "Install" };
    let bar = row()
        .w_full()
        .justify_between()
        .center_y()
        .gap(8.0)
        .px(10.0)
        .py(6.0)
        .child(text(format!("{verb} {name} · home folder")).font(sans(11.5, 400.0, 1.4)).color(t.muted))
        .child(
            row()
                .key("harness-setup-terminal-close")
                .role("button")
                .label("Close terminal")
                .padding(2.0)
                .radius(4.0)
                .hover_group()
                .cursor(Cursor::Pointer)
                .focus_ring(2.0, 0.0, t.accent)
                .child(icon(&X, 13.0).color(t.muted).group_color(t.text))
                .on_press(|s: &mut State| s.close_setup_terminal()),
        );
    // The shell itself is component 13's widget: the body shows what would be typed into it.
    let body = if active.terminal_id.is_none() {
        row()
            .key("harness-setup-starting")
            .center_y()
            .gap(8.0)
            .padding(12.0)
            .child(spinner(t))
            .child(text("Starting a terminal…").font(sans(12.5, 400.0, 1.4)).color(t.muted))
    } else {
        col()
            .key("harness-setup-terminal-body")
            .w_full()
            .items_start()
            .gap(10.0)
            .padding(12.0)
            .child(text(format!("$ {}", active.command)).font(mono(12.0, 400.0, 1.5)).color(t.text).wrap().w_full())
            .child(
                text("The live terminal comes with the Terminal component. Copy the command to run it in your own terminal.")
                    .font(sans(12.0, 400.0, 1.4))
                    .color(t.muted)
                    .wrap()
                    .w_full(),
            )
            .child(copy_button(s, t, &active.command))
    };
    let hint = text("Check the command, then press Enter to run it. Fleet won't run it for you.")
        .font(sans(12.0, 400.0, 1.4))
        .color(t.muted)
        .wrap()
        .w_full()
        .px(12.0)
        .py(8.0);
    let boxed = col()
        .key("harness-setup-terminal")
        .w_full()
        .radius(RADIUS_BTN)
        .fill(t.main_bg)
        .border(1.0, t.border)
        .border_layout()
        .child(bar)
        .child(el().h(1.0).w_full().fill(t.border))
        .child(el().w_full().h(220.0).child(body))
        .child(el().h(1.0).w_full().fill(t.border))
        .child(hint);
    let mut c = col().w_full().gap(10.0).child(boxed);
    if let Some(found) = rows.found(all) {
        c = c.child(
            row()
                .key("harness-setup-found")
                .role("status")
                .label(found.clone())
                .w_full()
                .center_y()
                .gap(8.0)
                .px(10.0)
                .py(8.0)
                .radius(RADIUS_BTN)
                .fill(t.running.alpha(0.1))
                .border(1.0, t.running.alpha(0.3))
                .border_layout()
                .child(icon(&CIRCLE_CHECK, 14.0).color(t.running))
                .child(text(found).font(sans(12.5, 400.0, 1.4)).color(t.text).wrap().grow(1.0).min_w(0.0)),
        );
    }
    c
}

/// `Copy`, which turns into `Copied` for 1.5 s. The flag is one for all rows, as in Fleet.
fn copy_button(s: &State, t: &Theme, command: &str) -> El {
    let copied = s.harness_setup.rows.copied_until.is_some();
    let command = command.to_string();
    button(t, false, Some(if copied { &CHECK } else { &COPY }), if copied { "Copied" } else { "Copy" })
        .key("harness-setup-copy")
        .on_press_tx(move |s: &mut State, tx: &mut Tx| s.copy_setup_command(&command, tx))
}

fn failure(s: &State, t: &Theme, message: &str, command: &str) -> El {
    col()
        .key("harness-setup-failure")
        .role("alert")
        .label(format!("{message} Run this in your own terminal instead, then check again:"))
        .w_full()
        .gap(8.0)
        .px(12.0)
        .py(10.0)
        .radius(RADIUS_BTN)
        .fill(t.error.alpha(0.08))
        .border(1.0, t.error.alpha(0.3))
        .border_layout()
        .child(
            text(format!("{message} Run this in your own terminal instead, then check again:"))
                .font(sans(12.5, 400.0, 1.4))
                .color(t.text)
                .wrap()
                .w_full(),
        )
        .child(row().w_full().flex_wrap().center_y().gap(8.0).child(code(t, command)).child(copy_button(s, t, command)))
}

fn code(t: &Theme, command: &str) -> El {
    row()
        .min_w(0.0)
        .max_w(458.0)
        .px(8.0)
        .py(5.0)
        .radius(6.0)
        .fill(t.card_bg)
        .border(1.0, t.border)
        .border_layout()
        .child(text(command.to_string()).key("harness-setup-failure-command").font(mono(12.0, 400.0, 1.5)).color(t.text).wrap().w_full())
}
