//! Piece E's tests: the close rules for each system, owned or not, working or not; the status call against a fake server;
//! the flow of a close, a quit and the tray with a recording stand-in for the window; the dialog's words and its default;
//! the updater waiting for its Fleet to stop. Fleet's own `desktop.test.ts` has no case for these (they are Electron
//! handlers), so the names say what `main.ts` does.
use super::close::{self, Close, Outcome, Surface, System, TrayAction};
use super::server::FleetServer;
use super::tests::{Scratch, options};
use crate::fleet::link::{Job, Link, Post};
use crate::tests::{FRAME, app};
use crate::{State, test_http, test_wait};
use rune::tray::TrayEvent;
use rune::{Key, KeyEvent, Mods, Runtime, Shell};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

mod rules {
    use super::*;

    #[test]
    fn macos_hides_on_close_whatever_the_fleet() {
        for owned in [false, true] {
            for working in [0, 3] {
                assert_eq!(close::decide(System::Mac, owned, working), Outcome::Hide, "owned {owned}, working {working}");
            }
        }
    }

    #[test]
    fn a_fleet_the_app_did_not_start_is_left_running_and_closing_quits() {
        for working in [0, 2] {
            assert_eq!(close::decide(System::Other, false, working), Outcome::Quit);
        }
        assert_eq!(close::on_close(System::Other, false), Close::Quit, "it is never asked");
    }

    #[test]
    fn its_own_fleet_with_sessions_working_goes_to_the_tray_and_with_none_quits() {
        assert_eq!(close::on_close(System::Other, true), Close::Ask);
        assert_eq!(close::decide(System::Other, true, 0), Outcome::Quit);
        assert_eq!(close::decide(System::Other, true, 1), Outcome::Tray(1));
        assert_eq!(close::decide(System::Other, true, 4), Outcome::Tray(4));
    }

    #[test]
    fn the_tooltip_and_the_note_say_how_many() {
        assert_eq!(close::tooltip(1), "Fleet: a session is working");
        assert_eq!(close::tooltip(3), "Fleet: 3 sessions are working");
        assert_eq!(
            close::still_running(1),
            ("Fleet is still running".to_string(), "A session is still working. Quit from the tray icon when you're done.".to_string())
        );
        assert_eq!(close::still_running(3).1, "3 sessions are still working. Quit from the tray icon when you're done.");
    }

    #[test]
    fn the_tray_menu_ids_become_actions() {
        assert_eq!(close::tray_action(&TrayEvent::Activate { token: None }), Some(TrayAction::Show));
        assert_eq!(close::tray_action(&TrayEvent::Menu(close::MENU_SHOW.into())), Some(TrayAction::Show));
        assert_eq!(close::tray_action(&TrayEvent::Menu(close::MENU_QUIT.into())), Some(TrayAction::Quit));
        assert_eq!(close::tray_action(&TrayEvent::Menu("other".into())), None);
        let ids: Vec<_> = super::super::tray::menu()
            .into_iter()
            .filter_map(|i| match i {
                rune::tray::TrayItem::Action { id, label, .. } => Some((id, label)),
                _ => None,
            })
            .collect();
        assert_eq!(
            ids,
            [("show".into(), "Show Fleet".into()), ("quit".into(), "Quit Fleet".into())],
            "the menu: Show Fleet, a line, Quit Fleet"
        );
    }

    #[test]
    fn the_tray_icon_decodes_to_its_pixels() {
        let icon = super::super::tray::decode_icon(include_bytes!("../../assets/tray.png")).expect("tray.png decodes");
        // Rune refuses an icon whose bytes aren't width × height × 4, so a good decode is one `Tray::new` would take.
        let spec = rune::tray::TraySpec::new("t", "T", icon);
        assert!(format!("{spec:?}").contains("width: 32"), "{spec:?}");
        assert!(super::super::tray::decode_icon(b"not a picture").is_err());
    }
}

mod status {
    use super::*;

    /// A Fleet that answers `/api/desktop/status` with `status` and `body`; returns its address and the paths it was asked.
    fn fake(status: u16, body: &'static str) -> (String, Arc<Mutex<Vec<String>>>) {
        let listener = crate::test_http::bind().unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let asked = Arc::new(Mutex::new(Vec::new()));
        let seen = asked.clone();
        std::thread::spawn(move || {
            while let Ok((mut stream, _)) = listener.accept() {
                let request = test_http::read(&mut stream);
                seen.lock().unwrap().push(request.path.clone());
                test_http::respond_json(&mut stream, status, body);
            }
        });
        (base, asked)
    }

    #[test]
    fn reads_the_number_of_working_sessions() {
        let (base, asked) = fake(200, r#"{"workingSessions":3}"#);
        assert_eq!(crate::fleet::http::desktop_working(&base), 3);
        assert_eq!(*asked.lock().unwrap(), ["/api/desktop/status"]);
    }

    #[test]
    fn a_missing_or_wrong_field_is_none_working() {
        for body in ["{}", r#"{"workingSessions":"3"}"#, r#"{"workingSessions":null}"#, r#"{"workingSessions":-1}"#, "[]", "not json"] {
            let (base, _) = fake(200, Box::leak(body.to_string().into_boxed_str()));
            assert_eq!(crate::fleet::http::desktop_working(&base), 0, "{body}");
        }
    }

    #[test]
    fn an_error_answer_or_no_answer_is_none_working() {
        let (base, _) = fake(500, r#"{"workingSessions":5}"#);
        assert_eq!(crate::fleet::http::desktop_working(&base), 0, "an error answer");
        // Nothing listens here: a port bound and let go.
        let gone = crate::test_http::bind().unwrap().local_addr().unwrap();
        assert_eq!(crate::fleet::http::desktop_working(&format!("http://{gone}")), 0, "no answer");
    }

    #[test]
    fn gives_up_after_two_seconds() {
        let listener = crate::test_http::bind().unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        // Accepts and never answers; the stream is kept open for the length of the call.
        let (hold_tx, hold_rx) = channel::<()>();
        let holder = std::thread::spawn(move || {
            let kept = listener.accept().map(|(s, _)| s);
            let _ = hold_rx.recv();
            drop(kept);
        });
        let started = std::time::Instant::now();
        assert_eq!(crate::fleet::http::desktop_working(&base), 0);
        let took = started.elapsed();
        assert!(took >= Duration::from_millis(1500) && took < Duration::from_secs(10), "{took:?}");
        let _ = hold_tx.send(());
        holder.join().unwrap();
    }
}

/// What the window and tray were asked, in order.
#[derive(Default)]
struct Calls(Mutex<Vec<String>>);

impl Calls {
    fn all(&self) -> Vec<String> {
        self.0.lock().unwrap().clone()
    }
}

impl Surface for Calls {
    fn show_window(&self) {
        self.0.lock().unwrap().push("window".into());
    }
    fn show_tray(&self, working: usize) {
        self.0.lock().unwrap().push(format!("tray {working}"));
    }
    fn hide_tray(&self) {
        self.0.lock().unwrap().push("tray off".into());
    }
}

struct Rig {
    rt: Runtime<State>,
    jobs: Receiver<Job>,
    calls: Arc<Calls>,
    asked: Arc<Mutex<Vec<String>>>,
    _scratch: Scratch,
}

/// An app whose Fleet is its own (a stand-in server it started) when `owned`, with `working` sessions working.
fn rig(name: &str, owned: bool, working: usize) -> Rig {
    let scratch = Scratch::new(&format!("close-{name}"));
    let (send, jobs) = channel::<Job>();
    let send = Mutex::new(send);
    let post = Post::new(move |job| {
        let _ = send.lock().unwrap().send(job);
    });
    let calls = Arc::new(Calls::default());
    let asked = Arc::new(Mutex::new(Vec::new()));
    let mut rt = app();
    let stopper = owned.then(|| {
        let server = FleetServer::start(options(&scratch, "serve")).unwrap();
        let stopper = server.stopper();
        // The stand-in is stopped by the test through the stopper; the handle only has to outlive it.
        std::mem::forget(server);
        stopper
    });
    let (surface, seen, link_post) = (calls.clone(), asked.clone(), post.clone());
    rt.update(move |s, _| {
        s.link = Some(Link::start("http://127.0.0.1:1".into(), link_post));
        s.start.owned = stopper;
        s.close.surface = Some(surface);
        s.close.post = Some(post);
        s.close.status = Arc::new(move |base| {
            seen.lock().unwrap().push(base.to_string());
            working
        });
    });
    Rig { rt, jobs, calls, asked, _scratch: scratch }
}

impl Rig {
    fn closed(&mut self) {
        self.rt.update(|s, tx| s.window_closed(tx));
    }

    fn quit_asked(&mut self) {
        self.rt.update(|s, tx| s.quit_requested(tx));
    }

    fn quit(&mut self) -> bool {
        self.rt.shell_request.take() == Some(Shell::Quit)
    }

    fn settle(&mut self, what: &str, done: impl FnMut(&State) -> bool) {
        test_wait::drive(&mut self.rt, &self.jobs, what, done, |s| {
            format!("{:?} {:?} asking {}", s.close.in_tray, s.close.quit_dialog, s.close.asking)
        });
    }

    /// Stops what the rig started, so no server outlives its test.
    fn stop(&mut self) {
        if let Some(owned) = self.rt.state.start.owned.take() {
            owned.stop(Duration::from_secs(10));
        }
    }
}

mod closing {
    use super::*;

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn closing_with_a_fleet_it_did_not_start_quits_without_asking() {
        let mut r = rig("not-owned", false, 5);
        r.closed();
        assert!(r.quit());
        assert!(r.asked.lock().unwrap().is_empty(), "no question to a Fleet that isn't its own");
        assert!(r.calls.all().is_empty());
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn closing_its_own_fleet_with_nothing_working_quits() {
        let mut r = rig("idle", true, 0);
        r.closed();
        r.settle("the Fleet to have been asked", |s| !s.close.asking);
        assert_eq!(r.asked.lock().unwrap().len(), 1);
        assert!(r.quit());
        assert!(r.calls.all().is_empty(), "no tray");
        r.stop();
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn closing_its_own_fleet_with_sessions_working_goes_to_the_tray_and_the_app_keeps_running() {
        let mut r = rig("tray", true, 2);
        r.closed();
        r.settle("the tray", |s| s.close.in_tray.is_some());
        assert_eq!(r.rt.state.close.in_tray, Some(2));
        assert_eq!(r.calls.all(), ["tray 2"]);
        assert!(!r.quit(), "the app and its Fleet keep running");
        assert!(r.rt.state.start.owned.is_some());
        r.stop();
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn a_second_close_while_the_question_is_out_waits_for_the_answer() {
        let mut r = rig("twice", true, 1);
        r.closed();
        r.closed();
        r.settle("the answer", |s| s.close.in_tray.is_some());
        assert_eq!(r.asked.lock().unwrap().len(), 1);
        r.stop();
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn closing_on_a_mac_leaves_the_app_in_the_dock() {
        let mut r = rig("mac", true, 2);
        r.closed();
        assert!(!r.quit());
        assert!(r.asked.lock().unwrap().is_empty());
        r.stop();
    }

    #[test]
    fn a_tray_that_cannot_be_had_gives_the_window_back() {
        let mut r = rig("no-tray", true, 1);
        r.rt.update(|s, _| {
            s.hide_to_tray(1);
            s.tray_failed("no session bus");
        });
        assert_eq!(r.calls.all(), ["tray 1", "window"]);
        assert_eq!(r.rt.state.close.in_tray, None);
        r.stop();
    }
}

mod tray {
    use super::*;

    fn in_the_tray(name: &str) -> Rig {
        let mut r = rig(name, true, 2);
        r.rt.update(|s, _| s.hide_to_tray(2));
        r
    }

    #[test]
    fn show_fleet_and_a_click_show_the_window_and_take_the_icon_away() {
        for event in [TrayEvent::Menu("show".into()), TrayEvent::Activate { token: None }] {
            let mut r = in_the_tray("show");
            r.rt.update(move |s, tx| s.tray_event(&event, tx));
            assert_eq!(r.calls.all(), ["tray 2", "window", "tray off"]);
            assert_eq!(r.rt.state.close.in_tray, None);
            assert!(!r.quit());
            r.stop();
        }
    }

    #[test]
    fn the_window_coming_back_by_any_way_takes_the_icon_away() {
        let mut r = in_the_tray("any-way");
        r.rt.update(|s, _| s.window_visible());
        assert_eq!(r.calls.all(), ["tray 2", "tray off"]);
        r.rt.update(|s, _| s.window_visible());
        assert_eq!(r.calls.all(), ["tray 2", "tray off"], "once");
        r.stop();
    }

    #[test]
    fn quit_fleet_with_sessions_working_asks_in_the_window() {
        let mut r = in_the_tray("quit-asks");
        r.rt.update(|s, tx| s.tray_event(&TrayEvent::Menu("quit".into()), tx));
        r.settle("the dialog", |s| s.close.quit_dialog.is_some());
        assert_eq!(r.rt.state.close.quit_dialog, Some(2));
        assert_eq!(r.calls.all(), ["tray 2", "window"], "the window comes forward to ask");
        assert!(!r.quit(), "nothing is stopped yet");
        r.stop();
    }

    #[test]
    fn quit_fleet_with_nothing_working_quits_and_one_not_its_own_quits_unasked() {
        let mut r = rig("quit-idle", true, 0);
        r.quit_asked();
        r.settle("the answer", |s| !s.close.asking);
        assert!(r.quit());
        assert!(r.rt.state.close.quit_dialog.is_none());
        r.stop();
        let mut r = rig("quit-foreign", false, 4);
        r.quit_asked();
        assert!(r.quit());
        assert!(r.asked.lock().unwrap().is_empty());
    }
}

mod dialog {
    use super::*;

    fn text_of(rt: &Runtime<State>, key: &str) -> Option<String> {
        rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting).and_then(|e| e.text)
    }

    fn open(working: usize) -> Rig {
        let mut r = rig("dialog", false, 0);
        r.rt.update(move |s, _| s.close.quit_dialog = Some(working));
        for _ in 0..4 {
            r.rt.frame(FRAME);
        }
        r
    }

    fn words(rt: &Runtime<State>) -> String {
        rt.elements().iter().filter(|e| e.key.is_none()).filter_map(|e| e.text.clone()).collect::<Vec<_>>().join(" | ")
    }

    #[test]
    fn says_a_session_is_still_working_for_one() {
        let r = open(1);
        let all = words(&r.rt);
        assert!(all.contains("A session is still working"), "{all}");
        assert!(all.contains("Quitting Fleet stops them."), "{all}");
        assert!(all.contains("Quit Fleet") && all.contains("Cancel"), "{all}");
        assert!(text_of(&r.rt, "quit-confirm").is_some() || r.rt.find("quit-confirm").is_some());
        assert_eq!(close::quit_message(1), "A session is still working");
    }

    #[test]
    fn says_how_many_for_three() {
        let r = open(3);
        assert!(words(&r.rt).contains("3 sessions are still working"));
    }

    // "Cancel is the default" (`defaultId: 1`) is the `autofocus` on the Cancel button (`view::quit_dialog`), as `ui::confirm` does for a
    // destructive action: where a modal puts focus on open can't be read headless (a headless modal focuses itself), so it is
    // looked at in a window (the pull request says how).

    #[test]
    fn escape_cancels() {
        let mut r = open(2);
        r.rt.key(KeyEvent { key: Key::Escape, mods: Mods::default() });
        r.rt.frame(FRAME);
        assert_eq!(r.rt.state.close.quit_dialog, None);
        assert!(!r.quit());
    }

    #[test]
    fn quit_fleet_quits_and_cancel_does_not() {
        let mut r = open(2);
        crate::tests::click(&mut r.rt, "quit-cancel");
        assert_eq!(r.rt.state.close.quit_dialog, None);
        assert!(!r.quit());
        r.rt.update(|s, _| s.close.quit_dialog = Some(2));
        r.rt.frame(FRAME);
        r.rt.frame(FRAME);
        crate::tests::click(&mut r.rt, "quit-confirm");
        assert!(r.quit());
    }
}

mod update {
    use super::*;

    #[test]
    fn the_updater_waits_until_its_fleet_is_gone_before_it_restarts() {
        let mut r = rig("update", true, 0);
        let log = r._scratch.log();
        let post = r.rt.state.close.post.clone().unwrap();
        let waiter = std::thread::spawn(move || crate::update::stop_owned_fleet(&post, Duration::from_secs(10)));
        test_wait::drive(
            &mut r.rt,
            &r.jobs,
            "the stop to finish",
            |_| waiter.is_finished(),
            |s| format!("owned {}", s.start.owned.is_some()),
        );
        waiter.join().unwrap();
        assert!(r.rt.state.start.owned.is_none(), "the state let go of it");
        assert!(std::fs::read_to_string(&log).is_ok_and(|t| t.contains("Fleet exited")), "it had exited by the time the wait returned");
    }

    #[test]
    fn with_no_fleet_of_its_own_the_updater_does_not_wait() {
        let mut r = rig("update-none", false, 0);
        let post = r.rt.state.close.post.clone().unwrap();
        let waiter = std::thread::spawn(move || crate::update::stop_owned_fleet(&post, Duration::from_secs(10)));
        test_wait::drive(&mut r.rt, &r.jobs, "the call to return", |_| waiter.is_finished(), |_| String::new());
        waiter.join().unwrap();
    }
}
