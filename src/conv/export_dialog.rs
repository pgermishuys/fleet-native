//! The export dialog's own state (`ExportMarkdownDialog.vue`): the options, which view, how far the load is and the file it
//! builds. Plain logic; `fleet::export` starts the load and takes the answers, `view::export` draws it.
use super::export::{DEFAULT_EXPORT_OPTIONS, ExportContext, ExportOptions, ExportResult, build_markdown, export_file_name, has_reasoning};
use super::time::local_offset_secs;
use crate::fleet::model::Msg;
use crate::view::conversation::Role;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Load {
    /// The pages that came so far, in messages (`Loading the conversation… {n} messages`).
    Loading(usize),
    Failed(String),
    Ready,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pane {
    /// The Markdown as text.
    Source,
    /// The Markdown drawn as the conversation draws it.
    Preview,
}

pub struct ExportDialog {
    pub session_id: String,
    pub title: String,
    pub harness: Option<String>,
    /// The session's folder, cut off paths when that option is on.
    pub directory: Option<String>,
    pub options: ExportOptions,
    pub pane: Pane,
    pub load: Load,
    pub messages: Vec<Msg>,
    pub built: Option<ExportResult>,
    /// The Copy button says "Copied" until this second on `State::clock`.
    pub copied_until: Option<f64>,
    /// What Copy or Save could not do (`actionError`).
    pub action_error: Option<String>,
    /// Which load the answers belong to: a closed or reopened dialog starts a new one, and an old answer is dropped.
    pub generation: u64,
    /// Set when the dialog closes or loads again, for the thread to stop at its next page (`AbortController`).
    pub cancel: Arc<AtomicBool>,
}

impl ExportDialog {
    pub fn new(session_id: String, title: String, harness: Option<String>, directory: Option<String>, generation: u64) -> ExportDialog {
        ExportDialog {
            session_id,
            title,
            harness,
            directory,
            options: DEFAULT_EXPORT_OPTIONS,
            pane: Pane::Source,
            load: Load::Loading(0),
            messages: Vec::new(),
            built: None,
            copied_until: None,
            action_error: None,
            generation,
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Whether Thinking can be on: only a session with some reasoning in it (`thinkingAvailable`).
    pub fn thinking_available(&self) -> bool {
        self.load == Load::Ready && has_reasoning(&self.messages)
    }

    /// The options as they build the file: a checked Thinking on a session with none stays off (`effective`).
    pub fn effective(&self) -> ExportOptions {
        ExportOptions { thinking: self.options.thinking && self.thinking_available(), ..self.options }
    }

    /// The model in the file's header: the last assistant message that names one.
    pub fn model(&self) -> Option<String> {
        self.messages.iter().rev().find(|m| m.role == Role::Assistant).and_then(|m| m.model_id.clone().filter(|id| !id.is_empty()))
    }

    /// Builds the file again from the messages and the options (`built`); `now` is the export's time, unix milliseconds.
    pub fn rebuild(&mut self, now: i64) {
        self.built = (self.load == Load::Ready).then(|| {
            let context = ExportContext {
                title: self.title.clone(),
                harness: self.harness.clone(),
                model: self.model(),
                directory: self.directory.clone(),
                exported_at: now,
                offset_secs: local_offset_secs(),
            };
            build_markdown(&self.messages, &self.effective(), &context)
        });
    }

    pub fn markdown(&self) -> &str {
        self.built.as_ref().map_or("", |b| b.markdown.as_str())
    }

    pub fn file_name(&self) -> String {
        export_file_name(&self.title, &self.session_id)
    }

    /// `{turns} turn(s) · {size}` over the preview (`export-size`).
    pub fn size_label(&self) -> Option<String> {
        let built = self.built.as_ref()?;
        let size = crate::composer::format::format_bytes(Some(built.markdown.len() as f64));
        Some(format!("{} {} \u{b7} {size}", built.turns, if built.turns == 1 { "turn" } else { "turns" }))
    }

    /// The note under the options when some lines look like keys (`export-key-lines`).
    pub fn key_lines_note(&self) -> Option<String> {
        let n = self.built.as_ref()?.key_lines;
        (n > 0).then(|| {
            format!(
                "{n} {} like a key. This is a hint, not a check. Read the file before you share it.",
                if n == 1 { "line looks" } else { "lines look" }
            )
        })
    }
}
