//! The script every page gets at document start (component 14, piece B; decision 3, rows 14.28, 14.30 and 14.33). It runs
//! before Fleet's own script in the page, which is why the page reads the theme from `window.name` before it draws.
//!
//! In Fleet a page runs in an iframe and talks to its parent with `parent.postMessage`. In a web view of its own the page
//! is the top level, so `parent === window` and Fleet's script's call lands on `window.postMessage`: wrapped here, so a
//! Fleet message goes to the app over IPC instead of back to the page. Messages to the page are handed in through
//! `__fleetDeliver`, which calls the original `postMessage`; Fleet's script accepts them because `event.source === parent`.
use serde_json::Value;

/// What the page tells the app besides Fleet's messages: it took the keyboard, or finished loading.
const SCRIPT: &str = r#"(function () {
  if (window.__fleetHost) return;
  window.__fleetHost = true;
  var ipc = function (o) { try { window.ipc.postMessage(JSON.stringify(o)); } catch (e) {} };
  var host = function (kind) { ipc({ host: kind }); };
  // The theme, before the page draws; a reload of the same page keeps the name, a theme change sets it again.
  if (String(window.name).indexOf("fleet:") !== 0) window.name = __NAME__;
  var original = window.postMessage.bind(window);
  var isFleet = function (m) {
    if (m === null || typeof m !== "object") return false;
    if (typeof m.type === "string" && m.type.indexOf("fleet:") === 0) return true;
    return m.jsonrpc === "2.0" && m.method === "ui/notifications/size-changed";
  };
  window.postMessage = function (message, a, b) {
    if (isFleet(message)) { ipc({ page: message }); return; }
    return original(message, a, b);
  };
  window.__fleetDeliver = function (message) { original(message, "*"); };
  window.addEventListener("focus", function () { host("focus"); }, true);
  window.addEventListener("DOMContentLoaded", function () { host("loaded"); });
})();"#;

/// The script for a page whose theme is `frame_name` (`PageTheme::frame_name`).
pub fn script(frame_name: &str) -> String {
    SCRIPT.replace("__NAME__", &Value::String(frame_name.to_string()).to_string())
}

/// Runs in a page to hand it a message of Fleet's.
pub fn deliver(message: &Value) -> String {
    format!("window.__fleetDeliver && window.__fleetDeliver({message})")
}

/// Runs in a page after a theme change: the name again (it survives the page's reloads) and the message.
pub fn retheme(frame_name: &str, message: &Value) -> String {
    format!("window.name = {}; {}", Value::String(frame_name.to_string()), deliver(message))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sets_the_name_and_forwards_only_fleets_messages() {
        let s = script("fleet:{\"place\":\"tab\"}");
        assert!(s.contains("window.name = \"fleet:{\\\"place\\\":\\\"tab\\\"}\""), "{s}");
        assert!(s.contains("m.type.indexOf(\"fleet:\") === 0"));
        assert!(s.contains("ui/notifications/size-changed"));
        // Anything else goes to the page's own postMessage.
        assert!(s.contains("return original(message, a, b)"));
        assert!(!s.contains("__NAME__"));
    }

    #[test]
    fn a_message_to_the_page_goes_around_the_wrapper() {
        let js = deliver(&serde_json::json!({"type": "fleet:theme"}));
        assert!(js.contains("__fleetDeliver({\"type\":\"fleet:theme\"})"));
    }
}
