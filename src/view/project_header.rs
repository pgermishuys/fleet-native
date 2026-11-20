//! A group's header in the session list: the header button of `ProjectGroup.vue` (`.project-header`). The Pinned
//! group's has a pin before its name; the Ungrouped group's is a plain project header. A real project's is wrapped by
//! `project_shell` (piece G), which adds the +, the right-click menu, F2 and the name field. The caller adds `.on_press`, so one header serves every group.
use crate::fleet::projects::RENAME_FIELD;
use crate::icons::*;
use crate::theme::{Theme, sans};
use crate::ui::text_menu::text_input;
use crate::{State, Tx};
use rune::{Cursor, El, Key, KeyEvent, el, icon, row, text};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Header {
    Project,
    /// Sessions with no project: always last; the same header as a project's.
    Ungrouped,
    /// `pinned-header`: `Pin` 11x11, 5 before the name.
    Pinned,
}

/// `plus`: the group has a +, so the title keeps 28px clear for it and nothing moves when it shows.
pub fn project_header(t: &Theme, key: String, kind: Header, name: &str, count: usize, open: bool, plus: bool) -> El {
    let turn = if open { 0.0 } else { -std::f32::consts::FRAC_PI_2 };
    let label = text(name.to_string()).font(sans(12.0, 600.0, 1.3)).color(t.muted).group_color(t.text).truncate();
    let title = match kind {
        Header::Pinned => row()
            .key("pinned-header")
            .center_y()
            .grow(1.0)
            .min_w(0.0)
            .child(icon(&PIN, 11.0).color(t.muted).group_color(t.text).mr(5.0))
            .child(label.grow(1.0)),
        _ => row().grow(1.0).min_w(0.0).mr(if plus { 28.0 } else { 0.0 }).child(label.grow(1.0)),
    };
    row()
        .key(key)
        .w_full()
        .min_h(26.0)
        .center_y()
        .gap(6.0)
        .pl(6.0)
        .pr(10.0)
        .radius(8.0)
        .hover(t.wash(5.0))
        .hover_group()
        .cursor(Cursor::Pointer)
        .focusable(true)
        // `.project-header:focus-visible`: 2px of the accent, drawn inside.
        .focus_ring(2.0, -2.0, t.accent)
        .child(icon(&CHEVRON_DOWN, 13.0).color(t.muted.alpha(0.7)).rotate(turn))
        .child(title)
        .child(text(count.to_string()).font(sans(12.0, 500.0, 1.3).features("'tnum' 1")).color(t.muted.alpha(0.7)))
}

/// `.project-shell`: the header of a real project with its + (`project-new-session`), its right-click menu and F2, or the
/// name field in its place while it is renamed. `count` is what the group shows.
pub fn project_shell(s: &State, t: &Theme, p: usize, count: usize) -> El {
    let project = &s.projects[p];
    let key = format!("project-{p}");
    let shell = el().w_full().hover_group().on_secondary_press(move |s: &mut State, at, _: &mut Tx| {
        // Not while renaming (`ContextMenu v-if="canShowContextMenu && !isInlineEditing"`).
        if s.project_is_real(p) && !s.project_renaming(p) {
            s.ui.point = (at.x, at.y);
            s.ui.open = Some(crate::ui::Open::ProjectMenu(p));
        }
    });
    if s.project_renaming(p) {
        return shell.child(editing(s, t, key, project.open));
    }
    let label = format!("New session in {}", project.name);
    let mut plus = el()
        .key(format!("project-new-{p}"))
        .role("button")
        .label(label.clone())
        .help(label)
        .size(22.0, 22.0)
        .center()
        .radius(6.0)
        .pin(None, Some(2.0), Some(34.0), None)
        .cursor(Cursor::Pointer)
        .focus_ring(2.0, -1.0, t.accent)
        .child(icon(&PLUS, 15.0).color(t.muted).group_color(t.accent_text));
    plus = if s.proj.pending {
        // The request in flight (`isAnyActionPending`) disables it.
        plus.disabled(true).group_opacity(0.0, 0.5)
    } else {
        // Hidden until the header is hovered or has focus inside (`.project-shell:hover .project-new`).
        plus.group_opacity(0.0, 1.0)
            .hover(t.wash(10.0))
            .hover_group()
            .on_press_tx(move |s: &mut State, tx: &mut Tx| s.new_session_in(p, tx))
    };
    let header = project_header(t, key, Header::Project, &project.name, count, project.open, true)
        .on_press(move |s: &mut State| s.toggle_project(p))
        .on_key(move |s: &mut State, ev: &KeyEvent, tx: &mut Tx| {
            if ev.key == Key::Function(2) && s.project_is_real(p) {
                s.start_rename(p, tx);
                return true;
            }
            false
        });
    shell.child(header).child(plus)
}

/// `.project-header--editing`: the same row with no button in it and the field where the title was; no count.
pub fn editing(s: &State, t: &Theme, key: String, open: bool) -> El {
    let turn = if open { 0.0 } else { -std::f32::consts::FRAC_PI_2 };
    let busy = s.proj.pending;
    // `.project-copy` (flex 1 1 auto) beside an empty `.project-spacer` (flex 1): the field is as wide as its own 193px plus
    // half of what is left, so it stops short of the right edge. Rune's `grow` has no basis to start from, so it is worked out.
    let left = s.sidebar_w - 16.0 - 6.0 - 10.0 - 13.0 - 12.0;
    let field_w = ((left + 193.0) / 2.0).max(60.0);
    // `InlineEdit`: 28px high, a 1px `--input` border, no radius, 12.25px text, a 3px ring while focused.
    let mut field = text_input(&s.proj.edit, sans(12.25, 400.0, 1.3))
        .key(RENAME_FIELD)
        .label("Project name")
        .placeholder("Project name", t.muted)
        .single_line()
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .w(field_w)
        .min_w(0.0)
        .h(28.0)
        .px(7.0)
        .py(3.5)
        .fill(if t.dark { t.wash(5.4) } else { rune::Color::TRANSPARENT })
        // `focus-visible:border-ring`.
        .border(1.0, if s.proj.edit.focused { t.accent } else { t.input() })
        .border_layout()
        .focus_ring(3.0, 0.0, t.accent.alpha(0.5))
        .on_edit(|s: &mut State| &mut s.proj.edit)
        .on_input_key(|s: &mut State, ev: &KeyEvent, tx: &mut Tx| match ev.key {
            Key::Enter => {
                s.finish_rename(true, tx);
                true
            }
            Key::Escape => {
                s.finish_rename(false, tx);
                true
            }
            _ => false,
        });
    if busy {
        field = field.disabled(true).opacity(0.5);
    }
    row()
        .key(key)
        .w_full()
        .min_h(26.0)
        .center_y()
        .gap(6.0)
        .pl(6.0)
        .pr(10.0)
        .child(icon(&CHEVRON_DOWN, 13.0).color(t.muted.alpha(0.7)).rotate(turn))
        .child(field)
}
