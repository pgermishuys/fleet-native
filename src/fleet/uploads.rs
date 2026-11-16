//! Files, large pastes and terminal lines for the next prompt (docs/parity/05-composer.md, piece O): a file is held until
//! Send, a big paste is offered as a file, and Send uploads what is held, in order, before the prompt goes
//! (`attachDraftFiles`, `handleTextPaste`, `handleOfferKey` of `Composer.vue`, `use-draft-files.ts`). The decisions are
//! `composer::attached`; this file starts the reading and uploading off the UI thread and takes the answers in `poll`.
use super::attach::Done;
use super::http;
use crate::State;
use crate::composer::attached::*;
use crate::composer::status::{Mode, mode_of};
use rune::{Key, KeyEvent, TextEdit, Tx};
use std::time::Duration;

/// Which button of the offer bar Enter takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    Attach,
    Text,
}

/// A big paste waiting for a choice: the text and where the caret was, so Paste as text goes back there (`pasteOffer`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Offer {
    pub owner: String,
    pub text: String,
    pub start: usize,
    pub end: usize,
    pub choice: Choice,
}

/// What a worker thread uploads: one held file.
struct Job {
    file: DraftFile,
}

fn file_error(name: &str, why: impl std::fmt::Display) -> String {
    format!("{name} couldn't be read: {why}")
}

impl State {
    /// The files held for the open session's next prompt.
    pub fn held_files(&self) -> &[DraftFile] {
        match self.draft_owner() {
            Some(owner) => self.held.files(&owner),
            None => &[],
        }
    }

    /// The terminal lines added to the open session's next message.
    pub fn terminal_lines(&self) -> &[TerminalDraft] {
        match self.draft_owner() {
            Some(owner) => self.held.terminals(&owner),
            None => &[],
        }
    }

    /// Pictures, files and terminal lines: whatever besides text makes a prompt worth sending.
    pub fn extras(&self) -> usize {
        self.held_images().len() + self.held_files().len() + self.terminal_lines().len()
    }

    /// Lines from a terminal drawer join the open session's draft (component 13 calls this).
    pub fn add_terminal_lines(&mut self, context: TerminalDraft) {
        if let Some(owner) = self.draft_owner() {
            self.held.add_terminal(&owner, context);
        }
    }

    pub fn remove_terminal_lines(&mut self, id: &str) {
        if let Some(owner) = self.draft_owner() {
            self.held.remove_terminal(&owner, id);
        }
    }

    /// The x on a file chip: not while the files are uploading.
    pub fn remove_file(&mut self, id: &str) {
        if self.uploading {
            return;
        }
        if let Some(owner) = self.draft_owner() {
            self.held.remove_file(&owner, id);
        }
    }

    /// The Large pastes setting (read-only here: Settings is component 16).
    pub fn large_pastes(&self) -> LargePastes {
        LargePastes::from_pref(self.pickers.prefs.get(LARGE_PASTES_KEY))
    }

    // ─── files ──────────────────────────────────────────────────────────────

    /// A file that is no picture, checked and read on a thread: its size against 25 MB and, for a text file, its lines.
    pub(super) fn add_plain_file(&mut self, owner: String, name: String, mime: &'static str, path: std::path::PathBuf, tx: &mut Tx) {
        self.attach.begin_read(&owner);
        let id = self.held.next_id();
        let send = self.reads.send.clone();
        std::thread::spawn(move || {
            let done = match inspect(&name, mime, &path) {
                Ok((size, lines)) => Done::File {
                    owner,
                    file: DraftFile { id, name, mime: mime.into(), size, lines, pasted: false, source: FileSource::Path(path) },
                },
                Err(message) => Done::Refused { owner, message },
            };
            let _ = send.send(done);
        });
        tx.wake_after(Duration::from_millis(30));
    }

    /// A file's answer: the chip joins the strip.
    pub(super) fn on_file_read(&mut self, owner: &str, file: DraftFile) {
        self.attach.end_read(owner);
        self.held.add_file(owner, file);
        self.attach.clear_error();
    }

    /// `attachPastedText`: text the person chose to attach is a file in memory, named `pasted-text.txt` and so on.
    pub fn attach_pasted_text(&mut self, text: &str, tx: &mut Tx) {
        let Some(owner) = self.draft_owner() else { return };
        let name = self.held.pasted_name(&owner);
        let size = text.len() as u64;
        let held = self.held_images().len() + self.held.files(&owner).len() + self.attach.reading(&owner);
        let refused = if held >= crate::conv::image_sniff::MAX_ATTACHMENTS_PER_PROMPT {
            Some(format!("{} files at most per prompt.", crate::conv::image_sniff::MAX_ATTACHMENTS_PER_PROMPT))
        } else if size > MAX_FILE_BYTES {
            Some(too_big(&name, size))
        } else {
            None
        };
        if let Some(message) = refused {
            self.attach.set_error(message, self.clock);
            tx.wake_after(Duration::from_secs_f64(crate::composer::attach::ERROR_SECONDS + 0.1));
            return;
        }
        let lines = (size <= LINE_COUNT_MAX_BYTES && !text.contains('\0')).then(|| count_lines(text));
        let id = self.held.next_id();
        let file = DraftFile { id, name, mime: "text/plain".into(), size, lines, pasted: true, source: FileSource::Text(text.into()) };
        self.held.add_file(&owner, file);
        self.attach.clear_error();
    }

    // ─── a big paste ────────────────────────────────────────────────────────

    /// `handleTextPaste`: a text paste of 2,000 characters or 25 lines is attached or offered as the setting says. False
    /// leaves the paste to the text area.
    pub fn paste_text(&mut self, text: &str, tx: &mut Tx) -> bool {
        let mode = mode_of(&self.draft.text, &self.caps(), self.side_open);
        if self.large_pastes() == LargePastes::Text || !is_big_paste(text) || mode == Mode::Shell || mode == Mode::Side {
            return false;
        }
        let Some(owner) = self.draft_owner() else { return false };
        if self.large_pastes() == LargePastes::Attach {
            self.attach_pasted_text(text, tx);
            return true;
        }
        let (start, end) = self.draft.range();
        self.offer = Some(Offer { owner, text: text.to_string(), start, end, choice: Choice::Attach });
        true
    }

    /// The offer, for the open session's composer.
    pub fn offer_view(&self) -> Option<&Offer> {
        self.offer.as_ref().filter(|o| self.draft_owner().as_deref() == Some(o.owner.as_str()))
    }

    /// `pasteOfferedText`: the text goes in the draft where the caret was.
    fn paste_offered_text(&mut self) {
        let Some(offer) = self.offer_view().cloned() else { return };
        self.offer = None;
        let len = self.draft.text.len();
        let at = |i: usize| (0..=i.min(len)).rev().find(|n| self.draft.text.is_char_boundary(*n)).unwrap_or(0);
        let (start, end) = (at(offer.start), at(offer.end).max(at(offer.start)));
        self.draft.set_selection(start, end);
        self.draft.insert(&offer.text);
    }

    /// `chooseOffer`: a click or Enter. The text area keeps the keyboard.
    pub fn choose_offer(&mut self, choice: Choice, tx: &mut Tx) {
        let Some(offer) = self.offer_view().cloned() else { return };
        match choice {
            Choice::Attach => {
                self.offer = None;
                self.attach_pasted_text(&offer.text, tx);
            }
            Choice::Text => self.paste_offered_text(),
        }
        tx.focus("composer", false);
    }

    /// `handleOfferKey`, ahead of the text area's own: Enter takes the chosen one, Tab flips, Esc pastes as text, a printable
    /// key pastes as text and is then typed, anything else leaves the bar. True when the key is used up.
    pub fn offer_key(&mut self, ev: &KeyEvent, tx: &mut Tx) -> bool {
        let Some(offer) = self.offer_view().cloned() else { return false };
        match &ev.key {
            Key::Enter => self.choose_offer(offer.choice, tx),
            Key::Tab => {
                let choice = if offer.choice == Choice::Attach { Choice::Text } else { Choice::Attach };
                self.offer = Some(Offer { choice, ..offer });
            }
            Key::Escape => self.paste_offered_text(),
            Key::Text(_) if !ev.mods.ctrl && !ev.mods.cmd && !ev.mods.alt => {
                self.paste_offered_text();
                return false;
            }
            _ => return false,
        }
        true
    }

    // ─── Send ───────────────────────────────────────────────────────────────

    /// Send with files held: they go up in order, and the prompt carries on with the note in its text (`attachDraftFiles`).
    /// Without a server the files leave with the words, as pictures do. True when an upload was started.
    pub fn start_upload(&mut self, modifier: bool, tx: &mut Tx) -> bool {
        let Some(owner) = self.draft_owner() else { return false };
        let files = self.held.files(&owner).to_vec();
        let Some(link) = self.open_id().and_then(|id| self.link_for(&id)) else {
            self.held.clear_files(&owner);
            return false;
        };
        let Some(session) = self.open_id() else { return false };
        let (base, send) = (link.base.clone(), self.reads.send.clone());
        self.uploading = true;
        std::thread::spawn(move || {
            let result = upload_all(&base, &session, files.into_iter().map(|file| Job { file }).collect());
            let _ = send.send(Done::Uploaded { owner, modifier, result });
        });
        tx.wake_after(Duration::from_millis(30));
        true
    }

    /// The uploads came back. A failure changes nothing (the draft and its files stay, the error is the server's); success
    /// writes the note into the text and sends on.
    pub(super) fn on_uploaded(
        &mut self,
        owner: &str,
        modifier: bool,
        result: Result<(Vec<String>, Vec<AttachedFile>), String>,
        tx: &mut Tx,
    ) {
        self.uploading = false;
        let (inlined, uploaded) = match result {
            Ok(done) => done,
            Err(message) => {
                self.attach.set_error(message, self.clock);
                tx.wake_after(Duration::from_secs_f64(crate::composer::attach::ERROR_SECONDS + 0.1));
                return;
            }
        };
        // Another session is open now: its draft is not this one's to write into. The files stay held and go again.
        if self.draft_owner().as_deref() != Some(owner) {
            return;
        }
        self.held.clear_files(owner);
        let typed = std::iter::once(self.draft.text.trim_end().to_string()).chain(inlined.iter().cloned()).filter(|p| !p.is_empty());
        let typed = typed.collect::<Vec<_>>().join("\n\n");
        let text = with_attached_note(if inlined.is_empty() { &self.draft.text } else { &typed }, &uploaded);
        self.draft = TextEdit::new(text);
        crate::submit(self, tx, modifier);
    }
}

fn too_big(name: &str, size: u64) -> String {
    format!("{name} is {} MB. The limit is 25 MB.", (size as f64 / 1024.0 / 1024.0).round())
}

/// The thread's half of adding a file: its size against the limit and, for a small text file, its lines (`readLineCount`).
fn inspect(name: &str, mime: &str, path: &std::path::Path) -> Result<(u64, Option<usize>), String> {
    let size = std::fs::metadata(path).map_err(|e| file_error(name, e))?.len();
    if size > MAX_FILE_BYTES {
        return Err(too_big(name, size));
    }
    if !is_text_file(name, mime) || size > LINE_COUNT_MAX_BYTES {
        return Ok((size, None));
    }
    let text = String::from_utf8_lossy(&std::fs::read(path).map_err(|e| file_error(name, e))?).into_owned();
    Ok((size, (!text.contains('\0') && !text.contains('\u{FFFD}')).then(|| count_lines(&text))))
}

fn bytes_of(file: &DraftFile) -> Result<Vec<u8>, String> {
    match &file.source {
        FileSource::Path(path) => std::fs::read(path).map_err(|e| file_error(&file.name, e)),
        FileSource::Text(text) => Ok(text.as_bytes().to_vec()),
    }
}

/// `attachDraftFiles`' loop: a small text file goes into the prompt, anything else is uploaded, in order; the first failure
/// stops it.
fn upload_all(base: &str, session: &str, jobs: Vec<Job>) -> Result<(Vec<String>, Vec<AttachedFile>), String> {
    let (mut inlined, mut uploaded) = (Vec::new(), Vec::new());
    for Job { file } in jobs {
        let bytes = bytes_of(&file)?;
        if file.goes_inline() {
            inlined.push(inline_file_block(&file.name, &String::from_utf8_lossy(&bytes)));
            continue;
        }
        let kind = if file.mime.is_empty() { "application/octet-stream" } else { file.mime.as_str() };
        let answer = http::upload(base, session, &file.name, kind, bytes)?;
        let text = |key: &str| answer.get(key).and_then(|v| v.as_str()).unwrap_or_default().to_string();
        let path = text("path");
        if path.is_empty() {
            return Err(format!("Failed to upload {}: the answer named no path", file.name));
        }
        uploaded.push(AttachedFile {
            path,
            name: text("name"),
            size: answer.get("size").and_then(|v| v.as_u64()).unwrap_or(file.size),
            lines: answer.get("lines").and_then(|v| v.as_u64()).map(|n| n as usize),
            mime: text("mime"),
        });
    }
    Ok((inlined, uploaded))
}

impl State {
    /// Scenes for the files, the paste offer and the terminal lines (piece O): invented files, put in the draft as if a
    /// thread had read them.
    pub fn show_files(&mut self, state: &str) {
        self.set_open((0, 2));
        let owner = self.draft_owner().unwrap_or_default();
        let hold = |s: &mut State, name: &str, size: u64, lines: Option<usize>, pasted: bool| {
            let id = s.held.next_id();
            let source = FileSource::Text("".into());
            s.held.add_file(&owner, DraftFile { id, name: name.into(), mime: String::new(), size, lines, pasted, source });
        };
        let paste: String = (1..=30).map(|n| format!("  at step {n} of the build\n")).collect();
        match state {
            "files" | "file-error" | "uploading" => {
                self.show_draft("Why does the build fail?");
                hold(self, "README.md", 10, Some(2), false);
                hold(self, "build-output.log", 190_464, Some(2031), false);
                hold(self, "incident-report.pdf", 421_888, None, false);
                hold(self, "Makefile", 1_200, Some(1), false);
                if state == "file-error" {
                    self.attach.set_error("dump.zip is 26 MB. The limit is 25 MB.", 0.0);
                }
                self.uploading = state == "uploading";
            }
            "paste-offer" => {
                self.show_draft("Look at this output:");
                let end = self.draft.text.len();
                self.offer = Some(Offer { owner: owner.clone(), text: paste, start: end, end, choice: Choice::Attach });
            }
            "pasted-file" => {
                self.show_draft("Look at this output:");
                hold(self, "pasted-text.txt", paste.len() as u64, Some(count_lines(&paste)), true);
            }
            _ => {
                self.show_draft("Why does this fail?");
                let line = |from, to, text: &str| TerminalDraft {
                    id: format!("t1:{from}"),
                    terminal_id: "t1".into(),
                    label: "zsh".into(),
                    from,
                    to,
                    text: text.into(),
                };
                self.held.add_terminal(&owner, line(9, 11, "FAIL  cache.test.ts\nAssertionError: expected 1, got 2"));
                self.held.add_terminal(&owner, line(42, 42, "npm ERR! missing script: build"));
            }
        }
    }
}
