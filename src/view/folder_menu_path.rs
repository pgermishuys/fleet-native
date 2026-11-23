//! Ports the path view of `NSD/FolderPicker.vue` ("Open or create a folder", rows B2.110-B2.145) and the warning panel it can end
//! in (B2.150-B2.154): the box with its Up button, what is in the folder, the create options with their preview, and what
//! the server said. What the rows are is `fleet::new_session::path`; what the keys do is `folder_path`.
use crate::fleet::new_session::folder_menu::{Make, NewKind, Opening};
use crate::fleet::new_session::folder_path::{BRANCH_KEY, PATH_KEY, path_row_key};
use crate::fleet::new_session::path::{PathAction, PathCreate, PathRow};
use crate::fleet::new_session::plan::tilde_path;
use crate::icons::*;
use crate::theme::{Theme, mono, sans};
use crate::ui::button::{Size, Variant, button};
use crate::ui::chip::{Option_, option, pop_note, pop_separator};
use crate::ui::text_menu::text_input;
use crate::view::folder_menu::{alert, make_error, spinner};
use crate::{State, Tx};
use rune::{Cursor, El, KeyEvent, ScrollSpec, col, el, icon, row, text};

/// `.ns-folder-list--path`: `min(240px, 36vh)`.
const LIST_MAX_H: f32 = 240.0;

fn back(t: &Theme) -> El {
    option(t, Option_ { icon: Some(&ARROW_LEFT), ..Option_::new("new-session-folder-back", "All folders") }, None, |s, tx| {
        s.show_folder_list(tx)
    })
}

/// `.ns-folder-pathbox`: the folder's icon, the path, the Up button. The box's border is always the accent colour.
fn pathbox(s: &State, t: &Theme) -> El {
    let input = text_input(&s.folder_menu.path, mono(12.0, 400.0, 1.3))
        .key(PATH_KEY)
        .label(if s.folder_allow_create() { "Open or create a folder" } else { "Open a folder" })
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .single_line()
        .grow(1.0)
        .min_w(0.0)
        .on_edit(|s: &mut State| &mut s.folder_menu.path)
        .on_input_change(|s: &mut State, _: &mut Tx| s.path_changed())
        .on_input_key(|s: &mut State, ev: &KeyEvent, tx: &mut Tx| s.path_key(ev, tx));
    let up = row()
        .key("new-session-path-up")
        .role("button")
        .label("Up one folder")
        .help("Up one folder")
        .size(24.0, 24.0)
        .center()
        .radius(4.0)
        .cursor(Cursor::Pointer)
        .hover_group()
        .hover(t.wash(8.0))
        .focusable(true)
        .focus_ring(2.0, 0.0, t.accent)
        .on_press_tx(|s: &mut State, tx: &mut Tx| s.go_up(tx))
        .child(icon(&CORNER_LEFT_UP, 13.0).color(t.muted).group_color(t.text));
    row()
        .key("new-session-path-box")
        .items_center()
        .gap(6.0)
        .pl(8.0)
        .pr(4.0)
        .radius(6.0)
        .border(1.0, t.accent)
        .fill(t.wash(3.0))
        .child(icon(&FOLDER_OPEN, 14.0).color(t.muted))
        .child(row().grow(1.0).min_w(0.0).h(30.0).items_center().child(input))
        .child(up)
}

fn field(s: &State, t: &Theme) -> El {
    let label = if s.folder_allow_create() { "Open or create a folder" } else { "Open a folder" };
    col()
        .key("new-session-path")
        .w_full()
        .gap(4.0)
        .px(8.0)
        .pt(6.0)
        .pb(4.0)
        .child(text(label).font(sans(11.5, 400.0, 1.3)).color(t.muted))
        .child(pathbox(s, t))
}

fn row_mark(t: &Theme, r: &PathRow, working: bool) -> El {
    if working {
        return spinner(t);
    }
    match (r.action, r.is_git_repo) {
        (PathAction::Create, _) => icon(&FOLDER_PLUS, 14.0).color(t.accent),
        (_, true) => icon(&FOLDER_GIT_2, 14.0).color(t.muted),
        (PathAction::Open, _) => icon(&FOLDER_OPEN, 14.0).color(t.muted),
        _ => icon(&FOLDER, 14.0).color(t.muted),
    }
}

fn rows(s: &State, t: &Theme) -> El {
    let rows = s.path_rows();
    let hl = s.folder_menu.path_highlight;
    let (making, opening) = (s.folder_menu.make, s.folder_menu.opening);
    let mut c = col().key("new-session-path-list").role("listbox").label("What's in this folder").w_full();
    for (i, r) in rows.iter().enumerate() {
        let creating = r.action == PathAction::Create && making == Make::Creating;
        let working = creating || (r.action != PathAction::Create && opening != Opening::Idle && i == hl);
        let hint = (i == hl).then(|| if r.action == PathAction::Enter && !r.is_git_repo { "Tab" } else { "Enter" }.to_string());
        let spec = Option_ {
            key: path_row_key(i, r),
            title: if creating { "Creating".into() } else { r.title.clone() },
            detail: Some(r.detail.clone()),
            mono_detail: r.action == PathAction::Open,
            highlighted: i == hl,
            bold: r.title_name.clone(),
            hint,
            ..Option_::new("", "")
        };
        let (run, at) = (r.clone(), i);
        c = c.child(
            option(t, spec, Some(row_mark(t, r, working)), move |s, tx| s.run_path_row(&run, tx))
                .on_hover(move |s: &mut State| s.folder_menu.path_highlight = at),
        );
    }
    let looking = s.current_listing().is_none() && s.folder_menu.listing_error.is_none();
    if looking {
        c = c.child(pop_note(t, "Looking…"));
    } else if s.current_listing().is_some() && rows.is_empty() && s.path_note().is_none() {
        c = c.child(pop_note(t, "Nothing in this folder."));
    }
    c.scroll(ScrollSpec { stick_bottom: false }).max_h(crate::ui::chip::list_max_h(s, LIST_MAX_H, 0.36))
}

fn kind_button(s: &State, t: &Theme, key: &str, glyph: &'static rune::Icon, label: &str, kind: NewKind) -> El {
    let pressed = s.folder_menu.new_kind == kind;
    let busy = s.folder_menu.make != Make::Idle;
    let ink = if pressed { t.text } else { t.muted };
    let weight = if pressed { 500.0 } else { 400.0 };
    let b = row()
        .key(key.to_string())
        .role("button")
        .label(label.to_string())
        .w(0.0)
        .grow(1.0)
        .min_w(0.0)
        .center()
        .gap(5.0)
        .px(4.0)
        .py(5.0)
        .radius(4.0)
        .fill(if pressed { t.wash(8.0) } else { rune::Color::TRANSPARENT })
        .child(icon(glyph, 13.0).color(ink))
        .child(text(label.to_string()).font(sans(12.0, weight, 1.3)).color(ink));
    if busy {
        return b.opacity(0.5).disabled(true);
    }
    b.cursor(Cursor::Pointer).focusable(true).focus_ring(2.0, 0.0, t.accent).on_press(move |s: &mut State| s.folder_menu.new_kind = kind)
}

fn branch_field(s: &State, t: &Theme) -> El {
    let input = text_input(&s.folder_menu.branch, mono(12.0, 400.0, 1.3))
        .key(BRANCH_KEY)
        .label("Branch")
        .placeholder(s.feeds.first_branch().to_string(), t.muted)
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .single_line()
        .w_full()
        .on_edit(|s: &mut State| &mut s.folder_menu.branch)
        .on_input_key(|s: &mut State, ev: &KeyEvent, _: &mut Tx| {
            let enter = ev.key == rune::Key::Enter;
            if enter {
                s.create_from_path();
            }
            enter
        });
    row().items_center().gap(6.0).child(text("on branch").font(sans(12.0, 400.0, 1.3)).color(t.muted)).child(
        row()
            .w(120.0)
            .h(26.0)
            .px(7.0)
            .items_center()
            .radius(5.0)
            .border(1.0, t.border)
            .fill(t.wash(3.0))
            .focus_within_ring(1.0, -1.0, t.accent)
            .child(input),
    )
}

/// `.ns-folder-create`: Empty folder or Git repository, and the first branch's name.
fn create_options(s: &State, t: &Theme) -> El {
    let kinds = row()
        .role("group")
        .label("Start with")
        .w_full()
        .gap(2.0)
        .padding(2.0)
        .radius(6.0)
        .border(1.0, t.border)
        .child(kind_button(s, t, "new-session-path-kind-empty", &FOLDER, "Empty folder", NewKind::Empty))
        .child(kind_button(s, t, "new-session-path-kind-git", &GIT_BRANCH, "Git repository", NewKind::Git));
    let mut c = col().key("new-session-path-create-options").w_full().gap(8.0).px(8.0).pt(6.0).pb(8.0).child(kinds);
    if s.folder_menu.new_kind == NewKind::Git {
        c = c.child(branch_field(s, t));
    }
    c
}

fn tree_row(t: &Theme, lead: &'static rune::Icon, ink: rune::Color, words: String, indent: f32, tag: bool) -> El {
    let mut r = row()
        .w_full()
        .pl(indent)
        .items_center()
        .gap(6.0)
        .child(icon(lead, 13.0).color(ink))
        .child(text(words).font(mono(12.0, 400.0, 1.3)).color(t.text).wrap().min_w(0.0));
    if tag {
        r = r.child(
            row().px(4.0).radius(4.0).fill(t.running.alpha(0.14)).child(text("new").font(sans(10.0, 600.0, 1.6)).color(t.running_text)),
        );
    }
    r
}

/// `.ns-folder-tree`: the folder that is there and the folders a create would make under it, then what kind of folder.
fn preview(s: &State, t: &Theme, plan: &PathCreate) -> El {
    let located = s.feeds.roots.contains(&plan.base);
    let lead = if located { &MAP_PIN } else { &FOLDER };
    let mut c =
        col().key("new-session-path-preview").gap(1.0).mx(8.0).mb(8.0).px(9.0).py(7.0).radius(6.0).fill(t.wash(4.0)).child(tree_row(
            t,
            lead,
            t.muted,
            s.shown_path(&plan.base),
            0.0,
            false,
        ));
    for (depth, name) in plan.folders.iter().enumerate() {
        c = c.child(tree_row(t, &FOLDER_PLUS, t.accent, name.clone(), (depth as f32 + 1.0) * 14.0, true));
    }
    let what = match s.folder_menu.new_kind {
        NewKind::Git => {
            let typed = s.folder_menu.branch.text.trim();
            let branch = if !typed.is_empty() { typed } else { s.feeds.first_branch() };
            let branch = if branch.is_empty() { "main" } else { branch };
            let lead = "A git repository on ";
            let tail = " with an empty first commit";
            let spans = vec![rune::Span::new(lead.len(), lead.len() + branch.len()).mono().size(11.0)];
            rune::rich(format!("{lead}{branch}{tail}"), spans)
        }
        NewKind::Empty => text("An empty folder"),
    };
    c.child(el().h(3.0)).child(what.font(sans(11.5, 400.0, 1.4)).color(t.muted).wrap().w_full())
}

/// `.ns-folder-hint`: the folder isn't under any location, so opening it adds it.
fn adds_location(t: &Theme, row_: &PathRow) -> El {
    let what = if row_.action == PathAction::Create { "it".to_string() } else { row_.title_name.clone().unwrap_or_default() };
    row()
        .key("new-session-path-adds-location")
        .mx(8.0)
        .mb(8.0)
        .gap(7.0)
        .child(el().mt(2.0).child(icon(&INFO, 13.0).color(t.status_waiting())))
        .child(
            text(format!("Fleet adds {what} to your locations, so it's listed here next time."))
                .font(sans(12.0, 400.0, 1.4))
                .color(t.muted)
                .wrap()
                .grow(1.0)
                .min_w(0.0),
        )
}

fn submit(s: &State, t: &Theme) -> El {
    let creating = s.folder_menu.make == Make::Creating;
    let b = button(t, Variant::Default, Size::Sm, None, if creating { "Creating…" } else { "Create and use" })
        .key("new-session-path-create-submit")
        .on_press(|s: &mut State| s.create_from_path());
    let b = if s.folder_menu.make != Make::Idle { b.opacity(0.5).disabled(true) } else { b };
    row().w_full().justify_end().gap(6.0).px(8.0).pb(6.0).child(b)
}

/// The whole view, one child of the popover per block.
pub fn path_view(s: &State, t: &Theme) -> Vec<El> {
    let mut body = vec![back(t), pop_separator(t), field(s, t), rows(s, t)];
    if let Some(note) = s.path_note() {
        body.push(alert(t, "new-session-path-note", note));
    }
    let highlighted = s.highlighted_path_row();
    let create = highlighted.as_ref().filter(|r| r.action == PathAction::Create).and_then(|_| s.path_create());
    if let Some(plan) = &create {
        body.push(pop_separator(t));
        body.push(create_options(s, t));
        body.push(preview(s, t, plan));
    }
    if s.path_adds_location()
        && let Some(r) = &highlighted
    {
        body.push(adds_location(t, r));
    }
    body.extend(make_error(s, t));
    if create.is_some() {
        body.push(submit(s, t));
    }
    body
}

/// `.ns-folder-add`: replaces the whole menu when a folder was made with something to say (B2.150-B2.154).
pub fn warning_panel(s: &State, t: &Theme) -> El {
    let Some(w) = &s.folder_menu.warning else { return el() };
    let title = format!("Created {}", tilde_path(&w.path));
    let ok = button(t, Variant::Default, Size::Sm, None, "OK")
        .key("new-session-folder-warning-ok")
        .on_press(|s: &mut State| s.close_folder_menu());
    let words = col()
        .min_w(0.0)
        .gap(2.0)
        .child(text(title).font(sans(13.0, 500.0, 1.3)).color(t.text).truncate())
        .child(text(w.text.clone()).font(sans(12.0, 400.0, 1.4)).color(t.muted).wrap().w_full());
    col()
        .key("new-session-folder-warning")
        .role("status")
        .mx(3.0)
        .my(3.0)
        .padding(8.0)
        .gap(8.0)
        .radius(6.0)
        .border(1.0, t.border_mix(t.accent, 35.0))
        .fill(t.accent.alpha(0.07))
        .child(row().gap(8.0).child(el().w(16.0).mt(2.0).child(icon(&GIT_BRANCH, 14.0).color(t.accent))).child(words.grow(1.0)))
        .child(row().w_full().justify_end().pt(2.0).child(ok))
}
