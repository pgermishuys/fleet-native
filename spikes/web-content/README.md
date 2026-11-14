# Spike: web content in a Rune window

Research for `docs/PORT.md`'s open decision 1. Never merged. The findings are in
`docs/research/web-content.md` on the research branch.

Two binaries:

| Binary | Route | Build |
|---|---|---|
| `web-content-spike` | The system web view (wry: WebView2, WKWebView, WebKitGTK) as a child of Rune's window, kept over a Rune panel | `cargo run` |
| `osr` | Chromium (CEF) rendering off screen into a `rune::Pixels` inside a scrolling Rune list | `cargo run --features osr --bin osr` (downloads CEF; set `CEF_PATH`) |

Try in `web-content-spike`: drag the gutter, resize the window, click **Menu** (a Rune popover over
the page), turn on **Hide under menus**, click **Add a tide card** in the page (the left panel
hears it over the bridge), switch **Page: dark/light** (the app pushes theme variables in), and
type in both the page's field and the Rune field to see where the keyboard goes.

`--demo` runs the steps by itself and runs `$SPIKE_SHOT` (with `{name}`) after each, so CI can
take pictures; `.github/workflows/spike-web-content.yml` does that on Windows and macOS.

Linux needs WebKitGTK 4.1 (`libwebkit2gtk-4.1-dev`) and an X11 session (or XWayland:
`WAYLAND_DISPLAY= cargo run`); wry's child web view doesn't work on Wayland.
