//! The app's side of Fleet's preview bridge, protocol v1 (component 14, piece G; row 14.54). Ports `lib/preview-bridge.ts`.
//! The gateway adds a script to every previewed page; it and the tab talk with messages, because the page is another
//! origin. Types and versions either side doesn't know are ignored, so later types can be added. Plain Rust: piece H wires it
//! to the web view.
use serde_json::{Value, json};

pub const BRIDGE_VERSION: i64 = 1;

/// The page's hot-reload client, as the bridge found it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreviewHmr {
    Vite,
    Next,
    Bun,
    Webpack,
    DotnetWatch,
    None,
}

impl PreviewHmr {
    fn read(name: &str) -> PreviewHmr {
        match name {
            "vite" => PreviewHmr::Vite,
            "next" => PreviewHmr::Next,
            "bun" => PreviewHmr::Bun,
            "webpack" => PreviewHmr::Webpack,
            "dotnet-watch" => PreviewHmr::DotnetWatch,
            _ => PreviewHmr::None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BridgeMessage {
    /// Once per page load.
    Hello { hmr: PreviewHmr, href: String, title: String },
    /// After every navigation.
    Location { href: String, title: String },
    /// The page's hot reload applied a change without reloading.
    Update,
    /// A link to another address on Fleet's machine, which the tab opens through Fleet: in a new tab, or in this one.
    Open { href: String, new_tab: bool },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavAction {
    Back,
    Forward,
    Reload,
}

impl NavAction {
    fn name(self) -> &'static str {
        match self {
            NavAction::Back => "back",
            NavAction::Forward => "forward",
            NavAction::Reload => "reload",
        }
    }
}

/// `readBridgeMessage`: the bridge message in `data`, or `None` for anything else (another version, another sender, a
/// malformed one).
pub fn read_bridge_message(data: &Value) -> Option<BridgeMessage> {
    let message = data.as_object()?;
    if message.get("fleet").and_then(Value::as_i64) != Some(BRIDGE_VERSION) {
        return None;
    }
    let text = |key: &str| message.get(key).and_then(Value::as_str).map(str::to_string);
    let href = text("href");
    let title = text("title").unwrap_or_default();
    match message.get("type").and_then(Value::as_str)? {
        "hello" => Some(BridgeMessage::Hello {
            hmr: message.get("hmr").and_then(Value::as_str).map_or(PreviewHmr::None, PreviewHmr::read),
            href: href?,
            title,
        }),
        "location" => Some(BridgeMessage::Location { href: href?, title }),
        "update" => Some(BridgeMessage::Update),
        "open" => Some(BridgeMessage::Open { href: href?, new_tab: message.get("newTab") == Some(&Value::Bool(true)) }),
        _ => None,
    }
}

/// `navMessage`: what the tab sends the page to go back, forward or reload.
pub fn nav_message(action: NavAction) -> Value {
    json!({"fleet": BRIDGE_VERSION, "type": "nav", "action": action.name()})
}

/// `appAddress`: the app's own address for a page the preview shows, the preview's origin swapped for the app's, so the
/// address bar reads `http://localhost:5173/cart` and not the preview's port.
pub fn app_address(preview_href: &str, preview_origin: &str, app_origin: &str) -> String {
    match preview_href.strip_prefix(preview_origin) {
        Some(rest) => format!("{app_origin}{rest}"),
        None => preview_href.to_string(),
    }
}
