# Component 14: Web content (checklist and pieces)

Planned on 17 November 2026, from weave-fleet `main` at commit `cd13d91fa5e93f4b5a40ed8113d6aa325d556003` (read in a shallow clone).
The released server the checks run against is v0.72.0; every route and message in this file was there (checked live: a page canvas with
a waiting pick from the scripted model's `show-page`, `GET /api/sessions/{id}/canvases`, `POST /api/sessions/{id}/browser`, the preview
gateway showing a local site). Decision 1 of `docs/PORT.md` governs: **`wry`** (the system web view as a child of the window) for the
places that are whole tabs; pages inside the conversation and HTML previews inside lists become cards.

**Why it is risky, and what goes first.** Nothing else in the app puts a native window over Rune's surface. The research spike
(`docs/research/web-content.md`, branch `spike/web-content`) showed it works on all three systems at Rune `f1f54ce8`, which is still the pin.
But that was a separate binary with an invented page. **Piece A is a thin end-to-end spike in the real app**: one page canvas from a scratch
Fleet, live in the content panel, sized and moved with it, hidden under a Rune menu, keyboard in and out, on Linux, Windows and macOS (CI).
Nothing else is built until it is merged. The owner uses page canvases all day (proposals, mockups), so the page tab comes right after it.

Sources (weave-fleet `client/src/`): `components/canvas/PageCanvas.vue` (PC), `BrowserCanvas.vue` (BC), `BrowserOpenDialog.vue` (BOD),
`VisualCanvas.vue` (VC), `components/visual-renderers/HtmlRenderer.vue` (HR), `MarkdownRenderer.vue` (MDR), `components/session/ConversationPage.vue` (CP),
`components/mods/ModPage.vue` (MP), `lib/page-bridge.ts` (PB), `lib/page-theme.ts` + `lib/page-theme-core.ts` (PT), `lib/page-choice.ts` (PCH),
`lib/server-canvas.ts` (SC), `lib/preview-bridge.ts` (PVB), `lib/browser-target.ts` (BT), `lib/visual-payload.ts` (VP), `stores/app-runs.ts` (AR),
`composables/use-server-canvases.ts` (USC), `components/session/ToolCard.vue` (TC), `MessageToolList.vue` (MTL), `activity-stream-tool-card.ts` (ATC),
`ActivityStream.vue` (AS). Server: `src/WeaveFleet.Application/Pages/PageTheme.cs` (the script every page gets), `src/WeaveFleet.Api/Endpoints/PageEndpoints.cs`,
`src/WeaveFleet.Api/Browser/PreviewGateway.cs` and `preview-bridge.js`.

## What the native app has today (looked at on 17 November 2026)

- **Page canvases are read, not shown.** `src/fleet/pages.rs` (component 4, piece F) loads `GET /api/sessions/{id}/canvases` and applies
  `canvas.updated`/`canvas.closed`, but keeps only `{id, title, version, choice}` of page canvases: no `pageId`, `entry`, `source`, `warnings`,
  `label` or `shownAt`, so there is no address to load. `State::open_page` (the page-pick card's **Open page**) stores `page_target` and opens
  the panel, which then shows the Plan list (`native-opened-*.png`).
- **The content panel is a prototype** (`src/view/sheet.rs::content`): Plan and Context, no tab strip, no canvas model. Component 10 builds those.
- `conv/message_list.rs` marks a call that showed a page in the conversation (`tool.page`), so folding keeps it visible; nothing is drawn for it.
- `conv/browser.rs` already has `browser_target` (BT, ported with `browser-target.test.ts`'s cases in `conv/tests/browser.rs`) and the browser
  steps under a call (component 3, piece O). `fleet/events.rs` knows `canvas.focused` and `app.updated`; nothing handles them.
- A tool card whose output is a visual payload (component 3's row 3.19) draws its output as text. No `parseVisualPayload`.
- No web view crate, no `src/web/`. `Tx::open_url` exists (smart-link cards use it) for "Open in a new tab".
- Rune at `f1f54ce8` has what the host stands on (checked in Cargo's checkout): `App::on_window`, `WindowHandle::with_raw_handle`, `El::on_layout`.
- Theme tokens for the page variables all exist in `src/theme.rs` (`panel_bg`, `card_bg`, `text`, `muted`, `border`, `accent`, `accent_dim`,
  `primary_fg`, `running`, `idle`, `error`, `complete`, `RADIUS_CARD`).

## Seams with components 10, 11, 12 and 22 (agreed with component 10's planner, 17 November 2026)

| Who | Owns |
|---|---|
| 10 | The agent's browser frames (`fleet::agent_browser`: tabs, `tab_for`, the 700 ms frame clock; `view::agent_tab::agent_tab`). The canvas model `src/fleet/canvases.rs` (per-session tabs, active id, open/introduce/activate/close, `open_visual`, `widened`, the tab pulse `mark_updated(tab_id)`), the server-canvas list (`GET /canvases`, `canvas.*` events, `DELETE`, `POST …/focus`, generalising `fleet/pages.rs`; `PageCanvas`/`waiting_page` move onto it), each server canvas kept as `{id, kind, title, version, state: Value}`; the strip, + menu, Widen and body dispatch (`src/view/canvas_host.rs`); `State::active_canvas()` and `State::panel_body_shown()`; Agents and `AgentTabView` |
| 14 | The bodies of page, browser and Document tabs: `view::page_tab::body(s, t, &ServerCanvas)`, `view::browser_tab::body(s, t, &ServerCanvas)`, `view::document_tab::body(s, t, &VisualPayload)`; the readers of a server canvas's `state` (`shown_page`, `browser_page`); the web view host `src/web/`; the page and preview bridges; app runs and `app.updated` (calling 10's `mark_updated`); `BrowserOpenDialog` with `State::open_browser_dialog()` (10 draws the + menu's "Browser…" row once that exists); `parseVisualPayload`, the known-visuals list and the live auto-open (10 draws "From this conversation" from that list); the conversation's page cards and the tool card's visual |
| 11 | The file tab and its Rendered view; for a `.html` file it puts 14's `web::html_body(s, t, key, content)` in its body |
| 12 | The review list; a rendered `.html` file there is 14's `view::web_card::html_card` (title, Expand to the Document tab) |
| 22 | Mod pages and panes; a mod's page is 14's conversation page card with `contained` set, at the address 22 builds (`modPageUrl`) |

Until 10's canvas host lands, piece A mounts on today's panel: while `page_target` names a page canvas, `sheet.rs::content` draws
`page_tab::body` in place of the Plan list. 10's host later replaces that one branch with its dispatch; nothing else in 14 changes.

## Decisions (the owner can overturn)

1. **Where a web view goes.** A live web view (wry) for: the page tab, the browser tab, the Document tab of an HTML payload, a rendered `.html`
   file (11), and the full-size view of a conversation page (inside a Rune dialog frame, the only thing in it). A **card** for: a page in the
   conversation, a mod page, an HTML preview in a tool card, a rendered `.html` file in the review list (Decision 1). A markdown payload is drawn
   by Rune (`rune-kit` Markdown) everywhere, inline in the tool card as Fleet does: no web view.
2. **The conversation's page card** (new, not Fleet's): the call's title, the page's file name, **Open full size** (Fleet's Maximize2 button,
   which opens Fleet's own modal) and **Open in a new tab** (system browser; not for `contained`). Fleet's frame sized to the page's reported
   height (120 px at first, at most 2,000) has no native equal; the card is 72 px. A still snapshot on the card is possible later and is not planned.
3. **The page bridge goes through wry's IPC with a script the host injects** at document start, before Fleet's own script:
   it sets `window.name` to `fleet:{place, appearance, variables}` (Fleet's frame name, so the page draws in the right theme first time), and
   wraps `window.postMessage` so a message that is a Fleet page message (`type` starting `fleet:`, or the `ui/notifications/size-changed`
   JSON-RPC) goes to the app instead of the page. At top level `parent === window`, so Fleet's script's `parent.postMessage` lands in that
   wrapper unchanged. Messages to the page (`fleet:page-state`, `fleet:theme`) go with `evaluate_script("window.postMessage(…, '*')")`, which
   Fleet's script accepts because `event.source === parent`. A theme change also re-sets `window.name`, which survives the page's reloads.
   Each web view has its own IPC handler, so "only this tab's own frame" (PC's `event.source` check) holds by construction.
4. **The preview bridge is the host's own copy.** Fleet's `preview-bridge.js` returns at once when `window.top === window`, which it always is
   in a top-level web view, so in a native browser tab the gateway's copy never runs. The host injects a port of it (same protocol v1, `send`
   through wry's IPC, `nav` through `evaluate_script`) into browser tabs only. Not a Fleet bug: Fleet's script is right for an iframe. Worth
   telling Fleet if it wants native hosts to reuse its file (a flag such as `window.__fleetHost` that skips the top check).
5. **One web view per open web tab, made when the tab is first shown.** Page tabs drop their web view when another tab becomes active (Fleet
   reloads them on return too: `KeepAlive` keeps the component but the page's state comes back through `fleet:page-hello`, and pages are cheap);
   browser tabs keep theirs while open, at most 8 per window like Fleet's `KeepAlive :max="8"`, the least recently shown dropped first.
   Switching session hides every web view of the old one; closing a tab or the session's view drops them.
6. **Hidden means moved out of the window** (`set_bounds` far off screen), not `set_visible(false)`, on every system: Linux shows a hidden view
   black when it comes back (spike), and one rule is simpler to test.
7. **Hidden under every Rune overlay.** A web view is hidden while `s.ui.open` is set, while any dialog of `ui::overlays` is up, while the
   lightbox is open, and while a floating overlay (toast, notice card, autocomplete, tooltip) would cover any part of its rectangle. Overlays
   that never reach the panel (the composer's pickers) still hide it in piece A; piece D narrows this to "overlaps".
8. **Fleet's shortcuts don't reach the app while a page has the keyboard.** That is Fleet's behaviour too: a focused cross-origin iframe keeps
   its keys. Nothing is forwarded. Clicking anywhere in Rune gives the keyboard back to Rune (Windows needs `focus_parent()`; Linux must not call
   it, it loses the keyboard; macOS is found out in piece A).
9. **When a web view can't be made** (Linux without WebKitGTK, a Wayland session without XWayland, Windows without the WebView2 runtime, or the
   app built without the `web` feature), the tab's body is a Rune panel: "This page can't be shown inside the app on this system." with
   **Open in browser** (the page's address, `Tx::open_url`). The same panel is what headless tests, scenes and `--snap` draw, so they stay the same
   on every run. This is the fallback if a system can't host the web view at all; that system then works as Fleet in a browser tab would.
10. **A cargo feature `web`, on by default**, holds wry (and `gtk` on Linux). `--no-default-features` builds the fallback only. CI builds with
    it on all three systems; Linux CI installs `libwebkit2gtk-4.1-dev`. Linux packages (component 25) then need `libwebkit2gtk-4.1-0`;
    Windows installers carry Microsoft's WebView2 bootstrapper. Say so in the README's requirements.
11. **HTML payloads run no scripts**, as Fleet's `sandbox="allow-same-origin"` `srcdoc` frame: the host loads the content with
    `load_html` after putting `<meta http-equiv="Content-Security-Policy" content="script-src 'none'">` at the start of its head.
12. **"Open in a new tab" is the system browser** everywhere (`Tx::open_url`). Pop-ups a page opens (`allow-popups`) and downloads
    (`allow-downloads`) go there too: wry's new-window handler opens the address in the system browser and refuses the window.

## Rows

Owner: `14` = this component, with the piece; another number = that component. "Native" says what exists on 17 November 2026.

### 1. The web view host (`src/web/`)

| # | Row | Source | Native today | Owner |
|---|---|---|---|---|
| 14.1 | A web view is a child of the Rune window: `App::on_window` hands the native handle (again after the window is remade), `with_raw_handle` lends it to `WebViewBuilder::build_as_child`; one web view per web tab, keyed by tab id | research spike | **Done (A):** `web::platform`: `on_window` keeps the handle, `build_as_child` makes the view when the tab is first shown, one per tab id | 14 A |
| 14.2 | It is placed from its body's `El::on_layout` (window points = wry's logical size): it follows the gutter drag, a window resize, Widen's 200 ms ease, the panel opening and closing, and the narrow layout's panel over the conversation | spike; `AppShell.vue` | **Done (A) for the gutter and a window resize**: the frame's `on_layout` → `set_bounds` (Linux walk, CI grabs). Widen, the narrow layout and the panel toggle are D's | 14 A (gutter, resize), D (the rest) |
| 14.3 | It is on screen only while its tab is the active tab of the open panel, on the Sessions route, of the open session; otherwise it is moved out of the window (decision 6) | PC/BC mounted only when active | **Done (A)** for the route, the panel and the page target (`web::wanted`, tests in `tests_web.rs`); session switches are D's | 14 A, D |
| 14.4 | Hidden while a Rune overlay is open over it (decision 7); back where it was, with its page as it was, when it closes | spike | **Done (A)** for `ui.open` (`Wanted::Hidden`; the palette over the page in the walk, the page back with its script still running); the other overlays are D's | 14 A (`ui.open`), D (all) |
| 14.5 | Linux: `gtk::init()` before the first web view; GTK's loop turned from `poll` every 16 ms while any web view exists, not at all otherwise | spike | **Done (A):** `gtk::init` in `on_window`; `poll` turns GTK every 16 ms while a view exists, not at all before | 14 A |
| 14.6 | Windows: the UI thread is a single-threaded COM apartment before WebView2 starts | spike | **Done (A):** `CoInitializeEx(APARTMENTTHREADED)` in `on_window` (CI) | 14 A |
| 14.7 | Keyboard: a click in the page gives it the keys; a press anywhere in Rune takes them back (decision 8), per system | spike (Windows, Linux tried) | **Shown (A):** the host script tells the app when the page takes focus (IPC `{"host":"focus"}`); Rune's focus is blurred; when a Rune element takes focus again the host calls `focus_parent` (Windows and macOS, not Linux). Linux walk: page got "tides", the composer "harbour". Windows and macOS: see the CI grabs in the PR | 14 A (shown), D (all of Rune's focusable things) |
| 14.8 | Fleet's shortcuts while the page has the keys: none reach the app (as Fleet) | iframe behaviour | n/a | 14 D (checked, written down) |
| 14.9 | The fallback panel when no web view can be made (decision 9): words, **Open in browser**; drawn by headless runs and scenes | ours | **Done (A):** `view::page_tab` fallback, scene `page-tab-fallback`; what headless runs draw | 14 A |
| 14.10 | Before the page draws, the area is white (`.page-canvas__frame { background: #fff; color-scheme: light }`), not the theme's ground | PC, BC, HR | **Done (A)** in Rune (`Theme::page_frame`) and wry (`with_background_color` white). **Open on Linux:** WebKitGTK's view is black until its first paint (about 1-2 s on a software renderer); GTK's container, which wry doesn't expose | 14 A |
| 14.11 | Scale: bounds right at 100 %, 150 % and 200 % (Windows' scale setting, Retina) | spike risk | None | 14 D (100 % in CI; others by hand, or "not checked") |
| 14.12 | A page's `alert`/`confirm`/`prompt` (`allow-modals`) show as the system's dialogs | PC sandbox | None | 14 D (per system, written down) |
| 14.13 | Pop-ups and downloads go to the system browser (decision 12) | PC sandbox | None | 14 D |
| 14.14 | Web views are dropped when their tab closes, their session's view goes, or (page tabs) another tab becomes active (decision 5); no web view outlives the app's window | ours | None | 14 D |
| 14.15 | The page's own right-click menu is the system web view's; developer tools off in release builds | ours | None | 14 D |
| 14.16 | Another machine's session: the page's address is on that machine's base (`apiUrlOn(machine.connection, …)`); `/pages` is anonymous, so no token is needed | PC; `PageEndpoints.cs` `AllowAnonymous` | n/a | 14 C |

### 2. The page tab (PC, PB, PT, PCH, SC)

| # | Row | Source | Native today | Owner |
|---|---|---|---|---|
| 14.20 | `shownPage(state)`: `pageId`, `entry`, `source`, `files`, `bytes`, `shownAt`, `warnings` (strings only), `label` (non-empty), `choice` (`readPageChoice`); missing ones empty | SC | Address fields **done (A):** `fleet::web_page::shown_page` (`pageId`, `entry`, `source`, `shownAt`), held on `PageCanvas::page`; the rest is C's | 14 A (address fields), C (all) |
| 14.21 | Address: `/pages/{pageId}/{entry}`, the id and each segment of `entry` percent-encoded, on the session's machine | SC `pageAddress` | **Done (A):** `page_address`, `page_url` on home's base (another machine's base is 14.16, C) | 14 A |
| 14.22 | Bar: 40 px, padding 0 8, gap 2, border-bottom `--border`. Reload icon button (28×28, `RotateCw` 14, `--muted`, hover `--accent-dim` and `--text`, title and label "Reload") | PC | None | 14 C |
| 14.23 | Source: the `label` if set (12 px, `--text`, 500), else the file name (`source`'s last part, or `entry`) then the whole `source` in mono 11.5 `--muted` with an ellipsis; margin 0 4, gap 8, tooltip the label or source | PC | None | 14 C |
| 14.24 | **Open in a new tab** (`ExternalLink` 14): the page's address in the system browser | PC `openOutside` | None | 14 C |
| 14.25 | Warnings, when any: a list labelled "Links that won't load", 12 px `--text`, padding 8 12, gap 4, border-bottom, ground `--status-waiting` at 8 %; each with `TriangleAlert` 13 in `--status-waiting` | PC | None | 14 C |
| 14.26 | The page area: `--panel-bg` behind a white web view filling it | PC | **Done (A):** `--panel-bg` holding the white frame the view covers | 14 A |
| 14.27 | Loads again when the agent shows it again (a new `shownAt`) and on Reload; not on any other redraw | PC `frameKey` | **Done (A)** for `shownAt`: the load key `{pageId}:{shownAt}`; a new one calls `load_url`. Reload is C's | 14 A (shownAt), C (Reload) |
| 14.28 | Theme on load: `window.name` = `fleet:` + `{"place":"tab","appearance":…,"variables":{…}}`; then `fleet:theme` `{appearance, variables}` after each change | PT `pageFrameName`, `pageThemeMessage` | None | 14 B |
| 14.29 | The variables: `--fleet-bg` ← `--panel-bg`, `-surface` ← `--card-bg`, `-text`, `-muted`, `-border`, `-accent`, `-accent-surface` ← `--accent-dim`, `-accent-text` ← `--primary-foreground`, `-success` ← `--running`, `-warning` ← `--idle`, `-danger` ← `--error`, `-info` ← `--complete`, `-radius` ← `--radius-card`, `-font-sans`, `-font-mono`, `--fleet-chart-1..5` (dark `#6366f1 #d95926 #199e70 #c98500 #d55181`, light first `#5B6EC7`), `--fleet-font-size` (14px); appearance `dark`/`light` | PT `FROM_APP`, `CHART_SERIES`, `readPageTheme` | None | 14 B |
| 14.30 | Messages read (`readPageMessage`): `fleet:page-reply` (text non-blank, ≤ 65,536 chars), `fleet:page-state` (JSON ≤ 262,144 chars), `fleet:page-hello`, `fleet:page-choice` (value string, non-blank, ≤ 16,000; summary string or none, trimmed, else the value; over 200 chars cut to 199 + `…`); a bad pick becomes `fleet:page-choice-refused` with Fleet's three sentences; anything else, and a refusal sent by the page itself, is ignored | PB | None | 14 B |
| 14.31 | Hello → `fleet:page-state` with what was kept for that `pageId`, else `null`. State → kept; at most 50 pages, the oldest dropped; kept for the app's run (same page id when shown again) | PB `keepPageState` | None | 14 B |
| 14.32 | Reply → appended to the session's composer draft (below what's typed) and the composer focused; nothing sent | PC; `appendDraftText`, `focus-prompt` | None | 14 B |
| 14.33 | Messages only from this tab's own web view (decision 3); a web view that's gone hears nothing | PC `onMessage` | None | 14 B |
| 14.34 | Choice bar (`pageChoiceBar`): waiting "The agent is waiting for your pick."; proposed "You picked **{summary}**." (no full stop after `…`); refused: the reason with `TriangleAlert` 13 `--status-waiting-text`, role alert; answered "Sent: **{summary or value}**." + "Answered {time}" (11 px); not answered: "Not answered. You skipped it." / "You wrote instead." / "The turn stopped." / "Fleet restarted." / "Not answered after 24 hours." / "Not answered." + time; a waiting choice on an idle session reads "Not answered." | PCH | None | 14 C |
| 14.35 | Bar layout: padding 8 12, gap 8, border-top, 12 px `--muted`; waiting, proposed, refused: `--text`, border-top `--status-waiting` 40 % into `--border`, ground `--status-waiting` 6 %; bold part `--text` 600 | PC | None | 14 C |
| 14.36 | Buttons: **Change** (proposed: clears the proposal) or **Skip**; **Send to agent** (primary: `--accent` fill and border, `--primary-foreground`, hover brightness 1.1), on only when proposed, "Sending…" while sending; padding 4 10, border, `--radius-btn`, 12 px; disabled at 50 % | PC | None | 14 C |
| 14.37 | Send: `POST /api/sessions/{id}/page-choices/{callId}/send` `{value, summary}`; Skip: `…/skip`; a failure shows `body.error` or "Couldn't send that. Try again." (11 px `--error-text`, role alert) and lets the user try again | PC `answerChoice` | None | 14 C |
| 14.38 | A proposal is taken only while the choice is live (`isChoiceLive`) and none is held; a refusal only then too; a new `callId` clears proposal, refusal, sending and error; a status change ends sending | PC watches | `is_choice_live` only | 14 C |
| 14.39 | The tab's title is the canvas's title (`plainTerminalText`) | `stores/canvases.ts` | n/a | 10 |
| 14.40 | The phone's compact bar | PC `compact` | n/a | Out of scope (phone) |

### 3. The browser tab (BC, BOD, AR, PVB, BT)

| # | Row | Source | Native today | Owner |
|---|---|---|---|---|
| 14.50 | `browserPage(state)`: `url` (string, else empty), `appId` when non-empty | SC | None | 14 G |
| 14.51 | Bar (40 px, padding 0 8, gap 2): Back (`ArrowLeft` 15), Forward (`ArrowRight` 15), Reload (`RotateCw` 14), the address, the port select, the Whose-view toggle, Open in a new tab (`ExternalLink` 14) | BC | None | 14 H |
| 14.52 | Address field: 28 px, padding 0 10, border, `--radius-btn`, ground `--text` 4 %, mono 12, focus border `--accent`, placeholder "Waiting for the app…" (with an app) or "localhost:5173"; not overwritten while focused; Enter: digits → `http://localhost:N/`, no scheme → `http://` | BC `submitAddress` | None | 14 H |
| 14.53 | Show a page: `POST /api/sessions/{id}/browser/proxy {url}` once per origin → `{origin}`; the web view loads `origin + path + search + hash`; errors "{page} isn't an address." and "Fleet couldn't show {origin}: {error}" (12.5 px `--error-text`, role alert) | BC `show` | None | 14 H |
| 14.54 | Preview bridge v1 (`readBridgeMessage`: `fleet: 1` only; hello `{hmr, href, title}` with unknown hmr → none; location; update; open `{href, newTab}`); `navMessage`; `appAddress` (the preview's origin swapped for the app's in the address bar) | PVB | None | 14 G (logic), H (wiring, decision 4) |
| 14.55 | Hello: the address shows; a hello for the same page nobody asked for is the page reloading itself → pulse the tab; update → pulse | BC `onMessage` | None | 14 H (calls 10's `mark_updated`) |
| 14.56 | Open: a new-tab link → `POST /api/sessions/{id}/browser {url}`, the canvases loaded again, that tab made active; if Fleet refuses, shown in this tab; a same-tab link shows through this tab's preview | BC `openLink` | None | 14 H |
| 14.57 | Back, Forward, Reload: the bridge's `nav` when the page has it; else Reload sets the address again | BC `navigate` | None | 14 H |
| 14.58 | Open in a new tab: the preview's address for the page (the app's own `localhost` only works on Fleet's machine) | BC `openOutside` | None | 14 H |
| 14.59 | Port select when the app has more than one port: 28 px, mono 12, `:{port}`; picking one shows `addressForPort` (a printed link with a query first, then any printed on that port, else `http://localhost:{port}/`) | BC; AR `addressForPort` | None | 14 G (logic), I (select) |
| 14.60 | Another machine's session: no web view; "This app runs on {machine}. Its preview opens there." with **Open on {machine}** (its web app's session page) or "{machine} has no web app of its own, so its previews can't open from here yet." | BC `elsewhere` | None | 14 H |
| 14.61 | The app's panel instead of the page: Starting… / Waiting for the app to serve a page… (Loading… without an app) / The app exited [with code N] / The app is stopped; a dot (running `--running`, starting `--idle`, exited and build-failed `--error`, else `--muted`); the command in mono 12; **Start** (exited, stopped; `Play` 13, accent fill) ; the current run's last 8 lines (after `── restarted by Fleet ──`) while starting or exited | BC `panel`, AR `currentRunLines` | None | 14 I |
| 14.62 | The app strip (36 px): dot, command (mono 11.5, ellipsis, tooltip), status (`exited (code)`, `build failed` in `--error-text`), Restart/Start (`RotateCcw`/`Play` 13), Stop (`Square` 12, while live), Output (`ScrollText` 13, pressed state); labels hide under 440 px wide | BC | None | 14 I |
| 14.63 | Output drawer: max 220 px, mono 11.5/1.5, "No output yet."; `GET …/apps/{id}/output?after=N` every 1 s while on screen and the app is live, once otherwise; at most 2,000 lines; fewer than before means Fleet lost them → start over; stays at the bottom when it was there; opens by itself once on a failed build | BC; AR `fetchOutput` | None | 14 I |
| 14.64 | Start/Restart/Stop: `POST …/apps/{id}/restart|stop`; a refusal shows for 10 s (12 px `--error-text`) | BC `appAction`; AR `act` | None | 14 I |
| 14.65 | App runs: `app.updated` sets the app (an event clock; a load read before an event never undoes it); printed links kept across events, dropped on `restarted`; `reason: reloaded` pulses every browser tab showing that app; the app loads when its tab opens, comes back and after a reconnect; `GET …/apps/{id}` 404 → none | AR; USC `applyAppEvent` | None | 14 G |
| 14.66 | Back from a restart: shows its page again (follows a new port for a tab with a page of its own) and pulses | BC watch | None | 14 I |
| 14.67 | Agent's view: when the agent has its own tab on this page, the "Your view / Agent's view" toggle (24 px buttons in a `--text` 7 % pill, short "You"/"Agent" under 520 px, a 6 px accent pip), the banner "The agent is trying this page in its own browser. What it types and clicks doesn't show here." with **Agent's view**, and in Agent's view "Live picture of the agent's tab. You can watch, not click. Its sign-ins and cookies are its own." over 10's `view::agent_tab::agent_tab(s, t, session, tab_id, active)` (the web view hidden; `active` false when the tab isn't on screen, so its frames stop); the agent's tab found with 10's `fleet::agent_browser::tab_for` | BC 516-521; `stores/agent-browser.ts` | None | 14 J (the frames and `tab_for` are 10's) |
| 14.68 | Kept between tab switches (decision 5) | BC `onActivated` | None | 14 D |
| 14.70 | Dialog "Open a browser tab" (`sm:max-w-md`): label "Command or address" (13 px 500), field mono 12.5, placeholder "npm run dev"; hint: "A command, like npm run dev, or an address on Fleet's machine, like localhost:5173." / "Shows {url} through Fleet." / "Runs it in this session's folder and shows the page it serves." (12 px `--muted`) | BOD | None | 14 J |
| 14.71 | It opens empty, then fills with the command that last served a page in this project unless the user typed; "Used here before" (11.5 px) lists that command and the apps' commands newest first, no repeats, as mono 11.5 chips (on: `--accent` border, `--accent-dim`) | BOD; AR `listSessionApps` (`GET …/apps`) | None | 14 J |
| 14.72 | Submit: **Run** for a command (`POST …/apps {command}` → `{app, canvasId}`), **Open** for an address (`POST …/browser {url}`), off until there is a target or while busy; then the canvases loaded, the tab made active, the dialog closed; an error in Fleet's error box (12.5 px, `--error` 30 % border, 8 % ground) | BOD | None | 14 J |
| 14.73 | `browserTarget` | BT | Done (`conv::browser::browser_target`, tested) | 3 |

### 4. The Document tab and HTML previews (VC, HR, MDR, VP, TC)

| # | Row | Source | Native today | Owner |
|---|---|---|---|---|
| 14.80 | `parseVisualPayload`: only text starting with `{`; JSON object with `$type` `html` or `markdown` and string `content`; optional string `title`; `__proto__`/`constructor` keys ignored | VP | None | 14 E |
| 14.81 | Known visuals: every payload in the conversation's tool outputs, oldest first, for 10's "From this conversation"; one that arrives within 5 s of the session opening (`LIVE_TOLERANCE_MS`) opens as a tab once per tool id, history never does | AS `conversationVisuals` (component 3 row 1.91) | None | 14 E (10 draws the list and the tab) |
| 14.82 | Bar: 40 px, padding 0 14, gap 8, 12.5 px; the title (600, ellipsis, tooltip) then the kind, "Document" for markdown or "HTML" (12 px `--muted`); title = payload title, else the file name, else "Document" | VC; `visualCanvasTitle` | None | 14 E |
| 14.83 | Markdown body: padding 12 14, Rune's markdown; HTML body: the web view of the content, no scripts (decision 11), over the dotted ground (`--text` 7 % dots every 16 px) | VC, HR | None | 14 E |
| 14.84 | A tool card with a visual payload: in a box (margin 8 0, padding 12, 1 px border, `--radius-card`, `--panel-bg`): markdown drawn inline; HTML as a card (title, "HTML") instead of the frame; then **Expand** (accent fill, `--primary-foreground`, 12 px 600, padding 4 12, hover 85 %) opens it as a tab and shows the panel | TC 3.19 | None | 14 E |
| 14.85 | Annotating a markdown Document (`useCanvasAnnotate`) | VC | None | 12 (the annotation popover) |
| 14.86 | A rendered `.html` file tab: the web view of its text, no padding; `web::html_body` | `FileCanvas.vue` | None | 11 (uses 14 E's) |
| 14.87 | A rendered `.html` file in the review list: a card with Expand | `ChangesReview.vue` | None | 12 (uses 14 E's `html_card`) |

### 5. Pages in the conversation (CP, MP, MTL, ATC)

| # | Row | Source | Native today | Owner |
|---|---|---|---|---|
| 14.90 | `toolPage`: a call's `state.metadata.page {id, entry}` → `{path: pageAddress, id, source: input.path}`; none without both | ATC | The flag only (`tool.page`) | 14 F |
| 14.91 | A page shown again in the same turn (same `source`) is drawn once, at its last show; earlier turns keep theirs; the calls stay | ATC `withoutRedrawnPages` | None | 14 F |
| 14.92 | Drawn after the message's calls, outside their box, on the conversation's ground | MTL | None | 14 F |
| 14.93 | The card (decision 2): `margin 12 0 4`, the call's title, the file name, **Open full size** ("Open full size: {title}", `Maximize2` 14), **Open in a new tab** (`ExternalLink` 14; not when contained); 26 px buttons, `--muted`, hover `--accent-dim` | CP (bar) + ours | None | 14 F |
| 14.94 | Full size: a dialog over the app (scrim `rgba(0,0,0,.6)`, padding 24), a sheet `min(1200, 100%)` × `min(900, 100%)`, border, `--radius-panel`, `--panel-bg`; head padding 8 10 8 16 with the title (600, ellipsis) and Close (`X` 16); the web view below it (the page's own padding 16 20 comes from Fleet's script); Escape, the scrim and Close put it away; focus on Close when it opens and back where it was after | CP | None | 14 F |
| 14.95 | Full size gets the place `conversation` in its name (transparent ground, Fleet's text and font) and theme changes | CP; `PageTheme.cs` | None | 14 F (with B) |
| 14.96 | A mod's page: contained (no new tab, no pop-ups: `allow-scripts allow-forms allow-modals allow-downloads`), address from the mod; "{title}: Page unavailable" (12 px `--muted`) when it can't be built | MP | None | 22 (with 14 F's card) |
| 14.97 | Fleet's frame sized to the page's reported height | CP | n/a | Out of reach (decision 2) |

### 6. States and both themes

| # | State | Where | Owner |
|---|---|---|---|
| 14.100 | Page tab: loading (white), loaded, a label instead of a source, a long source path, three warnings, each choice-bar state, a failed send | page-tab scenes (the web area as the fallback panel) + live pictures | 14 C |
| 14.101 | Page tab, no web view possible (fallback) | scene `page-tab-fallback` | **Done (A)** |
| 14.102 | Browser tab: loading, a page, an address error, starting with output, exited with code, stopped, build failed with output open, three ports, another machine (both wordings), agent's tab banners | scenes + live | 14 H, I, J |
| 14.103 | Document tab: markdown, HTML, a long title | scenes | 14 E |
| 14.104 | The page card in a conversation, full size open | scenes + live | 14 F |
| 14.105 | Every state above in light and dark | pictures | each piece |

## Tests to port, by Fleet's name

- `lib/__tests__/page-bridge.test.ts`: all 15 (`readPageMessage` ×4, `readPageMessage page-choice` ×6, kept page state ×2, `readPageSize` ×3). **B**
- `lib/__tests__/page-theme.test.ts`: "names the app's own colours and fonts as the page's variables", "gives a light theme the chart series for light grounds" (read from `Theme`, not CSS), "is fleet: and the place and theme as JSON, which the page's script reads". **B**
- `lib/__tests__/page-choice.test.ts`: `pageChoiceBar` ×6. (`isChoiceLive` is ported; `pageChoiceRowTitle` ×2 belongs to component 3's tool rows: check `conv` and port there if missing, in C.) **C**
- `lib/__tests__/server-canvas.test.ts`: "reads a page canvas's state and fills what's missing", "reads the choice a page waits on, and drops a malformed one", "addresses the page on Fleet, escaping each part of the file name". (`isServerCanvasKind` is 10's.) **A** (address), **C** (rest)
- `components/canvas/__tests__/PageCanvas.test.ts`: "frames Fleet's copy of the page…" (as: the address the host is told to load), "says what a page Fleet wrote itself is, in place of a file", "loads the page again when the agent shows it again, and on Reload", "lists the links that won't load", "opens the page in a tab of its own" **C**; "puts a reply in the composer, and sends nothing", "keeps what the user already typed above the reply", "ignores messages from any other window or origin" (as: another web view's), "answers a hello with the state the page saved, also after the page is shown again", "stops listening once the canvas is gone" **B**; "says when the page's pick is too long to send, and keeps Send off with that reason", "takes a pick the page sends after a refused one" **C**.
- `lib/__tests__/preview-bridge.test.ts`: all 6. **G**
- `stores/__tests__/app-runs.test.ts`: all 8. **G**
- `components/canvas/__tests__/BrowserCanvas.test.ts`: "frames the page through its preview and shows where the page is", "ignores messages from other origins and old bridges", "pulses its tab when the page hot-updates or reloads itself…", "opens a link to another port … in a new tab through Fleet", "follows a link meant for this tab through the preview of its address", the two other-machine cases **H**; "offers Start for a stopped app…", "shows why an app exited and its last output", "opens the output when a build fails", "follows its app's page when the tab has none of its own" **I**.
- `stores/__tests__/agent-browser.test.ts`: 10's (with `fleet::agent_browser`).
- `lib/__tests__/visual-payload.test.ts`: all 25. **E**
- `stores/__tests__/canvases.test.ts`: "titles visuals by title, then file name, then a generic label", "titles a visual without them" (the title helper is 14's; the store is 10's), "shows a browser canvas as a browser tab with its page, app and title" and "shows a page canvas as a page tab with Fleet's copy and its title" (the readers). **E**, **G**, **C**
- `components/session/__tests__/activity-stream-utils.test.ts`: "points a page shown in the conversation at Fleet's copy, and gives other calls none", "draws a page shown again in the same turn once, at its last show, and keeps earlier turns' pages". **F**
- `components/session/__tests__/ConversationPage.test.ts`: "offers to open the page in a new tab, and does not when it is contained", "shows the page under the calls, outside their box, named with Fleet's theme" (the card, and full size's name), "opens full size over the app, and Escape puts it away", "leaves calls without a page as they were". Not ported: "grows its frame to the height the page reports" (decision 2). **F**
- `MessageToolList.test.ts` "draws a call's page after the box" (if component 3 left it). **F**
- No Fleet test exists for `BrowserOpenDialog` or `VisualCanvas`: J and E add their own (target → button word, suggestions order, submit calls; title and kind).
- Ours for the host (plain Rust, no window): the visibility rule `web::wanted(&State) -> Option<(tab, rect)>` for every route, panel, overlay and session switch; the injected script's text (it sets the name and forwards only Fleet's messages); the drop order of decision 5. **A**, **D**

## Out of reach, and why

- **Live pages inside the conversation and mod lists** (decision 1): a native web view can't scroll or clip inside a Rune list. Cards instead.
  CEF off screen is the way if the owner wants them (about 380 MB, its own component).
- **Rune drawing over a web view**: Rune has no layer above native views (`docs/research/web-content.md`). The web view hides instead
  (decision 7). If a future Rune adds `NativeOverlay`, popovers could stay over it.
- **Wayland without XWayland**: wry can't make a child web view on a Wayland surface. The fallback panel shows (decision 9).
- **The frame's referrer policy** (`referrerpolicy="no-referrer"`): wry can't set it per web view; the server already sends
  `Referrer-Policy: no-referrer` for pages, so pages are the same. The browser tab's frame never set one.
- **`event.isTrusted`** (only a real click answers a pick): the bar is Rune's, so every press is the user's.
- **The phone's compact page bar**: out of scope.

## Pieces, in build order

Each piece is one pull request a fresh session can do from this file alone, a few hundred lines of Rust (at most about 800). Pictures go under
`shots/parity/14-web-content/<piece>/`. A web view is a native window, so **`--snap` can't draw it**: pictures of a live web view are screen
grabs of a real window (Linux under Xvfb, `ffmpeg -f x11grab`; Windows and macOS in CI, piece A's workflow), beside Fleet's own client at the
same size and theme. Rune-only parts (bars, the choice bar, the fallback, cards, dialogs) are scenes. Every piece updates this file's rows,
`docs/PORT.md`'s row 14 and `docs/RUNE.md`.

**A. The spike in the app: one page canvas, live, on three systems.** Rows 14.1, 14.2 (gutter, resize), 14.3, 14.4 (`ui.open`), 14.5, 14.6,
14.7 (shown), 14.9, 14.10, 14.20 (address fields), 14.21, 14.26, 14.27 (shownAt), 14.101.
Files: `Cargo.toml` (feature `web`, on by default: `wry`, `gtk` on Linux, the `windows` crate's COM feature, which Rune already brings),
`src/web/mod.rs` (the `Host` held in `State`: `ensure(tab, url, init_script)`, `place(tab, rect)`, `hide_all()`, `drop(tab)`, `available()`;
a stub without the feature), `src/web/platform.rs` (GTK init and pump, COM init, `on_window`), `src/web/wanted.rs` (plain Rust: which tab is
on screen, where, from state), `src/fleet/web_page.rs` (`shown_page`, `page_address`), `src/view/page_tab.rs` (the body: page area with
`on_layout`, the fallback panel; no bar yet), `src/view/sheet.rs` (the one branch while `page_target` is set; 10 replaces it),
`src/lib.rs` (`on_window`, `poll` pump, scene `page-tab-fallback`), `.github/workflows/` (a job on all three systems that runs the app on an
invented page served by a local `python3 -m http.server` from `tests/fixtures/web/`, opened through mock data that points the page canvas at
it, walks a script: show, drag the gutter, open a Rune menu, close it, click into the page and type, click the composer and type, resize; and
grabs the screen after each step as artifacts; the workflow change is pushed over SSH), `README.md` (WebKitGTK and WebView2 requirements).
**Before pushing**: this machine's shared build sysroot needs WebKitGTK's dev packages (`libwebkit2gtk-4.1-dev` and what it pulls in),
unpacked without root as the research did, or every other session's `cargo build` breaks the moment this merges; say what was added in the report.
Depends on: nothing (component 10 not needed).
Tests: `server-canvas.test.ts` "addresses the page on Fleet, escaping each part of the file name"; ours: `wanted` (no page target → none;
another route → none; a menu open → hidden; collapsed panel → none), the fallback drawn headless.
Pictures: Linux (Xvfb) and the CI grabs from Windows and macOS: the scratch Fleet's `show-page` page in the panel (Linux live), the invented
page after a gutter drag and after a resize, a Rune menu open with the page hidden, the page back with its state, the keyboard steps; the
fallback scene in both themes. **If a system can't host it**, stop and report with the pictures: that system gets the fallback (decision 9)
and the owner decides whether the component goes on. Write every finding into `docs/RUNE.md` (focus per system, re-show, scale).

**Piece A done (17 November 2026).** What it came to:
- The scratch Fleet's `show-page` page loads live in the panel on Linux (Xvfb), at the address `page_url` gives, beside Fleet's own
  client (`shots/parity/14-web-content/A/`). It draws in Fleet's dark theme already: the server's injected variables default to it.
- The walk is a step of CI's `check` job on all three systems (`scripts/web-walk.sh`, `src/web/demo.rs`, uploaded as `web-walk-<system>`),
  not a job of its own: it reuses the job's debug build instead of building the app a second time on each system.
- The gutter step sets the dragged width (`RightPanel::width`) rather than dragging with the OS pointer: the web view follows the
  panel's layout either way, and a real drag is one more thing to script per system.
- The host script reaches the app over wry's IPC from a page served with Fleet's `sandbox` CSP (opaque origin) on Linux; piece B
  stands on that. Windows and macOS: see the PR's CI grabs and its log lines (`web: heard from …`).
- Open from A: 14.10 on Linux (black until WebKit's first paint); the rest of 14.2, 14.3, 14.4 and 14.7 are D's, as planned.

**B. The page bridge and the theme** (plain Rust plus the injected script). Rows 14.28–14.33.
Files: `src/web/page_bridge.rs` (`read_page_message`, kept states, `read_page_size`, the limits), `src/web/page_theme.rs` (`PageTheme` from
`Theme`, `frame_name`, `theme_message`, chart series), `src/web/inject.rs` (the document-start script, decision 3), `src/web/mod.rs` (IPC per
web view → `State`, `evaluate_script` to the page), the composer's draft append and focus (`composer::drafts`), theme change → every page.
Depends on: A. Tests: `page-bridge.test.ts` all, `page-theme.test.ts` all, the five PageCanvas bridge cases. Live: the scratch Fleet's
`show-page` page in the app's light and dark themes beside Fleet's; a page that sends `fleet:page-reply` (invented, served by the scratch Fleet
via `fleet_page_show` with a fixture page) puts its text in the composer.

**C. The page tab: bar, warnings, choice bar.** Rows 14.16, 14.20, 14.22–14.25, 14.27 (Reload), 14.34–14.38, 14.100.
Files: `src/view/page_tab.rs` (bar, warnings, choice bar), `src/fleet/web_page.rs` (`shown_page` whole, `page_choice_bar`, the answer
`POST`s on a thread), `src/mock/` (page-tab scenes for every choice state, warnings, a label, a long path), `src/lib.rs` (scenes).
Depends on: A, B; uses 10's `ServerCanvas` if merged, else `fleet/pages.rs` extended with `state`.
Tests: `page-choice.test.ts` `pageChoiceBar` all; `server-canvas.test.ts` the other two; the seven PageCanvas cases listed above; ours: Send
and Skip post the right body, a failure says Fleet's words. Live: pick on the page, **Send to agent**, the scripted model answers "Going with
**Paged table**"; pictures beside Fleet's at 1280×760, both themes (waiting, proposed, answered).

**D. The host, everywhere it can go wrong.** Rows 14.2 (Widen, narrow panel, panel toggle), 14.3 (session and route switches), 14.4 (every
overlay, overlap rule), 14.7 (every focusable Rune element; Windows `focus_parent`), 14.8, 14.11–14.15, 14.68 (browser tabs kept).
Files: `src/web/wanted.rs`, `src/web/mod.rs` (drop rules, decision 5), `src/web/focus.rs`, the overlay rectangles from `ui::overlays`, `docs/RUNE.md`.
Depends on: A, C; 10's tab switching for the tab rows (without it, those rows stay open with that reason).
Tests: ours, `wanted` for every overlay kind in `ui::overlays` (one case each) and for toasts beside and over the panel; drop order.
Pictures: CI grabs of each overlay over a page tab on Windows and macOS (hidden), the narrow layout, a 150 % scale run if a runner allows.

**E. The Document tab and HTML previews.** Rows 14.80–14.84, 14.86 (the `html_body` it gives 11), 14.87 (the `html_card` it gives 12), 14.103.
Files: `src/conv/visual.rs` (`parse_visual_payload`, known visuals, the live auto-open rule calling 10's `open_visual`), `src/view/document_tab.rs`,
`src/view/web_card.rs` (`html_card`), `src/view/toolcard.rs` (the visual box and Expand), `src/web/mod.rs` (`load_html` with scripts off).
Depends on: A; 10's `open_visual` and tab model (else Expand opens the panel only, listed open).
Tests: `visual-payload.test.ts` all; `canvases.test.ts`'s two visual-title cases; ours for the card and Expand. Pictures: a tool card with a
markdown and an HTML payload, the Document tab of each, beside Fleet's (a scratch Fleet session whose shell tool prints a payload: invented
content), both themes.

**F. Pages in the conversation as cards, and full size.** Rows 14.90–14.95, 14.104.
Files: `src/conv/message_list.rs` (`tool_page`, `without_redrawn_pages`), `src/view/page_card.rs` (the card, the full-size dialog), the message
view (after the calls), `src/web/mod.rs` (a web view in the dialog's body, place `conversation`). Depends on: A, B.
Tests: the two `activity-stream-utils` cases, the four ConversationPage cases, "draws a call's page after the box" if not already in 3.
Pictures: a conversation page (the scripted model's `show-page` with placement `conversation`, or a fixture page) as Fleet draws it and as the
card, both themes; full size open on Linux (Xvfb) beside Fleet's modal.

**G. App runs and the browser tab's data** (plain Rust). Rows 14.50, 14.54 (logic), 14.59 (logic), 14.65.
Files: `src/fleet/app_runs.rs` (the store, the event clock, `load`/`act`/`fetch_output`/`list_session_apps`/`start_command`/`open_address` on
threads, `address_for_port`, `current_run_lines`), `src/web/preview_bridge.rs` (`read_bridge_message`, `nav_message`, `app_address`),
`src/fleet/browser_tab.rs` (`browser_page`), the `app.updated` handler (pulse through 10's `mark_updated`). Depends on: nothing but 2's events;
can run beside B–F. Tests: `preview-bridge.test.ts` all, `app-runs.test.ts` all, `canvases.test.ts` "shows a browser canvas as a browser tab…".
Pictures: none new (logic); the PR shows the app unchanged (a scene) and a live log of `app.updated` read from the scratch Fleet.

**H. The browser tab: bar, preview and its bridge.** Rows 14.51–14.58, 14.60, 14.102 (page, error, elsewhere).
Files: `src/view/browser_tab.rs` (bar, address, page area, error, elsewhere), `src/web/inject.rs` (the preview bridge port, decision 4),
`src/web/mod.rs` (browser web views, `nav`). Depends on: A, D, G.
Tests: the seven BrowserCanvas cases listed for H. Live: the scratch Fleet's browser tab on an invented local site (`python3 -m http.server`),
navigate, back, reload, a link to another port opens a new tab; pictures beside Fleet's, both themes.

**I. The browser tab: the app's panel, strip and output.** Rows 14.59 (select), 14.61–14.64, 14.66, 14.102 (app states).
Files: `src/view/browser_app.rs` (panel, strip, output drawer), scenes for each app state. Depends on: G, H.
Tests: the four BrowserCanvas cases listed for I. Live: an app started with `POST …/apps {command}` (a `python3 -m http.server` command
in the scratch project), stopped, started, a failing command; pictures beside Fleet's, both themes.

**J. Open a browser tab, and the agent's view.** Rows 14.67, 14.70–14.72.
Files: `src/view/browser_open.rs` (the dialog on `ui::dialog`), `State::open_browser_dialog()` (10's + menu calls it), the Whose-view toggle and
banners in `browser_tab.rs` over 10's `view::agent_tab::agent_tab`, found with 10's `fleet::agent_browser::tab_for`. Depends on: G, H; 10's
+ menu and agent browser (else the dialog opens from the palette only and the toggle waits, listed open). Tests: ours for the dialog (hint per
target, suggestions order, Run/Open, error) and the toggle (shown only with an agent tab, back to Your view when it goes).
Pictures: the dialog with suggestions, both themes, beside Fleet's; the agent's banners (agent tab from the scripted model's browser tools if
it has a keyword for them, else a scene).

**K. Sweep.** Every test above accounted for (ported, or owned elsewhere with the name), every row closed or open with its reason, the three
systems' grabs of every web tab, `docs/PORT.md` row 14 closed.

## Checking (for every piece)

- A scratch Fleet (`MACHINE.md`: own ports, `WORK=/tmp/fleet-native-scratch-<piece>`, `systemd-run` cap). Run the scripted model with
  `HOME` set to the scratch home, not yours: `show-page` shows `$HOME/src/demo-app/pages/paging.html`, and that path is drawn in the page bar.
  Make that page from weave-fleet's `mobile/e2e/start-fleet.sh` (its `paging.html`). A browser tab: `POST /api/sessions/{id}/browser
  {"url":"http://localhost:<port>/"}` on a local `python3 -m http.server` serving an invented page.
- Native pictures: `--server <scratch> --scene dark|light --size 1280x760 --script <file> --snap … --at mark:<name>`; the row keys are
  `session-<p>-<i>`. Fleet's with Playwright at 1280×760, `weave:theme` in local storage, `weave:right-collapsed` false; click the tab by name.
- A web view is drawn only in a real window: Xvfb plus a screen grab on Linux; the CI job of piece A on Windows and macOS.
- Invented pages only: never a page from a real Fleet or a real project.

## Pictures in the planning pull request

`shots/parity/14-web-content/`: `fleet-page-dark.png`, `fleet-page-light.png` (Fleet's own client: the scripted model's page waiting for a
pick in the page tab, the choice bar under it), `fleet-page-picked-dark.png` (after clicking "B. Paged table" on the page: "You picked
**Paged table**.", **Send to agent** on), `fleet-browser-dark.png`, `fleet-browser-light.png` (a browser tab showing an invented local site
through the preview gateway), `native-opened-dark.png`, `native-opened-light.png` (the native app, debug build of `main` on 17 November 2026,
against the same scratch Fleet, after pressing **Open page** on the same card: the panel opens on the Plan list; there is no page tab).
Native pictures are at the window's 2× scale (2560×1520), Fleet's at 1×; both are a 1280×760 window.
