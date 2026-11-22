//! A mod's leaves and the tables every element reads: `components/mods/ModPill.vue`, `ModIcon.vue`, `mod-style.ts` and
//! `mod-icons.ts` (component 22, piece B). A view here never holds a colour: a role or a tone is looked up in the theme.
use crate::State;
use crate::conv::message_list::ToolPage;
use crate::fleet::mods::page_url::mod_page_url;
use crate::icons::*;
use crate::theme::{Theme, sans};
use rune::{Color, El, Icon, el, icon, row, text};
use serde_json::{Map, Value};

/// `ROLE_COLORS`: where a mod's colour roles land in Fleet's theme (`good` is `--running`, `warn` is `--idle`, `bad` is
/// `--error`). Nothing for a word that isn't a role.
pub fn role_color(t: &Theme, role: &str) -> Option<Color> {
    Some(match role {
        "text" => t.text,
        "muted" => t.muted,
        "accent" => t.accent,
        "good" => t.running,
        "warn" => t.idle,
        "bad" => t.error,
        _ => return None,
    })
}

/// `TONE_COLORS`: the roles, and `neutral` as the quiet one (`--muted`).
pub fn tone_color(t: &Theme, tone: &str) -> Option<Color> {
    match tone {
        "neutral" => Some(t.muted),
        "good" | "warn" | "bad" | "accent" => role_color(t, tone),
        _ => None,
    }
}

/// `spacePx`: Fleet's spacing steps, 1 is 4 px. Nothing for a value that isn't a step number.
pub fn space_px(step: &Value) -> Option<f32> {
    let n = step.as_f64()?;
    (n.is_finite() && n >= 0.0).then_some(n as f32 * 4.0)
}

/// `str`: a prop as a string, or nothing when it isn't one.
pub fn str_prop<'a>(props: &'a Map<String, Value>, name: &str) -> Option<&'a str> {
    props.get(name)?.as_str()
}

/// `flag`: only `true` is on.
pub fn flag(props: &Map<String, Value>, name: &str) -> bool {
    props.get(name) == Some(&Value::Bool(true))
}

/// `MOD_ICONS`: the 26 icons a mod can name, as Fleet's own (lucide) set.
pub fn mod_icon(name: &str) -> Option<&'static Icon> {
    Some(match name {
        "check" => &CHECK,
        "x" => &X,
        "alert" => &TRIANGLE_ALERT,
        "info" => &INFO,
        "circle" => &CIRCLE,
        "dot" => &DOT,
        "clock" => &CLOCK,
        "loader" => &LOADER_CIRCLE,
        "play" => &PLAY,
        "skip" => &SKIP_FORWARD,
        "test" => &FLASK_CONICAL,
        "bug" => &BUG,
        "terminal" => &TERMINAL,
        "file" => &FILE,
        "folder" => &FOLDER,
        "git-branch" => &GIT_BRANCH,
        "search" => &SEARCH,
        "sparkles" => &SPARKLES,
        "zap" => &ZAP,
        "gauge" => &GAUGE,
        "arrow-right" => &ARROW_RIGHT,
        "chevron-right" => &CHEVRON_RIGHT,
        "external-link" => &EXTERNAL_LINK,
        "copy" => &COPY,
        "eye" => &EYE,
        "eye-off" => &EYE_OFF,
        _ => return None,
    })
}

/// `ModPill`: Fleet's status pill. 22 high, `0 9px`, round, 12 px 500, gap 5, the tone at 14% as the fill and 28% as the
/// border, the text and the 13 px icon in the tone. The label gives way with an ellipsis (`shrinks`) so a pill never makes its
/// row wider than the room; `shrinks` is false where the row wraps instead and only a lone pill may be cut.
pub fn pill(t: &Theme, key: &str, props: &Map<String, Value>, shrinks: bool) -> El {
    let ink = tone_color(t, str_prop(props, "tone").unwrap_or("neutral")).unwrap_or(t.muted);
    let label = str_prop(props, "label").unwrap_or_default().to_string();
    let mut body = row()
        .key(key.to_string())
        .h(22.0)
        .px(9.0)
        .gap(5.0)
        .center_y()
        .radius(999.0)
        .fill(ink.alpha(0.14))
        .border(1.0, ink.alpha(0.28))
        .border_layout()
        .min_w(0.0);
    if shrinks {
        body = body.shrink();
    }
    if let Some(glyph) = str_prop(props, "icon").and_then(mod_icon) {
        body = body.child(icon(glyph, 13.0).color(ink));
    }
    body.child(text(label.clone()).font(sans(12.0, 500.0, 1.3)).color(ink).truncate().shrink().min_w(0.0)).label(label)
}

/// `ModIcon`: one of Fleet's icons by name at 14 px in a role colour (`--text` of the place when none is given). With a
/// `label` it is an image that has that name; without, decoration. `loader` turns in Fleet; Rune has no looping rotation, so
/// it fades as the app's other spinners do (`docs/RUNE.md`).
pub fn icon_el(t: &Theme, key: &str, props: &Map<String, Value>, inherited: Color) -> Option<El> {
    let name = str_prop(props, "name")?;
    let glyph = mod_icon(name)?;
    let ink = str_prop(props, "color").and_then(|role| role_color(t, role)).unwrap_or(inherited);
    let mut e = if name == "loader" { crate::view::files_tab::spinner(ink, 14.0) } else { icon(glyph, 14.0).color(ink) };
    e = e.key(key.to_string());
    let e = match str_prop(props, "label") {
        Some(label) => e.role("img").label(label.to_string()),
        None => e.a11y_hidden(),
    };
    // `inline-flex; align-items: center`: in a row that stretches its items the icon stays 14 px and sits in the middle.
    Some(row().center().child(e))
}

/// A box that takes no room, for what a mod drew and Fleet can't (a tree whose `Fleet` node has no drawing).
pub fn nothing() -> El {
    el()
}

/// `ModPage`: a page the mod ships. Fleet draws it in the conversation's sandboxed frame; a web view can't scroll or clip inside
/// a Rune list (decision 1), so it is the conversation's page card, `contained` (no new tab), whose "Open full size" shows the
/// page from the session's machine. The address comes from the node's owner (`owner`, the host's `mod`), never from the props. One
/// that can't be built is a quiet line.
pub fn page(s: &State, t: &Theme, key: &str, props: &Map<String, Value>, owner: &str) -> El {
    let path = str_prop(props, "path").unwrap_or_default();
    let title = str_prop(props, "title").unwrap_or(path);
    // The address on the machine's own API; the card puts the machine's base before it, as every page's address.
    let address = crate::web::wanted::page_base(s)
        .and_then(|_| mod_page_url(str::to_string, &s.canvas_session(), owner, path, props.get("query").and_then(Value::as_object)));
    match address {
        Some(address) => {
            let page = ToolPage { path: address.clone(), id: format!("mod:{address}"), source: path.to_string() };
            crate::view::page_card::card(s, t, key, title, &page, true)
        }
        None => text(format!("{title}: Page unavailable"))
            .key(key.to_string())
            .font(sans(12.0, 400.0, 1.4))
            .color(t.muted)
            .wrap()
            .wrap_anywhere()
            .shrink()
            .min_w(0.0),
    }
}
