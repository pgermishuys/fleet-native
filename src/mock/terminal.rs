//! A pretend shell for the app with no server (component 13, piece C; row 13.42, decision 5). Ports
//! `lib/terminal-mock-shell.ts`: it knows a few commands (`ls`, `pwd`, `git status`, `clear`, `echo`, a test run, a dev server
//! that runs until Ctrl+C, `exit`), echoes typing, and remembers what it printed so opening it again replays it. The commands and
//! their output are invented: an imaginary project, "lantern".
use std::collections::HashMap;

const ESC: &str = "\x1b";
const CTRL_C: char = '\x03';
const CTRL_L: char = '\x0c';
const DELETE: char = '\x7f';

fn sgr(code: &str, text: &str) -> String {
    format!("{ESC}[{code}m{text}{ESC}[0m")
}

fn prompt() -> String {
    format!("{} {} {} ", sgr("1;34", "lantern"), sgr("35", "main"), sgr("32", "❯"))
}

fn status() -> Vec<String> {
    vec![
        format!(" {} src/wick.rs", sgr("33", "M")),
        format!(" {} src/glow/mod.rs", sgr("33", "M")),
        format!("{} src/glow/dim.rs", sgr("31", "??")),
    ]
}

fn test_run() -> Vec<String> {
    let dim = |t: &str| sgr("2", t);
    vec![
        String::new(),
        format!("{} {}", sgr("1;36", "running 12 tests"), dim("in lantern")),
        format!("test wick::lights_on_first_try ... {}", sgr("32", "ok")),
        format!("test glow::dims_by_steps ... {}", sgr("32", "ok")),
        format!("test glow::never_goes_below_zero ... {}", sgr("1;31", "FAILED")),
        String::new(),
        sgr("31", "assertion `left == right` failed: left: -1, right: 0"),
        format!("  {} {}", sgr("36", "at"), dim("src/glow/dim.rs:41")),
        String::new(),
        format!("test result: {}. 11 passed; {}; finished in 0.04s", sgr("1;31", "FAILED"), sgr("31", "1 failed")),
    ]
}

fn dev_server() -> Vec<String> {
    let dim = |t: &str| sgr("2", t);
    vec![
        String::new(),
        dim("> lantern@0.3.0 dev"),
        dim("> serve --port 4100"),
        String::new(),
        format!("  {}  {} {}", sgr("1;32", "READY"), dim("listening on"), sgr("36", "http://localhost:4100/")),
        format!("  {}  {}", sgr("32", "➜"), dim("press Ctrl+C to stop")),
    ]
}

/// One pretend terminal: what it printed so far, what is typed on its line, and whether it runs something.
#[derive(Debug, Clone)]
pub struct Shell {
    input: String,
    running: bool,
    transcript: Vec<u8>,
    exit: Option<i32>,
}

impl Shell {
    /// The first terminal of a session starts with some output so the drawer isn't empty (`greeting`: ids ending "-1").
    pub fn new(id: &str) -> Shell {
        let mut shell = Shell { input: String::new(), running: false, transcript: Vec::new(), exit: None };
        if id.ends_with("-1") {
            shell.emit(&format!("{}git status --short\r\n", prompt()));
            for line in status() {
                shell.emit(&format!("{line}\r\n"));
            }
        }
        shell.emit(&prompt());
        shell
    }

    /// Everything it printed, for a view that connects again.
    pub fn replay(&self) -> &[u8] {
        &self.transcript
    }

    /// How the shell ended, once `exit` was typed.
    pub fn exit_code(&self) -> Option<i32> {
        self.exit
    }

    fn emit(&mut self, text: &str) -> String {
        self.transcript.extend_from_slice(text.as_bytes());
        text.to_string()
    }

    /// The server's "clear": the scrollback is cut down to the prompt and what is typed.
    pub fn clear(&mut self) {
        self.transcript = format!("{}{}", prompt(), self.input).into_bytes();
    }

    /// What typing prints. Bytes that aren't text are skipped; after `exit` nothing does anything.
    pub fn write(&mut self, data: &[u8]) -> Vec<u8> {
        let mut out = String::new();
        for ch in String::from_utf8_lossy(data).chars() {
            if self.exit.is_some() {
                break;
            }
            match ch {
                CTRL_C => {
                    self.input.clear();
                    self.running = false;
                    out += &self.emit(&format!("^C\r\n{}", prompt()));
                }
                _ if self.running => {}
                '\r' => {
                    out += &self.emit("\r\n");
                    let command = std::mem::take(&mut self.input);
                    out += &self.run(&command);
                    if self.exit.is_none() && !self.running {
                        out += &self.emit(&prompt());
                    }
                }
                DELETE => {
                    if self.input.pop().is_some() {
                        out += &self.emit("\x08 \x08");
                    }
                }
                CTRL_L => out += &self.emit(&format!("{ESC}[2J{ESC}[H{}{}", prompt(), self.input)),
                c if c >= ' ' => {
                    self.input.push(c);
                    out += &self.emit(&c.to_string());
                }
                _ => {}
            }
        }
        out.into_bytes()
    }

    fn lines(&mut self, lines: Vec<String>) -> String {
        lines.iter().map(|l| self.emit(&format!("{l}\r\n"))).collect()
    }

    fn run(&mut self, command: &str) -> String {
        let text = command.trim();
        if text.is_empty() {
            return String::new();
        }
        let first = text.split_whitespace().next().unwrap_or("");
        if text == "clear" {
            return self.emit(&format!("{ESC}[2J{ESC}[H"));
        }
        if text == "ls" {
            return self.lines(vec![format!(
                "{}  Cargo.toml  README.md  {}  {}",
                sgr("1;34", "docs"),
                sgr("1;34", "src"),
                sgr("1;34", "tests")
            )]);
        }
        if text == "pwd" {
            return self.lines(vec!["/home/dev/lantern".to_string()]);
        }
        if text.starts_with("git status") {
            return self.lines(status());
        }
        if text.starts_with("cargo test") || text.starts_with("npm test") || text.starts_with("npm run test") {
            return self.lines(test_run());
        }
        if text.starts_with("npm run dev") || text.starts_with("cargo run") {
            self.running = true;
            return self.lines(dev_server());
        }
        if text == "exit" {
            self.exit = Some(0);
            return String::new();
        }
        if text.starts_with("echo") {
            return self.lines(vec![text.get(5..).unwrap_or("").to_string()]);
        }
        self.lines(vec![format!("zsh: command not found: {first}")])
    }
}

/// Every pretend terminal there has been, by terminal id, so reopening one replays its output.
#[derive(Debug, Default)]
pub struct Shells(HashMap<String, Shell>);

impl Shells {
    pub fn open(&mut self, id: &str) -> &mut Shell {
        self.0.entry(id.to_string()).or_insert_with(|| Shell::new(id))
    }
}
