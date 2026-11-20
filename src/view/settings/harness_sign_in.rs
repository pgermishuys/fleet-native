//! Settings → Harnesses → Providers (`HarnessSignInPanel.vue`): the head, the intro or the asking line, the harness's note, the
//! notice, the Signed in group with each sign-in and its buttons, and the search with the providers still to sign in to. The
//! flow a provider's Sign in opens (`HarnessSignInFlow`) is `harness_sign_in_flow.rs`, drawn in [`flow_slot`]. Sizes are the component's own
//! styles, in CSS pixels.
use crate::State;
use crate::fleet::settings::harness_sign_in::{Connection, Provider};
use crate::icons::*;
use crate::theme::{RADIUS_BTN, RADIUS_CARD, Theme, mono, sans};
use crate::ui::confirm::spinner;
use crate::ui::rem;
use crate::ui::text_menu::text_input;
use rune::{Color, Cursor, El, ScrollSpec, col, el, icon, row, text};

/// 22rem, the list's `max-height`.
const LIST_MAX_H: f32 = 308.0;

fn muted(t: &Theme, key: &str, words: &str) -> El {
    row()
        .center_y()
        .gap(6.0)
        .child(text(words.to_string()).key(key.to_string()).font(sans(12.5, 400.0, 1.5)).color(t.muted).wrap().shrink().min_w(0.0))
}

/// `.sign-in-panel__btn`: 1px border, `--radius-btn`, 3px 9px, 12px/500, `--main-bg`; a danger look after the first Sign out press.
fn small_button(t: &Theme, key: &str, words: &str, lead: Option<El>, danger: bool, on: bool) -> El {
    let ink = if danger { t.error } else { t.text };
    let mut b = row()
        .key(key.to_string())
        .center_y()
        .gap(6.0)
        .px(9.0)
        .py(3.0)
        .radius(RADIUS_BTN)
        .border(1.0, if danger { t.error.alpha(0.55) } else { t.border })
        .border_layout()
        .fill(t.main_bg)
        .hover(t.accent.alpha(0.1))
        .cursor(Cursor::Pointer)
        .label(words.to_string())
        .focus_ring(2.0, 0.0, t.accent);
    if let Some(lead) = lead {
        b = b.child(lead);
    }
    let b = b.child(text(words.to_string()).font(sans(12.0, 500.0, 1.5)).color(ink));
    if on { b } else { b.opacity(0.6).disabled(true).cursor(Cursor::Default) }
}

/// `.sign-in-panel__chip`: a pill, `--accent-text` on a tenth of the accent when it is the one in use.
fn chip(t: &Theme, key: &str, words: &str, active: bool) -> El {
    let (edge, fill, ink) =
        if active { (t.accent.alpha(0.35), t.accent.alpha(0.1), t.accent_text) } else { (t.border, Color::TRANSPARENT, t.muted) };
    row()
        .px(7.0)
        .radius(999.0)
        .border(1.0, edge)
        .border_layout()
        .fill(fill)
        .child(text(words.to_string()).key(key.to_string()).font(sans(11.0, 400.0, 1.5)).color(ink))
}

fn head(s: &State, t: &Theme, kind: &str, name: &str, listed: bool, loading: bool) -> El {
    let (kind, name) = (kind.to_string(), name.to_string());
    let mut head = row()
        .center_y()
        .gap(8.0)
        .child(icon(&KEY_ROUND, 14.0).color(t.text))
        .child(text("Providers").font(sans(13.0, 600.0, 1.5)).color(t.text));
    let _ = s;
    if listed {
        let again = if loading { spinner(t.muted) } else { icon(&REFRESH_CW, 12.0).color(t.muted) };
        let mut b = row().key("harness-sign-in-refresh").label("Check the providers again").cursor(Cursor::Pointer).child(again);
        if loading {
            b = b.opacity(0.6).disabled(true).cursor(Cursor::Default);
        } else {
            b = b.on_press(move |s: &mut State| {
                s.load_sign_ins(&kind, &name);
            });
        }
        head = head.child(el().grow(1.0).min_w(0.0)).child(b);
    }
    head
}

fn intro(t: &Theme, kind: &str, name: &str) -> El {
    let (kind, name) = (kind.to_string(), name.to_string());
    let words = format!("Sign in to the providers {name} uses, with a key or in the browser.");
    row()
        .w_full()
        .flex_wrap()
        .center_y()
        .justify_between()
        .gap(8.0)
        .child(text(words).font(sans(12.5, 400.0, 1.5)).color(t.muted).wrap().shrink().min_w(0.0))
        .child(small_button(t, "harness-sign-in-show", "Show providers", None, false, true).on_press(move |s: &mut State| {
            s.load_sign_ins(&kind, &name);
        }))
}

/// `HarnessSignInFlow` under a provider (piece L's `harness_sign_in_flow`).
fn flow_slot(s: &State, t: &Theme, kind: &str, provider: &Provider) -> Option<El> {
    super::harness_sign_in_flow::flow(s, t, kind, provider)
}

fn group_title(t: &Theme, words: &str) -> El {
    text(words.to_uppercase()).font(sans(11.0, 600.0, 1.5).tracking(0.08 * 11.0)).color(t.muted)
}

fn connection_row(s: &State, t: &Theme, kind: &str, p: &Provider, c: &Connection) -> El {
    let panel = s.sign_in_panel(kind);
    let working = panel.and_then(|p| p.working.as_deref());
    let confirming = panel.and_then(|p| p.confirming.as_deref()) == Some(c.id.as_str());
    let mut line = row()
        .key(format!("harness-sign-in-connection-{}", c.id))
        .w_full()
        .flex_wrap()
        .center_y()
        .gap(8.0)
        .child(text(c.label.clone()).font(sans(12.5, 400.0, 1.5)).color(t.text));
    if c.kind == "env" {
        line = line.child(text("from Fleet's environment").font(sans(12.5, 400.0, 1.5)).color(t.muted));
    }
    if c.active {
        line = line.child(chip(t, &format!("harness-sign-in-inuse-{}", c.id), "In use", true));
    }
    let mut actions = row().flex_wrap().gap(6.0);
    let idle = working.is_none();
    if c.is_credential() && !c.active {
        let (k, pn, id, label) = (kind.to_string(), p.name.clone(), c.id.clone(), c.label.clone());
        actions = actions.child(
            small_button(t, &format!("harness-sign-in-use-{}", c.id), "Use this one", None, false, idle)
                .on_press(move |s: &mut State| s.use_sign_in(&k, &pn, &id, &label)),
        );
    }
    if c.is_credential() {
        let (k, pn, id, label) = (kind.to_string(), p.name.clone(), c.id.clone(), c.label.clone());
        let lead = (working == Some(c.id.as_str())).then(|| spinner(t.muted));
        let words = if confirming { "Sign out?" } else { "Sign out" };
        actions = actions.child(
            small_button(t, &format!("harness-sign-out-{}", c.id), words, lead, confirming, idle)
                .on_press(move |s: &mut State| s.sign_out(&k, &pn, &id, &label)),
        );
    }
    line.child(el().grow(1.0).min_w(0.0)).child(actions)
}

/// `.sign-in-panel__provider`: 1px border, 8px radius, 8px 10px, `--card-bg`.
fn card(t: &Theme, key: &str) -> El {
    col().key(key.to_string()).w_full().gap(8.0).px(10.0).py(8.0).radius(8.0).border(1.0, t.border).border_layout().fill(t.card_bg)
}

fn toggle_button(t: &Theme, kind: &str, p: &Provider, key: &str, closed: &str, open: bool) -> El {
    let (k, id) = (kind.to_string(), p.id.clone());
    small_button(t, key, if open { "Close" } else { closed }, None, false, true)
        .role("button")
        .checked(open)
        .on_press(move |s: &mut State| s.toggle_sign_in(&k, &id))
}

fn signed_in_card(s: &State, t: &Theme, kind: &str, p: &Provider, open: bool) -> El {
    let mut head = row().w_full().flex_wrap().center_y().gap(8.0).child(text(p.name.clone()).font(sans(12.5, 500.0, 1.5)).color(t.text));
    if let Some(words) = p.summary() {
        head = head.child(chip(t, &format!("harness-sign-in-summary-{}", p.id), &words, false));
    }
    head = head.child(el().grow(1.0).min_w(0.0)).child(toggle_button(
        t,
        kind,
        p,
        &format!("harness-sign-in-open-{}", p.id),
        "Add a sign-in",
        open,
    ));
    let mut list = col().w_full().gap(6.0);
    for c in &p.connections {
        list = list.child(connection_row(s, t, kind, p, c));
    }
    let mut c = card(t, &format!("harness-sign-in-provider-{}", p.id)).child(head).child(list);
    if open && let Some(flow) = flow_slot(s, t, kind, p) {
        c = c.child(flow);
    }
    c
}

fn other_card(s: &State, t: &Theme, kind: &str, p: &Provider, open: bool) -> El {
    let head = row()
        .w_full()
        .flex_wrap()
        .center_y()
        .gap(8.0)
        .child(text(p.name.clone()).font(sans(12.5, 500.0, 1.5)).color(t.text))
        .child(text(p.id.clone()).font(mono(11.0, 400.0, 1.5)).color(t.muted))
        .child(el().grow(1.0).min_w(0.0))
        .child(toggle_button(t, kind, p, &format!("harness-sign-in-open-{}", p.id), "Sign in", open));
    let mut c = card(t, &format!("harness-sign-in-provider-{}", p.id)).child(head);
    if open && let Some(flow) = flow_slot(s, t, kind, p) {
        c = c.child(flow);
    }
    c
}

/// `.sign-in-panel__search`: a 12px magnifier and the box, in a `--card-bg` field.
fn search(s: &State, t: &Theme, edit: &rune::TextEdit) -> El {
    let _ = s;
    row()
        .w_full()
        .center_y()
        .gap(6.0)
        .px(8.0)
        .py(5.0)
        .radius(6.0)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.card_bg)
        .child(icon(&SEARCH, 12.0).color(t.muted))
        .child(
            text_input(edit, sans(12.5, 400.0, 1.5))
                .key("harness-sign-in-search")
                .single_line()
                .placeholder("Find a provider".to_string(), t.muted)
                .label("Find a provider".to_string())
                .color(t.text)
                .caret(t.text, t.accent.alpha(0.35))
                .grow(1.0)
                .min_w(0.0)
                .on_edit(|s: &mut State| &mut s.settings.harnesses.sign_in.query),
        )
}

fn body(s: &State, t: &Theme, kind: &str, name: &str) -> El {
    let panel = s.sign_in_panel(kind);
    let listed = s.sign_ins(kind);
    let loading = panel.is_some_and(|p| p.loading);
    let requested = panel.is_some_and(|p| p.requested);
    let error = panel.and_then(|p| p.error.as_deref());
    let mut c = col()
        .key(format!("harness-sign-in-{kind}"))
        .w_full()
        .gap(12.0)
        .px(16.0)
        .py(14.0)
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.main_bg)
        .child(head(s, t, kind, name, listed.is_some(), loading));
    if !requested && listed.is_none() {
        c = c.child(intro(t, kind, name));
    } else if loading && listed.is_none() {
        c =
            c.child(row().key("harness-sign-in-asking").center_y().gap(6.0).child(spinner(t.muted)).child(
                text(format!("Asking {name} which providers it can sign in to\u{2026}")).font(sans(12.5, 400.0, 1.5)).color(t.muted),
            ));
    }
    if let Some(list) = listed {
        let open = panel.and_then(|p| p.open.as_deref());
        if let Some(note) = &list.note {
            c = c.child(muted(t, "harness-sign-in-note", note));
        }
        if let Some(words) = panel.and_then(|p| p.notice.as_deref()) {
            c = c.child(
                text(words.to_string())
                    .key("harness-sign-in-notice")
                    .role("status")
                    .font(sans(12.5, 400.0, 1.5))
                    .color(t.accent_text)
                    .wrap(),
            );
        }
        let single = list.single();
        let signed = list.signed_in();
        let mut group = col().w_full().gap(8.0).child(group_title(t, "Signed in"));
        if signed.is_empty() {
            let words = if single { "Not signed in yet. Sign in below." } else { "No providers yet. Find one below to sign in." };
            group = group.child(muted(t, "harness-sign-in-none", words));
        }
        for p in signed {
            group = group.child(signed_in_card(s, t, kind, p, open == Some(p.id.as_str())));
        }
        c = c.child(group);
        let query = panel.map(|p| p.query.text.clone()).unwrap_or_default();
        let others = list.others(&query);
        if !single || !others.is_empty() {
            let mut group = col().w_full().gap(8.0).child(group_title(t, if single { "Sign in" } else { "Sign in to a provider" }));
            if !single && let Some(p) = panel {
                group = group.child(search(s, t, &p.query));
            }
            let mut rows = col().key("harness-sign-in-list").w_full().gap(6.0).pr(2.0);
            let none = others.is_empty();
            for p in others {
                rows = rows.child(other_card(s, t, kind, p, open == Some(p.id.as_str())));
            }
            if none {
                let words = match query.trim() {
                    "" => "Every provider is signed in.".to_string(),
                    q => format!("No provider matches \u{201c}{q}\u{201d}."),
                };
                rows = rows.child(muted(t, "harness-sign-in-empty", &words));
            }
            group = group.child(rows.scroll(ScrollSpec { stick_bottom: false }).max_h(LIST_MAX_H));
            c = c.child(group);
        }
    }
    if let Some(words) = error {
        let mut line = row().role("alert").w_full().flex_wrap().center_y().gap(8.0).child(
            text(words.to_string()).key("harness-sign-in-error").font(sans(12.5, 400.0, 1.5)).color(t.error).wrap().shrink().min_w(0.0),
        );
        if listed.is_none() {
            let (k, n) = (kind.to_string(), name.to_string());
            line = line.child(small_button(t, "harness-sign-in-retry", "Try again", None, false, true).on_press(move |s: &mut State| {
                s.load_sign_ins(&k, &n);
            }));
        }
        c = c.child(line);
    }
    c
}

/// The Providers group (`data-setting-group="providers"`): its heading and the panel.
pub fn section(s: &State, t: &Theme, kind: &str, name: &str) -> El {
    col()
        .key("settings-group-providers")
        .w_full()
        .gap(7.0)
        .child(row().px(3.5).child(text("Providers").font(sans(rem(0.875), 600.0, 1.43)).color(t.text)))
        .child(body(s, t, kind, name))
}
