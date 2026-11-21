//! Sending a used control to the mod that drew it (port of `lib/mods/actions.ts`): `POST /api/sessions/{id}/mods/action`.
//! The mod answers by invalidating (a `mod.ui` the site is already listening to); `State::mod_action` draws the row again
//! when the server could not deliver the action.
use super::types::{Action, ActionKind, Surface};
use crate::fleet::http;
use serde_json::{Value, json};

pub fn body(action: &Action, surface: Surface) -> Value {
    let kind = match action.kind {
        ActionKind::Press => "press",
        ActionKind::Input => "input",
        ActionKind::Submit => "submit",
        ActionKind::Select => "select",
    };
    let surface = match surface {
        Surface::Desktop => "desktop",
        Surface::Phone => "phone",
    };
    let mut body = json!({ "handle": action.handle, "kind": kind, "surface": surface });
    if let Some(value) = &action.value {
        body["value"] = json!(value);
    }
    body
}

/// Whether the server handled it: `handled: true` and nothing else counts.
pub fn send(base: &str, session: &str, action: &Action, surface: Surface) -> bool {
    let path = format!("/api/sessions/{}/mods/action", crate::conv::image_sniff::encode_component(session));
    match http::text_with("POST", base, &path, Some(&body(action, surface))) {
        Ok((status, reply)) if (200..300).contains(&status) => {
            serde_json::from_str::<Value>(&reply).ok().and_then(|v| v.get("handled").and_then(Value::as_bool)) == Some(true)
        }
        _ => false,
    }
}
