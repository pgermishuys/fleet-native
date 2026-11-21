//! The parts of the mods contract the app needs (port of `lib/mods/types.ts`): the sites, the elements, the names a
//! prop may take, the limits, and what a control sends back. Pinned to weave-fleet `c1a48bd` by
//! `tests::mods_validate::the_lists_and_limits_are_the_ones_of_c1a48bd`.

/// The places a mod can draw.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Site {
    ToolUse,
    ToolResult,
    ComposerBand,
    StatusChip,
    Pane,
}

pub const SITES: [Site; 5] = [Site::ToolUse, Site::ToolResult, Site::ComposerBand, Site::StatusChip, Site::Pane];

impl Site {
    /// The name on the wire and in messages.
    pub fn name(self) -> &'static str {
        match self {
            Site::ToolUse => "ToolUse",
            Site::ToolResult => "ToolResult",
            Site::ComposerBand => "ComposerBand",
            Site::StatusChip => "StatusChip",
            Site::Pane => "Pane",
        }
    }

    /// Sites that take inline elements only: `Text`, `Pill`, `Icon`, `Button`, and a row `Box` holding only those.
    pub fn inline(self) -> bool {
        matches!(self, Site::ToolUse | Site::StatusChip)
    }
}

pub const ELEMENT_TYPES: [&str; 10] = ["Box", "Text", "Pill", "Icon", "Button", "Input", "Select", "Markdown", "Code", "Page"];

pub const ICON_NAMES: [&str; 26] = [
    "check",
    "x",
    "alert",
    "info",
    "circle",
    "dot",
    "clock",
    "loader",
    "play",
    "skip",
    "test",
    "bug",
    "terminal",
    "file",
    "folder",
    "git-branch",
    "search",
    "sparkles",
    "zap",
    "gauge",
    "arrow-right",
    "chevron-right",
    "external-link",
    "copy",
    "eye",
    "eye-off",
];

pub const COLOR_ROLES: [&str; 6] = ["text", "muted", "accent", "good", "warn", "bad"];
pub const TONES: [&str; 5] = ["good", "warn", "bad", "neutral", "accent"];

/// Spacing in Fleet's steps: 1 is 4 px.
pub const SPACES: [u8; 7] = [0, 1, 2, 3, 4, 6, 8];

/// The callbacks a control can hold, as handle names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandleName {
    OnPress,
    OnSubmit,
    OnInput,
    OnSelect,
}

impl HandleName {
    pub fn name(self) -> &'static str {
        match self {
            HandleName::OnPress => "onPress",
            HandleName::OnSubmit => "onSubmit",
            HandleName::OnInput => "onInput",
            HandleName::OnSelect => "onSelect",
        }
    }
}

/// What a control hands on when it is used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionKind {
    Press,
    Input,
    Submit,
    Select,
}

/// A control was used: what a site hands on (posted to `/api/sessions/{id}/mods/action`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Action {
    pub handle: String,
    pub kind: ActionKind,
    pub value: Option<String>,
}

/// Where the user was when they used a control.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    Desktop,
    Phone,
}

/// A mod that had a hand in a tree, outermost first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Author {
    pub name: String,
    pub draft: bool,
}

/// Text drawn per tree (UTF-16 units, as Fleet counts them); the rest is cut.
pub const TREE_TEXT_CHARS: usize = 100_000;
pub const TREE_NODES: usize = 2_000;
pub const TREE_DEPTH: usize = 32;
/// A tree as JSON. Larger is invalid.
pub const TREE_BYTES: usize = 262_144;
/// `Input` changes sent per field.
pub const INPUT_CHANGES_PER_SECOND: u32 = 4;
pub const SELECT_OPTIONS_MAX: usize = 200;
/// Keys and pane ids.
pub const KEY_CHARS: usize = 64;
pub const PAGE_QUERY_BYTES: usize = 4_096;

/// A mod's id on the wire: its name, then `@v` and a version for a kept mod or `@draft:` and the session for a draft
/// (`MOD_ID_PATTERN` in `lib/mods/page-url.ts`). Session ids are letters, digits, `_` and `-`, 1 to 128 of them.
pub fn is_mod_id(id: &str) -> bool {
    let Some((name, rest)) = id.split_once('@') else { return false };
    let name = name.as_bytes();
    let name_ok = (1..=64).contains(&name.len())
        && name[0].is_ascii_lowercase()
        && name[1..].iter().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-');
    if !name_ok {
        return false;
    }
    if let Some(version) = rest.strip_prefix('v') {
        let v = version.as_bytes();
        return !v.is_empty() && (b'1'..=b'9').contains(&v[0]) && v.iter().all(u8::is_ascii_digit);
    }
    match rest.strip_prefix("draft:") {
        Some(session) => (1..=128).contains(&session.len()) && session.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-'),
        None => false,
    }
}
