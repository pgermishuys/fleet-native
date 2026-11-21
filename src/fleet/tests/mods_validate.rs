//! The cases of weave-fleet's `lib/mods/__tests__/validate.test.ts`, under the same names (snake_case), on
//! `fleet::mods::validate`. Cases about what a `serde_json::Value` can't hold (an `undefined` or function prop, an
//! object with a prototype, a cyclic tree, a `BigInt`) have no twin: see the module's comment.
use crate::fleet::mods::types::*;
use crate::fleet::mods::validate::{Check, element_props, node_fields, validate_tree};
use serde_json::{Value, json};
use std::borrow::Cow;

fn merged(mut base: Value, props: Value) -> Value {
    for (k, v) in props.as_object().unwrap() {
        base["props"][k] = v.clone();
    }
    base
}
fn with(mut el: Value, field: &str, value: Value) -> Value {
    el[field] = value;
    el
}
fn box_(props: Value, children: Vec<Value>) -> Value {
    // Built by hand: `json!` would copy the children again at every level of a deep tree.
    let mut node = serde_json::Map::new();
    node.insert("type".into(), Value::from("Box"));
    node.insert("props".into(), props);
    node.insert("children".into(), Value::Array(children));
    Value::Object(node)
}
fn text(children: Vec<Value>) -> Value {
    json!({"type": "Text", "props": {}, "children": children})
}
fn pill(props: Value) -> Value {
    merged(json!({"type": "Pill", "props": {"tone": "good", "label": "ok"}}), props)
}
fn icon(props: Value) -> Value {
    merged(json!({"type": "Icon", "props": {"name": "check"}}), props)
}
fn button_h(props: Value, handles: Value) -> Value {
    merged(json!({"type": "Button", "props": {"key": "run", "label": "Run"}, "handles": handles}), props)
}
fn button(props: Value) -> Value {
    button_h(props, json!({"onPress": "h1"}))
}
fn input_h(props: Value, handles: Value) -> Value {
    merged(json!({"type": "Input", "props": {"key": "q"}, "handles": handles}), props)
}
fn input(props: Value) -> Value {
    input_h(props, json!({}))
}
fn select_h(props: Value, handles: Value) -> Value {
    merged(json!({"type": "Select", "props": {"key": "pick", "options": [{"value": "a", "label": "A"}]}, "handles": handles}), props)
}
fn select(props: Value) -> Value {
    select_h(props, json!({"onSelect": "h2"}))
}
fn markdown(props: Value) -> Value {
    merged(json!({"type": "Markdown", "props": {"text": "hi"}}), props)
}
fn code(props: Value) -> Value {
    merged(json!({"type": "Code", "props": {"source": "x = 1"}}), props)
}
fn page_mod(props: Value, owner: Value) -> Value {
    merged(json!({"type": "Page", "props": {"key": "p", "path": "ui/a.html", "title": "A"}, "mod": owner}), props)
}
fn page(props: Value) -> Value {
    page_mod(props, json!("demo-mod@v3"))
}
fn fleet() -> Value {
    json!({"type": "Fleet"})
}
fn no() -> Value {
    json!({})
}

/// A tree that checked out, and whether text was cut.
fn ok_at(tree: &Value, site: Site) -> (Value, bool) {
    match validate_tree(tree, site) {
        Check::Ok { tree, text_cut } => (tree.into_owned(), text_cut),
        Check::Invalid { reason } => panic!("expected a valid tree, got: {reason}\n{tree}"),
    }
}
fn ok(tree: &Value) -> (Value, bool) {
    ok_at(tree, Site::Pane)
}
fn bad_at(tree: &Value, site: Site) -> String {
    match validate_tree(tree, site) {
        Check::Invalid { reason } => reason,
        Check::Ok { .. } => panic!("expected an invalid tree:\n{tree}"),
    }
}
fn bad(tree: &Value) -> String {
    bad_at(tree, Site::Pane)
}
fn is_ok(tree: &Value) -> bool {
    validate_tree(tree, Site::Pane).is_ok()
}
fn contains(reason: String, part: &str) {
    assert!(reason.contains(part), "{reason:?} should contain {part:?}");
}
fn filler(n: usize) -> String {
    "a".repeat(n)
}

// ─── pinned to weave-fleet c1a48bd (docs/parity/22-mods.md 14.4) ────────────────────────────────

#[test]
fn the_lists_and_limits_are_the_ones_of_c1a48bd() {
    let sites: Vec<&str> = SITES.iter().map(|s| s.name()).collect();
    assert_eq!(sites, ["ToolUse", "ToolResult", "ComposerBand", "StatusChip", "Pane"]);
    let inline: Vec<&str> = SITES.iter().filter(|s| s.inline()).map(|s| s.name()).collect();
    assert_eq!(inline, ["ToolUse", "StatusChip"]);
    assert_eq!(ELEMENT_TYPES, ["Box", "Text", "Pill", "Icon", "Button", "Input", "Select", "Markdown", "Code", "Page"]);
    assert_eq!(
        ICON_NAMES,
        [
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
            "eye-off"
        ]
    );
    assert_eq!(COLOR_ROLES, ["text", "muted", "accent", "good", "warn", "bad"]);
    assert_eq!(TONES, ["good", "warn", "bad", "neutral", "accent"]);
    assert_eq!(SPACES, [0, 1, 2, 3, 4, 6, 8]);
    assert_eq!(
        [
            TREE_TEXT_CHARS,
            TREE_NODES,
            TREE_DEPTH,
            TREE_BYTES,
            INPUT_CHANGES_PER_SECOND as usize,
            SELECT_OPTIONS_MAX,
            KEY_CHARS,
            PAGE_QUERY_BYTES
        ],
        [100_000, 2_000, 32, 262_144, 4, 200, 64, 4_096]
    );
    let handles: Vec<&str> =
        [HandleName::OnPress, HandleName::OnSubmit, HandleName::OnInput, HandleName::OnSelect].iter().map(|h| h.name()).collect();
    assert_eq!(handles, ["onPress", "onSubmit", "onInput", "onSelect"]);
    // The props each element takes (`mod-types-drift.test.ts` compares these with `fleet-mods.d.ts`).
    let expected: [(&str, &[&str]); 10] = [
        (
            "Box",
            &[
                "key",
                "flexDirection",
                "gap",
                "padding",
                "paddingX",
                "paddingY",
                "alignItems",
                "justifyContent",
                "flexWrap",
                "flexGrow",
                "width",
                "borderStyle",
                "borderColor",
                "background",
            ],
        ),
        ("Text", &["color", "bold", "italic", "strikethrough", "code", "dimColor", "wrap"]),
        ("Pill", &["tone", "label", "icon"]),
        ("Icon", &["name", "color", "label"]),
        ("Button", &["key", "label", "icon", "tone", "disabled"]),
        ("Input", &["key", "label", "placeholder", "value", "submitLabel"]),
        ("Select", &["key", "label", "options", "value"]),
        ("Markdown", &["key", "text", "dimColor"]),
        ("Code", &["source", "language", "path", "startLine", "format", "wrap"]),
        ("Page", &["key", "path", "title", "query"]),
    ];
    for (ty, props) in expected {
        assert_eq!(element_props(ty), props, "{ty}");
    }
    assert_eq!(node_fields("Page"), ["type", "props", "mod"]);
    assert_eq!(node_fields("Box"), ["type", "props", "children"]);
    assert_eq!(node_fields("Button"), ["type", "props", "handles"]);
}

// ─── shape ──────────────────────────────────────────────────────────────────────────────────────

#[test]
fn validate_mod_tree_shape_accepts_null() {
    assert_eq!(validate_tree(&Value::Null, Site::Pane), Check::Ok { tree: Cow::Borrowed(&Value::Null), text_cut: false });
}

#[test]
fn returns_the_same_tree_when_nothing_is_cut() {
    let tree = box_(no(), vec![text(vec![json!("a")])]);
    let Check::Ok { tree: got, text_cut } = validate_tree(&tree, Site::Pane) else { panic!("invalid") };
    assert!(matches!(got, Cow::Borrowed(t) if std::ptr::eq(t, &tree)));
    assert!(!text_cut);
}

#[test]
fn refuses_things_that_are_not_nodes() {
    for value in [json!(3), json!("x"), json!([]), json!(true)] {
        assert!(!is_ok(&value), "{value}");
    }
    contains(bad(&json!({"props": {}, "children": []})), "type");
    contains(bad(&json!({"type": "Table", "props": {}, "children": []})), "unknown element \"Table\"");
    contains(bad(&box_(no(), vec![json!(42)])), "Box > child[0]");
    contains(bad(&box_(no(), vec![Value::Null])), "Box > child[0]");
}

#[test]
fn refuses_unknown_fields_on_a_node_and_props_that_are_not_an_object() {
    contains(bad(&with(box_(no(), vec![]), "extra", json!(1))), "unknown field \"extra\"");
    contains(bad(&json!({"type": "Box", "props": null, "children": []})), "props");
    contains(bad(&json!({"type": "Box", "props": [], "children": []})), "props");
    contains(bad(&json!({"type": "Box", "children": []})), "props");
}

#[test]
fn accepts_every_element_type() {
    let tree = box_(
        no(),
        vec![
            text(vec![json!("t")]),
            pill(no()),
            icon(no()),
            button(no()),
            input(no()),
            select(no()),
            markdown(no()),
            code(no()),
            page(no()),
        ],
    );
    ok(&tree);
    assert_eq!(ELEMENT_TYPES.len(), 10);
}

#[test]
fn names_where_a_problem_is() {
    let tree = box_(no(), vec![json!("a"), text(vec![json!("x")]), with(text(vec![json!("y")]), "props", json!({"colour": "good"}))]);
    assert_eq!(bad(&tree), "Box > Text[2]: unknown prop \"colour\"");
}

// ─── Fleet ──────────────────────────────────────────────────────────────────────────────────────

#[test]
fn accepts_a_lone_fleet_and_one_inside_a_box() {
    ok(&fleet());
    ok(&box_(no(), vec![fleet(), text(vec![json!("after")])]));
}

#[test]
fn refuses_a_fleet_that_carries_anything() {
    contains(bad(&json!({"type": "Fleet", "props": {}})), "Fleet");
    contains(bad(&box_(no(), vec![json!({"type": "Fleet", "children": []})])), "Fleet");
}

#[test]
fn refuses_two_fleets_in_one_tree() {
    contains(bad(&box_(no(), vec![fleet(), box_(no(), vec![fleet()])])), "Fleet");
}

// ─── props ──────────────────────────────────────────────────────────────────────────────────────

#[test]
fn accepts_every_allowed_value_of_every_enum() {
    for gap in SPACES {
        ok(&box_(json!({"gap": gap, "padding": gap, "paddingX": gap, "paddingY": gap}), vec![]));
    }
    for color in COLOR_ROLES {
        ok(&box_(json!({"borderColor": color}), vec![]));
        ok(&json!({"type": "Text", "props": {"color": color}, "children": ["x"]}));
        ok(&icon(json!({"color": color})));
    }
    for tone in TONES {
        ok(&pill(json!({"tone": tone})));
    }
    for name in ICON_NAMES {
        ok(&icon(json!({"name": name})));
        ok(&pill(json!({"icon": name})));
        ok(&button(json!({"icon": name})));
    }
    for v in ["row", "column"] {
        ok(&box_(json!({"flexDirection": v}), vec![]));
    }
    for v in ["start", "center", "end", "stretch", "baseline"] {
        ok(&box_(json!({"alignItems": v}), vec![]));
    }
    for v in ["start", "center", "end", "space-between"] {
        ok(&box_(json!({"justifyContent": v}), vec![]));
    }
    for v in ["wrap", "nowrap"] {
        ok(&box_(json!({"flexWrap": v}), vec![]));
    }
    for v in [0, 1] {
        ok(&box_(json!({"flexGrow": v}), vec![]));
    }
    for v in ["round", "single", "dashed", "quote"] {
        ok(&box_(json!({"borderStyle": v}), vec![]));
    }
    for v in ["subtle", "tint"] {
        ok(&box_(json!({"background": v}), vec![]));
    }
    for wrap in ["wrap", "truncate"] {
        ok(&json!({"type": "Text", "props": {"wrap": wrap}, "children": []}));
        ok(&code(json!({"wrap": wrap})));
    }
    for v in ["primary", "danger", "quiet"] {
        ok(&button(json!({"tone": v})));
    }
    for v in ["source", "diff"] {
        ok(&code(json!({"format": v})));
    }
}

#[test]
fn refuses_a_value_outside_each_enum() {
    let cases: Vec<(Value, &str)> = vec![
        (box_(json!({"gap": 5}), vec![]), "gap"),
        (box_(json!({"padding": "1"}), vec![]), "padding"),
        (box_(json!({"paddingX": -1}), vec![]), "paddingX"),
        (box_(json!({"paddingY": 9}), vec![]), "paddingY"),
        (box_(json!({"borderColor": "red"}), vec![]), "borderColor"),
        (box_(json!({"flexDirection": "diagonal"}), vec![]), "flexDirection"),
        (box_(json!({"alignItems": "middle"}), vec![]), "alignItems"),
        (box_(json!({"justifyContent": "space-around"}), vec![]), "justifyContent"),
        (box_(json!({"flexWrap": "wrap-reverse"}), vec![]), "flexWrap"),
        (box_(json!({"flexGrow": 2}), vec![]), "flexGrow"),
        (box_(json!({"borderStyle": "double"}), vec![]), "borderStyle"),
        (box_(json!({"background": "red"}), vec![]), "background"),
        (json!({"type": "Text", "props": {"color": "red"}, "children": []}), "color"),
        (json!({"type": "Text", "props": {"wrap": "clip"}, "children": []}), "wrap"),
        (pill(json!({"tone": "loud"})), "tone"),
        (pill(json!({"icon": "rocket"})), "icon"),
        (icon(json!({"name": "rocket"})), "name"),
        (icon(json!({"color": "pink"})), "color"),
        (button(json!({"tone": "loud"})), "tone"),
        (button(json!({"icon": "nope"})), "icon"),
        (code(json!({"format": "patch"})), "format"),
        (code(json!({"wrap": "clip"})), "wrap"),
    ];
    for (tree, name) in cases {
        contains(bad(&tree), &format!("\"{name}\""));
    }
}

#[test]
fn checks_booleans() {
    for prop in ["bold", "italic", "strikethrough", "code", "dimColor"] {
        ok(&json!({"type": "Text", "props": {prop: true}, "children": []}));
        contains(bad(&json!({"type": "Text", "props": {prop: "yes"}, "children": []})), &format!("\"{prop}\""));
    }
    ok(&button(json!({"disabled": true})));
    contains(bad(&button(json!({"disabled": 1}))), "\"disabled\"");
    ok(&markdown(json!({"dimColor": false})));
    contains(bad(&markdown(json!({"dimColor": "no"}))), "\"dimColor\"");
}

#[test]
fn checks_box_width_as_a_percentage_from_0_to_100() {
    for width in ["0%", "50%", "33.3%", "100%"] {
        ok(&box_(json!({"width": width}), vec![]));
    }
    for width in
        [json!("101%"), json!("-5%"), json!("50"), json!("50px"), json!("%"), json!("5 %"), json!(50), json!("1e2%"), json!("100.5%")]
    {
        contains(bad(&box_(json!({"width": width}), vec![])), "\"width\"");
    }
}

#[test]
fn checks_start_line_as_a_positive_integer() {
    ok(&code(json!({"startLine": 1})));
    ok(&code(json!({"startLine": 120})));
    // JavaScript's NaN is `null` on the wire, so it is refused as not a number.
    for start_line in [json!(0), json!(-1), json!(1.5), json!("3"), Value::Null] {
        contains(bad(&code(json!({"startLine": start_line}))), "\"startLine\"");
    }
}

#[test]
fn checks_string_props() {
    ok(&code(json!({"language": "ts", "path": "a/b.ts"})));
    contains(bad(&code(json!({"language": 3}))), "\"language\"");
    contains(bad(&code(json!({"path": null}))), "\"path\"");
    contains(bad(&pill(json!({"label": 4}))), "\"label\"");
    contains(bad(&markdown(json!({"text": ["a"]}))), "\"text\"");
    contains(bad(&icon(json!({"label": 1}))), "\"label\"");
}

#[test]
fn refuses_props_an_element_does_not_list() {
    for ty in ELEMENT_TYPES {
        let base = match ty {
            "Box" => box_(no(), vec![]),
            "Text" => text(vec![]),
            "Button" => button(no()),
            "Input" => input(no()),
            "Select" => select(no()),
            "Page" => page(no()),
            other => json!({"type": other, "props": {}}),
        };
        contains(bad(&merged(base, json!({"notAProp": 1}))), "unknown prop \"notAProp\"");
    }
    contains(bad(&box_(json!({"children": ["a"]}), vec![json!("b")])), "unknown prop \"children\"");
    contains(bad(&button(json!({"onPress": "h"}))), "unknown prop \"onPress\"");
    contains(bad(&box_(json!({"colour": "good"}), vec![])), "unknown prop \"colour\"");
}

#[test]
fn lists_the_props_of_every_element() {
    let mut listed: Vec<&str> = ELEMENT_TYPES.iter().filter(|t| !element_props(t).is_empty()).copied().collect();
    listed.sort_unstable();
    let mut all = ELEMENT_TYPES.to_vec();
    all.sort_unstable();
    assert_eq!(listed, all);
    assert!(element_props("Button").contains(&"label"));
    assert!(!element_props("Button").contains(&"onPress"));
    assert!(!element_props("Box").contains(&"children"));
}

// ─── required props ─────────────────────────────────────────────────────────────────────────────

#[test]
fn requires_them() {
    let cases: Vec<(Value, &str)> = vec![
        (json!({"type": "Pill", "props": {"label": "a"}}), "tone"),
        (json!({"type": "Pill", "props": {"tone": "good"}}), "label"),
        (json!({"type": "Icon", "props": {}}), "name"),
        (json!({"type": "Button", "props": {"key": "a"}, "handles": {"onPress": "h"}}), "label"),
        (json!({"type": "Select", "props": {"key": "a"}, "handles": {"onSelect": "h"}}), "options"),
        (json!({"type": "Markdown", "props": {}}), "text"),
        (json!({"type": "Code", "props": {}}), "source"),
        (json!({"type": "Page", "props": {"key": "p", "title": "t"}, "mod": "m@v1"}), "path"),
        (json!({"type": "Page", "props": {"key": "p", "path": "a.html"}, "mod": "m@v1"}), "title"),
    ];
    for (tree, name) in cases {
        contains(bad(&tree), &format!("missing \"{name}\""));
    }
}

#[test]
fn requires_a_key_on_button_input_select_and_page() {
    let no_key = json!({"type": "Button", "props": {"label": "a"}, "handles": {"onPress": "h"}});
    assert_eq!(bad(&box_(no(), vec![no_key])), "Box > Button[0]: missing key");
    assert_eq!(bad(&json!({"type": "Input", "props": {}, "handles": {}})), "Input: missing key");
    assert_eq!(
        bad(&json!({"type": "Select", "props": {"options": [{"value": "a", "label": "A"}]}, "handles": {"onSelect": "h"}})),
        "Select: missing key"
    );
    assert_eq!(bad(&json!({"type": "Page", "props": {"path": "a.html", "title": "t"}, "mod": "m@v1"})), "Page: missing key");
}

// ─── Select ─────────────────────────────────────────────────────────────────────────────────────

fn options(n: usize) -> Value {
    Value::Array((0..n).map(|i| json!({"value": format!("v{i}"), "label": format!("L{i}")})).collect())
}

#[test]
fn takes_1_to_200_options() {
    ok(&select(json!({"options": options(1)})));
    ok(&select(json!({"options": options(SELECT_OPTIONS_MAX)})));
    contains(bad(&select(json!({"options": []}))), "options");
    contains(bad(&select(json!({"options": options(SELECT_OPTIONS_MAX + 1)}))), "options");
}

#[test]
fn checks_each_option_and_that_values_are_unique() {
    contains(bad(&select(json!({"options": "a"}))), "options");
    contains(bad(&select(json!({"options": [{"value": "a"}]}))), "options");
    contains(bad(&select(json!({"options": [{"value": 1, "label": "A"}]}))), "options");
    contains(bad(&select(json!({"options": [{"value": "a", "label": "A", "extra": 1}]}))), "options");
    contains(
        bad(&select(json!({"options": [{"value": "a", "label": "A"}, {"value": "a", "label": "B"}]}))),
        "duplicate option value \"a\"",
    );
}

#[test]
fn type_checks_value_without_requiring_it_to_be_an_option() {
    ok(&select(json!({"value": "zzz"})));
    contains(bad(&select(json!({"value": 3}))), "\"value\"");
}

// ─── Page ───────────────────────────────────────────────────────────────────────────────────────

#[test]
fn takes_a_query_of_strings_up_to_4096_bytes() {
    ok(&page(json!({"query": {"a": "1", "b": "two words"}})));
    ok(&page(json!({"query": {}})));
    contains(bad(&page(json!({"query": {"a": 1}}))), "\"query\"");
    contains(bad(&page(json!({"query": "a=1"}))), "\"query\"");
    contains(bad(&page(json!({"query": ["a"]}))), "\"query\"");
}

#[test]
fn measures_the_query_as_it_is_sent() {
    let fits = "a".repeat(PAGE_QUERY_BYTES - 2); // "q=" + value
    ok(&page(json!({"query": {"q": fits}})));
    contains(bad(&page(json!({"query": {"q": format!("{fits}a")}}))), "\"query\"");
    // A space is "+" and a multi-byte character is several percent escapes, so the sent form is what counts.
    contains(bad(&page(json!({"query": {"q": "é".repeat(700)}}))), "\"query\"");
}

#[test]
fn requires_the_owning_mods_id_as_a_top_level_field_name_at_vn_or_name_at_draft_session() {
    for good in
        ["a@v1", "a-1-b@v12", &format!("{}@v3", "a".repeat(64)), "demo@draft:ses_0123-abc", &format!("demo@draft:{}", "s".repeat(128))]
    {
        ok(&page_mod(no(), json!(good)));
    }
    let bads = [
        json!(""),
        json!(3),
        Value::Null,
        json!("demo"),
        json!("a"),
        json!("A@v1"),
        json!("1a@v1"),
        json!("-a@v1"),
        json!("a_b@v1"),
        json!("a/b@v1"),
        json!("a..@v1"),
        json!("a b@v1"),
        json!("../x@v1"),
        json!(format!("{}@v1", "a".repeat(65))),
        json!("a@v0"),
        json!("a@v01"),
        json!("a@v"),
        json!("a@1"),
        json!("a@V1"),
        json!("a@draft"),
        json!("a@draft:"),
        json!("a@draft:a/b"),
        json!("a@draft:a b"),
        json!(format!("a@draft:{}", "s".repeat(129))),
        json!("a@v1/../b"),
        json!("a@v1\n"),
        json!("a@v1@v2"),
        json!("@v1"),
    ];
    for owner in bads {
        contains(bad(&page_mod(no(), owner.clone())), "mod");
    }
    // `mod: undefined` is a missing field once it is JSON.
    let mut missing = page(no());
    missing.as_object_mut().unwrap().remove("mod");
    contains(bad(&missing), "mod");
}

#[test]
fn refuses_a_mod_in_props_and_on_any_other_node() {
    contains(bad(&page(json!({"mod": "other"}))), "unknown prop \"mod\"");
    contains(bad(&with(pill(no()), "mod", json!("demo-mod@v1"))), "unknown field \"mod\"");
    contains(bad(&with(box_(no(), vec![]), "mod", json!("demo-mod@v1"))), "unknown field \"mod\"");
    contains(bad(&with(button(no()), "mod", json!("demo-mod@v1"))), "unknown field \"mod\"");
}

#[test]
fn refuses_a_page_with_children_or_handles() {
    contains(bad(&with(page(no()), "children", json!([]))), "takes no children");
    contains(bad(&with(page(no()), "handles", json!({}))), "takes no handles");
}

// ─── names that every object has ────────────────────────────────────────────────────────────────

const NAMES: [&str; 7] = ["valueOf", "hasOwnProperty", "__proto__", "isPrototypeOf", "constructor", "toString", "toLocaleString"];

#[test]
fn is_an_unknown_prop_not_a_throw() {
    for name in NAMES {
        for ty in ELEMENT_TYPES {
            let tree: Value =
                serde_json::from_str(&format!(r#"{{"type":"{ty}","props":{{"{name}":"x"}},"children":[],"handles":{{}},"mod":"m@v1"}}"#))
                    .unwrap();
            assert!(!is_ok(&tree), "{ty}.{name}");
        }
        let pill_with: Value =
            serde_json::from_str(&format!(r#"{{"type":"Pill","props":{{"tone":"good","label":"x","{name}":"y"}}}}"#)).unwrap();
        contains(bad(&pill_with), &format!("unknown prop \"{name}\""));
    }
}

#[test]
fn is_not_an_element_type() {
    for name in NAMES {
        let el: Value = serde_json::from_str(&format!(r#"{{"type":"{name}","props":{{}}}}"#)).unwrap();
        contains(bad(&el), "unknown element");
        contains(bad(&box_(no(), vec![el])), "unknown element");
    }
}

#[test]
fn is_not_a_handle() {
    for name in NAMES {
        let tree: Value = serde_json::from_str(&format!(
            r#"{{"type":"Button","props":{{"key":"k","label":"x"}},"handles":{{"onPress":"h1","{name}":"h2"}}}}"#
        ))
        .unwrap();
        contains(bad(&tree), &format!("unknown handle \"{name}\""));
    }
}

// ─── what the host and the browser agree on ─────────────────────────────────────────────────────

fn units(v: &Value) -> usize {
    v.as_str().unwrap().encode_utf16().count()
}

#[test]
fn counts_a_button_label_then_the_markdown_against_the_100_000() {
    let (cut, was_cut) = ok(&box_(no(), vec![button(json!({"label": "Go"})), markdown(json!({"text": filler(100_000)}))]));
    assert!(was_cut);
    assert_eq!(cut["children"][0]["props"]["label"], "Go");
    assert_eq!(units(&cut["children"][1]["props"]["text"]), 100_000 - 2);
}

#[test]
fn counts_select_option_labels_in_tree_order() {
    let tree =
        box_(no(), vec![select(json!({"options": [{"value": "v", "label": filler(60_000)}]})), markdown(json!({"text": filler(60_000)}))]);
    let (cut, was_cut) = ok(&tree);
    assert!(was_cut);
    assert_eq!(units(&cut["children"][1]["props"]["text"]), 40_000);
}

#[test]
fn counts_an_inputs_label_placeholder_submit_label_then_value_as_the_host_does() {
    let full = json!({"label": filler(40_000), "placeholder": filler(40_000), "submitLabel": filler(40_000), "value": filler(40_000)});
    let tree = box_(no(), vec![input_h(full, json!({"onSubmit": "h1"})), markdown(json!({"text": "tail"}))]);
    let (cut, _) = ok(&tree);
    let p = &cut["children"][0]["props"];
    assert_eq!([units(&p["label"]), units(&p["placeholder"]), units(&p["submitLabel"]), units(&p["value"])], [40_000, 40_000, 20_000, 0]);
    assert_eq!(cut["children"][1]["props"]["text"], "");
}

#[test]
fn counts_every_input_text_prop_icon_label_and_page_title() {
    let props = json!({"label": filler(10), "value": filler(20), "placeholder": filler(30), "submitLabel": filler(40)});
    let tree = box_(
        no(),
        vec![
            input_h(props, json!({"onSubmit": "h1"})),
            icon(json!({"label": filler(50)})),
            page(json!({"title": filler(60)})),
            markdown(json!({"text": filler(100_000)})),
        ],
    );
    let (cut, _) = ok(&tree);
    assert_eq!(units(&cut["children"][3]["props"]["text"]), 100_000 - 210);
}

#[test]
fn does_not_count_the_select_value_keys_paths_or_a_code_language() {
    let tree = box_(
        no(),
        vec![
            select(json!({"value": filler(500)})),
            code(json!({"source": "x", "language": filler(500), "path": filler(500)})),
            markdown(json!({"text": filler(99_998)})),
        ],
    );
    assert!(!ok(&tree).1);
}

#[test]
fn measures_a_page_query_as_the_url_encoded_string() {
    // Slashes are percent-escaped (three bytes each as %2F), so 1366 of them are over 4096 as a query.
    ok(&page(json!({"query": {"q": "/".repeat(1364)}}))); // "q=" + 1364 * 3 = 4094
    contains(bad(&page(json!({"query": {"q": "/".repeat(1366)}}))), "\"query\"");
    // Non-ASCII: two bytes in UTF-8, six as escapes.
    ok(&page(json!({"query": {"q": "é".repeat(681)}})));
    contains(bad(&page(json!({"query": {"q": "é".repeat(683)}}))), "\"query\"");
    // Control characters: three bytes each.
    ok(&page(json!({"query": {"q": "\u{1}".repeat(1364)}})));
    contains(bad(&page(json!({"query": {"q": "\u{1}".repeat(1366)}}))), "\"query\"");
}

// ─── children ───────────────────────────────────────────────────────────────────────────────────

#[test]
fn gives_box_and_text_a_children_array() {
    contains(bad(&json!({"type": "Box", "props": {}})), "children");
    contains(bad(&json!({"type": "Box", "props": {}, "children": "a"})), "children");
    contains(bad(&json!({"type": "Text", "props": {}})), "children");
}

#[test]
fn lets_a_box_hold_nodes_and_strings() {
    ok(&box_(no(), vec![json!("a"), text(vec![json!("b")]), box_(no(), vec![]), pill(no())]));
}

#[test]
fn lets_a_text_hold_strings_and_text_nodes_only() {
    ok(&text(vec![json!("a"), text(vec![json!("b"), text(vec![json!("c")])])]));
    assert_eq!(bad(&text(vec![pill(no())])), "Text > Pill[0]: a Text holds strings and Text only");
    contains(bad(&text(vec![box_(no(), vec![])])), "Text > Box[0]");
    contains(bad(&text(vec![fleet()])), "Text > Fleet[0]");
    contains(bad(&text(vec![json!(5)])), "Text > child[0]");
}

#[test]
fn refuses_children_or_handles_where_an_element_has_none() {
    for el in [pill(no()), icon(no()), markdown(no()), code(no()), page(no())] {
        contains(bad(&with(el.clone(), "children", json!([]))), "children");
        contains(bad(&with(el, "handles", json!({}))), "handles");
    }
    contains(bad(&with(button(no()), "children", json!([]))), "children");
    contains(bad(&with(box_(no(), vec![]), "handles", json!({}))), "handles");
    contains(bad(&with(text(vec![json!("a")]), "handles", json!({}))), "handles");
}

// ─── handles ────────────────────────────────────────────────────────────────────────────────────

#[test]
fn requires_on_press_on_a_button_and_on_select_on_a_select() {
    contains(bad(&button_h(no(), json!({}))), "missing handle \"onPress\"");
    contains(bad(&json!({"type": "Button", "props": {"key": "a", "label": "A"}})), "handles");
    contains(bad(&select_h(no(), json!({}))), "missing handle \"onSelect\"");
}

#[test]
fn lets_an_input_have_on_submit_on_input_both_or_none() {
    ok(&input_h(no(), json!({})));
    ok(&input_h(no(), json!({"onSubmit": "h"})));
    ok(&input_h(no(), json!({"onInput": "h"})));
    ok(&input_h(no(), json!({"onSubmit": "h", "onInput": "g"})));
}

#[test]
fn refuses_handle_names_an_element_does_not_take() {
    contains(bad(&button_h(no(), json!({"onPress": "h", "onSubmit": "g"}))), "unknown handle \"onSubmit\"");
    contains(bad(&input_h(no(), json!({"onPress": "h"}))), "unknown handle \"onPress\"");
    contains(bad(&input_h(no(), json!({"onSelect": "h"}))), "unknown handle \"onSelect\"");
    contains(bad(&select_h(no(), json!({"onSelect": "h", "onInput": "g"}))), "unknown handle \"onInput\"");
}

#[test]
fn wants_non_empty_string_handle_values() {
    for value in [json!(""), json!(4), Value::Null, json!({})] {
        contains(bad(&button_h(no(), json!({"onPress": value}))), "handle \"onPress\"");
    }
    contains(bad(&json!({"type": "Button", "props": {"key": "a", "label": "A"}, "handles": []})), "handles");
    contains(bad(&json!({"type": "Button", "props": {"key": "a", "label": "A"}, "handles": null})), "handles");
}

// ─── keys ───────────────────────────────────────────────────────────────────────────────────────

#[test]
fn accepts_the_allowed_characters_up_to_64() {
    for key in ["a", "Run_1.x-y", &"A".repeat(64), "0"] {
        ok(&button(json!({"key": key})));
    }
}

#[test]
fn refuses_others() {
    for key in [json!(""), json!("a b"), json!("a/b"), json!("é"), json!("A".repeat(65)), json!(3), Value::Null] {
        contains(bad(&button(json!({"key": key}))), "\"key\"");
    }
}

#[test]
fn checks_optional_keys_on_box_and_markdown_too() {
    ok(&box_(json!({"key": "frame"}), vec![]));
    ok(&markdown(json!({"key": "doc"})));
    contains(bad(&box_(json!({"key": "no good"}), vec![])), "\"key\"");
    contains(bad(&markdown(json!({"key": ""}))), "\"key\"");
}

#[test]
fn requires_keys_to_be_unique_across_all_elements() {
    let dup = |k: &str| format!("duplicate key \"{k}\"");
    assert_eq!(bad(&box_(no(), vec![button(json!({"key": "run"})), button(json!({"key": "run"}))])), dup("run"));
    assert_eq!(bad(&box_(json!({"key": "x"}), vec![markdown(json!({"key": "x"}))])), dup("x"));
    assert_eq!(bad(&box_(no(), vec![input(json!({"key": "k"})), select(json!({"key": "k"}))])), dup("k"));
    assert_eq!(bad(&box_(no(), vec![page(json!({"key": "k"})), button(json!({"key": "k"}))])), dup("k"));
    ok(&box_(no(), vec![button(json!({"key": "a"})), button(json!({"key": "b"}))]));
}

// ─── inline sites ───────────────────────────────────────────────────────────────────────────────

const INLINE: [Site; 2] = [Site::ToolUse, Site::StatusChip];
const OTHER: [Site; 3] = [Site::ToolResult, Site::ComposerBand, Site::Pane];

#[test]
fn accepts_inline_elements() {
    for site in INLINE {
        ok_at(&text(vec![json!("a")]), site);
        ok_at(&pill(no()), site);
        ok_at(&icon(no()), site);
        ok_at(&button(no()), site);
        ok_at(&fleet(), site);
        ok_at(
            &box_(json!({"flexDirection": "row", "gap": 1}), vec![text(vec![json!("a")]), pill(no()), icon(no()), button(no()), fleet()]),
            site,
        );
        ok_at(&box_(json!({"flexDirection": "row"}), vec![box_(json!({"flexDirection": "row"}), vec![pill(no())])]), site);
        ok_at(&Value::Null, site);
    }
}

#[test]
fn refuses_the_other_elements() {
    for site in INLINE {
        let name = site.name();
        assert_eq!(bad_at(&input(no()), site), format!("Input is not inline ({name} takes inline elements only)"));
        contains(bad_at(&select(no()), site), "Select is not inline");
        contains(bad_at(&markdown(no()), site), "Markdown is not inline");
        contains(bad_at(&code(no()), site), "Code is not inline");
        contains(bad_at(&page(no()), site), "Page is not inline");
        assert_eq!(
            bad_at(&box_(json!({"flexDirection": "row"}), vec![input(no())]), site),
            format!("Box > Input[0] is not inline ({name} takes inline elements only)")
        );
    }
}

#[test]
fn takes_a_box_with_no_direction_as_a_row_and_refuses_only_a_column() {
    for site in INLINE {
        ok_at(&box_(no(), vec![pill(no())]), site);
        ok_at(&box_(no(), vec![box_(no(), vec![pill(no()), text(vec![json!("a")])]), button(no())]), site);
        ok_at(&box_(json!({"flexDirection": "row"}), vec![box_(no(), vec![pill(no())])]), site);
        contains(bad_at(&box_(json!({"flexDirection": "column"}), vec![pill(no())]), site), "Box is not inline");
        contains(bad_at(&box_(no(), vec![box_(json!({"flexDirection": "column"}), vec![pill(no())])]), site), "Box > Box[0] is not inline");
        contains(bad_at(&box_(no(), vec![input(no())]), site), "Box > Input[0] is not inline");
    }
}

#[test]
fn lets_the_other_sites_take_everything() {
    for site in OTHER {
        let tree = box_(
            no(),
            vec![input(no()), select(no()), markdown(no()), code(no()), page(no()), box_(json!({"flexDirection": "column"}), vec![])],
        );
        ok_at(&tree, site);
    }
}

// ─── limits ─────────────────────────────────────────────────────────────────────────────────────

fn nodes(n: usize) -> Value {
    // 1 box + (n - 1) pills.
    box_(no(), (0..n - 1).map(|_| pill(no())).collect())
}

fn deep(levels: usize) -> Value {
    let mut tree = text(vec![json!("leaf")]);
    for _ in 1..levels {
        tree = box_(no(), vec![tree]);
    }
    tree
}

#[test]
fn allows_2000_elements_and_refuses_2001() {
    ok(&nodes(TREE_NODES));
    assert_eq!(bad(&nodes(TREE_NODES + 1)), "tree has 2001 elements (limit 2000)");
}

#[test]
fn counts_fleet_and_text_nodes_as_elements_not_strings() {
    ok(&box_(no(), (0..5000).map(|_| json!("s")).collect()));
}

#[test]
fn allows_depth_32_root_is_1_and_refuses_33() {
    ok(&deep(TREE_DEPTH));
    assert_eq!(bad(&deep(TREE_DEPTH + 1)), "tree is 33 levels deep (limit 32)");
}

#[test]
fn refuses_a_very_deep_tree_without_overflowing_the_stack() {
    // Building and dropping a 20,000-level `Value` recurses too; give the test thread room for that.
    std::thread::Builder::new().stack_size(256 << 20).spawn(|| contains(bad(&deep(20_000)), "levels deep")).unwrap().join().unwrap();
}

#[test]
fn allows_262_144_bytes_of_json_and_refuses_one_more() {
    // {"props":{"text":""},"type":"Markdown"} plus the text.
    let base = markdown(json!({"text": ""})).to_string().len();
    let fits = markdown(json!({"text": "a".repeat(TREE_BYTES - base)}));
    assert_eq!(fits.to_string().len(), TREE_BYTES);
    // Over the text budget too? No: 262,144 chars is over 100,000, so the text is cut, not refused.
    assert!(ok(&fits).1);
    let over = markdown(json!({"text": "a".repeat(TREE_BYTES - base + 1)}));
    assert_eq!(bad(&over), "tree is 262145 bytes (limit 262144)");
}

#[test]
fn counts_bytes_not_characters() {
    let base = markdown(json!({"text": ""})).to_string().len();
    // 3 bytes each.
    let chars = (TREE_BYTES - base) / 3 + 1;
    contains(bad(&markdown(json!({"text": "€".repeat(chars)}))), "bytes (limit 262144)");
}

// ─── text cut ───────────────────────────────────────────────────────────────────────────────────

const BUDGET: usize = TREE_TEXT_CHARS;

#[test]
fn keeps_exactly_100_000_characters() {
    assert!(!ok(&markdown(json!({"text": "a".repeat(BUDGET)}))).1);
}

#[test]
fn cuts_at_100_001() {
    let (cut, was_cut) = ok(&markdown(json!({"text": "a".repeat(BUDGET + 1)})));
    assert!(was_cut);
    assert_eq!(cut["props"]["text"], "a".repeat(BUDGET));
}

#[test]
fn does_not_mutate_its_input_when_it_cuts() {
    let tree = markdown(json!({"text": "a".repeat(BUDGET + 50)}));
    let before = tree.to_string();
    let Check::Ok { tree: got, .. } = validate_tree(&tree, Site::Pane) else { panic!("invalid") };
    assert!(matches!(got, Cow::Owned(_)));
    assert_eq!(tree.to_string(), before);
}

#[test]
fn cuts_in_document_order_and_empties_what_follows() {
    let tree = box_(no(), vec![json!("a".repeat(60_000)), text(vec![json!("b".repeat(60_000))]), pill(json!({"label": "late"}))]);
    let (cut, was_cut) = ok(&tree);
    assert!(was_cut);
    assert_eq!(cut["children"][0], "a".repeat(60_000));
    assert_eq!(cut["children"][1]["children"][0], "b".repeat(40_000));
    assert_eq!(cut["children"][2]["props"]["label"], "");
    assert_eq!(cut["children"][2]["props"]["tone"], "good");
}

#[test]
fn counts_every_drawn_text() {
    let filler = markdown(json!({"text": "x".repeat(BUDGET - 5)}));
    let cases: Vec<(Value, &str)> = vec![
        (pill(json!({"label": "123456789"})), "/props/label"),
        (button(json!({"label": "123456789"})), "/props/label"),
        (input(json!({"placeholder": "123456789"})), "/props/placeholder"),
        (input(json!({"label": "123456789"})), "/props/label"),
        (code(json!({"source": "123456789"})), "/props/source"),
        (page(json!({"title": "123456789"})), "/props/title"),
        (select(json!({"options": [{"value": "v", "label": "123456789"}]})), "/props/options/0/label"),
    ];
    for (el, at) in cases {
        let ty = el["type"].clone();
        let (cut, was_cut) = ok(&box_(no(), vec![filler.clone(), el]));
        assert!(was_cut, "{ty}");
        assert_eq!(cut["children"][1].pointer(at).unwrap(), "12345", "{ty}");
    }
}

#[test]
fn truncates_the_string_where_the_budget_runs_out() {
    let (cut, _) = ok(&box_(no(), vec![markdown(json!({"text": "x".repeat(BUDGET - 5)})), pill(json!({"label": "123456789"}))]));
    assert_eq!(cut["children"][1]["props"]["label"], "12345");
}

#[test]
fn keeps_handles_keys_and_other_props_when_it_cuts() {
    let (cut, _) = ok(&box_(no(), vec![markdown(json!({"text": "x".repeat(BUDGET)})), button(json!({"key": "go", "label": "Go"}))]));
    assert_eq!(cut["children"][1], json!({"type": "Button", "props": {"key": "go", "label": ""}, "handles": {"onPress": "h1"}}));
}

#[test]
fn counts_text_in_utf_16_units_like_fleet_and_never_splits_a_character() {
    // "😀" is two UTF-16 units, so 50,000 of them fill the budget and the next one is cut whole.
    let (cut, was_cut) = ok(&markdown(json!({"text": "😀".repeat(50_001)})));
    assert!(was_cut);
    assert_eq!(cut["props"]["text"], "😀".repeat(50_000));
    let (cut, _) = ok(&markdown(json!({"text": format!("{}😀", "a".repeat(BUDGET - 1))})));
    assert_eq!(cut["props"]["text"], "a".repeat(BUDGET - 1));
}
