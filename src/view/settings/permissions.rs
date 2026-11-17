//! Settings → What agents can do (`PermissionsSection.vue`): the intro, and one bordered card of rows. This file draws the first
//! two, Edit files and Run commands & go online, with the chips of the harnesses that differ from the default; the rows after
//! them (messages, machines, nobody watching), the Browser group and the fine print are pieces N and O. Sizes are Fleet's scoped
//! CSS px (`.reach-*`).
use super::kit;
use crate::State;
use crate::fleet::settings::permissions::{Reach, Row, default_tag, reach_of};
use crate::icons::CHEVRON_DOWN;
use crate::theme::{RADIUS_CARD, Theme, sans};
use crate::ui::{Open, rem, select};
use rune::{Cursor, El, Span, col, icon, rich, row, text};
use std::rc::Rc;

const REACH: [(&str, &str); 2] = [("allowed", "Allowed"), ("ask", "Ask")];

/// `.reach-needs`: Fleet draws the waiting glyph and "Needs you" in one coloured inline run. A wrapped line can't hold an
/// element, so the glyph is the diamond character in the same colour.
fn intro(t: &Theme) -> El {
    let (a, b, c) = (
        "For every harness on this machine. When an agent needs your OK, the session shows ",
        "\u{25c6} Needs you",
        " and the ask waits in the conversation. A change reaches running sessions on their next tool call.",
    );
    let words = format!("{a}{b}{c}");
    let span = Span::new(a.len(), a.len() + b.len()).color(t.status_waiting_text).weight(600.0);
    row()
        .key("permissions-intro")
        .w_full()
        .px(3.5)
        .child(rich(words, vec![span]).font(sans(rem(0.875), 400.0, 1.43)).color(t.muted).wrap().shrink().min_w(0.0))
}

fn small(words: &str, size: f32, weight: f32, ink: rune::Color) -> El {
    text(words.to_string()).font(sans(size, weight, 1.33)).color(ink)
}

/// `.reach-chip`: a pill with the harness's name and a trigger that opens its value list.
fn chip(s: &State, t: &Theme, which: Row, kind: &str, name: &str, help: String) -> El {
    let key = format!("reach-exception-{}-{kind}", which.id());
    let value = s.permission_reach(kind, which);
    let open_kind = kind.to_string();
    let trigger = row()
        .key(format!("{key}-select"))
        .role("combobox")
        .label(format!("{name}: {}", which.action()))
        .center_y()
        .gap(3.0)
        .px(2.0)
        .py(1.0)
        .radius(999.0)
        .cursor(Cursor::Pointer)
        .hover(t.text.alpha(0.07))
        .focusable(true)
        .focus_ring(2.0, 1.0, t.accent)
        .child(small(value.name(), 12.0, 600.0, t.text))
        .child(icon(&CHEVRON_DOWN, 12.0).color(t.muted))
        .on_press(move |s: &mut State| {
            s.settings.permissions.chip = Some((which, open_kind.clone()));
            s.ui.open = Some(Open::Select("permission-chip"));
        });
    row()
        .key(key)
        .center_y()
        .gap(6.0)
        .pl(10.0)
        .pr(4.0)
        .py(2.0)
        .radius(999.0)
        .fill(t.main_bg)
        .border(1.0, t.border)
        .border_layout()
        .help(help)
        .child(small(name, 12.0, 500.0, t.text))
        .child(trigger)
}

/// `.reach-chip--fixed`: a harness Fleet can't hand a level says so, on both rows.
fn fixed_chip(t: &Theme, which: Row, kind: &str, name: &str) -> El {
    row()
        .key(format!("reach-unleveled-{}-{kind}", which.id()))
        .center_y()
        .gap(4.0)
        .px(10.0)
        .py(2.0)
        .radius(999.0)
        .fill(t.main_bg)
        .border(1.0, t.border)
        .border_layout()
        .border_dash(3.0, 3.0)
        .child(small(name, 12.0, 500.0, t.text))
        .child(small("its own settings: Fleet can't set it", 12.0, 400.0, t.muted))
}

/// The Browser chip of Run commands: what the browser is doing here, and a way to the Browser group.
fn browser_chip(s: &State, t: &Theme) -> El {
    row()
        .key("reach-browser-chip")
        .role("button")
        .label("Browser")
        .center_y()
        .gap(6.0)
        .px(10.0)
        .py(2.0)
        .radius(999.0)
        .fill(t.main_bg)
        .border(1.0, t.border)
        .border_layout()
        .cursor(Cursor::Pointer)
        .hover(t.mix(t.accent, t.main_bg, 0.12))
        .focusable(true)
        .focus_ring(2.0, 1.0, t.accent)
        .child(small("Browser", 12.0, 500.0, t.text))
        .child(small(&s.permission_browser_chip(), 12.0, 400.0, t.muted))
        .on_press(|s: &mut State| s.open_settings("permissions", Some("browser")))
}

/// `.reach-add`: the "+ exception" select, there while some harness follows the default on the row.
fn add(t: &Theme, which: Row) -> El {
    row()
        .key(format!("reach-add-{}", which.id()))
        .role("combobox")
        .label(format!("Add an exception to {}", which.short()))
        .center_y()
        .gap(4.0)
        .pl(10.0)
        .pr(6.0)
        .py(2.0)
        .radius(999.0)
        .border(1.0, t.border)
        .border_layout()
        .border_dash(3.0, 3.0)
        .cursor(Cursor::Pointer)
        .hover(t.text.alpha(0.07))
        .focusable(true)
        .focus_ring(2.0, 1.0, t.accent)
        .child(small("+ exception", 12.0, 400.0, t.muted))
        .child(icon(&CHEVRON_DOWN, 12.0).color(t.muted))
        .on_press(move |s: &mut State| {
            s.settings.permissions.add = Some(which);
            s.ui.open = Some(Open::Select("permission-add"));
        })
}

/// One row of the card: the title with its default tag, the summary, the default's segments, and the exceptions under them.
pub fn reach_row(s: &State, t: &Theme, which: Row, summary: &str, tag: String) -> El {
    let value = s.permission_default_reach(which);
    let id = which.id();
    let segments = kit::segments(
        t,
        &format!("reach-{id}"),
        which.label(),
        &REACH,
        value.value(),
        false,
        Rc::new(move |s: &mut State, v: &str| {
            if let Some(v) = Reach::parse(v) {
                s.set_permission_default(which, v);
            }
        }),
    );
    let title = row()
        .wrap()
        .items_end()
        .gap(8.0)
        .child(text(which.title().to_string()).font(sans(13.5, 600.0, 1.5)).color(t.text))
        .child(small(&tag, 11.0, 400.0, t.muted).key(format!("reach-tag-{id}")));
    let head =
        row()
            .w_full()
            .items_start()
            .justify_between()
            .gap(16.0)
            .child(col().shrink().min_w(0.0).child(title).child(
                row().mt(2.0).child(text(summary.to_string()).font(sans(12.0, 400.0, 1.45)).color(t.muted).wrap().shrink().min_w(0.0)),
            ))
            .child(segments);
    let mut chips = row().wrap().center_y().gap(6.0);
    for h in s.permission_exceptions(which) {
        chips = chips.child(chip(s, t, which, &h.kind, &h.display_name, s.permission_handed(&h)));
    }
    for h in s.permission_unleveled() {
        chips = chips.child(fixed_chip(t, which, &h.kind, &h.display_name));
    }
    if which == Row::Commands {
        chips = chips.child(browser_chip(s, t));
    }
    if !s.permission_candidates(which).is_empty() {
        chips = chips.child(add(t, which));
    }
    let mut r = col().key(format!("reach-row-{id}")).w_full().gap(8.0).px(16.0).py(14.0).child(head).child(chips);
    if which == Row::Commands {
        r = r.child(row().child(
            text("A command can change files too, so these go together: Edit files Ask makes commands ask, and Run commands Allowed allows edits.")
                .font(sans(11.5, 400.0, 1.45))
                .color(t.muted)
                .wrap()
                .shrink()
                .min_w(0.0),
        ));
    }
    r
}

/// `.reach-rows`: the card, its rows divided by a rule.
fn card(s: &State, t: &Theme) -> El {
    let default = reach_of(crate::asks::rules::PermissionLevel::All);
    let tag = |which: Row| {
        let now = s.permission_default_reach(which);
        default_tag(now == default.on(which), default.on(which).name())
    };
    let rows = vec![
        reach_row(s, t, Row::Edits, "Change and create files in the session's folder or worktree.", tag(Row::Edits)),
        reach_row(s, t, Row::Commands, "The shell, web fetch, the browser and other tools.", tag(Row::Commands)),
    ];
    let mut body = col()
        .key("reach-rows")
        .w_full()
        .radius(RADIUS_CARD)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.card_bg)
        .clip_rounded(RADIUS_CARD);
    for (at, r) in rows.into_iter().enumerate() {
        body = body.child(if at == 0 { r } else { col().w_full().border_sides([1.0, 0.0, 0.0, 0.0], t.border).child(r) });
    }
    body
}

pub fn content(s: &State, t: &Theme) -> El {
    col()
        .key("permissions-section")
        .w_full()
        .gap(21.0)
        .child(super::super::settings::page_header(s, t, &crate::fleet::settings_nav::Page::Permissions))
        .child(intro(t))
        .child(card(s, t))
}

/// The open value list of an exception chip: Allowed and Ask, the one the default has marked "(as the default)".
pub fn chip_list(s: &State, t: &Theme) -> Vec<El> {
    let Some((which, kind)) = s.settings.permissions.chip.clone() else { return Vec::new() };
    let base = s.permission_default_reach(which);
    let words: Vec<(String, String)> = [Reach::Allowed, Reach::Ask]
        .iter()
        .map(|r| (r.value().to_string(), format!("{}{}", r.name(), if *r == base { " (as the default)" } else { "" })))
        .collect();
    let options: Vec<(&str, &str)> = words.iter().map(|(v, l)| (v.as_str(), l.as_str())).collect();
    let current = s.permission_reach(&kind, which);
    let anchor = format!("reach-exception-{}-{kind}-select", which.id());
    let choose: select::Choose = Rc::new(move |s: &mut State, v: &str| {
        if let Some(v) = Reach::parse(v) {
            s.set_permission_harness(&kind, which, v);
        }
    });
    select::content(t, "reach-chip-list", &anchor, 160.0, &options, Some(current.value()), choose)
}

/// The "+ exception" list: one entry per harness that follows the default, worded with the opposite value.
pub fn add_list(s: &State, t: &Theme) -> Vec<El> {
    let Some(which) = s.settings.permissions.add else { return Vec::new() };
    let opposite = s.permission_default_reach(which).opposite();
    let words: Vec<(String, String)> =
        s.permission_candidates(which).iter().map(|h| (h.kind.clone(), format!("{} \u{b7} {}", h.display_name, opposite.name()))).collect();
    let options: Vec<(&str, &str)> = words.iter().map(|(v, l)| (v.as_str(), l.as_str())).collect();
    let choose: select::Choose = Rc::new(move |s: &mut State, kind: &str| s.set_permission_harness(kind, which, opposite));
    select::content(t, "reach-add-list", &format!("reach-add-{}", which.id()), 160.0, &options, None, choose)
}
