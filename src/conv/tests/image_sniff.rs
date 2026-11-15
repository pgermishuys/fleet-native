//! `lib/__tests__/image-sniff.test.ts` under its names, and the limits of `image-validation.ts` (which Fleet has no test
//! for), and `toolScreenshot`.
use crate::conv::image_sniff::*;
use serde_json::json;

/// The bytes padded to a header's length, as base64.
fn b64(bytes: &[u8]) -> String {
    const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut all = bytes.to_vec();
    all.extend([0u8; 12]);
    let mut out = String::new();
    for chunk in all.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |n, (i, b)| n | (u32::from(*b) << (16 - 8 * i)));
        for i in 0..4 {
            out.push(if i <= chunk.len() { char::from(ABC[(n >> (18 - 6 * i) & 63) as usize]) } else { '=' });
        }
    }
    out
}

// sniffImageMime
#[test]
fn names_each_accepted_format_from_its_first_bytes() {
    assert_eq!(sniff_image_mime_base64(&b64(&[0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a])), Some("image/png"));
    assert_eq!(sniff_image_mime_base64(&b64(&[0xff, 0xd8, 0xff, 0xe0])), Some("image/jpeg"));
    assert_eq!(sniff_image_mime_base64(&b64(b"GIF89a")), Some("image/gif"));
    assert_eq!(sniff_image_mime_base64(&b64(b"RIFF\0\0\0\0WEBP")), Some("image/webp"));
    // Raw bytes too, as the composer will hand them.
    assert_eq!(sniff_image_mime(b"GIF87a...."), Some("image/gif"));
}

#[test]
fn is_null_for_svg_bmp_and_ico() {
    assert_eq!(sniff_image_mime_base64(&b64(b"<svg xmlns")), None);
    assert_eq!(sniff_image_mime_base64(&b64(&[0x42, 0x4d, 0x36, 0, 0, 0])), None);
    assert_eq!(sniff_image_mime_base64(&b64(&[0, 0, 1, 0, 1, 0])), None);
    assert_eq!(sniff_image_mime(&[]), None);
}

#[test]
fn reads_base64_with_and_without_padding_and_refuses_what_is_not() {
    assert_eq!(decode_base64("aGk=").unwrap(), b"hi");
    assert_eq!(decode_base64("aGk").unwrap(), b"hi");
    assert_eq!(decode_base64("aG\nk=").unwrap(), b"hi");
    assert!(decode_base64("a*k=").is_none());
    let (mime, bytes) = data_url_image("data:image/png;base64,aGk=").unwrap();
    assert_eq!((mime.as_str(), bytes.as_slice()), ("image/png", &b"hi"[..]));
    assert!(data_url_image("file:///tmp/a.png").is_none() && data_url_image("data:text/plain;base64,aGk=").is_none());
}

// image-validation.ts (no test of its own in Fleet)
#[test]
fn refuses_a_type_it_does_not_accept_and_a_picture_over_five_megabytes() {
    let err = validate_image_attachment("image/bmp", 10, 2).unwrap();
    assert_eq!((err.index, err.message.as_str()), (2, "Unsupported image type: image/bmp"));
    assert!(validate_image_attachment("image/png", 1000, 0).is_none());
    // 6,990,508 base64 characters are about 5.2 MB.
    let err = validate_image_attachment("image/png", 6_990_508, 0).unwrap();
    assert_eq!(err.message, "Image exceeds the 5 MB limit (5.0 MB)");
}

#[test]
fn refuses_more_than_ten_images_for_the_whole_prompt() {
    let eleven = vec![("image/png", 10); 11];
    let errors = validate_attachments(&eleven);
    assert_eq!((errors[0].index, errors[0].message.as_str()), (-1, "Too many images (max 10)"));
    assert_eq!(errors.len(), 1);
    assert!(validate_attachments(&eleven[..10]).is_empty());
}

#[test]
fn says_sizes_as_fleet_does() {
    assert_eq!(format_bytes(812), "812 B");
    assert_eq!(format_bytes(34 * 1024), "34 KB");
    assert_eq!(format_bytes(5 * 1024 * 1024 + 300_000), "5.3 MB");
    assert_eq!(format_bytes(12 * 1024 * 1024), "12 MB");
}

// toolScreenshot
#[test]
fn a_call_keeps_its_screenshot_under_the_session_that_took_it() {
    let state = json!({"metadata": {"screenshot": {"sessionId": "ses 1", "id": "shot_1", "width": 1280, "height": 800}}});
    let shot = tool_screenshot(&state).unwrap();
    assert_eq!(shot.path, "/api/sessions/ses%201/screenshots/shot_1");
    assert_eq!((shot.width, shot.height), (1280, 800));
}

#[test]
fn a_screenshot_missing_any_of_its_four_fields_or_with_no_size_is_none() {
    let with = |s: serde_json::Value| tool_screenshot(&json!({"metadata": {"screenshot": s}}));
    assert!(with(json!({"sessionId": "s", "id": "a", "width": 10, "height": 10})).is_some());
    assert!(with(json!({"id": "a", "width": 10, "height": 10})).is_none());
    assert!(with(json!({"sessionId": "s", "width": 10, "height": 10})).is_none());
    assert!(with(json!({"sessionId": "s", "id": "a", "height": 10})).is_none());
    assert!(with(json!({"sessionId": "s", "id": "a", "width": 0, "height": 10})).is_none());
    assert!(tool_screenshot(&json!({})).is_none());
}
