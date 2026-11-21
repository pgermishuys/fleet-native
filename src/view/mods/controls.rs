//! The controls a mod draws: `components/mods/ModButton.vue`, `ModInput.vue`, `ModSelect.vue` (component 22, piece C). Each sends
//! an action up through `State::mods_ui` (`fleet::mods::controls`), which logs it and hands it to the site's `on_action`.
//! Colours come from the theme; a role or a tone is looked up by `leaves`.
use super::leaves::{flag, mod_icon, str_prop};
use crate::State;
use crate::fleet::mods::controls::ModControls;
use crate::theme::{RADIUS_BTN, Theme, sans};
use crate::ui::select;
use crate::ui::text_menu::text_input;
use crate::ui::{Open, button};
use rune::{Color, Cursor, El, Tx, col, icon, row, text};
use serde_json::{Map, Value};
use std::rc::Rc;

/// The list a Select opens (`Open::Select(OPEN)`).
pub const OPEN: &str = "mod-select";

fn handle(handles: &Map<String, Value>, name: &str) -> Option<String> {
    handles.get(name).and_then(Value::as_str).map(str::to_string)
}

/// `ModButton`: 26 px, `0 10px`, 12 px 500, gap 5, icon 12. `primary` is the accent with `--primary-foreground`, `danger` a red
/// wash, `quiet` (the default) the muted text alone. Disabled is half strength and does nothing. The label gives way with an
/// ellipsis (`shrinks`); a press never reaches the row the button sits in.
pub fn button(t: &Theme, key: &str, props: &Map<String, Value>, handles: &Map<String, Value>, shrinks: bool) -> El {
    let (fill, ink, hover_fill, hover_ink) = match str_prop(props, "tone") {
        Some("primary") => (t.accent, t.primary_fg, t.mix(t.accent, Color::BLACK, 0.88), t.primary_fg),
        Some("danger") => (t.error.alpha(0.16), t.error_text, t.error.alpha(0.24), t.error_text),
        _ => (Color::TRANSPARENT, t.muted, t.wash(6.0), t.text),
    };
    let label = str_prop(props, "label").unwrap_or_default().to_string();
    let off = flag(props, "disabled");
    let mut body = row()
        .key(key.to_string())
        .role("button")
        .label(label.clone())
        .h(26.0)
        .px(10.0)
        .gap(5.0)
        .center_y()
        .radius(RADIUS_BTN - 1.0)
        .fill(fill)
        .min_w(0.0)
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 1.0, t.accent);
    if shrinks {
        body = body.shrink();
    }
    if !off {
        body = body.hover(hover_fill).hover_group();
    }
    if let Some(glyph) = str_prop(props, "icon").and_then(mod_icon) {
        body = body.child(icon(glyph, 12.0).color(ink).group_color(hover_ink));
    }
    body = body.child(text(label).font(sans(12.0, 500.0, 1.3)).color(ink).group_color(hover_ink).truncate().shrink().min_w(0.0));
    match (off, handle(handles, "onPress")) {
        (true, _) => button::disabled(body),
        (false, Some(h)) => body.on_press(move |s: &mut State| s.mods_ui.press(&h)),
        (false, None) => body,
    }
}

/// A 12 px muted label over its control, 4 px apart (`.mod-input__label`, `.mod-select__label`).
fn labelled(t: &Theme, props: &Map<String, Value>, field: El) -> El {
    let mut c = col().gap(4.0).min_w(0.0);
    if let Some(label) = str_prop(props, "label").filter(|l| !l.is_empty()) {
        c = c.child(text(label.to_string()).font(sans(12.0, 400.0, 1.3)).color(t.muted));
    }
    c.child(field)
}

/// A control's field box: 30 px, `0 10px`, a 1 px border, the button radius, `--card-bg`, 12.5 px.
fn field_box(t: &Theme) -> El {
    row()
        .h(30.0)
        .px(10.0)
        .center_y()
        .min_w(0.0)
        .border(1.0, t.border)
        .border_layout()
        .radius(RADIUS_BTN)
        .fill(t.card_bg)
        .focus_ring(2.0, 1.0, t.accent)
}

/// `ModInput`: an optional label, a field with its placeholder and, with `onSubmit`, a submit button (Fleet caps it at half the row;
/// Rune has no percentage maximum, so a long label shrinks with the field instead, `docs/RUNE.md`). It
/// has room for a placeholder (10 em of the site's text) and shrinks below that only when its container does.
pub fn input(s: &State, t: &Theme, key: &str, props: &Map<String, Value>, handles: &Map<String, Value>, em: f32) -> El {
    let placeholder = str_prop(props, "placeholder").unwrap_or_default().to_string();
    let k = key.to_string();
    let edit =
        s.mods_ui.field(key).map(|f| f.edit.clone()).unwrap_or_else(|| rune::TextEdit::new(str_prop(props, "value").unwrap_or_default()));
    let mut field = text_input(&edit, sans(12.5, 400.0, 1.3))
        .key(format!("{key}.field"))
        .label(
            str_prop(props, "label")
                .filter(|l| !l.is_empty())
                .unwrap_or(if placeholder.is_empty() { "Text" } else { &placeholder })
                .to_string(),
        )
        .single_line()
        .placeholder(placeholder, t.muted)
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .grow(1.0)
        .shrink()
        .min_w(0.0)
        .on_edit({
            let k = k.clone();
            move |s: &mut State| s.mods_ui.edit_or_spare(&k)
        })
        .on_input_change({
            let k = k.clone();
            move |s: &mut State, tx: &mut Tx| {
                let now = s.clock;
                s.mods_ui.typed(&k, now, tx)
            }
        });
    let submit = handle(handles, "onSubmit").is_some();
    if submit {
        let k = k.clone();
        field = field.on_submit(move |s: &mut State, _: &mut Tx| s.mods_ui.submit(&k));
    }
    let mut line = row().gap(6.0).min_w(0.0).child(field_box(t).grow(1.0).shrink().child(field));
    if submit {
        let k = k.clone();
        let label = str_prop(props, "submitLabel").unwrap_or("Submit").to_string();
        line = line.child(
            row()
                .key(format!("{key}.submit"))
                .role("button")
                .label(label.clone())
                .h(30.0)
                .px(12.0)
                .center()
                .radius(RADIUS_BTN)
                .fill(t.accent)
                .hover(t.mix(t.accent, Color::BLACK, 0.88))
                .cursor(Cursor::Pointer)
                .focusable(true)
                .focus_ring(2.0, 1.0, t.accent)
                .shrink()
                .min_w(0.0)
                .child(text(label).font(sans(12.0, 500.0, 1.3)).color(t.primary_fg).truncate().shrink().min_w(0.0))
                .on_press(move |s: &mut State| s.mods_ui.submit(&k)),
        );
    }
    labelled(t, props, line).key(key.to_string()).grow(1.0).shrink().min_w(10.0 * em)
}

/// `ModSelect`'s options: a value, and its label (the value when there is none). Anything that isn't an object with a string
/// value is skipped.
pub fn options(props: &Map<String, Value>) -> Vec<(String, String)> {
    props
        .get("options")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|o| {
                    let value = o.as_object()?.get("value")?.as_str()?;
                    Some((value.to_string(), o.get("label").and_then(Value::as_str).unwrap_or(value).to_string()))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// `ModSelect`: an optional label over a 30 px select showing the chosen option (the tree's `value` until one is picked), with the
/// list in the same look as Fleet's other selects.
pub fn select_control(s: &State, t: &Theme, key: &str, props: &Map<String, Value>, handles: &Map<String, Value>) -> El {
    let list = options(props);
    let wanted = s.mods_ui.chosen(key).map(str::to_string).or_else(|| str_prop(props, "value").map(str::to_string));
    // A select with no matching option shows its first.
    let on = list.iter().find(|(v, _)| Some(v) == wanted.as_ref()).or(list.first());
    let (current, shown) = (on.map(|(v, _)| v.clone()), on.map(|(_, l)| l.clone()).unwrap_or_default());
    let (k, h) = (key.to_string(), handle(handles, "onSelect"));
    let trigger = field_box(t)
        .key(format!("{key}.trigger"))
        .role("combobox")
        .label(shown.clone())
        .gap(8.0)
        .justify_between()
        .shrink()
        .grow(1.0)
        .cursor(Cursor::Pointer)
        .focusable(true)
        .child(text(shown).font(sans(12.5, 400.0, 1.3)).color(t.text).truncate().shrink().min_w(0.0))
        .child(icon(&crate::icons::CHEVRON_DOWN, 12.0).color(t.muted))
        .on_press(move |s: &mut State| {
            s.mods_ui.open_select = Some(ModControls::open(&k, h.clone(), list.clone(), current.clone()));
            s.ui.open = Some(Open::Select(OPEN));
        });
    // The select fills its row, as Fleet's does (a flex column stretches its field).
    labelled(t, props, row().min_w(0.0).child(trigger)).key(key.to_string()).min_w(0.0).shrink()
}

/// The open list, under its select.
pub fn select_list(s: &State, t: &Theme) -> Vec<El> {
    let Some(open) = &s.mods_ui.open_select else { return Vec::new() };
    let rows: Vec<(&str, &str)> = open.options.iter().map(|(v, l)| (v.as_str(), l.as_str())).collect();
    let (k, h) = (open.key.clone(), open.handle.clone());
    let choose: select::Choose = Rc::new(move |s: &mut State, value: &str| s.mods_ui.picked(&k, h.as_deref(), value));
    let width = open.options.iter().map(|(_, l)| l.chars().count()).max().unwrap_or(0) as f32 * 6.9 + 44.0;
    select::content(t, "mod-select-list", &format!("{}.trigger", open.key), width, &rows, open.current.as_deref(), choose)
}

/// One line of the scene's log of what was sent: `press h1`, `input h2 = "abc"`.
pub fn sent_line(a: &crate::fleet::mods::types::Action) -> String {
    use crate::fleet::mods::types::ActionKind::*;
    let kind = match a.kind {
        Press => "press",
        Input => "input",
        Submit => "submit",
        Select => "select",
    };
    match &a.value {
        Some(v) => format!("{kind} {} = {v:?}", a.handle),
        None => format!("{kind} {}", a.handle),
    }
}
