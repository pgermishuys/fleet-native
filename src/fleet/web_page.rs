//! Where a page canvas's page is (component 14, piece A; rows 14.20 and 14.21). Ports the address half of
//! `lib/server-canvas.ts`: `shownPage` for the fields the address and the reload need, and `pageAddress`. The rest of
//! `shownPage` (files, bytes, warnings, label) is piece C's, with the bar that shows them.
use crate::conv::image_sniff::encode_component;
use serde_json::Value;

/// The page an agent showed: an HTML file Fleet copied and serves at `/pages/{pageId}/{entry}`. `shown_at` changes every
/// time the agent shows it again, so the tab loads it again.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ShownPage {
    pub page_id: String,
    pub entry: String,
    pub source: String,
    pub shown_at: String,
}

/// `shownPage`: a page canvas's `state`, with anything missing or of the wrong type empty.
pub fn shown_page(state: &Value) -> ShownPage {
    let text = |key: &str| state.get(key).and_then(Value::as_str).unwrap_or_default().to_string();
    ShownPage { page_id: text("pageId"), entry: text("entry"), source: text("source"), shown_at: text("shownAt") }
}

/// `pageAddress`: the page's path on Fleet, the id and each part of the file name escaped (`encodeURIComponent`).
pub fn page_address(page: &ShownPage) -> String {
    let entry: Vec<String> = page.entry.split('/').map(encode_component).collect();
    format!("/pages/{}/{}", encode_component(&page.page_id), entry.join("/"))
}

/// The page's whole address on the Fleet at `base` (`apiUrlOn`), or `None` for a canvas that names no page.
pub fn page_url(base: &str, page: &ShownPage) -> Option<String> {
    if page.page_id.is_empty() || page.entry.is_empty() {
        return None;
    }
    Some(format!("{}{}", base.trim_end_matches('/'), page_address(page)))
}
