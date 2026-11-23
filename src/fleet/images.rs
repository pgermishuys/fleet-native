//! The pictures the conversation shows: a call's screenshot (`use-machine-image.ts`) and the images a user sent. A view
//! names the picture it wants; `poll` fetches and decodes it off the UI thread and the view finds it ready next draw.
//! A picture that can't be had (deleted session, a Fleet that lost the file, bytes that aren't an image) is `Missing`,
//! and the part that asked draws nothing at all: no broken-image box (`ToolScreenshot.vue`'s `missing`).
use super::http;
use crate::State;
use crate::conv::image_sniff::{ALLOWED_IMAGE_MIMES, data_url_image, sniff_image_mime};
use rune::{Image, Tx};
use std::cell::RefCell;
use std::collections::HashMap;
use std::time::Duration;

/// How many decoded pictures are kept; the ones asked for longest ago go first.
const DECODED_KEPT: usize = 64;

/// Where a picture comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    /// A path on Fleet (`/api/sessions/{id}/screenshots/{id}`).
    Path(String),
    /// A `data:image/...;base64,` address, as a user's attachment arrives.
    Data(String),
}

pub enum Slot {
    /// Asked for, not decoded yet (its bytes may still be on the way).
    Loading(Option<Image>),
    Ready(Image),
    Missing,
}

/// The lightbox that is open: which picture, what to call it, and the thumbnail focus goes back to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lightbox {
    pub key: String,
    pub alt: String,
    pub opener: String,
}

pub struct Images {
    slots: HashMap<String, Slot>,
    /// What views have asked for since `poll` last looked. A view can't change the state, so it leaves its wants here.
    wanted: RefCell<Vec<(String, Source)>>,
    pub lightbox: Option<Lightbox>,
    /// When each picture was last asked for, so the oldest decoded ones can be let go (`evict`).
    touched: RefCell<HashMap<String, u64>>,
    clock: std::cell::Cell<u64>,
    /// Bytes that arrived before the window's image cache existed (a scene is applied before `setup`).
    parked: Vec<(String, Vec<u8>)>,
    /// The window as the lightbox last measured it, to size the picture to.
    pub window: (f32, f32),
}

impl Default for Images {
    fn default() -> Self {
        Images {
            slots: HashMap::new(),
            wanted: RefCell::default(),
            lightbox: None,
            touched: RefCell::default(),
            clock: Default::default(),
            parked: Vec::new(),
            window: (1280.0, 760.0),
        }
    }
}

impl Images {
    /// Asks for a picture, once: a view calls this on every draw.
    pub fn want(&self, key: &str, source: Source) {
        self.clock.set(self.clock.get() + 1);
        self.touched.borrow_mut().insert(key.to_string(), self.clock.get());
        let mut wanted = self.wanted.borrow_mut();
        if !self.slots.contains_key(key) && !wanted.iter().any(|(k, _)| k == key) {
            wanted.push((key.to_string(), source));
        }
    }

    pub fn slot(&self, key: &str) -> Option<&Slot> {
        self.slots.get(key)
    }

    /// The decoded picture and its size, once there is one.
    pub fn ready(&self, key: &str) -> Option<(&Image, [u32; 2])> {
        match self.slots.get(key)? {
            Slot::Ready(image) => image.size().map(|size| (image, size)),
            _ => None,
        }
    }

    /// Lets a picture go (a thumbnail whose image left the draft).
    pub fn forget(&mut self, key: &str) {
        self.slots.remove(key);
    }

    /// Lets go of every picture whose key `drop` picks.
    pub fn forget_where(&mut self, drop: impl Fn(&str) -> bool) {
        self.slots.retain(|k, _| !drop(k));
    }

    /// What views have asked for and `poll` hasn't started yet: a test reads what a draw or a tick wanted.
    #[cfg(test)]
    pub fn asked(&self) -> Vec<String> {
        self.wanted.borrow().iter().map(|(k, _)| k.clone()).collect()
    }

    pub fn is_missing(&self, key: &str) -> bool {
        matches!(self.slots.get(key), Some(Slot::Missing))
    }

    /// What a message's row draws from: moves when one of its pictures arrives or is lost.
    pub fn state_of(&self, key: &str) -> u8 {
        match self.slots.get(key) {
            None => 0,
            Some(Slot::Loading(_)) => 1,
            Some(Slot::Ready(_)) => 2,
            Some(Slot::Missing) => 3,
        }
    }

    /// Lets go of the decoded pictures asked for longest ago once more than `DECODED_KEPT` are held: a long conversation's
    /// screenshots would otherwise stay in memory for the whole run. One a row draws again is fetched again. The open
    /// lightbox's picture stays. Returns whether any was let go.
    fn evict(&mut self) -> bool {
        let ready = self.slots.values().filter(|s| matches!(s, Slot::Ready(_))).count();
        if ready <= DECODED_KEPT {
            return false;
        }
        let touched = self.touched.borrow();
        let mut ready_keys: Vec<(u64, String)> = self
            .slots
            .iter()
            .filter(|(k, s)| matches!(s, Slot::Ready(_)) && self.lightbox.as_ref().is_none_or(|l| l.key != **k))
            .map(|(k, _)| (touched.get(k).copied().unwrap_or(0), k.clone()))
            .collect();
        drop(touched);
        ready_keys.sort();
        for (_, key) in ready_keys.into_iter().take(ready - DECODED_KEPT) {
            self.slots.remove(&key);
            self.touched.borrow_mut().remove(&key);
        }
        true
    }

    fn pending(&self) -> bool {
        self.slots.values().any(|s| matches!(s, Slot::Loading(_)))
    }
}

/// The session a `/api/sessions/{id}/...` path belongs to: its machine is the one to ask, with that machine's token.
pub fn session_of_path(path: &str) -> Option<&str> {
    path.strip_prefix("/api/sessions/").and_then(|rest| rest.split('/').next()).filter(|id| !id.is_empty())
}

/// The bytes of a source, on a thread.
fn bytes_of(base: &str, source: &Source) -> Result<Vec<u8>, String> {
    match source {
        Source::Path(path) => http::get_bytes(base, path),
        Source::Data(url) => data_url_image(url).map(|(_, bytes)| bytes).ok_or_else(|| "not an image address".to_string()),
    }
}

impl State {
    /// Starts what the views asked for and settles what has decoded since; true when a picture changed, so the rows
    /// that show it redraw. Called from `poll`.
    pub fn keep_images(&mut self, tx: &mut Tx) -> bool {
        if self.cache.is_some() {
            for (key, bytes) in std::mem::take(&mut self.images.parked) {
                self.on_image_bytes(&key, Ok(bytes));
            }
        }
        let asked: Vec<_> = self.images.wanted.borrow_mut().drain(..).collect();
        for (key, source) in asked {
            if self.images.slots.contains_key(&key) {
                continue;
            }
            // With no Fleet there is nothing to ask; a scene or a test hands pictures over with `supply_image`. A picture of
            // a session on another machine is fetched from that machine, with its token (`useMachineImage`), whichever
            // session is open by the time the request starts.
            let Some(link) = self.image_link(&source) else { continue };
            self.images.slots.insert(key.clone(), Slot::Loading(None));
            link.off_thread(move |base| bytes_of(base, &source), move |s, _, bytes| s.on_image_bytes(&key, bytes));
        }
        let changed = self.settle_images() | self.images.evict();
        if self.images.pending() {
            tx.wake_after(Duration::from_millis(50));
        }
        changed
    }

    /// The link a picture is fetched through: its session's machine's, for a path of `/api/sessions/{id}/...`, else the open
    /// session's.
    fn image_link(&self, source: &Source) -> Option<super::link::Link> {
        let session = match source {
            Source::Path(path) => session_of_path(path),
            Source::Data(_) => None,
        };
        match session {
            Some(id) => self.link_for(id),
            None => self.open_link(),
        }
    }

    /// Pictures whose decoding has finished become ready, or lost when it failed.
    fn settle_images(&mut self) -> bool {
        let mut changed = false;
        for slot in self.images.slots.values_mut() {
            if let Slot::Loading(Some(image)) = slot
                && image.is_ready()
            {
                *slot = if image.error().is_some() { Slot::Missing } else { Slot::Ready(image.clone()) };
                changed = true;
            }
        }
        changed
    }

    /// What the thread found: bytes to decode, or why there are none. A picture that isn't PNG, JPEG, GIF or WebP is
    /// treated as lost.
    pub fn on_image_bytes(&mut self, key: &str, bytes: Result<Vec<u8>, String>) {
        if let (Ok(bytes), None) = (&bytes, &self.cache) {
            self.images.parked.push((key.to_string(), bytes.clone()));
            self.images.slots.insert(key.to_string(), Slot::Loading(None));
            return;
        }
        let slot = match (bytes, self.cache.as_ref()) {
            (Ok(bytes), Some(cache)) if sniff_image_mime(&bytes).is_some_and(|m| ALLOWED_IMAGE_MIMES.contains(&m)) => {
                Slot::Loading(Some(cache.load(bytes)))
            }
            _ => Slot::Missing,
        };
        self.images.slots.insert(key.to_string(), slot);
    }

    /// A picture already in hand (a scene's, a test's), decoded before it returns so a snapshot has it.
    pub fn supply_image(&mut self, key: &str, bytes: &[u8]) {
        self.on_image_bytes(key, Ok(bytes.to_vec()));
        if let Some(Slot::Loading(Some(image))) = self.images.slots.get(key) {
            let _ = image.wait();
        }
        self.settle_images();
    }

    /// A thumbnail was pressed (`expanded = true`): the lightbox opens over the window.
    pub fn open_lightbox(&mut self, key: &str, alt: &str, opener: &str) {
        self.images.lightbox = Some(Lightbox { key: key.into(), alt: alt.into(), opener: opener.into() });
    }

    pub fn close_lightbox(&mut self) {
        self.images.lightbox = None;
    }
}
