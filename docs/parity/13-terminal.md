# Component 13: Terminal drawer (checklist and pieces)

Planned on 18 November 2026, from weave-fleet `main` at commit `cd13d91fa5e93f4b5a40ed8113d6aa325d556003` (read in a shallow clone).
The released server the checks run against is v0.72.0; every route, message and close code below was there (checked live on a scratch
Fleet: `GET`/`POST /api/sessions/{id}/terminals`, the socket, the scrollback replayed after a reload, a second tab, the selection bubble).
Decision 2 of `docs/PORT.md` governs: **`alacritty_terminal` keeps the terminal's state, `rune::grid` draws it**
(`docs/research/terminal.md`; the spike is on the branch `spike/terminal`, `spikes/terminal/`, never merged, and may be copied from).

**What goes first.** Nothing in the app draws a terminal or holds a second socket yet. **Piece A is a thin end-to-end slice**: one Fleet
terminal's output drawn in a native grid, typing reaching the shell, and a resize the shell sees, proven live against a scratch Fleet's
terminal API, before anything else builds on it. The drawer's frame, tabs and the rest come after.

Sources (weave-fleet `client/src/`): `components/terminal/TerminalDrawer.vue` (TD), `TerminalView.vue` (TV), `TerminalToggleButton.vue` (TTB),
`stores/terminals.ts` (TS), `composables/use-session-terminals.ts` (UST), `lib/terminal-api.ts` (TA), `lib/terminal-socket.ts` (SO),
`lib/terminal-connection.ts` (TC), `lib/terminal-mock-shell.ts` (MS), `lib/terminal-theme.ts` (TT), `lib/terminal-keys.ts` (TK),
`lib/format-terminal-context.ts` (FTC), `composables/use-draft-terminal-context.ts` (DTC), `routes/sessions.$id.tsx` (RT),
`composables/use-commands.ts` (UC), `components/layout/StatusBar.vue` (SB), `lib/keybinding-types.ts` (KB),
`components/harness-setup/HarnessSetupRows.vue` (HSR), `components/session/Composer.vue` (CO). xterm.js 6 (`@xterm/xterm ^6.0.0`, with
`@xterm/addon-fit` and `@xterm/addon-web-links`) is what TV wraps: where a row says "xterm", the behaviour is xterm.js's own, which Fleet's
users get and this app copies. Server: `src/WeaveFleet.Api/Endpoints/TerminalEndpoints.cs`, `TerminalSocket.cs`,
`src/WeaveFleet.Application/Terminals/` (`TerminalManager.cs`, `TerminalService.cs`, `LiveTerminal.cs`, `TerminalHistory.cs`,
`TerminalReplaySanitizer.cs`), `Configuration/FleetOptions.cs`.

## What the native app has today (looked at on 18 November 2026, `main` at `ca676bb`)

- **No terminal at all.** No terminal crate in `Cargo.lock`, no `src/term/`, nothing that opens a session terminal's socket.
- The session header draws the terminal button first in its toolbar (`view/header.rs`, key `terminal`, `SQUARE_TERMINAL`); pressing it does
  nothing (component 8's row 5.1 waits for this component). Picture: `native-pressed-*.png`.
- The palette has **toggle-terminal** (`ui/commands.rs`: "Show terminal", Ctrl/Cmd+J, keywords, off with "Open a session to use its terminal."
  while no session is open) as `Run::Waiting(13)`; the status bar draws **Ctrl J Terminal** only while `/api/config/client` says
  `terminalEnabled` (`State::terminals_allowed`, `fleet::status_hints::terminal_shown`, `view/status.rs`), and pressing it runs that command.
- `fleet/events.rs` reads `terminal.opened` and `terminal.closed` (component 2 left their handling to 13, its row 4.1); nothing applies them.
- Component 5 built the composer's side of "Add lines to message" (its rows 11.12, 11.13): `composer::attached::TerminalDraft`
  `{id, terminal_id, label, from, to, text}`, `terminal_line_range`, `format_terminal_context`, the per-session registry, the chip strip
  (`view::attachments`), the send and put-back paths, and **`State::add_terminal_lines(TerminalDraft)`** for this component to call.
- Component 7 piece O built the setup rows (`fleet/harness_setup_rows.rs`, `view/harness_setup_rows.rs`) with `POST`/`DELETE
  /api/setup/terminals` (`fleet::http::setup_terminal_create/close`): the shell is really asked for and ended, but the 220 px body shows the
  command with Copy and the line "The live terminal comes with the Terminal component." (its row C13.57 is 13's).
- What a socket needs is there: `fleet::tls::Stream` (ws and wss), `link::host_of`, `auth::hub_query_for(base)` (the machine's token as
  `&access_token=`), `tungstenite` 0.28, `State::link_for(session)` and `Link::off_thread` (the session's machine).
- Theme: every token the terminal palette reads exists in `src/theme.rs` (`panel_bg`, `text`, `muted`, `accent`, `accent_text`, `error`,
  `running`, `idle`, `idle_text`, `complete`, `queued`, `border`, `card_bg`) and `Theme::mix`; **`--terminal-bg` is missing**. JetBrains Mono is
  bundled (`theme::mono`). Icons: `SQUARE_TERMINAL`, `PLUS`, `X`, `ERASER`, `CHEVRON_DOWN`, `COPY` exist; **`MESSAGE_SQUARE_PLUS` is missing**
  (generate it with `tools/gen-icons.py`).
- Rune at the pin `f1f54ce8` has `rune::grid(cols, cell_w, cell_h, rows)`, `GridRow::new/cell/fill/shape`, `.grid_caret(col, row)`
  (`crates/rune/src/el/grid.rs`), `AppSpec::on_key`, `on_paste`, `on_wheel`, `on_drag`, `on_ime`, `tx.copy`, `tx.request_paste`.

## Seams with other components

| Component | What it gives or takes | Where |
|---|---|---|
| 2 | `terminal.opened` / `terminal.closed` read in `fleet/events.rs`; 13 applies them (2's row 4.1) | piece C |
| 5 | The chips and send path are 5's. 13 calls `State::add_terminal_lines(TerminalDraft { id: "<terminal>:<from>-<to>:<ms>", terminal_id, label: tab title, from, to, text })`, then focuses the composer (`focus-prompt`, `ui/commands.rs`) | piece F |
| 7 | The setup rows' terminal body (7's C13.57): 13's view mounted at the base path `/api/setup/terminals`, session id `setup`, with `initial_input` (the installer command typed once, no Enter); `ended` calls `State::close_setup_terminal`. The row's bar, Close, Starting…, polling and the found banner stay 7's | piece I |
| 8 | The header's terminal button (8's row 5.1) becomes the toggle with its dot and words | piece D |
| 9 | The status bar's terminal variant (9's row 4.7, and StatusBar test "hides the terminal from the terminal's own hints…") | piece E |
| 10 | The drawer spans the conversation column only; the right panel stays beside it. Ctrl/Cmd+Shift+B (toggle right panel) passes through the terminal | pieces D, E |
| 17 | A session on another machine: the REST calls through `link_for(session)`, the socket on that machine's base with its token | pieces A, B |

## Decisions (the orchestrator's, from decision 2 and the research; the owner can overturn)

1. **xterm.js behaviour is Fleet's behaviour.** What Fleet doesn't code but xterm.js does for it (mouse reporting, double-click word and
   triple-click line selection, bracketed paste, the wheel in the alternate screen, bold drawn bright, the inactive cursor's outline, the
   scrollbar, the fit rule) is ported as xterm.js 6 does it, and each row says so.
2. **Where it runs.** Each open terminal has one connection thread (tungstenite over `fleet::tls::Stream`, as the hub). The thread feeds
   output to the `alacritty_terminal` state under a mutex (as the spike did), and asks for a frame at most once per 16 ms; the UI thread only
   reads rows. Answers the parser produces (`Event::PtyWrite`: device attributes, cursor position) go back over the socket as input, as
   xterm.js answers them. Nothing touches the network on the UI thread.
3. **The crate.** `alacritty_terminal = "0.26"` (its pty and I/O loop come along unused), wrapped by `src/term/` so nothing else names its
   types. Apache-2.0: a row in the README's Licences list. No other new crate (the WebSocket is `tungstenite`, already here).
4. **Remembered state.** Fleet's `weave:terminal-height` and `weave:terminal-open` (local storage) become `terminalHeight` and `terminalOpen`
   in the settings file, as `changesWrap` did for component 12.
5. **No server.** As Fleet's Vite mock mode has a pretend shell (`terminal-mock-shell.ts`), the native app with no server gets one in
   `src/mock/terminal.rs`, with **invented** commands and output (not Fleet's, which name Fleet's own repository). Scenes and tests use it and
   recorded streams; no scene needs a server.
6. **Graphemes and fonts** as the research found: ZWJ emoji draw as their parts (as in Fleet's xterm.js, which doesn't load the grapheme
   addon); glyphs whose advance isn't the cell's are pinned with `GridRow::cell`.

## Rows

Owner: `13` = this component, with the piece; another number = that component. "Native" is what exists on 18 November 2026.

### 1. The terminal view (TV, TT; xterm)

| # | Row | Source | Native today | Owner |
|---|---|---|---|---|
| 13.1 | One terminal: JetBrains Mono (`--font-mono-stack`) 12 px, `lineHeight: 1.2`; **measured in Fleet at 1×: cells 7 × 19 CSS px** (xterm floors the measured advance and multiplies the font's own height by 1.2) | TV `new Terminal`; measured | Done in A | 13 A  |
| 13.2 | Fit (xterm's fit addon): `cols = ⌊(box width − 14) / cell w⌋`, `rows = ⌊box height / cell h⌋` (14 is the scrollbar it reserves while there is scrollback), at least 2 × 1; measured: a 508 × 225 box is 70 × 11 | TV `fitNow`; fit addon | Done in A | 13 A  |
| 13.3 | The box: `position: absolute; inset: 0`, padding `6px 4px 4px 12px`; xterm paints no background, the drawer's `--terminal-bg` shows through | TV `.terminal-view` | Done in A | 13 A  |
| 13.4 | Refit whenever the box changes size (resize observer, next frame) and when it is shown again; skipped while the box has no size | TV `scheduleFit`, `watch(shown)` | Done in A | 13 A  |
| 13.5 | A size change sends `{"type":"resize",cols,rows}` and tells the drawer the size (13.47) | TV `term.onResize` | Done in A (send done; the readout is D) | 13 A (send), D (meta)  |
| 13.6 | Scrollback 5,000 lines; the wheel scrolls it; new output while scrolled back leaves the view where it is (xterm) | TV `scrollback: 5000`; xterm | None | 13 F |
| 13.7 | The scrollbar: xterm 6's own, 8 px wide, radius 4, slider `text` at 16 %, hover 28 %, dragged 36 %; no shadow | TV `:deep(.scrollbar)`, TT | None | 13 F |
| 13.8 | Colours (`terminalTheme`): foreground and cursor `--text`; cursor accent `--panel-bg`; selection `mix(panel, accent, .28)`, inactive selection `mix(panel, text, .14)`; red `--error`, green `--running`, yellow `--idle`, blue `--complete`, magenta `--queued`; the other ANSI colours from `DARK` or `LIGHT` (by `isLight(panel)`: relative luminance > 0.4); the 256-colour cube and grey ramp and true colour as xterm | TT | Done in A | 13 A  |
| 13.9 | A theme change re-colours every open terminal at once (no reconnect) | TV `watch(resolvedThemeId)` | Done in A | 13 A  |
| 13.10 | Cell attributes: bold, italic, underline, inverse, and wide (CJK, emoji) cells | xterm | Done in A | 13 A  |
| 13.11 | The rest of the attributes: dim (half opacity), strikethrough, curly/double/dotted/dashed underlines and the underline colour (SGR 58), hidden, overline; bold with colours 0–7 drawn in their bright colour (xterm's `drawBoldTextInBrightColors`, on by default) | xterm | None | 13 G |
| 13.12 | Cursor: a block that blinks while focused (`cursorBlink: true`), drawn as an outline while not focused; DECSCUSR shapes (bar, underline, steady); hidden by DECTCEM | TV `cursorBlink`; xterm | Done in A (block and outline done; blink and shapes G) | 13 A (block), G (the rest)  |
| 13.13 | Focus: a press in the terminal gives it the keyboard; the drawer border turns accent (13.43) and the status bar says so (13.62); **on focus the view sends its own size again**, because another client attaching resized the shell | TV `onFocus` | None | 13 E |
| 13.14 | `clear()` (the drawer's Clear): clears this view and sends `{"type":"clear"}` so the server drops the scrollback for everyone; `{"type":"cleared"}` from elsewhere clears this view | TV `defineExpose.clear`, `onCleared` | Done in B (clear sends `{"type":"clear"}`; `cleared` resets the view; the drawer's Clear button is D's) | 13 B 
| 13.15 | The window title (OSC 0/2) and the bell are ignored (tab titles come from the server; xterm 6's bell is off) | TV, xterm | n/a | 13 G |
| 13.16 | Parser answers (DA, DSR cursor position) go back to the shell as input | xterm `onData` | Done in A | 13 A  |
| 13.17 | A flood keeps the window drawing; rows that didn't change aren't rebuilt (`Rc<GridRow>` reused; alacritty's damage instead of hashing every cell) | research | Done (reuse in A; damage in B: `Screen::read` takes alacritty's damage and looks at no other row; the flood's live check is below) | 13 A, B 

### 2. The connection (SO, TA, TC; server)

| # | Row | Source | Native today | Owner |
|---|---|---|---|---|
| 13.20 | Socket address `{ws|wss}://<machine>/api/sessions/{id}/terminals/{terminalId}/socket?cols=&rows=` (or under a base path, 13.80), the session's machine, its token as `&access_token=` (`session-calls-machine.test.ts`) | TA `terminalSocketUrl` | Done (A: home; B: another machine's address and token, `socket_url`, test "opens a terminal's socket on the machine provided…") | 13 B 
| 13.21 | On open: reset the view; then binary frames are the saved scrollback, then `{"type":"ready"}`, then live output | SO | Done in A | 13 A  |
| 13.22 | Input as binary frames; `resize` and `clear` as text; anything typed before `ready` (or while reconnecting) kept up to 4,096 bytes and sent after `ready`; more is dropped | SO `write`, `MAX_PENDING_INPUT` | Done (A: send; B: the 4,096-byte queue, `Keep::hold`) | 13 B 
| 13.23 | `{"type":"exit","exitCode":n}`: the shell ended; `exitCode` null when Fleet ended it; no reconnect after | SO | Done in B (`Status::Ended(code)`, no reconnect; input after it is dropped) | 13 B 
| 13.24 | Close 1000 (or after `exit`): status `ended`, no reconnect; the tab goes (13.34) | SO `NORMAL_CLOSURE` | Done in B (status `Ended`; the view's tab goes: `end_terminal` hides the drawer, which tabs (C) will refine) | 13 B 
| 13.25 | Any other close (4001 "fell behind the terminal's output", 1001 Fleet stopping, a dropped network): reconnect after 250, 500, 1000, 2000, 4000, 8000, 8000… ms; each reconnect resets the view and the scrollback replays | SO `RECONNECT_DELAYS_MS` | Done in B (`reconnect_delay`; 4001, 1001 and a cut socket each reconnect, the view resets and the scrollback replays) | 13 B 
| 13.26 | 5 attempts in a row that never reached `ready`: status `failed`, stop | SO `MAX_FAILED_ATTEMPTS` | Done in B (five in a row that never reached `ready`; one that did starts the count again) | 13 B 
| 13.27 | Statuses `connecting` → `open` → `reconnecting` / `ended` / `failed` | SO | Done in B (`Status`: Connecting, Open, Reconnecting, Ended, Failed) | 13 B 
| 13.28 | Dispose (tab closed, session left): close 1000 "Closed by the page.", no reconnect | SO `dispose` | Done in B (dropping the `Socket` closes 1000 "Closed by the page."; also while waiting to reconnect) | 13 B 
| 13.29 | "Reconnecting…": top 8, right 12, padding 2/8, radius 6, `--idle` at 14 % behind `--idle-text`, 11.5 px/500, no pointer events | TV `.terminal-view__notice` | Done in B (`view::terminal_view::notice`, scene `terminal-reconnecting`) | 13 B 
| 13.30 | Failed: over the view (`--panel-bg` at 85 %), centred, `--muted` 12.5 px, max 44ch: "This terminal can't be reached. Its shell may have ended while Fleet was away." and **Close tab** (28 px, padding 0 12, 1 px border, radius btn, `--panel-bg`, 12.5 px) which ends the tab as 13.34 | TV `.terminal-view__failed` | Done in B (`failed`; Close tab ends the tab as 13.24; scene `terminal-failed`) | 13 B 
| 13.31 | REST: `GET …/terminals` → `[{id,title,status:"running"|"stopped",createdAt}]` oldest first (a `stopped` one starts a new shell under its old scrollback when its socket opens); `POST {cols,rows}` → 201; `DELETE …/{id}` (404 is fine). A refusal's words are the body's `error`, else "The terminal request failed (HTTP n)."; no answer at all: "Couldn't reach Fleet to open a terminal." Server words seen: "This session already has 8 terminals. Close one to open another.", "This session is archived, so it can't open terminals.", "Terminals are turned off." | TA, UST, `TerminalManager.cs` | `http::setup_terminal_*` only | 13 A (list, create), C (close, words) |
| 13.32 | Server facts the client lives with: one shell has one size; replay is the last 5,000 lines or 2 MiB with queries stripped; 64 queued frames per client then 4001; at most 8 terminals a session and 32 live; tab titles "zsh", "zsh 2" from the shell's name | server | n/a | written down |

### 3. Tabs per session (TS, UST, MS)

| # | Row | Source | Native today | Owner |
|---|---|---|---|---|
| 13.33 | Per session: its tabs (the server's list), the active tab, whether the list has loaded once; `setTerminals` keeps the active tab when still listed, else the first | TS | None | 13 C |
| 13.34 | `add(terminal, activate)`: replaces a tab with the same id, else appends; becomes active when asked or when none is. `remove`: the neighbour (same index, else the last) becomes active. A tab whose shell ended (13.24) or that can't be reached and is closed (13.30) is removed, and the drawer closes when it was the last | TS, TD `onEnded` | None | 13 C |
| 13.35 | Load the open session's list when it opens and after the hub reconnects, only while `terminalEnabled`; events that arrive while loading are applied on top; a drawer left open (remembered) on a session whose list comes back empty closes, but one opened while the list was loading stays open (the user asked for a shell) | UST `load` | None | 13 C |
| 13.36 | `terminal.opened` adds the tab without showing it (unless none is showing), ignored if known; `terminal.closed` removes it and closes the drawer when it was the last | TS `applyEvent`, UST | read, not applied | 13 C |
| 13.37 | New terminal: `POST` with the active view's size, else 120 × 14; the new tab shows and the drawer opens; a refusal returns its words (13.31) | UST `openNewTerminal`, TD | None | 13 C |
| 13.38 | Close a tab: it goes at once, the drawer closes if it was the last, then `DELETE`; if the server refuses, the list is read again so it comes back | UST `closeTerminalTab` | None | 13 C |
| 13.39 | Open drawers (by session) and the height are remembered across restarts (decision 4); the height is one value for the window, at least 140, 272 by default | TS | None | 13 C |
| 13.40 | `forgetSession` (drop a session's tabs and close its drawer): in the store and its test, **called nowhere in Fleet's client**; ported with its test only | TS | n/a | 13 C |
| 13.41 | The drawer is per session (`key={sessionId}`): leaving the session drops its views and closes their sockets; coming back connects again and the scrollback replays. Shells keep running on the server | RT | None | 13 C |
| 13.42 | No server: a pretend shell (decision 5) knows `ls`, `pwd`, `git status`, `clear`, `echo`, a test run, a dev server that runs until Ctrl+C, `exit`; echoes typing, Backspace, Ctrl+C, Ctrl+L; the first terminal starts with some output; reopening replays its transcript | MS | None | 13 C |

### 4. The drawer (TD)

| # | Row | Source | Native today | Owner |
|---|---|---|---|---|
| 13.43 | Under the composer, spanning the conversation column; `border-top: 1px --border` (accent at 60 % while a terminal has the keyboard), background `--terminal-bg` = `--panel-bg` 96 % + `--text` (add to `theme.rs`); height from 13.39, capped at 60 % of the column (never under 140); not drawn at all until first opened in this session, then hidden rather than dropped | TD `.terminal-drawer`, `everOpened` | Done (A plain box; D cap, hidden not dropped; grip E) | 13 A (a plain box), D  |
| 13.44 | Grip: 9 px tall over the top edge (−5 px), row-resize cursor; a 36 × 3 bar, radius 2, `--muted`, shows at 60 % on hover, while dragged or focused; drag sets the height (capped as 13.43); double-click resets to 272; ↑/↓ by 24 when focused; tooltip "Drag to resize, double-click to reset"; label "Resize terminal" | TD `.terminal-drawer__grip` | None | 13 E |
| 13.45 | Header: 36 px, padding `0 6px 0 8px`, gap 2, bottom border; tabs, **+** (New terminal, `Plus` 14, off while creating), a spacer (≥ 8), the meta, **Clear** (`Eraser` 13, off with no active tab, "Clear"), **Hide** (`ChevronDown` 15, "Hide terminal (Ctrl J). Shells keep running."); icon buttons 26 × 26, radius btn, `--muted`, hover `text` 6 % behind `--text`, off at 40 % | TD `.terminal-drawer__header` | Done in D (tabs, New off while starting, spacer, meta, Clear off with no tab, Hide) | 13 D |
| 13.46 | A tab: 26 px, padding `0 4px 0 8px`, gap 6, radius btn, 12.5 px `--muted`; hover `text` 5 % and `--text`; active `text` 9 %, `--text`, 500; `SquareTerminal` 13 at 70 %; the title (max 160, ellipsis); close 18 × 18, radius 4, `X` 11, shown on hover or when active, hover `text` 10 %, "Close (ends the shell)"; middle-click closes; tabs scroll sideways with no scrollbar | TD `.terminal-tab` | Done in D (middle click, close on hover or active, strip scrolls with no bar; tab keys E) | 13 D |
| 13.47 | Meta (mono 11 px, `--muted`, gap 10): the session's folder (max 260, **cut at its start**, 80 %, the whole path as tooltip) and the active size "70×11" (tabular, 80 %, tooltip "70 columns × 11 rows") | TD `.terminal-drawer__meta` | Done in D (folder cut at its start with the whole path as tooltip, size from the view) | 13 D |
| 13.48 | Tab keys (focus on the tab strip): ←/→ wrap, Home, End, Delete closes the active tab; the new active tab takes focus | TD `onTabKeydown` | None | 13 E |
| 13.49 | Opening an empty drawer starts a shell once the session's list has loaded (not while one is starting, not after an error) | TD watch | Done in D | 13 D |
| 13.50 | Body states: "Starting a shell…" (`--muted`, 12.5 px, centred) while the first shell starts; an error: its words (max 52ch) and **Try again** (28 px, padding 0 12, border, radius btn, `--panel-bg`, 12.5 px; hover `text` 5 %); closing the drawer clears the error | TD `.terminal-drawer__message` | Done in D (the error is cut at about 52ch by a fixed width) | 13 D |
| 13.51 | A tab's view is made the first time it shows and kept while the session is open, so switching tabs or hiding the drawer doesn't reconnect; only the active one is shown | TD `visited`, `mounted` | Done in D (`Slot::parked`; the views of tabs shown before stay connected) | 13 D |

### 5. The toggle, the command and the status bar (TTB, UC, SB, KB)

| # | Row | Source | Native today | Owner |
|---|---|---|---|---|
| 13.55 | The header's terminal button (toolbar icon), only while `terminalEnabled` and not on a phone; pressed look while open (`text` 9 %, `--text`); a 6 px `--running` dot at top 5, right 5, ringed 2 px `--panel-bg`, while shells run with the drawer hidden; words "Hide terminal (Ctrl J)", "Show terminal: shells are running (Ctrl J)", "Show terminal (Ctrl J)" (⌘ J on a Mac) as tooltip and label | TTB | Done in D (hidden while terminals are off; with no server it opens the pretend shell) | 13 D |
| 13.56 | `toggle-terminal`: "Hide terminal" / "Show terminal" by the open session's drawer; "Show or hide the terminal under the chat." or, with no session open or terminals off, "Open a session to use its terminal." and off; Mod+J, works from a text field; keywords terminal, shell, console, drawer | UC, KB | Done (A runs; D: the label follows the drawer) | 13 A (runs), D (label)  |
| 13.57 | Status bar Ctrl J Terminal hint (shown only while `terminalEnabled`) runs the command | SB | Done (9's 4.6) | 9 |

### 6. Keys (TK, TV, xterm)

| # | Row | Source | Native today | Owner |
|---|---|---|---|---|
| 13.60 | Who gets a key while a terminal has focus (`terminalKeyOwner`): Fleet's pass-through shortcuts (toggle-terminal Mod+J, toggle-right-panel Mod+Shift+B, as currently bound) go to Fleet; off a Mac, Ctrl+Shift+C copies the selection and Ctrl+Shift+V pastes; everything else goes to the shell and **never** reaches Fleet (Esc, Ctrl K, Ctrl B, Ctrl [ …) | TK, TV `onHostKeydown` | Done in A (shell gets keys, Mod+J and Mod+Shift+B pass; copy keys E) | 13 A (shell gets keys), E (owner)  |
| 13.61 | Keys to bytes as xterm: printable text; Enter `\r`; Backspace `\x7f` (Ctrl+Backspace `\x08`); Tab, Shift+Tab `CSI Z`; Esc; Ctrl+letter and Ctrl+@[\]^_; Alt+key prefixed with ESC (off a Mac); arrows, Home, End in normal or application cursor mode (`CSI`/`SS3`); Insert, Delete, PgUp, PgDn, F1–F12 with modifier parameters (`CSI 1;m X`); Shift+PgUp/PgDn scroll the view; Windows Ctrl+Break (`Key::Pause`) as `\x03` | xterm | Done in A (table whole in `src/term/keys.rs`, basics tested; rest E) | 13 A (text, Enter, Backspace, Tab, Esc, Ctrl+letter, arrows), E (all)  |
| 13.62 | While a terminal has the keyboard the status bar's left side is: "Terminal has the keyboard" (accent text) · Esc Ctrl K Ctrl B go to the shell · Ctrl J Hide terminal (a button running the command) · Ctrl Shift C Copy (⌘ C on a Mac) | SB 100-130 | None (9's 4.7) | 13 E |
| 13.63 | Mac: ⌘C copies, ⌘V pastes; Option types characters (xterm's `macOptionIsMeta` is off and Fleet doesn't set it); other ⌘ keys go nowhere | xterm, TK | None | 13 E |
| 13.64 | Input methods (dead keys, CJK, the emoji picker): `ime_cursor` on the drawn cursor, `App::on_ime`; composed text goes to the shell | xterm | None | 13 H (Linux can't drive one; Windows and macOS by hand or "not checked") |

### 7. Selection, copy, paste, "Add lines to message" (TV, FTC, DTC)

| # | Row | Source | Native today | Owner |
|---|---|---|---|---|
| 13.66 | Select by dragging; double-click a word, triple-click a line; Shift+click extends; the selection colour 13.8 (inactive colour while unfocused) | xterm | None | 13 F |
| 13.67 | Copy: Ctrl+Shift+C (⌘C) and the bubble's Copy put the selected text on the clipboard; the bubble's Copy also clears the selection | TV `copySelection` | None | 13 F |
| 13.68 | Paste: Ctrl+Shift+V (⌘V) pastes into the shell; bracketed (`ESC[200~ … ESC[201~`) when the program asked; newlines as `\r` | xterm | None | 13 F |
| 13.69 | The bubble over a selection: 3 px padding, gap 2, 1 px border, radius btn + 1, `--card-bg`, shadow `0 10px 30px -12px rgba(0,0,0,.35), 0 1px 2px rgba(0,0,0,.08)`, fades in; above the selection's first row (+6), or under its last row when that is less than 34 px from the top; centred on the selection's end column, kept 130 px from either side; follows a scroll; gone when the view hides | TV `placeSelectionPop` | None | 13 F |
| 13.70 | Its buttons: 26 px, padding 0 9, gap 6, radius btn − 2, 12 px/500, hover `text` 6 %: **"Add line 9 to message"** / **"Add lines 9–11 to message"** (`MessageSquarePlus` 13, `--accent-text`) and **Copy** (`Copy` 13, `--text`) | TV template | None | 13 F |
| 13.71 | Selected lines: whole lines numbered from the top of the scrollback, from 1; a row that continues a wrapped line is joined onto it; a selection ending at column 0 of a later row leaves that row out; trailing whitespace trimmed; nothing but blanks gives no bubble | TV `selectedLines` | None | 13 F |
| 13.72 | Add: the lines go into the open session's draft as a chip (label = the tab's title), the same lines of the same terminal once; the selection clears and the composer takes focus | TD `onAttach`, DTC | The composer side is done (5's 11.12, 11.13) | 13 F (calls 5) |

### 8. Links and mouse (xterm, web-links addon)

| # | Row | Source | Native today | Owner |
|---|---|---|---|---|
| 13.75 | URLs in the output (the web-links addon's pattern: `http(s)://…`) underline under the pointer with a hand cursor; a click opens them in the system browser (Fleet: a new window, `noopener`) | TV `WebLinksAddon` | None | 13 G |
| 13.76 | OSC 8 hyperlinks behave as xterm 6's default link handler does (read it in the piece and port it) | xterm | None | 13 G |
| 13.77 | Mouse reporting when a program asks (modes 9, 1000, 1002, 1003; encodings default, UTF-8 1005, SGR 1006): presses, releases, drags, motion and the wheel go to the program as xterm encodes them; Shift (Option on a Mac) forces a selection instead | xterm | None | 13 H |
| 13.78 | The wheel in the alternate screen with no mouse mode sends ↑/↓ (as xterm); focus reporting (mode 1004) sends `CSI I` / `CSI O` | xterm | None | 13 H |

### 9. The setup terminal (HSR, 7's C13.57)

| # | Row | Source | Native today | Owner |
|---|---|---|---|---|
| 13.80 | The view hosted outside a session: base path `/api/setup/terminals`, session id `setup`, in the setup row's 220 px body; no drawer, no tabs | HSR 411-421, TA | Stand-in body (7 O) | 13 I |
| 13.81 | `initialInput`: typed into the shell once, on the first `ready` only (not again after a reconnect), without Enter | TV `onReady` | None | 13 I |
| 13.82 | `ended` (or Close tab on a failed one) closes the setup terminal (`close_setup_terminal`: DELETE, stop checking) | HSR | Close exists (7 O) | 13 I |

### 10. States and both themes

| # | Row | Source | Native today | Owner |
|---|---|---|---|---|
| 13.85 | Both themes: the drawer, tabs, meta, bubble, notices and every ANSI colour, each in a scene | TT, TD | None | every piece with a view |
| 13.86 | Long text: a long folder cut at its start, a long tab title cut at 160, many tabs scroll; a long line wraps in the grid and reflows on resize (alacritty) | TD, xterm | Done for D (long folder, long title, many tabs scroll); a long line wraps (A) | 13 D, A |
| 13.87 | Terminals off (`terminalEnabled` false): no button, the command off, nothing loaded | TTB, UST | Done (C: nothing loads; D: no button) | 13 C, D |

## Tests to port, by Fleet's name

| File | Cases | Piece |
|---|---|---|
| `lib/__tests__/terminal-theme.test.ts` | "takes text, selection and status colours from the Fleet theme", "uses a light palette for the colours a light theme doesn't set", "falls back when the theme sets nothing" (the native theme always sets them: port as the palette's fallbacks), `isLight` (1), `mix` "paints one colour over another, opaque", "gives up on a colour it can't read", `withAlpha` "turns a hex colour into rgba", "leaves a colour it can't read alone" | A |
| `lib/__tests__/terminal-socket.test.ts` | "connects with the terminal's size, then gets the scrollback, ready, and live output" (A); "sends input as binary and resize and clear as text" (A); "connects to a terminal that isn't a session's under its base path", "keeps what's typed before the socket is ready and sends it after", "reports the exit and doesn't reconnect after the shell ends", "reports a null exit code when Fleet ended the shell", "reconnects after a drop, and the new connection starts from a reset", "gives up after repeated attempts that never get the scrollback", "closes normally when disposed and doesn't reconnect", "passes on a clear from another window" (B) | A, B |
| `composables/__tests__/session-calls-machine.test.ts` | "opens a terminal's socket on the machine provided, with its token in the query" (B); "reads the session's terminals" (C) | B, C |
| `composables/__tests__/session-events-machine.test.ts` | "follows the session's terminals" | C |
| `stores/__tests__/terminals.test.ts` | all 7: "keeps the active tab when the list is reloaded, and falls back to the first", "shows a tab you opened, but not one another window opened", "ignores an opened event for a tab it already has", "shows the neighbouring tab when the active one closes", "keeps each session's tabs and drawer apart", "never lets the drawer get shorter than the minimum", "forgets a session's tabs and closes its drawer" | C |
| `composables/__tests__/use-session-terminals.test.ts` | all 9: "loads the session's terminals and applies its terminal events", "closes the drawer when the last terminal goes", "closes a drawer left open on a session with no terminals left", "keeps a drawer opened while the empty list was loading, so it starts a shell", "does nothing when Fleet has terminals turned off", "reloads after a reconnect", "opens a new terminal, shows it and opens the drawer", "returns the server's message when a terminal can't be opened", "closes a tab at once and puts it back if the server refuses" | C |
| `lib/__tests__/terminal-mock-shell.test.ts` | all 5 (with the invented output): "starts the first terminal with some output and a prompt", "echoes typing and runs the commands it knows", "keeps a dev server running until Ctrl C", "replays its output when opened again", "ends on exit" | C |
| `components/terminal/__tests__/TerminalDrawer.test.ts` | "renders nothing, and starts nothing, until the drawer is opened", "starts a shell when opened with no terminals, once the list has loaded", "shows why a shell couldn't start, and tries again on request", "mounts a tab's terminal the first time it shows, and keeps it", "closes a tab through the server, and clears the one showing", "hides without ending the shells", "shows the session folder" (D); "moves between tabs with the arrow keys", "resizes from the keyboard, within its limits" (E); "puts selected lines in the session's draft and asks the composer to take focus" (F) | D, E, F |
| `components/terminal/__tests__/TerminalToggleButton.test.ts` | all 3: "isn't there when Fleet has terminals turned off", "shows and hides the session's drawer", "marks shells still running while the drawer is hidden" | D |
| `components/terminal/__tests__/TerminalView.test.ts` | "tells the shell its own size when it gets the keyboard, so another client that attached since doesn't leave it wrapping at their width" | E |
| `lib/__tests__/terminal-keys.test.ts` | all 4: "leaves Ctrl J and Ctrl Shift B to Fleet", "copies with Ctrl Shift C and pastes with Ctrl Shift V", "follows a rebound terminal shortcut" (the native app has no rebinding yet: port against the binding table), "leaves Ctrl Shift C to the shell, since ⌘ C copies" | E |
| `components/layout/__tests__/StatusBar.test.ts` | "hides the terminal from the terminal's own hints, and leaves the rest as text" | E |
| `lib/__tests__/format-terminal-context.test.ts`, `composables/__tests__/use-draft-terminal-context.test.ts` | done by component 5 (piece O) | 5 |

### Piece D, what was done on 18 November 2026

Done: 13.43 (cap and hidden-not-dropped), 13.45–13.47, 13.49–13.51, 13.55, 13.56 (label), 13.86, 13.87. `view/terminal_drawer.rs` is the drawer (the old box
in `terminal_view.rs` is now only the view); `Slot::parked` keeps a connected view per tab shown before; `Terminal::clear` is xterm's `clear()`.
With no server the header button opens the pretend shell and New adds another tab (a scene or the demo). Ported by name: the seven D cases of
`TerminalDrawer.test.ts` and the three of `TerminalToggleButton.test.ts`.
Left open and why: the grip, tab keys and the focused border colour's keyboard cases are E; **the "Starting a shell…" and error states are shown by scenes
only** (a scratch Fleet can't be made to refuse or to be slow); the error's width is 52 × 6.6 px, not the `ch` of the 12.5 px font; the Mac's ⌘ wording
was not looked at (Linux only); the folder tooltip and the size tooltip are `help` text.
Pictures: `shots/parity/13-terminal/D/`.

### Piece C, what was done on 18 November 2026

Done: 13.33–13.42 and 13.31 (close, words); 13.87 for what C owns (nothing loads and the toggle does nothing while `terminalEnabled` is off).
`src/fleet/terminals.rs` is the store (tabs, active, loaded, open, height), `src/fleet/session_terminals.rs` loads the list when a session
opens, after a reconnect, and when `terminalEnabled` arrives, applies `terminal.opened`/`closed` (buffered while a load is out), opens a new
terminal and closes a tab with put-back; `terminal_slot.rs` keeps the one view in step with the active tab and drops it when the session
changes; `mock/terminal.rs` is the pretend shell. Open drawers and the height are `terminalOpen` and `terminalHeight` in the settings file.
Ported by name: all 7 of `terminals.test.ts`, all 9 of `use-session-terminals.test.ts`, all 5 of `terminal-mock-shell.test.ts`.
Left open and why: the **header button and the command stay enabled with terminals off** (the status bar test needs the command enabled
while a hint is clicked; the button's absence is D's, 13.87); there is **no tab strip**, so switching tabs is only by the neighbour rule
(D); the pretend shell is shown by the `terminal-mock` scenes and by `State::terminal_write`, an invented session with no server cannot
open it from the header (D); a scripted headless run did not write the settings file, so remembering across restarts is proven by the
settings round-trip test only; the two machine-routing cases (`session-calls-machine` "reads the session's terminals",
`session-events-machine`) rest on `link_for(session)`, which every session call uses, with no separate test.
Pictures: `shots/parity/13-terminal/C/`.

### Piece B, what was done on 18 November 2026

Done: 13.14, 13.17 (damage), 13.20 (other machines), 13.22 (the 4,096-byte queue), 13.23–13.30. `Socket` is now a connection that keeps itself:
one thread connects, serves, and after a drop waits 250, 500, 1000, 2000, 4000, 8000, 8000… ms (`reconnect_delay`; `Socket::open_with` takes other waits so
tests needn't sit through them) and connects again, until the shell ends, the page lets go, or five attempts in a row never reached `ready`.
Ported by name: all nine remaining cases of `terminal-socket.test.ts` and the `session-calls-machine` socket case; ours: 4001, 1001 and a cut
socket each reconnect into a reset terminal; a normal close ends it; an attempt that reached `ready` resets the failed count; a dispose while waiting
stops it; the view's notice and failed panel; the shell's exit takes the tab; the damage test.
Left open and why: the **failed panel's Close tab and a shell's exit only hide the drawer and drop the socket**: closing the tab on the server and
the neighbouring tab are C's. **Live, not done:** the flood (`yes | head -c 50000000`) and "stop and restart the scratch Fleet, Reconnecting… then back"
were not run in a real window (the headless clock isn't real time, and driving an Xvfb window by hand didn't work in this session); both are covered by tests
against a real WebSocket server on loopback (kill and reconnect, replay, reset), and Fleet's side (kill -9 of the scratch server) is in the pictures.
Written down for Fleet: the reconnect delay index never resets after a good connection, so a long session waits 8 s after its third drop (ported as is);
a graceful stop of the v0.72.0 server showed no Reconnecting… in Fleet's own page within 8 s, a kill -9 did after about 4 s.
Pictures: `shots/parity/13-terminal/B/`.

### Piece A, what was done on 18 November 2026

Done: 13.1–13.5, 13.8–13.10, 13.12 (block, outline when unfocused; blink is G), 13.16, 13.17 (row reuse; damage is B), 13.20, 13.21, 13.22 (send), 13.31 (list,
create), 13.43 (plain box), 13.56 (runs), 13.60 (shell gets keys), 13.61 (basic keys). Live on a scratch Fleet v0.72.0: prompt drawn, `ls` and
`stty size` typed (`13 115`), a wider column then `13 130`: the shell sees the resize.
Left open and why: the cursor does not blink yet (G); box drawing and block characters (`┌─┬`, `▁▂▃`) show small gaps at 19 px rows (looked at in
`scene-unicode-*`; G with the other cell attributes); the scene boxes are taller than the window leaves, so the conversation above is squeezed (the
real drawer is 272 px, D); Ctrl+Shift+C sends `^C` until E gives copy its owner; after a reconnect or the session changing nothing reconnects (B, C).
Pictures: `shots/parity/13-terminal/A/`.

## Out of reach, and why

- **The phone's terminal item** (`SessionActionToolbar` "Show terminal" on a phone, `PhoneSessionPage`): phones are out of scope.
- **Pictures of a real window on Windows and macOS** are CI's (`snap-scenes` on `main`); typing into them is by hand. The research's spike ran
  there; nobody has typed into this app's terminal on them until someone does.
- **Input methods** can't be driven under Xvfb (13.64): built, checked on Windows/macOS by hand or written down as not checked.
- **Speed on a GPU** is unmeasured on this machine (software renderer); the flood check shows only that the window keeps drawing.

## Pieces, in build order

Each piece is one pull request a fresh session can do from this file alone, a few hundred lines of Rust (at most about 800). Pictures go under
`shots/parity/13-terminal/<piece>/`: the native app beside Fleet's own client, the same scratch Fleet and session, 1280 × 760, both themes
(Fleet's: Playwright, `weave:theme` in local storage, press `[data-testid=terminal-toggle]`; see "Checking"). Every piece updates this
file's rows, `docs/PORT.md`'s row 13 and `docs/RUNE.md`.

**A. One live terminal, end to end (the slice).** Rows 13.1–13.4, 13.5 (send), 13.8–13.10, 13.12 (block), 13.16, 13.17 (row reuse), 13.20
(home machine), 13.21, 13.22 (send), 13.31 (list, create), 13.43 (a plain box), 13.56 (runs), 13.60 (the shell gets keys), 13.61 (the basic keys).
Files: `Cargo.toml` (`alacritty_terminal = "0.26"`; the PR says why: decision 2), `README.md` (Licences row), `src/term/mod.rs` (the wrapper:
`Terminal::new(cols, rows)`, `feed(&[u8])`, `resize`, `reset`, `take_answers()` for 13.16, `mode()`), `src/term/screen.rs` (rows to
`GridRow`s: spans for colour and weight, `fill` for backgrounds, `cell` for wide or fallback glyphs, the cursor; a row cache by content),
`src/term/palette.rs` (`terminal_theme`, `mix`, `is_light`, `with_alpha`, `DARK`/`LIGHT`, the 256 colours), `src/term/keys.rs` (basic keys),
`src/term/fit.rs` (13.2), `src/fleet/terminal_socket.rs` (connect, read loop on its own thread feeding the `Terminal`, write, resize; no
reconnect yet), `src/fleet/http.rs` (`terminals`, `terminal_create`), `src/view/terminal_view.rs` (the grid in its padded box, `on_layout`
→ fit → resize, the cursor), `src/view/sheet.rs` (a 272 px box under the composer while the open session's terminal is open), `src/theme.rs`
(`terminal_bg`, `--terminal-bg`), `src/ui/commands.rs` and `src/view/header.rs` (Ctrl/Cmd+J and the header button toggle it), `src/lib.rs`
(scenes). On open: list the session's terminals; use the first, else `POST {120,14}`; attach. Keys go to the shell while the box has focus.
No-server scenes replay recorded streams: copy `spikes/terminal/streams/{vim,git-log,unicode}.bin` and `record.sh` from the branch
`spike/terminal` to `tests/fixtures/terminal/` (they record an invented project, "lantern"; read them for anything real before committing).
Scenes `terminal-vim`, `terminal-git-log`, `terminal-unicode`, each `-light`.
Depends on: nothing.
Tests: `terminal-theme.test.ts` (all); `terminal-socket.test.ts` "connects with the terminal's size, then gets the scrollback, ready, and live
output" and "sends input as binary and resize and clear as text" (against a local `tungstenite::accept` server, as `fleet/tests/wire.rs`);
ours: a recorded stream read back row by row, the fit (508 × 225 → 70 × 11), keys (a, Enter, Ctrl+C, ↑ in normal and application mode),
a DSR query answered over the socket, an unchanged row handed back as the same `Rc`.
Live (the proof): on a scratch Fleet, press the header button: the shell's prompt appears; type `ls` and `stty size` and see the output;
resize the window and `stty size` again shows the new size; the same session open in Fleet's own client shows the same typing (one shell).
Pictures: that run beside Fleet's drawer showing the same scrollback, both themes; the three scenes.

**B. The connection, whole.** Rows 13.14, 13.17 (damage, flood), 13.20 (other machines), 13.22 (pending), 13.23–13.30.
Files: `src/fleet/terminal_socket.rs` (reconnect schedule, failed attempts, pending input, statuses, exit, cleared, clear, dispose,
close codes, base path), `src/term/mod.rs` (alacritty's damage), `src/view/terminal_view.rs` (Reconnecting…, the failed panel).
Depends on: A. Tests: the other eight cases of `terminal-socket.test.ts`; `session-calls-machine` "opens a terminal's socket on the machine
provided, with its token in the query"; ours: 4001 then reconnect resets and replays; 1001 reconnects.
Live: `yes | head -c 50000000` ends with the client dropped (4001), reconnected and the scrollback back, the window drawing throughout;
`exit` ends the tab; stop and restart the scratch Fleet: Reconnecting…, then back. Pictures: Reconnecting… and the failed panel (scenes, both
themes) beside Fleet's (stop its server to see them), the flood's end.

**C. Tabs per session (plain Rust).** Rows 13.31 (close, words), 13.33–13.42, 13.87.
Files: `src/fleet/terminals.rs` (the store: per-session tabs, active, loaded, open, height, focused; `set_terminals`, `add`, `remove`,
`set_active`, `apply_event`, `set_open`, `toggle_open`, `set_height`, `forget_session`), `src/fleet/session_terminals.rs` (load on open and
on reconnect with buffered events, the open-before rule, open new, close with put-back, the `terminalEnabled` gate, views and sockets dropped
when the session changes), `src/fleet/http.rs` (`terminal_close`, the refusal words), `src/fleet/settings.rs` (`terminalOpen`,
`terminalHeight`), `src/mock/terminal.rs` (the pretend shell, invented output), `src/fleet/events.rs` hand-off. A's open logic moves here.
Depends on: A. Tests: `terminals.test.ts` (7), `use-session-terminals.test.ts` (9), `terminal-mock-shell.test.ts` (5),
`session-events-machine` "follows the session's terminals", `session-calls-machine` "reads the session's terminals".
Pictures: no new view; the app showing it live: two terminals opened from Fleet's own client appear in the native app's list (a scene-free
picture of the box switching between them with Ctrl/Cmd+J), and the mock shell's first terminal (scene), both themes.

**D. The drawer.** Rows 13.43, 13.45–13.47, 13.49–13.51, 13.55, 13.56 (label), 13.86.
Files: `src/view/terminal_drawer.rs` (section, header, tabs, buttons, meta, body states, the views kept per tab), `src/view/header.rs` (the
toggle with its pressed look, dot and words), `src/ui/commands.rs` (the label), `src/view/sheet.rs` (A's box becomes the drawer), `src/lib.rs`
(scenes `terminal-drawer`, `-two-tabs`, `-starting`, `-error`, `-long`, `-hidden-dot`, each `-light`).
Depends on: B, C. Tests: `TerminalDrawer.test.ts` (the seven D cases), `TerminalToggleButton.test.ts` (3).
Pictures: the drawer with the scrollback of `fleet-drawer-*.png`'s commands, two tabs, Starting a shell…, an error with Try again, the dot
with the drawer hidden: native beside Fleet, both themes.

**E. Height and keyboard.** Rows 13.13, 13.44, 13.48, 13.60 (owner), 13.61 (all), 13.62, 13.63.
Files: `src/view/terminal_drawer.rs` (grip, tab keys, the focused border), `src/term/keys.rs` (every key, modifiers, application keypad),
`src/fleet/terminal_keys.rs` (`terminal_key_owner`), `src/lib.rs` (`on_key` routes a focused terminal's keys before Fleet's shortcuts),
`src/view/status.rs` (the "Terminal has the keyboard" variant).
Depends on: D. Tests: `TerminalDrawer.test.ts` "moves between tabs with the arrow keys", "resizes from the keyboard, within its limits";
`terminal-keys.test.ts` (4); `TerminalView.test.ts` (1); `StatusBar.test.ts` "hides the terminal from the terminal's own hints, and leaves the
rest as text"; ours: Esc, Ctrl+K, Ctrl+B reach the shell and not Fleet; Ctrl+J hides; the key table.
Pictures: the status bar variant beside Fleet's (`fleet-statusbar-terminal-focused-dark.png`), the grip shown while dragged, a taller drawer;
live: vim run in the drawer and quit with Esc `:q` (Esc reaches vim, not Fleet's Interrupt).

**F. Selection, copy, paste and "Add lines to message".** Rows 13.6, 13.7, 13.66–13.72.
Files: `src/term/selection.rs` (alacritty's `Selection`, `selected_lines`), `src/view/terminal_view.rs` (drag, clicks, the wheel, the
scrollbar), `src/view/terminal_pop.rs` (the bubble), `src/icons.rs` via `tools/gen-icons.py` (`message-square-plus`).
Depends on: E. Tests: `TerminalDrawer.test.ts` "puts selected lines in the session's draft and asks the composer to take focus"; ours:
wrapped rows joined, the column-0 end, blanks give none, the bubble above or under, the 130 px clamp, bracketed paste.
Pictures: the bubble over two lines beside `fleet-drawer-selection-*.png`, the chip in the composer after Add (5's strip), both themes; live
copy and paste on Linux (X11 clipboard).

**G. Every cell, the cursor and links.** Rows 13.11, 13.12 (the rest), 13.15, 13.75, 13.76.
Files: `src/term/screen.rs` (the attributes, `GridRow::shape` for strikethrough and the underline styles), `src/view/terminal_view.rs`
(cursor blink from the app's clock, outline when unfocused, shapes; link hover and open with `tx.open_url`), `src/term/links.rs`.
Depends on: F. Tests: ours per attribute and cursor shape; a URL found and opened; an OSC 8 link.
Pictures: the Unicode and colour stream (13.11's attributes) beside Fleet's drawer showing the same `printf` (both themes), a hovered link.

**H. Mouse reporting and the rest of the modes.** Rows 13.64, 13.77, 13.78.
Files: `src/term/mouse.rs` (encodings), `src/view/terminal_view.rs` (presses, motion, the wheel by mode; Shift/Option forces selection),
`src/term/keys.rs` (focus reports), IME through `on_ime`.
Depends on: G. Tests: ours: each mode and encoding's bytes for a press, drag and wheel; the alternate-screen wheel; focus in/out.
Pictures: live on Linux: an invented TUI that echoes mouse reports (a short script, e.g. `printf '\e[?1000;1006h'; cat -v`), and `less` scrolled
by the wheel in the alternate screen; beside Fleet's drawer doing the same.

**I. The setup terminal.** Rows 13.80–13.82 (7's C13.57).
Files: `src/view/harness_setup_rows.rs` (the body becomes the terminal view), `src/fleet/terminal_socket.rs` (the base path, `initial_input`
once), `src/fleet/harness_setup_rows.rs` (the view's `ended` closes).
Depends on: B (C not needed). Tests: ours: the command typed once on the first `ready`, not after a reconnect; ended closes the setup terminal.
Pictures: the setup wizard's row with the live terminal and the command typed, beside Fleet's, both themes (a scratch Fleet with no harness on
its PATH, as 7 O did; the shell's prompt set to an invented one, see "Checking").

**J. Sweep.** Every row read again, every Fleet test above accounted for by name, a live end-to-end run (open, two tabs, type, select and
Add, hide and show, close, a flood), the CI pictures from `main` on Windows and macOS looked at, `docs/RUNE.md` updated, closing pictures in
`shots/parity/13-terminal/J/`.

## Checking (for every piece)

- A scratch Fleet (`MACHINE.md`: own ports, `WORK=/tmp/fleet-native-scratch-<piece>`, the `systemd-run` cap). Its shells start in the
  scratch home and print the machine's user and host names in the default prompt: **before opening a terminal, write
  `PS1='demo $ '` (and `export PS1`) into the scratch home's `.zshrc`, `.bashrc` and `.profile`**. Look at every picture for paths, user names
  and host names anyway. Type only invented commands; give the demo project invented files (`src/pager.ts`, `tests/pager.test.ts`).
- Native pictures: `--server <scratch> --scene dark|light --size 1280x760 --script <file> --snap … --at mark:<name>`; the session row is
  `session-0-0`, the header button `terminal`. Native pictures are at the window's 2× scale (2560 × 1520).
- Fleet's pictures: Playwright at 1280 × 760, `weave:theme` in local storage, the session at `/sessions/<id>`, press
  `[data-testid=terminal-toggle]`, wait for `.xterm-rows` to hold the prompt, click `.terminal-view__host` and type.
- Typing into the native window live: Xvfb and `xdotool` (`MACHINE.md`, "Looking at the app").

## Pictures in the planning pull request

`shots/parity/13-terminal/` (all from one scratch Fleet v0.72.0, one session, 1280 × 760): `fleet-drawer-dark.png`, `fleet-drawer-light.png`
(Fleet's own client: the drawer open with `ls`, a coloured `printf`, `git log --oneline` and `cat src/pager.ts`, the folder and "70×11" in the
header, the status bar saying the terminal has the keyboard), `fleet-drawer-two-tabs-dark.png` (a second tab, "zsh 2"),
`fleet-drawer-selection-light.png` (two lines selected: "Add lines 5–6 to message" and Copy), `fleet-drawer-hidden-dark.png` (the drawer hidden,
the green dot on the header button while shells run), `native-pressed-dark.png`, `native-pressed-light.png` (the native app, debug build of
`main` at `ca676bb`, the same session after pressing the header's terminal button and Ctrl+J: nothing opens).
