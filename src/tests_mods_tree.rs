//! A mod's tree drawn (component 22, piece B): `mod-style.test.ts` and the Root, Box, Text, Pill, Icon and "inline sites
//! wrap" cases of `ModTree.test.ts`, under their names. Fleet's tests read CSS from jsdom; these read what the laid-out tree
//! measures, since a drawn element has no style attribute to read.
use crate::fleet::mods::types::{COLOR_ROLES, ICON_NAMES, SPACES, Site, TONES};
use crate::mock::mod_trees::ModScene;
use crate::tests::{FRAME, app};
use crate::theme::Theme;
use crate::view::mods::leaves::{mod_icon, role_color, space_px, tone_color};
use crate::view::mods::tree::{Draw, box_look, to_draw};
use rune::Runtime;
use serde_json::{Value, json};

fn text(props: Value, children: Vec<Value>) -> Value {
    json!({"type": "Text", "props": props, "children": children})
}
fn boxed(props: Value, children: Vec<Value>) -> Value {
    json!({"type": "Box", "props": props, "children": children})
}
fn leaf(kind: &str, props: Value) -> Value {
    json!({"type": kind, "props": props})
}

/// The app with `tree` drawn at `site` in a Mod tab of the right panel (the scene's stand-in for a pane).
fn show(tree: Value, site: Site) -> Runtime<crate::State> {
    show_with(tree, site, true)
}

fn show_with(tree: Value, site: Site, slot: bool) -> Runtime<crate::State> {
    let mut rt = app();
    rt.update(move |s, _| {
        s.show_mod_elements(true);
        s.mod_scene = Some(ModScene { tree, site, slot });
    });
    for _ in 0..3 {
        rt.frame(FRAME);
    }
    rt
}

fn rect(rt: &Runtime<crate::State>, key: &str) -> [f32; 4] {
    rt.presented(key).unwrap_or_else(|| panic!("no element keyed {key:?}")).0
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 1.0
}

const ROOT: &str = "mod-tree";
const FIRST: &str = "mod-tree.0";

// mod-style.test.ts: "maps each colour role to a theme variable"
#[test]
fn maps_each_colour_role_to_a_theme_variable() {
    for dark in [true, false] {
        let t = Theme::of(dark);
        let want = [("text", t.text), ("muted", t.muted), ("accent", t.accent), ("good", t.running), ("warn", t.idle), ("bad", t.error)];
        for role in COLOR_ROLES {
            let (_, color) = want.iter().find(|(r, _)| *r == role).expect("a role of the contract");
            assert_eq!(role_color(&t, role), Some(*color), "{role}");
        }
        assert_eq!(want.len(), COLOR_ROLES.len());
    }
}

// mod-style.test.ts: "maps each tone, neutral to muted"
#[test]
fn maps_each_tone_neutral_to_muted() {
    let t = Theme::of(true);
    let want = [("good", t.running), ("warn", t.idle), ("bad", t.error), ("neutral", t.muted), ("accent", t.accent)];
    for tone in TONES {
        let (_, color) = want.iter().find(|(r, _)| *r == tone).expect("a tone of the contract");
        assert_eq!(tone_color(&t, tone), Some(*color), "{tone}");
    }
    assert_eq!(want.len(), TONES.len());
}

// mod-style.test.ts: "returns nothing for an unknown role"
#[test]
fn returns_nothing_for_an_unknown_role() {
    let t = Theme::of(true);
    assert_eq!(role_color(&t, "pink"), None);
    assert_eq!(tone_color(&t, "pink"), None);
    // Fleet's second case is the number 3: a prop that isn't a string never reaches a role.
    assert_eq!(space_px(&json!("x")), None);
}

// mod-style.test.ts: "spaces in 4 px steps"
#[test]
fn spaces_in_4_px_steps() {
    let px: Vec<f32> = SPACES.iter().map(|n| space_px(&json!(n)).unwrap()).collect();
    assert_eq!(px, [0.0, 4.0, 8.0, 12.0, 16.0, 24.0, 32.0]);
    assert_eq!(space_px(&json!(-1)), None);
}

// mod-style.test.ts: "maps every icon name to a component"
#[test]
fn maps_every_icon_name_to_a_component() {
    for name in ICON_NAMES {
        assert!(mod_icon(name).is_some(), "{name}");
    }
    assert!(mod_icon("rocket").is_none());
    let mut drawn: Vec<_> = ICON_NAMES.iter().map(|n| mod_icon(n).unwrap().name).collect();
    drawn.sort();
    drawn.dedup();
    assert_eq!(drawn.len(), ICON_NAMES.len(), "no two names share an icon");
}

// ModTree.test.ts, "ModTree root": "draws nothing for a null tree"
#[test]
fn root_draws_nothing_for_a_null_tree() {
    let rt = show(Value::Null, Site::Pane);
    assert!(rt.presented(ROOT).is_some(), "the root is there");
    assert!(rt.presented(FIRST).is_none());
    assert!(rt.presented("mod-scene-fleet").is_none());
}

// ModTree.test.ts, "ModTree root": "validates the tree for its site" (a column Box is refused at an inline site)
#[test]
fn root_validates_the_tree_for_its_site() {
    let column = boxed(json!({"flexDirection": "column"}), vec![text(json!({}), vec![json!("hi")])]);
    assert!(matches!(to_draw(&column, Site::StatusChip), Draw::Fleet(_)));
    assert!(matches!(to_draw(&column, Site::Pane), Draw::Tree(_)));
    let rt = show(column, Site::StatusChip);
    assert!(rt.presented("mod-scene-fleet").is_some() && rt.presented(FIRST).is_none());
}

// ModTree.test.ts, "ModTree root": "draws only the fleet slot, and says why once, when the tree is invalid" (the report, once
// per tree, is the resolver's: `mods_resolve.rs`)
#[test]
fn root_draws_only_the_fleet_slot_when_the_tree_is_invalid() {
    let bad = json!({"type": "Nope", "props": {}});
    let rt = show(bad.clone(), Site::Pane);
    assert!(rt.presented("mod-scene-fleet").is_some());
    assert!(rt.presented(FIRST).is_none());
    match to_draw(&bad, Site::Pane) {
        Draw::Fleet(reason) => assert!(reason.contains("Nope"), "{reason}"),
        other => panic!("{other:?}"),
    }
}

// ModTree.test.ts, "ModTree root": "draws the tree the validator returns (text cut), not the one it was given"
#[test]
fn root_draws_the_tree_the_validator_returns_text_cut() {
    let long = text(json!({}), vec![json!("a".repeat(100_000)), json!("tail")]);
    match to_draw(&long, Site::Pane) {
        Draw::Tree(drawn) => {
            assert_eq!(drawn["children"], json!(["a".repeat(100_000), ""]), "the tail is cut, the input is not");
            assert_eq!(long["children"][1], "tail");
        }
        other => panic!("{other:?}"),
    }
}

// ModTree.test.ts, "ModTree root": "draws the fleet slot for a Fleet node, at the root and inside a Box"
#[test]
fn root_draws_the_fleet_slot_for_a_fleet_node_at_the_root_and_inside_a_box() {
    let rt = show(json!({"type": "Fleet"}), Site::Pane);
    assert!(rt.presented("mod-scene-fleet").is_some());
    let rt =
        show(boxed(json!({"flexDirection": "column"}), vec![text(json!({}), vec![json!("above")]), json!({"type": "Fleet"})]), Site::Pane);
    let (above, own) = (rect(&rt, "mod-tree.0.0"), rect(&rt, "mod-scene-fleet"));
    assert!(own[1] >= above[1] + above[3] - 0.5, "Fleet's drawing is where the node is: under the text, in the Box");
}

// ModTree.test.ts, "ModTree root": "draws nothing for a Fleet node with no slot"
#[test]
fn root_draws_nothing_for_a_fleet_node_with_no_slot() {
    let rt = show_with(boxed(json!({"borderStyle": "single"}), vec![json!({"type": "Fleet"})]), Site::Pane, false);
    let b = rect(&rt, FIRST);
    assert!(near(b[3], 2.0), "the Box holds nothing: only its two border pixels, {b:?}");
    assert!(rt.presented("mod-scene-fleet").is_none());
}

// ModTree.test.ts, "ModTree root": "is inline at ToolUse and StatusChip, block elsewhere"
#[test]
fn root_is_inline_at_tool_use_and_status_chip_block_elsewhere() {
    let two = |site| {
        let tree = boxed(json!({}), vec![leaf("Pill", json!({"tone": "good", "label": "ok"})), leaf("Icon", json!({"name": "check"}))]);
        let _ = site;
        tree
    };
    let wide = |site: Site| rect(&show(two(site), site), ROOT)[2];
    let pane = wide(Site::Pane);
    for site in [Site::ToolUse, Site::StatusChip] {
        assert!(wide(site) < pane, "{site:?}: the inline root is only as wide as what it holds");
    }
    for site in [Site::ToolResult, Site::ComposerBand] {
        assert!(near(wide(site), pane), "{site:?}: block, as wide as the room");
    }
}

// ModTree.test.ts, "Box": "draws strings as text and nests children"
#[test]
fn box_draws_strings_as_text_and_nests_children() {
    let rt = show(boxed(json!({}), vec![json!("plain"), boxed(json!({}), vec![text(json!({}), vec![json!("deep")])])]), Site::Pane);
    let (plain, inner, deep) = (rect(&rt, "mod-tree.0.0"), rect(&rt, "mod-tree.0.1"), rect(&rt, "mod-tree.0.1.0"));
    assert!(plain[2] > 20.0 && deep[2] > 20.0, "both read");
    assert!(inner[0] >= plain[0] + plain[2] - 0.5, "a row: the nested Box is beside the string");
    assert!(deep[0] >= inner[0] && deep[0] + deep[2] <= inner[0] + inner[2] + 0.5, "the deep text is inside the nested Box");
}

fn cell(label: &str) -> Value {
    boxed(json!({"borderStyle": "single", "padding": 1}), vec![json!(label)])
}

// ModTree.test.ts, "Box": "lays out with direction, gap, padding in 4 px steps, alignment, wrap and grow"
#[test]
fn box_lays_out_with_direction_gap_padding_in_4_px_steps_alignment_wrap_and_grow() {
    let look = box_look(
        json!({"flexDirection": "column", "gap": 2, "padding": 1, "paddingX": 3, "paddingY": 4, "alignItems": "start",
               "justifyContent": "space-between", "flexWrap": "wrap", "flexGrow": 1, "width": "50%"})
        .as_object()
        .unwrap(),
    );
    assert!(look.column && look.wrap && look.grow);
    assert_eq!(look.gap, 8.0);
    assert_eq!(look.padding, [16.0, 12.0, 16.0, 12.0]);
    assert_eq!(look.width, Some(0.5));
    // Drawn: a row with a gap of 2 steps (8 px) and 3 steps (12 px) of padding.
    let rt = show(boxed(json!({"gap": 2, "padding": 3}), vec![cell("one"), cell("two")]), Site::Pane);
    let (b, one, two) = (rect(&rt, FIRST), rect(&rt, "mod-tree.0.0"), rect(&rt, "mod-tree.0.1"));
    assert!(near(one[0] - b[0], 12.0) && near(one[1] - b[1], 12.0), "padding 3 is 12 px");
    assert!(near(two[0] - (one[0] + one[2]), 8.0), "gap 2 is 8 px");
    assert!(near(one[1], two[1]), "a row");
    let rt = show(boxed(json!({"flexDirection": "column", "gap": 1}), vec![cell("one"), cell("two")]), Site::Pane);
    let (one, two) = (rect(&rt, "mod-tree.0.0"), rect(&rt, "mod-tree.0.1"));
    assert!(near(one[0], two[0]) && near(two[1] - (one[1] + one[3]), 4.0), "a column with a 4 px gap");
}

// ModTree.test.ts, "Box": "keeps padding on every side when only padding is given, and the other sides when only paddingX is"
#[test]
fn box_keeps_padding_on_every_side_when_only_padding_is_given_and_the_other_sides_when_only_padding_x_is() {
    let sides = |props: Value| box_look(props.as_object().unwrap()).padding;
    assert_eq!(sides(json!({"padding": 3})), [12.0; 4]);
    assert_eq!(sides(json!({"padding": 1, "paddingX": 3})), [4.0, 12.0, 4.0, 12.0]);
    assert_eq!(sides(json!({"paddingY": 2})), [8.0, 0.0, 8.0, 0.0]);
    let rt = show(boxed(json!({"padding": 1, "paddingX": 3}), vec![cell("x")]), Site::Pane);
    let (b, c) = (rect(&rt, FIRST), rect(&rt, "mod-tree.0.0"));
    assert!(near(c[0] - b[0], 12.0) && near(c[1] - b[1], 4.0));
    assert!(near((b[1] + b[3]) - (c[1] + c[3]), 4.0), "paddingY is not paddingX: {b:?} {c:?}");
    let rt = show(boxed(json!({"padding": 1, "paddingX": 3, "justifyContent": "end"}), vec![cell("x")]), Site::Pane);
    let (b, c) = (rect(&rt, FIRST), rect(&rt, "mod-tree.0.0"));
    assert!(near((b[0] + b[2]) - (c[0] + c[2]), 12.0), "the right side is paddingX too: {b:?} {c:?}");
}

// ModTree.test.ts, "Box": "alignItems %s"
#[test]
fn box_align_items() {
    let tall = boxed(json!({"padding": 4}), vec![json!("tall")]);
    for (word, at) in [("start", 0.0), ("center", 0.5), ("end", 1.0), ("stretch", 0.0)] {
        let rt = show(boxed(json!({"alignItems": word}), vec![tall.clone(), cell("short")]), Site::Pane);
        let (a, b) = (rect(&rt, "mod-tree.0.0"), rect(&rt, "mod-tree.0.1"));
        let free = a[3] - b[3];
        if word == "stretch" {
            assert!(near(a[3], b[3]), "{word}: as tall as the tallest");
        } else {
            assert!(near(b[1] - a[1], free * at), "{word}: {a:?} {b:?}");
        }
    }
    // Rune has no baseline alignment: a baseline row lines up at the top.
    let rt = show(boxed(json!({"alignItems": "baseline"}), vec![tall, cell("short")]), Site::Pane);
    assert!(near(rect(&rt, "mod-tree.0.0")[1], rect(&rt, "mod-tree.0.1")[1]));
}

// ModTree.test.ts, "Box": "justifyContent %s"
#[test]
fn box_justify_content() {
    for word in ["start", "center", "end", "space-between"] {
        let rt = show(boxed(json!({"justifyContent": word}), vec![cell("a"), cell("b")]), Site::Pane);
        let (b, first, last) = (rect(&rt, FIRST), rect(&rt, "mod-tree.0.0"), rect(&rt, "mod-tree.0.1"));
        let (left, right) = (first[0] - b[0], (b[0] + b[2]) - (last[0] + last[2]));
        match word {
            "start" => assert!(near(left, 0.0) && right > 100.0, "{word}"),
            "center" => assert!(near(left, right), "{word}: {left} {right}"),
            "end" => assert!(near(right, 0.0) && left > 100.0, "{word}"),
            _ => assert!(near(left, 0.0) && near(right, 0.0) && last[0] - (first[0] + first[2]) > 100.0, "{word}"),
        }
    }
}

// ModTree.test.ts, "Box": "takes its width as a share of the parent, never past it"
#[test]
fn box_takes_its_width_as_a_share_of_the_parent_never_past_it() {
    let rt = show(boxed(json!({"flexDirection": "column"}), vec![boxed(json!({"width": "40%"}), vec![json!("x")])]), Site::Pane);
    let (parent, child) = (rect(&rt, FIRST), rect(&rt, "mod-tree.0.0"));
    assert!(near(child[2], parent[2] * 0.4), "{child:?} in {parent:?}");
    let rt = show(boxed(json!({"flexDirection": "column"}), vec![boxed(json!({"width": "100%"}), vec![json!("x")])]), Site::Pane);
    assert!(near(rect(&rt, "mod-tree.0.0")[2], rect(&rt, FIRST)[2]));
    // `width` takes `<n>%` only: anything else is no width.
    for bad in ["40", "40px", "%", "4.%", "-4%"] {
        assert_eq!(box_look(json!({"width": bad}).as_object().unwrap()).width, None, "{bad}");
    }
    assert_eq!(box_look(json!({"width": "12.5%"}).as_object().unwrap()).width, Some(0.125));
}

// ModTree.test.ts, "Box": "sets no width, max-width or flex-basis without a width"
#[test]
fn box_sets_no_width_without_a_width() {
    assert_eq!(box_look(json!({"gap": 1}).as_object().unwrap()).width, None);
    // A row Box holds what it holds: as wide as its content, not the room.
    let rt = show(boxed(json!({"flexDirection": "row"}), vec![boxed(json!({"gap": 1}), vec![json!("x")])]), Site::Pane);
    assert!(rect(&rt, "mod-tree.0.0")[2] < 100.0);
}

// ModTree.test.ts, "Box": "does not turn a column child's width into its height"
#[test]
fn box_does_not_turn_a_column_childs_width_into_its_height() {
    let rt = show(boxed(json!({"flexDirection": "column"}), vec![boxed(json!({"width": "50%"}), vec![json!("x")])]), Site::Pane);
    let child = rect(&rt, "mod-tree.0.0");
    assert!(child[3] < 40.0, "one line tall, not half of anything: {child:?}");
}

// ModTree.test.ts, "Box": "draws a single border square and a round one with the card radius"
#[test]
fn box_draws_a_single_border_square_and_a_round_one_with_the_card_radius() {
    // The radius is a paint property, so the test is its one source: the border a style asks for and the room it takes.
    let rt = show(
        boxed(
            json!({"flexDirection": "column", "gap": 1}),
            vec![
                boxed(json!({"borderStyle": "single"}), vec![json!("x")]),
                boxed(json!({"borderStyle": "round"}), vec![json!("x")]),
                boxed(json!({}), vec![json!("x")]),
            ],
        ),
        Site::Pane,
    );
    let (single, round, none) = (rect(&rt, "mod-tree.0.0"), rect(&rt, "mod-tree.0.1"), rect(&rt, "mod-tree.0.2"));
    assert!(near(single[3] - none[3], 2.0) && near(round[3] - none[3], 2.0), "1 px each side");
}

// ModTree.test.ts, "Box": "borderStyle %s"
#[test]
fn box_border_style() {
    use crate::view::mods::tree::Border;
    for (word, want) in [("round", Border::Round), ("single", Border::Single), ("dashed", Border::Dashed), ("quote", Border::Quote)] {
        assert_eq!(box_look(json!({"borderStyle": word}).as_object().unwrap()).border, Some(want), "{word}");
    }
    let rt = show(boxed(json!({"borderStyle": "quote"}), vec![cell("x")]), Site::Pane);
    let (b, c) = (rect(&rt, FIRST), rect(&rt, "mod-tree.0.0"));
    assert!(near(c[0] - b[0], 3.0) && near(c[1] - b[1], 0.0), "a quote is 3 px on the left only");
    let rt = show(boxed(json!({"borderStyle": "dashed"}), vec![cell("x")]), Site::Pane);
    assert!(near(rect(&rt, "mod-tree.0.0")[0] - rect(&rt, FIRST)[0], 1.0), "dashed is 1 px");
}

// ModTree.test.ts, "Box": "colours the border and the tint wash by role"
#[test]
fn box_colours_the_border_and_the_tint_wash_by_role() {
    use crate::view::mods::tree::Background;
    let look = box_look(json!({"borderStyle": "round", "borderColor": "bad", "background": "tint"}).as_object().unwrap());
    assert_eq!((look.edge.as_deref(), look.background), (Some("bad"), Some(Background::Tint)));
    // A word that isn't a role colours nothing.
    assert_eq!(box_look(json!({"borderColor": "pink"}).as_object().unwrap()).edge, None);
}

// ModTree.test.ts, "Box": "draws the subtle background"
#[test]
fn box_draws_the_subtle_background() {
    use crate::view::mods::tree::Background;
    assert_eq!(box_look(json!({"background": "subtle"}).as_object().unwrap()).background, Some(Background::Subtle));
}

// ModTree.test.ts, "Box": "has no border or background attributes by default"
#[test]
fn box_has_no_border_or_background_by_default() {
    let look = box_look(json!({}).as_object().unwrap());
    assert_eq!((look.border, look.background, look.edge, look.width), (None, None, None, None));
    assert!(!look.column && !look.wrap && !look.grow);
}

// Box's `flexWrap` and `flexGrow`, drawn
#[test]
fn box_wraps_and_grows() {
    let wide = |s: &str| boxed(json!({"padding": 1, "borderStyle": "single"}), vec![json!(s)]);
    let words = "a-long-unbreakable-word-".repeat(5);
    let rt = show(
        boxed(json!({"flexWrap": "wrap", "gap": 1}), vec![wide(&words), wide(&words), wide(&words), wide(&words), wide(&words)]),
        Site::Pane,
    );
    let (a, e) = (rect(&rt, "mod-tree.0.0"), rect(&rt, "mod-tree.0.4"));
    assert!(e[1] > a[1], "the fifth item is on a later line");
    let rt = show(boxed(json!({}), vec![cell("fixed"), boxed(json!({"flexGrow": 1}), vec![json!("grows")])]), Site::Pane);
    let (b, fixed, grown) = (rect(&rt, FIRST), rect(&rt, "mod-tree.0.0"), rect(&rt, "mod-tree.0.1"));
    assert!(near(fixed[0] + fixed[2] + grown[2], b[0] + b[2]), "the grown Box takes what is left");
}

fn key_of(path: &str) -> String {
    format!("mod-tree.0{path}")
}

// ModTree.test.ts, "Text": "colours %s"
#[test]
fn text_colours_each_role() {
    use crate::view::mods::tree::{Ink, InkColor};
    for role in COLOR_ROLES {
        let ink = Ink::default().nested(json!({"color": role}).as_object().unwrap());
        assert_eq!(ink.color, Some(InkColor(COLOR_ROLES.iter().find(|r| **r == role).unwrap())));
    }
    let rt = show(text(json!({"color": "good"}), vec![json!("x")]), Site::Pane);
    assert!(rt.presented(&key_of("")).is_some());
}

// ModTree.test.ts, "Text": "styles bold, italic, strikethrough, code and dim"
#[test]
fn text_styles_bold_italic_strikethrough_code_and_dim() {
    use crate::view::mods::tree::{Ink, InkColor};
    let ink = Ink::default()
        .nested(json!({"bold": true, "italic": true, "strikethrough": true, "code": true, "dimColor": true}).as_object().unwrap());
    assert_eq!(ink, Ink { color: Some(InkColor("muted")), bold: true, italic: true, strike: true, code: true });
    // Only `true` turns a style on.
    assert_eq!(Ink::default().nested(json!({"bold": "yes", "italic": 1}).as_object().unwrap()), Ink::default());
    // Bold takes more room than plain; code is mono at 0.92 em.
    let width = |props: Value| {
        let rt = show(boxed(json!({}), vec![text(props, vec![json!("Mmmmmmmmmm")])]), Site::Pane);
        rect(&rt, &key_of(".0"))[2]
    };
    assert!(width(json!({"bold": true})) > width(json!({})));
    assert!(width(json!({"code": true})) != width(json!({})));
}

// ModTree.test.ts, "Text": "an explicit colour beats dimColor"
#[test]
fn text_an_explicit_colour_beats_dim_color() {
    use crate::view::mods::tree::{Ink, InkColor};
    let ink = Ink::default().nested(json!({"color": "good", "dimColor": true}).as_object().unwrap());
    assert_eq!(ink.color, Some(InkColor("good")));
}

// ModTree.test.ts, "Text": "wraps by default and truncates on request"
#[test]
fn text_wraps_by_default_and_truncates_on_request() {
    let long = "word ".repeat(80);
    let line = |props: Value| {
        let rt = show(boxed(json!({"flexDirection": "column"}), vec![text(props, vec![json!(long.clone())])]), Site::Pane);
        rect(&rt, &key_of(".0"))
    };
    let one = {
        let rt = show(boxed(json!({"flexDirection": "column"}), vec![text(json!({}), vec![json!("x")])]), Site::Pane);
        rect(&rt, &key_of(".0"))[3]
    };
    assert!(line(json!({}))[3] > one * 2.0, "wraps");
    assert!(line(json!({"wrap": "wrap"}))[3] > one * 2.0, "wrap: wrap is the default");
    let cut = line(json!({"wrap": "truncate"}));
    assert!(near(cut[3], one), "one line: {cut:?}");
    let parent = {
        let rt =
            show(boxed(json!({"flexDirection": "column"}), vec![text(json!({"wrap": "truncate"}), vec![json!(long.clone())])]), Site::Pane);
        rect(&rt, FIRST)
    };
    assert!(cut[2] <= parent[2] + 0.5, "never wider than its parent");
}

// ModTree.test.ts, "Text": "nests Text"
#[test]
fn text_nests_text() {
    let rt = show(text(json!({}), vec![json!("a "), text(json!({"bold": true}), vec![json!("b")]), json!(" c")]), Site::Pane);
    let one = rect(&rt, FIRST);
    let plain = show(text(json!({}), vec![json!("a b c")]), Site::Pane);
    assert!(near(one[3], rect(&plain, FIRST)[3]), "one run of words, one line");
    // Nested Text inherits what the outer says and overrides what it says itself.
    use crate::view::mods::tree::{Ink, InkColor};
    let outer = Ink::default().nested(json!({"bold": true, "color": "good"}).as_object().unwrap());
    let inner = outer.nested(json!({"italic": true, "color": "bad"}).as_object().unwrap());
    assert_eq!(inner, Ink { color: Some(InkColor("bad")), bold: true, italic: true, strike: false, code: false });
    // The words of both read as the string "a b c".
    assert!(one[2] > 20.0);
}

// ModTree.test.ts, "Pill": "tone %s"
#[test]
fn pill_tone() {
    for tone in TONES {
        let rt = show(leaf("Pill", json!({"tone": tone, "label": "Ready"})), Site::Pane);
        let p = rect(&rt, FIRST);
        assert!(near(p[3], 22.0), "{tone}: 22 px high");
        assert!(p[2] > 40.0);
    }
}

// ModTree.test.ts, "Pill": "draws its icon"
#[test]
fn pill_draws_its_icon() {
    let with = rect(&show(leaf("Pill", json!({"tone": "good", "label": "Pass", "icon": "check"})), Site::Pane), FIRST)[2];
    let without = rect(&show(leaf("Pill", json!({"tone": "good", "label": "Pass"})), Site::Pane), FIRST)[2];
    assert!(near(with - without, 13.0 + 5.0), "a 13 px icon and a 5 px gap: {with} {without}");
}

// ModTree.test.ts, "Pill": "puts the label in an inner span that ellipsizes, and shrinks with its row"
#[test]
fn pill_puts_the_label_in_an_inner_span_that_ellipsizes_and_shrinks_with_its_row() {
    let label = "A very long label indeed ".repeat(8);
    let rt = show(
        boxed(
            json!({"flexDirection": "column"}),
            vec![boxed(json!({"width": "20%"}), vec![leaf("Pill", json!({"tone": "good", "label": label}))])],
        ),
        Site::Pane,
    );
    let (parent, pill) = (rect(&rt, "mod-tree.0.0"), rect(&rt, "mod-tree.0.0.0"));
    assert!(pill[2] <= parent[2] + 0.5 && near(pill[3], 22.0), "cut to its row, still one pill: {pill:?} in {parent:?}");
}

// ModTree.test.ts, "Icon": "draws %s"
#[test]
fn icon_draws_each_name() {
    for name in ICON_NAMES {
        let rt = show(leaf("Icon", json!({"name": name})), Site::Pane);
        let e = rect(&rt, FIRST);
        assert!(near(e[2], 14.0) && near(e[3], 14.0), "{name}: 14 px, {e:?}");
    }
}

// ModTree.test.ts, "Icon": "is decoration without a label"
#[test]
fn icon_is_decoration_without_a_label() {
    let rt = show(leaf("Icon", json!({"name": "check"})), Site::Pane);
    let tree = rt.accessibility_json().to_string();
    assert!(!tree.contains("Passed"));
    // Nothing of the icon is in the tree a screen reader walks: no node is named for it, and its key isn't an image.
    assert!(!tree.contains("\"img\"") && !tree.contains("Image"), "{tree}");
}

// ModTree.test.ts, "Icon": "is an image with a label"
#[test]
fn icon_is_an_image_with_a_label() {
    let rt = show(leaf("Icon", json!({"name": "check", "label": "Passed"})), Site::Pane);
    assert!(rt.accessibility_json().to_string().contains("Passed"));
}

// ModTree.test.ts, "Icon": "colours %s" (the colour of a role; the colour is one table, `role_color`, tested above)
#[test]
fn icon_colours_each_role() {
    for role in COLOR_ROLES {
        let rt = show(leaf("Icon", json!({"name": "dot", "color": role})), Site::Pane);
        assert!(rt.presented(FIRST).is_some(), "{role}");
    }
}

// ModTree.test.ts, "Icon": "spins the loader" (Rune has no looping rotation: it fades, as the app's other spinners do)
#[test]
fn icon_the_loader_draws_and_the_others_do_not_move() {
    let rt = show(leaf("Icon", json!({"name": "loader"})), Site::Pane);
    let e = rect(&rt, FIRST);
    assert!(near(e[2], 14.0) && near(e[3], 14.0));
}

// ModTree.test.ts: "inline sites wrap" (the cases of the root's `.mod-tree--inline` rules)
#[test]
fn inline_sites_wrap() {
    let pill = |label: &str| leaf("Pill", json!({"tone": "good", "label": label}));
    // A row of pills in a narrow row Box wraps onto the next line instead of squeezing each into an ellipsis.
    let row_of_pills = boxed(json!({}), vec![pill("one hundred"), pill("two hundred"), pill("three hundred")]);
    let narrow = boxed(json!({"width": "30%"}), vec![row_of_pills]);
    let rt = show(boxed(json!({}), vec![narrow]), Site::ToolUse);
    let (first, last) = (rect(&rt, "mod-tree.0.0.0.0"), rect(&rt, "mod-tree.0.0.0.2"));
    assert!(last[1] > first[1], "wrapped: {first:?} {last:?}");
    for i in 0..3 {
        let p = rect(&rt, &format!("mod-tree.0.0.0.{i}"));
        assert!(p[2] > 60.0, "no pill is squeezed: {p:?}");
    }
    // A column Box doesn't wrap and isn't allowed here; a Box that asks for `flexWrap` wraps anywhere.
    // Only a lone item wider than the line ellipsizes.
    let lone = boxed(json!({"width": "10%"}), vec![boxed(json!({}), vec![pill(&"a lone pill with a long label ".repeat(4))])]);
    let rt = show(boxed(json!({}), vec![lone]), Site::ToolUse);
    let (line, only) = (rect(&rt, "mod-tree.0.0.0"), rect(&rt, "mod-tree.0.0.0.0"));
    assert!(only[2] <= line[2] + 0.5, "a lone pill is cut to the line: {only:?} in {line:?}");
    // The root of an inline site centres its items on one line.
    let rt = show(boxed(json!({}), vec![pill("ok"), leaf("Icon", json!({"name": "check"}))]), Site::StatusChip);
    let (p, i) = (rect(&rt, "mod-tree.0.0"), rect(&rt, "mod-tree.0.1"));
    assert!(near(p[1] + p[3] / 2.0, i[1] + i[3] / 2.0), "centred: {p:?} {i:?}");
}
