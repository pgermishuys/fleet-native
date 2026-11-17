//! The tab model (component 10, piece A): the cases of weave-fleet's `stores/__tests__/canvases.test.ts`,
//! `lib/__tests__/canvas-registry.test.ts` and `lib/__tests__/panel-content.test.ts` that the model covers, under their names.
use super::super::canvases::*;
use crate::fleet::settings::Sidebar;

fn flow() -> VisualPayload {
    VisualPayload {
        kind: VisualType::Markdown,
        title: Some("Session event flow".into()),
        source_path: None,
        content: "# Session event flow".into(),
    }
}

fn ids(c: &Canvases, session: &str) -> Vec<String> {
    c.session(session).canvases.iter().map(|t| t.id.clone()).collect()
}

fn names(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

// canvases.test.ts: "starts every session with Changes and Files, Changes active"
#[test]
fn starts_every_session_with_changes_and_files_changes_active() {
    let c = Canvases::default();
    assert_eq!(ids(&c, "s1"), names(&["changes", "files"]));
    assert_eq!(c.session("s1").active_id, "changes");
}

// "keeps canvases separate per session"
#[test]
fn keeps_canvases_separate_per_session() {
    let mut c = Canvases::default();
    c.activate("s1", "files");
    c.open_visual("s2", flow(), true);
    assert_eq!(c.session("s1").active_id, "files");
    assert_eq!(c.session("s1").canvases.len(), 2);
    assert_eq!(c.session("s2").active_id, visual_id(&flow()));
}

// "opens a visual as a new active tab and updates it in place when revised"
#[test]
fn opens_a_visual_as_a_new_active_tab_and_updates_it_in_place_when_revised() {
    let mut c = Canvases::default();
    c.open_visual("s1", flow(), true);
    c.activate("s1", "changes");
    let revised = VisualPayload { content: "# Hub".into(), ..flow() };
    c.open_visual("s1", revised.clone(), true);
    let s = c.session("s1");
    let visuals: Vec<_> = s.canvases.iter().filter(|t| t.kind == CanvasKind::Visual).collect();
    assert_eq!(visuals.len(), 1);
    assert_eq!(visuals[0].payload.as_ref(), Some(&revised));
    // It was asked to take focus, so the revised one does (Fleet's test reads the id, which is the same one).
    assert_eq!(s.active_id, visual_id(&flow()));
}

// "can open a visual without taking focus"
#[test]
fn can_open_a_visual_without_taking_focus() {
    let mut c = Canvases::default();
    c.open_visual("s1", flow(), false);
    assert_eq!(c.session("s1").active_id, "changes");
    assert_eq!(c.session("s1").canvases.len(), 3);
}

// "closing the active canvas activates its left neighbour"
#[test]
fn closing_the_active_canvas_activates_its_left_neighbour() {
    let mut c = Canvases::default();
    c.open_visual("s1", flow(), true);
    c.close("s1", &visual_id(&flow()));
    assert_eq!(c.session("s1").active_id, "files");
}

#[test]
fn closing_the_first_tab_takes_the_one_now_in_its_place() {
    let mut c = Canvases::default();
    c.introduce("s1", CanvasKind::Context);
    c.activate("s1", "context");
    c.close("s1", "context");
    assert_eq!(c.session("s1").active_id, "changes");
}

#[test]
fn closing_a_tab_that_is_not_active_leaves_the_active_one() {
    let mut c = Canvases::default();
    c.open_visual("s1", flow(), false);
    assert!(c.close("s1", &visual_id(&flow())).is_some());
    assert_eq!(c.session("s1").active_id, "changes");
    assert!(c.close("s1", &visual_id(&flow())).is_none(), "a second close does nothing");
}

// "never closes Changes"
#[test]
fn never_closes_changes() {
    let mut c = Canvases::default();
    assert!(c.close("s1", "changes").is_none());
    assert!(ids(&c, "s1").contains(&"changes".to_string()));
    assert!(!c.session("s1").canvases[0].closable());
}

// "reopens a closed built-in canvas at the end and focuses it"
#[test]
fn reopens_a_closed_built_in_canvas_at_the_end_and_focuses_it() {
    let mut c = Canvases::default();
    c.close("s1", "files");
    c.open_visual("s1", flow(), true);
    c.open("s1", CanvasKind::Files);
    let s = c.session("s1");
    assert_eq!(ids(&c, "s1"), names(&["changes", &visual_id(&flow()), "files"]));
    assert_eq!(s.active_id, "files");
}

// "introduces Context as the first tab once, without taking focus"
#[test]
fn introduces_context_as_the_first_tab_once_without_taking_focus() {
    let mut c = Canvases::default();
    c.introduce("s1", CanvasKind::Context);
    c.introduce("s1", CanvasKind::Context);
    assert_eq!(ids(&c, "s1"), names(&["context", "changes", "files"]));
    assert_eq!(c.session("s1").active_id, "changes");

    // Closing it keeps it closed; opening it again brings it back.
    c.close("s1", "context");
    c.introduce("s1", CanvasKind::Context);
    assert_eq!(ids(&c, "s1"), names(&["changes", "files"]));
    c.open("s1", CanvasKind::Context);
    assert_eq!(c.session("s1").active_id, "context");
}

#[test]
fn only_a_built_in_opens_or_is_introduced_by_kind() {
    let mut c = Canvases::default();
    c.open("s1", CanvasKind::Page);
    c.introduce("s1", CanvasKind::Browser);
    assert_eq!(ids(&c, "s1"), names(&["changes", "files"]));
}

// "persists Widen across reloads"
#[test]
fn persists_widen_across_reloads() {
    let mut saved = Sidebar::default();
    saved.save_canvas_widened(true);
    let back = Sidebar::parse(&saved.to_json());
    assert!(back.canvas_widened());
    assert!(saved.to_json().contains("canvasWidened"));
    let mut off = back.clone();
    off.save_canvas_widened(false);
    assert!(!Sidebar::parse(&off.to_json()).canvas_widened(), "and off again");
    assert!(!Sidebar::parse("{}").canvas_widened());
}

// "titles visuals by title, then file name, then a generic label"
#[test]
fn titles_visuals_by_title_then_file_name_then_a_generic_label() {
    assert_eq!(visual_title(&flow()), "Session event flow");
    let by_file =
        VisualPayload { kind: VisualType::Markdown, title: None, source_path: Some("docs/plan.md".into()), content: String::new() };
    assert_eq!(visual_title(&by_file), "plan.md");
    let bare = VisualPayload { kind: VisualType::Html, title: None, source_path: None, content: "<p/>".into() };
    assert_eq!(visual_title(&bare), "Document");
}

// "titles a visual without them" (terminal codes)
#[test]
fn titles_a_visual_without_them() {
    let p = VisualPayload {
        kind: VisualType::Html,
        title: Some("\u{1b}[31mReport\u{1b}[0m".into()),
        source_path: None,
        content: "<p/>".into(),
    };
    assert_eq!(visual_title(&p), "Report");
}

// "openChanges makes Changes active for review and can ask for a file"
#[test]
fn open_changes_makes_changes_active_for_review_and_can_ask_for_a_file() {
    let mut c = Canvases::default();
    c.open_changes("s1", Some("src/a.ts"));
    let s = c.session("s1");
    assert_eq!(s.active_id, "changes");
    assert!(s.review);
    assert_eq!(s.review_focus, Some(ReviewFocus { path: "src/a.ts".into(), seq: 1 }));
    c.open_changes("s1", Some("src/a.ts"));
    assert_eq!(c.session("s1").review_focus.map(|f| f.seq), Some(2));
}

#[test]
fn open_changes_adds_it_first_when_it_is_missing_and_keeps_the_focus_without_a_path() {
    let mut c = Canvases::default();
    c.open_changes("s1", Some("a"));
    c.close("s1", "files");
    // Changes can't be closed, so take it away by hand to see the rule.
    let mut s = c.session("s1");
    s.canvases.retain(|t| t.id != "changes");
    c.by_session_for_test("s1", s);
    c.open_changes("s1", None);
    let s = c.session("s1");
    assert_eq!(s.ids()[0], "changes");
    assert_eq!(s.review_focus.map(|f| f.seq), Some(1));
}

// "leaving Changes ends the review; clicking its tab again starts it"
#[test]
fn leaving_changes_ends_the_review_clicking_its_tab_again_starts_it() {
    let mut c = Canvases::default();
    c.open_changes("s1", None);
    c.activate("s1", "files");
    assert!(!c.session("s1").review);
    c.activate("s1", "changes");
    assert!(c.session("s1").review);
}

#[test]
fn pressing_changes_again_leaves_the_review_as_it_was() {
    let mut c = Canvases::default();
    c.activate("s1", "changes");
    assert!(!c.session("s1").review, "a session that was never reviewing isn't made to");
    c.open_changes("s1", None);
    c.activate("s1", "changes");
    assert!(c.session("s1").review);
}

#[test]
fn activating_a_tab_that_isnt_there_does_nothing() {
    let mut c = Canvases::default();
    c.activate("s1", "turns");
    assert_eq!(c.session("s1").active_id, "changes");
}

#[test]
fn opening_changes_through_open_is_for_review_and_any_other_built_in_is_not() {
    let mut c = Canvases::default();
    c.open("s1", CanvasKind::Changes);
    assert!(c.session("s1").review);
    c.open("s1", CanvasKind::Turns);
    assert!(!c.session("s1").review);
    assert!(c.session("s1").introduced.contains(&CanvasKind::Turns));
}

#[test]
fn keeps_the_conversations_visuals_and_when_a_tab_updated_itself() {
    let mut c = Canvases::default();
    c.set_known_visuals("s1", vec![flow()]);
    assert_eq!(c.session("s1").known_visuals, vec![flow()]);
    assert_eq!(c.updated_at("canvas:a"), None);
    c.mark_updated("canvas:a", 4.5);
    assert_eq!(c.updated_at("canvas:a"), Some(4.5));
}

// canvas-registry.test.ts: "has these kinds, each with its own label"
#[test]
fn has_these_kinds_each_with_its_own_label() {
    let kinds: Vec<_> = CanvasKind::ALL.iter().map(|k| k.slug()).collect();
    assert_eq!(kinds, ["changes", "files", "context", "progress", "turns", "agents", "visual", "browser", "page", "file", "mod"]);
    let labels: Vec<_> = CanvasKind::ALL.iter().map(|k| k.label()).collect();
    assert_eq!(labels, ["Changes", "Files", "Context", "Progress", "Turns", "Agents", "Document", "Browser", "Page", "File", "Mod"]);
    let icons: Vec<_> = CanvasKind::ALL.iter().map(|k| k.icon().name).collect();
    assert_eq!(
        icons,
        [
            "lucide-git-compare",
            "lucide-folder-tree",
            "lucide-paperclip",
            "lucide-list-checks",
            "lucide-history",
            "lucide-network",
            "lucide-file-text",
            "lucide-globe",
            "lucide-panels-top-left",
            "lucide-file",
            "lucide-puzzle"
        ]
    );
}

// "offers these in the + menu, in this order"
#[test]
fn offers_these_in_the_plus_menu_in_this_order() {
    let slugs: Vec<_> = PICKABLE.iter().map(|k| k.slug()).collect();
    assert_eq!(slugs, ["context", "progress", "changes", "files", "turns", "agents"]);
}

fn tab(kind: CanvasKind) -> CanvasInstance {
    CanvasInstance::built_in(kind)
}

// "titles a canvas by its file, page, browser, payload, or kind"
#[test]
fn titles_a_canvas_by_its_file_page_browser_payload_or_kind() {
    assert_eq!(tab(CanvasKind::Changes).title(), "Changes");
    assert_eq!(CanvasInstance { file: Some("src/a/b.ts".into()), ..tab(CanvasKind::File) }.title(), "b.ts");
    assert_eq!(CanvasInstance { title: Some("Options".into()), ..tab(CanvasKind::Page) }.title(), "Options");
    assert_eq!(CanvasInstance { title: Some("Tab".into()), ..tab(CanvasKind::Browser) }.title(), "Tab");
    let notes = VisualPayload { kind: VisualType::Markdown, title: Some("Notes".into()), source_path: None, content: "# Notes".into() };
    assert_eq!(CanvasInstance { payload: Some(notes), ..tab(CanvasKind::Visual) }.title(), "Notes");
}

// "icons a canvas by its file, payload or kind"
#[test]
fn icons_a_canvas_by_its_file_payload_or_kind() {
    assert_eq!(tab(CanvasKind::Files).icon().name, CanvasKind::Files.icon().name);
    let json = CanvasInstance { file: Some("a.json".into()), ..tab(CanvasKind::File) };
    assert_eq!(json.icon().name, "lucide-file-braces");
    let html = VisualPayload { kind: VisualType::Html, title: None, source_path: None, content: "<p/>".into() };
    assert_ne!(CanvasInstance { payload: Some(html), ..tab(CanvasKind::Visual) }.icon().name, CanvasKind::Visual.icon().name);
}

#[test]
fn files_are_iconed_by_type() {
    assert_eq!(file_icon("notes/plan.MD").name, "lucide-file-text");
    assert_eq!(file_icon("src/main.rs").name, "lucide-file-code");
    assert_eq!(file_icon("Makefile").name, "lucide-file");
    assert_eq!(file_icon("a.jsonc").name, "lucide-file-braces");
    assert_eq!(file_name("a/b/"), "a/b/", "nothing after the last slash: the path itself");
}

// "throws for a kind nobody registered"
#[test]
fn throws_for_a_kind_nobody_registered() {
    assert_eq!(CanvasKind::parse("mystery"), Err(UnknownKind("mystery".into())));
    assert_eq!(CanvasKind::parse("turns"), Ok(CanvasKind::Turns));
}

// panel-content.test.ts
#[test]
fn has_something_to_show_when_the_changes_failed_to_load_the_panel_opens_on_the_error_and_try_again_not_shut_as_if_empty() {
    let defaults = Canvases::default().session("s1").canvases;
    assert!(panel_has_content(PanelContent { changes_failed: true, canvases: &defaults, ..Default::default() }));
}

#[test]
fn is_empty_for_a_fresh_session() {
    let defaults = Canvases::default().session("s1").canvases;
    assert!(!panel_has_content(PanelContent { canvases: &defaults, ..Default::default() }));
}

#[test]
fn anything_the_person_should_see_is_content() {
    let defaults = Canvases::default().session("s1").canvases;
    let some = |f: fn(&mut PanelContent)| {
        let mut c = PanelContent { canvases: &defaults, ..Default::default() };
        f(&mut c);
        panel_has_content(c)
    };
    assert!(some(|c| c.changed_files = 1));
    assert!(some(|c| c.progress = true));
    assert!(some(|c| c.context = true));
    assert!(some(|c| c.agents = true));
    let with_page = [tab(CanvasKind::Changes), tab(CanvasKind::Page)];
    assert!(panel_has_content(PanelContent { canvases: &with_page, ..Default::default() }));
}
