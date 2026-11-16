//! The usage popover the chip opens (ports the popover half of `UsageLimitsPopover.vue`; docs/parity/09-app-frame.md, Part 6,
//! piece J): per provider its windows with a bar, the pace tick and note, credits, and the states that have no bars
//! (Allow, pay-per-token, signed out, unsupported, unreadable), grouped by machine when two or more answer.
use super::message::now_ms;
use super::usage_chip::{TRIGGER, bar, ink};
use crate::State;
use crate::conv::time::local_offset_secs;
use crate::fleet::usage::{ProviderState, ProviderUsage, Window, usage_tone, window_percent};
use crate::fleet::usage_chip::read_line;
use crate::fleet::usage_rules::{
    MachineUsageGroup, UsageRow, ai_credits_line, credits_text, pace_note, provider_window_label, provider_windows, reset_label,
    tick_percent,
};
use crate::theme::{RADIUS_PANEL, Theme, sans};
use crate::ui::Open;
use crate::ui::overlay::{anchored, scrim};
use rune::{Align, Cursor, El, ScrollSpec, Shadow, Side, col, el, row, text};

const NAME: &str = "usage";
const WIDTH: f32 = 380.0;
/// `max-height: min(560px, …available height)`.
const MAX_H: f32 = 560.0;
const BAR_W: f32 = 90.0;

fn is_open(s: &State) -> bool {
    s.ui.open == Some(Open::Popover(NAME))
}

fn close(s: &mut State) {
    s.ui.open = None;
}

impl State {
    /// Opens the popover, or closes it. Reading while it is open follows `ui.open` (`keep_usage_watch`).
    pub fn toggle_usage_popover(&mut self) {
        self.ui.open = if is_open(self) { None } else { Some(Open::Popover(NAME)) };
    }

    /// `onAllow`: Allow one provider's sign-in to be read.
    pub fn allow_usage(&mut self, provider: &str) {
        self.set_usage_allowed(provider, true);
    }
}

/// `.usage-pop__note`: 12 px muted.
fn note(t: &Theme, key: String, words: String) -> El {
    text(words).key(key).font(sans(12.0, 400.0, 1.4)).color(t.muted).wrap().w_full().mt(2.0).mb(6.0)
}

/// `.usage-pop__credits`.
fn credits_line(t: &Theme, key: String, words: String, title: Option<&str>) -> El {
    let line = text(words).key(key).font(sans(12.0, 400.0, 1.4)).color(t.text).wrap().w_full().mt(4.0);
    match title {
        Some(title) => line.help(title.to_string()),
        None => line,
    }
}

/// A window's row: label, bar with the pace tick, percent, reset; the pace note under it when ahead of pace.
fn window_row(t: &Theme, id: &str, w: &Window, now: i64, offset: i64) -> El {
    let key = format!("usage-window-{id}-{}", w.window);
    let tone_ink = match usage_tone(w) {
        crate::fleet::usage::Tone::Quiet => t.accent_text,
        tone => ink(t, tone),
    };
    let percent = window_percent(w);
    let mut track = bar(tone_ink, BAR_W, 6.0, percent).key(format!("{key}-bar")).role("progressbar").label(format!("{percent}"));
    if let Some(at) = tick_percent(w, now) {
        let tick = el()
            .key(format!("{key}-tick"))
            .absolute(BAR_W * at as f32 / 100.0 - 1.0, 0.0)
            .w(2.0)
            .h(6.0)
            .fill(t.usage_tick())
            .box_shadow(Shadow { x: 0.0, y: 0.0, blur: 0.0, spread: 1.0, color: t.usage_tick_ring(), inset: false })
            .help(format!("An even share of this window by now: {at}%"));
        // The bar clips its fill to its round ends; the tick sits over it, so it is laid in a frame the bar doesn't clip.
        track = el().w(BAR_W).h(6.0).child(track).child(tick);
    }
    let figures = row()
        .w_full()
        .center_y()
        .gap(8.0)
        .child(
            text(provider_window_label(&w.window))
                .key(format!("{key}-label"))
                .font(sans(12.0, 400.0, 1.4))
                .color(t.text)
                .wrap()
                .grow(1.0)
                .min_w(70.0),
        )
        .child(track)
        .child(row().w(36.0).justify_end().child(
            text(format!("{percent}%")).key(format!("{key}-percent")).font(sans(12.0, 400.0, 1.4).features("'tnum' 1")).color(tone_ink),
        ))
        .child(
            row().w(112.0).justify_end().child(
                text(reset_label(w.resets_at, now, offset).unwrap_or_default())
                    .key(format!("{key}-reset"))
                    .font(sans(12.0, 400.0, 1.4).features("'tnum' 1"))
                    .color(t.muted),
            ),
        );
    let mut r = col().key(key.clone()).w_full().items_stretch().gap(8.0).py(3.0).child(figures);
    if let Some(p) = pace_note(w, now) {
        r = r.child(text(p.text).key(format!("{key}-pace")).font(sans(11.0, 400.0, 1.4)).color(t.status_waiting_text).wrap().w_full());
    }
    r
}

fn provider(s: &State, t: &Theme, g: &MachineUsageGroup, row_: &UsageRow, first: bool, now: i64, offset: i64) -> El {
    let p: &ProviderUsage = &row_.provider;
    let id = format!("{}-{}", p.provider, g.key);
    let mut head = row()
        .w_full()
        .justify_between()
        .gap(8.0)
        .mb(2.0)
        .child(text(p.name.clone()).key(format!("usage-name-{id}")).font(sans(12.0, 600.0, 1.4)).color(t.text));
    if let Some(account) = &p.account {
        head = head.child(text(account.clone()).key(format!("usage-account-{id}")).font(sans(12.0, 400.0, 1.4)).color(t.muted));
    }
    let mut c = col().key(format!("usage-provider-{id}")).w_full().items_stretch().child(head);
    if !first {
        c = c.mt(12.0).pt(12.0).border_sides([1.0, 0.0, 0.0, 0.0], t.border);
    }
    if !row_.also_on.is_empty() {
        c = c.child(note(t, format!("usage-also-on-{id}"), format!("also on {}", row_.also_on.join(", "))));
    }
    if let Some(read) = read_line(p, offset) {
        c = c.child(note(t, format!("usage-read-{id}"), read));
    }
    let source_in = p.source.as_ref().map(|x| format!(" in {x}")).unwrap_or_default();
    match p.state {
        ProviderState::Ok => {
            for w in provider_windows(p, now) {
                c = c.child(window_row(t, &id, &w, now, offset));
            }
            if let Some(line) = credits_text(p) {
                c = c.child(credits_line(t, format!("usage-credits-{id}"), format!("Credits: {line}"), None));
            }
            if let Some(line) = ai_credits_line(p) {
                c = c.child(credits_line(t, format!("usage-ai-credits-{id}"), line, Some(AI_TITLE)));
            }
        }
        ProviderState::NotAllowed => {
            c = c.child(note(
                t,
                format!("usage-ask-{id}"),
                format!(
                    "Fleet found a sign-in{source_in}. Allow it to ask {n} how much you've used? It reads the sign-in read-only, never saves it, and sends it nowhere but {n}.",
                    n = p.name
                ),
            ));
            if g.key != s.usage_live_key() {
                c = c.child(note(t, format!("usage-elsewhere-{id}"), format!("Open {} to allow it there.", g.name)));
            } else {
                c = c.child(row().child(allow_button(s, t, p)));
            }
        }
        ProviderState::NoWindow => {
            c = c.child(note(t, format!("usage-note-{id}"), "Pay-per-token: no limits to show.".into()));
            if let Some(line) = ai_credits_line(p) {
                c = c.child(credits_line(t, format!("usage-ai-credits-{id}"), line, Some(AI_TITLE)));
            }
        }
        ProviderState::SignedOut => c = c.child(note(t, format!("usage-note-{id}"), format!("Sign in again{source_in}."))),
        ProviderState::Unsupported => c = c.child(note(t, format!("usage-note-{id}"), "Fleet can't read this sign-in yet.".into())),
        ProviderState::Unreadable => c = c.child(note(t, format!("usage-note-{id}"), "Can't read this right now.".into())),
    }
    c
}

const AI_TITLE: &str = "GitHub's count; it can run a few minutes behind.";

/// `.usage-pop__allow`: accent fill, padding 3 12, radius 6; disabled while its call runs.
fn allow_button(s: &State, t: &Theme, p: &ProviderUsage) -> El {
    let busy = s.pickers.usage.allowing.as_deref() == Some(p.provider.as_str());
    let name = p.provider.clone();
    let b = row()
        .key(format!("usage-allow-{}", p.provider))
        .role("button")
        .label("Allow")
        .px(12.0)
        .py(3.0)
        .radius(6.0)
        .fill(t.accent)
        .child(text("Allow").font(sans(12.0, 500.0, 1.4)).color(t.primary_fg));
    if busy { b.opacity(0.6).disabled(true) } else { b.cursor(Cursor::Pointer).on_press(move |s: &mut State| s.allow_usage(&name)) }
}

/// The list: a heading per machine with two or more, each machine's providers, or the empty note.
fn body(s: &State, t: &Theme, now: i64) -> El {
    let offset = local_offset_secs();
    let groups = s.usage_groups();
    let several = groups.len() >= 2;
    let mut list = col().key("usage-list").w_full().items_stretch();
    for (i, g) in groups.iter().enumerate() {
        if several {
            let mut h = text(g.name.to_uppercase())
                .key(format!("usage-machine-{}", g.key))
                .font(sans(10.0, 600.0, 1.4).tracking(0.06 * 10.0))
                .color(t.muted)
                .mb(6.0);
            if i > 0 {
                h = h.mt(14.0);
            }
            list = list.child(h);
            if g.rows.is_empty() {
                list = list.child(note(t, format!("usage-none-{}", g.key), "No provider sign-ins found.".into()));
            }
        }
        for (r, row_) in g.rows.iter().enumerate() {
            list = list.child(provider(s, t, g, row_, r == 0, now, offset));
        }
    }
    if !several && s.pickers.usage.limits.providers.is_empty() {
        list = list.child(note(t, "usage-none".into(), "No provider sign-ins found.".into()));
    }
    list
}

/// The scrim and the card (`side="top" align="end" :side-offset="8"`).
pub fn overlay(s: &State, t: &Theme) -> Vec<El> {
    let (w, h) = s.ui.sel.window;
    let width = if w > 0.0 { WIDTH.min(w - 16.0) } else { WIDTH };
    let h = if h > 0.0 { h } else { 760.0 };
    let now = now_ms(s);
    let card = col()
        .key("usage-limits-popover")
        .role("dialog")
        .label("Usage limits")
        .w(width)
        .items_stretch()
        .fill(t.card_bg)
        .border(1.0, t.border)
        .border_layout()
        .radius(RADIUS_PANEL)
        .clip_rounded(RADIUS_PANEL)
        .child(
            col()
                .w_full()
                .max_h(MAX_H.min(h - 16.0) - 2.0)
                .min_h(0.0)
                .items_stretch()
                .px(14.0)
                .py(12.0)
                .child(body(s, t, now))
                .scroll(ScrollSpec { stick_bottom: false }),
        );
    let card =
        t.menu_shadows().into_iter().fold(card, |e, shadow| e.box_shadow(shadow)).focus_return(TRIGGER.to_string()).modal(true, close);
    vec![scrim("usage-limits-scrim", close), anchored(card, TRIGGER, Side::Top, Align::End, 8.0)]
}
