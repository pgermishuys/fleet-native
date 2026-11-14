//! The kit on one screen: every control in its states, and the buttons that open each overlay.
//! Scenes open it with an overlay already showing (`kit-menu`, `kit-dialog`, ...).
use super::button::{Size, Variant, button, disabled, icon_button};
use super::controls::*;
use super::dialog::{Dialog, SheetSide, footer_button};
use super::select::{self, Choose};
use super::{Open, rem};
use crate::State;
use crate::icons::*;
use crate::theme::{RADIUS_PANEL, Theme, sans};
use rune::{Align, El, ScrollSpec, Side, col, el, row, text};
use std::rc::Rc;

const MODELS: [(&str, &str); 4] = [("sonnet", "Sonnet 5.5"), ("opus", "Opus 5.5"), ("haiku", "Haiku 5.5"), ("auto", "Default")];

fn section(t: &Theme, title: &str, body: El) -> El {
    col().w_full().gap(8.0).child(text(title.to_string()).font(sans(11.0, 600.0, 1.2).tracking(0.55)).color(t.muted)).child(body)
}

fn flow() -> El {
    row().w_full().gap(10.0).center_y().wrap()
}

pub fn view(s: &State, t: &Theme) -> El {
    let u = &s.ui;
    let variants = [
        ("Default", Variant::Default),
        ("Destructive", Variant::Destructive),
        ("Outline", Variant::Outline),
        ("Secondary", Variant::Secondary),
        ("Ghost", Variant::Ghost),
        ("Link", Variant::Link),
    ];
    let mut buttons = flow();
    for (name, v) in variants {
        buttons = buttons.child(button(t, v, Size::Default, None, name).key(format!("g-btn-{name}")));
    }
    let sizes = flow()
        .child(button(t, Variant::Default, Size::Sm, None, "Small"))
        .child(button(t, Variant::Default, Size::Default, Some(&PLUS), "With icon"))
        .child(button(t, Variant::Default, Size::Lg, None, "Large"))
        .child(disabled(button(t, Variant::Default, Size::Default, None, "Disabled")))
        .child(icon_button(t, Variant::Outline, Size::Icon, &SETTINGS, "Settings"))
        .child(icon_button(t, Variant::ToolbarIcon, Size::Toolbar, &ELLIPSIS, "More"))
        .child(icon_button(t, Variant::ToolbarIconDanger, Size::Toolbar, &X, "Remove"))
        .child(button(t, Variant::Filter { active: true }, Size::Sm, None, "Active"))
        .child(button(t, Variant::Filter { active: false }, Size::Sm, None, "Filter"));
    let badges = flow()
        .child(badge(t, BadgeVariant::Default, "Default"))
        .child(badge(t, BadgeVariant::Secondary, "Secondary"))
        .child(badge(t, BadgeVariant::Destructive, "Destructive"))
        .child(badge(t, BadgeVariant::Outline, "Outline"))
        .child(badge(t, BadgeVariant::Status(StatusInk::Running), "Running"))
        .child(badge(t, BadgeVariant::Status(StatusInk::Idle), "Needs input"));
    let tab = |v, key: &str, label, on| tab(t, v, key, label, on);
    let seg = tabs(
        t,
        TabsVariant::Default,
        ["Account", "Password", "Team"]
            .iter()
            .enumerate()
            .map(|(i, l)| tab(TabsVariant::Default, &format!("g-tab{i}"), l, u.g_tab == i).on_press(move |s: &mut State| s.ui.g_tab = i))
            .collect(),
    );
    let und = tabs(
        t,
        TabsVariant::Underline,
        ["Overview", "Daily", "Models"]
            .iter()
            .enumerate()
            .map(|(i, l)| {
                tab(TabsVariant::Underline, &format!("g-utab{i}"), l, u.g_tab2 == i).on_press(move |s: &mut State| s.ui.g_tab2 = i)
            })
            .collect(),
    );
    let fields = row()
        .w_full()
        .gap(12.0)
        .items_start()
        .child(
            col()
                .w(rem(15.0))
                .gap(8.0)
                .child(input(t, &u.g_input, "Search sessions").key("g-input").on_edit(|s: &mut State| &mut s.ui.g_input))
                .child(textarea(t, &u.g_area, "Write a note…", 4).key("g-area").on_edit(|s: &mut State| &mut s.ui.g_area)),
        )
        .child(
            col()
                .gap(10.0)
                .child(
                    row()
                        .gap(8.0)
                        .center_y()
                        .child(switch(t, u.g_switch).key("g-switch").on_press(|s: &mut State| s.ui.g_switch = !s.ui.g_switch))
                        .child(text("Notifications").font(sans(rem(0.875), 400.0, 1.3)).color(t.text)),
                )
                .child(progress(t, 62.0, rem(12.0)))
                .child(
                    row()
                        .gap(8.0)
                        .center_y()
                        .child(avatar(t, "P"))
                        .child(separator_v(t).h(rem(1.5)))
                        .child(text("Orchard").font(sans(rem(0.875), 500.0, 1.3)).color(t.text)),
                ),
        );
    let card_demo = card(t)
        .w(rem(20.0))
        .child(card_header("Orchard", Some("Five sessions, one running."), t))
        .child(card_content().child(text("Cards are 10px radius with 21px of padding.").font(sans(rem(0.875), 400.0, 1.4)).color(t.text)))
        .child(card_footer().gap(8.0).child(button(t, Variant::Outline, Size::Sm, None, "Open")).child(button(
            t,
            Variant::Default,
            Size::Sm,
            None,
            "Run",
        )));
    let sel = u.g_select.as_deref();
    let label = MODELS.iter().find(|(v, _)| Some(*v) == sel).map(|(_, l)| *l).unwrap_or("Choose a model");
    let openers = flow()
        .child(
            select::trigger(t, label, sel.is_none(), false, rem(12.0))
                .key("g-select")
                .on_press(|s: &mut State| s.ui.open = Some(Open::Select("g"))),
        )
        .child(
            button(t, Variant::Outline, Size::Default, Some(&ELLIPSIS), "Menu")
                .key("g-menu")
                .on_press(|s: &mut State| s.ui.open = Some(Open::Menu("gallery"))),
        )
        .child(
            button(t, Variant::Outline, Size::Default, None, "Popover")
                .key("g-popover")
                .on_press(|s: &mut State| s.ui.open = Some(Open::Popover("g"))),
        )
        .child(
            button(t, Variant::Outline, Size::Default, Some(&FOLDER), "Folder…")
                .key("g-dir")
                .on_press(|s: &mut State| s.ui.open = Some(Open::Directory)),
        )
        .child(
            button(t, Variant::Outline, Size::Default, None, "Dialog")
                .key("g-dialog")
                .on_press(|s: &mut State| s.ui.open = Some(Open::Dialog)),
        )
        .child(
            button(t, Variant::Destructive, Size::Default, None, "Alert")
                .key("g-alert")
                .on_press(|s: &mut State| s.ui.open = Some(Open::Alert)),
        )
        .child(
            button(t, Variant::Outline, Size::Default, None, "Sheet")
                .key("g-sheet")
                .on_press(|s: &mut State| s.ui.open = Some(Open::Sheet(SheetSide::Right))),
        )
        .child(
            button(t, Variant::Outline, Size::Default, Some(&SEARCH), "Palette")
                .key("g-palette")
                .on_press_tx(|s: &mut State, tx: &mut rune::Tx| super::palette::open(s, tx)),
        )
        .child(button(t, Variant::Outline, Size::Default, None, "Toast").key("g-toast").on_press_tx(|s: &mut State, tx: &mut rune::Tx| {
            s.toast("Archived “Tidy the release notes”.".into(), Some("Undo".into()), 6.0, tx)
        }))
        .child(icon_button(t, Variant::Outline, Size::Icon, &CIRCLE_QUESTION_MARK, "Hover for a tooltip"));
    let body = col()
        .w_full()
        .padding(24.0)
        .gap(22.0)
        .child(text("Kit").font(sans(rem(1.25), 600.0, 1.2)).color(t.text))
        .child(section(t, "OVERLAYS", openers))
        .child(section(t, "BUTTONS", buttons))
        .child(section(t, "SIZES AND ICON BUTTONS", sizes))
        .child(section(t, "BADGES", badges))
        .child(section(t, "TABS", col().gap(14.0).items_start().child(seg.w(rem(24.0))).child(und.w(rem(24.0)))))
        .child(section(t, "FIELDS, SWITCH, PROGRESS, AVATAR", fields))
        .child(section(t, "CARD", card_demo));
    col()
        .key("gallery")
        .grow(1.0)
        .h_full()
        .fill(t.panel_bg)
        .border(1.0, t.border)
        .radius(RADIUS_PANEL)
        .clip_rounded(RADIUS_PANEL)
        .child(body.scroll(ScrollSpec { stick_bottom: false }).key("gallery-scroll").grow(1.0))
}

pub fn select_content(s: &State, t: &Theme, _name: &str) -> Vec<El> {
    let choose: Choose = Rc::new(|s: &mut State, v: &str| s.ui.g_select = Some(v.to_string()));
    select::content(t, "g-select-pop", "g-select", rem(12.0), &MODELS, s.ui.g_select.as_deref(), choose)
}

pub fn popover_content(_s: &State, t: &Theme, _name: &str) -> Vec<El> {
    let body = col()
        .gap(8.0)
        .child(text("Dimensions").font(sans(rem(0.875), 500.0, 1.3)).color(t.text))
        .child(text("Set the dimensions for the layer.").font(sans(rem(0.875), 400.0, 1.4)).color(t.muted));
    super::dialog::popover(t, "g-pop", "g-popover", Side::Bottom, Align::Center, rem(18.0), rem(1.0), body)
}

pub fn dialog(_s: &State, t: &Theme) -> Vec<El> {
    super::dialog::dialog(
        t,
        Dialog {
            key: "g-dlg",
            title: "Rename session".into(),
            description: Some("Give this session a name you will recognise in the list.".into()),
            body: vec![el().w_full().child(text("Name").font(sans(rem(0.875), 500.0, 1.3)).color(t.text))],
            footer: vec![
                button(t, Variant::Outline, Size::Default, None, "Cancel").key("g-dlg-cancel").on_press(|s: &mut State| s.ui.open = None),
                button(t, Variant::Default, Size::Default, None, "Save").key("g-dlg-save").on_press(|s: &mut State| s.ui.open = None),
            ],
            close_button: true,
            click_outside: true,
        },
    )
}

pub fn alert(_s: &State, t: &Theme) -> Vec<El> {
    super::dialog::dialog(
        t,
        Dialog {
            key: "g-alert",
            title: "Delete this session?".into(),
            description: Some("This removes its conversation for good. It can't be undone.".into()),
            body: vec![],
            footer: vec![
                footer_button(t, "Cancel", false).key("g-alert-cancel").on_press(|s: &mut State| s.ui.open = None),
                footer_button(t, "Delete", true).key("g-alert-action").on_press(|s: &mut State| s.ui.open = None),
            ],
            close_button: false,
            click_outside: false,
        },
    )
}

pub fn sheet(_s: &State, t: &Theme, side: SheetSide) -> Vec<El> {
    super::dialog::sheet(
        t,
        "g-sheet-pop",
        side,
        "Session details",
        "Everything Fleet knows about this session.",
        vec![text("Orchard · cache-on-disk").font(sans(rem(0.875), 400.0, 1.4)).color(t.text)],
    )
}
