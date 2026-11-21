//! Checks a mod's tree against the contract (port of `lib/mods/validate.ts`): elements and props, keys, handles, the
//! inline rule for a site, and the limits. Anything wrong makes the whole tree invalid. Text past the budget is cut
//! instead, in a copy; the input is untouched.
//!
//! Differences from the TypeScript, all because a `serde_json::Value` can't hold what they guard against: a prop that is
//! `undefined` or a function, a cyclic tree, a `BigInt` and an object with a prototype can't exist here, so those
//! cases of `validate.test.ts` have no Rust twin (`docs/parity/22-mods.md`, 1.x). Text is counted in UTF-16 units like
//! JavaScript's `length`, and a cut never splits a character.
use super::types::{
    COLOR_ROLES, ELEMENT_TYPES, ICON_NAMES, PAGE_QUERY_BYTES, SELECT_OPTIONS_MAX, SPACES, Site, TONES, TREE_BYTES, TREE_DEPTH, TREE_NODES,
    TREE_TEXT_CHARS, is_mod_id,
};
use serde_json::{Map, Value};
use std::borrow::Cow;
use std::collections::HashSet;

/// The answer: the tree to draw (the input itself when nothing was cut) or the reason it can't be drawn.
#[derive(Debug, PartialEq)]
pub enum Check<'a> {
    Ok { tree: Cow<'a, Value>, text_cut: bool },
    Invalid { reason: String },
}

impl Check<'_> {
    pub fn is_ok(&self) -> bool {
        matches!(self, Check::Ok { .. })
    }
}

/// How one prop is checked. `check` returns the problem, or `None` when the value is fine.
#[derive(Clone, Copy)]
enum Prop {
    Str(&'static [&'static str]),
    Num(&'static [u8]),
    Bool,
    Text,
    Key,
    Width,
    Line,
    Options,
    Query,
}

const SPACE: Prop = Prop::Num(&SPACES);
const COLOR: Prop = Prop::Str(&COLOR_ROLES);
const ICON: Prop = Prop::Str(&ICON_NAMES);
const WRAP: Prop = Prop::Str(&["wrap", "truncate"]);

fn list<T: ToString>(allowed: &[T]) -> String {
    allowed.iter().map(T::to_string).collect::<Vec<_>>().join(", ")
}

impl Prop {
    fn check(self, v: &Value) -> Option<String> {
        match self {
            Prop::Str(allowed) => match v.as_str() {
                Some(s) if allowed.contains(&s) => None,
                _ => Some(format!("must be one of {}", list(allowed))),
            },
            Prop::Num(allowed) => match v.as_f64() {
                Some(n) if allowed.iter().any(|a| f64::from(*a) == n) => None,
                _ => Some(format!("must be one of {}", list(allowed))),
            },
            Prop::Text => (!v.is_string()).then(|| "must be a string".into()),
            Prop::Bool => (!v.is_boolean()).then(|| "must be true or false".into()),
            Prop::Key => match v.as_str() {
                Some(s) if is_key(s) => None,
                _ => Some("must be 1 to 64 letters, digits, _, - or .".into()),
            },
            Prop::Width => match v.as_str().and_then(percent) {
                Some(n) if n <= 100.0 => None,
                _ => Some("must be a percentage from \"0%\" to \"100%\"".into()),
            },
            Prop::Line => match v.as_f64() {
                Some(n) if n.fract() == 0.0 && n >= 1.0 => None,
                _ => Some("must be a positive integer".into()),
            },
            Prop::Options => options(v),
            Prop::Query => query(v),
        }
    }
}

/// `^[A-Za-z0-9_.-]{1,64}$`
fn is_key(s: &str) -> bool {
    (1..=64).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
}

/// `^(\d+(?:\.\d+)?)%$`, ASCII digits only.
fn percent(s: &str) -> Option<f64> {
    let n = s.strip_suffix('%')?;
    let (whole, frac) = match n.split_once('.') {
        Some((w, f)) => (w, Some(f)),
        None => (n, None),
    };
    let digits = |d: &str| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit());
    if !digits(whole) || frac.is_some_and(|f| !digits(f)) {
        return None;
    }
    n.parse().ok()
}

fn options(v: &Value) -> Option<String> {
    let Some(list) = v.as_array().filter(|a| (1..=SELECT_OPTIONS_MAX).contains(&a.len())) else {
        return Some(format!("must be 1 to {SELECT_OPTIONS_MAX} options"));
    };
    let mut seen = HashSet::new();
    for option in list {
        let ok = option
            .as_object()
            .is_some_and(|o| o.len() == 2 && o.get("value").is_some_and(Value::is_string) && o.get("label").is_some_and(Value::is_string));
        if !ok {
            return Some("each option is { value: string, label: string }".into());
        }
        let value = option["value"].as_str().unwrap_or_default();
        if !seen.insert(value) {
            return Some(format!("duplicate option value \"{value}\""));
        }
    }
    None
}

/// The size of `URLSearchParams(query).toString()` in bytes: spaces are `+`, `*-._` and alphanumerics stay, every other
/// byte of the UTF-8 is `%XX`. Counted, not built.
fn query_bytes(map: &Map<String, Value>) -> usize {
    let encoded = |s: &str| -> usize {
        s.bytes().map(|b| if b.is_ascii_alphanumeric() || matches!(b, b'*' | b'-' | b'.' | b'_' | b' ') { 1 } else { 3 }).sum()
    };
    let pairs: usize = map.iter().map(|(k, v)| encoded(k) + 1 + encoded(v.as_str().unwrap_or_default())).sum();
    pairs + map.len().saturating_sub(1)
}

fn query(v: &Value) -> Option<String> {
    match v.as_object().filter(|o| o.values().all(Value::is_string)) {
        None => Some("must be an object of strings".into()),
        Some(o) => {
            let size = query_bytes(o);
            (size > PAGE_QUERY_BYTES).then(|| format!("is {size} bytes as a query (limit {PAGE_QUERY_BYTES})"))
        }
    }
}

/// Every prop an element takes and how to check it. Callbacks and `children` are not props on the wire.
fn prop_checks(ty: &str) -> &'static [(&'static str, Prop)] {
    use Prop::*;
    match ty {
        "Box" => &[
            ("key", Key),
            ("flexDirection", Str(&["row", "column"])),
            ("gap", SPACE),
            ("padding", SPACE),
            ("paddingX", SPACE),
            ("paddingY", SPACE),
            ("alignItems", Str(&["start", "center", "end", "stretch", "baseline"])),
            ("justifyContent", Str(&["start", "center", "end", "space-between"])),
            ("flexWrap", Str(&["wrap", "nowrap"])),
            ("flexGrow", Num(&[0, 1])),
            ("width", Width),
            ("borderStyle", Str(&["round", "single", "dashed", "quote"])),
            ("borderColor", COLOR),
            ("background", Str(&["subtle", "tint"])),
        ],
        "Text" => &[
            ("color", COLOR),
            ("bold", Bool),
            ("italic", Bool),
            ("strikethrough", Bool),
            ("code", Bool),
            ("dimColor", Bool),
            ("wrap", WRAP),
        ],
        "Pill" => &[("tone", Str(&TONES)), ("label", Text), ("icon", ICON)],
        "Icon" => &[("name", ICON), ("color", COLOR), ("label", Text)],
        "Button" => &[("key", Key), ("label", Text), ("icon", ICON), ("tone", Str(&["primary", "danger", "quiet"])), ("disabled", Bool)],
        "Input" => &[("key", Key), ("label", Text), ("placeholder", Text), ("value", Text), ("submitLabel", Text)],
        "Select" => &[("key", Key), ("label", Text), ("options", Options), ("value", Text)],
        "Markdown" => &[("key", Key), ("text", Text), ("dimColor", Bool)],
        "Code" => &[
            ("source", Text),
            ("language", Text),
            ("path", Text),
            ("startLine", Line),
            ("format", Str(&["source", "diff"])),
            ("wrap", WRAP),
        ],
        "Page" => &[("key", Key), ("path", Text), ("title", Text), ("query", Query)],
        _ => &[],
    }
}

/// The props each element takes, for the drift check against `fleet-mods.d.ts` (`MOD_ELEMENT_PROPS`).
pub fn element_props(ty: &str) -> Vec<&'static str> {
    prop_checks(ty).iter().map(|(name, _)| *name).collect()
}

fn required_props(ty: &str) -> &'static [&'static str] {
    match ty {
        "Pill" => &["tone", "label"],
        "Icon" => &["name"],
        "Button" => &["key", "label"],
        "Input" => &["key"],
        "Select" => &["key", "options"],
        "Markdown" => &["text"],
        "Code" => &["source"],
        "Page" => &["key", "path", "title"],
        _ => &[],
    }
}

/// The handles a control takes (allowed, required).
fn handles(ty: &str) -> Option<(&'static [&'static str], &'static [&'static str])> {
    match ty {
        "Button" => Some((&["onPress"], &["onPress"])),
        "Input" => Some((&["onSubmit", "onInput"], &[])),
        "Select" => Some((&["onSelect"], &["onSelect"])),
        _ => None,
    }
}

/// Props holding text a person reads, in the order they are drawn.
fn text_props(ty: &str) -> &'static [&'static str] {
    match ty {
        "Pill" | "Icon" | "Button" | "Select" => &["label"],
        "Input" => &["label", "placeholder", "submitLabel", "value"],
        "Markdown" => &["text"],
        "Code" => &["source"],
        "Page" => &["title"],
        _ => &[],
    }
}

/// The fields of a node on the wire (`NODE_FIELDS`); the fields of a wire `Page` are checked against the `.d.ts`.
pub fn node_fields(ty: &str) -> &'static [&'static str] {
    match ty {
        "Box" | "Text" => &["type", "props", "children"],
        "Button" | "Input" | "Select" => &["type", "props", "handles"],
        "Page" => &["type", "props", "mod"],
        _ => &["type", "props"],
    }
}

const INLINE_TYPES: [&str; 5] = ["Text", "Pill", "Icon", "Button", "Fleet"];

type Fail = String;

fn fail<T>(path: &str, message: &str) -> Result<T, Fail> {
    Err(if path.is_empty() { message.to_string() } else { format!("{path}: {message}") })
}

struct Walk {
    site: Site,
    inline: bool,
    keys: HashSet<String>,
    nodes: usize,
    fleet: bool,
}

fn json_string(s: &str) -> String {
    Value::String(s.to_string()).to_string()
}

fn check_node(node: &Value, path: &str, depth: usize, walk: &mut Walk, parent: Option<&str>) -> Result<(), Fail> {
    let Some(obj) = node.as_object() else { return fail(path, "not an element") };
    let Some(ty) = obj.get("type").and_then(Value::as_str) else { return fail(path, "element has no type") };
    if ty != "Fleet" && !ELEMENT_TYPES.contains(&ty) {
        return fail(path, &format!("unknown element {}", json_string(ty)));
    }

    walk.nodes += 1;
    if walk.nodes > TREE_NODES {
        return Err(format!("tree has {} elements (limit {TREE_NODES})", walk.nodes));
    }
    if depth > TREE_DEPTH {
        return Err(format!("tree is {depth} levels deep (limit {TREE_DEPTH})"));
    }

    if walk.inline {
        // A Box is a row unless it says column.
        let row_box = ty == "Box"
            && obj.get("props").and_then(Value::as_object).is_some_and(|p| p.get("flexDirection") != Some(&Value::from("column")));
        if !INLINE_TYPES.contains(&ty) && !row_box {
            return Err(format!("{path} is not inline ({} takes inline elements only)", walk.site.name()));
        }
    }

    if parent == Some("Text") && ty != "Text" {
        return fail(path, "a Text holds strings and Text only");
    }

    if ty == "Fleet" {
        if obj.len() != 1 {
            return fail(path, "Fleet carries nothing else");
        }
        if walk.fleet {
            return fail(path, "Fleet appears more than once");
        }
        walk.fleet = true;
        return Ok(());
    }

    let allowed = node_fields(ty);
    for field in obj.keys() {
        if !allowed.contains(&field.as_str()) {
            let message = if field == "children" || field == "handles" {
                format!("{ty} takes no {field}")
            } else {
                format!("unknown field \"{field}\"")
            };
            return fail(path, &message);
        }
    }
    if ty == "Page" && !obj.get("mod").and_then(Value::as_str).is_some_and(is_mod_id) {
        return fail(path, "Page needs the id of the mod that made it (name@v1, name@draft:session)");
    }
    let Some(props) = obj.get("props").and_then(Value::as_object) else { return fail(path, "props must be an object") };

    let checks = prop_checks(ty);
    for (name, value) in props {
        let Some((_, check)) = checks.iter().find(|(n, _)| n == name) else { return fail(path, &format!("unknown prop \"{name}\"")) };
        if let Some(problem) = check.check(value) {
            return fail(path, &format!("prop \"{name}\" {problem}"));
        }
    }
    for name in required_props(ty) {
        if !props.contains_key(*name) {
            return fail(path, &if *name == "key" { "missing key".to_string() } else { format!("missing \"{name}\"") });
        }
    }
    if let Some(key) = props.get("key").and_then(Value::as_str)
        && !walk.keys.insert(key.to_string())
    {
        return Err(format!("duplicate key \"{key}\""));
    }

    if let Some((allowed, required)) = handles(ty) {
        let Some(held) = obj.get("handles").and_then(Value::as_object) else { return fail(path, "handles must be an object") };
        for (name, value) in held {
            if !allowed.contains(&name.as_str()) {
                return fail(path, &format!("unknown handle \"{name}\""));
            }
            if !value.as_str().is_some_and(|s| !s.is_empty()) {
                return fail(path, &format!("handle \"{name}\" must be a non-empty string"));
            }
        }
        for name in required {
            if !held.contains_key(*name) {
                return fail(path, &format!("missing handle \"{name}\""));
            }
        }
    }

    if ty == "Box" || ty == "Text" {
        let Some(children) = obj.get("children").and_then(Value::as_array) else { return fail(path, "children must be an array") };
        for (index, child) in children.iter().enumerate() {
            if child.is_string() {
                continue;
            }
            let child_type = child.get("type").and_then(Value::as_str).filter(|_| child.is_object()).unwrap_or("child");
            check_node(child, &format!("{path} > {child_type}[{index}]"), depth + 1, walk, Some(ty))?;
        }
    }
    Ok(())
}

struct Budget {
    left: usize,
    cut: bool,
}

fn take_text(text: &str, budget: &mut Budget) -> String {
    let mut units = 0;
    let mut end = text.len();
    for (at, c) in text.char_indices() {
        units += c.len_utf16();
        if units > budget.left {
            end = at;
            break;
        }
    }
    if end == text.len() {
        budget.left -= text.encode_utf16().count();
        return text.to_string();
    }
    budget.left = 0;
    budget.cut = true;
    text[..end].to_string()
}

fn cut_node(node: &Value, budget: &mut Budget) -> Value {
    if let Some(s) = node.as_str() {
        return Value::String(take_text(s, budget));
    }
    let ty = node["type"].as_str().unwrap_or_default();
    if ty == "Fleet" {
        return node.clone();
    }
    if ty == "Box" || ty == "Text" {
        // Everything but the children is copied as is; the children are cut one by one.
        let mut copy: Map<String, Value> =
            node.as_object().into_iter().flatten().filter(|(k, _)| *k != "children").map(|(k, v)| (k.clone(), v.clone())).collect();
        let children = node["children"].as_array().into_iter().flatten().map(|c| cut_node(c, budget)).collect();
        copy.insert("children".into(), Value::Array(children));
        return Value::Object(copy);
    }
    let mut copy = node.clone();
    for name in text_props(ty) {
        if let Some(s) = node["props"][*name].as_str() {
            copy["props"][*name] = Value::String(take_text(s, budget));
        }
    }
    if ty == "Select"
        && let Some(options) = node["props"]["options"].as_array()
    {
        let cut: Vec<Value> = options
            .iter()
            .map(|o| {
                let mut o = o.clone();
                o["label"] = Value::String(take_text(o["label"].as_str().unwrap_or_default(), budget));
                o
            })
            .collect();
        copy["props"]["options"] = Value::Array(cut);
    }
    copy
}

/// Checks a tree against the contract for `site`. `Value::Null` is valid: it draws nothing.
pub fn validate_tree(tree: &Value, site: Site) -> Check<'_> {
    if tree.is_null() {
        return Check::Ok { tree: Cow::Borrowed(tree), text_cut: false };
    }
    let mut walk = Walk { site, inline: site.inline(), keys: HashSet::new(), nodes: 0, fleet: false };
    let root = tree.get("type").and_then(Value::as_str).filter(|_| tree.is_object()).unwrap_or("tree");
    if let Err(reason) = check_node(tree, root, 1, &mut walk, None) {
        return Check::Invalid { reason };
    }
    let bytes = tree.to_string().len();
    if bytes > TREE_BYTES {
        return Check::Invalid { reason: format!("tree is {bytes} bytes (limit {TREE_BYTES})") };
    }
    let mut budget = Budget { left: TREE_TEXT_CHARS, cut: false };
    let cut = cut_node(tree, &mut budget);
    if budget.cut { Check::Ok { tree: Cow::Owned(cut), text_cut: true } } else { Check::Ok { tree: Cow::Borrowed(tree), text_cut: false } }
}
