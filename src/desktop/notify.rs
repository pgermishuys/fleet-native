//! System notifications (component 24, piece F): what `new Notification(...)` does in Fleet's client. A notification has a
//! tag (the session's id: posting again with the same tag replaces it), a title and a body; a click on it comes back as the
//! tag. macOS and Windows go through Rune's `rune-notify`; Linux has no such crate, so it speaks the freedesktop
//! notification protocol (`org.freedesktop.Notifications`) over D-Bus on a thread of its own.
//!
//! Nothing here blocks the UI thread: `post` and `take_down` only hand a command to that thread (Linux) or to the system's
//! own queue (macOS, Windows).
use crate::State;
use crate::fleet::link::Post;
use std::sync::Arc;

/// The name the system shows and groups notifications under; on Windows also the `AppUserModelID`. Not Electron's
/// `com.pgermishuys.fleet`, so the two apps can run together (decision 10).
pub const APP_ID: &str = "com.pgermishuys.fleet-native";
pub const APP_NAME: &str = "Fleet";

/// What the app needs from the system's notifications.
pub trait Notifier: Send + Sync {
    /// Whether a post can reach the person: a bundled app on macOS, a registered one on Windows. (Fleet's client asks the
    /// browser for permission; native has none to ask, the system decides.)
    fn can_post(&self) -> bool;
    /// Shows the notification, replacing the one with this tag. `silent` asks the system for no sound where it can.
    fn post(&self, tag: &str, title: &str, body: &str, silent: bool);
    /// Takes the notification with this tag off the screen and out of the system's list.
    fn take_down(&self, tag: &str);
}

/// The notifications the app has posted, and who posts them.
#[derive(Default)]
pub struct NotifyState {
    /// `None` in scenes, tests and pictures, which post nothing.
    pub notifier: Option<Arc<dyn Notifier>>,
    /// The tags posted and not yet taken down or clicked, so opening a session only asks the system to remove what it holds.
    pub shown: std::collections::HashSet<String>,
}

/// The system's notifications for this app. A click goes to the app as a job on the UI thread.
pub struct Real {
    #[cfg(target_os = "linux")]
    linux: linux::Bus,
}

impl Real {
    pub fn new(post: Post) -> Real {
        let on_click = move |tag: String| post.update(move |s: &mut State, tx| s.notification_clicked(&tag, tx));
        #[cfg(target_os = "linux")]
        {
            Real { linux: linux::Bus::start(Arc::new(on_click)) }
        }
        #[cfg(not(target_os = "linux"))]
        {
            // Windows gives the process its AppUserModelID (and groups its toasts); macOS reads both from the bundle.
            let _ = rune_notify::register(&rune_notify::Identity::new(APP_ID, APP_NAME));
            rune_notify::on_click(move |click| {
                if let Some(tag) = click.get("tag") {
                    on_click(tag.to_string());
                }
            });
            Real {}
        }
    }
}

#[cfg(not(target_os = "linux"))]
impl Notifier for Real {
    fn can_post(&self) -> bool {
        rune_notify::available()
    }
    fn post(&self, tag: &str, title: &str, body: &str, _silent: bool) {
        // `rune-notify` posts silently, which is what every note of this app asks for.
        rune_notify::post(&rune_notify::Notification::new(tag, title).body(body).info("tag", tag));
    }
    fn take_down(&self, tag: &str) {
        rune_notify::remove(&[tag]);
    }
}

#[cfg(target_os = "linux")]
impl Notifier for Real {
    /// Whether a notification daemon listens is only known by asking; the first post tells (and a refusal is logged once).
    fn can_post(&self) -> bool {
        true
    }
    fn post(&self, tag: &str, title: &str, body: &str, silent: bool) {
        self.linux.send(linux::Cmd::Post { tag: tag.into(), title: title.into(), body: body.into(), silent });
    }
    fn take_down(&self, tag: &str) {
        self.linux.send(linux::Cmd::Down(tag.into()));
    }
}

/// The arguments of `Notify` (freedesktop notification spec 1.2), as plain data so the call can be checked without a bus.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
#[derive(Debug, PartialEq)]
pub struct NotifyCall {
    pub replaces_id: u32,
    pub summary: String,
    pub body: String,
    /// `["default", "Open"]`: the action a click on the notification itself invokes.
    pub actions: Vec<&'static str>,
    pub silent: bool,
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub fn notify_call(replaces_id: u32, title: &str, body: &str, silent: bool) -> NotifyCall {
    NotifyCall { replaces_id, summary: title.into(), body: body.into(), actions: vec!["default", "Open"], silent }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::{APP_ID, APP_NAME, notify_call};
    use futures_util::StreamExt;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
    use zbus::Variant;
    use zbus::{Connection, Proxy};

    const DEST: &str = "org.freedesktop.Notifications";
    const PATH: &str = "/org/freedesktop/Notifications";

    pub enum Cmd {
        Post { tag: String, title: String, body: String, silent: bool },
        Down(String),
    }

    pub struct Bus {
        commands: UnboundedSender<Cmd>,
    }

    impl Bus {
        pub fn start(on_click: Arc<dyn Fn(String) + Send + Sync>) -> Bus {
            let (commands, mut inbox) = unbounded_channel();
            let spawned = std::thread::Builder::new().name("fleet-notify".into()).spawn(move || {
                let Ok(runtime) = tokio::runtime::Builder::new_current_thread().enable_all().build() else { return };
                runtime.block_on(async move {
                    if let Err(why) = serve(&mut inbox, on_click).await {
                        eprintln!("fleet-native: no desktop notifications ({why})");
                        // Nobody to post to: take what comes and drop it, so a sender is never blocked or leaking.
                        while inbox.recv().await.is_some() {}
                    }
                });
            });
            if let Err(why) = spawned {
                eprintln!("fleet-native: no desktop notifications ({why})");
            }
            Bus { commands }
        }

        pub fn send(&self, cmd: Cmd) {
            let _ = self.commands.send(cmd);
        }
    }

    async fn serve(inbox: &mut tokio::sync::mpsc::UnboundedReceiver<Cmd>, on_click: Arc<dyn Fn(String) + Send + Sync>) -> zbus::Result<()> {
        let conn = Connection::session().await?;
        let proxy = Proxy::new(&conn, DEST, PATH, DEST).await?;
        let mut invoked = proxy.receive_signal("ActionInvoked").await?;
        let mut closed = proxy.receive_signal("NotificationClosed").await?;
        // tag -> the daemon's id for it; a post with a tag already here replaces that notification.
        let mut ids: HashMap<String, u32> = HashMap::new();
        loop {
            tokio::select! {
                cmd = inbox.recv() => match cmd {
                    None => return Ok(()),
                    Some(Cmd::Post { tag, title, body, silent }) => {
                        let call = notify_call(ids.get(&tag).copied().unwrap_or(0), &title, &body, silent);
                        let mut hints: HashMap<&str, Variant<'_>> = HashMap::new();
                        hints.insert("desktop-entry", Variant::from(APP_ID));
                        if call.silent {
                            hints.insert("suppress-sound", Variant::from(true));
                        }
                        let sent = proxy
                            .call_method("Notify", &(APP_NAME, call.replaces_id, "", &call.summary, &call.body, &call.actions, hints, -1i32))
                            .await;
                        match sent.and_then(|reply| reply.body().deserialize::<u32>()) {
                            Ok(id) => {
                                ids.insert(tag, id);
                            }
                            Err(why) => eprintln!("fleet-native: a notification wasn't posted ({why})"),
                        }
                    }
                    Some(Cmd::Down(tag)) => {
                        if let Some(id) = ids.remove(&tag) {
                            let _ = proxy.call_method("CloseNotification", &(id,)).await;
                        }
                    }
                },
                Some(signal) = invoked.next() => {
                    if let Ok((id, key)) = signal.body().deserialize::<(u32, String)>()
                        && key == "default"
                        && let Some(tag) = ids.iter().find(|(_, v)| **v == id).map(|(k, _)| k.clone())
                    {
                        ids.remove(&tag);
                        on_click(tag);
                    }
                },
                Some(signal) = closed.next() => {
                    if let Ok((id, _reason)) = signal.body().deserialize::<(u32, u32)>() {
                        ids.retain(|_, v| *v != id);
                    }
                },
            }
        }
    }
}

#[cfg(test)]
pub use recorder::Recorder;

#[cfg(test)]
mod recorder {
    use super::Notifier;
    use std::sync::Mutex;

    /// A notifier that only writes down what it was asked, for tests.
    pub struct Recorder {
        pub can: bool,
        pub calls: Mutex<Vec<String>>,
    }

    impl Recorder {
        pub fn new(can: bool) -> Recorder {
            Recorder { can, calls: Mutex::new(Vec::new()) }
        }
        pub fn all(&self) -> Vec<String> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl Notifier for Recorder {
        fn can_post(&self) -> bool {
            self.can
        }
        fn post(&self, tag: &str, title: &str, body: &str, silent: bool) {
            self.calls.lock().unwrap().push(format!("post {tag} | {title} | {body}{}", if silent { " | silent" } else { "" }));
        }
        fn take_down(&self, tag: &str) {
            self.calls.lock().unwrap().push(format!("down {tag}"));
        }
    }
}
