//! The words of the progress strip (`SessionMetadataHeader.vue`): its heading, its current line, its tooltip and how far
//! its bar reaches. Pure: no Rune, no I/O. Fleet's strip also draws a plan (`plan` with groups and steps); the plan is
//! component 10's piece F (`fleet::progress`); the strip draws it when the open session's detail has one.
use super::progress::{Plan, strip_heading, strip_line};
use super::row_chips::Progress;

/// The tab the strip opens, and the one it hides on (the id of the Progress kind).
pub const PROGRESS_TAB: &str = "progress";

/// The strip's first words: the current step's group title, or the plan's title or path with markdown stripped; with no plan,
/// "Todos".
pub fn heading(_progress: &Progress, plan: Option<&Plan>) -> String {
    plan.map_or_else(|| "Todos".to_string(), strip_heading)
}

/// The line after the heading. With a plan: "Next: 4. Title" or "Every step is ticked". Without: the todo being worked on,
/// or "Every todo is done" when the summary names none.
pub fn current_line(progress: &Progress, plan: Option<&Plan>) -> String {
    match plan {
        Some(plan) => strip_line(plan),
        None => progress.current.clone().unwrap_or_else(|| "Every todo is done".into()),
    }
}

/// The tooltip and the accessible label.
pub fn label(progress: &Progress, plan: Option<&Plan>) -> String {
    let unit = if plan.is_some() { "steps" } else { "todos" };
    format!("{} of {} {unit} done. {}. Open the Progress tab", progress.done, progress.total, current_line(progress, plan))
}

/// How much of the top edge the bar covers, from 0 to 1.
pub fn fraction(progress: &Progress) -> f32 {
    if progress.total == 0 { 0.0 } else { (progress.done as f32 / progress.total as f32).min(1.0) }
}

/// `visible`: there is something to count, and the tab that shows it all isn't the open one.
pub fn visible<'a>(progress: Option<&'a Progress>, panel_tab: &str) -> Option<&'a Progress> {
    progress.filter(|p| p.total > 0 && panel_tab != PROGRESS_TAB)
}
