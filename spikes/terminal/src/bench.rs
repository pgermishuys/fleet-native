//! `--bench`: how fast the terminal state takes bytes, and what a frame costs, with no window.
//! Wall time on one thread of whatever machine runs it; the frame numbers leave out the GPU.
use std::time::Instant;

use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::vte::ansi::{Processor, StdSyncHandler};
use rune::Env;

use crate::engine::{Listener, Size};
use crate::screen::{LINE, cell_w};

fn repeat(pattern: &[u8], total: usize) -> Vec<u8> {
    pattern.iter().copied().cycle().take(total).collect()
}

fn mb_per_s(bytes: usize, start: Instant) -> f64 {
    bytes as f64 / start.elapsed().as_secs_f64() / 1e6
}

fn parse_speed() {
    let mut mixed = Vec::new();
    for s in crate::STREAMS {
        mixed.extend_from_slice(s.1);
    }
    let lines: Vec<u8> = (0..1000).flat_map(|i| format!("{i:>6} y y y y y y y y y y y y lantern wick oil\r\n").into_bytes()).collect();
    let total = 64 << 20;
    println!("parse, 200x60, 5000 lines of scrollback, 64 KiB chunks:");
    for (name, data) in [("recorded streams", repeat(&mixed, total)), ("plain lines", repeat(&lines, total))] {
        let size = Size { cols: 200, lines: 60 };
        let mut term = Term::new(Config { scrolling_history: 5000, ..Config::default() }, &size, Listener(Default::default()));
        let mut parser: Processor<StdSyncHandler> = Processor::new();
        let start = Instant::now();
        for chunk in data.chunks(65536) {
            parser.advance(&mut term, chunk);
        }
        let a = mb_per_s(data.len(), start);
        let mut vt = vt100::Parser::new(60.try_into().unwrap(), 200.try_into().unwrap(), 5000);
        let start = Instant::now();
        for chunk in data.chunks(65536) {
            vt.process(chunk);
        }
        println!("  {name:<17} alacritty_terminal {a:>6.0} MB/s   vt100 {:>6.0} MB/s", mb_per_s(data.len(), start));
    }
}

fn percentiles(mut us: Vec<u128>) -> String {
    us.sort_unstable();
    format!("p50 {:>6} µs  p90 {:>6} µs", us[us.len() / 2], us[us.len() * 9 / 10])
}

fn frames() {
    let (cols, lines) = (200usize, 60usize);
    let size = (cols as f32 * cell_w() + 20.0, lines as f32 * LINE + 40.0);
    let mut rt = rune::App::new(crate::spec()).headless_runtime(size, Env::new(true, 1.0));
    rt.update(|s, _| {
        s.engine.resize(Size { cols, lines }, (cell_w(), LINE));
        s.fixed_size();
    });
    let line = |i: usize| format!("\r\n\x1b[32m{i:>6}\x1b[0m \x1b[1;34mlantern\x1b[0m {} \x1b[43;30m wick \x1b[0m", "oil glow ".repeat(18));
    let mut n = 0;
    rt.frame(0.0);
    rt.frame(1.0 / 60.0);
    println!("a frame, {cols}x{lines}, headless (layout, scene and paint lists; no GPU):");
    for (name, rows) in [("every row changes", lines), ("one row changes", 1), ("nothing changes", 0)] {
        let (mut read, mut frame) = (Vec::new(), Vec::new());
        for _ in 0..240 {
            let bytes: String = match rows {
                0 => String::new(),
                1 => "x".into(),
                _ => (0..lines)
                    .map(|_| {
                        line({
                            n += 1;
                            n
                        })
                    })
                    .collect(),
            };
            rt.update(|s, _| {
                if !bytes.is_empty() {
                    s.engine.feed_now(bytes.as_bytes());
                }
                s.refresh();
            });
            read.push(rt.state.screen.read_us);
            let start = Instant::now();
            if rows == 0 {
                rt.invalidate();
            }
            rt.frame(1.0 / 60.0);
            frame.push(start.elapsed().as_micros());
        }
        println!("  {name:<18} read rows {}   frame {}", percentiles(read), percentiles(frame));
    }
}

pub fn run() {
    parse_speed();
    frames();
}
