# Rune, as this app uses it

Notes on [Rune](https://github.com/anomalyco/rune) at the commit `Cargo.toml` pins
(`f1f54ce8`, 14 November 2026). Rune's own docs are the reference: `crates/rune/README.md` (core)
and `crates/rune-kit/README.md` (components), read **at that commit**. This page is what to know
before you open them, and what we found out the hard way.

Each claim is marked:

- **used**: this app does it, and it worked.
- **read**: taken from Rune's docs or source, not tried here yet. Try it small before you plan
  around it, and change the mark when you have.

Add to this page in the pull request where you learn something.

## What Rune is

- Rust. Views are plain functions: `fn view(&State, &mut Cx<State>) -> El`. Handlers get
  `&mut State`. The view runs again after a handler; hover, press and scroll don't re-run it.
  (used)
- It draws everything itself on the GPU: Metal on macOS, Direct3D 12 on Windows, Vulkan on Linux
  (a software Vulkan driver works for pictures). (used on Linux and Windows; on macOS CI builds it,
  runs the tests and draws every scene, but nobody has opened its window yet)
- Pre-release, MIT. No releases: apps pin a commit of its `dev` branch. About 40 changelog entries
  a day in early November 2026, some marked "Breaking:". It takes no pull requests.
- Apps build some of Rune's types as struct literals (`AppSpec`, `ScrollSpec`, `TextStyle`,
  `Span`, ...), so a new field in Rune breaks the build on a pin move. Always end such a literal
  with `..Type::new(..)` or `..Default::default()` where Rune offers one, as `spec()` does.

## What it has, and where Fleet needs it

| Fleet needs | Rune has | Mark |
|---|---|---|
| Layout: rows, columns, grow, shrink, min and max sizes, absolute children, clipping | `row()`, `col()`, `el()`, `spacer()` and builder methods | used |
| Text, icons, images | `text()`, `icon()` (SVG paths), `ImageCache` (PNG, JPEG, GIF, WebP, SVG) | used (PNG, icons) |
| Text fields: caret, selection, undo, clipboard, IME, placeholder, grow with content | `text_input`, `TextEdit`, `auto_grow` | used |
| Rich composer: `@` pills, marks, paste | `TextEdit::rich()` | read |
| Hover, press, focus, tooltips, cursors | `.hover()`, `.press()`, `.help()`, `.cursor()` | used |
| Drag (resize gutters) | `.on_drag` with `Drag`/`Phase` | used |
| Keyboard shortcuts | `AppSpec::on_key`, `KeyEvent::is_command` | used |
| Scrolling that stays at the bottom | `.scroll(ScrollSpec { stick_bottom })`, `Tx::scroll_to_bottom(key)` | used |
| Very long lists (a long conversation, 5,000 sessions) | `virtual_rows(count, est_height, key, build)` builds only what's on screen; `memo(id, version, build)` reuses a subtree | used (2,000 messages) |
| Text that streams in | `.streaming(true)`, `.reveal(..)` | read |
| Markdown: CommonMark, tables, streaming, selectable, find in document | `rune_kit::markdown::Markdown` (feature `markdown`) | used (paragraphs, lists, tables, code, quotes, streaming; not find or tasks) |
| Code highlighting | about 80 languages, incremental, in `rune-kit` (`syntax::highlight`) | used (through Markdown) |
| Diffs: unified and split, word-level, folding, 50,000 lines | `rune_kit::diff` (`DiffView`) | read |
| Mermaid | flowcharts, sequence and state diagrams only (feature `mermaid`) | read |
| Menus, popovers, selects, comboboxes, dialogs, toasts, tooltips | `rune-kit`; `.modal(..)` traps focus | read |
| Tree and table | `rune-kit` `Tree` (50,000 files), `Table` (100,000 rows) | read |
| Selecting and copying text across messages | `.selectable(true)`, `.selection_group(true)` | used (single text) |
| Animation | springs, presence transitions, shared elements; honours reduced motion | used (loops, slides) |
| Several windows, own title bar, file dialogs, file drops | `run_windows`, `Titlebar`, `tx.open_file`, `on_drop_files` (no drops on Wayland) | read |
| Screen readers | AccessKit: Narrator/NVDA, VoiceOver, Orca; `.label()`, `.role()` | read (`.label()` used) |
| Tests and pictures with no window | `App::headless_runtime`, `--snap`, `--script`, `--video`, `--tree`, `--trace` | used |

## What it doesn't have

Each of these is something Fleet's app does today. `docs/PORT.md` lists the decision each one
needs.

| Fleet uses | In Rune | What it means |
|---|---|---|
| A web view: pages agents show (`fleet_page_show`), HTML previews, the browser canvas (all sandboxed iframes today) | None of its own; `App::on_window` + `WindowHandle::with_raw_handle` hand the window to `wry` | `wry` works on all three systems as a child web view, but it sits on top of everything Rune draws. Chosen route and limits: `docs/research/web-content.md`; decision 1 in `docs/PORT.md`. |
| A terminal (xterm.js) | `rune::grid` / `GridRow` draws a terminal grid a row at a time (since 13 November 2026) | With `alacritty_terminal` for the state it draws vim, colours and wide characters right on all three systems. The pty is the server's. `docs/research/terminal.md`. |
| A code editor (CodeMirror: the file viewer, merge view, search) | Text fields and syntax colours, but no line numbers, gutters, folding or find and replace | The diff view covers Changes; the file viewer needs building. |
| HTTP and WebSocket | None on desktop | The app brings its own crates. See "Work off the UI thread". |
| A menu bar | macOS only | Windows and Linux draw their own, as Fleet's Electron app mostly does anyway. |
| Native right-click menus | macOS and Windows; not Linux | Use `rune-kit`'s `Menu` everywhere, or native where it exists. |
| Notifications | macOS and Windows; not Linux | Linux needs its own (D-Bus). |
| Tray icon | Windows and Linux; not macOS | |
| Video | macOS and Windows; not Linux | Fleet barely uses it. |
| Installers, signing, auto-update | "Rune doesn't bundle apps"; one script makes a macOS `.app` for development | Packaging is all ours. |

## Things we learned

- **A view isn't told the window's size.** `Cx` has none. To lay out by width (the content panel
  opens over the conversation in a narrow sheet), measure with `.on_layout` and store the width in
  the state. It arrives one frame late, which is why pictures are taken with `--at 0.3` and tests
  draw two frames before they look.
- **No percentage sizes.** There's no "at most 80% of the parent". Use measured widths or
  `grow`/`shrink` weights.
- **`.key("...")` is identity.** Tests and scripts find elements by key, animations follow keys,
  and virtual rows need unique ones. Key anything a test will click.
- **A formatted builder chain is long.** rustfmt puts one call on a line. That's fine; don't fight
  it with `#[rustfmt::skip]`.
- **Release builds on Windows** need `#![windows_subsystem = "windows"]` (no console window) and
  `-C target-feature=+crt-static` (no Visual C++ runtime to install). In a script, pipe a
  windowed program's output (`| cat`) or the shell won't wait for it.
- **Headless tools in release builds** need Rune's `tooling` feature, which `Cargo.toml` turns on.
- **Linux, to open a window on X11:** `libxkbcommon-x11-0` and `libxcb-xkb1`. Under Xvfb with
  lavapipe a real window opens and draws.
- **Cross-compiling doesn't work** from Linux to Windows or macOS (a dependency's C and assembly
  need the target's own toolchain). Each system builds on its own runner.
- **The web build is unproven.** It compiles and starts, but we couldn't confirm it draws (the
  test machine's headless browser shows a blank WebGPU canvas for any page). Nothing in the plan
  depends on it.
- **Speed numbers from a software renderer mean nothing.** Headless runs print frame times;
  under lavapipe they are 100 ms and more. Measure on real hardware, in a release build.

- **`rune-kit` is a second crate from the same repository.** Depend on it with the same `rev` as
  `rune` (the rev now appears six times: `rune`, `rune-kit`, four patches). Its `markdown` feature
  adds `pulldown-cmark`; `rune-highlights` brings `wasmparser` and `unicase` into the build.
- **A test's `rt.wheel(x, y, dy)`: positive `dy` scrolls up** (towards the start of the content).
- **`virtual_rows` plus `memo`:** the row's `key` closure and `build` closure get `&State`, not the
  view's `Cx`, so copy what the rows need (the theme, the markdown style) into the closure. Build
  the row's own `memo` inside `build` with a small tuple version (a message's part count and the
  sum of its text lengths, never its text). A reply that only ever grows says so by its length.
- **`Markdown` is cached when it isn't streaming** (no handlers, built-in highlighter), so a
  finished message costs a lookup; only the part still arriving (`streaming(true)`) is built each
  frame.
- **`poll` returning true during a scroll** logs "committed the view on N of N frames of a scroll"
  in debug builds. Streaming does that on purpose: the text changes every frame.
- **Headings can't be tinted separately** in `MarkdownStyle`: `text()` colours body and headings
  alike, so Fleet's accent-tinted headings (`--md-heading`) aren't drawn yet.

## Work off the UI thread

Rune has no HTTP or WebSocket client, and nothing slow may run in a view, a handler or `poll`
(debug builds warn at 8 ms). What it gives you (all "read"):

- `AppHandle<State>`: `Clone + Send + Sync`. From any thread, `handle.update(|s, tx| ..)` runs
  a closure on the UI thread and draws on the next frame. This is how a connection hands a
  snapshot or an event to the state.
- `AppSpec::poll(state, clock, tx) -> bool` runs before each frame; return true only when
  something visible changed, or the window redraws for nothing. `tx.wake_after(duration)` asks
  for a later poll. (used)
- `AppSpec::setup` gets a `Waker`; `on_ready` runs after the first frame, which is where a
  connection should start so the window appears at once.
- `rune::Latest<T>`: latest-wins requests on a small worker pool, for things like a search box.
  Not for a long-lived stream.

What the first connection (slice B) found:

- `AppSpec`'s `App::args(argv)` lets the app take its own flags (`--server`) and hand the rest to
  Rune; `App::handle()` before `run` gives the `AppHandle` for a thread started first. (used)
- `AppHandle::update` closures are applied by the real loop and by `--snap` / `--script` runs, so
  a headless picture can show a live server. They are **not** drained by `headless_runtime`
  (that queue is crate-private), so a test can't use the handle. The app posts through a small
  `Post` (`src/fleet/link.rs`) that is the handle in the app and a queue the test drains itself.
- A headless picture of a live server needs the clock to cover the network's: the script's marks
  are virtual time while frames render at software speed, so take the picture a few seconds after
  the last event and hold a streaming reply with the scripted model (`long-reply`).
- Blocking `ureq` (without TLS) and `tungstenite` on plain threads were enough: no async runtime.
  A read timeout of 20 ms on the socket lets one thread both read the hub and take commands.

Starting point for the server connection: plain threads and blocking crates (one thread for the
hub's WebSocket, short-lived ones or a small pool for HTTP), each handing results over with
`AppHandle::update`. No async runtime unless a slice shows it's needed: it would be the largest
addition to build time in the app. Batch events before you hand them over (the Vue client applies
a frame's worth at once in `use-session-stream.ts`), so a fast stream costs one redraw a frame,
not one per token.

## Rules Rune sets for fast apps

From Rune's "Smooth scrolling" and "Launching fast" sections (read):

- Build only the rows on screen (`virtual_rows`), with the row's content built inside the closure.
- `memo` rows that rarely change. Keep the version small; never use a message's text as one.
- No hover handler per row that changes state; use `.hover()` styles and `.hover_group()`.
- Read sizes with `Cx::layout_size`, not whole rects, so a scroll doesn't rebuild the view.
- No file reads or writes during frames, input or animation.
- `RUNE_COMMIT_BUDGET_MS` prints slow commits; debug builds warn when something rebuilds the view
  during a scroll.

## Examples worth reading first

In Rune's repository at the pinned commit:

| File | Shows |
|---|---|
| `crates/rune-kit/examples/shell.rs` | A desktop agent-chat app's shell: sidebar, toolbar, inspector; `-- virtual` for 5,000 rows |
| `crates/rune/examples/growth_reorder.rs`, `keep_stuck.rs` | A chat timeline pinned to its end, with a virtual list |
| `crates/rune-kit/examples/diff.rs` | A five-file review |
| `crates/rune-kit/examples/markdown_resize.rs` | Markdown under resize, with timings |
| `crates/rune-kit/examples/rich_editor.rs` | A rich composer |
| `examples/background` | A background thread and `AppHandle` |
| `examples/settings` | A whole Settings window |
| `crates/rune/examples/custom_titlebar.rs`, `popup_menu.rs` | An app-drawn title bar; native menus |

## What the kit (component 1) taught us

All found at the pinned commit, building Fleet's controls in `src/ui/`.

- **`rune-kit`'s Menu, Select, Dialog and Popover hardcode their sizes** (a 6px menu radius, 28px
  rows, a 6px dialog radius, `surface_raised` fills), so they can't be styled as Fleet's
  (10px radius, 5px padding, 30px rows, a 1px border, square dialogs) through a `Kit`. The kit here
  is built straight on Rune's own primitives instead: `.anchored_fit(key, Side, Align, gap)`,
  `.modal(click_outside, dismiss)`, `.focus_group()`, `.focus_highlight(c)`, `.focus_return(key)`,
  `Transition::scale_anchor`. (used)
- **Overlays are mounted once, at the root**, from one `Ui::open` value, anchored by the *key* of
  the trigger: `rune_kit::root(view, overlays)`. A view only keys its trigger and sets `open`.
  (used)
- **A menu row needs `.focusable(true)`** or arrow keys never reach it. A modal panel opens with
  focus on the panel: the first Down lands on the first row (as reka's menus do), and
  `.focus_selected(true)` on a checked row makes it the one Down starts from. (used, tested)
- **Edge order differs between calls:** `border_sides([top, right, bottom, left], c)` but
  `pin(left, top, right, bottom)`. (used)
- **Tooltips:** `Cx::help_look(HelpLook)` restyles every `.help("..")` at once; Fleet's tooltip
  (foreground fill, background ink, square, `px-3 py-1.5`, 700ms) is one `HelpLook`. (used)
- **A text field's own keys and changes** are `.on_input_key(..)` and `.on_input_change(..)`;
  `.on_key` is for focus groups. (used)
- **`Tx::scroll_into_view(key)`** keeps the keyboard's row on screen in a scroll. (used)
- **`--scene` takes one name**; a second `--scene` doesn't add to the first, so a state and a
  theme are one scene each (`kit-menu`, `kit-menu-light`).
- **No hover border.** `.hover(c)` is a fill only, so Fleet's ghost button (a border that appears
  on hover) shows its border only when pressed in the CSS and not here. Gap, not blocking.
- **No underline** on `text()`, so the link button and `hover:underline` are colour only.
- **No clipboard write from a handler** at this pin (`MemoryClipboard` is the test double), so
  "Copy session ID" in the palette is listed as waiting.
- **`↵` and `⌫`** (U+21B5, U+232B) draw as empty boxes in Inter and JetBrains Mono; the palette's
  footer says "Enter" and "Backspace" in words.

## Component 2, piece A (events)

Nothing in Rune was needed: the event catalogue and the subscriber list are plain Rust. One thing worth
knowing: `State` holds subscribers as `Box<dyn FnMut + Send>`, and Rune's `AppHandle::update` closures reach
them on the UI thread, so a listener may touch `State` freely but must not block.

## Component 2, piece B (delegations)

Nothing in Rune was needed. A scene is a plain `fn(&mut State)`, so it can't capture anything: a scene that needs a
state (here, the row status derived from a snapshot) calls a `State` method, and a light variant is a second scene.

## Component 2, piece C (work, context, marks)

Nothing in Rune was needed. To check that a snapshot taken again rebuilds only what changed, a test writes a marker
into a part id (not part of a row's `version()`) on the view rows and sees which survive; `State.projects` is public
for that.

## Component 2, piece D (optimistic prompts)

Nothing in Rune was needed. Two things learned about driving it:

- A real window under Xvfb takes `xdotool type` only after the window is given the X focus (`xdotool windowfocus`,
  there is no window manager to do it); a click alone puts the caret in the composer but the keys go nowhere.
- The 15 s timeout is the state's clock, not a thread: `poll` gets the time, so `Tx::wake_after` at the send is what
  makes the frame that turns a prompt to `needs_retry` happen when nothing else is going on.

## Component 2, piece E (reconnect, kept streams, partial)

Nothing in Rune was needed. Learned about driving it:

- A real window under Xvfb opens larger than a small screen, and with no window manager nothing resizes it: give Xvfb a
  big screen, then `xdotool windowmove <w> 0 0 windowsize <w> 1280 760` (find `<w>` with `search --pid`), and grab
  1280x760. The theme toggle (Ctrl+Shift+L) needs `windowfocus` first, as typing does.
- The Rune window draws no colour emoji, so Fleet's partial banner mark (a "warning sign" emoji) is the text glyph U+26A0 in
  the banner's amber.

## Component 2, piece F (older history)

Nothing in Rune was needed. Learned:

- No looping rotation: `rotate` takes an angle that springs, and `Cycle` animates opacity only. The loading line's
  spinner (Fleet's turns) fades slowly, which is what Fleet's own spinner does under reduced motion.
- Prepending rows to a `virtual_rows` list keeps the view where it was: the messages that were on screen stay put and the
  older ones appear above. No scroll fix-up was needed.
- A test that clicks something at the top of a stick-to-bottom stream has to scroll up first (`rt.wheel(x, y, 600.0)` a few
  times, one frame each): `find` still reports the element, but under the header, where the press lands on the header.
- To show a request in flight in a picture, `kill -STOP` the scratch Fleet's server (by the PID listening on its port),
  take the picture, `kill -CONT` it. Held 30 s it also shows the timeout wording. Always `CONT` it, in a `finally`.

## Component 2, piece G (presence and focus)

Rune has what is needed; nothing to work around. Learned:

- `App::on_focus(fn(&mut S, bool, &mut Tx))` fires once per focus change (also while hidden or covered); `App::on_visibility`
  fires when the window goes off screen (minimised, hidden, covered whole where the system says so) and back. Both are on `App`,
  not `AppSpec`, so they are set in `start_desktop` and never run in scenes or tests. Tests drive the link's commands directly.
- Under Xvfb with no window manager `on_visibility` did not fire when the window was unmapped or covered by a larger one;
  `on_focus` did (`xdotool windowfocus`). So visibility can't be shown on this machine; a real desktop is needed.
- `xdotool windowfocus` between two windows of the app itself is the way to see focus change under Xvfb: no other
  program (no tkinter, no WM) is needed.
- To see the hub frames the app sends, a small TCP relay in front of the scratch Fleet that unmasks the client's WebSocket text
  frames is enough; the app's `--server` is pointed at the relay.


## Component 2, piece H (the session list follows the hub)

Nothing in Rune was needed. Learned:

- A row changed from the link thread shows at the next frame with no extra call: `Post::update` marks the state changed, and the
  list is read from `State` in the view, so a status glyph, a title or a removed row needs no handler of its own.
- `on_focus(true)` is the "window came back" signal for refetching the list; the pointer crossing into a popup can fire it, so the
  refetch is one GET per gain, never a loop.
- Under Xvfb the scratch Fleet's rename and archive are `PATCH /api/sessions/{id}` (`{"title"}`) and `PATCH …/retention`
  (`{"retentionStatus":"archived"}`); a session started with `initialPrompt` `slow-task` stays working for about 45 s, which is long
  enough to take both themes' pictures of the working glyph.

## Component 2, piece I (finding the Fleet)

Nothing in Rune was needed. Learned:

- `App::handle()` can be cloned into a thread started before `app.run()`; `handle.update` from it applies once the window runs, so
  discovery needs no `poll` and no channel of our own.
- Under Xvfb a run for a picture needs a clean environment (`env -i`) to hold `HOME` and `FLEET_DESKTOP_DATA_DIR` to the scratch
  folder, and then `XAUTHORITY`, `DISPLAY`, `VK_ICD_FILENAMES` and the sysroot's `LD_LIBRARY_PATH` have to be passed back in by hand.


## Component 2, piece J (the sweep)

Nothing in Rune was needed. Learned:

- A test can be a live watcher: build a headless runtime (`App::headless_runtime`), start the `Link` against a real server with a
  `Post` that sends jobs down a channel, then loop `rt.update(job)` / `rt.frame(..)` and print the state each time it changes. It
  needs no window and no GPU, and reads the parts of the state no view draws yet. Mark it `#[ignore]`.
- A `Link` waiting out a reconnect delay is woken by the same command channel that carries `window_focus`, so "come back now" needs
  no extra thread: end the `recv_timeout` loop when focus arrives.
- Under Xvfb, `ffmpeg -f x11grab` needs `-frames:v 1` to write a single picture; without it the muxer fails ("Invalid argument").

## Component 3, piece A (tool rows)

- **No letter-spacing helper, but the text style has it**: `TextStyle::tracking(px)` (0.02em at 11px is `0.22`).
- **A filled triangle**: `canvas(Shape::new().fill(points, color)).size(8.0, 8.0)` draws Fleet's error glyph; no icon file needed.
- **A memoised row that depends on a set in `State`** (the open "Read N files" groups) must put that set's effect in the memo key (`open_mask`), or a click changes the state and the row doesn't redraw.
- Lucide renamed `message-circle-question` to `message-circle-question-mark`; the generator takes the file name, so the constant is `MESSAGE_CIRCLE_QUESTION_MARK`.
