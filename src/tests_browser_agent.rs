//! The browser tab's Your view / Agent's view (component 14, piece J; row 14.67). `BrowserCanvas.vue` has no test of its own for
//! the toggle, so these are ours, read from its rules: the toggle and banner only with an agent tab on this page, Agent's view
//! hides the web view and watches the agent's tab, and the view goes back to the user's own when the agent's tab goes.
use crate::mock::browser_agent::{AGENT_TAB, AgentCase};
use crate::mock::browser_tab::TAB;
use crate::tests::{FRAME, app, click};
use crate::tests_machines_page::exists;
use crate::web::wanted::shown_browser;
use rune::Runtime;

fn shown(case: AgentCase) -> Runtime<crate::State> {
    let mut rt = app();
    rt.update(move |s, _| s.show_browser_agent(case, true));
    for _ in 0..3 {
        rt.frame(FRAME);
    }
    rt
}

fn words(rt: &Runtime<crate::State>) -> String {
    rt.elements().into_iter().filter_map(|e| e.text.or(e.label)).collect::<Vec<_>>().join("\n")
}

fn close_agent_tab(rt: &mut Runtime<crate::State>) {
    rt.update(|s, _| {
        let session = s.canvas_session();
        let step = crate::conv::browser::BrowserStep::read(&serde_json::json!({
            "sessionId": session, "seq": 9, "kind": "tabs.close", "summary": "Closed the tab", "detail": "", "ok": true, "tabId": AGENT_TAB,
        }))
        .unwrap();
        s.browser.apply(&session, step);
    });
    rt.frame(FRAME);
}

#[test]
fn shows_the_toggle_and_the_banner_only_with_an_agent_tab_on_this_page() {
    let mut rt = app();
    rt.update(|s, _| s.show_browser_tab(crate::mock::browser_tab::BrowserCase::Shown));
    rt.frame(FRAME);
    assert!(!exists(&rt, "browser-views") && !exists(&rt, "browser-agent-banner"), "no agent tab, nothing");
    let rt = shown(AgentCase::You);
    assert!(exists(&rt, "browser-views") && exists(&rt, "browser-agent-banner"));
    assert!(words(&rt).contains("The agent is trying this page in its own browser. What it types and clicks doesn't show here."));
    assert!(exists(&rt, "browser-banner-agent"), "the banner offers Agent's view");
}

#[test]
fn an_agent_tab_on_another_site_is_not_this_pages() {
    let mut rt = shown(AgentCase::You);
    rt.update(|s, _| {
        let session = s.canvas_session();
        let tab = crate::conv::agent_tabs::AgentTab::read(&serde_json::json!({"id": "tab_2", "url": "http://localhost:9999/"})).unwrap();
        s.browser.set_tabs(&session, vec![tab], Some("tab_2".into()));
    });
    rt.frame(FRAME);
    assert!(!exists(&rt, "browser-views"), "the agent's tab is on another app");
}

#[test]
fn the_banner_button_and_the_toggle_switch_to_the_agents_view_and_back() {
    let mut rt = shown(AgentCase::You);
    click(&mut rt, "browser-banner-agent");
    rt.frame(FRAME);
    rt.frame(FRAME);
    assert!(rt.state.browser_ui(TAB).unwrap().agent);
    assert_eq!(rt.state.agent_view.target, Some((rt.state.canvas_session(), AGENT_TAB.to_string())), "the agent's tab is watched");
    assert!(exists(&rt, "agent-tab-view"), "component 10's picture fills the page area");
    assert!(words(&rt).contains("Live picture of the agent's tab. You can watch, not click. Its sign-ins and cookies are its own."));
    assert!(!exists(&rt, "browser-banner-agent"), "the watching banner has no button");
    assert!(shown_browser(&rt.state).is_none(), "the web view is hidden under the picture");
    click(&mut rt, "browser-view-you");
    rt.frame(FRAME);
    assert!(!rt.state.browser_ui(TAB).unwrap().agent && rt.state.agent_view.target.is_none(), "back to your view stops watching");
    assert!(!exists(&rt, "agent-tab-view") && exists(&rt, "browser-banner-agent"));
    click(&mut rt, "browser-view-agent");
    rt.frame(FRAME);
    assert!(rt.state.browser_ui(TAB).unwrap().agent);
}

#[test]
fn goes_back_to_your_view_when_the_agents_tab_goes() {
    let mut rt = shown(AgentCase::Watching);
    assert!(rt.state.browser_ui(TAB).unwrap().agent && exists(&rt, "agent-tab-view"));
    close_agent_tab(&mut rt);
    rt.update(|s, tx| {
        s.keep_browser(tx);
    });
    rt.frame(FRAME);
    assert!(!rt.state.browser_ui(TAB).unwrap().agent, "back to Your view");
    assert!(rt.state.agent_view.target.is_none(), "nothing is watched");
    assert!(!exists(&rt, "browser-views") && !exists(&rt, "agent-tab-view"));
}

#[test]
fn the_toggle_says_you_and_agent_in_a_narrow_tab() {
    use crate::view::browser_agent_bar::short;
    assert!(short(Some(520.0)) && short(Some(300.0)) && !short(Some(520.5)) && !short(None));
    // The words drawn, not the buttons' names (those stay "Your view" and "Agent's view").
    let drawn = |rt: &Runtime<crate::State>| rt.elements().into_iter().filter_map(|e| e.text).collect::<Vec<_>>();
    let narrow = shown(AgentCase::Narrow);
    let words = drawn(&narrow);
    assert!(words.iter().any(|w| w == "You") && words.iter().any(|w| w == "Agent"), "{words:?}");
    assert!(!words.iter().any(|w| w == "Your view"), "{words:?}");
    assert!(narrow.elements().iter().any(|e| e.label.as_deref() == Some("Agent's view")), "the button keeps its name");
}
