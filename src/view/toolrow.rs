//! One tool call as a row, and the "Read N files" row that folds reads together. Ports the header of
//! `components/session/ToolCard.vue` (`.tool-header`) and the reads head of `MessageToolList.vue`. The body that opens
//! under the row is `toolcard.rs`; a click on the row opens and closes it.
//!
//! What a call is called, drawn with and summarised as comes from the registry (`conv::tools`); this file only draws.
use super::agent_row;
use super::conversation::{Part, ToolState};
use super::image;
use super::parts::{pulse, triangle};
use super::toolcard;
use crate::State;
use crate::conv::background::{AgentRow, delegation_of};
use crate::conv::card::{Toggle, is_open};
use crate::conv::tools::{Category, ToolIcon, ToolStatus, format_call_duration, get_tool, is_shell_row};
use crate::icons::*;
use crate::theme::{Theme, mono, sans};
use rune::{Cursor, El, Icon, el, icon, row, text};
use std::f32::consts::FRAC_PI_2;

/// The `src/icons.rs` constant for each icon the registry names.
pub fn glyph_of(name: ToolIcon) -> &'static Icon {
    match name {
        ToolIcon::AppWindow => &APP_WINDOW,
        ToolIcon::BookOpen => &BOOK_OPEN,
        ToolIcon::Camera => &CAMERA,
        ToolIcon::CircleCheck => &CIRCLE_CHECK,
        ToolIcon::Code => &CODE,
        ToolIcon::FileText => &FILE_TEXT,
        ToolIcon::Folder => &FOLDER,
        ToolIcon::GitBranch => &GIT_BRANCH,
        ToolIcon::Globe => &GLOBE,
        ToolIcon::Layers => &LAYERS,
        ToolIcon::Lightbulb => &LIGHTBULB,
        ToolIcon::LightbulbOff => &LIGHTBULB_OFF,
        ToolIcon::ListChecks => &LIST_CHECKS,
        ToolIcon::ListTodo => &LIST_TODO,
        ToolIcon::MessageCircleQuestion => &MESSAGE_CIRCLE_QUESTION_MARK,
        ToolIcon::MessagesSquare => &MESSAGES_SQUARE,
        ToolIcon::MonitorUp => &MONITOR_UP,
        ToolIcon::PackageCheck => &PACKAGE_CHECK,
        ToolIcon::PanelsTopLeft => &PANELS_TOP_LEFT,
        ToolIcon::Pencil => &PENCIL,
        ToolIcon::RefreshCw => &REFRESH_CW,
        ToolIcon::Search => &SEARCH,
        ToolIcon::Send => &SEND,
        ToolIcon::Server => &SERVER,
        ToolIcon::Terminal => &TERMINAL,
        ToolIcon::Wrench => &WRENCH,
    }
}

/// `.tool-header`: 30px min, 9px gap, 8px side padding, hover wash 5.
fn header(t: &Theme) -> El {
    row().w_full().min_h(30.0).center_y().gap(9.0).px(8.0).radius(6.0).hover(t.wash(5.0)).cursor(Cursor::Pointer)
}

/// The call's own part, as a row.
pub fn tool_entry(t: &Theme, s: &State, part: &Part) -> El {
    let Part::Tool { id, tool, call_id, detail, status, state, body, agent } = part else { return el() };
    // A sub-agent call whose session exists is one row of its own: who, what, how it's going, and a way in.
    if get_tool(tool).category == Category::Subagent
        && let Some(delegation) = s.stream.as_ref().and_then(|st| delegation_of(&st.delegations, call_id, id))
        && let Some(row) = AgentRow::of(agent, delegation)
    {
        return agent_row::agent_row(t, id, &row);
    }
    let open = is_open(tool, body, s.inline_diffs, s.tool_open.get(id.as_str()).copied());
    let under = body.initially_collapsed;
    let key = id.clone();
    let header = tool_row(t, id, tool, detail, *status, *state).on_press(move |s: &mut State| {
        s.tool_open.insert(key.clone(), Toggle { open: !open, under });
    });
    let mut card = rune::col().w_full().child(header);
    // A failed call keeps what it printed, one line, under its closed row.
    if let (false, ToolStatus::Error, Some(line)) = (open, *status, body.preview.as_deref()) {
        card = card.child(toolcard::preview(t, id, line));
    }
    if open {
        card = card.child(toolcard::body(t, s, id, tool, body));
    }
    // What the agent saw sits under the call, open or folded.
    if let Some(shot) = &body.screenshot {
        card = card.child(image::screenshot(t, s, id, shot, detail));
    }
    card
}

/// `.tool-header__background`: 11px, 500, uppercase, 0.02em apart.
fn status_word(word: &'static str, ink: rune::Color) -> El {
    text(word).font(sans(11.0, 500.0, 1.3).tracking(0.22)).color(ink)
}

pub fn tool_row(t: &Theme, id: &str, kind: &str, detail: &str, status: ToolStatus, state: ToolState) -> El {
    let tool = get_tool(kind);
    // Reads are quieter than commands and edits.
    let (weight, ink) = if tool.category == Category::Read { (400.0, t.muted) } else { (500.0, t.text) };
    let count = mono(12.0, 400.0, 1.3).features("'tnum' 1");
    // A call with no path yet is titled with its own name, which the verb already says.
    let shown = if detail.eq_ignore_ascii_case(kind) { "" } else { detail };
    // Copilot's turn row and HydraFusion's pass rows are a sentence of facts, not a path: they wrap rather than clip.
    let wraps = tool.name == "copilot_turn" || tool.name.starts_with("hydrafusion_");
    let mut line = header(t).key(format!("tool-{id}"));
    if wraps {
        line = line.py(6.0);
    }
    line = line
        .child(icon(glyph_of(tool.icon), 14.0).color(t.muted.alpha(0.75)))
        .child(text(tool.heading.clone()).font(sans(13.0, weight, 1.3)).color(ink));
    let spacer = || el().grow(1.0);
    line = if tool.pattern_label && !shown.is_empty() {
        // `.tool-header__pattern`: the pattern in a pill.
        line.child(
            text(shown.to_string())
                .font(mono(12.0, 500.0, 1.3))
                .color(t.accent_text)
                .truncate()
                .px(8.0)
                .py(1.0)
                .radius(6.0)
                .fill(t.accent.alpha(0.08))
                .border(1.0, t.accent.alpha(0.25))
                .border_layout()
                .shrink(),
        )
        .child(spacer())
    } else if wraps {
        line.child(text(shown.to_string()).font(mono(12.0, 400.0, 1.3)).color(t.muted).wrap().grow(1.0).shrink())
    } else {
        line.child(text(shown.to_string()).font(mono(12.0, 400.0, 1.3)).color(t.muted).truncate().grow(1.0))
    };
    match status {
        ToolStatus::Running => line.child(pulse(t.running)),
        ToolStatus::Background => line.child(row().gap(6.0).center_y().child(status_word("BACKGROUND", t.running)).child(pulse(t.running))),
        ToolStatus::Error => line.child(triangle(t.error)),
        ToolStatus::Stopped => line.child(status_word("STOPPED", t.muted)),
        _ => match state {
            // `+N −N`, and nothing when the diff only moved context.
            ToolState::Diff(added, removed) if added + removed > 0 => line.child(
                row()
                    .gap(6.0)
                    .child(text(format!("+{added}")).font(count).color(t.running_text))
                    .child(text(format!("\u{2212}{removed}")).font(count).color(t.error_text)),
            ),
            ToolState::Took(secs) if is_shell_row(kind) => match format_call_duration(Some(f64::from(secs) * 1000.0)).as_str() {
                "" => line,
                took => line.child(text(took.to_string()).font(count).color(t.muted)),
            },
            _ => line,
        },
    }
}

/// `Read N files` (`MessageToolList.vue`, `.reads-head`): the folded reads, closed until asked. Open, each read is its
/// own row, indented 14px.
pub fn reads_entry(t: &Theme, s: &State, id: &str, tools: &[&Part], open: bool) -> El {
    let working = tools.iter().any(|p| matches!(p, Part::Tool { status: ToolStatus::Running | ToolStatus::Pending, .. }));
    let key = id.to_string();
    let mut head = header(t)
        .key(format!("reads-{id}"))
        .on_press(move |s: &mut State| {
            if !s.open_reads.remove(&key) {
                s.open_reads.insert(key.clone());
            }
        })
        .child(icon(&FILE_TEXT, 14.0).color(t.muted.alpha(0.75)))
        .child(text(format!("Read {} files", tools.len())).font(sans(13.0, 400.0, 1.3)).color(t.muted));
    head = head.child(el().grow(1.0));
    if working {
        head = head.child(pulse(t.running_text));
    }
    head = head.child(icon(&CHEVRON_RIGHT, 12.0).color(t.muted.alpha(0.75)).rotate(if open { FRAC_PI_2 } else { 0.0 }));
    let list = rune::col().w_full().child(head);
    if open { list.child(rune::col().w_full().pl(14.0).children(tools.iter().map(|p| tool_entry(t, s, p)))) } else { list }
}
