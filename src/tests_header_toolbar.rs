//! The header's toolbar and its ⋯ menu (component 8, piece D): `SessionActionToolbar.test.ts` under its names (the phone's two
//! left out), `use-session-detail-actions.test.ts`, and the route's abort, fork, delete, archive and restore from reading
//! (Fleet has no test for the route). The requests are read from the fake server `tests_header_edit` runs.
use crate::fleet::rows::Capabilities;
use crate::fleet::session_commands::{iso_utc, raw_export, raw_file_name};
use crate::mock::header::Case;
use crate::tests::{FRAME, click};
use crate::tests_header_edit::{Live, has, scene};
use crate::ui::menu::Entry;
use crate::view::conversation::Role;
use rune::{FileDialogKind, Runtime};
use serde_json::{Value, json};

const MORE: &str = "more-pop-item";

fn caps(rt: &mut Runtime<crate::State>, c: Capabilities) {
    rt.update(move |s, _| s.projects[0].sessions[0].row.capabilities = c.clone());
    rt.frame(FRAME);
}

fn open_menu(rt: &mut Runtime<crate::State>) {
    click(rt, "more");
    assert!(has(rt, "more-pop-menu"), "the menu opened");
}

/// The labels of the menu's rows, in order (the rule is not one).
fn menu(rt: &Runtime<crate::State>) -> Vec<String> {
    crate::ui::menus::more(&rt.state)
        .into_iter()
        .filter_map(|e| match e {
            Entry::Item(i) => Some(i.label),
            _ => None,
        })
        .collect()
}

/// The key of the row named `label` (the rule takes a place in the numbering).
fn row_key(rt: &Runtime<crate::State>, label: &str) -> String {
    let at = crate::ui::menus::more(&rt.state)
        .iter()
        .position(|e| matches!(e, Entry::Item(i) if i.label == label))
        .unwrap_or_else(|| panic!("no {label:?} row"));
    format!("{MORE}{at}")
}

const DIRECTORY: &str = "/srv/work/orchard-worktrees/fleet-session-4c2d1e90";

// ---- SessionActionToolbar.test.ts ------------------------------------------------------------------------------------------

#[test]
fn shows_actions_enabled_by_session_capabilities() {
    let mut rt = scene(Case::Retry);
    caps(
        &mut rt,
        Capabilities { can_abort: Some(true), can_archive: Some(true), can_fork: Some(true), can_delete: Some(true), ..Default::default() },
    );
    assert!(has(&rt, "abort") && has(&rt, "fork") && has(&rt, "more"));
    open_menu(&mut rt);
    assert_eq!(menu(&rt), ["Copy folder path", "Rename", "Export as Markdown…", "Archive", "Delete…"]);
}

#[test]
fn hides_actions_disabled_by_session_capabilities() {
    let mut rt = scene(Case::Retry);
    caps(
        &mut rt,
        Capabilities {
            can_abort: Some(false),
            can_archive: Some(false),
            can_fork: Some(false),
            can_delete: Some(false),
            ..Default::default()
        },
    );
    assert!(!has(&rt, "abort"), "no Abort");
    open_menu(&mut rt);
    assert_eq!(menu(&rt), ["Copy folder path", "Rename", "Export as Markdown…"], "no Archive, no Delete");
}

#[test]
fn shows_fork_off_with_its_reason_when_the_harness_cant_fork() {
    let mut rt = scene(Case::Fork);
    caps(
        &mut rt,
        Capabilities {
            can_fork: Some(false),
            fork_disabled_reason: Some("Copilot can't copy a conversation.".into()),
            ..Default::default()
        },
    );
    assert!(has(&rt, "fork"), "still there, off");
    assert_eq!(rt.state.header_model().actions.fork_label(), "Fork: Copilot can't copy a conversation.");
    assert_eq!(rt.state.header_model().actions.fork_tip(), "Copilot can't copy a conversation.");
    click(&mut rt, "fork");
    assert!(rt.state.head.bar.fork_for.is_none(), "an off Fork opens nothing");
}

#[test]
fn says_what_fork_does_when_it_is_on() {
    let rt = scene(Case::Fork);
    assert_eq!(rt.state.header_model().actions.fork_label(), "Fork");
    assert_eq!(rt.state.header_model().actions.fork_tip(), "Fork: a new session with a copy of this conversation");
}

#[test]
fn keeps_archive_and_delete_inside_the_more_menu() {
    let mut rt = scene(Case::Fork);
    assert!(!has(&rt, "toolbar-archive") && !has(&rt, "toolbar-delete"), "not in the bar");
    open_menu(&mut rt);
    let rows = menu(&rt);
    assert!(rows.contains(&"Archive".into()) && rows.contains(&"Delete…".into()));
}

#[test]
fn offers_restore_for_an_archived_session() {
    let mut rt = scene(Case::Archived);
    assert!(!has(&rt, "fork"), "an archived session has no Fork");
    open_menu(&mut rt);
    let rows = menu(&rt);
    assert!(rows.contains(&"Restore".into()), "{rows:?}");
    assert!(!rows.contains(&"Archive".into()) && !rows.contains(&"Rename".into()), "{rows:?}");
}

#[test]
fn copies_the_folder_path_from_the_first_row_of_the_more_menu() {
    let clipboard = rune::clipboard::memory();
    let mut rt = scene(Case::Fork);
    rt.use_clipboard(&clipboard);
    open_menu(&mut rt);
    assert_eq!(menu(&rt).first().map(String::as_str), Some("Copy folder path"));
    let key = row_key(&rt, "Copy folder path");
    click(&mut rt, &key);
    assert_eq!(clipboard.text().as_deref().map(String::as_str), Some(DIRECTORY));
    assert!(rt.state.ui.toast.is_none() && rt.state.head.copied_until.is_none(), "no toast, no Copied");
    assert!(!has(&rt, "more-pop-menu"), "the menu closed");
}

#[test]
fn has_no_copy_row_without_a_folder() {
    let mut rt = scene(Case::Fork);
    rt.update(|s, _| s.projects[0].sessions[0].row.directory.clear());
    rt.frame(FRAME);
    open_menu(&mut rt);
    assert!(!menu(&rt).contains(&"Copy folder path".into()));
}

#[test]
fn asks_to_export_from_the_more_menu() {
    let mut rt = scene(Case::Fork);
    open_menu(&mut rt);
    let key = row_key(&rt, "Export as Markdown…");
    click(&mut rt, &key);
    assert!(rt.state.export.dialog.is_some(), "the export dialog is open for this session");
}

// ---- use-session-detail-actions.test.ts ------------------------------------------------------------------------------------

// "asks first, naming the session, with the delete run inside the confirm, then goes home"
#[test]
fn asks_first_naming_the_session_with_the_delete_run_inside_the_confirm_then_goes_home() {
    let mut live = Live::start(Case::Fork, None);
    open_menu(&mut live.rt);
    let key = row_key(&live.rt, "Delete…");
    click(&mut live.rt, &key);
    let c = live.rt.state.ui.confirm.clone().expect("it asks first");
    assert_eq!((c.title.as_str(), c.subject.as_str()), ("Delete this session?", "Try a smaller cache limit"));
    assert_eq!((c.confirm_label.as_str(), c.destructive), ("Delete session", true));
    assert_eq!(c.body, "It and its session data are deleted for good.");
    assert!(live.calls().is_empty(), "nothing is sent before the answer");
    click(&mut live.rt, "confirm-action");
    live.wait("the delete and the way home", |l| l.rt.state.row_of("a-open").is_none());
    // Fleet's "/" has no session open; the native app opens the first one (6H's decision, `deleting_the_open_session_opens_another`).
    assert_ne!(live.rt.state.open_id().as_deref(), Some("a-open"));
    assert_eq!(live.calls(), [("DELETE".to_string(), "/api/sessions/a-open".to_string(), Value::Null)]);
}

// "stays on the session when the confirm is cancelled"
#[test]
fn stays_on_the_session_when_the_confirm_is_cancelled() {
    let mut live = Live::start(Case::Fork, None);
    live.rt.update(|s, tx| s.toolbar_delete(tx));
    live.rt.frame(FRAME);
    click(&mut live.rt, "confirm-cancel");
    live.rt.frame(FRAME);
    assert!(live.calls().is_empty());
    assert!(matches!(live.rt.state.route, crate::fleet::store::Route::Session(_)) && live.rt.state.ui.confirm.is_none());
}

// ---- abort ---------------------------------------------------------------------------------------------------------------

#[test]
fn abort_is_offered_only_while_the_session_can_abort() {
    assert!(!has(&scene(Case::Fork), "abort"), "idle: no Abort");
    assert!(has(&scene(Case::Retry), "abort"), "working: Abort");
    let mut rt = scene(Case::Fork);
    caps(&mut rt, Capabilities { can_abort: Some(true), ..Default::default() });
    assert!(has(&rt, "abort"), "the capability wins over the fallback");
}

#[test]
fn abort_posts_to_the_session_and_is_off_while_it_runs() {
    let mut live = Live::start(Case::Retry, None);
    click(&mut live.rt, "abort");
    assert!(live.rt.state.toolbar_pending(), "pending until the answer");
    live.wait("the abort", |l| !l.calls().is_empty() && l.rt.state.head.bar.aborting.is_none());
    assert_eq!(live.calls(), [("POST".to_string(), "/api/sessions/a-open/abort".to_string(), json!({}))]);
    assert!(live.rt.state.head.bar.errors("a-open").is_empty());
}

#[test]
fn a_refused_abort_is_a_red_line_under_the_toolbar() {
    let mut live = Live::start(Case::Retry, Some((409, r#"{"error":"Nothing to abort."}"#)));
    click(&mut live.rt, "abort");
    live.wait("the refusal", |l| l.rt.state.head.bar.abort_error.is_some());
    live.rt.frame(FRAME);
    assert_eq!(live.rt.state.head.bar.errors("a-open"), ["Nothing to abort."]);
    assert!(has(&live.rt, "toolbar-error-0"));
}

#[test]
fn abort_does_nothing_while_another_action_is_pending() {
    let mut live = Live::start(Case::Retry, None);
    live.rt.update(|s, _| s.head.bar.restoring = Some("a-open".into()));
    live.rt.update(|s, tx| s.toolbar_abort(tx));
    assert!(live.calls().is_empty() && live.rt.state.head.bar.aborting.is_none());
}

// ---- fork ----------------------------------------------------------------------------------------------------------------

#[test]
fn fork_opens_the_fork_dialog_when_allowed() {
    let mut rt = scene(Case::Fork);
    click(&mut rt, "fork");
    assert_eq!(rt.state.head.bar.fork_for.as_deref(), Some("a-open"));
}

#[test]
fn fork_is_not_offered_when_the_harness_cannot_fork() {
    let mut rt = scene(Case::Fork);
    caps(&mut rt, Capabilities { can_fork: Some(false), ..Default::default() });
    rt.update(|s, tx| s.toolbar_fork(tx));
    assert!(rt.state.head.bar.fork_for.is_none());
    assert_eq!(rt.state.header_model().actions.fork_label(), "Fork: This session can't be forked.");
}

// ---- delete ---------------------------------------------------------------------------------------------------------------

#[test]
fn delete_is_not_offered_when_the_session_cannot_be_deleted() {
    let mut rt = scene(Case::Fork);
    caps(&mut rt, Capabilities { can_delete: Some(false), ..Default::default() });
    open_menu(&mut rt);
    assert!(!menu(&rt).contains(&"Delete…".into()));
    rt.update(|s, tx| s.toolbar_delete(tx));
    assert!(rt.state.ui.confirm.is_none(), "and the handler checks too");
}

#[test]
fn a_refused_delete_keeps_the_dialog_open_and_the_words_under_the_toolbar() {
    let mut live = Live::start(Case::Fork, Some((409, r#"{"error":"It is still running."}"#)));
    live.rt.update(|s, tx| s.toolbar_delete(tx));
    live.rt.frame(FRAME);
    click(&mut live.rt, "confirm-action");
    live.wait("the refusal", |l| l.rt.state.head.bar.delete_error.is_some());
    live.rt.frame(FRAME);
    assert!(has(&live.rt, "confirm-error"), "the dialog stays, with its reason");
    assert_eq!(live.rt.state.head.bar.errors("a-open"), ["It is still running."]);
    assert!(matches!(live.rt.state.route, crate::fleet::store::Route::Session(_)), "still on the session");
}

#[test]
fn delete_names_the_session_and_the_menu_spins_while_it_runs() {
    let mut rt = scene(Case::Fork);
    rt.update(|s, tx| s.toolbar_delete(tx));
    rt.update(|s, _| s.ui.confirm.as_mut().unwrap().busy = true);
    assert!(rt.state.toolbar_busy() && rt.state.toolbar_pending());
}

// ---- rename ---------------------------------------------------------------------------------------------------------------

#[test]
fn a_refused_header_rename_is_a_red_line_and_the_old_title_comes_back() {
    let mut live = Live::start(Case::Fork, Some((409, r#"{"error":"Title too long."}"#)));
    live.rt.update(|s, tx| s.start_header_rename(tx));
    live.rt.update(|s, _| s.head.title_edit.replace_text("Unify status".to_string()));
    live.rt.update(|s, tx| s.finish_header_rename(true, tx));
    assert!(live.rt.state.toolbar_pending(), "the rename is pending");
    live.wait("the refusal", |l| l.rt.state.head.bar.rename_error.is_some());
    assert_eq!(live.rt.state.head.bar.errors("a-open"), ["Title too long."]);
    assert_eq!(live.rt.state.projects[0].sessions[0].title, "Try a smaller cache limit");
    assert!(!live.rt.state.toolbar_pending());
}

#[test]
fn rename_in_the_menu_starts_the_title_edit() {
    let mut rt = scene(Case::Fork);
    open_menu(&mut rt);
    let key = row_key(&rt, "Rename");
    click(&mut rt, &key);
    assert!(rt.state.head.editing_title("a-open"));
}

// ---- archive and restore --------------------------------------------------------------------------------------------------

#[test]
fn archive_sends_the_retention_and_the_session_is_archived() {
    let mut live = Live::start(Case::Fork, None);
    live.rt.update(|s, tx| s.toolbar_archive(tx));
    assert!(live.rt.state.toolbar_busy(), "the ⋯ spins while it is sent");
    live.wait("the archive", |l| l.rt.state.projects[0].sessions[0].row.archived);
    assert_eq!(
        live.calls(),
        [("PATCH".to_string(), "/api/sessions/a-open/retention".to_string(), json!({ "retentionStatus": "archived" }))]
    );
    assert!(!live.rt.state.toolbar_busy());
}

#[test]
fn a_refused_archive_says_so_in_a_toast_and_the_session_stays() {
    let mut live = Live::start(Case::Fork, Some((500, "{}")));
    live.rt.update(|s, tx| s.toolbar_archive(tx));
    live.wait("the refusal", |l| l.rt.state.ui.toast.is_some());
    assert_eq!(live.rt.state.ui.toast.as_ref().unwrap().message, "Couldn't archive \"Try a smaller cache limit\".");
    assert!(!live.rt.state.projects[0].sessions[0].row.archived);
}

#[test]
fn restore_sends_the_retention_and_the_session_is_active_again() {
    let mut live = Live::start(Case::Archived, None);
    open_menu(&mut live.rt);
    let key = row_key(&live.rt, "Restore");
    click(&mut live.rt, &key);
    live.wait("the restore", |l| !l.rt.state.projects[0].sessions[0].row.archived);
    assert_eq!(live.calls(), [("PATCH".to_string(), "/api/sessions/a-open/retention".to_string(), json!({ "retentionStatus": "active" }))]);
}

#[test]
fn a_refused_restore_is_a_toast_and_the_session_stays_archived() {
    let mut live = Live::start(Case::Archived, Some((409, r#"{"error":"Its folder is gone."}"#)));
    live.rt.update(|s, tx| s.toolbar_restore(tx));
    live.wait("the refusal", |l| l.rt.state.ui.toast.is_some());
    assert_eq!(live.rt.state.ui.toast.as_ref().unwrap().message, "Its folder is gone.");
    assert!(live.rt.state.projects[0].sessions[0].row.archived);
}

// ---- the palette commands -------------------------------------------------------------------------------------------------

// ActivityStream.commands.test.ts: "copy-session-id puts this session's id on the clipboard"
#[test]
fn copy_session_id_puts_this_sessions_id_on_the_clipboard() {
    let clipboard = rune::clipboard::memory();
    let mut rt = scene(Case::Fork);
    rt.use_clipboard(&clipboard);
    rt.update(|s, tx| s.copy_session_id(tx));
    rt.frame(FRAME);
    assert_eq!(clipboard.text().as_deref().map(String::as_str), Some("a-open"));
}

// ActivityStream.commands.test.ts: "export-raw-messages downloads the messages as JSON named after the session"
#[test]
fn export_raw_messages_saves_the_messages_as_json_named_after_the_session() {
    let mut rt = scene(Case::Fork);
    let mut m = crate::fleet::model::Msg::new("m1", "a-open", Role::User);
    m.parts = vec![crate::fleet::model::PartData::Text { id: "p1".into(), text: "Hello".into() }];
    rt.update(move |s, _| {
        let mut stream = crate::fleet::tests::new_state();
        stream.messages = vec![m.clone()];
        s.stream = Some(stream);
    });
    rt.update(|s, tx| s.export_raw_messages(tx));
    let (kind, options) = rt.file_dialog().expect("the system's save dialog");
    assert_eq!(kind, FileDialogKind::Save);
    assert_eq!(options.file_name.as_deref(), Some("Try-a-smaller-cache-limit-conversation.json"));
    let folder = std::env::temp_dir().join(format!("fleet-native-raw-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    let path = folder.join("raw.json");
    rt.resolve_file_dialog(Some(path.clone())).unwrap();
    crate::test_wait::until(
        &mut rt,
        "the raw messages file and its toast",
        |rt| {
            rt.frame(FRAME);
            path.exists() && rt.state.ui.toast.is_some()
        },
        |rt| format!("file {}, toast {:?}", path.exists(), rt.state.ui.toast.is_some()),
    );
    let payload: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!((payload["sessionId"].as_str(), payload["title"].as_str()), (Some("a-open"), Some("Try a smaller cache limit")));
    assert_eq!(payload["messages"][0]["messageId"], "m1");
    assert!(payload["exportedAt"].as_str().is_some_and(|t| t.ends_with('Z')));
    std::fs::remove_dir_all(&folder).unwrap();
}

#[test]
fn the_raw_file_is_named_and_written_as_fleet_writes_it() {
    assert_eq!(raw_file_name("Fix: the login bug!", "s1"), "Fix-the-login-bug-conversation.json");
    assert_eq!(raw_file_name("  ", "s1"), "s1-conversation.json");
    assert_eq!(raw_file_name("???", "s1"), "s1-conversation.json");
    assert_eq!(iso_utc(0), "1970-01-01T00:00:00.000Z");
    assert_eq!(iso_utc(1_700_000_000_123), "2023-11-14T22:13:20.123Z");
    let text = raw_export("s1", "T", "2024-01-01T00:00:00.000Z", &[]);
    assert_eq!(
        text,
        "{\n  \"sessionId\": \"s1\",\n  \"title\": \"T\",\n  \"exportedAt\": \"2024-01-01T00:00:00.000Z\",\n  \"messages\": []\n}"
    );
}

#[test]
fn the_archived_banners_restore_button_restores() {
    let mut live = Live::start(Case::Archived, None);
    click(&mut live.rt, "session-restore-button");
    live.wait("the restore", |l| !l.rt.state.projects[0].sessions[0].row.archived);
    assert_eq!(live.calls().len(), 1);
}
