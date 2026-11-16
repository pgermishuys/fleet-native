//! The words of the progress strip (`SessionMetadataHeader.vue`): its heading, its current line, its tooltip and how far
//! its bar reaches. Pure: no Rune, no I/O. Fleet's strip also draws a plan (`plan` with groups and steps); the plan is
//! component 10's (`lib/session-progress.ts`), so this is the todo form until it lands.
use super::context_tab::PanelTab;
use super::row_chips::Progress;

/// The tab the strip opens, and the one it hides on. Fleet's is the Progress tab (component 10); until that is built the
/// panel's nearest tab, Plan, stands in for it, so the strip shows on the Context tab and hides on Plan.
pub const PROGRESS_TAB: PanelTab = PanelTab::Plan;

/// The strip's bold-less first words: with no plan, always "Todos".
pub fn heading(_progress: &Progress) -> &'static str {
    "Todos"
}

/// The line after the heading: the todo being worked on, or "Every todo is done" when the summary names none.
pub fn current_line(progress: &Progress) -> String {
    progress.current.clone().unwrap_or_else(|| "Every todo is done".into())
}

/// The tooltip and the accessible label.
pub fn label(progress: &Progress) -> String {
    format!("{} of {} todos done. {}. Open the Progress tab", progress.done, progress.total, current_line(progress))
}

/// How much of the top edge the bar covers, from 0 to 1.
pub fn fraction(progress: &Progress) -> f32 {
    if progress.total == 0 { 0.0 } else { (progress.done as f32 / progress.total as f32).min(1.0) }
}

/// `visible`: there is something to count, and the tab that shows it all isn't the open one.
pub fn visible(progress: Option<&Progress>, panel_tab: PanelTab) -> Option<&Progress> {
    progress.filter(|p| p.total > 0 && panel_tab != PROGRESS_TAB)
}
