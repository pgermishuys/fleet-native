//! The server log's copying (`Log::copy`): the server's two pipes are copied on two threads into one file, and each pipe's
//! lines must arrive whole. A pipe can carry a line in pieces: Rust's stdout writes a line longer than its 1 KB buffer as
//! its start, then the rest (Windows' long PATH did this in `the_server_starts_with_the_login_shells_environment`).
use super::log::Log;
use super::tests::Scratch;
use crate::test_wait;
use std::io::Read;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};

/// A pipe the test feeds piece by piece. It counts its reads, so the test knows a piece has been dealt with once the
/// reader asks for the next one.
struct Pipe {
    pieces: Receiver<&'static [u8]>,
    reads: Arc<AtomicUsize>,
}

impl Read for Pipe {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        let Ok(piece) = self.pieces.recv() else { return Ok(0) };
        buf[..piece.len()].copy_from_slice(piece);
        Ok(piece.len())
    }
}

struct Copying {
    feed: Sender<&'static [u8]>,
    reads: Arc<AtomicUsize>,
    thread: std::thread::JoinHandle<()>,
}

impl Copying {
    fn start(log: &Log) -> Copying {
        let (feed, pieces) = channel();
        let reads = Arc::new(AtomicUsize::new(0));
        let pipe = Pipe { pieces, reads: reads.clone() };
        let log = log.clone();
        Copying { feed, reads, thread: std::thread::spawn(move || log.copy(pipe)) }
    }

    /// Gives the reader `piece` and waits until it has done what it does with it.
    fn send(&self, piece: &'static [u8]) {
        let asked = self.reads.load(Ordering::SeqCst);
        self.feed.send(piece).unwrap();
        test_wait::until_true("the piece to be taken", || self.reads.load(Ordering::SeqCst) > asked);
    }

    fn end(self) {
        drop(self.feed);
        self.thread.join().unwrap();
    }
}

fn lines(scratch: &Scratch) -> Vec<String> {
    scratch.log_text().lines().map(String::from).collect()
}

#[test]
fn a_line_that_comes_in_pieces_stays_whole_when_the_other_pipe_writes_between() {
    let scratch = Scratch::new("log-pieces");
    let log = Log::open(&scratch.log(), super::log::LIMIT).unwrap();
    let (stdout, stderr) = (Copying::start(&log), Copying::start(&log));
    test_wait::until_true("both readers to be waiting", || {
        stdout.reads.load(Ordering::SeqCst) == 1 && stderr.reads.load(Ordering::SeqCst) == 1
    });
    stdout.send(b"fake fleet serve args=x\nenv PATH=");
    stderr.send(b"fake fleet stderr line\n");
    stdout.send(b"/login-shell/bin:/usr/bin\ncwd=/data\n");
    stdout.end();
    stderr.end();
    let lines = lines(&scratch);
    assert!(lines.iter().any(|l| l == "env PATH=/login-shell/bin:/usr/bin"), "{lines:#?}");
    assert!(lines.iter().any(|l| l == "fake fleet stderr line"), "{lines:#?}");
    assert_eq!(lines.len(), 4, "{lines:#?}");
}

#[test]
fn a_line_with_no_end_is_written_once_it_is_too_long_to_hold() {
    static PIECE: [u8; 8192] = [b'x'; 8192];
    let scratch = Scratch::new("log-long");
    let log = Log::open(&scratch.log(), super::log::LIMIT).unwrap();
    let stdout = Copying::start(&log);
    for _ in 0..super::log::LONGEST_LINE / PIECE.len() {
        stdout.send(&PIECE);
    }
    assert_eq!(scratch.log_text().len(), super::log::LONGEST_LINE, "written before the pipe ends");
    stdout.end();
}

#[test]
fn what_a_pipe_left_without_a_line_end_is_written_when_it_ends() {
    let scratch = Scratch::new("log-tail");
    let log = Log::open(&scratch.log(), super::log::LIMIT).unwrap();
    let stdout = Copying::start(&log);
    stdout.send(b"one\ntwo, unfinished");
    stdout.end();
    assert_eq!(scratch.log_text(), "one\ntwo, unfinished");
}
