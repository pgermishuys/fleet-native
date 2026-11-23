//! Component 26, piece O (part 2): the composer's loose ends (rows 2.33-2.36, 2.40-2.43 of docs/parity/26-parity-audit.md).
use crate::tests::{FRAME, click};
use crate::{State, spec};
use rune::{App, Env, Runtime};

fn rect(rt: &Runtime<State>, key: &str) -> [f32; 4] {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting).unwrap_or_else(|| panic!("no element {key:?}")).rect
}

fn sized(w: f32, h: f32) -> Runtime<State> {
    let mut rt = App::new(spec()).headless_runtime((w, h), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.frame(FRAME);
    rt
}

fn scene(rt: &mut Runtime<State>, name: &str) {
    let (_, f) = spec().scenes.iter().find(|(n, _)| *n == name).unwrap_or_else(|| panic!("no scene {name}"));
    rt.update(|s, _| f(s));
    for _ in 0..4 {
        rt.frame(FRAME);
    }
}

// 2.33 (`.paste-offer` is a block-level box in `.composer`, 05 11.8): the pill spans the composer's content, 14 px past the
// 760 px frame on each side in a wide window.
#[test]
fn the_paste_offer_pill_reaches_14_px_past_the_frame_on_each_side() {
    let mut rt = sized(1280.0, 760.0);
    scene(&mut rt, "composer-paste-offer");
    let (pill, frame) = (rect(&rt, "paste-offer"), rect(&rt, "composer-frame"));
    assert_eq!(frame[0] - pill[0], 14.0, "left: pill {pill:?} frame {frame:?}");
    assert_eq!((pill[0] + pill[2]) - (frame[0] + frame[2]), 14.0, "right: pill {pill:?} frame {frame:?}");
}

// 2.34 (`SelectorDropdown.place()`): the menu keeps inside the pane the composer sits in, with 8 px to spare. Rune's own anchoring
// does it, by flipping the menu to the chip's right edge; Fleet slides it by the overhang instead, so the two stop at different
// places (listed in the checklist).
#[test]
fn the_agent_menu_stays_inside_a_narrow_conversation_pane() {
    let mut rt = sized(720.0, 700.0);
    click(&mut rt, "agent");
    for _ in 0..4 {
        rt.frame(FRAME);
    }
    let (menu, pane) = (rect(&rt, "agent-pop-menu"), rect(&rt, "composer-section"));
    assert!(menu[0] + menu[2] <= pane[0] + pane[2] - 8.0 + 0.5, "menu {menu:?} sticks out of the pane {pane:?}");
}

// 2.35 (`canSend` in `use-send-prompt.ts`): with no `capabilities` on the row, only a "running" lifecycle takes a prompt;
// `canPrompt` decides when the row has it.
#[test]
fn a_row_with_no_capabilities_takes_a_prompt_only_while_its_lifecycle_is_running() {
    let mut rt = sized(1280.0, 760.0);
    let row = |lifecycle: &str| {
        crate::fleet::rows::Row::from_list_item(&serde_json::json!({ "sessionStatus": "idle", "lifecycleStatus": lifecycle }))
    };
    for (word, disabled) in [("running", false), ("stopped", true), ("completed", true), ("error", true), ("disconnected", true)] {
        rt.update(|s, _| s.projects[0].sessions[0].row = row(word));
        assert_eq!(rt.state.composer_disabled(), disabled, "lifecycleStatus {word}");
    }
    // The row's own `canPrompt` wins over the lifecycle.
    rt.update(|s, _| {
        s.projects[0].sessions[0].row = row("stopped");
        s.projects[0].sessions[0].row.can_prompt = Some(true);
    });
    assert!(!rt.state.composer_disabled());
}

// 2.40, A2.6 (`startSessionInFolderOf`): the session the palette's "New session in this folder" made is in the list and open at
// once, not when a refetch returns.
#[test]
fn a_session_made_in_the_folder_shows_in_the_list_and_opens_at_once() {
    let mut rt = sized(1280.0, 760.0);
    rt.update(|s, _| s.set_session_id((0, 0), "ses_1"));
    let answer = serde_json::json!({ "instanceId": "i", "workspaceId": "w", "harnessType": "opencode",
        "session": { "id": "ses_new", "title": "", "tags": [] } });
    rt.update(|s, tx| s.made_done("ses_1", crate::fleet::settings::HOME_MACHINE_KEY, crate::fleet::fork::Made::InFolder, answer, tx));
    rt.frame(FRAME);
    let (p, i) = rt.state.open_at();
    assert_eq!(rt.state.projects[p].sessions[i].id, "ses_new");
}

// 2.40, A3.10 (`SessionsPanel`'s watch on `draftGroupId`): a folded group opens when a draft lands in it, and stays as the
// person leaves it while the draft stays.
#[test]
fn a_folded_group_unfolds_when_a_draft_lands_in_it() {
    let mut rt = sized(1280.0, 760.0);
    rt.update(|s, _| s.projects[0].open = false);
    rt.frame(FRAME);
    click(&mut rt, "new-session");
    let project = rt.state.projects[0].id.clone();
    rt.update(move |s, _| s.new_draft.draft_mut().expect("the page has a draft").project_id = Some(project));
    for _ in 0..4 {
        rt.frame(FRAME);
    }
    let landed = rt.state.draft_group().expect("the draft has a group");
    assert!(rt.state.projects[landed].open, "the group opens when the draft lands in it");
    rt.update(|s, _| s.toggle_project(landed));
    rt.frame(FRAME);
    assert!(!rt.state.projects[landed].open, "folding it again is the person's, while the draft stays in it");
}

// 2.41 (`weave:saved:harnesses`, `weave:saved:harness-catalogs`): after a restart the harness list and the catalogs of each
// machine are on screen at once, and asked for again behind them; each machine keeps its newest twelve catalogs.
#[test]
fn the_harness_list_and_catalogs_saved_on_the_last_visit_show_at_once_after_a_restart() {
    use crate::fleet::new_session::feed_state::CatalogKey;
    use crate::fleet::new_session::harness_choice::Load;
    use crate::fleet::settings::{HOME_MACHINE_KEY as HOME, SavedCatalog, Sidebar};
    use serde_json::json;
    let list = json!([{ "type": "opencode", "displayName": "OpenCode", "available": true, "userEnabled": true }]);
    let catalog = json!({ "supported": true, "agents": [{ "name": "build", "mode": "primary", "hidden": false }], "providers": [],
        "defaultAgent": "build" });
    let mut before = Sidebar::default();
    before.save_harnesses(HOME, &list);
    before.save_harnesses("falcon", &json!([]));
    for n in 0..14 {
        let saved = SavedCatalog {
            harness: "opencode".into(),
            directory: Some(format!("/src/app{n}")),
            profile: None,
            fetched_at: n,
            catalog: catalog.clone(),
        };
        before.save_catalog(HOME, saved);
    }
    let kept = before.to_json();
    let mut rt = sized(1280.0, 760.0);
    rt.update(|s, _| {
        s.feeds.harnesses.clear();
        s.feeds.catalogs.clear();
        s.sidebar = Sidebar::parse(&kept);
        s.seed_feeds_from_settings();
    });
    let feed = &rt.state.feeds.harnesses[HOME];
    assert_eq!((feed.harnesses.all.len(), feed.harnesses.all[0].kind.as_str(), feed.harnesses.load), (1, "opencode", Load::Done));
    assert!(feed.wants_read(0), "it is checked again behind the saved list");
    assert!(rt.state.feeds.harnesses.get("falcon").is_none_or(|f| f.harnesses.all.is_empty()), "an empty list seeds nothing");
    let at = |n: i64| CatalogKey { harness: "opencode".into(), directory: Some(format!("/src/app{n}")), profile: None };
    let feed = &rt.state.feeds.catalogs[HOME];
    assert!(feed.is_cached(&at(13)) && feed.is_cached(&at(2)), "the newest twelve are kept");
    assert!(!feed.is_cached(&at(1)) && !feed.is_cached(&at(0)), "the oldest go");
}

// 2.43, B5.8 and B5.9 (`liveReachable`): the live machine's row says "Working here" while it answers, and "Unreachable" once
// it doesn't. Home always answers.
#[test]
fn the_live_machine_row_says_working_here_while_it_answers_and_unreachable_when_it_does_not() {
    use crate::mock::machines::Case;
    let mut rt = sized(1280.0, 760.0);
    rt.update(|s, _| s.show_machines(Case::Down, true));
    let live = |rt: &Runtime<State>| {
        let key = rt.state.machines.current_key().to_string();
        rt.state.machine_rows().into_iter().find(|r| r.key == key).expect("the live machine has a row")
    };
    let home = live(&rt);
    assert!(home.reachable && home.detail.starts_with("Working here"), "home: {home:?}");
    rt.update(|s, _| s.machines.set_current("m-harbor"));
    let harbor = live(&rt);
    assert!(!harbor.reachable && harbor.detail == "Unreachable", "the machine the scene says is down: {harbor:?}");
    rt.update(|s, _| s.machines.others.get_mut("m-harbor").unwrap().error = None);
    let answering = live(&rt);
    assert!(answering.reachable && answering.detail.starts_with("Working here"), "answering again: {answering:?}");
}

// 2.42, B2.136 (`roots.value !== null`): a machine that answers with no locations at all counts as having been read.
#[test]
fn an_answer_with_no_locations_counts_as_the_locations_being_known() {
    let mut rt = sized(1280.0, 760.0);
    rt.update(|s, _| {
        s.feeds.roots.clear();
        s.feeds.roots_error = None;
        s.feeds.roots_read = false;
    });
    assert!(!rt.state.roots_known());
    rt.update(|s, _| s.feeds.roots_read = true);
    assert!(rt.state.roots_known());
}

// 2.42, B2.84 and B2.121: an option row with a title and a detail is 48 high (`.ns-option` inherits line-height 1.5), and a
// list is `min(340px, 50vh)` / `min(240px, 36vh)` high.
#[test]
fn a_two_line_option_row_is_48_high_and_the_lists_follow_the_window_height() {
    use crate::fleet::new_session::folder_menu::Scene;
    let mut tall = sized(1280.0, 900.0);
    tall.update(|s, _| s.show_folder_menu(Scene::Open));
    for _ in 0..4 {
        tall.frame(FRAME);
    }
    let rows: Vec<f32> = tall
        .elements()
        .into_iter()
        .filter(|e| e.key.as_deref().is_some_and(|k| k.starts_with("new-session-folder-row-")))
        .map(|e| e.rect[3])
        .collect();
    assert!(rows.iter().any(|h| (48.0..=49.5).contains(h)), "row heights {rows:?}");
    assert_eq!(crate::ui::chip::list_max_h(&tall.state, 340.0, 0.5), 340.0);
    let mut short = sized(720.0, 480.0);
    short.frame(FRAME);
    assert_eq!(crate::ui::chip::list_max_h(&short.state, 340.0, 0.5), 240.0);
    assert!((crate::ui::chip::list_max_h(&short.state, 240.0, 0.36) - 172.8).abs() < 0.01);
}
