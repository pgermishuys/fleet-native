//! `fleet-native --bench-conversation`: times the long conversation with no window and no GPU.
//!
//! A headless runtime runs the same frames a window would (poll, view, layout, scroll), and each
//! `frame` call is timed here. That is the CPU cost of a frame before any drawing, so it means the
//! same on any machine, a laptop or this repository's CI runner. What it leaves out: rasterising,
//! the GPU, and the display. The cases mirror `scripts/bench/*.json`.
use crate::fleet::reducer::Event;
use crate::{State, spec};
use rune::{App, Env, Runtime};
use std::time::Instant;

const FRAME: f64 = 1.0 / 60.0;
const SIZE: (f32, f32) = (1280.0, 760.0);

fn runtime() -> Runtime<State> {
    let mut rt = App::new(spec()).headless_runtime(SIZE, Env::new(true, 1.0));
    rt.frame(0.0);
    rt.frame(FRAME);
    rt
}

/// One frame, in milliseconds.
fn timed(rt: &mut Runtime<State>, times: &mut Vec<f64>) {
    let start = Instant::now();
    rt.frame(FRAME);
    times.push(start.elapsed().as_secs_f64() * 1000.0);
}

/// A trackpad swipe of `dy` points over `seconds`, eased out as Rune's script `scroll` is. Positive
/// scrolls up, as `Runtime::wheel` does.
fn swipe(rt: &mut Runtime<State>, dy: f32, seconds: f64, times: &mut Vec<f64>) {
    let steps = (seconds / FRAME).round() as usize;
    let mut prev = 0.0;
    for s in 1..=steps {
        let f = s as f32 / steps as f32;
        let at = dy * f * (2.0 - f);
        rt.wheel(640.0, 400.0, at - prev);
        prev = at;
        timed(rt, times);
    }
}

/// The long conversation as the reducer holds it, with the last reply open, standing in for a
/// server's session that is mid-stream. Returns the open session's id.
pub fn long_from_events(rt: &mut Runtime<State>) -> &'static str {
    use crate::fleet::model::{Explicit, Msg, PartData, StreamState, StreamStatus};
    use crate::view::conversation::Role;
    let messages = (0..2_000)
        .map(|n| {
            let mut m = Msg::new(format!("msg_{n:05}"), "bench", if n % 2 == 0 { Role::User } else { Role::Assistant });
            let body = format!("Message {n}.\n\n- one\n- two\n\n```rust\nfn f{n}() {{}}\n```\n").repeat(4);
            m.parts.push(PartData::Text { id: format!("prt_{n:05}"), text: body });
            m
        })
        .collect();
    let state = StreamState {
        messages,
        delegations: Vec::new(),
        explicit_status: Explicit::Busy,
        session_status: StreamStatus::Busy,
        last_event_id: None,
    };
    rt.update(move |s, tx| {
        let at = s.long_session().expect("the long conversation is in the list");
        s.open = at;
        s.projects[at.0].sessions[at.1].id = "bench".into();
        s.on_snapshot("bench", state, tx);
    });
    rt.frame(FRAME);
    rt.frame(FRAME);
    "bench"
}

fn report(name: &str, mut times: Vec<f64>) {
    times.sort_by(f64::total_cmp);
    let n = times.len();
    let pct = |p: f64| times[((n as f64 * p) as usize).min(n - 1)];
    let mean = times.iter().sum::<f64>() / n as f64;
    let over = |ms: f64| times.iter().filter(|t| **t > ms).count();
    println!(
        "| {name} | {n} | {mean:.2} | {:.2} | {:.2} | {:.2} | {:.2} | {} | {} |",
        pct(0.5),
        pct(0.95),
        pct(0.99),
        times[n - 1],
        over(8.0),
        over(16.7)
    );
}

pub fn conversation() {
    println!("| case | frames | mean ms | p50 | p95 | p99 | max | over 8 ms | over 16.7 ms |");
    println!("|---|--:|--:|--:|--:|--:|--:|--:|--:|");

    // Open: fold out the project and click the session. The first frame after the click builds the
    // visible rows of 2,000 messages for the first time.
    let mut rt = runtime();
    let mut times = Vec::new();
    for key in ["project-3", "session-3-0"] {
        let (x, y) = rt.find(key).expect("the long conversation is in the list");
        rt.pointer_move(x, y);
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        timed(&mut rt, &mut times);
    }
    for _ in 0..30 {
        timed(&mut rt, &mut times);
    }
    report("open (click, then 0.5 s)", times);

    // Stream: the reply arrives at 280 characters a second, the view stuck at the end; 18 seconds is
    // two replies.
    let mut rt = runtime();
    rt.update(|s, _| s.open_long_live());
    let mut times = Vec::new();
    for _ in 0..(18.0 / FRAME) as usize {
        timed(&mut rt, &mut times);
    }
    assert!(rt.state.session().messages.len() > 2_002, "the stream never started");
    report("stream (18 s, at the end)", times);

    // Scroll up through the last messages and back, while the reply streams.
    let mut rt = runtime();
    rt.update(|s, _| s.open_long_live());
    let mut times = Vec::new();
    for _ in 0..10 {
        swipe(&mut rt, 1800.0, 1.0, &mut times);
    }
    for _ in 0..10 {
        swipe(&mut rt, -1800.0, 1.0, &mut times);
    }
    assert!(rt.find("a999-p0").is_none() || rt.scroll_at_bottom("stream") == Some(true), "the scroll did nothing");
    report("scroll end + stream (20 s)", times);

    // Jump to the first message, then read downwards at about 1,800 points a second, no stream.
    let mut rt = runtime();
    rt.update(|s, _| s.open_long());
    let mut times = Vec::new();
    swipe(&mut rt, 4_000_000.0, 0.05, &mut Vec::new());
    for _ in 0..30 {
        timed(&mut rt, &mut Vec::new());
    }
    assert!(rt.find("u0").is_some() || rt.find("a0-p0").is_some(), "the jump didn't reach the first message");
    for _ in 0..20 {
        swipe(&mut rt, -1800.0, 1.0, &mut times);
    }
    assert!(rt.find("a0-p0").is_none() && rt.scroll_at_bottom("stream") == Some(false), "reading downwards didn't move");
    report("scroll from the top (20 s)", times);

    events("stream from events (18 s, 4 deltas a frame)", true);
    events("stream from events, one 25 KB paragraph", false);
}

/// The same stream fed the way a server feeds it: a frame's batch of word deltas applied to a
/// 2,000-message conversation through the reducer, then drawn. Without `paragraphs` the reply is one
/// paragraph that grows to about 25 KB, which no real reply does: the worst case for text layout.
fn events(name: &str, paragraphs: bool) {
    let mut rt = runtime();
    let id = long_from_events(&mut rt);
    let mut times = Vec::new();
    for frame in 0..(18.0 / FRAME) as usize {
        let batch: Vec<Event> = (0..4)
            .map(|k| Event::TextDelta {
                session_id: "bench".into(),
                message_id: "msg_01999".into(),
                part_id: "prt_01999".into(),
                // A paragraph break every 40 words, as a written reply has.
                delta: if paragraphs && (frame * 4 + k) % 40 == 39 { "word\n\n".into() } else { format!("word{} ", frame * 4 + k) },
                offset: None,
            })
            .collect();
        let start = Instant::now();
        rt.update(move |s, tx| s.on_events(id, batch, tx));
        rt.frame(FRAME);
        times.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    report(name, times);
}
