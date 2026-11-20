//! Component 24, piece H: `menu.test.ts` under its names (adapted: the macOS list is data here, Windows and Linux have the
//! palette), and what each item does.
use super::menu::{self, Item};
use crate::State;
use crate::tests::{FRAME, app};
use crate::ui::commands;
use rune::{Runtime, Shell};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

fn palette_of(s: &State, id: &str) -> commands::Command {
    commands::find(s, id).unwrap_or_else(|| panic!("no palette entry {id}"))
}

#[test]
fn has_edit_find_in_conversation_shown_with_the_command_key_but_with_the_key_left_to_the_app() {
    let find = menu::macos()
        .into_iter()
        .find(|e| e.menu == Some("Edit") && e.title == "Find in Conversation")
        .expect("Edit → Find in Conversation");
    assert_eq!(find.item, Some(Item::FindInConversation));
    // `CommandOrControl+F`, and `registerAccelerator: false`.
    assert_eq!(find.key, Some("mod+f"));
    assert!(find.key_left_to_app);
    // It follows the standard Edit items, as Electron's list has it.
    assert_eq!(find.after, Some("selectAll:"));
}

#[test]
fn on_windows_and_linux_the_items_found_nowhere_else_are_in_the_palette() {
    let rt = app();
    for (item, id, label, _) in menu::palette() {
        let cmd = palette_of(&rt.state, id);
        assert_eq!(cmd.label, label);
        assert!(matches!(cmd.run, commands::Run::Now(_)), "{id} is live");
        assert_ne!(item, Item::FindInConversation);
    }
    let labels: Vec<_> = menu::palette().iter().map(|p| p.2).collect();
    assert_eq!(labels, ["Quit Fleet", "Check for updates", "Open logs folder", "Open Fleet data folder"]);
}

#[test]
fn the_macos_bar_has_fleets_items_and_ids_are_unique() {
    let titles: Vec<_> = menu::macos().iter().filter(|e| e.item.is_some()).map(|e| (e.menu.unwrap_or("Fleet"), e.title)).collect();
    assert_eq!(
        titles,
        [
            ("Fleet", "Check for Updates…"),
            ("Edit", "Find in Conversation"),
            ("View", "Toggle Full Screen"),
            ("Help", "Report a Problem…"),
            ("Help", "Open Logs Folder"),
            ("Help", "Open Fleet Data Folder"),
        ]
    );
    // Check for Updates is in Help on the others and in the app menu here, so never twice on a system.
    assert!(menu::macos().iter().all(|e| e.title != "Quit Fleet"));
}

fn opened_into(rt: &mut Runtime<State>) -> Arc<Mutex<Vec<PathBuf>>> {
    let asked = Arc::new(Mutex::new(Vec::new()));
    let seen = asked.clone();
    rt.update(move |s, _| {
        s.menu.logs = Some(PathBuf::from("app/logs"));
        s.menu.data = Some(PathBuf::from("fleet-data"));
        s.menu.open = Arc::new(move |dir| seen.lock().unwrap().push(dir.to_path_buf()));
    });
    asked
}

#[test]
fn the_folder_items_ask_for_the_logs_and_for_fleets_data() {
    let mut rt = app();
    let asked = opened_into(&mut rt);
    rt.update(|s, tx| s.menu_run(Item::OpenLogs, tx));
    rt.update(|s, tx| s.menu_run(Item::OpenDataFolder, tx));
    assert_eq!(*asked.lock().unwrap(), [PathBuf::from("app/logs"), PathBuf::from("fleet-data")]);
    // The palette entry is the same call.
    rt.update(|s, tx| commands::run(s, tx, "open-logs-folder"));
    assert_eq!(asked.lock().unwrap().len(), 3);
}

#[test]
fn no_folder_known_asks_for_nothing() {
    let mut rt = app();
    let asked = opened_into(&mut rt);
    rt.update(|s, tx| {
        s.menu.logs = None;
        s.menu_run(Item::OpenLogs, tx);
    });
    assert!(asked.lock().unwrap().is_empty());
}

#[test]
fn report_a_problem_opens_the_report_and_find_opens_the_bar() {
    let mut rt = app();
    rt.frame(FRAME);
    assert!(rt.state.report.opening.is_none());
    rt.update(|s, tx| s.menu_run(Item::ReportProblem, tx));
    assert!(rt.state.report.opening.is_some());
    rt.update(|s, _| s.report_close());

    let was = rt.state.find.open;
    rt.update(|s, tx| s.menu_run(Item::FindInConversation, tx));
    assert!(!was && rt.state.find.open, "the menu opened the conversation's bar");
}

#[test]
fn full_screen_is_a_live_palette_entry_that_asks_the_window() {
    let mut rt = app();
    assert!(matches!(palette_of(&rt.state, "toggle-fullscreen").run, commands::Run::Now(_)));
    rt.update(|s, tx| s.menu_run(Item::ToggleFullScreen, tx));
    // Headless runs have no window to change; the handler's request is what Rune keeps.
}

#[test]
fn quit_goes_through_the_quit_rules() {
    let mut rt = app();
    rt.update(|s, tx| s.menu_run(Item::Quit, tx));
    assert_eq!(rt.shell_request.take(), Some(Shell::Quit), "a Fleet the app didn't start: quits at once");
}

#[test]
fn check_for_updates_says_what_it_can_and_wakes_the_checker() {
    let mut rt = app();
    let said = |rt: &Runtime<State>| rt.state.ui.toast.as_ref().map(|t| t.message.clone());

    // A build that never checks (a dev build) has no checker.
    rt.update(|s, tx| s.menu_run(Item::CheckForUpdates, tx));
    assert_eq!(said(&rt).as_deref(), Some(menu::NO_SELF_UPDATE));

    let (wake, woken) = std::sync::mpsc::channel();
    rt.update(move |s, _| s.update.wake = Some(wake));
    rt.update(|s, tx| s.menu_run(Item::CheckForUpdates, tx));
    assert_eq!(said(&rt).as_deref(), Some(menu::CHECKING));
    assert!(woken.try_recv().is_ok(), "the checker was woken");

    // Asked again while it looks: told so, and not woken twice.
    rt.update(|s, tx| s.menu_run(Item::CheckForUpdates, tx));
    assert_eq!(said(&rt).as_deref(), Some(menu::ALREADY_CHECKING));
    assert!(woken.try_recv().is_err());
}

#[test]
fn every_macos_entry_becomes_a_menu_item_with_a_key_rune_accepts() {
    assert_eq!(crate::menu_bar::items().len(), menu::macos().len());
    for e in menu::macos() {
        if let Some(k) = e.key {
            assert!(k.parse::<rune::Shortcut>().is_ok(), "{k} is a key Rune can show on a menu item");
        }
    }
}
