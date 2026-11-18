//! Asking about rendered text (component 12, piece N): `ChangesReview.test.ts` "rendered markdown can be asked about, like a file tab's",
//! and the popover's own flow (`AnnotationPopover.vue`, `use-annotation.ts`, `format-annotation-prompt.ts`).
use crate::State;
use crate::fleet::annotation::{WIDTH, annotation_prompt, clamp, extract_anchor_text, format_annotation_prompt, preview};
use crate::mock::changes::Case;
use crate::tests::{FRAME, app_tall, click};
use crate::view::conversation::Role;
use rune::{Key, KeyEvent, Mods, Runtime};

type Rt = Runtime<State>;

const MD: &str = "docs/paging.md";
const HTML: &str = "docs/preview.html";

fn settle(rt: &mut Rt) {
    for _ in 0..6 {
        rt.frame(FRAME);
    }
}

fn rendered(path: &str) -> Rt {
    let mut rt = app_tall();
    rt.update(|s, _| s.show_changes_scene(Case::ReviewDocs, true));
    settle(&mut rt);
    let path = path.to_string();
    rt.update(move |s, _| s.set_review_rendered(&path, true));
    settle(&mut rt);
    rt
}

fn rect(rt: &Rt, key: &str) -> [f32; 4] {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting).unwrap_or_else(|| panic!("no element {key}")).rect
}

/// A person drags from the first word of the rendered text across to a point further on, and lets go.
fn drag_across(rt: &mut Rt, key: &str) {
    let r = rect(rt, key);
    let (y, x0, x1) = (r[1] + 10.0, r[0] + 2.0, r[0] + r[2] * 0.6);
    rt.pointer_move(x0, y);
    rt.pointer_down(x0, y);
    rt.frame(FRAME);
    rt.pointer_move(x1, y);
    rt.frame(FRAME);
    rt.pointer_up(x1, y);
    settle(rt);
}

fn type_text(rt: &mut Rt, words: &str) {
    rt.key(KeyEvent { key: Key::Text(words.into()), mods: Mods::default() });
    settle(rt);
}

// ChangesReview.test.ts: "rendered markdown can be asked about, like a file tab's"
#[test]
fn rendered_markdown_can_be_asked_about_like_a_file_tabs() {
    let mut rt = rendered(MD);
    assert!(rt.state.annotation.open.is_none(), "nothing asks until text is selected");
    drag_across(&mut rt, &format!("review-rendered-{MD}-markdown"));
    let open = rt.state.annotation.open.clone().expect("letting go of a selection opens the popover");
    assert_eq!(open.path, MD);
    assert!(!open.anchor_text.trim().is_empty(), "it quotes what was selected");
    assert!(rt.find("annotation-popover").is_some() && rt.find("annotation-field").is_some());
    assert!(rt.state.draft.text.is_empty(), "nothing is in the draft before Send");
}

#[test]
fn a_selection_in_html_asks_nothing() {
    let mut rt = rendered(HTML);
    drag_across(&mut rt, &format!("review-rendered-{HTML}-html"));
    assert!(rt.state.annotation.open.is_none());
    // And dragging over the diff of a file not viewed as Rendered asks nothing either: its lines are for selecting lines.
    let mut rt = app_tall();
    rt.update(|s, _| s.show_changes_scene(Case::ReviewDocs, true));
    settle(&mut rt);
    drag_across(&mut rt, "review-lines-docs/paging.md");
    assert!(rt.state.annotation.open.is_none());
}

#[test]
fn send_is_off_until_there_is_a_comment_then_the_prompt_goes_as_a_message() {
    let mut rt = rendered(MD);
    drag_across(&mut rt, &format!("review-rendered-{MD}-markdown"));
    let quoted = rt.state.annotation.open.clone().unwrap().anchor_text;
    let before = rt.state.session().messages.iter().filter(|m| m.role == Role::User).count();
    click(&mut rt, "annotation-send");
    assert!(rt.state.annotation.open.is_some(), "an empty comment sends nothing");
    // Pressing the off button took the focus; the field takes it back.
    click(&mut rt, "annotation-field");
    type_text(&mut rt, "  Why 25 a page?  ");
    click(&mut rt, "annotation-send");
    settle(&mut rt);
    assert!(rt.state.annotation.open.is_none(), "Send closes it: {:?} {:?}", rt.state.annotation.open, rt.state.annotation.field.text);
    assert!(rt.find("annotation-popover").is_none(), "and it is gone");
    let sent: Vec<String> = rt.state.session().messages.iter().filter(|m| m.role == Role::User).map(|m| m.text()).collect();
    assert_eq!(sent.len(), before + 1);
    assert_eq!(sent.last().unwrap().trim(), annotation_prompt(MD, &quoted, "Why 25 a page?"));
}

#[test]
fn cancel_and_escape_close_it_and_leave_the_draft_alone() {
    let mut rt = rendered(MD);
    rt.update(|s, _| s.set_draft_text("my own words"));
    drag_across(&mut rt, &format!("review-rendered-{MD}-markdown"));
    assert!(rt.state.annotation.open.is_some());
    click(&mut rt, "annotation-cancel");
    assert!(rt.state.annotation.open.is_none());
    drag_across(&mut rt, &format!("review-rendered-{MD}-markdown"));
    type_text(&mut rt, "half a thought");
    rt.key(KeyEvent { key: Key::Escape, mods: Mods::default() });
    settle(&mut rt);
    assert!(rt.state.annotation.open.is_none());
    assert_eq!(rt.state.draft.text, "my own words");
    // Opened again, the field starts empty.
    drag_across(&mut rt, &format!("review-rendered-{MD}-markdown"));
    assert_eq!(rt.state.annotation.field.text, "");
}

#[test]
fn ctrl_enter_sends() {
    let mut rt = rendered(MD);
    drag_across(&mut rt, &format!("review-rendered-{MD}-markdown"));
    type_text(&mut rt, "Is this right?");
    let mods = Mods { ctrl: true, cmd: true, ..Mods::default() };
    rt.key(KeyEvent { key: Key::Enter, mods });
    settle(&mut rt);
    assert!(rt.state.annotation.open.is_none());
    let last = rt.state.session().messages.iter().rev().find(|m| m.role == Role::User).map(|m| m.text()).unwrap();
    assert!(last.starts_with("[Annotation on docs/paging.md]\n> "), "{last}");
}

#[test]
fn the_popover_opens_where_the_selection_ended_and_stays_inside_the_window() {
    assert_eq!(clamp((300.0, 200.0), [1280.0, 760.0]), (300.0, 200.0));
    assert_eq!(clamp((1200.0, 700.0), [1280.0, 760.0]), (1280.0 - WIDTH - 16.0, 760.0 - 300.0 - 16.0));
    assert_eq!(clamp((-5.0, -5.0), [1280.0, 760.0]), (16.0, 16.0));
    let mut rt = rendered(MD);
    drag_across(&mut rt, &format!("review-rendered-{MD}-markdown"));
    let card = rect(&rt, "annotation-popover");
    assert!(card[0] >= 16.0 && card[0] + card[2] <= 1280.0 - 16.0 && card[2] == WIDTH, "{card:?}");
}

#[test]
fn the_prompt_is_formatted_as_fleet_does() {
    assert_eq!(format_annotation_prompt("a/b.md", "the text", "Why?"), "[Annotation on a/b.md]\n> the text\n\nWhy?");
    assert_eq!(format_annotation_prompt("", "the text", "Why?"), "[Annotation]\n> the text\n\nWhy?");
    assert_eq!(annotation_prompt("a.md", "t", "Why?"), "[Annotation on a.md]\n> t\n\nWhy?\n\nContext: t");
    let long = "x".repeat(300);
    assert_eq!(extract_anchor_text(&long), format!("{}…", "x".repeat(240)));
    assert_eq!(extract_anchor_text(&"y".repeat(240)), "y".repeat(240));
    assert_eq!(preview(&long), format!("{}…", "x".repeat(120)));
    assert_eq!(preview("short"), "short");
}
