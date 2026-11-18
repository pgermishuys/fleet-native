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

/// The preview bridge for a browser tab's page (decision 4, rows 14.54 and 14.55): Fleet's `preview-bridge.js`, protocol v1,
/// which the gateway adds to every previewed page but which returns at once when `window.top === window`, as it always is in a
/// web view of its own. The same messages go to the app over IPC instead of `parent.postMessage`; Back, Forward and Reload are
/// run from the app (`browser_navigate`), so no `nav` message is listened for.
const PREVIEW_SCRIPT: &str = r#"(function () {
  if (window.__fleetPreviewHost) return;
  window.__fleetPreviewHost = true;
  var UPDATE = /"type"\s*:\s*"(update|UpdateStaticFile|ApplyManagedCodeUpdates)"|"action"\s*:\s*"built"/;
  var STYLE_AFTER_MESSAGE_MS = 1500;
  var socketHmr = null, lastHmrMessage = 0, updateTimer = 0;
  function send(type, fields) {
    var m = fields || {};
    m.fleet = 1;
    m.type = type;
    try { window.ipc.postMessage(JSON.stringify({ preview: m })); } catch (e) {}
  }
  function reportLocation() { send("location", { href: location.href, title: document.title }); }
  function reportUpdate() { clearTimeout(updateTimer); updateTimer = setTimeout(function () { send("update"); }, 150); }
  function hmrOfSocket(url, protocols) {
    if ([].concat(protocols || []).indexOf("vite-hmr") >= 0) return "vite";
    if (url.indexOf("/_next/webpack-hmr") >= 0) return "next";
    if (url.indexOf("/_bun/hmr") >= 0) return "bun";
    if (url.indexOf("/__fleet_browser/ws/") >= 0) return "dotnet-watch";
    return null;
  }
  function hmrOfPage() {
    if (socketHmr) return socketHmr;
    for (var i = 0; i < document.scripts.length; i++) {
      var src = document.scripts[i].src || "";
      if (src.indexOf("/@vite/client") >= 0) return "vite";
      if (src.indexOf("/_next/") >= 0) return "next";
      if (src.indexOf("/_bun/") >= 0) return "bun";
      if (src.indexOf("aspnetcore-browser-refresh.js") >= 0) return "dotnet-watch";
    }
    for (var key in window) { if (key.indexOf("webpackHotUpdate") === 0) return "webpack"; }
    return "none";
  }
  var NativeSocket = window.WebSocket;
  if (NativeSocket) {
    var FleetSocket = function (url, protocols) {
      var socket = protocols === undefined ? new NativeSocket(url) : new NativeSocket(url, protocols);
      try {
        var hmr = hmrOfSocket(String(url), protocols);
        if (hmr) {
          socketHmr = socketHmr || hmr;
          socket.addEventListener("message", function (event) {
            lastHmrMessage = Date.now();
            if (typeof event.data === "string" && UPDATE.test(event.data)) reportUpdate();
          });
        }
      } catch (e) {}
      return socket;
    };
    FleetSocket.prototype = NativeSocket.prototype;
    FleetSocket.CONNECTING = 0; FleetSocket.OPEN = 1; FleetSocket.CLOSING = 2; FleetSocket.CLOSED = 3;
    window.WebSocket = FleetSocket;
  }
  var LOOPBACK_HOST = /^(localhost|[a-z0-9-]+\.localhost|127(\.\d{1,3}){3}|\[::1\]|0\.0\.0\.0|\[::\])$/i;
  function elsewhereOnMachine(href) {
    var url;
    try { url = new URL(href, location.href); } catch (e) { return null; }
    if (url.protocol !== "http:" && url.protocol !== "https:") return null;
    return url.origin !== location.origin && LOOPBACK_HOST.test(url.hostname) ? url.href : null;
  }
  function linkOf(event) {
    var path = event.composedPath ? event.composedPath() : [event.target];
    for (var i = 0; i < path.length; i++) {
      var node = path[i];
      if (node && node.nodeName === "A" && node.href && !node.hasAttribute("download")) return node;
    }
    return null;
  }
  function opensNewTab(link, event) {
    var target = (link.getAttribute("target") || "").toLowerCase();
    return event.button === 1 || event.ctrlKey || event.metaKey || event.shiftKey
      || (target !== "" && target !== "_self" && target !== "_parent" && target !== "_top");
  }
  function onLinkClick(event) {
    if (event.defaultPrevented || event.button > 1) return;
    var link = linkOf(event);
    var href = link && elsewhereOnMachine(link.href);
    if (!href) return;
    event.preventDefault();
    send("open", { href: href, newTab: opensNewTab(link, event) });
  }
  addEventListener("click", onLinkClick);
  addEventListener("auxclick", onLinkClick);
  var nativeOpen = window.open;
  window.open = function (url) {
    var href = url === undefined || url === null ? null : elsewhereOnMachine(String(url));
    var activated = !navigator.userActivation || navigator.userActivation.isActive;
    if (!href || !activated) return nativeOpen.apply(window, arguments);
    send("open", { href: href, newTab: true });
    return null;
  };
  function isStyle(node) {
    return !!node && (node.nodeName === "STYLE" || (node.nodeName === "LINK" && /stylesheet/i.test(node.rel || "")));
  }
  function watchStyles() {
    if (!window.MutationObserver || !document.head) return;
    new MutationObserver(function (records) {
      if (Date.now() - lastHmrMessage > STYLE_AFTER_MESSAGE_MS) return;
      for (var i = 0; i < records.length; i++) {
        var record = records[i];
        var target = record.target.nodeType === 3 ? record.target.parentNode : record.target;
        var changed = isStyle(target);
        for (var j = 0; !changed && j < record.addedNodes.length; j++) changed = isStyle(record.addedNodes[j]);
        for (var k = 0; !changed && k < record.removedNodes.length; k++) changed = isStyle(record.removedNodes[k]);
        if (changed) { reportUpdate(); return; }
      }
    }).observe(document.head, { childList: true, subtree: true, characterData: true, attributes: true, attributeFilter: ["href"] });
  }
  ["pushState", "replaceState"].forEach(function (name) {
    var original = history[name];
    history[name] = function () { var result = original.apply(this, arguments); reportLocation(); return result; };
  });
  addEventListener("popstate", reportLocation);
  addEventListener("hashchange", reportLocation);
  addEventListener("load", reportLocation);
  // Back to a page the engine kept whole (its page cache) runs no script again, so only this says where it is.
  addEventListener("pageshow", function (e) { if (e.persisted) reportLocation(); });
  addEventListener("focus", function () { try { window.ipc.postMessage(JSON.stringify({ host: "focus" })); } catch (e) {} }, true);
  function hello() { send("hello", { hmr: hmrOfPage(), href: location.href, title: document.title }); watchStyles(); }
  if (document.readyState === "loading") document.addEventListener("DOMContentLoaded", hello); else hello();
})();"#;

/// The script for a browser tab's page.
pub fn preview_script() -> &'static str {
    PREVIEW_SCRIPT
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
