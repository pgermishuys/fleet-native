//! Ports `NSD/MoreOptions.vue`: the "…" chip at the right end of the strip, and the menu above it with the session's project,
//! title and tags (rows B8.1-B8.10). What the fields hold is `fleet::new_session::more`.
use crate::fleet::new_session::more::{MORE_CHIP, PROJECT_KEY, TAGS_KEY, TITLE_KEY};
use crate::icons::*;
use crate::theme::{RADIUS_BTN, Theme, sans};
use crate::ui::chip::{Chip, pop};
use crate::{State, Tx};
use rune::{Align, Cursor, El, Key, KeyEvent, Side, col, icon, row, text, text_input};
use std::rc::Rc;

const WIDTH: f32 = 300.0;
const SCRATCH: &str = "__scratch__";

/// B8.2: the ellipsis alone, or a layers mark and the project's name before it. The press opens the menu on the first field.
pub fn chip(s: &State, t: &Theme) -> El {
    let disabled = s.new_draft.draft().is_some_and(|d| d.is_starting);
    let name = s.more_project_name();
    let mut chip = Chip::new(MORE_CHIP, name.clone().unwrap_or_default())
        .trailing(&ELLIPSIS)
        .open(s.more_menu_open())
        .disabled(disabled)
        .help(Some(s.more_chip_help()));
    if name.is_some() {
        chip = chip.glyph(&LAYERS);
    }
    chip.view(t, |s: &mut State, tx: &mut Tx| {
        s.open_more_menu();
        tx.focus(if s.more_projects().is_empty() { TITLE_KEY } else { PROJECT_KEY }, false);
    })
    .label(s.more_chip_label())
}

/// `.ns-field__input`: the bordered box a value sits in; `grow` is the focus ring Fleet draws as a border colour.
fn field_box(t: &Theme, inner: El) -> El {
    row()
        .w_full()
        .px(8.0)
        .py(6.0)
        .items_center()
        .radius(RADIUS_BTN - 2.0)
        .border(1.0, t.border)
        .fill(t.wash(3.0))
        .focus_within_ring(1.0, -1.0, t.accent)
        .child(inner)
}

/// `.ns-field`: a label over its box, `6px 8px 8px`, 4px apart.
fn field(t: &Theme, label: &str, input: El) -> El {
    col().w_full().gap(4.0).px(8.0).pt(6.0).pb(8.0).child(text(label).font(sans(11.5, 400.0, 1.3)).color(t.muted)).child(input)
}

fn input(
    s: &State,
    t: &Theme,
    key: &str,
    label: &str,
    placeholder: &str,
    which: fn(&mut State) -> &mut rune::TextEdit,
    edited: fn(&mut State),
) -> El {
    let edit = if key == TITLE_KEY { &s.pickers_ns.more.title } else { &s.pickers_ns.more.tags };
    text_input(edit, sans(13.0, 400.0, 1.3))
        .key(key.to_string())
        .label(label.to_string())
        .placeholder(placeholder.to_string(), t.muted)
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .single_line()
        .w_full()
        .on_edit(which)
        .on_input_change(move |s: &mut State, _: &mut Tx| edited(s))
        .on_input_key(|s: &mut State, ev: &KeyEvent, _: &mut Tx| {
            let enter = ev.key == Key::Enter;
            if enter {
                s.close_more_menu();
            }
            enter
        })
}

/// The Project field's box: the project's name (`Scratch` for none) and a chevron at half strength (B8.6).
fn project_trigger(s: &State, t: &Theme) -> El {
    let current = s.more_project_name().unwrap_or_else(|| "Scratch".into());
    row()
        .key(PROJECT_KEY)
        .role("combobox")
        .label(current.clone())
        .w_full()
        .px(8.0)
        .py(6.0)
        .gap(8.0)
        .items_center()
        .justify_between()
        .radius(RADIUS_BTN - 2.0)
        .border(1.0, t.border)
        .fill(t.wash(3.0))
        .cursor(Cursor::Pointer)
        .focus_ring(1.0, -1.0, t.accent)
        .on_press_tx(|s: &mut State, tx: &mut Tx| {
            s.open_project_select();
            // The list opens on the project in use, which is drawn highlighted as Fleet's Select does.
            let id = s.new_draft.draft().and_then(|d| d.project_id.clone());
            let at = id.and_then(|id| s.more_projects().iter().position(|p| p.id == id)).map_or(0, |i| i + 1);
            tx.focus(format!("new-session-project-select-opt{at}"), false);
        })
        .child(text(current).font(sans(13.0, 400.0, 1.3)).color(t.text).truncate().shrink())
        .child(icon(&CHEVRON_DOWN, 14.0).color(t.muted).opacity(0.5))
}

/// The menu: 6px above the chip, its right edge on the chip's (B8.5). The Project field is there only with a project to pick.
pub fn overlay(s: &State, t: &Theme) -> Vec<El> {
    let mut body = Vec::new();
    let projects = s.more_projects();
    if !projects.is_empty() {
        body.push(field(t, "Project", project_trigger(s, t)));
    }
    body.push(field(
        t,
        "Title",
        field_box(
            t,
            input(s, t, TITLE_KEY, "Title", "Taken from your first message", |s| &mut s.pickers_ns.more.title, |s| s.title_edited()),
        ),
    ));
    body.push(field(
        t,
        "Tags",
        field_box(t, input(s, t, TAGS_KEY, "Tags", "review-requested, deploy", |s| &mut s.pickers_ns.more.tags, |s| s.tags_edited())),
    ));
    let panel =
        pop(t, "new-session-more-pop", WIDTH, body).focus_return(MORE_CHIP.to_string()).modal(true, |s: &mut State| s.close_more_menu());
    let mut layer = vec![
        crate::ui::overlay::scrim("new-session-more-scrim", |s: &mut State| s.close_more_menu()),
        crate::ui::overlay::anchored(panel, MORE_CHIP, Side::Top, Align::End, 6.0),
    ];
    if s.pickers_ns.more.select_open && !projects.is_empty() {
        layer.extend(project_list(s, t, &projects));
    }
    layer
}

/// `.ns-select-content`: Scratch first, then each project, as wide as the field (B8.6).
fn project_list(s: &State, t: &Theme, projects: &[crate::fleet::groups::ProjectInfo]) -> Vec<El> {
    let mut options: Vec<(&str, &str)> = vec![(SCRATCH, "Scratch")];
    options.extend(projects.iter().map(|p| (p.id.as_str(), p.name.as_str())));
    let current = s.new_draft.draft().and_then(|d| d.project_id.clone()).filter(|id| projects.iter().any(|p| &p.id == id));
    let choose: crate::ui::select::Choose = Rc::new(|s: &mut State, v: &str| s.set_project((v != SCRATCH).then(|| v.to_string())));
    let close: Rc<dyn Fn(&mut State)> = Rc::new(|s: &mut State| s.close_project_select());
    crate::ui::select::content_in(
        t,
        "new-session-project-select",
        PROJECT_KEY,
        WIDTH - 10.0 - 16.0,
        &options,
        Some(current.as_deref().unwrap_or(SCRATCH)),
        choose,
        close,
        RADIUS_BTN - 2.0,
    )
}
