//! An image of a session's folder, read from the session's machine (`composables/use-session-image.ts`, rows 7.1 to 7.3 of
//! docs/parity/11-files.md). Bytes come from `GET …/files/image?path=` on a thread, with the machine's token; a newer read of the same
//! file wins over an older one, and an old picture stays up while its file is read again. Decoding is Rune's `ImageCache`, on its own
//! worker, so nothing here waits.
use super::http;
use crate::State;
use crate::composer::format::format_bytes;
use rune::{Image, Tx};
use std::collections::HashMap;
use std::time::Duration;

/// `useSessionImage`'s words for a file that is gone, and for one that can't be read.
pub const GONE: &str = "This file isn't in the session's folder any more.";
pub const UNREADABLE: &str = "The image couldn't be read.";
/// `imageBroken`: bytes came, but nothing could be drawn from them.
pub const UNDRAWABLE: &str = "This image couldn't be shown.";

#[derive(Default)]
struct Entry {
    /// Which read is the latest; an answer from an older one is dropped.
    read: u64,
    /// The file's size in bytes, once read.
    bytes: Option<usize>,
    /// Why it can't be shown (the read's, not the decoder's).
    error: Option<&'static str>,
    image: Option<Image>,
    /// An SVG that gives no width and height of its own has no pixel size to show (`naturalWidth` is 0).
    sized: bool,
    /// Bytes that came before the window's image cache existed (a scene is applied before `setup`).
    parked: Option<Vec<u8>>,
    /// Wether the picture is an SVG, which Rune draws at twice its size to stay sharp when it is scaled.
    svg: bool,
}

/// What a file tab draws for an image.
pub enum Shown<'a> {
    /// Nothing has come yet: "Opening {path}…".
    Opening,
    Note(&'static str),
    Picture {
        image: &'a Image,
        size: [f32; 2],
        meta: String,
    },
}

#[derive(Default)]
pub struct SessionImages {
    entries: HashMap<String, Entry>,
    reads: u64,
    /// The picture's area as the view last measured it (the tab body), to scale the picture to.
    pub area: [f32; 2],
    /// Whether the last look found a picture still decoding, so the frame it finishes in is drawn.
    was_pending: bool,
}

fn key(machine: &str, session: &str, path: &str) -> String {
    format!("{machine}\u{1}{session}\u{1}{path}")
}

/// Does an SVG's root element say its own width and height? A percentage or a missing one leaves the browser no pixel size.
pub fn svg_has_own_size(bytes: &[u8]) -> bool {
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(4096)]).to_ascii_lowercase();
    let Some(at) = head.find("<svg") else { return false };
    let tag = &head[at..head[at..].find('>').map_or(head.len(), |e| at + e)];
    let has = |name: &str| {
        tag.match_indices(name).any(|(i, _)| {
            let before = tag[..i].chars().last().is_some_and(char::is_whitespace);
            let rest = tag[i + name.len()..].trim_start();
            before
                && rest.strip_prefix('=').is_some_and(|v| {
                    let v = v.trim_start().trim_start_matches(['"', '\'']);
                    v.starts_with(|c: char| c.is_ascii_digit() || c == '.') && !v.split(['"', '\'']).next().unwrap_or("").ends_with('%')
                })
        })
    };
    has("width") && has("height")
}

fn is_svg(bytes: &[u8]) -> bool {
    String::from_utf8_lossy(&bytes[..bytes.len().min(1024)]).to_ascii_lowercase().contains("<svg")
}

impl SessionImages {
    fn entry(&self, machine: &str, session: &str, path: &str) -> Option<&Entry> {
        self.entries.get(&key(machine, session, path))
    }

    /// Lets a file's picture go (its tab closed).
    pub fn forget(&mut self, machine: &str, session: &str, path: &str) {
        self.entries.remove(&key(machine, session, path));
    }

    /// What the tab draws: the note, the picture and what the bar says about it.
    pub fn shown(&self, machine: &str, session: &str, path: &str) -> Shown<'_> {
        let Some(e) = self.entry(machine, session, path) else { return Shown::Opening };
        if let Some(why) = e.error {
            return Shown::Note(why);
        }
        let Some(image) = &e.image else { return Shown::Opening };
        if image.error().is_some() {
            return Shown::Note(UNDRAWABLE);
        }
        let Some([w, h]) = image.size() else { return Shown::Opening };
        let k = if e.svg { 0.5 } else { 1.0 };
        let size = format_bytes(e.bytes.map(|b| b as f64));
        let meta = if e.sized { format!("{} \u{d7} {} \u{b7} {size}", (w as f32 * k).round(), (h as f32 * k).round()) } else { size };
        Shown::Picture { image, size: [w as f32 * k, h as f32 * k], meta }
    }

    /// A read was started for the file: the number its answer must carry.
    fn start(&mut self, machine: &str, session: &str, path: &str) -> u64 {
        self.reads += 1;
        let e = self.entries.entry(key(machine, session, path)).or_default();
        e.read = self.reads;
        e.read
    }

    fn pending(&self) -> bool {
        self.entries.values().any(|e| e.parked.is_some() || e.image.as_ref().is_some_and(|i| !i.is_ready()))
    }

    /// The files whose picture has been asked for (a test reads what is there).
    #[cfg(test)]
    pub fn count(&self) -> usize {
        self.entries.len()
    }
}

impl State {
    /// Reads the image of `path` again from its machine: when its tab opens, and each time it comes back to the front, since the
    /// agent may have changed it while it was behind (`onActivated`).
    pub fn read_session_image(&mut self, session: &str, path: &str) {
        let machine = self.machine_key_of(session).to_string();
        let read = self.session_images.start(&machine, session, path);
        let Some(link) = self.link_for(session) else { return };
        let (id, file) = (session.to_string(), path.to_string());
        let (id2, file2) = (id.clone(), file.clone());
        link.off_thread(
            move |base| http::files_image(base, &id, &file),
            move |s, _, answer| s.on_session_image(&machine, &id2, &file2, read, answer),
        );
    }

    /// What the read found. A 404 says the file is gone, any other refusal or no answer that it can't be read; bytes go to the decoder.
    pub fn on_session_image(&mut self, machine: &str, session: &str, path: &str, read: u64, answer: Result<(u16, Vec<u8>), String>) {
        let Some(e) = self.session_images.entries.get_mut(&key(machine, session, path)) else { return };
        if e.read != read {
            return;
        }
        match answer {
            Ok((200, bytes)) => {
                e.error = None;
                e.bytes = Some(bytes.len());
                e.svg = is_svg(&bytes);
                e.sized = !e.svg || svg_has_own_size(&bytes);
                match &self.cache {
                    Some(cache) => e.image = Some(cache.load(bytes)),
                    None => e.parked = Some(bytes),
                }
            }
            Ok((404, _)) => self.fail_session_image(machine, session, path, GONE),
            _ => self.fail_session_image(machine, session, path, UNREADABLE),
        }
    }

    fn fail_session_image(&mut self, machine: &str, session: &str, path: &str, why: &'static str) {
        if let Some(e) = self.session_images.entries.get_mut(&key(machine, session, path)) {
            e.error = Some(why);
            e.bytes = None;
            e.image = None;
        }
    }

    /// A picture already in hand (a scene's, a test's), decoded before it returns so a snapshot has it.
    pub fn supply_session_image(&mut self, session: &str, path: &str, bytes: &[u8]) {
        let machine = self.answer_session_image(session, path, Ok((200, bytes.to_vec())));
        self.keep_session_images_now();
        if let Some(image) = self.session_images.entry(&machine, session, path).and_then(|e| e.image.clone()) {
            let _ = image.wait();
        }
    }

    /// A read and its answer at once (a scene's, a test's); the machine the file is on.
    pub fn answer_session_image(&mut self, session: &str, path: &str, answer: Result<(u16, Vec<u8>), String>) -> String {
        let machine = self.machine_key_of(session).to_string();
        let read = self.session_images.start(&machine, session, path);
        self.on_session_image(&machine, session, path, read, answer);
        machine
    }

    fn keep_session_images_now(&mut self) {
        let Some(cache) = &self.cache else { return };
        for e in self.session_images.entries.values_mut() {
            if let Some(bytes) = e.parked.take() {
                e.image = Some(cache.load(bytes));
            }
        }
    }

    /// Hands parked bytes to the decoder once the window has a cache, and wakes while a picture is still decoding; true when
    /// something is waiting, so `poll` draws again.
    pub fn keep_session_images(&mut self, tx: &mut Tx) -> bool {
        self.keep_session_images_now();
        let pending = self.session_images.pending();
        if pending {
            tx.wake_after(Duration::from_millis(50));
        }
        pending | std::mem::replace(&mut self.session_images.was_pending, pending)
    }
}
