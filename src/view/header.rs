//! The session header (ports `SessionDetailHeader.vue`'s title row and meta row; the banners under it are in
//! `header_banners.rs`). What it says comes from `fleet::header::HeaderModel`; this file only draws it.
//!
//! The context chips between the text and the buttons are `header_context.rs`. Rename, tags, the copy button and Restore now are `fleet::header_actions`.
use super::parts::status_glyph;
use crate::fleet::groups::LineageKind;
use crate::fleet::header::{Checkout, HeaderModel, Indicator, MachineChip};
use crate::fleet::header_actions::{TAG_FIELD, TITLE_FIELD};
use crate::icons::*;
use crate::mock::{Project, Session};
use crate::theme::{RADIUS_BTN, Theme, mono, sans};
use crate::{State, Tx};
use rune::{Cursor, Cycle, El, Key, KeyEvent, Span, col, el, icon, rich, row, text, text_input};

/// `@container session-detail-header (min-width: 48rem)`: the header's sides widen from 1rem to 1.25rem.
const WIDE_AT: f32 = 672.0;
/// The buttons at the right edge: five of 24.5 and a rule, with the 0.75rem gap before them (the pill's "half" is of the
/// room that is left).
const ACTIONS_W: f32 = 5.0 * 24.5 + 4.0 * 2.0 + 5.0 + 10.5;

pub fn header(s: &State, t: &Theme, centre_w: f32) -> El {
    let m = s.header_model();
    let session = s.session();
    let project = s.project();
    let px = if centre_w >= WIDE_AT { 17.5 } else { 14.0 };
    let room = (centre_w - 2.0 * px - ACTIONS_W).max(0.0);
    let main = col().grow(1.0).shrink().min_w(0.0).gap(3.0).child(title_row(s, t, &m, room)).child(meta_row(s, t, &m, session, project));
    row()
        .key("session-header")
        .w_full()
        .min_h(56.0)
        .center_y()
        .px(px)
        .py(8.0)
        .border_sides([0.0, 0.0, 1.0, 0.0], t.border)
        .child(main)
        .children(super::header_context::context_chips(s, t).map(|c| c.ml(10.5)))
        .child(actions(s, t, &m))
}

fn title_row(s: &State, t: &Theme, m: &HeaderModel, room: f32) -> El {
    // The glyph says what the list's row would, so it shows only while no list does.
    let glyph = (!s.sidebar_open).then(|| {
        let glyph = el()
            .key("session-header-glyph")
            .size(8.0, 8.0)
            .center()
            .label(m.status_label.clone())
            .help(m.status_label.clone())
            .child(status_glyph(t, m.glyph.word(), m.indicator == Indicator::Retry));
        // The row's slot is 8px; the glyph is centred in it with the gap the title row has.
        glyph
    });
    let editing = s.head.editing_title(&s.session().id);
    let title = if editing { title_field(s, t) } else { title_text(t, m) };
    row()
        .w_full()
        .min_w(0.0)
        .center_y()
        .gap(6.0)
        .children(glyph)
        .child(title)
        .children(if editing { None } else { lineage_pill(s, t, m, room) })
        .children(m.retry_note.as_ref().map(|note| {
            text(note.clone())
                .key("session-retry-note")
                .font(sans(12.0, 500.0, 1.3))
                .color(t.status_waiting_text)
                .truncate()
                .shrink()
                .min_w(0.0)
                .help(note.clone())
        }))
        .children(m.archived.then(|| badge(t, "session-archived-badge", "Archived")))
        .children(m.checkout.as_ref().map(|c| checkout_chip(s, t, m, c)))
}

/// The title as text: a double-click or F2 renames it unless the session is archived (`session-detail-header__title--editable`).
fn title_text(t: &Theme, m: &HeaderModel) -> El {
    let title = text(m.title.clone())
        .key("session-title")
        .font(sans(14.0, 600.0, 1.3).tracking(-0.07))
        .color(t.text)
        .truncate()
        .shrink()
        .min_w(0.0);
    if m.archived {
        return title;
    }
    title
        .ml(-6.0)
        .px(6.0)
        .radius(6.0)
        .hover(t.wash(5.0))
        .cursor(Cursor::Text)
        .focusable(true)
        .focus_ring(2.0, 0.0, t.accent)
        .help("Double-click to rename")
        .on_double_press_tx(|s: &mut State, tx: &mut Tx| s.start_header_rename(tx))
        .on_key(|s: &mut State, ev: &KeyEvent, tx: &mut Tx| {
            if ev.key == Key::Function(2) {
                s.start_header_rename(tx);
                return true;
            }
            false
        })
}

/// `.session-detail-header__title-input`: the title's own size in its place, an accent border and a 3px ring.
fn title_field(s: &State, t: &Theme) -> El {
    text_input(&s.head.title_edit, sans(14.0, 600.0, 1.3).tracking(-0.07))
        .key(TITLE_FIELD)
        .label("Session name")
        .placeholder("Session name", t.muted)
        .single_line()
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .grow(1.0)
        .shrink()
        .min_w(0.0)
        .max_w(480.0)
        .ml(-7.0)
        .px(6.0)
        .fill(t.card_bg)
        .radius(6.0)
        .border(1.0, t.accent)
        .border_layout()
        .focus_ring(3.0, 0.0, t.accent_dim)
        .on_edit(|s: &mut State| &mut s.head.title_edit)
        .on_input_key(|s: &mut State, ev: &KeyEvent, tx: &mut Tx| match ev.key {
            Key::Enter => {
                s.finish_header_rename(true, tx);
                true
            }
            Key::Escape => {
                s.finish_header_rename(false, tx);
                true
            }
            _ => false,
        })
}

/// `Badge variant="secondary"`.
fn badge(t: &Theme, key: &str, word: &str) -> El {
    text(word).key(key).font(sans(12.0, 500.0, 1.3)).color(t.text).px(8.0).py(2.0).radius(6.0).fill(t.secondary())
}

/// "Forked from <title>": a quiet pill that opens the parent. A subagent's session has its banner instead.
fn lineage_pill(s: &State, t: &Theme, m: &HeaderModel, room: f32) -> Option<El> {
    let (parent, kind) = m.lineage.clone().filter(|(_, k)| *k != LineageKind::Subagent)?;
    let label = match kind {
        LineageKind::Fork => "Forked from",
        _ => "Started by",
    };
    let title = s.parent_title(&parent).unwrap_or_else(|| "another session".into());
    let said = format!("This session was {} {title}", label.to_lowercase());
    let to = parent.clone();
    Some(
        row()
            .key("session-lineage-link")
            .shrink()
            .min_w(0.0)
            .max_w((room * 0.5).max(0.0))
            .center_y()
            .gap(4.0)
            .px(8.0)
            .py(2.0)
            .radius(999.0)
            .border(1.0, t.border)
            .border_layout()
            .hover(t.wash(5.0))
            .hover_group()
            .cursor(Cursor::Pointer)
            .help(said.clone())
            .label(said)
            .on_press_tx(move |s: &mut State, tx: &mut rune::Tx| s.open_parent(&to, tx))
            .child(icon(&CORNER_LEFT_UP, 12.0).color(t.muted).group_color(t.text))
            .child(text(label).font(sans(12.0, 400.0, 1.3)).color(t.muted).group_color(t.text))
            .child(text(title).font(sans(12.0, 500.0, 1.3)).color(t.text).truncate().shrink().min_w(0.0)),
    )
}

fn spinner(color: rune::Color) -> El {
    static FADE: [(f32, f32); 3] = [(0.0, 1.0), (0.5, 0.35), (1.0, 1.0)];
    icon(&LOADER_CIRCLE, 11.0).color(color).cycle(Cycle { period: 1.2, phase: 0.0, stops: &FADE, still: 1.0, fps: 30.0 })
}

/// The removed-checkout chip (`.session-detail-header__checkout`) and, after a failed restore, the server's sentence in red.
fn checkout_chip(s: &State, t: &Theme, m: &HeaderModel, c: &Checkout) -> El {
    let error = s.head.restore_error_of(&s.session().id).map(str::to_string);
    let quiet = *c == Checkout::Restored;
    let ink = if quiet { t.muted } else { t.status_waiting_text };
    let font = sans(11.0, 400.0, 1.3);
    let mut chip = row()
        .key("session-checkout-chip")
        .center_y()
        .gap(6.0)
        .px(8.0)
        .py(1.0)
        .radius(999.0)
        .border(1.0, t.idle.alpha(0.45))
        .border_layout()
        .help(error.clone().unwrap_or_else(|| m.checkout_sentence.clone()));
    chip = if quiet { chip.fill(rune::Color::TRANSPARENT) } else { chip.fill(t.idle.alpha(0.10)) };
    if *c == Checkout::Restoring {
        chip = chip.child(spinner(ink));
    }
    chip = chip.child(text(c.text()).font(font).color(ink));
    if *c == Checkout::Removed {
        let word = "Restore now";
        chip = chip.child(
            row()
                .key("session-checkout-restore")
                .cursor(Cursor::Pointer)
                .label(word)
                .on_press_tx(|s: &mut State, tx: &mut Tx| s.restore_checkout(tx))
                .child(rich(word, vec![Span::new(0, word.len()).underline(t.text)]).font(font).color(t.text)),
        );
    }
    if let Some(error) = error {
        chip = chip.child(text(error).key("session-checkout-error").font(font).color(t.error_text).label("Checkout restore failed"));
    }
    chip
}

fn meta_row(s: &State, t: &Theme, m: &HeaderModel, session: &Session, project: &Project) -> El {
    let meta_font = sans(12.0, 400.0, 1.4);
    let path_font = mono(11.5, 400.0, 1.4);
    let dot = || text("·").font(meta_font).color(t.muted.alpha(0.6));
    let plain = |w: &str| text(w).font(meta_font).color(t.muted).max_w(14.0 * 14.0);
    // The invented sessions keep their invented words; a session from a server says only what it knows.
    let mock = s.is_mock();
    let project_name = m.project.clone().or_else(|| mock.then(|| project.name.clone()));
    let harness = m.harness_label.clone().or_else(|| mock.then(|| "OpenCode".to_string()));
    let model = m.model_label.clone().or_else(|| mock.then(|| "Sonnet 5.5".to_string()));
    let branch = m.branch.clone().or_else(|| (!session.branch.is_empty()).then(|| session.branch.clone()));
    let (head, leaf, has_dir) = if m.directory.is_empty() && mock {
        let (h, l) = crate::fleet::header::directory_parts(&project.path);
        (h, l, true)
    } else {
        (m.directory_head.clone(), m.directory_leaf.clone(), !m.directory.is_empty())
    };
    let has_project = project_name.is_some();

    let mut meta = row().w_full().min_w(0.0).center_y().gap(6.0).clip_rounded(0.0);
    if let Some(chip) = &m.machine {
        meta = meta.child(machine_chip(t, chip));
    }
    if let Some(name) = &project_name {
        meta = meta.child(plain(name).key("session-project"));
    }
    if has_project && harness.is_some() {
        meta = meta.child(dot());
    }
    if let Some(h) = &harness {
        meta = meta.child(plain(h).key("session-harness-label"));
    }
    if let Some(profile) = &m.profile {
        let tip = format!("Started with the {profile} profile, which it keeps");
        meta = meta.child(dot()).child(
            row()
                .key("session-profile-label")
                .center_y()
                .gap(4.0)
                .shrink()
                .min_w(0.0)
                .max_w(12.0 * 14.0)
                .help(tip)
                .child(icon(&LAYERS, 11.0).color(t.accent_text))
                .child(text(profile.clone()).font(meta_font).color(t.accent_text).truncate().shrink().min_w(0.0)),
        );
    }
    if let Some(model) = &model {
        if has_project || harness.is_some() || m.profile.is_some() {
            meta = meta.child(dot());
        }
        meta = meta.child(plain(model).key("session-model-label").help("The model the next prompt will get"));
    }
    if let Some(name) = &branch {
        if has_project || harness.is_some() {
            meta = meta.child(dot());
        }
        // Long names cut at 18ch plus the icon (`max-width: calc(18ch + 16px)`).
        meta = meta.child(
            row()
                .key("session-branch")
                .center_y()
                .gap(4.0)
                .min_w(0.0)
                .max_w(18.0 * 6.9 + 16.0)
                .help(format!("Branch {name}"))
                .child(icon(&GIT_BRANCH, 12.0).color(t.muted))
                .child(text(name.clone()).font(path_font).color(t.text).truncate().shrink().min_w(0.0)),
        );
    }
    if has_dir {
        if has_project || harness.is_some() || branch.is_some() {
            meta = meta.child(dot());
        }
        // The head gives way first, from its start; the folder's own name stays until the head is gone.
        let full = if mock && m.directory.is_empty() { project.path.clone() } else { m.directory.clone() };
        let copied = s.head.copied_until.is_some_and(|until| s.clock < until);
        meta = meta.child(
            row()
                .key("session-directory")
                .shrink()
                .min_w(24.0)
                .center_y()
                .gap(4.0)
                .clip_rounded(0.0)
                .hover_group()
                .help(full.clone())
                .child(text(head).font(path_font).color(t.muted.alpha(0.8)).group_color(t.text).truncate_head().shrink().min_w(0.0))
                .child(text(leaf).font(path_font).color(t.muted.alpha(0.8)).group_color(t.text))
                .child(copy_button(t, full, copied))
                .children(copied.then(|| text("Copied").key("session-copied").font(sans(11.0, 500.0, 1.4)).color(t.muted))),
        );
    }
    meta.child(tags_row(s, t, &m.tags))
}

/// `.session-detail-header__copy-path`: 20 square, shown while the path is hovered (always while "Copied" shows), a check in
/// the running colour once the path is on the clipboard.
fn copy_button(t: &Theme, path: String, copied: bool) -> El {
    let button = el()
        .key("session-copy-directory")
        .size(20.0, 20.0)
        .center()
        .radius(RADIUS_BTN - 2.0)
        .hover(t.wash(7.0))
        .hover_group()
        .cursor(Cursor::Pointer)
        .focusable(true)
        .focus_ring(2.0, 0.0, t.accent)
        .help(path.clone())
        .label("Copy folder path")
        .on_press_tx(move |s: &mut State, tx: &mut Tx| s.copy_directory(&path, tx));
    if copied {
        return button.child(icon(&CHECK, 13.0).color(t.running_text));
    }
    el().group_opacity(0.0, 1.0).child(button.child(icon(&COPY, 13.0).color(t.muted).group_color(t.text)))
}

/// `.session-detail-header__tags-row`: an outline badge per tag with its ×, then the dashed + or the field.
fn tags_row(s: &State, t: &Theme, tags: &[String]) -> El {
    let adding = s.head.adding_tag_to(&s.session().id);
    let mut line = row().key("session-tags").center_y().gap(4.0).ml(4.0);
    for tag in tags {
        let gone = tag.clone();
        line = line.child(
            row()
                .key(format!("session-tag-{tag}"))
                .center_y()
                .gap(3.5)
                .pl(8.0)
                .pr(3.5)
                .py(2.0)
                .radius(6.0)
                .border(1.0, t.border)
                .border_layout()
                .child(text(tag.clone()).font(sans(10.5, 600.0, 1.3)).color(t.text))
                .child(
                    el().key(format!("session-tag-remove-{tag}"))
                        .px(1.75)
                        .py(1.75)
                        .hover_group()
                        .cursor(Cursor::Pointer)
                        .label(format!("Remove tag {tag}"))
                        .on_press(move |s: &mut State| s.remove_tag(&gone))
                        .child(icon(&X, 12.0).color(t.muted).group_color(t.text)),
                ),
        );
    }
    line.child(if adding { tag_field(s, t) } else { add_tag_button(t) })
}

fn add_tag_button(t: &Theme) -> El {
    el().key("add-tag")
        .size(20.0, 20.0)
        .center()
        .radius(RADIUS_BTN - 2.0)
        .border(1.0, t.muted.alpha(0.45))
        .border_dash(3.0, 2.0)
        .hover_group()
        .cursor(Cursor::Pointer)
        .help("Add tag")
        .label("Add tag")
        .on_press_tx(|s: &mut State, tx: &mut Tx| s.start_adding_tag(tx))
        .child(icon(&PLUS, 12.0).color(t.muted.alpha(0.7)).group_color(t.text))
}

/// `.session-detail-header__tag-input`: Enter and blur add, Escape cancels.
fn tag_field(s: &State, t: &Theme) -> El {
    text_input(&s.head.tag_edit, sans(12.0, 400.0, 1.3))
        .key(TAG_FIELD)
        .label("Tag name")
        .placeholder("Tag name\u{2026}", t.muted)
        .single_line()
        .color(t.text)
        .caret(t.text, t.accent.alpha(0.35))
        .min_w(120.0)
        .px(8.0)
        .py(1.0)
        .fill(t.card_bg)
        .radius(RADIUS_BTN - 2.0)
        .border(1.0, if s.head.tag_edit.focused { t.accent } else { t.border })
        .border_layout()
        .on_edit(|s: &mut State| &mut s.head.tag_edit)
        .on_input_key(|s: &mut State, ev: &KeyEvent, _: &mut Tx| match ev.key {
            Key::Enter => {
                s.finish_adding_tag(true);
                true
            }
            Key::Escape => {
                s.finish_adding_tag(false);
                true
            }
            _ => false,
        })
}

/// `.session-detail-header__machine`, shown when the user has more than one machine.
fn machine_chip(t: &Theme, chip: &MachineChip) -> El {
    let (fill, ink) = if chip.answering { (t.coral.alpha(0.14), t.coral_text) } else { (t.muted.alpha(0.14), t.muted) };
    text(if chip.answering { chip.name.clone() } else { format!("{} \u{b7} not answering", chip.name) })
        .key("session-machine")
        .font(mono(10.5, 600.0, 1.4))
        .color(ink)
        .px(6.0)
        .py(1.0)
        .radius(5.0)
        .fill(fill)
        .help(chip.tooltip())
}

/// `toolbar-icon` and `toolbar-icon-danger` (`button/index.ts`): the glyph, or a spinner in its place (`Spinner size="lg"`,
/// 20px); off, it is half faded and takes no press.
struct Tool {
    key: &'static str,
    glyph: &'static rune::Icon,
    tip: String,
    danger: bool,
    off: bool,
    spinning: bool,
    /// The analytics button once either number is known: filled, with its border (`analytics-trigger--active`).
    active: bool,
    /// The terminal button while its drawer is open (`terminal-toggle[aria-pressed=true]`): `--text` at 9 % behind `--text`.
    pressed: bool,
    /// The terminal button's `--running` dot: shells are running with the drawer hidden.
    dot: bool,
}

fn toolbar_button(t: &Theme, b: Tool) -> El {
    let (hover, ink) = if b.danger { (t.error.alpha(0.10), t.error_text) } else { (t.wash(6.0), t.text) };
    let glyph = if b.spinning {
        icon(&LOADER_CIRCLE, 20.0).color(t.muted).cycle(crate::view::ring_popover::spinner())
    } else {
        icon(b.glyph, 14.0).color(if b.pressed { t.text } else { t.muted }).group_color(ink)
    };
    let mut e = el().key(b.key).size(24.5, 24.5).center().radius(8.0).help(b.tip.clone()).label(b.tip).child(glyph);
    if b.active {
        e = e.fill(t.main_bg).border(1.0, t.border).border_layout();
    }
    if b.pressed {
        e = e.fill(t.wash(9.0));
    }
    if b.dot {
        // `.terminal-toggle__dot`: 6 px at top 5, right 5, ringed 2 px in `--panel-bg`.
        e = e.child(
            el().key("terminal-dot")
                .absolute(24.5 - 5.0 - 6.0 - 2.0, 5.0 - 2.0)
                .size(10.0, 10.0)
                .radius(5.0)
                .fill(t.panel_bg)
                .center()
                .child(el().size(6.0, 6.0).radius(3.0).fill(t.running)),
        );
    }
    if b.off {
        return e.opacity(0.5);
    }
    e.hover(hover).press(t.wash(10.0)).hover_group().cursor(Cursor::Pointer).on_press(|_: &mut State| {})
}

/// The buttons at the right edge (`SessionActionToolbar.vue`, desktop), and the error lines under them.
fn actions(s: &State, t: &Theme, m: &HeaderModel) -> El {
    let pending = s.toolbar_pending();
    let a = &m.actions;
    let tool = |key, glyph, tip: &str| Tool {
        key,
        glyph,
        tip: tip.into(),
        danger: false,
        off: false,
        spinning: false,
        active: false,
        pressed: false,
        dot: false,
    };
    let mut buttons = row().center_y().gap(2.0);
    // `TerminalToggleButton.vue`: there only while Fleet has terminals on (13.55, 13.87); with no server there is the pretend one.
    if s.terminals_shown() {
        let (open, running) = s.terminal_toggle_state();
        let b = Tool { pressed: open, dot: running, ..tool("terminal", &SQUARE_TERMINAL, &s.terminal_toggle_words()) };
        buttons = buttons.child(toolbar_button(t, b).on_press_tx(|s: &mut State, tx: &mut Tx| s.toggle_terminal(tx)));
    }
    if a.can_abort {
        let b = Tool {
            danger: true,
            off: pending,
            spinning: s.head.bar.aborting.as_deref() == Some(s.session().id.as_str()),
            ..tool("abort", &OCTAGON_X, "Abort")
        };
        buttons = buttons.child(toolbar_button(t, b).on_press_tx(|s: &mut State, tx: &mut Tx| s.toolbar_abort(tx)));
    }
    buttons = buttons.child(el().size(1.0, 16.0).mx(2.0).fill(t.border));
    if a.show_fork(m.archived) {
        let b = Tool { off: pending || !a.can_fork, ..tool("fork", &GIT_FORK, &a.fork_tip()) };
        buttons = buttons.child(toolbar_button(t, b).label(a.fork_label()).on_press_tx(|s: &mut State, tx: &mut Tx| s.toolbar_fork(tx)));
    }
    let has_usage = m.tokens_text != "\u{2014}" || m.cost_text != "\u{2014}";
    let more = Tool { spinning: s.toolbar_busy(), ..tool("more", &ELLIPSIS, "More actions") };
    buttons = buttons.child(toolbar_button(t, more).on_press(|s: &mut State| s.ui.open = Some(crate::ui::Open::Menu("more")))).child(
        toolbar_button(t, Tool { active: has_usage, ..tool("analytics", &CHART_NO_AXES_COLUMN, "Session analytics") })
            .label("Show session analytics")
            .on_press(|s: &mut State| s.ui.open = Some(crate::ui::Open::Popover("analytics"))),
    );
    let errors = s.head.bar.errors(&s.session().id);
    if errors.is_empty() {
        return buttons.ml(10.5);
    }
    // `session-action-toolbar__error`: one `role=alert` line each, 10px, the width of the buttons.
    let mut lines = col().key("toolbar-errors").w(ACTIONS_W - 10.5).gap(2.0).mt(2.0);
    for (i, e) in errors.into_iter().enumerate() {
        lines = lines.child(
            text(e.to_string()).key(format!("toolbar-error-{i}")).role("alert").font(sans(10.0, 400.0, 1.4)).color(t.error_text).wrap(),
        );
    }
    col().ml(10.5).child(buttons).child(lines)
}
