//! The images held for the next prompt, and the error the composer shows when one is refused
//! (`use-image-attachments.ts`, `use-draft-attachments.ts`, and `addFiles` / `handlePaste` of `Composer.vue`).
//! No Rune and no I/O: reading a file and encoding it is `fleet::attach`, on a thread; this file decides.
use crate::conv::image_sniff::{ALLOWED_IMAGE_MIMES, MAX_ATTACHMENTS_PER_PROMPT, MAX_IMAGE_BYTES, format_bytes, sniff_image_mime};
use std::collections::HashMap;
use std::sync::Arc;

/// `PASTE_ERROR_MS`, in seconds.
pub const ERROR_SECONDS: f64 = 5.0;

/// What a picture is called when it has no name (`att.filename ?? 'image.png'`).
pub const DEFAULT_NAME: &str = "image.png";

/// One picture waiting for the prompt (`PendingAttachment`). The bytes are base64 text, as the prompt carries them, and
/// shared: sending and the put-back after a refusal hold the same text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeldImage {
    pub id: String,
    pub mime: String,
    pub filename: String,
    pub data: Arc<str>,
}

impl HeldImage {
    /// The key its thumbnail is decoded under (`State::images`).
    pub fn preview_key(&self) -> String {
        preview_key(&self.id)
    }
}

pub fn preview_key(id: &str) -> String {
    format!("attach-{id}")
}

/// Whether a name's type is one Fleet takes as a picture. A browser's `File.type` comes from the extension, so this does
/// too: the bytes are checked after they are read.
pub fn mime_from_name(name: &str) -> &'static str {
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" | "jpe" | "jfif" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "bmp" => "image/bmp",
        "txt" | "md" | "log" => "text/plain",
        "json" => "application/json",
        "pdf" => "application/pdf",
        _ => "",
    }
}

pub fn is_image_mime(mime: &str) -> bool {
    ALLOWED_IMAGE_MIMES.contains(&mime)
}

pub const QUEUE_REFUSAL: &str = "Images can't wait in the queue yet. Send now with Ctrl+Enter, or send once the turn ends.";

pub const NO_IMAGES: &str = "This session's harness can't take images.";

/// The checks `addImage` makes before it reads a file, in Fleet's order: the harness, the type, the size, the count.
pub fn check_image(supports_images: bool, mime: &str, size: usize, held: usize) -> Result<(), String> {
    if !supports_images {
        return Err(NO_IMAGES.into());
    }
    if !is_image_mime(mime) {
        return Err(format!("Unsupported image type: {}", if mime.is_empty() { "unknown" } else { mime }));
    }
    if size > MAX_IMAGE_BYTES {
        return Err(format!("Image exceeds the 5 MB limit ({})", format_bytes(size)));
    }
    if held >= MAX_ATTACHMENTS_PER_PROMPT {
        return Err(format!("Too many images (max {MAX_ATTACHMENTS_PER_PROMPT})"));
    }
    Ok(())
}

/// `addFiles`' checks for a picked, dropped or pasted file, then `addImage`'s. `held` is every attachment the prompt has,
/// pictures and files together (and reads still on their way).
pub fn check_file(supports_images: bool, mime: &str, size: usize, held: usize) -> Result<(), String> {
    if is_image_mime(mime) && !supports_images {
        return Err(NO_IMAGES.into());
    }
    if held >= MAX_ATTACHMENTS_PER_PROMPT {
        return Err(format!("{MAX_ATTACHMENTS_PER_PROMPT} files at most per prompt."));
    }
    check_image(supports_images, mime, size, held)
}

/// What the bytes say they are, and the base64 text the prompt carries: the type comes from the bytes, not the name (a
/// PNG called `shot.jpg` is sent as a PNG), and bytes that are no picture are refused.
pub fn prepare(name: &str, bytes: &[u8]) -> Result<(&'static str, String), String> {
    match sniff_image_mime(bytes) {
        Some(mime) => Ok((mime, encode_base64(bytes))),
        None => Err(format!("{} isn't a PNG, JPEG, GIF or WebP picture.", if name.is_empty() { "That file" } else { name })),
    }
}

/// Standard base64 with padding.
pub fn encode_base64(bytes: &[u8]) -> String {
    const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |n, (i, b)| n | (u32::from(*b) << (16 - 8 * i)));
        for i in 0..4 {
            out.push(if i <= chunk.len() { char::from(ABC[(n >> (18 - 6 * i) & 63) as usize]) } else { '=' });
        }
    }
    out
}

/// A paste (`handlePaste`): the files it carries are taken, unless the harness can't take pictures, every file is one, and the
/// clipboard has text too (a spreadsheet's picture): then the text is what's meant.
pub fn paste_takes_files(supports_images: bool, mimes: &[&str], has_text: bool) -> bool {
    if mimes.is_empty() {
        return false;
    }
    let only_unusable = !supports_images && mimes.iter().all(|m| is_image_mime(m));
    !(only_unusable && has_text)
}

/// The images of every draft, by the draft's owner (the session), and the error box's pasted-file message.
#[derive(Default)]
pub struct Attached {
    held: HashMap<String, Vec<HeldImage>>,
    /// Reads started and not finished, by owner: they count toward the limit of ten.
    reading: HashMap<String, usize>,
    /// What was sent and may come back if the server refuses, by correlation id.
    sent: HashMap<String, (String, Vec<HeldImage>)>,
    /// `pasteError` and when it clears.
    error: Option<(String, f64)>,
    serial: u64,
}

impl Attached {
    pub fn of(&self, owner: &str) -> &[HeldImage] {
        self.held.get(owner).map_or(&[], Vec::as_slice)
    }

    /// A fresh id (`Date.now()-random`).
    pub fn next_id(&mut self) -> String {
        self.serial += 1;
        format!("img{}", self.serial)
    }

    pub fn reading(&self, owner: &str) -> usize {
        self.reading.get(owner).copied().unwrap_or(0)
    }

    /// Whether any read is still on its way.
    pub fn has_reads(&self) -> bool {
        self.reading.values().any(|n| *n > 0)
    }

    pub fn begin_read(&mut self, owner: &str) {
        *self.reading.entry(owner.to_string()).or_insert(0) += 1;
    }

    pub fn end_read(&mut self, owner: &str) {
        if let Some(n) = self.reading.get_mut(owner) {
            *n = n.saturating_sub(1);
        }
    }

    /// Holds one picture for the prompt (`addAttachment`).
    pub fn add(&mut self, owner: &str, image: HeldImage) {
        self.held.entry(owner.to_string()).or_default().push(image);
    }

    /// `removeAttachment`: the picture taken off, if it was there.
    pub fn remove(&mut self, owner: &str, id: &str) -> Option<HeldImage> {
        let list = self.held.get_mut(owner)?;
        let at = list.iter().position(|a| a.id == id)?;
        Some(list.remove(at))
    }

    /// `clearAttachments`: every picture of the draft, handed back so their previews can be let go.
    pub fn clear(&mut self, owner: &str) -> Vec<HeldImage> {
        self.held.remove(owner).unwrap_or_default()
    }

    /// The prompt goes: the pictures leave the draft and wait, by the prompt's id, in case the server refuses it.
    pub fn take_for_send(&mut self, owner: &str, correlation_id: &str) -> Vec<HeldImage> {
        let images = self.held.remove(owner).unwrap_or_default();
        if !images.is_empty() {
            self.sent.insert(correlation_id.to_string(), (owner.to_string(), images.clone()));
        }
        images
    }

    /// The server took the prompt: nothing to put back.
    pub fn delivered(&mut self, correlation_id: &str) {
        self.sent.remove(correlation_id);
    }

    /// The server refused it (`putBackRefused`): the pictures that are not back already, for the draft they came from.
    pub fn refused(&mut self, correlation_id: &str) -> Option<(String, Vec<HeldImage>)> {
        let (owner, images) = self.sent.remove(correlation_id)?;
        let back: Vec<HeldImage> = images.into_iter().filter(|i| !self.of(&owner).iter().any(|a| a.id == i.id)).collect();
        Some((owner, back))
    }

    /// `setPasteError`: shown for five seconds from `now`.
    pub fn set_error(&mut self, message: impl Into<String>, now: f64) {
        self.error = Some((message.into(), now + ERROR_SECONDS));
    }

    /// `clearPasteError`.
    pub fn clear_error(&mut self) {
        self.error = None;
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_ref().map(|(m, _)| m.as_str())
    }

    /// The clock ran: the error goes once its time is up. True when it did.
    pub fn expire(&mut self, now: f64) -> bool {
        match &self.error {
            Some((_, until)) if now >= *until => {
                self.error = None;
                true
            }
            _ => false,
        }
    }

    pub fn error_pending(&self) -> bool {
        self.error.is_some()
    }
}
