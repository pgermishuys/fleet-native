//! Spike: a terminal drawn with Rune's `grid`, its state kept by alacritty_terminal. Ports the
//! look of Fleet's `TerminalView.vue` (xterm.js) without the drawer around it.
//!
//!   terminal-spike                  a local shell (the spike's stand-in for Fleet's socket)
//!   terminal-spike --fleet http://127.0.0.1:6391   a scratch Fleet's setup terminal, over its socket
//!   terminal-spike --flood 200      200 MB of output parsed on a thread while the UI draws
//!   terminal-spike --scene vim      a recorded stream (see `--list-scenes`), as `--snap` uses
//!   terminal-spike --bench          parse and draw numbers, no window
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
mod bench;
mod engine;
mod fleet;
mod keys;
mod screen;

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use alacritty_terminal::grid::Scroll;
use alacritty_terminal::index::{Column, Line, Point, Side};
use alacritty_terminal::selection::{Selection, SelectionType};
use alacritty_terminal::term::TermMode;
use alacritty_terminal::vte::ansi::CursorShape;
use engine::{Engine, Size};
use rune::{AppSpec, Drag, El, Ime, KeyEvent, Phase, Tx, WheelEvent, col, el, grid, row, text};
use screen::{LINE, Palette, Screen, cell_w, style};

/// Recorded at 100 x 30 by `streams/record.sh`, from an invented project.
pub const STREAMS: &[(&str, &[u8])] = &[
    ("ls", include_bytes!("../streams/ls.bin")),
    ("git-log", include_bytes!("../streams/git-log.bin")),
    ("vim", include_bytes!("../streams/vim.bin")),
    ("less", include_bytes!("../streams/less.bin")),
    ("unicode", include_bytes!("../streams/unicode.bin")),
];

/// `.terminal-view`'s padding: 6px 4px 4px 12px.
const PAD: [f32; 4] = [6.0, 4.0, 4.0, 12.0];

static FRAMES: AtomicU64 = AtomicU64::new(0);

pub struct State {
    pub engine: Engine,
    pub screen: Screen,
    dark: bool,
    pal: Palette,
    /// A replay keeps the size it was recorded at; a shell follows the window.
    fixed: bool,
    source: &'static str,
    /// Wheel travel not yet a whole line.
    wheel: f32,
    /// For the status line, once a second: (when, bytes, frames) last time, and the rates.
    tick: (f64, u64, u64),
    rate: (f64, f64),
    started: Option<Instant>,
}

impl Default for State {
    fn default() -> Self {
        State {
            engine: Engine::new(Size { cols: 100, lines: 30 }),
            screen: Screen::default(),
            dark: true,
            pal: Palette::new(true),
            fixed: false,
            source: "shell",
            wheel: 0.0,
            tick: (0.0, 0, 0),
            rate: (0.0, 0.0),
            started: None,
        }
    }
}

impl State {
    fn replay(&mut self, name: &'static str) {
        let bytes = STREAMS.iter().find(|s| s.0 == name).expect("a stream").1;
        self.engine.feed_now(bytes);
        (self.fixed, self.source) = (true, name);
    }

    pub fn fixed_size(&mut self) {
        self.fixed = true;
    }

    /// Reads the grid if it changed. True when there is something new to draw.
    pub fn refresh(&mut self) -> bool {
        if !self.engine.shared.dirty.swap(false, Ordering::AcqRel) {
            return false;
        }
        self.screen.read(&self.engine.term.lock(), &self.pal, self.dark);
        true
    }

    fn write(&mut self, bytes: Vec<u8>) {
        let mut term = self.engine.term.lock();
        term.selection = None;
        term.scroll_display(Scroll::Bottom);
        drop(term);
        self.engine.write(bytes);
        self.engine.shared.wake();
    }

    /// The box's laid-out size to columns and rows, as xterm's fit addon does.
    fn fit(&mut self, [_, _, w, h]: [f32; 4]) {
        let cols = ((w - PAD[1] - PAD[3]) / cell_w()).floor().max(2.0) as usize;
        let lines = ((h - PAD[0] - PAD[2]) / LINE).floor().max(1.0) as usize;
        if !self.fixed && (cols, lines) != (self.engine.size.cols, self.engine.size.lines) {
            self.engine.resize(Size { cols, lines }, (cell_w(), LINE));
            self.engine.shared.wake();
        }
    }

    fn point(&self, x: f32, y: f32) -> (Point, Side) {
        let col = (x / cell_w()).clamp(0.0, self.engine.size.cols as f32 - 1.0);
        let line = (y / LINE).clamp(0.0, self.engine.size.lines as f32 - 1.0) as i32 - self.screen.offset as i32;
        (Point::new(Line(line), Column(col as usize)), if col.fract() < 0.5 { Side::Left } else { Side::Right })
    }
}

fn on_key(s: &mut State, ev: &KeyEvent, tx: &mut Tx) -> bool {
    // Fleet's terminal-keys.ts: ⌘C/⌘V on a Mac, Ctrl+Shift+C/V elsewhere; the shell gets the rest.
    let (copy, paste) = if cfg!(target_os = "macos") {
        (ev.is_command('c') && !ev.mods.ctrl, ev.is_command('v') && !ev.mods.ctrl)
    } else {
        let chord = ev.mods.ctrl && ev.mods.shift && !ev.mods.alt;
        (chord && ev.is_command('c'), chord && ev.is_command('v'))
    };
    if copy {
        if let Some(text) = s.engine.term.lock().selection_to_string() {
            tx.copy(text);
        }
    } else if paste {
        tx.request_paste();
    } else if let Some(bytes) = keys::encode(ev, s.screen.mode.contains(TermMode::APP_CURSOR)) {
        s.write(bytes);
    } else {
        return false;
    }
    true
}

fn on_paste(s: &mut State, text: &str, _: &mut Tx) {
    // Bracketed paste, so a shell doesn't run a pasted line; ESC removed so a paste can't end it.
    let text = text.replace('\x1b', "").replace("\r\n", "\r").replace('\n', "\r");
    let bytes = if s.screen.mode.contains(TermMode::BRACKETED_PASTE) { format!("\x1b[200~{text}\x1b[201~") } else { text };
    s.write(bytes.into_bytes());
}

fn on_ime(s: &mut State, ime: &Ime, _: &mut Tx) -> bool {
    if let Ime::Commit(text) = ime {
        s.write(text.as_bytes().to_vec());
    }
    true
}

fn on_wheel(s: &mut State, e: WheelEvent, _: &mut Tx) -> bool {
    s.wheel += e.dy / LINE;
    let lines = s.wheel.trunc() as i32;
    s.wheel -= lines as f32;
    if lines == 0 {
        return true;
    }
    if s.screen.mode.contains(TermMode::ALT_SCREEN | TermMode::ALTERNATE_SCROLL) {
        // A full-screen program gets arrow keys for the wheel, as xterm sends them.
        let arrow = if lines > 0 { "A" } else { "B" };
        s.engine.write(format!("\x1bO{arrow}").repeat(lines.unsigned_abs() as usize).into_bytes());
    } else {
        s.engine.term.lock().scroll_display(Scroll::Delta(lines));
        s.engine.shared.wake();
    }
    true
}

fn on_drag(s: &mut State, d: &mut Drag, _: &mut Tx) {
    let (point, side) = s.point(d.x, d.y);
    let mut term = s.engine.term.lock();
    match d.phase {
        Phase::Down => term.selection = Some(Selection::new(SelectionType::Simple, point, side)),
        Phase::Move | Phase::Up => {
            if let Some(sel) = term.selection.as_mut() {
                sel.update(point, side);
            }
        }
    }
    drop(term);
    s.engine.shared.wake();
}

fn poll(s: &mut State, now: f64, tx: &mut Tx) -> bool {
    let changed = s.refresh();
    let ticked = now - s.tick.0 >= 1.0;
    if ticked {
        let (bytes, frames) = (s.engine.shared.bytes.load(Ordering::Relaxed), FRAMES.load(Ordering::Relaxed));
        let dt = now - s.tick.0;
        s.rate = ((bytes - s.tick.1) as f64 / dt / 1e6, (frames - s.tick.2) as f64 / dt);
        s.tick = (now, bytes, frames);
    }
    tx.wake_after(Duration::from_secs(1));
    changed || ticked
}

fn setup(s: &mut State, waker: rune::Waker) {
    s.engine.set_waker(waker);
    let args: Vec<String> = std::env::args().collect();
    let has = |a: &str| args.iter().any(|x| x == a);
    if has("--scene") || has("--snap") || has("--script") || has("--bench") {
        return;
    }
    if let Some(at) = args.iter().position(|a| a == "--flood") {
        let mb: u64 = args.get(at + 1).and_then(|v| v.parse().ok()).unwrap_or(200);
        s.engine.spawn_feeder(engine::flood(mb << 20));
        s.source = "flood";
        s.started = Some(Instant::now());
    } else if let Some(at) = args.iter().position(|a| a == "--fleet") {
        s.source = "fleet";
        if let Err(e) = fleet::attach(&s.engine, args.get(at + 1).map_or("", |v| v.as_str())) {
            eprintln!("no Fleet terminal: {e}");
        }
    } else if let Err(e) = s.engine.spawn_shell((cell_w(), LINE)) {
        eprintln!("no shell: {e}");
    }
}

/// The cursor xterm.js draws: a block over the cell (its character in the background colour),
/// a bar or an underline. It carries the input method's caret.
fn cursor(s: &State) -> Option<El> {
    let c = s.screen.cursor.as_ref()?;
    let (cw, w) = (cell_w(), cell_w() * if c.wide { 2.0 } else { 1.0 });
    let base = el().absolute(PAD[3] + c.col as f32 * cw, PAD[0] + c.line as f32 * LINE).ime_cursor();
    Some(match c.shape {
        CursorShape::Beam => base.w(1.0).h(LINE).fill(s.pal.fg),
        CursorShape::Underline => base.w(w).h(1.0).mt(LINE - 1.0).fill(s.pal.fg),
        CursorShape::HollowBlock => base.w(w).h(LINE).border(1.0, s.pal.fg),
        _ => base.w(w).h(LINE).fill(s.pal.fg).child(text(c.ch.to_string()).font(style()).color(s.pal.bg)),
    })
}

fn status(s: &State) -> String {
    let size = format!("{}x{}", s.engine.size.cols, s.engine.size.lines);
    let mut line = format!(
        "{}  {size}  scrollback {}  {:.1} MB in  {:.1} MB/s  {:.0} frames/s  last read {} rows in {} µs",
        s.source,
        s.screen.history,
        s.engine.shared.bytes.load(Ordering::Relaxed) as f64 / 1e6,
        s.rate.0,
        s.rate.1,
        s.screen.built,
        s.screen.read_us,
    );
    if let (Some(t), true) = (s.started, s.source == "flood") {
        line += &format!("  {:.1} s", t.elapsed().as_secs_f32());
    }
    if s.source == "fleet" {
        line += &format!("  last control: {}", s.engine.shared.title.lock().unwrap());
    }
    if s.engine.shared.exited.load(Ordering::Relaxed) {
        line += "  [exited]";
    }
    line
}

fn view(s: &State, _: &mut rune::Cx<State>) -> El {
    let (cols, lines) = (s.engine.size.cols, s.engine.size.lines);
    let mut screen = grid(cols as u16, cell_w(), LINE, s.screen.rows.clone()).w(cols as f32 * cell_w()).h(lines as f32 * LINE);
    if let Some(c) = &s.screen.cursor {
        screen = screen.grid_caret(c.col as u16, c.line as u16);
    }
    let mut term = el()
        .key("terminal")
        .grow(1.0)
        .min_h(0.0)
        .w_full()
        .pt(PAD[0])
        .pr(PAD[1])
        .pb(PAD[2])
        .pl(PAD[3])
        .fill(s.pal.bg)
        .clip_rounded(0.0)
        .on_layout(State::fit)
        .on_wheel(on_wheel)
        .on_drag(on_drag)
        .cursor(rune::Cursor::Text)
        .child(screen);
    if let Some(c) = cursor(s) {
        term = term.child(c);
    }
    let bar = row()
        .w_full()
        .h(22.0)
        .px(12.0)
        .center_y()
        .fill(s.pal.bar)
        .child(text(status(s)).font(rune::tok::TextStyle::new(11.0, 400.0).mono()).color(s.pal.fg.alpha(0.7)).single_line());
    col().w_full().h_full().fill(s.pal.bg).on_paste(on_paste).child(term).child(bar)
}

fn light(s: &mut State) {
    (s.dark, s.pal) = (false, Palette::new(false));
}

pub fn spec() -> AppSpec<State> {
    AppSpec {
        scenes: &[
            ("ls", |s| s.replay("ls")),
            ("git-log", |s| s.replay("git-log")),
            ("vim", |s| s.replay("vim")),
            ("less", |s| s.replay("less")),
            ("unicode", |s| s.replay("unicode")),
            ("vim-light", |s| {
                light(s);
                s.replay("vim");
            }),
            ("git-log-light", |s| {
                light(s);
                s.replay("git-log");
            }),
        ],
        fonts: &[
            include_bytes!("../../../assets/fonts/InterVariable.ttf"),
            include_bytes!("../../../assets/fonts/JetBrainsMono-Variable.ttf"),
        ],
        families: ("Inter", "JetBrains Mono"),
        setup: Some(setup),
        poll: Some(poll),
        on_key: Some(on_key),
        ..AppSpec::new("Terminal spike", (900.0, 560.0), State::default, view)
    }
}

fn main() {
    if std::env::args().any(|a| a == "--bench") {
        return bench::run();
    }
    rune::App::new(spec())
        .on_ime(on_ime)
        .on_frame_drawn(|_| {
            FRAMES.fetch_add(1, Ordering::Relaxed);
        })
        .run();
}

#[cfg(test)]
mod tests {
    use alacritty_terminal::index::{Column, Line};

    #[test]
    fn every_scene_reads_its_rows() {
        for (name, scene) in super::spec().scenes {
            let mut rt = rune::App::new(super::spec()).headless_runtime((760.0, 480.0), rune::Env::new(true, 1.0));
            rt.update(|s, _| scene(s));
            rt.frame(0.0);
            rt.frame(1.0 / 60.0);
            assert_eq!(rt.state.screen.rows.len(), 30, "{name}: a row for each line");
        }
    }

    #[test]
    fn a_replay_lands_in_the_grid() {
        let mut s = super::State::default();
        s.replay("ls");
        let term = s.engine.term.lock();
        let row: String = (0..12).map(|c| term.grid()[Line(0)][Column(c)].c).collect();
        assert_eq!(row, "total 20K   ");
    }
}
