//! A watcher for the live check (`docs/parity/02-connect-and-events.md`, "The live check"). Ignored: it needs a
//! scratch Fleet and a driver sending prompts. It reads what the app's state holds (the parts no view draws yet:
//! work, delegations, context, the events handed to subscribers) and prints one line each time that changes.
//!
//!     FLEET_LIVE_URL=http://127.0.0.1:6411 FLEET_LIVE_SECS=240 cargo test live_watch -- --ignored --nocapture
use super::super::link::{Job, Link, Post};
use super::reconnect::app;
use crate::State;
use std::collections::BTreeMap;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn summary(s: &State, seen: &BTreeMap<String, u32>) -> String {
    let rows: Vec<String> = s
        .projects
        .iter()
        .flat_map(|p| &p.sessions)
        .map(|r| format!("{}={:?}/{}/{}", r.title, r.status, r.row.activity, r.row.lifecycle))
        .collect();
    let stream = match &s.stream {
        None => "no stream".to_string(),
        Some(st) => {
            let deleg: Vec<String> =
                st.delegations.iter().map(|d| format!("{:?}/bg={}/child={:?}", d.status, d.background, d.child_activity_status)).collect();
            let work: Vec<String> = st.running_work.iter().map(|w| format!("{:?}:{:?}/bg={}", w.kind, w.status, w.background)).collect();
            format!(
                "status={:?}/{:?} msgs={} delegations={deleg:?} work={work:?} context={:?} cursor={:?}",
                st.session_status,
                st.explicit_status,
                st.messages.len(),
                st.context.as_ref().map(|c| (c.used, c.limit)),
                st.last_event_id
            )
        }
    };
    let kinds: Vec<String> =
        seen.iter().filter(|(k, _)| !k.starts_with("Message") && !k.starts_with("Turn")).map(|(k, n)| format!("{k}x{n}")).collect();
    format!(
        "{:?} | open={:?} | {stream} | rows={rows:?} | events={kinds:?}",
        s.connection,
        s.open_id().map(|i| i[..8.min(i.len())].to_string())
    )
}

#[test]
#[ignore = "needs a scratch Fleet"]
fn live_watch() {
    let Ok(url) = std::env::var("FLEET_LIVE_URL") else { return };
    let secs: u64 = std::env::var("FLEET_LIVE_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(120);
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    let post = Post::new(move |job| {
        let _ = jobs.lock().unwrap().send(job);
    });
    let mut rt = app();
    let seen = Arc::new(Mutex::new(BTreeMap::<String, u32>::new()));
    let link = Link::start(url, post);
    let counted = seen.clone();
    rt.update(move |s, _| {
        s.link = Some(link);
        s.subscribe(None, move |_, e| *counted.lock().unwrap().entry(format!("{:?}", e.kind())).or_default() += 1);
    });
    let (start, mut last) = (Instant::now(), String::new());
    while start.elapsed() < Duration::from_secs(secs) {
        while let Ok(job) = queue.try_recv() {
            rt.update(job);
        }
        rt.frame(1.0 / 60.0);
        let now = summary(&rt.state, &seen.lock().unwrap());
        if now != last {
            println!("[{:>5.1}s] {now}", start.elapsed().as_secs_f32());
            last = now;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
