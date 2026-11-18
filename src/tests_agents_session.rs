//! The Agents tab reading a row's own session (component 10, the sweep): what its detail shows from the child's
//! messages (`AgentDetail.vue`'s activity, rows 5.29, 5.30, 5.32-5.34), and the parent's title read when the list lacks it
//! (`AgentsCanvas.test.ts`, "fetches the parent's title when the list doesn't have it, and opens it"). A fake Fleet
//! answers `GET /api/sessions/{id}/messages` and `GET /api/sessions/{id}` and records what it was asked.
use crate::fleet::link::{Job, Link, Post};
use crate::mock::agents_tab::Case;
use crate::tests::{FRAME, app, click};
use crate::tests_machines_page::{exists, words};
use crate::{State, test_wait};
use rune::Runtime;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};

type Asked = Arc<Mutex<Vec<String>>>;

const MESSAGES: &str = r#"{"messages":[
  {"id":"m1","role":"user","parts":[{"type":"text","text":"Move the cache\nand tidy up"}]},
  {"id":"m2","role":"assistant","modelId":"fake-model","parts":[{"type":"tool","toolCallId":"c1","toolName":"read","state":2},{"type":"tool","toolCallId":"c2","toolName":"grep","state":2}]},
  {"id":"m3","role":"assistant","modelId":"fake-model","parts":[{"type":"text","text":"Checked the cache\nAll 12 files moved"},{"type":"step-finish","tokensInput":38200,"tokensOutput":1900}]}
],"pagination":{"hasMore":false}}"#;

fn serve(asked: Asked) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a port");
    let base = format!("http://{}", listener.local_addr().expect("an address"));
    std::thread::spawn(move || {
        for mut stream in listener.incoming().flatten() {
            let asked = asked.clone();
            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                let n = stream.read(&mut buf).unwrap_or(0);
                let head = String::from_utf8_lossy(&buf[..n]).to_string();
                let path = head.lines().next().unwrap_or("").split(' ').nth(1).unwrap_or("").to_string();
                let reply = if path.contains("/messages") {
                    MESSAGES.to_string()
                } else if path.starts_with("/api/sessions/gone-parent") {
                    r#"{"title":"The session that was archived"}"#.to_string()
                } else {
                    "{}".to_string()
                };
                asked.lock().unwrap().push(path);
                let out = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{reply}",
                    reply.len()
                );
                let _ = stream.write_all(out.as_bytes());
            });
        }
    });
    base
}

struct Rig {
    rt: Runtime<State>,
    queue: Receiver<Job>,
    asked: Asked,
}

fn rig(case: Case, prepare: impl FnOnce(&mut State)) -> Rig {
    let asked: Asked = Default::default();
    let base = serve(asked.clone());
    let (jobs, queue) = channel::<Job>();
    let jobs = Mutex::new(jobs);
    let post = Post::new(move |job| {
        let _ = jobs.lock().unwrap().send(job);
    });
    let mut rt = app();
    let link = Link::start(base, post);
    rt.update(move |s, _| {
        s.link = Some(link);
        s.show_agents_tab(case, true);
        prepare(s);
    });
    rt.frame(FRAME);
    Rig { rt, queue, asked }
}

impl Rig {
    fn wait(&mut self, what: &str, done: impl FnMut(&State) -> bool) {
        let asked = self.asked.clone();
        test_wait::drive(&mut self.rt, &self.queue, what, done, move |_| format!("asked {:?}", asked.lock().unwrap()));
    }

    fn heard(&self, part: &str) -> usize {
        self.asked.lock().unwrap().iter().filter(|p| p.contains(part)).count()
    }
}

#[test]
fn a_forks_detail_shows_what_its_own_session_did() {
    let mut rig = rig(Case::Main, |_| {});
    let fork = rig.rt.state.agents_lineage(&rig.rt.state.canvas_session()).running.iter().find(|r| r.work.is_none()).cloned().unwrap();
    let child = fork.session_id.clone().unwrap();
    click(&mut rig.rt, &format!("agents-row-{}", fork.key));
    assert!(exists(&rig.rt, "agents-reading"), "the detail says it is reading until the session has answered");
    rig.wait("the fork's messages", |s| s.agents_detail(&s.canvas_session(), &fork).latest.is_some());
    let detail = rig.rt.state.agents_detail(&rig.rt.state.canvas_session(), &fork);
    assert_eq!(detail.asked.as_deref(), Some("Move the cache"));
    assert_eq!(detail.tools.as_deref(), Some("2 calls · Read, Grep"));
    assert_eq!(detail.tokens.as_deref(), Some("38.2k in · 1.9k out"));
    assert_eq!(detail.latest.as_deref(), Some("All 12 files moved"));
    assert!(!detail.reading);
    rig.rt.frame(FRAME);
    rig.rt.frame(FRAME);
    assert!(!exists(&rig.rt, "agents-reading") && exists(&rig.rt, "agents-latest"));
    assert_eq!(rig.heard(&format!("/api/sessions/{child}/messages")), 1, "read once, not on every frame");
    let seen = rig.rt.state.agents_runs_on(&rig.rt.state.canvas_session(), &fork).unwrap_or_default();
    assert!(seen.contains("fake-model") || seen.contains("Fake"), "the model its session answered with names the row: {seen}");
}

#[test]
fn fetches_the_parents_title_when_the_list_doesnt_have_it() {
    let mut rig = rig(Case::Parent, |s| {
        let at = s.open_at();
        s.projects[at.0].sessions[at.1].listing.forked_from = Some("gone-parent".into());
        // The scene's own frame already read the history for the parent it had; the tab reads again for this one.
        s.agents_tab.loaded_for = None;
    });
    rig.wait("the parent's title", |s| s.parent_title("gone-parent").is_some());
    rig.rt.frame(FRAME);
    rig.rt.frame(FRAME);
    assert_eq!(rig.heard("/api/sessions/gone-parent"), 1);
    assert!(words(&rig.rt, "agents-row-parent:gone-parent").contains("The session that was archived"));
}
