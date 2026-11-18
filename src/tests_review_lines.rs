//! The review's hunks, folds and lines (component 12, piece F): `ChangesReview.test.ts` under its names.
use crate::mock::changes::Case;
use crate::tests::{FRAME, app, click};
use crate::{State, conv};
use rune::Runtime;

type Rt = Runtime<State>;

const NOTES: &str = "src/paginate.py";

fn shown(case: Case) -> Rt {
    let mut rt = app();
    rt.update(move |s, _| s.show_changes_scene(case, true));
    settle(&mut rt);
    rt
}

fn settle(rt: &mut Rt) {
    for _ in 0..4 {
        rt.frame(FRAME);
    }
}

fn keys(rt: &Rt, prefix: &str) -> Vec<String> {
    rt.elements().into_iter().filter(|e| !e.exiting).filter_map(|e| e.key).filter(|k| k.starts_with(prefix)).collect()
}

fn lines_in(rt: &Rt, path: &str) -> usize {
    keys(rt, &format!("review-line-{path}-")).len()
}

// ChangesReview.test.ts: "marks only the changed word on a changed line"
#[test]
fn marks_only_the_changed_word_on_a_changed_line() {
    let before = "const a = 1;\nlet total = oldName + 1;\nconst b = 2;\n";
    let after = "const a = 1;\nlet total = newName + 1;\nconst b = 2;\n";
    let items = conv::review::build_review(&conv::diff::parse_diff_lines(before, after));
    let conv::review::ReviewItem::Hunk(hunk) = &items[0] else { panic!("a hunk first") };
    let marked: Vec<&str> = hunk
        .lines
        .iter()
        .flat_map(|l| conv::review::marked_segments(&l.line.content, &l.marks))
        .filter(|(_, on)| *on)
        .map(|(piece, _)| piece)
        .collect();
    assert_eq!(marked, ["oldName", "newName"]);
    // The scene's edit draws its changed lines, each with its marks on the line's text.
    let rt = shown(Case::Review);
    assert!(lines_in(&rt, NOTES) >= 6, "the changed lines and their context are drawn");
    assert!(!keys(&rt, "review-hunk-src/paginate.py-").is_empty(), "a header per hunk");
}

// ChangesReview.test.ts: "opens a folded run of unchanged lines on click"
#[test]
fn opens_a_folded_run_of_unchanged_lines_on_click() {
    let mut rt = shown(Case::Review);
    let before = lines_in(&rt, NOTES);
    let folds = keys(&rt, &format!("review-fold-{NOTES}-"));
    assert!(!folds.is_empty(), "the edit has long unchanged runs, folded");
    let label = rt.elements().into_iter().find(|e| e.key.as_deref() == Some(folds[0].as_str())).and_then(|e| e.label).unwrap();
    assert!(label.contains("unchanged line"), "{label}");
    click(&mut rt, &folds[0]);
    settle(&mut rt);
    assert!(lines_in(&rt, NOTES) > before, "its lines are drawn in place");
    assert!(keys(&rt, &format!("review-fold-{NOTES}-")).len() < folds.len(), "the button is gone");
}

// ChangesReview.test.ts: "wraps long lines only when asked"
#[test]
fn wraps_long_lines_only_when_asked_and_remembers_it() {
    let mut rt = shown(Case::Review);
    let pressed = |rt: &Rt| rt.elements().into_iter().find(|e| e.key.as_deref() == Some("review-wrap")).map(|e| e.fill.a > 0.0);
    let scrolls = |rt: &Rt| {
        rt.elements().into_iter().filter(|e| e.key.as_deref().is_some_and(|k| k.starts_with("review-region-src/paginate.py-"))).count()
    };
    assert!(!rt.state.sidebar.changes_wrap());
    assert!(scrolls(&rt) > 0);
    click(&mut rt, "review-wrap");
    settle(&mut rt);
    assert!(rt.state.sidebar.changes_wrap());
    assert_eq!(pressed(&rt), Some(true));
    // Remembered: the settings file says so and reads back.
    let json = rt.state.sidebar.to_json();
    assert!(json.contains("changesWrap"), "{json}");
    assert!(crate::fleet::settings::Sidebar::parse(&json).changes_wrap());
    click(&mut rt, "review-wrap");
    assert!(!rt.state.sidebar.changes_wrap());
}

// ChangesReview.test.ts: "a huge diff draws its first lines and shows the rest on a click"
#[test]
fn a_huge_diff_draws_its_first_lines_and_shows_the_rest_on_a_click() {
    let mut rt = shown(Case::ReviewLong);
    const PATH: &str = "data/items.json";
    assert!(rt.state.review.is_open(PATH), "1,200 changed lines is under the 1,500 that start shut");
    let drawn = lines_in(&rt, PATH);
    assert!(drawn > 0 && drawn <= 700, "{drawn}");
    let more = format!("review-show-all-{PATH}");
    let label = rt.elements().into_iter().find(|e| e.key.as_deref() == Some(more.as_str())).and_then(|e| e.label).expect("Show more");
    assert!(label.starts_with("Show ") && label.ends_with(" more lines"), "{label}");
    // The button is below the first screen of lines: bring it into view, then press it.
    let key = more.clone();
    rt.update(move |_, tx| tx.scroll_into_view(key.clone()));
    settle(&mut rt);
    click(&mut rt, &more);
    settle(&mut rt);
    // Nothing is cut now; the rows themselves are drawn from a window, so count the whole answer.
    assert!(rt.state.review.is_drawn_all(PATH));
    assert!(rt.elements().into_iter().all(|e| e.exiting || e.key.as_deref() != Some(more.as_str())), "the button is gone");
}
