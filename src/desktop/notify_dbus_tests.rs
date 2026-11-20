//! Linux notifications against a stand-in notification daemon on a real session bus (component 24, piece F). It needs a
//! session bus of its own and no daemon already on it, so it runs only when `FLEET_NATIVE_DBUS_TEST=1`, under
//! `dbus-run-session -- cargo test notify_dbus`. Without that it says it was skipped.
use super::notify::{Notifier, Real};
use crate::fleet::link::{Job, Post};
use crate::test_wait;
use std::collections::HashMap;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use zbus::OwnedVariant;

const DEST: &str = "org.freedesktop.Notifications";
const PATH: &str = "/org/freedesktop/Notifications";

type Log = Arc<Mutex<Vec<String>>>;

struct Daemon {
    log: Log,
    next: u32,
}

#[zbus::interface(name = "org.freedesktop.Notifications")]
impl Daemon {
    #[allow(clippy::too_many_arguments)]
    fn notify(
        &mut self,
        app: String,
        replaces: u32,
        _icon: String,
        summary: String,
        body: String,
        actions: Vec<String>,
        hints: HashMap<String, OwnedVariant>,
        timeout: i32,
    ) -> u32 {
        let id = if replaces == 0 {
            self.next += 1;
            self.next
        } else {
            replaces
        };
        let mut keys: Vec<_> = hints.keys().cloned().collect();
        keys.sort();
        self.log
            .lock()
            .unwrap()
            .push(format!("notify {app} #{id} replaces={replaces} {summary} | {body} | {actions:?} | {keys:?} | {timeout}"));
        id
    }
    fn close_notification(&self, id: u32) {
        self.log.lock().unwrap().push(format!("close #{id}"));
    }
}

#[test]
fn notify_dbus_posts_replaces_closes_and_hears_a_click() {
    if std::env::var("FLEET_NATIVE_DBUS_TEST").as_deref() != Ok("1") {
        eprintln!("skipped: set FLEET_NATIVE_DBUS_TEST=1 under dbus-run-session");
        return;
    }
    let log: Log = Arc::default();
    let (click_tx, mut click_rx) = tokio::sync::mpsc::unbounded_channel::<()>();
    let (served_tx, served_rx) = channel::<()>();
    let daemon_log = log.clone();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        runtime.block_on(async move {
            let conn = zbus::connection::Builder::session()
                .name(DEST)
                .serve_at(PATH, Daemon { log: daemon_log, next: 40 })
                .build()
                .await
                .expect("a session bus with no notification daemon on it");
            served_tx.send(()).unwrap();
            // The person clicks notification #41.
            // Awaiting (not a blocking recv): the bus connection is served by this same runtime.
            while click_rx.recv().await.is_some() {
                conn.emit_signal(None::<&str>, PATH, DEST, "ActionInvoked", &(41u32, "default")).await.unwrap();
            }
        });
    });
    test_wait::recv("the stand-in daemon", &served_rx);

    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    let notifier = Real::new(Post::new(move |job| {
        let _ = jobs.lock().unwrap().send(job);
    }));
    let seen = |n: usize| {
        let log = log.clone();
        move || log.lock().unwrap().len() >= n
    };
    notifier.post("s1", "Make the dashboard true", "Waiting on your answer.", false);
    test_wait::until_true("the first notify", seen(1));
    notifier.post("s1", "Make the dashboard true", "Finished its turn.", true);
    test_wait::until_true("the second notify", seen(2));
    {
        let log = log.lock().unwrap();
        assert_eq!(
            log[0],
            "notify Fleet #41 replaces=0 Make the dashboard true | Waiting on your answer. | [\"default\", \"Open\"] | [\"desktop-entry\"] | -1"
        );
        assert_eq!(
            log[1],
            "notify Fleet #41 replaces=41 Make the dashboard true | Finished its turn. | [\"default\", \"Open\"] | [\"desktop-entry\", \"suppress-sound\"] | -1",
            "the same tag replaces the notification the daemon holds"
        );
    }
    // A click on it comes back as the session to open.
    click_tx.send(()).unwrap();
    let job = test_wait::recv("the click", &queue);
    let mut rt = crate::tests::app();
    rt.update(move |s, tx| job(s, tx));
    assert_eq!(rt.state.open_id().as_deref(), Some("s1"));
    // The click spent the notification: taking it down afterwards asks the daemon for nothing.
    notifier.post("s2", "Another", "One", false);
    test_wait::until_true("a notification for s2", seen(3));
    notifier.take_down("s2");
    test_wait::until_true("the close", seen(4));
    assert_eq!(log.lock().unwrap()[3], "close #42");
}
