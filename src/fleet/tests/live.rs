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

/// The whole composer against a scratch Fleet, headless: a model picked, a send, a message queued and one steered, a
/// file attached and uploaded, an `@` session reference, `!` and `/btw`. Prints each step with what the server holds.
///
///     FLEET_LIVE_URL=http://127.0.0.1:6471 cargo test live_composer -- --ignored --nocapture
///
/// The scratch Fleet needs "Fix pagination" and a second session ("Notes"), which is the one that opens; the `@` list offers the other.
#[test]
#[ignore = "needs a scratch Fleet"]
fn live_composer() {
    use rune::{Key, KeyEvent, Mods, Runtime};
    let Ok(url) = std::env::var("FLEET_LIVE_URL") else { return };
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    let post = Post::new(move |job| {
        let _ = jobs.lock().unwrap().send(job);
    });
    let mut rt = app();
    let link = Link::start(url.clone(), post);
    rt.update(move |s, _| s.link = Some(link));

    fn pump(rt: &mut Runtime<State>, queue: &std::sync::mpsc::Receiver<Job>, secs: f32) {
        let start = Instant::now();
        while start.elapsed().as_secs_f32() < secs {
            while let Ok(job) = queue.try_recv() {
                rt.update(job);
            }
            rt.frame(1.0 / 60.0);
            std::thread::sleep(Duration::from_millis(30));
        }
    }
    fn key(rt: &mut Runtime<State>, key: Key, ctrl: bool) {
        rt.key(KeyEvent { key, mods: Mods { ctrl, ..Mods::default() } });
        rt.frame(1.0 / 60.0);
        rt.frame(1.0 / 60.0);
    }
    fn typed(rt: &mut Runtime<State>, text: &str) {
        key(rt, Key::Text(text.into()), false);
    }
    fn click(rt: &mut Runtime<State>, name: &str) {
        let (x, y) = rt.find(name).unwrap_or_else(|| panic!("no element keyed {name:?}"));
        rt.pointer_move(x, y);
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        for _ in 0..30 {
            rt.frame(1.0 / 60.0);
        }
    }
    let messages = |id: &str| -> String {
        let out = std::process::Command::new("curl").args(["-s", &format!("{url}/api/sessions/{id}/messages")]).output().unwrap();
        String::from_utf8_lossy(&out.stdout).to_string()
    };

    pump(&mut rt, &queue, 6.0);
    let id = rt.state.open_id().expect("the first session is open");
    println!("open session {} ({} sessions)", &id[..8], rt.state.projects.iter().map(|p| p.sessions.len()).sum::<usize>());
    println!("model chip: {:?}", rt.state.choice().model);

    // A model picked from the session's own list, then a prompt that makes the agent work.
    click(&mut rt, "model");
    typed(&mut rt, "fake");
    key(&mut rt, Key::Enter, false);
    println!("picked model: {:?}", rt.state.choice().model);
    click(&mut rt, "composer");
    typed(&mut rt, "slow-task");
    key(&mut rt, Key::Enter, false);
    pump(&mut rt, &queue, 4.0);
    println!("after send: status={:?} interrupt={} draft={:?}", rt.state.composer_status(), rt.state.can_interrupt(), rt.state.draft.text);

    // While it works: Enter queues, Ctrl+Enter steers (OpenCode 1 queues by default).
    typed(&mut rt, "queued while working");
    key(&mut rt, Key::Enter, false);
    pump(&mut rt, &queue, 2.0);
    println!("queued items: {:?}", rt.state.queue.of(&id).iter().map(|i| i.text.clone()).collect::<Vec<_>>());
    typed(&mut rt, "steer this one in");
    key(&mut rt, Key::Enter, true);
    pump(&mut rt, &queue, 4.0);
    let held = messages(&id);
    println!("steered text on the server: {}", held.contains("steer this one in"));

    // An attached file and an `@` session reference go with the next prompt.
    let dir = std::env::temp_dir().join("fleet-native-live-composer");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("notes.txt");
    std::fs::write(&file, "one\ntwo\n").unwrap();
    rt.update(move |s, tx| s.add_files(&[file], tx));
    pump(&mut rt, &queue, 1.0);
    typed(&mut rt, "@Fix");
    pump(&mut rt, &queue, 2.0);
    key(&mut rt, Key::Enter, false);
    typed(&mut rt, "look at both");
    println!("draft before send: {:?}", rt.state.draft.text);
    pump(&mut rt, &queue, 40.0);
    println!("draft after the wait: {:?} attached={}", rt.state.draft.text, rt.state.held_files().len());
    key(&mut rt, Key::Enter, false);
    pump(&mut rt, &queue, 6.0);
    let all = messages(&id);
    // A small text file goes in the prompt itself (11.4); a bigger or binary one is uploaded and named in a note.
    println!("small file inline in the prompt: {}", all.contains("notes.txt:"));
    println!("session references block: {}", all.contains("fleet-session-references"));

    // `!` runs in the folder; `/btw` shows the toast until the side panel exists.
    typed(&mut rt, "!ls");
    key(&mut rt, Key::Enter, false);
    pump(&mut rt, &queue, 3.0);
    println!("shell ran: {}", messages(&id).contains("README.md"));
    typed(&mut rt, "/btw what is this");
    key(&mut rt, Key::Enter, false);
    println!("draft kept after /btw: {:?}", rt.state.draft.text);
}

/// The running-work counter's data against a scratch Fleet, headless: the label and the groups, as the status bar's popover
/// will list them. Send `background-work` to two sessions (each runs a sub-agent for about 30 s) after it starts.
///
///     FLEET_LIVE_URL=http://127.0.0.1:6481 cargo test live_running_work -- --ignored --nocapture
#[test]
#[ignore = "needs a scratch Fleet"]
fn live_running_work() {
    let Ok(url) = std::env::var("FLEET_LIVE_URL") else { return };
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    let post = Post::new(move |job| {
        let _ = jobs.lock().unwrap().send(job);
    });
    let mut rt = app();
    let link = Link::start(url, post);
    rt.update(move |s, _| s.link = Some(link));
    let (start, mut last) = (Instant::now(), String::new());
    while start.elapsed() < Duration::from_secs(60) {
        while let Ok(job) = queue.try_recv() {
            rt.update(job);
        }
        rt.frame(1.0 / 60.0);
        let s = &rt.state;
        let groups: Vec<String> = s
            .placed_work_groups()
            .iter()
            .map(|g| {
                let items: Vec<String> =
                    g.group.items.iter().map(|i| format!("{} {:?}", i.kind.label(), i.label.as_deref().unwrap_or(&i.title))).collect();
                format!("[{} · {} | {}]", g.title, g.place.as_deref().unwrap_or("-"), items.join(", "))
            })
            .collect();
        let now =
            if s.list_work.is_empty() { "nothing running".to_string() } else { format!("{} {}", s.list_work.label(), groups.join(" ")) };
        if now != last {
            println!("[{:>5.1}s] {now}", start.elapsed().as_secs_f32());
            last = now;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}
