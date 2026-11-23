//! The picture scenes (piece 3N): a call whose screenshot Fleet kept, one Fleet has lost, and a user's attached images.
//! The "page" is `assets/mock_page.png`, an invented shop page; the shot is handed over as if Fleet had served it.
use super::failure::NOW;
use super::tools::{call, message};
use crate::State;
use crate::mock::Status;
use serde_json::{Value, json};

/// The invented page both the scene and Fleet's own client are shown.
pub const PAGE: &[u8] = include_bytes!("../../assets/mock_page.png");
/// The invented picture a first message carries (`assets/login_error.png`: a window with an error line).
pub const FIRST_PICTURE: &[u8] = include_bytes!("../../assets/login_error.png");
/// The path the scene's screenshot is kept under (`/api/sessions/{id}/screenshots/{id}`).
pub const SHOT_PATH: &str = "/api/sessions/scene/screenshots/shot_1";

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Case {
    /// A screenshot under its call.
    Shot,
    /// The same, with the lightbox open.
    Lightbox,
    /// Fleet has lost the file: the call shows no picture and no box.
    Lost,
    /// A user's message with two images, and one with images only.
    Attachments,
    /// The first message of a new session, shown as sent with its picture before the server's copy arrives (A9.7).
    FirstMessage,
}

fn base64(bytes: &[u8]) -> String {
    const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |n, (i, b)| n | (u32::from(*b) << (16 - 8 * i)));
        for i in 0..4 {
            out.push(if i <= chunk.len() { char::from(ABC[(n >> (18 - 6 * i) & 63) as usize]) } else { '=' });
        }
    }
    out
}

fn file(id: &str, name: &str, bytes: &[u8]) -> Value {
    json!({"type": "file", "id": id, "mime": "image/png", "filename": name, "url": format!("data:image/png;base64,{}", base64(bytes))})
}

impl State {
    pub fn show_images(&mut self, case: Case) {
        self.set_open((0, 0));
        let text = |id: &str, body: &str| json!({"type": "text", "id": id, "text": body});
        let messages = if case == Case::FirstMessage {
            Vec::new()
        } else if case == Case::Attachments {
            vec![
                message(
                    "u1",
                    "user",
                    NOW - 120_000,
                    vec![
                        text("u1-t", "Which of these two layouts is closer to the spec?"),
                        file("f1", "checkout.png", PAGE),
                        file("f2", "logo.png", include_bytes!("../../assets/weave_logo.png")),
                    ],
                ),
                message("a1", "assistant", NOW - 100_000, vec![text("a1-t", "The first one: it keeps the cart in the header.")]),
                message("u2", "user", NOW - 60_000, vec![file("f3", "cart.png", PAGE)]),
            ]
        } else {
            let shot = call(
                "c1",
                "fleet_browser_screenshot",
                "completed",
                json!({"viewport": "1280x800"}),
                json!({
                    "title": "Shop · 1280×800",
                    "output": "Screenshot of http://localhost:5173/ at 1280×800.",
                    "metadata": {"screenshot": {"sessionId": "scene", "id": "shot_1", "width": 1280, "height": 800}},
                }),
            );
            vec![
                message("u1", "user", NOW - 120_000, vec![text("u1-t", "Open the shop and show me the front page.")]),
                message(
                    "a1",
                    "assistant",
                    NOW - 100_000,
                    vec![shot, text("a1-t", "Here's the page: the spring collection, six products, the cart at the top right.")],
                ),
            ]
        };
        let snapshot = json!({"session": {}, "messages": messages, "delegations": [], "activityStatus": "idle"});
        self.stream = Some(crate::fleet::reducer::create_state(&snapshot));
        if let Some(session) = self.projects.first_mut().and_then(|p| p.sessions.first_mut()) {
            session.status = Status::Idle;
            session.row.activity = "idle".into();
        }
        self.now_ms = Some(NOW);
        self.stream_of = None;
        if case == Case::FirstMessage {
            let image = crate::fleet::prompts::SentImage {
                mime: "image/png".into(),
                filename: Some("login-error.png".into()),
                data: base64(FIRST_PICTURE).into(),
            };
            let body =
                "Pick up where @fix-pagination left off: the retry in @src/billing/retry-policy.py still fails, as the picture shows.";
            let id = self.session().id.clone();
            let references = vec![("@fix-pagination".to_string(), "ses-pagination".to_string())];
            self.prompts.add_with(&id, body, NOW - 5_000, 0.0, vec![image], references);
        }
        self.sync_open();
        match case {
            Case::FirstMessage => {
                let id = self.session().id.clone();
                if let Some(prompt) = self.prompts.of(&id).first() {
                    self.supply_image(&format!("img-optimistic-{}-img-0", prompt.correlation_id), FIRST_PICTURE);
                }
            }
            Case::Shot | Case::Lightbox => self.supply_image(SHOT_PATH, PAGE),
            Case::Lost => self.on_image_bytes(SHOT_PATH, Err("HTTP 404".into())),
            Case::Attachments => {
                for (key, bytes) in [("img-f1", PAGE), ("img-f2", include_bytes!("../../assets/weave_logo.png")), ("img-f3", PAGE)] {
                    self.supply_image(key, bytes);
                }
            }
        }
        if case == Case::Lightbox {
            self.open_lightbox(SHOT_PATH, "Screenshot: Shop · 1280×800", "shot-c1");
        }
    }
}
