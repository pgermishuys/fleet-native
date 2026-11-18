//! Asking the agent about text on a canvas (`AnnotationPopover.vue`, `use-annotation.ts`, `format-annotation-prompt.ts`,
//! `annotation-types.ts`, and the flow in `SessionsV2RightPanel.vue`, row 9.2 of docs/parity/12-changes-and-review.md).
//!
//! Any canvas that draws text calls `State::open_annotation(anchor_text, position, path)` (Changes' Rendered view does; the file tab
//! of component 11 and the visual tab of component 14 are to). The popover quotes the text, takes a comment, and Send puts the
//! prompt in the open session's draft and sends it, as Fleet's `setText` then `sendPrompt` do.
use crate::{State, Tx};
use rune::{TextEdit, TextSelection};

/// The popover's width, the most it is tall, and its gap from the window's edges (`POPOVER_WIDTH`, `POPOVER_MAX_HEIGHT`, `VIEWPORT_PADDING`).
pub const WIDTH: f32 = 400.0;
pub const MAX_HEIGHT: f32 = 300.0;
pub const PADDING: f32 = 16.0;
/// How much of the text the popover's quote shows, and how much the prompt carries (`truncatedAnchorText`, `extractAnchorText`).
const PREVIEW_CHARS: usize = 120;
const PROMPT_CHARS: usize = 240;

/// The field's key, for focus.
pub const FIELD: &str = "annotation-field";

/// The popover while it is open.
#[derive(Clone, Debug, PartialEq)]
pub struct Open {
    /// The text asked about, as the prompt carries it (cut at 240 characters).
    pub anchor_text: String,
    /// The file it was selected in; empty for none.
    pub path: String,
    /// Where it opens: the pointer's place when the selection was made (window points, before the clamp).
    pub at: (f32, f32),
}

#[derive(Default)]
pub struct Annotation {
    pub open: Option<Open>,
    pub field: TextEdit,
    /// The pointer over the text drawn as Rendered, kept to place the popover where the drag ended (`event.clientX/Y`).
    pub pointer: (f32, f32),
    /// A drag is making a selection; its release opens the popover (`mouseup`), not a selection made by a key or by code.
    pub dragging: bool,
}

/// `text.slice(0, max) + "…"` when longer.
fn cut(text: &str, max: usize) -> String {
    if text.chars().count() <= max { text.to_string() } else { format!("{}…", text.chars().take(max).collect::<String>()) }
}

/// `extractAnchorText`: the text a prompt carries, cut at 240 characters.
pub fn extract_anchor_text(text: &str) -> String {
    cut(text, PROMPT_CHARS)
}

/// `truncatedAnchorText`: the quote in the popover, cut at 120 characters.
pub fn preview(anchor_text: &str) -> String {
    cut(anchor_text, PREVIEW_CHARS)
}

/// `formatAnnotationPrompt`.
pub fn format_annotation_prompt(file_path: &str, anchor_text: &str, user_comment: &str) -> String {
    let header = if file_path.is_empty() { "[Annotation]".to_string() } else { format!("[Annotation on {file_path}]") };
    format!("{header}\n> {anchor_text}\n\n{user_comment}")
}

/// What `submitAnnotation` and the panel's `onSubmit` make of a comment: the comment and its context, in the prompt's frame. (Fleet
/// nests the two, so the quote appears twice: once in the frame and once as the comment's `Context:` line. Kept as it is.)
pub fn annotation_prompt(file_path: &str, anchor_text: &str, comment: &str) -> String {
    let comment = format!("{comment}\n\nContext: {anchor_text}");
    format_annotation_prompt(file_path, anchor_text, &comment)
}

/// `clampedPosition`: the popover's top left kept 16 points inside the window.
pub fn clamp(at: (f32, f32), window: [f32; 2]) -> (f32, f32) {
    let [w, h] = window;
    let (mut x, mut y) = at;
    if x + WIDTH > w - PADDING {
        x = w - WIDTH - PADDING;
    }
    if x < PADDING {
        x = PADDING;
    }
    if y + MAX_HEIGHT > h - PADDING {
        y = h - MAX_HEIGHT - PADDING;
    }
    if y < PADDING {
        y = PADDING;
    }
    (x, y)
}

impl Annotation {
    pub fn can_send(&self) -> bool {
        !self.field.text.trim().is_empty()
    }
}

impl State {
    /// `openAnnotation`: the popover opens over `position`, asking about `anchor_text` selected in `path`, with an empty field.
    pub fn open_annotation(&mut self, anchor_text: &str, position: (f32, f32), path: &str, tx: &mut Tx) {
        self.annotation.open = Some(Open { anchor_text: extract_anchor_text(anchor_text), path: path.to_string(), at: position });
        self.annotation.field = TextEdit::new("");
        tx.focus(FIELD, false);
    }

    /// `closeAnnotation`.
    pub fn close_annotation(&mut self) {
        self.annotation.open = None;
        self.annotation.field = TextEdit::new("");
    }

    /// Send: the comment, trimmed and not empty, becomes the prompt in the draft, and goes.
    pub fn send_annotation(&mut self, tx: &mut Tx) {
        let Some(open) = self.annotation.open.clone() else { return };
        if !self.annotation.can_send() {
            return;
        }
        let comment = self.annotation.field.text.trim().to_string();
        self.close_annotation();
        self.set_draft_text(&annotation_prompt(&open.path, &open.anchor_text, &comment));
        crate::send(self, tx);
    }

    /// A rendered text of `path` hears its selection change. Dragging a selection opens nothing; its release does, with the text
    /// trimmed, and the selection goes (`handleMouseUp`).
    pub fn read_rendered_selection(&mut self, path: &str, selection: Option<&TextSelection>, dragging: bool, tx: &mut Tx) {
        if dragging {
            self.annotation.dragging = true;
            return;
        }
        if !std::mem::take(&mut self.annotation.dragging) {
            return;
        }
        let Some(sel) = selection else { return };
        let text = sel.text().trim().to_string();
        if text.is_empty() {
            return;
        }
        let at = self.annotation.pointer;
        self.open_annotation(&text, at, path, tx);
        tx.clear_text_selection();
    }
}
