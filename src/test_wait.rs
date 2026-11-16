//! The one way a test waits for background work (a thread, a server, a file, the clock). A fixed short bound fails on a
//! busy runner (the queue test on a Windows runner with no build cache), so every wait gets the same generous bound,
//! polls instead of sleeping a set time, and says what it was waiting for and what the state was when it gave up.
//! A bound that tests real timing on purpose (a debounce, a timeout the app itself keeps) does not belong here.
use std::time::{Duration, Instant};

/// Long enough that only something that will never happen runs into it; a passing wait returns as soon as it holds.
pub const BOUND: Duration = Duration::from_secs(60);

const POLL: Duration = Duration::from_millis(5);

/// Calls `step` until it returns true, with `context` handed to it each time. Panics after [`BOUND`] with `what` and
/// `describe(context)`, so a failure on a runner says what was missing and where things stood.
pub fn until<C>(context: &mut C, what: &str, mut step: impl FnMut(&mut C) -> bool, describe: impl Fn(&C) -> String) {
    let deadline = Instant::now() + BOUND;
    loop {
        if step(context) {
            return;
        }
        assert!(Instant::now() < deadline, "timed out after {}s waiting for {what}; state: {}", BOUND.as_secs(), describe(context));
        std::thread::sleep(POLL);
    }
}

/// [`until`] for a wait with nothing to describe beyond its name.
pub fn until_true(what: &str, mut step: impl FnMut() -> bool) {
    until(&mut (), what, |_| step(), |_| "(nothing kept)".into());
}

/// Waits for a value another thread sends.
pub fn recv<T>(what: &str, channel: &std::sync::mpsc::Receiver<T>) -> T {
    channel.recv_timeout(BOUND).unwrap_or_else(|e| panic!("timed out after {}s waiting for {what}: {e}", BOUND.as_secs()))
}

/// Runs a headless app until `done` holds: its jobs (what threads hand back) are applied and a frame is drawn between
/// looks. `describe` says what the state was if it never does.
pub fn drive(
    rt: &mut rune::Runtime<crate::State>,
    jobs: &std::sync::mpsc::Receiver<crate::fleet::link::Job>,
    what: &str,
    mut done: impl FnMut(&crate::State) -> bool,
    describe: impl Fn(&crate::State) -> String,
) {
    until(
        rt,
        what,
        |rt| {
            if done(&rt.state) {
                return true;
            }
            while let Ok(job) = jobs.try_recv() {
                rt.update(job);
            }
            rt.frame(1.0 / 60.0);
            done(&rt.state)
        },
        |rt| describe(&rt.state),
    );
}

/// What most waits want said about the app.
pub fn app_state(s: &crate::State) -> String {
    format!(
        "connection {:?}, notice {:?}, {} projects, open {:?}, {} messages",
        s.connection,
        s.notice,
        s.projects.len(),
        s.open_id(),
        s.session().messages.len()
    )
}
