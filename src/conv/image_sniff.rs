//! What a picture is, from its bytes, and what Fleet will accept as one (`lib/image-validation.ts`), plus where a call's
//! screenshot lives (`toolScreenshot` in `activity-stream-tool-card.ts`). No Rune in here.
use serde_json::Value;

pub const ALLOWED_IMAGE_MIMES: [&str; 4] = ["image/png", "image/jpeg", "image/gif", "image/webp"];
/// 5 MB, Claude's per-image limit.
pub const MAX_IMAGE_BYTES: usize = 5 * 1024 * 1024;
pub const MAX_ATTACHMENTS_PER_PROMPT: usize = 10;

/// The image type the bytes themselves say (`sniffImageMime`): a file's type from its name lies (a PNG called `shot.jpg`
/// arrives as JPEG and the provider refuses it mid-turn). `None` for anything but PNG, JPEG, GIF and WebP.
pub fn sniff_image_mime(bytes: &[u8]) -> Option<&'static str> {
    let at = |offset: usize, text: &[u8]| bytes.get(offset..offset + text.len()) == Some(text);
    if at(0, &[0x89]) && at(1, b"PNG\r\n\x1a\n") {
        Some("image/png")
    } else if at(0, &[0xff, 0xd8, 0xff]) {
        Some("image/jpeg")
    } else if at(0, b"GIF87a") || at(0, b"GIF89a") {
        Some("image/gif")
    } else if at(0, b"RIFF") && at(8, b"WEBP") {
        Some("image/webp")
    } else {
        None
    }
}

/// `sniffImageMime` of base64 text: only its first 24 characters are read.
pub fn sniff_image_mime_base64(head: &str) -> Option<&'static str> {
    let end = head.char_indices().nth(24).map_or(head.len(), |(i, _)| i);
    sniff_image_mime(&decode_base64(&head[..end])?)
}

/// Standard base64, padded or not, whitespace ignored; `None` for anything else. A crate would do the same in one line,
/// but this is the only place the app reads it and the lock holds two versions of the crate already.
pub fn decode_base64(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let (mut bits, mut have) = (0u32, 0u32);
    for c in text.bytes().filter(|c| !c.is_ascii_whitespace()) {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => break,
            _ => return None,
        };
        bits = (bits << 6) | u32::from(v);
        have += 6;
        if have >= 8 {
            have -= 8;
            out.push((bits >> have) as u8);
            bits &= (1 << have) - 1;
        }
    }
    Some(out)
}

/// The mime type and bytes of a `data:image/...;base64,` URL; `None` for any other address.
pub fn data_url_image(url: &str) -> Option<(String, Vec<u8>)> {
    let rest = url.strip_prefix("data:")?;
    let (head, data) = rest.split_once(',')?;
    let mime = head.strip_suffix(";base64")?;
    mime.starts_with("image/").then(|| decode_base64(data).map(|bytes| (mime.to_string(), bytes)))?
}

/// `formatBytes`: `812 B`, `34 KB`, `5.1 MB`, `12 MB`.
pub fn format_bytes(bytes: usize) -> String {
    const KB: f64 = 1024.0;
    let n = bytes as f64;
    if n < KB {
        return format!("{bytes} B");
    }
    if n < KB * KB {
        return format!("{} KB", (n / KB).round());
    }
    let (value, unit) = if n < KB * KB * KB { (n / (KB * KB), "MB") } else { (n / (KB * KB * KB), "GB") };
    if value < 10.0 { format!("{value:.1} {unit}") } else { format!("{value:.0} {unit}") }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageValidationError {
    /// The attachment it is about; -1 for the whole prompt.
    pub index: i32,
    pub message: String,
}

/// One attachment (`validateImageAttachment`): its type, and its size from the base64 text's length.
pub fn validate_image_attachment(mime: &str, base64_len: usize, index: i32) -> Option<ImageValidationError> {
    if !ALLOWED_IMAGE_MIMES.contains(&mime) {
        return Some(ImageValidationError { index, message: format!("Unsupported image type: {mime}") });
    }
    let approx = (base64_len * 3).div_ceil(4);
    (approx > MAX_IMAGE_BYTES)
        .then(|| ImageValidationError { index, message: format!("Image exceeds the 5 MB limit ({})", format_bytes(approx)) })
}

/// A prompt's attachments (`validateAttachments`), as (mime, base64 length).
pub fn validate_attachments(attachments: &[(&str, usize)]) -> Vec<ImageValidationError> {
    let mut errors = Vec::new();
    if attachments.len() > MAX_ATTACHMENTS_PER_PROMPT {
        errors.push(ImageValidationError { index: -1, message: format!("Too many images (max {MAX_ATTACHMENTS_PER_PROMPT})") });
    }
    errors.extend(attachments.iter().enumerate().filter_map(|(i, (mime, len))| validate_image_attachment(mime, *len, i as i32)));
    errors
}

/// A screenshot Fleet kept for a call, so the conversation can show it under the call (`ToolCardScreenshot`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shot {
    /// Its API path. It can name the parent's session: a sub-agent's shot is kept under the session you started.
    pub path: String,
    /// The size it was taken at, so the thumbnail keeps its shape before the picture arrives.
    pub width: u32,
    pub height: u32,
}

/// `encodeURIComponent`: everything but letters, digits and `-_.!~*'()` as `%XX` of its UTF-8 bytes.
pub fn encode_component(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// The screenshot in a call's `metadata.screenshot` (`{sessionId, id, width, height}`); all four must be there and the
/// size nonzero (`toolScreenshot`).
pub fn tool_screenshot(state: &Value) -> Option<Shot> {
    let shot = state.pointer("/metadata/screenshot")?;
    let text = |k: &str| shot.get(k).and_then(Value::as_str).filter(|s| !s.is_empty());
    let size = |k: &str| shot.get(k).and_then(Value::as_f64).filter(|n| n.is_finite() && *n > 0.0).map(|n| n as u32).filter(|n| *n > 0);
    Some(Shot {
        path: format!("/api/sessions/{}/screenshots/{}", encode_component(text("sessionId")?), encode_component(text("id")?)),
        width: size("width")?,
        height: size("height")?,
    })
}
