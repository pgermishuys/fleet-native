//! Pictures for the next prompt: the picker, a paste, a drop, and the thread that reads and encodes them
//! (`use-image-attachments.ts`, `Composer.vue`'s `addFiles`, `handlePaste`, `putBackRefused`). The decisions are
//! `composer::attach`; this file starts the work off the UI thread and takes its answers in `poll`.
use crate::State;
use crate::composer::attach::{self, HeldImage, check_file, check_image, mime_from_name, prepare, preview_key};
use rune::{ClipboardContent, FileDialogOptions, Tx};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

/// What a worker thread found.
pub enum Done {
    /// A picture is ready to hold: its type from the bytes, the base64 text, and the bytes for the thumbnail.
    Read { owner: String, id: String, name: String, mime: String, data: String, bytes: Vec<u8> },
    /// It could not be held; the message goes in the error box.
    Refused { owner: String, message: String },
    /// A picture put back after a refusal: only its thumbnail is missing.
    Preview { id: String, bytes: Vec<u8> },
    /// A file that is no picture, checked and measured (piece O): its chip can be drawn.
    File { owner: String, file: crate::composer::attached::DraftFile },
    /// The held files went up (or one failed): what goes into the text.
    Uploaded { owner: String, modifier: bool, result: Result<(Vec<String>, Vec<crate::composer::attached::AttachedFile>), String> },
}

/// Where the threads leave their answers.
pub struct Reads {
    pub(super) send: Sender<Done>,
    receive: Receiver<Done>,
}

impl Default for Reads {
    fn default() -> Self {
        let (send, receive) = channel();
        Reads { send, receive }
    }
}

/// Where a picture's bytes come from.
enum Source {
    File(PathBuf),
    Bytes(Vec<u8>),
}

/// The name a pasted picture gets (`blob.name || "image.png"`).
const PASTED_NAME: &str = attach::DEFAULT_NAME;

fn name_of(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

impl State {
    /// Whose pictures these are: the open session's draft, or on the New session page the draft's row.
    fn attach_owner(&self) -> Option<String> {
        if self.on_new_session_page() { self.new_row_key() } else { self.draft_owner() }
    }

    /// Whether the harness the next prompt goes to passes pictures on (`supportsImages`).
    fn attach_supports_images(&self) -> bool {
        if self.on_new_session_page() { self.new_page_supports_images() } else { self.caps().supports_images }
    }

    /// The pictures held for the open session's next prompt.
    pub fn held_images(&self) -> &[HeldImage] {
        match self.attach_owner() {
            Some(owner) => self.attach.of(&owner),
            None => &[],
        }
    }

    /// What the error box shows besides the send errors: the pasted-file message.
    pub fn attach_error(&self) -> Option<&str> {
        self.attach.error()
    }

    /// The paperclip: the system's file picker. Rune's dialog takes one file at a time (docs/RUNE.md).
    pub fn pick_files(&mut self, tx: &mut Tx) {
        if if self.on_new_session_page() { self.new_session_starting() } else { self.composer_disabled() } {
            return;
        }
        let options = FileDialogOptions { title: "Attach files".into(), ..Default::default() };
        tx.open_file(options, |s: &mut State, answer, tx| match answer {
            Ok(Some(path)) => s.add_files(&[path], tx),
            Ok(None) => {}
            Err(why) => s.attach.set_error(format!("The file picker couldn't open: {why}"), s.clock),
        });
    }

    /// `addFiles`: each file is checked in Fleet's order, then read and encoded on a thread.
    pub fn add_files(&mut self, paths: &[PathBuf], tx: &mut Tx) {
        for path in paths {
            let name = name_of(path);
            self.add_one(name.clone(), mime_from_name(&name), Source::File(path.clone()), tx);
        }
    }

    /// A picture on the clipboard (PNG bytes), held as `image.png`.
    pub fn add_pasted_png(&mut self, png: Vec<u8>, tx: &mut Tx) {
        self.add_one(PASTED_NAME.into(), "image/png", Source::Bytes(png), tx);
    }

    fn add_one(&mut self, name: String, mime: &'static str, source: Source, tx: &mut Tx) {
        let Some(owner) = self.attach_owner() else { return };
        let held = self.attach.of(&owner).len() + self.held.files(&owner).len() + self.attach.reading(&owner);
        // The first message takes pictures only: anything else is refused in the order `addImage` checks (A9.3).
        if self.on_new_session_page()
            && !attach::is_image_mime(mime)
            && let Err(message) = attach::check_image(self.attach_supports_images(), mime, 0, held)
        {
            self.attach.set_error(message, self.clock);
            tx.wake_after(Duration::from_secs_f64(attach::ERROR_SECONDS + 0.1));
            return;
        }
        // Anything but a picture is held as a file until Send (row 11.1).
        if !attach::is_image_mime(mime) {
            if held >= crate::conv::image_sniff::MAX_ATTACHMENTS_PER_PROMPT {
                let max = crate::conv::image_sniff::MAX_ATTACHMENTS_PER_PROMPT;
                self.attach.set_error(format!("{max} files at most per prompt."), self.clock);
                tx.wake_after(Duration::from_secs_f64(attach::ERROR_SECONDS + 0.1));
            } else if let Source::File(path) = source {
                self.add_plain_file(owner, name, mime, path, tx);
            }
            return;
        }
        let size = match &source {
            Source::Bytes(bytes) => bytes.len(),
            // A file's size is read on the thread, with the file.
            Source::File(_) => 0,
        };
        if let Err(message) = check_file(self.attach_supports_images(), mime, size, held) {
            self.attach.set_error(message, self.clock);
            tx.wake_after(Duration::from_secs_f64(attach::ERROR_SECONDS + 0.1));
            return;
        }
        self.attach.begin_read(&owner);
        let id = self.attach.next_id();
        let send = self.reads.send.clone();
        std::thread::spawn(move || {
            let done = match read(&name, source) {
                Ok((mime, data, bytes)) => Done::Read { owner, id, name, mime, data, bytes },
                Err(message) => Done::Refused { owner, message },
            };
            let _ = send.send(done);
        });
        tx.wake_after(Duration::from_millis(30));
    }

    /// `handlePaste`: a paste that carries files (copied in a file manager) or a picture (a screenshot) takes them; anything
    /// else pastes as text. False lets the field do that.
    pub fn paste_files(&mut self, content: &ClipboardContent, tx: &mut Tx) -> bool {
        let mut mimes: Vec<&str> = content.files().iter().map(|f| mime_from_name(&name_of(f))).collect();
        if content.image().is_some() {
            mimes.push("image/png");
        }
        if !attach::paste_takes_files(self.attach_supports_images(), &mimes, !content.text().is_empty()) {
            return !self.on_new_session_page() && self.paste_text(content.text(), tx);
        }
        self.add_files(content.files(), tx);
        if let Some(image) = content.image() {
            self.add_pasted_png(image.png().to_vec(), tx);
        }
        true
    }

    /// The thumbnail's key is let go once the picture leaves the draft.
    pub fn forget_previews(&mut self, images: &[HeldImage]) {
        for image in images {
            self.images.forget(&image.preview_key());
        }
    }

    /// The x on a chip.
    pub fn remove_image(&mut self, id: &str) {
        let Some(owner) = self.attach_owner() else { return };
        if let Some(image) = self.attach.remove(&owner, id) {
            self.forget_previews(&[image]);
        }
    }

    /// What the threads found since the last frame, and the error's time running out. True when the composer changed.
    pub fn keep_attachments(&mut self, tx: &mut Tx) -> bool {
        let mut changed = false;
        while let Ok(done) = self.reads.receive.try_recv() {
            changed = true;
            match done {
                Done::Read { owner, id, name, mime, data, bytes } => {
                    self.attach.end_read(&owner);
                    let image = HeldImage { id, mime, filename: name, data: Arc::from(data) };
                    self.on_image_bytes(&image.preview_key(), Ok(bytes));
                    self.attach.add(&owner, image);
                    self.attach.clear_error();
                }
                Done::Refused { owner, message } => {
                    self.attach.end_read(&owner);
                    self.attach.set_error(message, self.clock);
                    tx.wake_after(Duration::from_secs_f64(attach::ERROR_SECONDS + 0.1));
                }
                Done::Preview { id, bytes } => self.on_image_bytes(&preview_key(&id), Ok(bytes)),
                Done::File { owner, file } => self.on_file_read(&owner, file),
                Done::Uploaded { owner, modifier, result } => self.on_uploaded(&owner, modifier, result, tx),
            }
        }
        if self.attach.expire(self.clock) {
            changed = true;
        }
        if self.attach.has_reads() || self.uploading {
            tx.wake_after(Duration::from_millis(30));
        } else if self.attach.error_pending() {
            tx.wake_after(Duration::from_millis(250));
        }
        changed
    }

    /// `putBackRefused`: the server refused the prompt, so its pictures come back beside any added since.
    pub fn put_back_images(&mut self, correlation_id: &str, tx: &mut Tx) {
        let Some((owner, images)) = self.attach.refused(correlation_id) else { return };
        for image in images {
            let (id, data, send) = (image.id.clone(), image.data.clone(), self.reads.send.clone());
            self.attach.add(&owner, image);
            // The thumbnail is made again from the text the prompt carried.
            std::thread::spawn(move || {
                if let Some(bytes) = crate::conv::image_sniff::decode_base64(&data) {
                    let _ = send.send(Done::Preview { id, bytes });
                }
            });
        }
        tx.wake_after(Duration::from_millis(30));
    }
}

/// The thread's half of `addImage`: the file's size against the limit, its bytes, and what they say they are.
fn read(name: &str, source: Source) -> Result<(String, String, Vec<u8>), String> {
    let bytes = match source {
        Source::Bytes(bytes) => bytes,
        Source::File(path) => {
            let size = std::fs::metadata(&path).map_err(|e| format!("{name} couldn't be read: {e}"))?.len() as usize;
            check_image(true, "image/png", size, 0)?;
            std::fs::read(&path).map_err(|e| format!("{name} couldn't be read: {e}"))?
        }
    };
    let (mime, data) = prepare(name, &bytes)?;
    Ok((mime.to_string(), data, bytes))
}
