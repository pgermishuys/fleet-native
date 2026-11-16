//! `GET /api/plugins` at start and then every 30 s (`IconRail.vue`, `loadPluginStatuses`), on a thread of its own and
//! never on the UI thread. The answer reaches `State::plugins` with `Post::update`; the thread ends when the sender
//! the link keeps is dropped.
use super::http;
use super::link::Post;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::time::Duration;

pub const EVERY: Duration = Duration::from_secs(30);

/// Starts the poll for the Fleet at `base`. Keep the returned sender for as long as the poll should run.
pub fn start(base: String, post: Post, every: Duration) -> Sender<()> {
    let (tx, rx) = channel();
    std::thread::spawn(move || run(&base, &rx, &post, every));
    tx
}

fn run(base: &str, rx: &Receiver<()>, post: &Post, every: Duration) {
    loop {
        let answer = http::get(base, "/api/plugins");
        post.update(move |s, _| s.plugins.adopt(answer.as_ref().map_err(Clone::clone)));
        match rx.recv_timeout(every) {
            Err(RecvTimeoutError::Timeout) | Ok(()) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}
