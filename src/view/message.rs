//! The frame of a message: the user's bubble with its chips, the marks above it, the copy button and the time pill that
//! show on hover. Ports `components/session/MessageBubble.vue` and the `.peer-from` marks of `ActivityStream.vue`; the
//! rules (what the marks say, who shows a pill) are `conv::bubble`, and the note of attached files is `conv::attached`.
//!
//! Not drawn here yet: image thumbnails (piece N), `@` session and `$` skill chips and file links in running text (Rune's
//! `Markdown` has no inline-chip hook; `docs/RUNE.md`), the workflow and peer marks (other components), `message--shown`.
use super::conversation::{Message, Part, Role, Styles};
use super::prose;
use super::toolcard::mix;
use crate::State;
use crate::composer::choice::model_display_name;
use crate::conv::attached::{AttachedFileRef, file_extension_label, parse_attached_note};
use crate::conv::bubble::{STEERED_TEXT, STEERED_TIP, has_expanded_prompt, looks_like_markdown, mod_mark_text};
use crate::conv::references::format_slash_command;
use crate::conv::time::{format_absolute_timestamp, format_relative_time, local_offset_secs};
use crate::fleet::model::SlashCommand;
use crate::icons::*;
use crate::theme::{RADIUS_BTN, RADIUS_PANEL, Theme, mono, sans};
use rune::{Cursor, El, Icon, Tx, col, el, icon, row, text};

/// How long "Copied" shows in place of the copy icon.
pub const COPIED_SECONDS: f64 = 1.5;

/// What the message says when copied: the command as typed, else the text as written (`copyMessage`).
pub fn copy_text(msg: &Message) -> String {
    if let Some(command) = &msg.meta.command {
        return format_slash_command(&command.name, command.arguments.as_deref());
    }
    msg.text()
}

/// Wall-clock time in unix milliseconds; a scene fixes it so its picture reads the same every time.
pub fn now_ms(s: &State) -> i64 {
    s.now_ms.unwrap_or_else(|| std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64))
}

/// `.peer-from`: the pill above a bubble that says where it came from.
fn chip(t: &Theme, key: &str, glyph: &'static Icon, label: String, tip: &str) -> El {
    row()
        .key(key)
        .gap(5.0)
        .center_y()
        .max_w(380.0)
        .px(10.0)
        .py(4.0)
        .radius(999.0)
        .border(1.0, t.border.alpha(t.border.a * 0.9))
        .border_layout()
        .fill(mix(t.card_bg, t.accent_dim, 0.04))
        .help(tip)
        .child(icon(glyph, 11.9).color(t.muted))
        .child(text(label).font(sans(10.5, 600.0, 1.5)).color(t.text).truncate())
}

/// The marks above a user's bubble (rows 1.55, 1.56 / 2.15, 2.16), right-aligned with it.
pub fn marks(t: &Theme, msg: &Message, lead: f32) -> Option<El> {
    if msg.role != Role::User || (msg.meta.mod_mark.is_none() && !msg.meta.steered) {
        return None;
    }
    // Fleet's chips end in a 6px margin and the bubble's own 2px padding follows: 8px to the bubble.
    let mut marks = col().items_end().w_full().mt(lead).gap(6.0).mb(6.0);
    if let Some(m) = &msg.meta.mod_mark {
        let (label, tip) = mod_mark_text(m.kind, &m.name);
        marks = marks.child(chip(t, "mod-mark", &PUZZLE, label, tip));
    }
    if msg.meta.steered {
        marks = marks.child(chip(t, "steered-mark", &CORNER_DOWN_RIGHT, STEERED_TEXT.to_string(), STEERED_TIP));
    }
    Some(marks)
}

/// `.msg-copy-btn`: 26px square, hidden until the message is hovered, "Copied" for 1.5s after a press.
pub fn copy_button(t: &Theme, s: &State, msg: &Message) -> El {
    let id = msg.id.clone();
    let copied = s.copied.as_ref().is_some_and(|(m, until)| *m == msg.id && s.clock < *until);
    let face = if copied { text("Copied").font(sans(9.0, 500.0, 1.0)).color(t.text) } else { text("").font(sans(9.0, 500.0, 1.0)) };
    let inner = el()
        .key(format!("copy-{}", msg.id))
        .size(26.0, 26.0)
        .center()
        .radius(RADIUS_BTN)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.card_bg)
        .hover_group()
        .cursor(Cursor::Pointer)
        .help(if copied { "Copied" } else { "Copy message" })
        .label("Copy message")
        .on_press_tx(move |s: &mut State, tx: &mut Tx| {
            let Some(msg) = s.session().messages.iter().find(|m| m.id == id) else { return };
            tx.copy(copy_text(msg));
            s.copied = Some((id.clone(), s.clock + COPIED_SECONDS));
            tx.wake_after(std::time::Duration::from_millis(100));
        });
    let inner = if copied { inner.child(face) } else { inner.child(icon(&COPY, 13.0).color(t.muted).group_color(t.text)) };
    el().group_opacity(0.0, 1.0).child(inner)
}

/// `.msg-meta` of an assistant message: a pill in the 20px gap below it with the model and how long ago.
pub fn meta_pill(t: &Theme, s: &State, msg: &Message) -> Option<El> {
    let created = msg.meta.created_at?;
    let now = now_ms(s);
    let offset = local_offset_secs();
    let mut pill = row()
        .key(format!("meta-{}", msg.id))
        .h(20.0)
        .px(9.0)
        .gap(7.0)
        .center_y()
        .radius(999.0)
        .border(1.0, t.border)
        .border_layout()
        .fill(t.card_bg)
        .help(format_absolute_timestamp(created, offset))
        .group_opacity(0.0, 1.0)
        .paint_order(1);
    for shadow in t.sheet_shadows() {
        pill = pill.box_shadow(shadow);
    }
    let font = sans(11.0, 400.0, 1.0).features("'tnum' 1");
    let model = model_display_name(msg.meta.model.as_deref(), &s.lists().models);
    if !model.is_empty() {
        pill = pill
            .child(el().size(5.0, 5.0).radius(2.5).fill(t.accent.alpha(0.75)))
            .child(text(model).font(font).color(t.text.alpha(0.72)))
            .child(text("\u{b7}").font(font).color(t.muted.alpha(0.65)));
    }
    pill = pill.child(text(format_relative_time(created, now, offset)).font(font).color(t.muted));
    Some(el().pin(Some(0.0), None, None, Some(-20.0)).child(pill))
}

/// A user's pill has no chrome: the time in plain muted text to the left of the bubble.
fn user_time(t: &Theme, s: &State, msg: &Message) -> El {
    let time = msg.meta.created_at.map(|created| {
        text(format_relative_time(created, now_ms(s), local_offset_secs()))
            .font(sans(11.0, 400.0, 1.0).features("'tnum' 1"))
            .color(t.muted)
            .mr(10.0)
            .mb(6.0)
            .group_opacity(0.0, 1.0)
            .help(format_absolute_timestamp(msg.meta.created_at.unwrap_or(0), local_offset_secs()))
    });
    // The empty 18% in front of the bubble keeps it to Fleet's `max-width: 82%`; the time sits in it.
    row().w_frac(0.18).justify_end().items_end().children(time)
}

/// `.msg-command`: "/name arguments", the command in the accent colour.
fn command_line(t: &Theme, command: &SlashCommand) -> El {
    let mut line = row().key("message-command").gap(0.0).flex_wrap();
    line = line.child(text(format!("/{}", command.name)).font(mono(13.8, 600.0, 1.55)).color(t.accent_text));
    if let Some(args) = command.arguments.as_deref().filter(|a| !a.is_empty()) {
        line = line.child(text(format!(" {args}")).font(sans(15.0, 400.0, 1.55)).color(t.text).wrap());
    }
    line
}

/// "Show prompt" / "Hide prompt" and the expanded template behind it (row 2.21).
fn prompt_toggle(t: &Theme, s: &State, md: &Styles, msg: &Message, body: &str) -> El {
    let open = s.prompt_open.contains(&msg.id);
    let id = msg.id.clone();
    let label = if open { "Hide prompt" } else { "Show prompt" };
    let toggle = row()
        .key("message-command-toggle")
        .gap(2.0)
        .center_y()
        .mt(4.0)
        .ml(-2.0)
        .cursor(Cursor::Pointer)
        .hover_group()
        .label(label)
        .on_press(move |s: &mut State| {
            if !s.prompt_open.remove(&id) {
                s.prompt_open.insert(id.clone());
            }
        })
        .child(icon(&CHEVRON_RIGHT, 11.9).color(t.muted).group_color(t.text).rotate(if open { std::f32::consts::FRAC_PI_2 } else { 0.0 }))
        .child(text(label).font(sans(10.5, 400.0, 1.3)).color(t.muted).group_color(t.text));
    let mut block = col().w_full().child(toggle);
    if open {
        block = block.child(
            col().key("message-command-prompt").w_full().mt(8.0).pt(8.0).border_sides([1.0, 0.0, 0.0, 0.0], t.border).child(prose::prose(
                &md.prose,
                &format!("{}-prompt", msg.id),
                body,
                false,
            )),
        );
    }
    block
}

fn file_chip(t: &Theme, file: &AttachedFileRef) -> El {
    row()
        .key(format!("attached-{}", file.path))
        .gap(8.0)
        .center_y()
        .px(8.0)
        .py(3.0)
        .radius(RADIUS_BTN)
        .border(1.0, t.border)
        .border_layout()
        .cursor(Cursor::Pointer)
        .hover(t.wash(6.0))
        .help(file.path.clone())
        .label(file.name.clone())
        // Opening the file belongs to the content panel (component 10); the chip is drawn and pressable.
        .on_press(|_: &mut State| {})
        .child(
            text(file_extension_label(&file.name))
                .font(mono(9.0, 400.0, 1.55))
                .color(t.muted)
                .px(4.0)
                .radius(3.0)
                .border(1.0, t.border)
                .border_layout(),
        )
        .child(text(file.name.clone()).font(sans(12.0, 400.0, 1.55)).color(t.text).truncate())
        .child(text(file.details.clone()).font(sans(11.0, 400.0, 1.55)).color(t.muted))
}

/// The chips under the typed text and the note the agent got, kept visible (rows 2.22, 2.23).
fn attachments(t: &Theme, files: &[AttachedFileRef]) -> El {
    let mut chips = row().w_full().mt(8.0).gap(6.0).flex_wrap();
    let mut note = col().mt(8.0).pl(8.0).border_sides([0.0, 0.0, 0.0, 2.0], t.border);
    for file in files {
        chips = chips.child(file_chip(t, file));
        note = note.child(text(format!("Attached: {} ({})", file.path, file.details)).font(mono(11.0, 400.0, 1.5)).color(t.muted).wrap());
    }
    col().w_full().child(chips).child(note)
}

/// A user's text part: the bubble, right-aligned, with the small corner at bottom right (rows 2.2, 2.3, 2.20-2.23).
pub fn user_bubble(t: &Theme, s: &State, md: &Styles, msg: &Message, id: &str, body: &str) -> El {
    let (typed, files) = match &msg.meta.command {
        Some(_) => (body.to_string(), Vec::new()),
        None => parse_attached_note(body),
    };
    let mut content = col().w_full();
    match &msg.meta.command {
        Some(command) => {
            content = content.child(command_line(t, command));
            if has_expanded_prompt(body, command) {
                content = content.child(prompt_toggle(t, s, md, msg, body));
            }
        }
        None if typed.trim().is_empty() => {}
        None if looks_like_markdown(&typed) => {
            content = content.child(prose::prose_marked(&md.prose, id, &typed, false, s.find_marks(id, false).as_ref()))
        }
        None => {
            let marks = s.find_marks(id, false).map(|m| super::marks::over(t, &typed, &m)).unwrap_or_default();
            content = content.child(text(typed).font(sans(15.0, 400.0, 1.55)).color(t.text).wrap().selectable(true).text_highlights(marks));
        }
    }
    if !files.is_empty() {
        content = content.child(attachments(t, &files));
    }
    // The images ride in the bubble of the first words (or of nothing, when there are none).
    let first_words = msg.parts.iter().find_map(|p| if let Part::Text { id, .. } = p { Some(id.as_str()) } else { None });
    if first_words.unwrap_or("") == id {
        let images: Vec<&Part> = msg.parts.iter().filter(|p| super::conversation::is_image(p)).collect();
        if !images.is_empty() {
            content = content.child(super::image::user_images(t, s, &images));
        }
    }
    let bubble = col()
        .key(format!("bubble-{}", msg.id))
        .shrink()
        .px(14.0)
        .py(10.0)
        .corners([RADIUS_PANEL + 2.0, RADIUS_PANEL + 2.0, 4.0, RADIUS_PANEL + 2.0])
        .border(1.0, t.border)
        .border_layout()
        .fill(t.wash(5.0))
        .child(content)
        .child(copy_button(t, s, msg).pin(Some(-34.0), Some(4.0), None, None));
    row().w_full().justify_end().child(user_time(t, s, msg)).child(bubble)
}
