//! Invented documents the agent rendered, for the tool card's box and the Document tab (component 14, piece E): the same JSON a
//! tool prints, read by the same code a live conversation goes through.
use super::tools::{call, conversation, message};
use crate::State;
use crate::fleet::canvases::{VisualPayload, VisualType};
use serde_json::{Value, json};

pub const NOTES: &str = "# Cache eviction\n\nThe image cache keeps the **last 200** entries and drops the rest when a new one arrives.\n\n## Why\n\n- Memory stays flat while scrolling long lists.\n- A hit is one lookup, a miss one decode.\n\n```rust\nfn evict(cache: &mut Cache) {\n    while cache.len() > 200 {\n        cache.pop_oldest();\n    }\n}\n```\n\n| Size | Hit rate |\n|---|---|\n| 100 | 71 % |\n| 200 | 88 % |\n";

pub const TOTALS: &str = "<html><head><style>body{font-family:sans-serif;margin:0;padding:24px;background:#fff;color:#222}\
h1{font-size:20px;margin:0 0 12px}table{border-collapse:collapse;width:100%}td,th{border-bottom:1px solid #ddd;padding:8px;text-align:left}\
.total{font-weight:700;background:#eef4ff}</style></head><body><h1>Order totals</h1>\
<table><tr><th>Item</th><th>Qty</th><th>Price</th></tr><tr><td>Notebook</td><td>3</td><td>$12.00</td></tr>\
<tr><td>Pencil set</td><td>2</td><td>$7.50</td></tr><tr class=\"total\"><td>Total</td><td>5</td><td>$19.50</td></tr></table></body></html>";

fn output(kind: &str, title: Option<&str>, content: &str) -> Value {
    let mut payload = json!({"$type": kind, "content": content});
    if let Some(title) = title {
        payload["title"] = json!(title);
    }
    json!({"output": payload.to_string()})
}

/// A markdown payload and an HTML one printed by two calls, with the words around them.
pub fn of_visuals() -> Vec<crate::view::conversation::Message> {
    let ok = "completed";
    conversation(vec![
        message(
            "u1",
            "user",
            1,
            vec![json!({"type": "text", "id": "u1-t", "text": "Write up how the cache evicts, and show the totals table."})],
        ),
        message(
            "a1",
            "assistant",
            2,
            vec![
                call("v1", "bash", ok, json!({"command": "render notes"}), output("markdown", Some("Cache eviction"), NOTES)),
                call("v2", "bash", ok, json!({"command": "render totals"}), output("html", Some("Order totals"), TOTALS)),
            ],
        ),
    ])
}

pub fn payload(kind: VisualType, title: Option<&str>) -> VisualPayload {
    let content = if kind == VisualType::Html { TOTALS } else { NOTES };
    VisualPayload { kind, title: title.map(str::to_string), source_path: None, content: content.to_string() }
}

impl State {
    /// The conversation with both calls open, their boxes showing.
    pub fn show_visual_calls(&mut self, dark: bool) {
        self.dark = Some(dark);
        self.show_conversation(of_visuals());
        for id in ["v1", "v2"] {
            self.tool_open.insert(id.into(), crate::conv::card::Toggle { open: true, under: true });
        }
    }

    /// The Document tab of a payload, in the open panel.
    pub fn show_document(&mut self, payload: VisualPayload, dark: bool) {
        self.dark = Some(dark);
        self.show_conversation(of_visuals());
        self.sidebar_open = false;
        self.right.widened = true;
        self.expand_visual(payload);
    }
}
