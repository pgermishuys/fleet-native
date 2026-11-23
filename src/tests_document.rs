//! The Document tab and the tool card's visual (component 14, piece E; no Fleet test exists for `VisualCanvas`, so these are
//! ours): title and kind, the card instead of a frame, Expand, the live auto-open, and the rule that places an HTML document.
use crate::fleet::canvases::{CanvasKind, VisualType};
use crate::mock::visuals::{TOTALS, payload};
use crate::tests::{FRAME, app, click};
use crate::web::{Wanted, wanted};
use rune::Runtime;

fn calls() -> Runtime<crate::State> {
    let mut rt = app();
    rt.update(|s, _| s.show_visual_calls(true));
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt
}

fn tab(rt: &mut Runtime<crate::State>, p: crate::fleet::canvases::VisualPayload) {
    rt.update(move |s, _| s.show_document(p, true));
    rt.frame(FRAME);
    rt.frame(FRAME);
    rt.frame(FRAME);
}

fn text_of(rt: &Runtime<crate::State>, key: &str) -> String {
    rt.elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some(key) && !e.exiting)
        .and_then(|e| e.text)
        .unwrap_or_else(|| panic!("no text keyed {key:?}"))
}

#[test]
fn a_call_that_printed_markdown_draws_it_in_a_box_with_expand() {
    let rt = calls();
    assert!(rt.find("tool-v1-visual").is_some());
    assert!(rt.find("tool-v1-visual-markdown").is_some(), "drawn inline, as Rune's markdown");
    assert!(rt.find("tool-v1-visual-expand").is_some());
    assert!(rt.find("tool-v1-output").is_none(), "the box takes the place of the output");
}

#[test]
fn a_call_that_printed_html_is_a_card_with_its_title_and_expand_not_a_frame() {
    let rt = calls();
    assert!(rt.find("tool-v2-visual-html").is_some());
    assert_eq!(text_of(&rt, "tool-v2-visual-html-title"), "Order totals");
    assert_eq!(text_of(&rt, "tool-v2-visual-html-kind"), "HTML");
    assert!(rt.find("tool-v2-visual-html-expand").is_some());
    assert!(rt.find("web-frame").is_none());
}

#[test]
fn expand_opens_the_visual_as_the_active_tab_and_shows_the_panel() {
    let mut rt = calls();
    rt.update(|s, _| s.content_open = false);
    click(&mut rt, "tool-v1-visual-expand");
    rt.frame(FRAME);
    assert!(rt.state.content_open);
    assert_eq!(rt.state.active_canvas(), "visual:Cache eviction");
    assert!(rt.find("document-tab").is_some());
}

#[test]
fn expanding_a_revised_document_again_keeps_one_tab() {
    let mut rt = calls();
    click(&mut rt, "tool-v1-visual-expand");
    rt.update(|s, _| s.expand_visual(payload(VisualType::Markdown, Some("Cache eviction"))));
    let tabs = rt.state.open_canvases();
    assert_eq!(tabs.canvases.iter().filter(|c| c.kind == CanvasKind::Visual).count(), 1);
}

#[test]
fn the_bar_names_a_markdown_document_with_its_title_and_the_word_document() {
    let mut rt = app();
    tab(&mut rt, payload(VisualType::Markdown, Some("Cache eviction")));
    assert_eq!(text_of(&rt, "document-title"), "Cache eviction");
    assert_eq!(text_of(&rt, "document-kind"), "Document");
    assert!(rt.find("document-markdown").is_some());
    let (_, _, _, h) = rt.presented("document-bar").map(|r| (r.0[0], r.0[1], r.0[2], r.0[3])).unwrap_or_default();
    assert_eq!(h, 40.0);
}

#[test]
fn the_bar_names_an_html_page_with_the_word_html_and_no_web_view_here_says_so() {
    let mut rt = app();
    tab(&mut rt, payload(VisualType::Html, Some("Order totals")));
    assert_eq!(text_of(&rt, "document-kind"), "HTML");
    assert!(rt.find("document-html-fallback").is_some(), "headless: no window, so the fallback panel");
    assert_eq!(wanted(&rt.state), Wanted::Nothing);
}

#[test]
fn a_document_with_no_title_takes_the_name_of_its_file_and_then_document() {
    let mut rt = app();
    let mut p = payload(VisualType::Markdown, None);
    p.source_path = Some("/work/notes/cache-eviction.md".into());
    tab(&mut rt, p);
    assert_eq!(text_of(&rt, "document-title"), "cache-eviction.md");
    tab(&mut rt, payload(VisualType::Markdown, None));
    assert_eq!(text_of(&rt, "document-title"), "Document");
}

#[test]
fn a_long_title_is_cut_to_one_line_and_the_kind_stays() {
    let mut rt = app();
    let title = "Cache eviction, the long version with every table and the reasons behind each of the numbers in it";
    tab(&mut rt, payload(VisualType::Markdown, Some(title)));
    assert!(rt.find("document-kind").is_some());
    let (_, _, w, _) = rt.presented("document-title").map(|r| (r.0[0], r.0[1], r.0[2], r.0[3])).unwrap_or_default();
    assert!(w < 700.0, "one line, cut: {w}");
}

#[test]
fn an_html_document_is_placed_where_its_area_is_with_scripts_refused() {
    let mut rt = app();
    tab(&mut rt, payload(VisualType::Html, Some("Order totals")));
    // A window that can host a web view: the area's frame is drawn and its rectangle is what the host is told.
    rt.update(|s, _| {
        s.web.available = true;
        s.web.rect = Some([600.0, 60.0, 360.0, 500.0]);
    });
    let Wanted::Shown(p) = wanted(&rt.state) else { panic!("{:?}", wanted(&rt.state)) };
    assert_eq!(p.tab, "visual:Order totals");
    assert!(p.url.is_empty());
    let html = p.html.unwrap_or_default();
    assert!(html.contains("script-src 'none'") && html.contains("Order totals"));
    assert!(html.find("script-src").unwrap_or(usize::MAX) < html.find("<style>").unwrap_or(0), "the policy comes first");
}

#[test]
fn a_revised_html_document_loads_again_and_an_unchanged_one_does_not() {
    let mut rt = app();
    tab(&mut rt, payload(VisualType::Html, Some("Order totals")));
    rt.update(|s, _| {
        s.web.available = true;
        s.web.rect = Some([600.0, 60.0, 360.0, 500.0]);
    });
    let key = |rt: &Runtime<crate::State>| match wanted(&rt.state) {
        Wanted::Shown(p) => p.load_key,
        other => panic!("{other:?}"),
    };
    let first = key(&rt);
    rt.update(|s, _| s.expand_visual(payload(VisualType::Html, Some("Order totals"))));
    assert_eq!(key(&rt), first, "the same content is the same load");
    let mut revised = payload(VisualType::Html, Some("Order totals"));
    revised.content = TOTALS.replace("19.50", "21.00");
    rt.update(move |s, _| s.expand_visual(revised));
    assert_ne!(key(&rt), first);
}

#[test]
fn a_document_policy_goes_after_the_head_tag_or_makes_one() {
    use crate::web::html_document;
    assert!(html_document("<html><head lang=en><title>a</title></head>").starts_with("<html><head lang=en><meta http-equiv"));
    assert!(html_document("<h1>Hi</h1>").starts_with("<head><meta http-equiv"));
    assert!(html_document("<h1>Hi</h1>").ends_with("</head><h1>Hi</h1>"));
}

// ActivityStream.vue: a visual that arrives while the conversation is open opens as a tab once; history never does.
#[test]
fn a_visual_that_arrives_live_opens_once_and_history_never_does() {
    let mut rt = app();
    rt.update(move |s, _| s.show_visual_calls(true));
    let session = rt.state.canvas_session();
    // The frames above already saw the conversation open, at the real time; every moment here is counted from then.
    let base = rt.state.visuals.mounted_at(&session).unwrap_or_default();
    let ids = |s: &crate::State| s.open_canvases().ids().iter().map(|i| i.to_string()).collect::<Vec<_>>();
    // Both calls were made at 2 ms: long before the conversation was opened here.
    rt.update(move |s, _| s.watch_visuals(base));
    assert!(!ids(&rt.state).iter().any(|i| i.starts_with("visual:")), "history opens nothing: {:?}", ids(&rt.state));
    assert_eq!(rt.state.canvases.session(&session).known_visuals.len(), 2, "but both are listed");
    // A third arrives, made just now.
    rt.update(move |s, _| {
        let mut again = crate::mock::visuals::of_visuals();
        if let Some(m) = again.last_mut() {
            m.meta.created_at = Some(base + 100);
        }
        s.show_conversation(again);
        s.watch_visuals(base + 200);
    });
    assert!(!ids(&rt.state).iter().any(|i| i.starts_with("visual:")), "the same two calls are still history");
    rt.update(move |s, _| {
        let mut live = crate::mock::visuals::of_visuals();
        if let Some(m) = live.last_mut() {
            m.meta.created_at = Some(base + 300);
            m.id = "a2".into();
            for p in &mut m.parts {
                if let crate::view::conversation::Part::Tool { id, .. } = p {
                    id.push_str("-live");
                }
            }
        }
        let mut all = crate::mock::visuals::of_visuals();
        all.extend(live);
        s.show_conversation(all);
        s.watch_visuals(base + 400);
    });
    let opened = ids(&rt.state).iter().filter(|i| i.starts_with("visual:")).count();
    assert_eq!(opened, 2, "the two new calls opened their tabs (same titles, so replaced in place): {:?}", ids(&rt.state));
    rt.update(move |s, _| s.watch_visuals(base + 500));
    assert_eq!(ids(&rt.state).iter().filter(|i| i.starts_with("visual:")).count(), 2, "once per call");
}

// `VisualCanvas.vue` `:annotatable="isMarkdown"` (row 14.85): letting go of a drag over a Markdown document asks the agent about it.
#[test]
fn a_markdown_document_can_be_annotated() {
    use rune::{Key, KeyEvent, Mods};
    let mut rt = app();
    let mut p = payload(VisualType::Markdown, Some("Cache eviction"));
    p.source_path = Some("notes/cache-eviction.md".into());
    tab(&mut rt, p);
    assert!(rt.state.annotation.open.is_none());
    let r = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("document-markdown") && !e.exiting).expect("the text").rect;
    let (y, x0, x1) = (r[1] + 10.0, r[0] + 2.0, r[0] + r[2] * 0.6);
    rt.pointer_move(x0, y);
    rt.pointer_down(x0, y);
    rt.frame(FRAME);
    rt.pointer_move(x1, y);
    rt.frame(FRAME);
    rt.pointer_up(x1, y);
    for _ in 0..6 {
        rt.frame(FRAME);
    }
    let open = rt.state.annotation.open.clone().expect("letting go opens the popover");
    assert_eq!(open.path, "notes/cache-eviction.md", "the source file, as Fleet's `sourceFilePath ?? title`");
    assert!(!open.anchor_text.trim().is_empty());
    rt.key(KeyEvent { key: Key::Escape, mods: Mods::default() });
    rt.frame(FRAME);
    assert!(rt.state.annotation.open.is_none());
}

#[test]
fn an_html_document_is_not_annotatable() {
    let mut rt = app();
    tab(&mut rt, payload(VisualType::Html, Some("Order totals")));
    assert!(rt.find("document-markdown").is_none());
}
