//! Lineage and sub-agent rows (piece M of docs/parity/06-session-list.md): `lineage-moves.test.ts` ("lineage moves"),
//! `SessionItem.test.ts` ("lineage", "moving it out of its parent" is in `tests_row_menu.rs`), `SessionItem.lineage-muted.test.ts`,
//! and the sheet's own numbers for the child rows, the lines and the sub-agent row (3.120-3.131, 4.1-4.6). The pure functions are
//! in `src/fleet/tests/lineage.rs`. The invented list needs no server; the move's requests use a fake one.
use crate::fleet::archive_queue::UNDO_SECS;
use crate::fleet::work::to_item;
use crate::tests::{FRAME, app, click};
use crate::tests_archive_pins::{Fake, Rig};
use crate::theme::Theme;
use rune::{Key, KeyEvent, Mods, PhysicalInput, Runtime};
use serde_json::json;

fn id(rt: &Runtime<crate::State>, p: usize, i: usize) -> String {
    rt.state.projects[p].sessions[i].id.clone()
}

fn has(rt: &Runtime<crate::State>, key: &str) -> bool {
    rt.find(key).is_some()
}

fn rect(rt: &Runtime<crate::State>, key: &str) -> [f32; 4] {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting).unwrap_or_else(|| panic!("no element {key:?}")).rect
}

fn fork_of(rt: &mut Runtime<crate::State>, child: usize, parent: usize) {
    let parent = id(rt, 0, parent);
    rt.update(move |s, _| {
        s.projects[0].sessions[child].listing.forked_from = Some(parent);
        s.projects[0].sessions[child].listing.spawn_kind = Some("fork".into());
    });
    rt.frame(FRAME);
}

/// The first row's session has the second as a fork and the third as one it started.
fn family() -> Runtime<crate::State> {
    let mut rt = app();
    fork_of(&mut rt, 1, 0);
    let parent = id(&rt, 0, 0);
    rt.update(move |s, _| s.projects[0].sessions[2].listing.spawned_by = Some(parent));
    rt.frame(FRAME);
    rt
}

fn press(rt: &mut Runtime<crate::State>, key: Key) {
    rt.key(KeyEvent { key, mods: Mods::default() });
    rt.frame(FRAME);
    rt.frame(FRAME);
}

fn text_of(rt: &Runtime<crate::State>, key: &str) -> Option<String> {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting).and_then(|e| e.text)
}

// ---- lineage-moves.test.ts, "lineage moves" -------------------------------------------------------------------------------

fn moves_rig(answer: impl Fn(&str, &str) -> (u16, String) + Send + Sync + 'static) -> (Rig, Fake, String, String) {
    let fake = Fake::start(answer);
    let mut r = Rig::home(&fake);
    fork_of(&mut r.rt, 1, 0);
    let (parent, child) = (id(&r.rt, 0, 0), id(&r.rt, 0, 1));
    (r, fake, parent, child)
}

fn detached(r: &Rig) -> bool {
    r.state().projects[0].sessions[1].listing.detached
}

#[test]
fn lineage_moves_moves_a_session_out_at_once_tells_the_server_and_offers_undo_for_a_while() {
    let (mut r, fake, _, child) = moves_rig(|_, _| (200, "{}".into()));
    let (title, parent) = (r.state().projects[0].sessions[1].title.clone(), r.state().projects[0].sessions[0].title.clone());
    let c = child.clone();
    r.rt.update(move |s, tx| s.move_lineage(&c, true, tx));
    assert!(detached(&r), "the list takes it at once");
    r.wait("the lineage request", |_| !fake.calls("/lineage").is_empty());
    let call = &fake.calls("/lineage")[0];
    assert_eq!(
        (call.0.as_str(), call.1.clone(), call.2.clone()),
        ("PATCH", format!("/api/sessions/{child}/lineage"), json!({ "detached": true }))
    );
    r.wait("Undo to be offered", |s| s.lineage_moves.pending.is_some());
    assert_eq!(r.state().lineage_moves.pending.as_ref().map(|p| p.message.clone()), Some(format!("Moved \"{title}\" out of \"{parent}\"")));
    r.rt.frame(FRAME);
    assert!(has(&r.rt, "toast-action"), "the one toast offers Undo");
    r.rt.frame(UNDO_SECS + 0.1);
    assert!(r.state().lineage_moves.pending.is_none(), "the offer ends after six seconds");
}

#[test]
fn lineage_moves_puts_it_back_under_its_parent_on_undo() {
    let (mut r, fake, _, child) = moves_rig(|_, _| (200, "{}".into()));
    let c = child.clone();
    r.rt.update(move |s, tx| s.move_lineage(&c, true, tx));
    r.wait("Undo to be offered", |s| s.lineage_moves.pending.is_some());
    r.rt.frame(FRAME);
    click(&mut r.rt, "toast-action");
    r.wait("both requests", |_| fake.calls("/lineage").len() == 2);
    let bodies: Vec<_> = fake.calls("/lineage").iter().map(|c| c.2.clone()).collect();
    assert_eq!(bodies, [json!({ "detached": true }), json!({ "detached": false })]);
    assert!(!detached(&r));
    assert!(r.state().lineage_moves.pending.is_none(), "Undo offers nothing further");
}

#[test]
fn lineage_moves_moves_a_session_back_under_its_parent_and_undo_moves_it_out_again() {
    let (mut r, fake, _, child) = moves_rig(|_, _| (200, "{}".into()));
    r.rt.update(|s, _| s.projects[0].sessions[1].listing.detached = true);
    let (title, parent) = (r.state().projects[0].sessions[1].title.clone(), r.state().projects[0].sessions[0].title.clone());
    let c = child.clone();
    r.rt.update(move |s, tx| s.move_lineage(&c, false, tx));
    assert!(!detached(&r));
    r.wait("Undo to be offered", |s| s.lineage_moves.pending.is_some());
    assert_eq!(
        r.state().lineage_moves.pending.as_ref().map(|p| p.message.clone()),
        Some(format!("Moved \"{title}\" back under \"{parent}\""))
    );
    r.rt.frame(FRAME);
    click(&mut r.rt, "toast-action");
    r.wait("both requests", |_| fake.calls("/lineage").len() == 2);
    let bodies: Vec<_> = fake.calls("/lineage").iter().map(|c| c.2.clone()).collect();
    assert_eq!(bodies, [json!({ "detached": false }), json!({ "detached": true })]);
    assert!(detached(&r));
}

#[test]
fn lineage_moves_puts_the_list_back_and_says_why_when_the_server_refuses() {
    let why = "A subagent's session belongs to its parent's turn; it can't be moved out.";
    let (mut r, _fake, _, child) = moves_rig(move |_, _| (409, format!(r#"{{"error":"{why}"}}"#)));
    let c = child.clone();
    r.rt.update(move |s, tx| s.move_lineage(&c, true, tx));
    assert!(detached(&r), "at once");
    r.wait("the refusal", |s| s.lineage_moves.error.is_some());
    assert!(!detached(&r), "put back");
    assert!(r.state().lineage_moves.pending.is_none());
    assert_eq!(r.state().lineage_moves.error.as_deref(), Some(why));
    r.rt.frame(FRAME);
    assert!(has(&r.rt, "toast"), "the toast says it");
    click(&mut r.rt, "toast-action");
    assert!(r.state().lineage_moves.error.is_none(), "the X clears it");
}

#[test]
fn a_move_with_no_words_from_the_server_says_couldnt_move_the_session() {
    let (mut r, _fake, _, child) = moves_rig(|_, _| (500, String::new()));
    r.rt.update(move |s, tx| s.move_lineage(&child, true, tx));
    r.wait("the refusal", |s| s.lineage_moves.error.is_some());
    assert_eq!(r.state().lineage_moves.error.as_deref(), Some("Couldn't move the session."));
}

// ---- SessionItem.test.ts, "lineage" ---------------------------------------------------------------------------------------

#[test]
fn lineage_shows_a_green_chip_counting_the_work_its_agent_has_running() {
    let mut rt = family();
    rt.update(|s, _| s.projects[0].sessions[1].row.running_work = 2);
    rt.frame(FRAME);
    assert!(has(&rt, "session-running-chip-0-1"), "on the child row as on any other");
}

#[test]
fn lineage_under_its_parent_says_how_it_came_from_it_instead_of_its_status() {
    let rt = family();
    assert_eq!(text_of(&rt, "session-kind-0-1").as_deref(), Some("FORK"));
    assert_eq!(text_of(&rt, "session-kind-0-2").as_deref(), Some("STARTED"));
    assert!(!has(&rt, "session-status-0-1") && !has(&rt, "session-status-0-2"), "the word gives way");
    assert!(!has(&rt, "session-kind-0-0"), "a top-level row has no label");
}

#[test]
fn lineage_a_parent_opens_and_closes_its_children_with_the_caret_or_the_arrow_keys_without_opening_the_session() {
    let mut rt = family();
    assert_eq!(rt.state.open_at(), (0, 0), "the parent is the open session, so it shows its children");
    rt.update(|s, tx| {
        let id = s.projects[0].sessions[3].id.clone();
        s.set_family_open(&id, true);
        tx.focus("session-0-0", true);
    });
    click(&mut rt, "session-caret-0-0");
    assert!(!has(&rt, "session-0-1") && !has(&rt, "session-0-2"), "the caret closes the family");
    assert_eq!(rt.state.open_at(), (0, 0), "and opens no session");
    click(&mut rt, "session-caret-0-0");
    assert!(has(&rt, "session-0-1"), "and opens it again");
    rt.update(|_, tx| tx.focus("session-0-0", true));
    rt.frame(FRAME);
    press(&mut rt, Key::Left);
    assert!(!has(&rt, "session-0-1"), "Left closes");
    press(&mut rt, Key::Right);
    assert!(has(&rt, "session-0-1"), "Right opens");
    assert_eq!(rt.state.open_at(), (0, 0));
    assert_eq!(
        rt.elements().into_iter().find(|e| e.key.as_deref() == Some("session-caret-0-0")).and_then(|e| e.help).as_deref(),
        Some("Hide what it started")
    );
}

#[test]
fn lineage_keeps_the_carets_place_on_a_top_level_row_with_nothing_under_it() {
    let rt = family();
    let slot = rect(&rt, "session-caret-0-3");
    assert_eq!((slot[2], slot[3]), (14.0, 14.0));
    let (with, without) = (rect(&rt, "session-title-0-0"), rect(&rt, "session-title-0-3"));
    assert_eq!(with[0], without[0], "every title in a project lines up");
}

#[test]
fn lineage_leads_a_top_level_row_with_the_caret_so_the_status_dot_sits_beside_the_title() {
    let rt = family();
    let (caret, row) = (rect(&rt, "session-caret-0-0"), rect(&rt, "session-0-0"));
    assert_eq!(caret[0], row[0] + 6.0, "the caret is first, 6px in");
    let glyph = if has(&rt, "session-glyph-0-0") { "session-glyph-0-0" } else { "session-slot-0-0" };
    let (glyph, title) = (rect(&rt, glyph), rect(&rt, "session-title-0-0"));
    assert!(glyph[0] > caret[0] + caret[2] - 5.0 && glyph[0] < title[0], "caret, dot, title");
}

#[test]
fn a_child_row_is_28_tall_and_a_top_level_row_32_and_the_content_starts_at_25_41_and_57() {
    let mut rt = family();
    let (parent, child) = (rect(&rt, "session-0-0"), rect(&rt, "session-0-1"));
    assert_eq!((parent[3], child[3]), (32.0, 28.0));
    let glyph = |rt: &Runtime<crate::State>, n: usize| {
        let key = if has(rt, &format!("session-glyph-0-{n}")) { format!("session-glyph-0-{n}") } else { format!("session-slot-0-{n}") };
        rect(rt, &key)[0] - rect(rt, &format!("session-0-{n}"))[0]
    };
    assert_eq!(glyph(&rt, 1), 25.0, "depth 1");
    // A fork of the fork is a level deeper, and so is its own child.
    let (a, b) = (id(&rt, 0, 1), id(&rt, 0, 2));
    rt.update(move |s, _| {
        s.projects[0].sessions[2].listing.spawned_by = None;
        s.projects[0].sessions[2].listing.forked_from = Some(a);
        s.projects[0].sessions[2].listing.spawn_kind = Some("fork".into());
        s.projects[0].sessions[3].listing.forked_from = Some(b);
        s.projects[0].sessions[3].listing.spawn_kind = Some("fork".into());
    });
    rt.frame(FRAME);
    assert_eq!((glyph(&rt, 1), glyph(&rt, 2), glyph(&rt, 3)), (25.0, 41.0, 57.0));
}

#[test]
fn the_lines_hang_at_the_levels_x_and_the_last_one_stops_at_the_middle() {
    let rt = family();
    let row = rect(&rt, "session-0-1");
    let (first, last) = (rect(&rt, "lineage-line-0-1"), rect(&rt, "lineage-line-0-2"));
    assert_eq!((first[0] - row[0], first[2]), (13.0, 1.0));
    assert_eq!(first[3], 30.0, "full height: the row and its 1px above and below");
    assert_eq!(last[3], 15.0, "the last child's line stops at the middle");
    let stub = rect(&rt, "lineage-stub-0-1");
    assert_eq!((stub[2], stub[3], stub[1] - first[1]), (9.0, 1.0, 15.0));
}

// ---- SessionItem.lineage-muted.test.ts ------------------------------------------------------------------------------------

/// The colours lineage draws, read from the frame: the kind label, the line, the stub and what the caret draws.
fn muted(rt: &Runtime<crate::State>, when: &str) -> Vec<Option<[u32; 4]>> {
    let all = rt.elements();
    let bits = |c: rune::Color| [c.r.to_bits(), c.g.to_bits(), c.b.to_bits(), c.a.to_bits()];
    let by_key = |k: &str| all.iter().position(|e| e.key.as_deref() == Some(k) && !e.exiting);
    let mut out = Vec::new();
    for k in ["session-kind-0-1", "lineage-line-0-1", "lineage-stub-0-1", "session-caret-0-0"] {
        let at = by_key(k).unwrap_or_else(|| {
            let keys: Vec<_> = all.iter().filter_map(|e| e.key.clone()).filter(|k| k.starts_with("session-")).collect();
            panic!("no {k} ({when}): {keys:?}")
        });
        out.push(all[at].text_color.map(bits));
        out.push(Some(bits(all[at].fill)));
        // The chevron inside the caret.
        for e in all.iter().filter(|e| e.parent == Some(at)) {
            out.push(e.text_color.map(bits));
            out.push(Some(bits(e.fill)));
        }
    }
    out
}

#[test]
fn the_caret_is_muted_never_lights_on_hover_or_focus() {
    let mut rt = family();
    let rest = muted(&rt, "rest");
    // Hover the row, the caret, the label.
    for key in ["session-0-0", "session-caret-0-0", "session-0-1", "session-kind-0-1"] {
        let (x, y) = rt.find(key).unwrap();
        rt.pointer_move(x, y);
        rt.frame(FRAME);
        rt.frame(FRAME);
        assert_eq!(muted(&rt, key), rest, "hovering {key}");
    }
    // Focus a row; pick one; open the child (the open session's family).
    rt.update(|_, tx| tx.focus("session-0-0", true));
    rt.frame(FRAME);
    assert_eq!(muted(&rt, "focus"), rest, "focus");
    rt.physical_input(PhysicalInput::Modifiers(Mods { ctrl: true, cmd: true, ..Mods::default() }));
    click(&mut rt, "session-0-2");
    rt.physical_input(PhysicalInput::Modifiers(Mods::default()));
    rt.frame(FRAME);
    assert_eq!(muted(&rt, "picked"), rest, "picked");
    rt.update(|s, _| s.selection.clear());
    rt.frame(FRAME);
    click(&mut rt, "session-0-1");
    assert_eq!(rt.state.open_at(), (0, 1));
    assert_eq!(muted(&rt, "open"), rest, "the open session's own row, and its family");
}

#[test]
fn lineage_lines_labels_and_caret_have_the_themes_muted_colours_in_both_themes() {
    for dark in [true, false] {
        let t = Theme::of(dark);
        let mut rt = family();
        rt.update(move |s, _| s.dark = Some(dark));
        rt.frame(FRAME);
        let all = rt.elements();
        let find = |k: &str| all.iter().find(|e| e.key.as_deref() == Some(k)).unwrap();
        assert_eq!(find("session-kind-0-1").text_color, Some(t.row_meta_ink()), "the kind label is the row's meta colour, dark: {dark}");
        assert_eq!(find("lineage-line-0-1").fill, t.muted.alpha(0.16), "the line is muted at 16%, dark: {dark}");
    }
}

// ---- running sub-agents ---------------------------------------------------------------------------------------------------

fn subagent(rt: &mut Runtime<crate::State>, parent: usize, work: &str, child: &str) {
    let session = id(rt, 0, parent);
    let item = to_item(&json!({
        "id": work, "sessionId": session, "kind": "subagent", "title": "general", "label": "Migrate the cache",
        "status": "running", "childSessionId": child,
    }))
    .unwrap();
    rt.update(move |s, _| s.list_work.apply(item));
    rt.frame(FRAME);
}

#[test]
fn a_running_subagent_shows_under_the_session_that_started_it_and_opens_its_session_under_its_parent() {
    let mut rt = app();
    subagent(&mut rt, 1, "w1", "sub-session");
    assert!(has(&rt, "subagent-row-w1"));
    assert_eq!(text_of(&rt, "subagent-title-w1").as_deref(), Some("Migrate the cache"));
    assert_eq!(text_of(&rt, "subagent-kind-w1").as_deref(), Some("SUBAGENT"));
    let row = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("subagent-row-w1")).unwrap();
    assert_eq!(row.rect[3], 28.0);
    assert_eq!(row.help.as_deref(), Some("general: Migrate the cache"));
    let parent = id(&rt, 0, 1);
    assert!(has(&rt, "lineage-line-w1") || rect(&rt, "lineage-w1")[3] > 0.0, "with its lines");
    click(&mut rt, "subagent-row-w1");
    assert_eq!(rt.state.open_id().as_deref(), Some("sub-session"));
    assert_eq!(rt.state.work.opened_from.as_deref(), Some(parent.as_str()));
    // The open sub-agent's row is the lit one.
    assert!(has(&rt, "subagent-row-w1"));
}

#[test]
fn a_subagent_that_finishes_drops_off_and_one_with_no_session_never_shows() {
    let mut rt = app();
    subagent(&mut rt, 1, "w1", "sub-session");
    assert!(has(&rt, "subagent-row-w1"));
    let ended = to_item(
        &json!({ "id": "w1", "sessionId": id(&rt, 0, 1), "kind": "subagent", "status": "completed", "childSessionId": "sub-session" }),
    )
    .unwrap();
    rt.update(move |s, _| s.list_work.apply(ended));
    rt.frame(FRAME);
    assert!(!has(&rt, "subagent-row-w1"), "the Agents tab keeps it, the list doesn't");
    let loose = to_item(&json!({ "id": "w2", "sessionId": id(&rt, 0, 1), "kind": "subagent", "status": "running" })).unwrap();
    rt.update(move |s, _| s.list_work.apply(loose));
    rt.frame(FRAME);
    assert!(!has(&rt, "subagent-row-w2"));
}

#[test]
fn a_running_subagent_opens_its_parents_family_and_a_closed_one_stays_closed() {
    let mut rt = app();
    // Row 2 is not the open session; its running sub-agent opens it by itself.
    subagent(&mut rt, 2, "w1", "sub-session");
    assert!(has(&rt, "subagent-row-w1"));
    let parent = id(&rt, 0, 2);
    rt.update(move |s, _| s.set_family_open(&parent, false));
    rt.frame(FRAME);
    assert!(!has(&rt, "subagent-row-w1"), "the user's caret decides");
}
