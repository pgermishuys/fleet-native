//! What a tool card holds beyond its row: the output it prints, the one-line preview under a failed call, and whether
//! the card is open. Ports `toToolCardItem` (`getToolOutput`, `buildPreview`) from `activity-stream-tool-card.ts` and
//! the open/closed rules of `ToolCard.vue`. No Rune in here.
use super::diff::{DiffLine, card_diff_lines, file_edits_of};
use super::output::parse_tool_output;
use super::tools::{is_shell_row, is_write_call, shell_header, tool_duration_ms};
use serde_json::{Map, Value};

/// The keys of a tool's state that hold what it printed, first one present wins.
const OUTPUT_KEYS: [&str; 7] = ["output", "result", "content", "error", "message", "stdout", "stderr"];
/// What the fallback (the whole state as JSON) leaves out: it is shown elsewhere.
const FALLBACK_EXCLUDED: [&str; 7] = ["input", "status", "summary", "title", "diff", "diffLines", "patch"];

/// A string as it is, anything else pretty-printed JSON with 2 spaces (`stringifyToolValue`).
fn stringify(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => serde_json::to_string_pretty(other).unwrap_or_default(),
    }
}

/// What the call printed: the first of `output|result|content|error|message|stdout|stderr` that is set, else the state
/// less the keys shown elsewhere, as JSON, if anything is left (`getToolOutput`).
pub fn get_tool_output(state: &Value) -> Option<String> {
    let state = state.as_object()?;
    for key in OUTPUT_KEYS {
        if let Some(value) = state.get(key).filter(|v| !v.is_null()) {
            return Some(stringify(value));
        }
    }
    if state.is_empty() {
        return None;
    }
    let rest: Map<String, Value> =
        state.iter().filter(|(k, _)| !FALLBACK_EXCLUDED.contains(&k.as_str())).map(|(k, v)| (k.clone(), v.clone())).collect();
    (!rest.is_empty()).then(|| stringify(&Value::Object(rest)))
}

/// The line under a failed call (`buildPreview`): `└ ` and the first line with text once its terminal codes are gone,
/// cut to 80 characters, with `(N lines)` when the output has more than one; else the summary.
pub fn build_preview(output: Option<&str>, summary: Option<&str>) -> Option<String> {
    let from_summary = || summary.filter(|s| !s.is_empty()).map(|s| format!("└ {s}"));
    let Some(output) = output.filter(|o| !o.is_empty()) else { return from_summary() };
    // Trailing line ends aren't lines: the count agrees with the open row's.
    let lines: Vec<&str> = output.trim_end_matches('\n').split('\n').collect();
    // The first line with text once its terminal codes are gone; only that far is parsed.
    let first = lines.iter().filter(|l| !l.trim().is_empty()).map(|l| parse_tool_output(l).plain).find(|plain| !plain.trim().is_empty());
    let Some(first) = first else { return from_summary() };
    let shown = if first.chars().count() > 80 { format!("{}…", first.chars().take(79).collect::<String>()) } else { first };
    Some(if lines.len() > 1 { format!("└ {shown} ({} lines)", lines.len()) } else { format!("└ {shown}") })
}

/// Everything a card draws under its row, boiled down from the call's state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ToolBody {
    /// `state.summary`: a short paragraph above the output.
    pub summary: String,
    pub output: String,
    /// A shell call's one header, "bun test · exit 0 · 0.1 s"; `None` for other kinds.
    pub header: Option<String>,
    /// The line under a failed call's row.
    pub preview: Option<String>,
    /// What the card shows as its diff (`diffLines` of the card item): what the harness attached, else for a write the
    /// lines worked out from its input (`withFileEdit`).
    pub diff: Vec<DiffLine>,
    pub has_diff: bool,
    /// Only a call that failed starts open.
    pub initially_collapsed: bool,
    /// The screenshot Fleet kept for the call, which shows under it even while the row is folded.
    pub screenshot: Option<super::image_sniff::Shot>,
    /// A canvas the call opened or changed, which the row offers to show; never for a retired diagram call.
    pub canvas_id: Option<String>,
    /// The output is a document or page the agent rendered: the box shows it instead of the output (`parseVisualPayload`).
    pub visual: Option<crate::fleet::canvases::VisualPayload>,
    /// The page the call showed in the conversation, drawn as a card after the message's calls (`toolPage`).
    pub page: Option<super::message_list::ToolPage>,
    /// What names the page: the call's title (`state.title`, else `input.title`).
    pub page_title: String,
}

/// The card's diff lines (`cardDiffLines`, then `withFileEdit`'s fallback for write kinds).
fn card_diff(tool: &str, state: &Value) -> Vec<DiffLine> {
    let harness = card_diff_lines(state);
    if !harness.is_empty() || !is_write_call(tool) {
        return harness;
    }
    file_edits_of(tool, state).into_iter().next().map(|e| e.diff).unwrap_or_default()
}

/// `metadata.canvasId`, unless the call is a retired diagram tool (`toToolCardItem`).
pub fn canvas_id_of(tool: &str, state: &Value) -> Option<String> {
    if super::tools::is_retired_canvas_tool(tool) {
        return None;
    }
    state.pointer("/metadata/canvasId").and_then(Value::as_str).filter(|c| !c.is_empty()).map(str::to_string)
}

pub fn tool_body(tool: &str, title: &str, state: &Value) -> ToolBody {
    let output = get_tool_output(state);
    let summary = state.get("summary").and_then(Value::as_str).unwrap_or("").to_string();
    let meta = |k: &str| state.pointer(&format!("/metadata/{k}")).and_then(Value::as_i64);
    let header = is_shell_row(tool).then(|| {
        let input = state.get("input").unwrap_or(&Value::Null);
        shell_header(input, title, meta("exit").or_else(|| meta("exitCode")), tool_duration_ms(state))
    });
    let diff = card_diff(tool, state);
    let visual = output.as_deref().and_then(super::visual::parse_visual_payload);
    ToolBody {
        has_diff: !diff.is_empty(),
        diff,
        preview: build_preview(output.as_deref(), Some(&summary)),
        output: output.unwrap_or_default(),
        summary,
        header,
        initially_collapsed: state.get("status").and_then(Value::as_str) != Some("error"),
        screenshot: super::image_sniff::tool_screenshot(state),
        canvas_id: canvas_id_of(tool, state),
        visual,
        page: super::message_list::tool_page(state),
        page_title: ["/title", "/input/title"]
            .iter()
            .find_map(|p| state.pointer(p).and_then(Value::as_str).filter(|t| !t.is_empty()))
            .unwrap_or(title)
            .to_string(),
    }
}

/// A click on a card's header, remembered with the `initially_collapsed` it was made under: a toggle sticks until the
/// call's own state changes (a running call that fails), as `ToolCard.vue`'s watch resets it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Toggle {
    pub open: bool,
    pub under: bool,
}

/// Whether the card is open (`ToolCard.vue` `isCollapsed`). A write opens on what it wrote; any other row shows a
/// diff only when the user turned inline diffs on, and then opens by itself.
pub fn is_open(tool: &str, body: &ToolBody, inline_diffs: bool, user: Option<Toggle>) -> bool {
    let write = is_write_call(tool);
    let show_diff = body.has_diff && (write || inline_diffs);
    if let Some(t) = user.filter(|t| t.under == body.initially_collapsed) {
        return t.open;
    }
    if body.initially_collapsed { !write && show_diff } else { true }
}

/// Whether to show a diff in the body at all: a write always, other rows when inline diffs are on.
pub fn shows_diff(tool: &str, body: &ToolBody, inline_diffs: bool) -> bool {
    body.has_diff && (is_write_call(tool) || inline_diffs)
}
