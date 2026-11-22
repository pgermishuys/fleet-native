//! A mod's Page (component 22, piece J): the "Page" cases of `ModTree.test.ts` and the contained half of
//! `ConversationPage.test.ts`'s "offers to open the page in a new tab, and does not when it is contained". Fleet's tests read the
//! frame's `src`; the page here is a card (decision 1), so they read the address its "Open full size" would load.
use crate::fleet::mods::types::Site;
use crate::mock::mod_trees::ModScene;
use crate::tests::{FRAME, app, click};
use rune::Runtime;
use serde_json::{Value, json};

const CARD: &str = "page-card-mod-tree.0";

fn page(props: Value, owner: &str) -> Value {
    let mut p = json!({"key": "p"});
    p.as_object_mut().unwrap().extend(props.as_object().unwrap().clone());
    json!({"type": "Page", "props": p, "mod": owner})
}

fn show(tree: Value) -> Runtime<crate::State> {
    let mut rt = app();
    rt.update(move |s, _| {
        s.show_mod_elements(true);
        s.mod_scene = Some(ModScene { tree, site: Site::Pane, slot: true });
    });
    for _ in 0..3 {
        rt.frame(FRAME);
    }
    rt
}

fn has(rt: &Runtime<crate::State>, key: &str) -> bool {
    rt.elements().iter().any(|e| e.key.as_deref() == Some(key) && !e.exiting)
}

fn text_of(rt: &Runtime<crate::State>, key: &str) -> String {
    rt.elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some(key))
        .and_then(|e| e.text)
        .unwrap_or_else(|| panic!("no text keyed {key:?}"))
}

/// What "Open full size" loads: the page at the mod's address under the open session.
fn opened(rt: &mut Runtime<crate::State>) -> String {
    click(rt, &format!("{CARD}-expand"));
    let full = rt.state.page_full().cloned().expect("the dialog is open");
    rt.state.page_full_url(&full).expect("an address")
}

// ModTree.test.ts "Page": "draws the page through the conversation frame, at the mod's address under the session"
#[test]
fn draws_the_page_through_the_conversation_card_at_the_mods_address_under_the_session() {
    let mut rt = show(page(json!({"path": "ui/report.html", "title": "Report", "query": {"run": "1"}}), "demo-mod@v3"));
    assert_eq!(text_of(&rt, &format!("{CARD}-title")), "Report");
    assert_eq!(text_of(&rt, &format!("{CARD}-file")), "report.html");
    let session = rt.state.canvas_session();
    let url = opened(&mut rt);
    assert!(url.ends_with(&format!("/api/sessions/{session}/mods/demo-mod%40v3/pages/ui/report.html?run=1")), "{url}");
}

// ModTree.test.ts "Page": "takes the owner from the node, not from the props"
#[test]
fn takes_the_owner_from_the_node_not_from_the_props() {
    let mut rt = show(page(json!({"path": "a.html", "title": "A"}), "other-mod@draft:ses_2"));
    assert!(opened(&mut rt).contains("/mods/other-mod%40draft%3Ases_2/pages/a.html"));
}

// ModTree.test.ts "Page": "has no open in a new tab control, but still opens full size"
// ConversationPage.test.ts "offers to open the page in a new tab, and does not when it is contained" (the contained half)
#[test]
fn has_no_open_in_a_new_tab_control_but_still_opens_full_size() {
    let rt = show(page(json!({"path": "a.html", "title": "A"}), "demo-mod@v3"));
    assert!(!has(&rt, &format!("{CARD}-open")), "a contained page has no new tab");
    assert!(has(&rt, &format!("{CARD}-expand")));
}

// ModTree.test.ts "Page": "is a quiet placeholder for a %s path"
#[test]
fn is_a_quiet_placeholder_for_a_bad_path() {
    for path in ["../../../api/sessions.html", "javascript:alert(1).html", "https://evil.example/x.html", "ui/x.js"] {
        let rt = show(page(json!({"path": path, "title": "Report"}), "demo-mod@v3"));
        assert!(!has(&rt, CARD), "{path}: no card");
        assert_eq!(text_of(&rt, "mod-tree.0"), "Report: Page unavailable", "{path}");
    }
}

// ModTree.test.ts "Page": "is a quiet placeholder with the title when there is no session"
#[test]
fn is_a_quiet_placeholder_with_the_title_when_there_is_no_machine_to_serve_it() {
    let mut rt = app();
    rt.update(|s, _| {
        s.show_mod_elements(true);
        s.web.mock_base = None;
        s.mod_scene =
            Some(ModScene { tree: page(json!({"path": "a.html", "title": "Report"}), "demo-mod@v3"), site: Site::Pane, slot: true });
    });
    for _ in 0..3 {
        rt.frame(FRAME);
    }
    assert!(!has(&rt, CARD));
    assert_eq!(text_of(&rt, "mod-tree.0"), "Report: Page unavailable");
}

// ModTree.test.ts "Page": "is a quiet placeholder for a bad owner"
#[test]
fn is_a_quiet_placeholder_for_a_bad_owner() {
    let rt = show(page(json!({"path": "a.html", "title": "Report"}), "../x@v1"));
    assert!(!has(&rt, CARD));
}
