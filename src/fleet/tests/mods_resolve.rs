//! The cases of weave-fleet's `resolve.test.ts`, `failures.test.ts` and `points.test.ts`, under the same names
//! (snake_case), on `fleet::mods::{resolve, failures, points}`. Failures are an owned value here, not a module-level
//! set, so each test hears only its own trees. "Frozen" and Vue's reactivity have no twin: the shared "nothing drawn"
//! answer is one enum case, and `ViewWatch` stands for `useModView`'s notify-only-on-change.
use crate::fleet::mods::failures::{Failure, Failures};
use crate::fleet::mods::points::*;
use crate::fleet::mods::resolve::{Resolved, Resolver};
use crate::fleet::mods::types::{Author, Site};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

fn pill() -> Value {
    json!({"type": "Pill", "props": {"tone": "good", "label": "ok"}})
}
fn input(key: &str) -> Value {
    json!({"type": "Input", "props": {"key": key}, "handles": {}})
}
fn author(name: &str, draft: bool) -> Author {
    Author { name: name.into(), draft }
}
fn view_with(tree: Value, mods: Vec<Author>) -> Arc<ModTreeView> {
    Arc::new(ModTreeView::new(tree, mods))
}
fn view(tree: Value) -> Arc<ModTreeView> {
    view_with(tree, vec![author("chips", false)])
}

/// A resolver whose failures and warnings are written down.
struct Recorder {
    resolver: Resolver,
    seen: Arc<Mutex<Vec<Failure>>>,
    warned: Arc<Mutex<Vec<String>>>,
}

fn recorder() -> Recorder {
    let failures = Failures::default();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let warned = Arc::new(Mutex::new(Vec::new()));
    let (s, w) = (seen.clone(), warned.clone());
    failures.on_invalid(move |f| s.lock().unwrap().push(f.clone()));
    failures.set_warn(move |m| w.lock().unwrap().push(m.to_string()));
    Recorder { resolver: Resolver::new(failures), seen, warned }
}

fn tree_of<V>(r: &Resolved<V>) -> Arc<Value> {
    match r {
        Resolved::Draws { tree, .. } => tree.clone(),
        Resolved::NothingDrawn => panic!("nothing is drawn"),
    }
}

// ─── resolve ────────────────────────────────────────────────────────────────────────────────────

#[test]
fn does_not_draw_without_a_view_or_without_mods() {
    let r = recorder();
    assert!(!r.resolver.resolve::<ModTreeView>(None, Site::Pane).draws());
    assert!(!r.resolver.resolve(Some(&view_with(pill(), vec![])), Site::Pane).draws());
}

#[test]
fn answers_nothing_drawn_with_one_shared_answer() {
    let r = recorder();
    let a = r.resolver.resolve::<ModTreeView>(None, Site::Pane);
    let b = r.resolver.resolve(Some(&view_with(pill(), vec![])), Site::ToolUse);
    let c = r.resolver.resolve(Some(&view(input("z"))), Site::ToolUse);
    for answer in [a, b, c] {
        assert!(matches!(answer, Resolved::NothingDrawn));
    }
}

#[test]
fn draws_a_valid_tree() {
    let r = recorder();
    let v = view(pill());
    let got = r.resolver.resolve(Some(&v), Site::Pane);
    let Resolved::Draws { tree, draft, view } = got else { panic!("not drawn") };
    // The view's own tree comes back when nothing was cut.
    assert!(Arc::ptr_eq(&tree, &v.tree));
    assert!(!draft);
    assert!(Arc::ptr_eq(&view, &v));
}

#[test]
fn draws_a_null_tree_as_nothing() {
    let r = recorder();
    let got = r.resolver.resolve(Some(&view(Value::Null)), Site::ToolResult);
    assert!(got.draws());
    assert!(tree_of(&got).is_null());
}

#[test]
fn marks_a_draft() {
    let r = recorder();
    let v = view_with(pill(), vec![author("a", false), author("b", true)]);
    assert!(matches!(r.resolver.resolve(Some(&v), Site::Pane), Resolved::Draws { draft: true, .. }));
}

#[test]
fn falls_back_to_fleets_site_for_an_invalid_tree_and_reports_it() {
    let r = recorder();
    let v = view_with(input("q"), vec![author("m", true)]);
    assert!(!r.resolver.resolve(Some(&v), Site::ToolUse).draws());
    let seen = r.seen.lock().unwrap();
    assert_eq!(
        *seen,
        [Failure {
            site: Site::ToolUse,
            reason: "Input is not inline (ToolUse takes inline elements only)".into(),
            mods: vec![author("m", true)]
        }]
    );
}

#[test]
fn draws_the_same_tree_at_a_site_that_allows_it() {
    let r = recorder();
    assert!(r.resolver.resolve(Some(&view(input("q"))), Site::Pane).draws());
}

#[test]
fn reports_an_invalid_tree_once_however_often_it_is_resolved() {
    let r = recorder();
    let v = view(input("q"));
    for _ in 0..5 {
        r.resolver.resolve(Some(&v), Site::StatusChip);
    }
    assert_eq!(r.seen.lock().unwrap().len(), 1);
    // Same tree object at a different site is a different check.
    r.resolver.resolve(Some(&v), Site::Pane);
    assert_eq!(r.seen.lock().unwrap().len(), 1);
    // A new view with a new tree reports again.
    r.resolver.resolve(Some(&view(input("q"))), Site::StatusChip);
    assert_eq!(r.seen.lock().unwrap().len(), 2);
}

#[test]
fn validates_a_tree_once_per_site() {
    let r = recorder();
    let tree = view(pill());
    // A second view over the same tree object (the host's `Arc`) shares the checks.
    let again = Arc::new(ModTreeView { tree: tree.tree.clone(), mods: tree.mods.clone(), on_action: None });
    r.resolver.resolve(Some(&tree), Site::Pane);
    r.resolver.resolve(Some(&again), Site::Pane);
    r.resolver.resolve(Some(&tree), Site::ToolUse);
    assert_eq!(r.resolver.validations(), 2);
}

#[test]
fn warns_in_development() {
    let r = recorder();
    r.resolver.resolve(Some(&view(input("q"))), Site::ToolUse);
    let warned = r.warned.lock().unwrap();
    assert_eq!(warned.len(), 1);
    assert!(warned[0].contains("is not inline"), "{warned:?}");
}

#[test]
fn returns_the_cut_tree_when_text_was_cut() {
    let r = recorder();
    let long = json!({"type": "Markdown", "props": {"text": "a".repeat(100_001)}});
    let got = r.resolver.resolve(Some(&view(long)), Site::Pane);
    assert!(got.draws());
    assert_eq!(tree_of(&got)["props"]["text"].as_str().unwrap().len(), 100_000);
}

#[test]
fn a_tree_that_is_gone_is_forgotten_and_its_address_can_be_reused() {
    let r = recorder();
    for _ in 0..50 {
        // Each round's tree is freed at the end of it; a later tree may land on the same address and is still checked.
        assert!(!r.resolver.resolve(Some(&view(input("q"))), Site::StatusChip).draws());
    }
    assert_eq!(r.seen.lock().unwrap().len(), 50);
}

// ─── failures ───────────────────────────────────────────────────────────────────────────────────

fn failure() -> Failure {
    Failure { site: Site::Pane, reason: "bad".into(), mods: vec![author("m", false)] }
}

#[test]
fn tells_every_listener_until_it_leaves() {
    let f = Failures::default();
    f.set_warn(|_| {});
    let (a, b) = (Arc::new(Mutex::new(Vec::new())), Arc::new(Mutex::new(Vec::new())));
    let (a2, b2) = (a.clone(), b.clone());
    let off_a = f.on_invalid(move |x| a2.lock().unwrap().push(x.clone()));
    let off_b = f.on_invalid(move |x| b2.lock().unwrap().push(x.clone()));
    f.report(&failure());
    f.off(off_a);
    f.report(&failure());
    f.off(off_b);
    assert_eq!(a.lock().unwrap().len(), 1);
    assert_eq!(b.lock().unwrap().len(), 2);
    assert_eq!(a.lock().unwrap()[0], failure());
}

#[test]
fn warns_in_development_naming_the_site_mods_and_reason() {
    let f = Failures::default();
    let warned = Arc::new(Mutex::new(Vec::new()));
    let w = warned.clone();
    f.set_warn(move |m| w.lock().unwrap().push(m.to_string()));
    f.report(&failure());
    let warned = warned.lock().unwrap();
    assert_eq!(warned.len(), 1);
    assert_eq!(warned[0], "[mods] Pane tree from m is invalid: bad");
}

#[test]
fn survives_a_listener_that_throws() {
    let f = Failures::default();
    f.set_warn(|_| {});
    let after = Arc::new(Mutex::new(0));
    let a = after.clone();
    f.on_invalid(|_| panic!("boom"));
    f.on_invalid(move |_| *a.lock().unwrap() += 1);
    f.report(&failure());
    assert_eq!(*after.lock().unwrap(), 1);
}

// ─── points ─────────────────────────────────────────────────────────────────────────────────────

fn mods() -> Vec<Author> {
    vec![author("m", false)]
}
fn base() -> ModTreeView {
    ModTreeView::new(Value::Null, mods())
}
fn row(site: ToolRowSite, session: &str, call: &str, tool: &str) -> ToolRowView {
    ToolRowView { site, session_id: session.into(), call_id: call.into(), tool: tool.into(), view: base() }
}
fn pane(session: &str, id: &str, title: &str, owner: &str) -> ModPaneView {
    ModPaneView { session_id: session.into(), pane_id: id.into(), title: title.into(), mod_id: owner.into(), view: base() }
}
fn band(session: &str, tree: Value, mods: Vec<Author>) -> ComposerBandView {
    ComposerBandView { session_id: session.into(), view: ModTreeView::new(tree, mods) }
}
fn chip(session: &str, mods: Vec<Author>) -> StatusChipView {
    StatusChipView { session_id: session.into(), view: ModTreeView::new(Value::Null, mods) }
}

#[test]
fn builds_tool_row_ids_from_site_session_and_call_and_pane_ids_from_session_mod_and_pane() {
    assert_eq!(tool_row_view_id(ToolRowSite::ToolUse, "s1", "c1"), "ToolUse:s1:c1");
    assert_eq!(mod_pane_view_id("s1", "ci", "p1"), "s1:ci:p1");
    assert_eq!(mod_pane_owner("s1", "ci", "p1"), "mod-pane:s1:ci:p1");
}

#[test]
fn keeps_tool_use_and_tool_result_of_one_call_apart_and_the_same_call_id_in_two_sessions() {
    let mut rows = Views::<ToolRowView>::default();
    let token = rows.contribute(
        "m",
        vec![
            row(ToolRowSite::ToolUse, "s1", "c1", "bash"),
            row(ToolRowSite::ToolResult, "s1", "c1", "bash"),
            row(ToolRowSite::ToolUse, "s2", "c1", "bash"),
        ],
    );
    assert_eq!(rows.len(), 3);
    assert_eq!(rows.get("ToolUse:s1:c1").unwrap().site, ToolRowSite::ToolUse);
    assert_eq!(rows.get("ToolResult:s1:c1").unwrap().site, ToolRowSite::ToolResult);
    assert_eq!(rows.get("ToolUse:s2:c1").unwrap().session_id, "s2");
    rows.dispose(token);
    assert!(rows.is_empty());
}

#[test]
fn contributes_replaces_and_removes_a_composer_band_per_session() {
    let mut bands = Views::<ComposerBandView>::default();
    bands.contribute("m", vec![band("s1", Value::Null, mods())]);
    bands.contribute("m", vec![band("s2", Value::Null, mods())]);
    let mut ids: Vec<String> = bands.items().iter().map(|v| v.session_id.clone()).collect();
    ids.sort();
    assert_eq!(ids, ["s1", "s2"]);
    bands.contribute("m2", vec![band("s1", json!({"type": "Fleet"}), mods())]);
    let next = bands.get("s1").unwrap();
    assert_eq!(*next.view.tree, json!({"type": "Fleet"}));
    assert_eq!(bands.len(), 2);
    bands.remove_by_owner("m");
    assert!(bands.get("s2").is_none());
    assert!(Arc::ptr_eq(&bands.get("s1").unwrap(), &next));
}

#[test]
fn contributes_replaces_and_removes_a_status_chip_per_session() {
    let mut chips = Views::<StatusChipView>::default();
    let token = chips.contribute("m", vec![chip("s1", mods())]);
    assert!(chips.get("s1").is_some());
    chips.contribute("n", vec![chip("s1", vec![author("n", true)])]);
    let replacement = chips.get("s1").unwrap();
    assert!(replacement.view.mods[0].draft);
    // The first owner's disposer leaves a replacement made by someone else.
    chips.dispose(token);
    assert!(Arc::ptr_eq(&chips.get("s1").unwrap(), &replacement));
    assert_eq!(chips.owner_of("s1"), Some("n"));
    chips.remove_by_owner("n");
    assert!(chips.is_empty());
}

#[test]
fn keeps_the_tool_name_as_data_a_row_is_found_without_it() {
    let mut rows = Views::<ToolRowView>::default();
    rows.contribute("m", vec![row(ToolRowSite::ToolUse, "s1", "c1", "whatever")]);
    assert_eq!(rows.get(&tool_row_view_id(ToolRowSite::ToolUse, "s1", "c1")).unwrap().tool, "whatever");
}

#[test]
fn keys_panes_by_session_mod_and_pane_id_and_sorts_by_title() {
    let mut panes = Views::<ModPaneView>::default();
    panes.contribute("m", vec![pane("s1", "b", "Beta", "m"), pane("s1", "a", "Alpha", "m"), pane("s2", "a", "Other", "m")]);
    assert_eq!(panes.get("s1:m:a").unwrap().title, "Alpha");
    let titles = |p: &Views<ModPaneView>| p.items().iter().map(|v| v.title.clone()).collect::<Vec<_>>();
    assert_eq!(titles(&panes), ["Alpha", "Beta", "Other"]);
    panes.contribute("m", vec![pane("s1", "a", "Renamed", "m")]);
    assert_eq!(panes.len(), 3);
    assert_eq!(panes.get("s1:m:a").unwrap().title, "Renamed");
    panes.remove_by_owner("m");
    assert!(panes.is_empty());
}

#[test]
fn two_mods_can_use_the_same_pane_id_in_one_session() {
    let mut panes = Views::<ModPaneView>::default();
    panes.contribute("one", vec![pane("s1", "runs", "One", "one")]);
    panes.contribute("two", vec![pane("s1", "runs", "Two", "two")]);
    let titles: Vec<String> = panes.items().iter().map(|v| v.title.clone()).collect();
    assert_eq!(titles, ["One", "Two"]);
    assert_eq!(panes.get(&mod_pane_view_id("s1", "one", "runs")).unwrap().title, "One");
    assert_eq!(panes.get(&mod_pane_view_id("s1", "two", "runs")).unwrap().title, "Two");
}

#[test]
fn calls_a_view_a_draft_when_any_author_is_a_draft() {
    let v = |drafts: &[bool]| ModTreeView::new(Value::Null, drafts.iter().enumerate().map(|(i, d)| author(&format!("m{i}"), *d)).collect());
    assert!(!is_draft_view(&v(&[])));
    assert!(!is_draft_view(&v(&[false, false])));
    assert!(is_draft_view(&v(&[false, true])));
    assert!(is_draft_view(&v(&[true])));
}

#[test]
fn use_mod_view_notifies_only_when_its_own_ids_view_changes() {
    let mut bands = Views::<ComposerBandView>::default();
    let mut watch = ViewWatch::default();
    let mut seen = Vec::new();
    let mut look = |bands: &Views<ComposerBandView>, seen: &mut Vec<String>| {
        let now = bands.get("s1");
        if watch.changed(now.clone()) {
            seen.push(now.map(|v| v.view.mods[0].name.clone()).unwrap_or_default());
        }
    };
    look(&bands, &mut seen);
    bands.contribute("a", vec![band("s1", Value::Null, vec![author("first", false)])]);
    look(&bands, &mut seen);
    bands.contribute("b", vec![band("s2", Value::Null, mods())]);
    look(&bands, &mut seen);
    bands.remove_by_owner("b");
    look(&bands, &mut seen);
    assert_eq!(seen, ["first"]);
}
