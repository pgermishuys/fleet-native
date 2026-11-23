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
| A web view: pages agents show (`fleet_page_show`), HTML previews, the browser canvas (all sandboxed iframes today) | None of its own; `App::on_window` + `WindowHandle::with_raw_handle` hand the window to `wry` | `wry` works on all three systems as a child web view, but it sits on top of everything Rune draws. Chosen route and limits: `docs/research/web-content.md`; decision 1 in `docs/PORT.md`. `--snap`, `--video` and scenes draw only Rune's own surface, never a native child view, so a picture of a web view is a screen grab of a real window (component 14's plan, `docs/parity/14-web-content.md`). Both calls are still there at the pin `f1f54ce8` (17 November 2026). |
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

- **Restarting the app (the updater).** Rune has no quit or relaunch call that waits for a save. The updater starts the new
  process itself (`Command::spawn`, or `open -n` for a macOS bundle) and ends with `std::process::exit(0)` from an
  `AppHandle::update` closure, after writing the drafts and settings synchronously (`save_now`, in the same order as the
  threads that normally write them). A running Linux program can be replaced by a rename; Windows needs `self-replace`.
- **`ureq` needs its `rustls` feature for https.** Plain `default-features = false` talks http only; GitHub's release downloads
  are https and redirect to another host, which `ureq` follows by itself.

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

## Component 3, piece B (tool card and output)

- **Coloured terminal output is one `rich(content, spans)` text**, a `Span` per styled run (colour, weight 600, italic,
  underline, `.bg(colour, 0.0)`); a 2,000-line box is a single element and lays out fast enough. Dim is the colour at 60% alpha.
- **`Color::mix` mixes in linear light, CSS `color-mix(in srgb)` in sRGB.** `view::toolcard::mix` encodes, mixes and decodes
  so the bright and cyan terminal colours match Fleet.
- **A freshly opened row in a `virtual_rows` list measures itself a few frames later**: a click on something inside it
  right after opening can land on the stale layout. A headless test runs about half a second (`run`) before clicking it.
- **`on_press_tx` + `tx.scroll_to_bottom_instant(key)`** moves a scroll box that doesn't exist yet this frame: the box
  opens on its last lines. `ScrollSpec { stick_bottom }` alone didn't (it showed the top in a picture).
- **No clipboard write from a handler** (still): the output's Copy button is drawn and says so.

## Component 3, piece C (diffs)

- **`rune_kit::diff::DiffView` is not used.** It is a review tool (file headers, unified and split, word marks, comments, a
  virtual list over many files); Fleet's `DiffView.vue` is one flat grid with a 2px stripe, two number columns, a marker and
  an expander row, so the rows are drawn here (`view/diff.rs`) from `conv::diff`.
- **Sticky line numbers:** two columns of rows of the same fixed height. The numbers are a plain column on the left, the code a
  `scroll_x` box on the right, so the numbers stay put while the code scrolls sideways. A tint must reach the end of the longest
  line, so every code row gets `min_w` from the longest line (characters x 7.2, the 12px monospace advance): an estimate, not a
  measure.
- **`scroll_x()` turns its box into a row**: children of `el().scroll_x()` lay out side by side. Put one `col()` inside it.
- **`El::clip` doesn't exist**; `clip_rounded(radius)` clips a box to its rounded corners.
- **Another memoised row (`open_mask`):** a set in `State` that changes what a card draws (`diff_all`, `diff_open`, `inline_diffs`)
  must be hashed into it too, or a click changes state and nothing redraws (a test showed it: the toggle's handler ran).
- **Under `virtual_rows` a card taller than the view isn't all built**: a headless test of its last rows scrolls first
  (`rt.wheel`) or checks the state instead.
- Not done: the hover underline on "Show N unchanged lines" (a `Span` underline needs the hover state in the view).

## Component 5, piece A (drafts and routing)

- **A click's handler runs after the frame's `poll`.** Switching session from a click changed `State.open`, but a
  `poll` that follows `open` to swap the text area's draft saw the old session until the next frame. The swap is
  also made in `open_session`, so the text area is right in the frame the click lands in; `poll` keeps it right
  for every other way `open` changes.
- **Scripted runs end the process at the last event.** A thread started for a file write can be cut off, so a
  script that wants the file written ends a second after its last keystroke (the write waits 0.3 s).
- **Pictures and scenes must not touch the data folder.** The drafts file is read and written only for a real
  window, or a headless run that sets `FLEET_NATIVE_DATA_DIR`.

## Component 3, piece D (reasoning, shell block, compaction, recap)

- **`TextStyle::italic()` works for a whole Markdown body**: `MarkdownStyle::body_font(sans(13.0, 400.0, 1.5).italic())` (and
  the same on the heading fonts) gives Fleet's italic thinking text. Inter has no italic file here, so it is a synthetic oblique.
- **No gradient text** (`background-clip: text`): the "Thinking…" shimmer is two stacked words, muted underneath and
  text-coloured on top with a `Cycle` fading the top one. `still` is the reduced-motion value.
- **No `mx_auto`**: a centred, max-width box is a `row().w_full().justify_center()` with the box (`w_full().max_w(..)`) in it.
- **`El::rotate` takes radians, clockwise like CSS** (`FRAC_PI_2` turns a right chevron to point down).
- **One `selectable` per text**: Fleet's "$ " before a command isn't selectable; here it is a muted `Span` in the same text.
- **A `Hash` bound on `memo` keys**: a `State` setting that changes a row (`Thinking`) goes into the key as well as into `open_mask`.
- **The shared `CARGO_TARGET_DIR` binary can be another session's build.** A live check that runs `target/debug/fleet-native` for
  minutes should copy the binary out first; a script step failed with "no element with key" because the binary had been replaced
  by a build without this piece.

## Component 5, piece B (the frame and the toolbar)

- **`focus_within_ring(1.0, -1.0, colour)`** is CSS `:focus-within` with a 1px ring just inside the border box: it draws over the
  frame's own border whether the pointer or the keyboard put focus in the text field. It doesn't animate (Fleet's 150 ms
  `border-color` transition is not drawn).
- **`color-mix(accent 55%, border)` with a translucent border**: `Color::mix` is a straight blend in linear light, which turns a
  white 7.5% border into a grey-white line. `Theme::accent_border` blends premultiplied instead; it is still a linear blend where
  the browser mixes sRGB numbers, so the indigo is a little lighter than Fleet's.
- **`.border(w, c)` without `.border_layout()` doesn't add the border to the box's size**: an error box with a 1px border and
  `py(10)` measured 37px, 3px under Fleet's. Add `border_layout()` when the border is part of the size in CSS.
- **`text_input` has no read-only or disabled state**: `.disabled(true)` only keeps it out of focus (so nothing can be typed); the
  dimmed look is `.opacity(0.5)`.
- **A scene can't set the draft directly**: the first `poll` replaces the text area with the open session's stored draft, so a
  scene writes the text into `State.drafts` (`State::show_draft`). The same first look must not clear per-session state a scene sets.
- **`El::on_key` on an ancestor** of the text field is where Ctrl/Cmd+Enter is read; plain Enter and Shift+Enter stay the
  field's (`on_submit`, newline). A headless test can't tell which of the two sent a Ctrl+Enter on an idle session (both send), so
  the steer-versus-queue difference is first testable in piece I.

## Component 5, piece C (choice logic)

Nothing in Rune was needed: this piece is plain Rust. Two things for whoever builds on it:

- **Number formatting is ours to write.** Rust has no `Intl`; `src/composer/format.rs` writes en-US grouping itself, and
  rounds the decimal text half up. Rust's `format!("{:.1}", 1.25)` gives "1.2" (ties to even), JavaScript's `toFixed` and
  `Intl` give "1.3", so a token count of 1,250 would have drawn differently from Fleet's meter.
- **Fleet's `watch` is a state machine here.** `EffortMemory::update` runs only when `[fetched, key, levels]` changed since
  its last run, as Vue's `watch` does; call it from `poll` after the catalog or preferences change, with the draft's effort.

## Component 3, piece E (message frame)

- **A clipboard write from a handler exists at this pin**: `.on_press_tx(|s, tx| tx.copy(text))` (`Tx::copy`). The older notes saying
  there is none are out of date; the message's copy button uses it. Headless runs never write the system clipboard, so a test can
  only check the state around it. (The tool output's Copy and the palette's "Copy session ID" can now be switched on.)
- **Out-of-flow children work outside the parent's box**: `.pin(left, top, right, bottom)` takes negative values
  (`pin(Some(0.0), None, None, Some(-20.0))` hangs a pill below its parent), nothing clips it, and `.paint_order(n)` lifts it over the
  next row. `.hover_group()` on the parent plus `.group_opacity(0.0, 1.0)` on the child reveals it, hit area included.
- **`.border_sides([top, right, bottom, left], c)`** is the one-sided border (the note's left rule, the prompt's top rule). It takes no layout space: pad the box.
- **`memo` re-keys a row by its id**, so an element key set on the row's root (`msg-<id>`) is replaced by the id itself: scripts and tests find the row as `<id>`.
- **A script's first frames have no layout yet**: a `move` at 0.3s found no element; at 1.0s it did.
- **No inline chips or clickable inline code in `Markdown`**: the `@`, `$` and file-link chips of rows 2.25-2.29 need a pre-pass that splits the text into runs; not done.

## Component 5, piece D (the pickers on the session's lists)

- **A kit menu scrolls only if its list column does.** `menu::panel_capped` puts `.scroll(..).max_h(..)` on the rows column, not on
  the panel; a `max_h` on the padded panel overflows instead. A scrolling column also shrinks to its widest child's width, so the
  capped menus take a fixed 380 px (Fleet's selector width). A list of 20 models shows 10 rows and scrolls.
- **Measure a width with `on_layout` on the element, store it in `State`.** The narrow rule (composer under 640 px) reads
  `State.pickers.composer_w`, set from the frame's `on_layout`; it is 0 on the first frame, which counts as roomy.
- **`Event::Other(Kind, props)` is how an event nobody types yet arrives.** `harness.catalog_changed` is read in `State::dispatch`
  (no `tx` there), so it can only start a thread, not touch Rune state.

## Component 3, piece F (turn folding, logic)

- **Nothing new about Rune: this piece has no view.** A scene can still show logic: `mock::fold` builds a conversation, runs
  `build_list` and `fold_turns`, then keeps or drops the drawn `Part`s by id, so the picture is what the logic decided.
- **The fold-row glyph `▸` is missing from the text font** (it draws as a box); the scene says the line in plain words, and the
  row in piece G uses an icon.

## Component 3, piece G (fold row and the stream's list)

- **`virtual_rows` indexes a list you own.** The folded list is shorter than the session's messages, so the stream keeps the
  mapping (`FoldPlan::visible`) and its row closure looks the message up through it; the row key and the memo key use the
  message id, and the memo key carries a hash of what folding did to that message (open state, calls kept).
- **`focus_ring(width, offset, colour)` takes a negative offset** for an inset ring (`outline-offset: -2px`).
- **`hover_group` plus `group_color` on children** gives CSS's "button hover turns its text and icon `--text`".
## Component 5, piece E (the selector dropdown)

- **Up and Down in a focused text field are the field's**, even on an ancestor's `on_key`. A list driven from a filter box
  (the selector, the palette) needs `.on_input_key(..)` on the field itself; `on_key` only ever saw the typed characters.
  `.on_input_change(..)` is where "typing moves the highlight to the first match" goes (no polling).
- **`on_hover` fires on the element the pointer is over, not on its ancestors**: a row made of a button and a star put it on the
  button. Scroll-under-a-still-pointer didn't fight the arrow keys in our runs.
- **"N more" without a visible range:** each row's `on_layout` rect (window points, after scrolling) against the scroll viewport's
  own `on_layout` bottom gives Fleet's `offsetTop + height/2 > bottom` rule exactly. Write the state only when it moved by more than half
  a point, or the handler re-runs the view every frame.
- **`anchored_fit` already keeps an overlay 8 px inside the window** and flips it, which is Fleet's `place()` for the up case; the
  width cap (`100vw - 16`) still needs the window's size: measure it with `on_layout` on the root frame.
- **A filled lucide icon** is the same paths with `fill="currentColor"`: `tools/gen-icons.py` takes `star:filled`.
- **An overlay is mid-entrance for about 120 ms**: a headless test that reads a rect or an opacity, or hovers a row, right after
  opening sees the 0.97 scale and the fade. Run 30 frames first.

## Component 3, piece H (failure card and retry bar)

- **No hover border colour.** A pill whose border turns to the accent on hover is a 1px ring (`fill(border)`, `hover(accent)`) around the pill's own fill.
- **`rich` text takes links**: `Span` for the look, `Link::new(range, id)` for the press, `on_link` with `tx.open_url` to open it. Give a text in a `scroll` box `w_full` or it won't wrap at the box.
- **`ureq` treats a 404 or 500 as an error** unless the agent is built with `http_status_as_error(false)`; the retry calls need the status itself.
- **A countdown redraws through the row's `memo` key**: put the tick's bucket (`now / tick`) in it and have `poll` compare the bucket and `wake_after` the next tick; no timer in the view.

## Component 3, piece I (working line and interrupted card)

- **A virtual row takes the key of its own** (the key function's): a `.key()` on the element a row builder returns is replaced, so a test can't `find` it. Key the element inside.
- **A negative margin works** (`mb(-6.0)`, `ml(-5.0)`): used to take back space a shared padding leaves, and for Fleet's `margin-left: -5px`.
- **No gradient clipped to glyphs**, so Fleet's shimmer on "Working" isn't drawn (it stays plain `--muted`, as under reduced motion). A `Span` takes a colour and a weight, which is enough for the card's bold parts (`rich` with `Span::new(start, end).color(..).weight(500.0)`).
- **A conversation pinned to its end moves a row above it out from under the pointer**: a test that opens a fold and then clicks it again must scroll back first (`rt.wheel`), or the click lands on the header.

## Component 5, piece F (usage bars)

- **A track and a fill of one colour at two alphas** is two nested `row()`s: the outer `fill(ink.alpha(0.22))` with `radius(999)`, the inner `w(percent * width)`. No overflow clip was needed because the fill never exceeds the track.
- **A scene can't scroll a list**, but a `--script` run given the same `--scene` can: `{"type":"key","name":"Down"}` repeated moves the highlight and scrolls it into view (`scripts/composer-usage.json`).
- **`ElementInfo` has `opacity`, `text` and (at the current pin) `help`, but no checked state**: `.checked(on)` on a `role("switch")`
  isn't read back, so a test reads a switch from the label it is given ("Design: on", component 19 N's step chips).
- **`El::on_scroll_offset(|ScrollPos|)` is how an app reads a scroll box, and it may not change state.** `ScrollPos` has `offset`, `max` and `viewport` (`[x, y]`). Leave the value in an `Rc<Cell<Option<ScrollPos>>>` the state owns and read it in `poll`, which runs on every frame of a scroll; return true from `poll` when the view must change (the "Jump to latest" pill). `on_at_bottom` and `on_far_from_bottom(min)` do state changes directly, but "far" is more than the box's own height, not Fleet's 80 px.
- **A scroll box reports growth and movement in one `ScrollPos`**, with no separate resize event. Tell them apart by comparing `max + viewport` and `viewport` with the last report: the offset unchanged and the size changed is a layout change, which Fleet's observers handle before any scroll event is read (`conv::pin`).
- **Wheel and key input don't reach the app from a scroll box** (`on_wheel` is for canvases). "The user moved it" is a move up of the offset that the content didn't cause (the end didn't drop with it); a wheel over a code block that scrolls on its own never moves the stream, which is what Fleet's nested-scroller test wants.
- **`Tx::scroll_to_bottom` glides, and passes through places far from the end**; an app that unpins when far from the end must hold the pin for the glide (`Pin::follow`). `scroll_to_bottom_instant` doesn't glide (Jump to latest uses it, as Fleet's click is instant).
- **`Tx::scroll_center(key)` reveals a `virtual_rows` row that isn't built**, by the key the key closure returned. The `.key()` of the element inside the row (`msg-<id>`) is found only once the row is built. A glide across 1,700 rows takes about three seconds in a headless run.
- **`El::outline(width, offset, color)` draws Fleet's `outline` + `outline-offset`**, but has no motion, so the jumped-to ring (`--shown`) ends hard where Fleet fades it over 400 ms. **`El::raise()` is a flag, not a hover state**: a hovered message can't come forward over the next one.
- **Prepending to a `virtual_rows` list keeps the reader's rows where they were** (test `keeps_the_reading_position_when_they_arrive_above`: ten rows added above, the row at 300 pt is still at 300 pt), so the "remember the height, put the offset back" step of `use-stream-scroll.ts` isn't ported.

## Component 3, piece K (background strip)

- **A clock that rests**: `poll` asks the strip for the time to its next visible change (`WorkStrip::next_change_ms`) and calls `tx.wake_after` with it, and returns true only when what is drawn differs from the last frame (`WorkStrip::changed`). Nothing wakes while nothing is shown.
- **A container query** (`@container (max-width: 440px)`) is the element's own measured width kept in `State` by `on_layout`, read one frame later.
- **No hover border colour** again: the ghost button gets a hover fill only.
- **`text(...).tracking(0.04 * size)` with `.to_uppercase()`** is the way to draw `uppercase; letter-spacing: 0.04em`.

## Component 5, piece G (the context ring)

- **`stroke_path` has no arc.** A ring is a closed polyline of 96 points on the circle (`view/ring.rs`); at 18 px it
  draws as a smooth circle, so the 48-point fallback was not needed.
- **A partial ring: `live_trim` on a closed stroke.** Draw the whole circle with a round cap and bind
  `live_trim(Live<[f32; 2]>.smooth(Spring::new(0.3, 0.0)))` to `[0, fill]`: the arc starts at the polyline's first point
  and the round caps follow the cut. Publish the end with `Live::set` while the same session shows and `Live::snap` when
  another one does, or a switch of session sweeps. `Live::set` takes `&self`, so a view may do it.
- **A dashed stroke is several strokes.** There is no dash pattern; draw each dash as its own open stroke with
  `LineCap::Butt`.
- **Translucent ink is brighter than CSS's.** Rune blends in linear light, so `--text` at 62% over a dark card is much
  paler than the browser's. Where a solid colour sits under it (the ring on the composer card), mix the two in sRGB
  numbers yourself and draw the result opaque. A faded closed polyline also blends twice where its joints overlap.

## Component 3, piece L (work actions)

- **`stick_bottom` is Fleet's "follow unless you scrolled up" for the output box** (WorkOutput's 24 px test): a `col().max_h(220.0).scroll(ScrollSpec { stick_bottom: true })` holding one wrapped, selectable `text` needs nothing more.
- **A request that comes back later is tied to the opening that asked** (`Reader::epoch`): close and open again, and the old answer finds a different epoch and is dropped. A timer is a `next_at` the strip's `poll` checks, with `tx.wake_after` to wake it; closing the panel removes the reader, which stops it.
- **`gen` is a reserved word in this edition**; a field or argument called `gen` doesn't compile.
- **No `shrink0` and no `max_w` that subtracts a margin**: a panel indented by `ml` takes the column's width without `w_full`.

## Component 5, piece H (the ring's popover)

- **A popover is `ui::overlay::anchored(card, key, Side::Top, Align::End, 8.0)` over a `scrim`**, opened by `Open::Popover("context")`;
  the card's `.modal(true, close)` gives Escape and focus return. The anchor's `key` ("context-ring") is the button's, so the
  button stays on screen under it and can keep its open look (the wash and the ink) from the same state.
- **A CSS border takes room; Rune's doesn't unless asked.** `.border(1.0, c)` draws over the box: add `.border_layout()` or the
  padding is measured from the outer edge. A `border-top` only is `.border_sides([1.0, 0.0, 0.0, 0.0], c)`.
- **No percent widths from a measured parent.** The popover is a fixed 340 wide, so the meters, the tick, the dashed line and
  the bars use the inner width (310) as a constant. A popover whose width followed `100vw - 16` would need `on_layout`.
- **`on_hover_change(|s, inside, tx|)` is not a hit target**: it is the "mouse left the chart" the caption needs. The bar's own
  `on_hover` (once on entry) sets the hovered turn; the chart's `on_hover_change(false)` clears it.
- **Tabular figures are a font feature**: `sans(..).features("'tnum' 1")`, not an element method.
- **Meters don't animate here.** Fleet's `transition: width 300ms` on the size meter and the limits' fills is not ported: the
  bars are drawn at their final width. (`live_trim` moves only a canvas stroke.)
- **A spinner doesn't turn** (no looping rotation): it fades, as in the older-messages row. **Out of date (22 Nov 2026):** `.looping(LoopSpec::rotation(period))` turns an element forever and holds `still` under reduced motion (`crates/rune/README.md`, "Periodic motion"); the spinners can turn.

## Component 5, piece I (queue and steer)

- **A text field's own keys are `on_input_key`, on the field.** `El::on_key` on the field or on a parent never sees a key the
  field handles, and Ctrl/Cmd+Enter reaches the field's `on_submit` (the same call as Enter) first. To tell the two apart, put
  `on_input_key` on the field and return `true` for Ctrl/Cmd+Enter; an `on_key` on the frame silently does nothing there.
- **`on_submit` can't see the modifiers** (`Tx` has none), so "Enter does one thing, Ctrl+Enter the other" needs the above.
- **A list the hub also replaces needs one number per load**: a `session.queue` event and a `GET` race, so every load takes a
  generation and is dropped if an event bumped it meanwhile (`fleet::queue::Queues`).

## Component 5, piece J (autocomplete logic)

Nothing of Rune is used by this piece: it is plain Rust. What it settles for K and L:

- **Positions are byte offsets on character boundaries**, as the text edit's caret is; Fleet's are UTF-16 indices. Slice with them only after the caret is known to be on a boundary.
- **`Autocomplete::on_key` is a pure function of the text, caret and lists**; it returns an `Edit` and a `KeyOutcome` (`consumed`, `stop`, `swallowed`). The view applies the edit, sets the caret now and again on the next tick, and calls `poll` after the lists, text or caret change.

## Component 3, piece M (sub-agent rows and background notices)

- **A memo key tuple stops at 12 elements** (`Hash` is only implemented that far): nest the newest keys in a sub-tuple.
- **A row that reads state at draw time needs that state in its memo key.** The sub-agent row reads the delegation list, which changes with no change to the message; `view::agent_row::key` hashes what the row shows.
- **`group_opacity` inside a row with `.hover_group()`** shows "Open" on that row's hover only; the message's own group is the next one out.

## Component 5, piece K (the autocomplete popup)

- **A popup over a field is `anchored_fit(frame_key, Side::Top, Align::Start, 6.0)` in `ui::overlays`**, with `.w(measured frame width)` from the frame's `on_layout`: there is no "as wide as the anchor". It needs no scrim and takes no focus, so the field keeps typing.
- **A pressed row keeps the field's focus with `.focusable(false)`**; then `tx.focus("composer", false)` after the edit is belt and braces.
- **The field keeps Esc for itself** (an `on_key` on the app never sees it while a text field has focus), so Esc to interrupt, and to close a popup, are both answered in the field's `on_input_key`.
- **`on_input_key` runs before the editor and before `on_submit`**: returning `true` for Enter stops the send, and for Tab keeps focus in the field.
- **Replacing the text as one undoable edit**: `set_selection(0, len)` then `insert(new)`; `replace_text` drops the undo history. Set the caret after, and once more on the next frame.
- **`rich(..).truncate()` gives a one-line label with a marked range** (a `Span` colour and weight): the popup's match mark.
- **A footer under a scroller is a sibling, not a sticky child**, which draws the same and needs no negative margins.

## Component 5, piece L (references in the draft)

- A `text_input` can be tinted without being rich text: `.text_highlights([TextHighlight::new(bytes, colour).pad(3.0)])` on the field's `El` paints a rounded fill behind each wrapped line of a byte range, never reshapes the text and leaves bold/italic shortcuts and HTML paste off. So Fleet's mirror element is not needed. The corner radius isn't settable (Fleet's is 4 px).
- `on_input_key` runs before the field's own keys, so a handler can call `TextEdit::set_selection(start, end)` and return `false`: the field then deletes that selection as one undoable edit (Backspace takes a reference whole, and undo brings it back). Returning `true` consumes the key (the arrows).
- There is no click or key-up hook on the field; the caret is moved out of a reference by checking `TextEdit::range()` each frame.

## Component 3, piece N (images and the lightbox)

- **`ImageCache::load(bytes)` returns at once and decodes on a worker**: keep the `Image`, ask `is_ready()` and `error()` from `poll` (with `tx.wake_after`), and draw `el().pixels(image.pixels())`; the element draws whatever has decoded by then.
- **A scene is applied before `setup`**, so `State::cache` is `None` there: bytes handed over by a scene must wait until the cache exists (`Images::parked`).
- **`pixels_fit(.., SurfaceFit::Fit | Fill)` is `object-fit` `contain` and `cover`, but only about the centre.** There is no top-left anchor.
- **A view can't start work, so it leaves a want in a `RefCell`** that `poll` drains (`Images::want`); the row's memo key has to include the picture's state or the row never redraws when it arrives.
- **A full-window overlay is a `col().absolute(0,0).w_full().h_full()` with `.modal(false, close)`** and its own `on_press(close)`; the picture inside takes an empty `on_press` so a press on it doesn't reach the scrim. `on_layout` on that frame is the only way to know the window's size (the lightbox's 90%).
- **Focus can't be read headless**: after `autofocus` on a modal's button, Space and Enter pressed nothing in a `Runtime`, so "focus lands on the close button / returns to the thumbnail" is unverified by tests.

## Component 5, piece M (history, shell mode, `/btw`)

- A list that sits on the frame's top edge is `anchored_fit("composer-frame", Side::Top, Align::Start, 0.0)` like the popup; per-corner rounding needs both `.corners([..])` and `.clip_corners([..])` (the rows would square it off otherwise), and a border without its bottom is `.border_sides([1.0, 1.0, 1.0, 0.0], c).border_layout()`.
- A scroller that should open on its last row is `ScrollSpec { stick_bottom: true }`; moving the highlight calls `tx.scroll_into_view_in(scroller_key, row_key)`.
- `on_input_key` sees every key, including a plain character, so "any other key closes the list and goes on" is `return false` after closing; no keydown/keyup split is needed.
- `on_hover` fires once as the pointer enters a row: it is Fleet's `mouseenter`, and with the keyboard's highlight in the same field the pointer moving over a row wins only when it moves.

## Component 3, piece O (browser steps)

- **A row's memo key has to include everything that arrives later for it**: a step's picture decoding after the row was built left a black box until its state was part of the key (`browser_steps::key`), just as in piece N.
- **A headless test can't read a drawn text**: `Runtime` finds elements by key and position only, so wording is tested in the plain logic (`conv::browser::heading`) and the test checks the element exists.
- **`El` has `px` and `py` but no `p`.**

## Pictures in the composer (component 5, piece N)

- `tx.open_file` picks **one** file; Fleet's picker is multiple. Several come from a paste or a drop.
- `on_input_paste` hears a clipboard with `content.files()` and `content.image()` (PNG bytes); `Runtime::paste_content` feeds them in a test. Returning false lets the field paste the text, which is how a spreadsheet's picture is left for its text.
- `on_drop_files` on the frame plus `on_drop_hover` is the whole drop; `Runtime::drag_files(paths, at)` and `drop_files` drive it. No drops on Wayland.
- A hover group follows keyboard focus too, so a control shown with `group_opacity(0, 1)` shows when Tab reaches it; the hidden button still takes a click, so a test can press it without hovering.
- A file read, a 5 MB base64 encode and a decode belong on a thread: a thread answers through an `mpsc` channel that `poll` drains, with `tx.wake_after(30 ms)` while one is out (the picture cache's own loads work like this).

## Component 3, piece P (find bar)

- **Ctrl/Cmd+F reaches the window's `on_key` after a focused text field passes it on** (step 7 of "Key routing"), so `tx.focused()` names the field and the app decides by its key; the composer's `text_input` passes it too.
- **A bar over a scroller is an `absolute(0,0).w_full()` row with `justify_end` and padding** inside the column; it doesn't take presses outside the card.
- **`on_input_key` sees Escape and Enter first**, so "Esc closes, Shift+Enter steps back" is a match on `ev.key` and `ev.mods.shift`. Selecting a field's text is `TextEdit::set_selection(0, len)`, with `tx.focus(key, false)` for the focus.
- **A headless `Runtime` can't say what has focus, but `rt.elements()` has a `focused` flag per element**, which a test can read by key.
- **There is no `search` field type with a clear button**: Chromium draws one in Fleet's field; here it is plain.
- **A row keeping its width is `.shrink().min_w(0.0)` with `.wrap()` on the text**: without it a long status line ran out of the card.

## Files, pastes and terminal lines in the composer (component 5, piece O)

- `on_input_key` runs before the field's own handling and may change the same `State::draft` the field then edits: pasting the offered text there and returning false makes a typed key land after it (`TextEdit::set_selection` + `insert`, then the caret is after the text).
- `on_input_paste` gets `content.text()` too: return true after taking a big paste so the field doesn't also paste it.
- **A new element fades in**, so a `Runtime` click in the first frames after it appears misses it (`presented` shows an opacity under 1): run about 30 frames before clicking a bar that was just drawn.
- Tests that write files must not share a folder by name: tests run side by side in one process, so two tests writing `build-output.log` under the same folder delete each other's file. A counter in the folder name fixes it.
- A body that isn't JSON needs `ureq`'s `send(Vec<u8>)` with the `content-type` set by hand, and `http_status_as_error(false)` to read the server's reason from a 4xx.

## Component 3, piece Q (find marks)

- **`Markdown::highlights(markdown::find(src, query, Case::Insensitive))` marks the drawn text** across bold, code, lists, tables and code blocks with no layout change; the colours are `MarkdownStyle::highlight_*` and `highlight_current_*` (set the ring to `None` for Fleet's look), and `Highlight::current().key(k)` makes the current mark reachable by `tx.scroll_center(k)`.
- **`El::text_highlights([TextHighlight::new(bytes, bg).color(fg).key(k)])` works on a `rich` text with its own colour spans**, so tool output keeps its terminal colours under the washes. The corner radius isn't settable.
- **A scroll to a mark must wait for its row**: `scroll_center(message)` first builds a virtual row; the mark's key exists only after, so the second scroll is timed (250 ms) from `poll`.
- **`memo` keys are tuples of at most 12 elements** (`Hash`): nest the extras.
- **Markdown's find counts per rendered block and ignores markup**, so it can disagree with a count of the source; use it for what is drawn, and `locate` clamps.

## Component 3, piece R (export builder)

- Nothing new about Rune: the builder is plain logic. Its scene (`export-markdown`) is one `text` in a scroll box, set in `mono(10.5, 400, 14/10.5)` to match the dialog's Source tab. A `text` keeps `\n` and blank lines as written, so the file draws as it is written.
- The standard library can't read the machine's time zone; `conv::time::local_offset_secs` (Unix only) is what the export dialog (piece S) should pass as `offset_secs`.
## The composer's sweep (component 5, piece P)

- `Tx::scroll_into_view_in(scroller, row)` needs the row's key from the previous frame; keying every popup row by index (`autocomplete-row-{i}`) is enough, also when the arrows wrap.
- A headless `Runtime` can drive a live server: give the `Link` a `Post` that sends each job down a channel, and `rt.update(job)` them between frames (`fleet::tests::live::live_composer`). Type with `Key::Text`, send with `Key::Enter`, Ctrl+Enter with `Mods { ctrl: true, .. }`.
- A menu with more than eight models from several providers is grouped, so `model-pop-item{N}` is the Nth displayed row, not the Nth model: pick through the filter (type, Enter) in tests that must not depend on order.

## Export dialog (component 3, piece S)

- **A save dialog exists at this pin**: `tx.save_file(FileDialogOptions { file_name, .. }.filter("Markdown", &["md"]), |s, answer, tx| ..)` answers `Ok(Some(path))`, `Ok(None)` on cancel. A headless runtime lists it (`rt.file_dialog()` gives its kind and options) and `rt.resolve_file_dialog(Some(path))` answers it. Write the file on a thread; tests write to a temporary folder.
- **`Tx::copy` is the clipboard write**; it reports no failure, so a copy-failed message can't be shown.
- **`MarkdownStyle` has no heading colour or rule**: Fleet's preview draws headings in the accent colour with a line under h1 and h2; only the fonts can be matched.
- **A modal's Esc and outside press** both call the `.modal(true, dismiss)` closure, so one handler closes a dialog.
## Session list, piece A (projects, groups, the filter's data)

- Nothing new about Rune: the grouping, the filter and the pin split are plain Rust in `src/fleet/groups.rs`. One shape fits both the app's `Session` and the tests' own item: a `Listed` trait (id, title, status word, `Listing`), so the logic runs on whatever row a later piece adds.
- A scene that must show an empty project builds it in `mock/groups.rs`; the invented list gets project ids and positions in `mock::projects()` so the filter's project-name match is tested without a server.

## Session list, piece B (sessions store, selection by id)

- Nothing new about Rune. A scripted run (`--script`) steps a virtual clock one frame at a time (about 0.4 s of wall time per frame on a software renderer), while the app's own threads and `Instant` timers run in wall time. To change the server in the middle of a run, work out the wall time of the mark first (here 24 s of script took about 240 s) and change the server about 60 % of the way.
- A scene or test that gives the open session another id must also move the route: `State::set_session_id`.

## Session list, piece D (chips, progress, the ring)

- A ring that eases (`transition: stroke-dashoffset`) is a `canvas` of the full circle with `.live_trim(binding)`: the
  arc is cut by a `Live<[f32; 2]>` smoothed with `Spring::new(0.15, 0.0)`. A view runs every frame and has no place for
  per-row state, so the live values sit in a `RefCell<HashMap<owner, Live>>` on `State` (`view::progress_ring::Trims`);
  the first draw of an owner `snap`s, later ones `set`. Aim it even at zero, or the first item ticked lands instead of
  sweeping.
- A translucent track (`--text` at 14%) blends in linear light and draws brighter than a browser's; mix it in sRGB
  against the panel colour (`view::ring::srgb_mix`) as the context ring does.
- `icon(..)` draws at its file's stroke (2): Fleet's running chip uses 2.25 on an 11 px icon and there is no way to ask
  for it, so the chip's loader is a touch lighter.

## Component 3, piece T (the sweep)

- **`MarkdownStyle::muted` is the list marker colour** (bullets and numbers) and the colour of h4 and deeper headings, nothing else; set it to the marker tint to get Fleet's accent-coloured markers. Number markers draw in the body weight (Fleet's are 600 and tabular).
- **`list_markers_outside(true)`** puts a list's markers a space's width before the text, as a browser sets `1. `; without it a two-digit number touches its text.
- **Code colours**: `MarkdownStyle::syntax(kind, colour)` per `Syntax` kind. hljs has eight colours; Rune's kinds map many-to-one (`Number` stands for hljs's `number` and `literal`; `Type` for `type`, `built_in`, `variable`; `Property` and `Attribute` for `attr`), and nothing sets italics for comments. Set `Punctuation`, `Operator` and `Parameter` to the text colour to match Fleet, which leaves them plain.
- **A scripted run can scroll the stream**: `{"type":"scroll","key":"stream","dy":-700,"dur":0.3}`; `{"type":"key","name":"l","cmd":true,"shift":true}` flips the theme for a live picture. Rune draws a debug frame in 120-400 ms under load, so a live picture of a 45 s state needs a release build or a quiet machine.
- **A headless test has no fonts**: `rune::text::measure` in a test measures a fallback face (a 100-character mono line came out 682 px at 12.5 px, not 750); only the running app has the bundled fonts.
## Session list, piece C (row anatomy and status)

- Rune's colours are linear and CSS mixes the sRGB numbers, so `color-mix(in srgb, var(--warning) 55%, var(--text))` is
  `Theme::text_mix`, which decodes, mixes and encodes (`rune::color::srgb_decode/encode`). A straight `Color::mix` is a
  little off.
- A row's title goes from dimmed to full `--text` on hover with `hover_group()` on the row and `group_color(..)` on the
  text; no extra state is needed. Hover has no transition, so Fleet's 150 ms colour ease is instant here.
- `rt.find(key)` gives an element's centre, so two keyed elements of different widths don't share an x: test alignment
  on elements of the same size (the glyph and its empty slot), not on titles.
- A negative margin (`.mr(-4.0)`) works for the caret slot. `focus_ring(2.0, -2.0, ..)` takes a negative offset for
  Fleet's inset outline; not yet looked at with the keyboard.

## Session list, piece E (panel states and the filter)

- **`TextEdit::focused` is kept by the runtime** for a `text_input` bound with `on_edit`, so a view can read `s.filter.focused` to
  change the fill and border while it has the caret (CSS `:focus`). `focus_within_ring` draws a ring over the border instead,
  which isn't Fleet's look. There is no transition, so Fleet's 150 ms ease is instant.
- **No hover border colour**: `.hover(c)` changes the fill only. A button whose hover is a border (Fleet's "Try again", the New
  session button) can't show it.
- **Centred wrapped text**: `.wrap().w_full().align(Align::Center)` inside a column with `.items_center()` gives the empty
  state's two lines. (used)
## Permission card (component 4, piece A)

- `scroll_xy()` on a box sizes its content to what is in it. A diff line's tint therefore ends at the longest line, not
  at the box's right edge as Fleet's `display: block` lines do. Each line gets the longest line's width (7.2 px a
  character at 12 px mono) so the tints at least agree; when every line is shorter than the box they stop short. Wanted:
  a child that is "at least as wide as the scroller".
- No border colour on hover: the choice rows keep Fleet's border where Fleet tints it toward the accent at 35%.
- A `Span` can be underlined, but only for a whole run, not on hover, so "Permission settings" changes no more than colour.
- A fake model that writes a path takes it from its own `HOME`: start the scripted model with an invented `HOME`, or
  the picture of Fleet's card shows the machine's home folder.

## Session list, piece F (settings file, project header)

- **A scene can open the app's own folder safely**: `--script`/`--snap` runs read and write `settings.json` only when
  `FLEET_NATIVE_DATA_DIR` names a folder, so a picture never touches the real one. The write happens on a thread; a scripted run
  that folds a group and ends has written it by the time the process exits (checked: a second start folded the same group).
- **`.label(..)` on a keyed column** gives the panel its "Sessions context panel" name without changing layout.
- **`focus_ring(2.0, -2.0, accent)` on a pressable row** is the header's `:focus-visible`; Rune draws it for keyboard focus only.
  Not looked at with a real keyboard here (no window), only that the call builds and the headers draw the same.
- Still no transition: the caret's 150 ms turn and the 150 ms height animation on folding are instant (as for every other ease).

## Permission asks live (component 4, piece B)

- **A window `on_key` sees a key from a focused button** (step 7 of "Key routing") with `tx.focused()` naming it, so a card's number keys, Esc and Enter are one handler that first checks the focused key is one of the card's. A focused text field's keys come to its `on_input_key` first, where `Key::Enter` and `Key::Escape` can answer.
- **`tx.focus(key, ring)`**: `ring = false` for a card taking the keyboard as it appears (Fleet's script focus draws no ring), `true` for an arrow moving it. It needs the element on screen, so ask in the same handler that added the card and it lands next frame.
- **A row with `on_press` needs `.focusable(true)`** to be reached by Tab or `tx.focus`; `.focusable(false)` on the deny row keeps the caret in its field.
- **Two siblings with one key warn and rename the later**: a second card is keyed by its ask's id. Keys inside different cards still collide for `find` and `tx.focus`, so keys act on the first card.
- **A headless test reads focus from `rt.elements()`' `focused` flag**; `rt.key(..)` after `tx.focus` needs a frame in between.

## Session list, piece G (project menu, rename, delete)

- **A pressable row is not focusable until you say so**: `Tab` never reached the project header and `tx.focus("project-1", true)`
  did nothing. `.focusable(true)` fixes both, and `on_key` (F2) then fires on it. Fleet's header is a `<button>`, so it is focusable there.
- **`El::grow` has no flex basis**: a field with `.w(193).grow(1)` beside a `grow(1)` spacer came out half as wide as CSS's
  `flex: 1 1 auto` next to `flex: 1`. The width is worked out from the panel's own (`project_header::editing`) instead.
- **A text field's blur isn't an event**: only its model (`TextEdit::focused`) says it lost focus. An edit that ends on blur
  (`InlineEdit`) polls it (`keep_rename`, from `poll`), remembering that it had focus first, so the Enter that already ended the
  edit isn't followed by a second commit.
- **A dialog with state of its own needs its own Escape**: `ui::dialog::dialog` sent Escape to `Ui::open`; `Dialog::dismiss` names
  what it does instead.
- **`group_opacity(0.0, 1.0)` under a `hover_group`** is `opacity-0 group-hover:opacity-100`, and an invisible element still takes
  the press, as in CSS; a headless script can `move` the pointer over a header to show it.

## Question card (component 4, piece C)

- **A field has no change hook** (`on_edit` only hands back the `TextEdit`), so a rule that reacts to typing (typing drops a pick) runs from `poll` and shows one frame late.
- **`rich` text keeps `\n` as a line break** and wraps inside a `w_full` column; a `Span` with `underline` plus a `Link` gives Fleet's underlined address that opens with `tx.open_url`.
- **Negative margins work** (`.mt(-4.0)` for the "Pick any that apply." line).
- **`focus_within_ring(1.0, -1.0, ..)`** on a row is `:focus-within` for a field inside it; `text_input` line height 1.7 matches a browser input's row height at 13px.

## Question card live (component 4, piece D)

- **A text field inside a `memo`d row must have its own `TextEdit` in the memo key** (`(&card.typed, ..).hash(..)`), not just its text: the memo keeps the handlers of its first build, and a field built without its focus and selection in the key never took a click or a keystroke (the card looked right and typed nothing). It took a bisect to find; the memo docs say so in one line.
- **`Runtime::elements()` shows focus** (`e.focused`, `e.key`): a headless test reads where the keyboard is without a window.

## Session list, piece H (row rename, confirm dialog)

- **A headless modal focuses itself, not its `autofocus` child**: `rt.elements()` shows the dialog panel as focused whatever is marked
  `.autofocus()`, and `tx.focus(key, true)` from the handler that opens it fails (the buttons aren't mounted yet). Mark the button
  `autofocus` and look in a window. Keys injected with xdotool into an Xvfb window with no window manager did not reach the app here.
- **`on_double_press_tx` fires after the first press's handler too**: a row that opens on press and renames on double press opens twice
  first; two `click`s in a test with no pointer move between them are a double press.
- **A text field starts scrolled to the selection's end**: select-all on a title longer than the field shows its end, a browser keeps the start.

## Machines, piece A (per-machine tokens)

- **ureq middleware sees the final URL**: `request.uri()` in `MiddlewareNext` is the address about to be called, so one agent middleware can pick a token per machine; nothing in Rune is involved. A `static` `RwLock<HashMap>` can't be built in a const, so hold it as `Option<HashMap>` and `get_or_insert_with`.

## New session, piece A (create request, plan line, naming; no Rune)

- **Nothing here needed Rune**, by design: `src/fleet/new_session/` has no Rune import, so the pieces after it read plain types.
- **Cargo.lock has no `regex` and no `unicode-normalization`** (Rune doesn't bring them). The naming preview needs both (a
  `capture` pattern, NFKD for accents), so `pattern.rs` is a small matcher and the slug folds accents by table rather than
  adding two crates to three systems' builds. Reach for these only if a later piece needs a full regex.

## Machines, piece B (reading the machines; no Rune)

- **Nothing here needed Rune**: `machines.rs` is plain Rust, `machine_poll.rs` is threads that hand answers to the state with `Post::update`.
  A headless runtime doesn't drain `AppHandle::update`, so a test of a poll posts to its own queue and pumps it, as `tests/auth.rs` does.
- **Rune's `Session` row (`mock::Session`) has no `Clone` or `Debug`**, so the per-machine state can't derive either; tests compare titles.

## New session, piece B (draft, defaults, harness choice; no Rune)

- **Nothing here needed Rune.** `State` already had a `draft` (the composer's text), so the page's draft is `State::new_draft`.
- **One settings file, one writer**: remembered choices went into `settings::Sidebar` rather than a second file, so the existing ordered, whole-file write covers them.

## Component 4, piece E (the Allowed chip and list)

- **One `rich` text does a mixed-weight wrapping line**: Fleet's rule row is words, an inline `<code>` and words that wrap anywhere.
  `rich(content, vec![Span::new(a, b).mono().size(..)])` with `.wrap()` draws it in one element, and `ElementInfo.text` reads back the
  whole sentence, so a test compares "Run bun test* without asking".
- **A fade over time is the app's clock, not Rune's**: `allowed-flash` (35% accent fading to none in 0.9 s) is drawn from
  `State.clock` in the view and kept redrawing by `poll` (`wake_after` 33 ms) until it has run. Rune's `Cycle` and transitions animate
  opacity of an element, not a background colour to nothing. Reduced motion is not read anywhere yet (the same gap as the other fades).
- **The popover gap is a parameter of `anchored`, not of `dialog::popover`** (which fixes 4 px): the chip's `side-offset 8` builds its
  own card with `lift` and `anchored(.., 8.0)`.

## New session, piece C (the page's data; no Rune)

- **Nothing here needed Rune.** The calls are threads that hand answers to the state with `Post::update`; a test of them posts to its own queue and pumps it (`tests_feeds.rs`, `live`), because a headless runtime doesn't drain `AppHandle::update`.
- **A thread's answer can arrive after the page moved on**, so each read is numbered on the UI thread and an answer whose number is no longer the latest is dropped: that is the whole of Fleet's `AbortController` here (a blocking `ureq` call can't be aborted, only ignored).

## Component 4, piece F (the page-pick card)

- Nothing new missing. A flex row needs no `shrink0` for its icon and button: children don't shrink unless `.shrink()` is set, so
  only the text column takes `.shrink().min_w(0.0)` for its ellipsis.
- A snapshot of a live run (`--server` with `--script ... --snap --at mark:..`) draws whatever has loaded by the mark: a mark too early
  (3 s after the tap) once showed no card because the canvas list hadn't come back yet; 6 s was enough.

## Machines, piece I (https and wss; no Rune)

- **Nothing here needed Rune.** A self-signed certificate made by `openssl req -x509` is `CA:TRUE`, which rustls/webpki refuses as a server certificate (`CaUsedAsEndEntity`) even when trusted: make a test certificate with `basicConstraints=CA:FALSE`.
- tungstenite takes any `Read + Write`, so the hub's TLS is a stream enum, not a feature; `set_read_timeout` goes through to the socket and rustls keeps a half-read record for the next read.

## New session, piece D (the route and the page shell)

- **No CSS `margin: auto`.** The empty block is centred in the stage with `.center()` on the stage column and `w_full().max_w(640.0)` on the block; that matches Fleet's `margin: auto` while the block is narrower than the stage.
- **A command key from `on_key` runs after the focused element.** `Ctrl+N` reaches the app's `on_key` only when nothing focused took it, which is what Fleet's `useKeyboardShortcut` does with editable targets, so no extra guard was needed.
- **Moving the route is one function.** Rune has no router; the arriving and leaving rules of the draft hang on `State::go`, and any code that assigns `route` for another reason must go through it.

## Machines, piece C (the machine part of the list)

- `.opacity(0.6)` on a column dims everything inside it, so a down machine's rows are dimmed by wrapping only the rows (Fleet's `.session-item` rule leaves the project headings at full strength).
- `rt.find(key)` finds an element even below the scroll area's edge, but a press there lands on whatever is drawn at that point: a test that presses a row at the foot of a long list must use a scene short enough to fit (the Add a machine test uses the `Down` case).

## Component 4, piece G (the sweep)

- Nothing new missing. Two things about checking: a Fleet-side media query (`@media (max-width: 520px)`) is the *window's* width, not
  the panel's, so a rule like the permission card's "hide the scope text under 520 px" can't fire in an app whose window is at least
  720 px wide (`min_size`); it needs no code here.
- A scripted run records at the app's own pace on a software renderer: a 16 s script with several taps takes about 1.5 minutes of
  wall time (65 ms a frame). Plan 20 pictures as half an hour, one process each.

## A link per machine (component 17, piece D)

Nothing new about Rune. One rule for the app: a call about a session takes its `Link` from `State::link_for(session)`, never from `State::link`, which is home's. A test for it needs only fake Fleets that record the token each request carried.
## New session, piece E (the strip, chips and starters)

- **A hover colour for a child needs `hover_group()` on the parent.** The chip's label is always `--text`; only its glyph and chevron go from `--muted` to `--text` on hover, so those are `.group_color(..)` children of a `.hover_group()` row, as the menu rows do.
- **`rt.state` edited directly doesn't rebuild the view.** A test that changes the draft and then looks for an element must change it inside `rt.update(|s, _| ..)`; with a plain assignment the next frame draws the old tree.
- **Overflow scrolling without a scrollbar isn't needed here.** Fleet's starters row scrolls sideways on a phone; desktop only, so it is a centred row with a 640 px limit and no scrolling.
- **No `max-width: calc(100vw - 32px)`.** `.ns-pop` caps at the window less 32; the popover here is a fixed width and `anchored_fit` keeps it in the window.

## New session, piece F (the folder menu)

- **A text box inside a popover takes the arrow keys through `on_input_key`.** Return true for Down, Up and Enter and false for everything else, so Tab and Esc still reach the popover's own handling (`.modal(true, close)` closes on Esc).
- **No native `<select>`.** Fleet's Location select is the operating system's menu; here the box opens a short list right under it, inside the popover.
- **A chip row doesn't wrap a shrinking item.** With `.shrink()` and `.min_w(0.0)` a long folder name is cut with an ellipsis next to the machine chip; Fleet moves it to its own line first (its `flex-wrap`). Taking the shrink away makes the chip overflow instead, because Rune has no `max-width: 100%`. Left as the ellipsis.
- **A mouse hover sets the highlight with `.on_hover`**, which is Fleet's `mousemove`; the row's own hover wash and the keyboard highlight are the same colour, so a row never shows two.

## Component 8, piece A (header data)

Nothing new about Rune. One thing about the harness: `row().shrink()` with a `truncate()` child and a plain child after it
gives the "head gives way first" path of Fleet's header (the leaf is drawn whole, the head takes the ellipsis); no
workaround was needed.

## Opening another machine's session (component 17, piece E)

Nothing new about Rune. The open session was found by index into `State::projects`; a session kept from another machine is not in that list, so `open_at()` returns `(store::ELSEWHERE, n)` for it and every reader of the open row goes through `session()`, `session_mut()` or `session_at()`. A scripted run that sends a message sends it again on every run: for pictures at two marks, use a new session per run, or the asks pile up.

## New session, piece G (the folder menu's path view)

- `row().wrap()` wraps the words of its text children, not the children: a line that is some text and a link needs
  `flex_wrap()` to put the link on a second line.
- `text_input(..).h(..)` is ignored: a box of a set height is a row of that height with the input inside it,
  `items_center()`.
- `TextEdit::new(..)` over a focused input drops its focus, so Enter and Tab no longer reach it; `replace_text(..)` keeps it.
  A test that sets the text uses `replace_text`.
- A row that arrives under the pointer takes `on_hover` at once, so a headless test that clicked a row leaves the pointer
  there and the next list's highlight moves to the row under it: move the pointer away (`pointer_move(0, 0)`) first.
- `focus_within_ring(1.0, -1.0, color)` on a bordered box paints an accent border while its input has focus (Fleet's
  `:focus` border on the branch box).

## Component 8, piece B (the header drawn)

- **A border adds to the box only with `.border_layout()`.** A pill with `.border(1.0, ..)` and `.py(2.0)` came out 19.6 high; Fleet's
  (CSS `border` plus padding) is 21.6. Add `.border_layout()` wherever the CSS box has a border and a height matters.
- **`.truncate()` also lets a text shrink.** Fleet's project, harness and model are `flex-shrink: 0`; with `.truncate()` they were
  cut ("Orch…") in a narrow header. Give them `.max_w()` only, and leave the shrinking to the path's head.
- **Underlined text is a `rich(..)` with one `Span`.** `El` has no `.underline()`.
- **`rt.presented(key)` gives a rect.** A "the pill keeps half" test reads widths from it; no screenshot needed.

## Another machine's rows (component 17, piece F)

Nothing new about Rune. `Open` is `Copy`, so a menu on another machine's row can't carry the machine's key and the session's id in it: it names the kind (`MachineContext`) and `State::machine_menu` holds the pair.

## A menu row with a differently styled name (component 17, piece G)

A context-menu row's label is one text run, so a machine's name in the mono face needs its own run: `ui::menu::Item::with_name` draws the label as two texts in a row, the second one truncating (`min_w(0.0)`), which gives Fleet's one-line, cut-short label. A key on the `context` panel's items is `<key>-item<n>` where `n` counts separators too.

## New session, piece H (the folder menu's clone view)

- `text_input` has no read-only mode (only an `aria-readonly` flag on any element). Fleet's repository box is read-only
  while a clone runs; here it is drawn as plain text in the same frame for that time.
- A native `<select>` can't be drawn from inside a popover, so the Location is a box that opens its own list under itself
  (as in the list view); a headless test needs a `frame` between opening it and clicking an item, or the item isn't there yet.
- A reply that arrives in pieces needs more than `Post::update` once: `feed_stream` hands the worker the `Post` so each of
  git's progress lines is posted as it comes, in order.

## Component 8, piece C (rename, tags, copy path, Restore now)

- **`Tx::copy` gives no answer.** A refused clipboard can't be told from a written one, so "Copied" shows after every copy. The state
  keeps the answer's place (`directory_copied(ok)`) so a test, and Rune later, can say no.
- **A field's blur is only its model's `focused`.** Like the list row's rename, the header polls `TextEdit::focused` once the field has
  held it and commits when it is lost (`keep_header`).
- **`clip_rounded(0.0)` on a shrinking row** keeps what overflows (the path's leaf, the copy button) from painting over the next item;
  CSS's `min-width: 24px` would let it spill.
- **`group_opacity(0, 1)` needs a `hover_group()` ancestor** (here the directory row); a button inside it that is itself a `hover_group()`
  is fine, as the message's copy button shows.

## A tooltip with its own delay and width (component 17, piece H)

`HelpLook` is set once per frame for the whole window (`cx.help_look`), so one element can't ask for another delay or width: Fleet's machine tooltip is 150 ms and 224 px, every other tooltip is 700 ms. We keep the window's look and break the words by hand (`machine_header::wrapped`); `.help(headline).help_row(try)` gives the two lines but not a medium-weight headline. A per-element delay and max width would remove the workaround.

## Component 7, piece I (workspace and base pickers)

- **`focus_group` roves with Up and Down only.** Fleet's workspace menu (a Reka dropdown) also takes Home and End and type-ahead on the item
  text; here Up, Down and type-select work, Home and End do nothing.
- **A label with muted words inside it** (`from ` before the base, ` · no fetch` after) is one `rich(..)` text with spans coloured
  `--muted` at weight 400, not a hint child: a child adds the row's 5px gap, which Fleet's inline `<span>` doesn't have.
- **A text whose tail is a muted tag** (`origin/main · default`) is also one `rich(..)` with a span at 11.5px.
- **A mono path or branch can't break inside a plan-line run**: Fleet's inline text wraps in the middle of `fleet/fix-login-` / `redirect`;
  `flex_wrap` breaks only between runs, so a long run wraps whole.
## Component 8, piece D (toolbar, ⋯ menu, session commands)

- **A disabled element has no tooltip of its own to give a wrapper**, so the Fork button keeps `help()` on the off button itself; Rune
  shows it, Fleet needed a wrapper only because a browser doesn't.
- **A headless test reads the clipboard with `rune::clipboard::memory()`** given to the runtime with `use_clipboard`; `Tx::copy` writes
  to it synchronously, so a menu item's copy can be checked without the OS.
- **`serde_json` sorts object keys** (no `preserve_order`), so a file whose key order matters (the raw messages export) is assembled
  by hand at the top level.

## Hover buttons beside a row, one toast for three sources (component 6, piece I)

- **The hover group goes on the shell, not the row.** Fleet's `.session-item-shell:hover` covers the row and the buttons beside it; a `hover_group()` on the row alone lets the word and ring show under a button the pointer is on. The shell is the group, and the row keeps only its own `.hover()` fill.
- **`group_opacity(1.0, 0.0)` is `visibility: hidden` on hover** for the age, ring and chips the buttons replace; the hidden text still takes the layout it had, as CSS's does.
- **A headless click can miss a row below the fold.** A machine's rows came after home's whole tree, off screen at 760 px; the test folds home's heading first.
- **No transition on the toast.** Fleet fades it 150 ms; here it appears and goes at once.
## Component 7, piece J (harness, profile, agent and model chips)

- **Nothing new missing.** The five menus reuse `ui::chip::pop`, `option` and `focus_group`; the harness and profile rows have no mark column
  (grid `1fr 14px`), which `Option_::no_mark` now says. One selector menu serves both the conversation's composer and the New session page
  because the menu is keyed by name and reads its rows through `State::selector_lists`.

## Component 7, piece K (machine picker, More options)

- **A list inside a popover needs its own close.** `ui::select::content` closed `ui.open`, which would shut the whole "…" menu; `content_in` takes the
  close action and the radius, so the Project list closes alone (its modal layer takes Esc first). Scenes can't focus a row, so the list's selected
  row is not drawn highlighted in a scene as it is when opened by a press.
- **`Chip` gained `trailing`** (the ellipsis in place of the chevron, no label) and **`Option_` gained `mono_title`** (the machine names).
- **Pictures of the real window on a virtual display** need `XAUTHORITY` passed through if the app is started under `env -i` inside `xvfb-run`, and `LD_LIBRARY_PATH` for
  `libxkbcommon-x11` on a machine with no root. The window opens larger than a 1280x760 screen: use 1500x1000 and crop.

## Component 6, piece J (Pinned group, selection)

- **A click can read the modifiers: `tx.mods()` in `on_press_tx`.** Shift-click and Ctrl/Cmd-click need nothing more. A test sends them with `rt.physical_input(PhysicalInput::Modifiers(..))` before the click; `--script` has no modifier keys, so a scripted run can't Ctrl-click.
- **A press's handler runs inside the frame, after that frame's `poll`.** State a press changes (a fold, a filter) is seen by `poll` one frame late, and a frame that draws nothing new schedules no next one: anything `poll` must keep in step with the list needs `tx.wake_after` while it matters (`sync_selection`, 100 ms while rows are picked).
- **An icon's stroke width can't be set**: Fleet's check in the checkbox is 10 px at stroke 3; here it is 10 px at the icon's own weight.

## Component 6, piece K (Archived view)

- **`cargo fmt` sorts the `mod` lines in `lib.rs`, and an attribute stays with the line it was above.** Adding a `mod tests_x;` next to a `#[cfg(test)]` one and running fmt left a test module without its `cfg`; read the diff of `lib.rs` after fmt.
- **A live list needs the hub to be connected before the list is asked for**; a test with a fake server that has no hub can ask by `retry_list()`.

## Component 8, piece E (fork dialog, analytics popover)

- A scoped style in Fleet is not what the browser draws: `SessionAnalyticsPopover.vue` says `width: 240px; padding: 0`, and
  the popover measures 252 wide with 14 px padding and square corners. Measure Fleet's DOM (`getBoundingClientRect`,
  `getComputedStyle`) before copying a number from a `.vue` file.
- `ui::dialog::Dialog.description` is one line that does not wrap; pass `description_el` with `.wrap().w_full()` for a long text.
- A `TextEdit` handed to a dialog's field needs `set_selection(0, len)` for the browser's "autofocused text is selected"; focus
  itself is `tx.focus(key, false)` on open.

## Component 7, piece L (the message box and sending)

- **A text input is edited through a `&mut TextEdit` the element is given, so a copy of its text elsewhere (the draft's `message`) has to be reconciled by `poll`.**
  `sync_new_message` compares both with what was last seen: a draft that moved was changed from outside (a starter, a test) and the field takes it, else the field's text is copied to the
  draft. The view stays a plain function of `State`.
- **`min_h(64.0)` on the text input, not on `auto_grow`, gives Fleet's "room for two lines"** (`auto_grow(field, 2, 7)` alone sizes to the text).
- **A headless click on a text input focuses it, and `Key::Text` then types**; Enter reaches `on_submit` and Shift+Enter inserts a newline with no extra code.
- **A script's clock is virtual but a thread's answer is real time.** A live run that clicks a row the instant a menu opens can find nothing there; give a live script seconds between a call and its result.

## Component 8, piece F (smart links, the small GitHub parts)

- No turning animation: `Cycle` only moves opacity, so a spinning icon (`CheckIcon` pending) fades in and out like the other spinners here. **Out of date (22 Nov 2026):** `.looping(LoopSpec::rotation(period))` turns an element forever and holds `still` under reduced motion (`crates/rune/README.md`, "Periodic motion"); the spinners can turn.
- A ring like `box-shadow: 0 0 0 2px bg, 0 0 0 3.5px ring` is two `box_shadow` calls with `spread`, the wider one first; it takes no layout room, as in CSS.
- A bar of segments sized by `flex-grow` is `el().grow(n)` children in a row with `clip_rounded` on the row.
- A fake server for a test of a request: the hub's own negotiate `POST` reaches it too, so filter calls by path before counting them.

## Component 8, piece G (context chips and the pill)

- `overflow: hidden` with a radius (the pill's number part under the rounded border) is `clip_rounded(radius)` on the container; without it a child's square fill pokes past the corners.
- Two `impl State` blocks in different files can't use the same method name (`show_context` already belonged to the ring's scenes): name scene helpers after their file.
- **A `Lists` with no session is the New session page's `@` list.** `composer::triggers::Autocomplete` is reusable as it is: `Lists::for_draft(None)` plus `draft_key` gives sessions and files only. Keep a second engine and popup in the page's own state: writing the page's popup into the session composer's `auto.popup` flips with `keep_autocomplete`, which clears it every poll when no session is open.
- **A scripted key with Cmd needs `"cmd": true`, not `"mods"` or `"hold"`:** `{"type":"key","name":"l","cmd":true,"shift":true}` is what `Key::Command` shortcuts see. A script's `{"paste": {"png": "file"}}` pastes a picture.

## Component 6, piece L (the row's right-click menu)

- **A menu has no "highlighted row" to read.** `focus_highlight` moves focus with the pointer, but the view can't ask which row has it. The footer takes its text from `on_hover_change` on each row (a pointer-driven hint); keyboard focus doesn't update it.
- **A submenu is a second `anchored` panel** (`Side::Right`, `Align::Start`) on the trigger's key, with its own `modal`, shown while a `State` field names the trigger. Escape closes the submenu first.
- **Text wraps only with `.wrap()` (`.wrap_anywhere()` for `overflow-wrap:anywhere`)**; a text in a `grow` row is cut off without it. A label that must shorten needs `.min_w(0.0).truncate()`.
- **`clip_corners` on a footer that runs over its panel's padding** (negative margins) rounds its bottom corners without clipping the panel's shadow, which `clip_rounded` on the panel would.
- **`--script` has `secondary-click` and the app takes `--theme light|dark`**: a live picture of a right-click menu in both themes needs no scene.

## Component 7, piece N (the harness setup wizard)

- **A modal that must not be dismissed** is `.modal(false, |_| {})`: Escape and a click outside call the empty handler, focus stays trapped.
- **A headless app seeds invented harness lists and preferences**; a test of the first-launch rule clears `feeds.harnesses` and `pickers.prefs` before connecting, or the rule sees the invented ones as already read.
- **A disabled button drawn with `opacity(0.5)` animates in**: a test that reads `ElementInfo.opacity` runs about 30 frames after the change first.
- **`text(..)` is centred with `.align(Align::Center)`** (there is no `text_center`), and needs `.wrap()` and a width to centre across lines.

## Component 8, piece H (the pull request popover)

- A row that wraps (`flex_wrap`) with a `grow(1)` row holding right-aligned text is `margin-left: auto` that drops onto its own line when the headline leaves no room; a spacer `el().grow(1.0)` doesn't, it stays on the first line and the text overflows.
- A disabled element still takes a test's `click`, so a "sent" or "sending" guard belongs in the state's method, not only in `.disabled(true)`.
- `rt.find(key)` gives only a position; read an element's label with `rt.elements()` and its `label`.

## Component 6, piece M (lineage and sub-agent rows)

- **A line that fills its parent's height is `el().absolute(x, 0.0).w(1.0).h_full()`; there is no percent height.** Fleet's `bottom: 50%` (the last child's line stops at the row's middle) and `top: 50%` (the stub) are written as half of the child's known height, 15 of 30. That holds only while a child row is one line tall, which it is.
- **A state-style test can read `ElementInfo.text_color` and `.fill` from `rt.elements()`** (no GPU needed), and `parent` to reach an icon's colour inside a button. Hover the pointer, focus, pick and open a row, and compare what lineage draws with its colour at rest: that is the muted rule's test (`tests_lineage.rs`).
- **Two quick clicks on one row in a test are a double press**, which starts the rename. Click a different row between them.
- **`children(Option<El>)`** adds an element only when there is one; there is no `child_if`.

## Component 7, piece O (the setup rows)

- **No hover border.** `hover(color)` changes a fill only; CSS's `:hover { border-color }` (the row buttons, the install-place cards) has no equivalent, so the plain button gets a faint accent wash on hover instead.
- **A wrapping row is sized by its text's longest line**, not by `flex: 1 1 220px`: `flex_wrap()` plus a grown text block wraps the actions below at any width where the subtitle's full line doesn't fit. Fleet's head wraps only when tile + 220 + the actions exceed the row, so `view::harness_setup_rows` makes that test from the labels' lengths and builds the one or two-line layout itself.
- **A panel that must scroll when taller than the window** is `max_h(..)` on a column holding the body with `.scroll(ScrollSpec { stick_bottom: false })` and `min_h(0.0)`; the panel's own padding comes off the maximum by hand.
- **A test can't see a radio's checked flag** (`ElementInfo` has none): read the state that drives it.
- **Text won't break inside a long word** (a shell command): give the text's box a `max_w` and `.wrap().w_full()`; it breaks at spaces and at hyphens.
- **A scripted scroll** is `{"type":"wheel","key":"<element>","dy":-330}` (negative scrolls down).

## Component 6, piece N (the Open in submenu)

- **A panel sized to its content leaves the widest row touching the right edge**, and a `truncate()` label in it is cut to "Code..." because the panel measures rows at their minimum. `ui::menu::fit_width` estimates the width from the longest label (letters, icon, chevron, paddings) and the rows of such a panel don't truncate. A real measure-then-size would replace it.
- **Submenus nest by key, one field per level** (`Ui::sub`, `Ui::sub2`): a row's press sets its level's field, and a press on level 0 clears level 1. Escape closes the innermost `modal` first, so each level's panel carries its own close handler.
- **No looping rotation**: the spinner is the same slow fade as `ui::confirm::spinner`, now shared with the menu's "Detecting tools..." and "Loading..." rows. **Out of date (22 Nov 2026):** `.looping(LoopSpec::rotation(period))` turns an element forever and holds `still` under reduced motion (`crates/rune/README.md`, "Periodic motion"); the spinners can turn.

## Component 8, piece I (the Context tab)

- **A `grow(1)` spacer inside a scroll column does pin a footer to the viewport's bottom** when the content is shorter than the viewport (the Context tab's footer, as Fleet's `margin-top: auto`); a longer one scrolls with it.
- **Negative side margins (`.ml(-6.0).mr(-6.0)`) on a row in a stretched column** widen it past the column as CSS does, so a row can have its own hover padding and still line its text up with the heading.
- **`flex_wrap()` on a row** is the only wrap switch; `justify-content: space-between` has no counterpart, so a wrapped notice's button sits at the left of its own line (Fleet's does too once it wraps).
- **A flash is `box_shadow` with `spread` and no blur, its alpha computed from `State.clock`**, redrawn by `tx.wake_after(33 ms)` while it lasts; `scroll_into_view_in("<scroll key>", "<row key>")` scrolls the target to view.

## Component 8, piece J (the smart-link card)

- **CSS's adjacent-sibling rules (`.signal + .signal`) are ordinary data here**: build the parts as `(kind, El)` pairs and add the top border to the ones that follow a signal or a list; a bar between two breaks the rule, as in Fleet.
- **A chevron that turns 90 degrees** is two icons (`CHEVRON_RIGHT`, `CHEVRON_DOWN`): Rune has no transform.
- **`hover_group()` plus `group_color(..)`** gives a button's glyph and words their hover colour together (the Send buttons' accent).

## `auto_grow` takes the key `scroll`

`auto_grow(field, min, max)` returns a scroll wrapper keyed `"scroll"`. A `.key(..)` set on what it returns replaces that
key, so clicks and `tx.focus` on the key never reach the field. Key the field and attach `on_edit` first, then wrap it
(`ui::controls::textarea_field` and `grow`).

## Component 6, piece P (the draft row)

- Nothing new about Rune. `TextEdit::replace_text` (and `clear`) drops the undo history; `State::set_draft_text` selects all and `insert`s instead, so a text written into the open composer from outside can still be undone.

## Component 7, piece P (the Welcome and Repositories pages)

- **No `padding(x, y)`**: `padding` takes one number; `px` and `py` set the axes.
- **No grid**: Fleet's `grid-cols-1 xl:grid-cols-2` is a column of rows, each with `columns` cards of `grow(1.0).w(0.0)` (so they share the width evenly) and empty stand-ins to fill the last row.
- **A dashed border** is `border(..).border_layout().border_dash(dash, gap)`; the dash needs the border laid out or it draws over the content.
- **`on_press` has no `Tx`**: use `on_press_tx` for a handler that toasts or wakes.

## Component 6, piece Q (the right panel's shell)

- **Layout eases on a press's commit, not on a drag's.** `tx.motion_curve(Curve::tween(0.2, Easing::cubic(..)))` in the handler gives a toggle Fleet's own 200 ms curve; `tx.instant()` in an `on_drag` lands each step at once, which is Fleet's "never while dragging".
- **A negative margin works** (`mx(-4.0)` puts an 8 px gutter over the seam without taking width from either side).
- **Centre text over a canvas** with a keyed-less `el().size(..).absolute(0,0).center()` box; `absolute` plus `.center()` on the text itself leaves it in the corner.

## Component 6, piece S (dragging a row)

The small case the checklist asked for (`tests_row_drag.rs`, first test): **Rune can carry a row drag with drop targets.** No HTML5 drag-and-drop is needed.

- **Which element is under a dragged pointer**: in `on_drag`, `d.window_point()` gives the pointer in window points and `tx.element_at(x, y, |s, picked, tx| ..)` hands back the deepest element there. `Picked::path()` names every keyed ancestor (`.../drop-project-1/#2/project-1`; unkeyed ones are `#n`), so a target is a key on a container and the handler walks the path from the end. It works inside a scroll box. `Picked::key()` is only the deepest element's own key, which is no use for "which group".
- **The answer arrives after the handler**, so the target shows one move late; the release drops on the last one found. A press that moves under Rune's 4 pt threshold stays a click, so the same row can have `on_press` and `on_drag`.
- **`Drag::cancelled()` on `Phase::Up`** is Escape; put the drag down without dropping.
- **No drag ghost.** The dragged row is dimmed to 40 % (Fleet's `.dragging`) and the targets light up; a floating copy under the pointer isn't drawn.
- **The list changes under the pointer when a drag begins** (the empty Pinned group and the Archive box appear), as in Fleet. A test has to move off the row, run two frames and find the target again before moving to it.

## Component 8, piece K (the progress strip)

- **A bar whose width eases has no tool**: Fleet's `transition: width 300ms` on the strip's 1 px bar is drawn here at once (`w_frac`). `live_trim` eases a canvas stroke, not a box width; a stroke along the edge would do it, but a 1 px accent line with a stroke cap isn't the same pixels. Open, noted in the checklist.
- **A strip pinned under a tab's body**: wrap the body in `col().grow(1.0).min_h(0.0)` so it takes the room, then the strip is the next child. A body with no growing parent leaves the strip floating under the last row.
- **`focus_ring(2.0, -2.0, colour)`** is `outline: 2px; outline-offset: -2px` (inside the edge).

## Component 6, piece T (keyboard focus on a hidden row button)

- **`group_opacity(0.0, 1.0)` hides a control from the keyboard.** A row's Pin and Archive buttons show only while the row is hovered;
  `tx.focus("session-archive-0-1", true)` leaves nothing focused (with or without the pointer over the row), and Tab skips them. Fleet shows them on
  `:focus-visible` and hides the kind label then (#1048). Tested in `tests_lineage` while writing the piece. Wanted: a way for a control that
  is faded out by its group to still take focus and show itself when it has it.
- **A menu row held lit while its submenu is open** needs only `.fill(highlight)` on the trigger; the open state lives in `State::ui.sub`, so `ui::menu` takes it as an argument.

## Component 7, piece Q (the sweep)

- Nothing new about Rune. A test that adds a machine by hand loses it at the first answer of home's own `GET /api/machines` poll (it clears the list), so the fake home must list the machine from its first answer, not be told afterwards.

## Component 17, piece J (Settings → Machines)

- **No password field.** `text_input` has no masked mode, so Fleet's `type="password"` New token field is drawn as plain text; the token is visible while it is pasted. The token shown on the page is a plain `text`, swapped for dots by the view. **Out of date:** `TextEdit::secure()` makes one (see the note under component 17, piece J, below).
- **No max length on a field**: Fleet's `maxlength="64"` is applied when the value is saved (`chars().take(64)`).
- **`border_sides` takes `[top, right, bottom, left]`.** A divider above a row is `[1.0, 0.0, 0.0, 0.0]`; `[0.0, 1.0, 0.0, 0.0]` draws a stripe down the right edge.
- **`clip_rounded(r)`** is `overflow: hidden` on a rounded box (the setting group's card, the segmented control).
- **A test types into a field after clicking its button**: replacing the field's `TextEdit` from `update` drops its focus, so a following Enter goes nowhere; press `Key::Text` instead (the field's text is selected, so it replaces it).
- **`--snap` with `--size` draws at scale 2 unless `--scale 1` is given**: a 1280x760 picture came out 2560x1520 until then; pictures beside a browser's need `--scale 1`.

## Component 9 (planning)

- **`--theme light|dark` works with `--server`**, so a live picture can be taken in either theme without touching the system's.
- **`--at` counts frames, not seconds on the wall.** With `--server`, a software renderer drew 6 s of frames (360 at 60 fps) in about two
  minutes of real time, so a live state that lasts 30 s (a `background-work` sub-agent) had ended by the mark. For a picture of a passing live
  state, take it from a scene, or use a real window grabbed at the right moment (`checks/window.sh` style), not `--snap --at` on lavapipe.

## Component 17, piece M (the address list)

- **A row can be a drop target by its key's place.** The drag finds the row under the pointer through `Picked::path()` (as in 06 S); a URL can't be in the key because the path is split on `/`, so rows are keyed by index (`machine-address-<card>-<n>`).
- **An element that left the tree can still be found for a frame or two** (`rt.find` / `elements()` after the view stopped drawing it); a test asserts on the state that drew it, not on the key being gone.
- **A keyed handle keeps focus when its row moves**: `tx.focus(key, false)` in the handler that moved it puts focus on the new place.

## Component 9, piece B (the Help menu and the status bar's hints)

- **`preserve_focus_on_press(true)` only helps an element that can't take focus itself.** On a `focusable(true)` button the press focuses the button and the
  composer loses the keyboard; with `focusable(false)` + `preserve_focus_on_press(true)` the composer keeps it. Fleet's `mousedown.prevent` hints stay
  reachable by Tab; here they can't be (no way found to be in the Tab order and keep a press from taking focus).
- **No pressed fill, and no hover border.** `hover(color)` is the only state tint, so Fleet's pressed 10% tint and the `[`/`]` key's lit border are not drawn.
- **`ElementInfo` is enough to test a button**: `help` (tooltip), `label`, `opacity` and `focused` read a hint's state without a picture; `focusable` is false
  on any element that only has `on_press`, so it says nothing about enabled or disabled.
- **A menu measured mid-animation is narrower** (190 of 196 px a frame after opening); run frames before asserting its width.

## Component 17, piece K (Add a machine)

- **A form that reacts to what was typed has no "on change".** A `TextEdit` is edited in place by `on_edit`, so nothing is called with the new text; the poll compares what the fields mean (the address and code they parse to) with what was last looked at and starts the request when they differ. A scene that sets the result by hand must mark that look done (`AddForm::mark_looked`) or the first poll throws the result away.
- **A scripted run's clock is virtual, a thread's answer is not.** A live script has to leave seconds between typing a link and the mark that expects the answer, and a first-run dialog that opens when a read returns can cover the page: set its preference on the scratch Fleet first.
## Component 9, piece C (the landing rule and the dashboard's data)

Nothing in Rune got in the way: the piece is plain Rust (`fleet/dashboard.rs`, `fleet/home.rs`). One lesson about our own
tests: 128 tests assumed a list load opens the first session. A normal run starts with no route, so "start on Home" is
decided on whether the route being replaced was an invented session (a scene, a headless run), which keeps those and
every scene as they were.

## Component 9, piece A (the rail)

- `.help()` takes one global look (`Cx::help_look`) and a delay, so a tooltip that differs from the app's (Fleet's rail
  one is the popover colours, no delay, 16 px clear of a 40 px button) can't use it. Track the hovered element with
  `on_hover_change` in `State`, and draw the bubble as an overlay with `anchored(.., Side::Right, Align::Center, gap)`
  against the button's key. A test reads the overlay with `rt.find("rail-tip")` after `pointer_move`.
- Overlay entrance: `anchored` fades and scales it in; there is no per-element fade or slide, so Fleet's 4 px slide is left out.

## Component 9, piece D (the home page)

- **No dashed border.** Fleet's bordered empty state is dashed; Rune's border is solid only, so the empty box is solid. **Out of date:** `.border(..).border_layout().border_dash(dash, gap)` draws one (see the "dashed border" notes below).
- **A focusable element needs its own Enter and Space.** `on_press` alone opened a row on a click and on Space but not on Enter in a headless run, so the row has an `on_key` for both.
- **Space arrives as `Key::Text(" ")`**, not a named key.
## Component 17, piece O: a drag target that belongs to one group among many

Rune's pointer drag finds the target with `Tx::element_at` and the keys on the path, so a second tree (another machine's) needs its own keys.
Put the machine and the project in the key (`drop-mproject-<machine>~<project>`, `mpinned-<machine>`), and read the row a drag is over from
the same path (`machine-session-<machine>-<id>`). Check the dragged row's machine against the target's machine when the drag moves: nothing
in Rune stops a drop in the wrong tree. Nothing in Rune got in the way otherwise.

## Component 9, piece E (row actions, the filter, another machine)

- **A hover-only strip beside a row**: put the row and the strip in a shell with `hover_group()`, pin the strip over the row's right
  edge and give it `group_opacity(0.0, 1.0)`; the buttons stay in the tree at opacity 0, so a headless test clicks them by key.
  Fleet's strip fades over a gradient; give the strip an opaque fill of the row's hover colour instead
  (`Theme::mix(text, panel_bg, 0.05)`: the first colour is the one with the share).
- **A press needs the element on screen**: a test window of 1280x760 cuts off a button below the fold, and a click there lands on nothing.
  Run the button's own action from `rt.update` and check the key exists, or draw the scene taller.
- **Hover in a script**: `scroll` with `"dy": 0` on a key, or `move` with a point, leaves the pointer over an element for a picture;
  `secondary-click` on a key opens a right-click menu.

## Component 9, piece H (the running-work counter)

- A chip that opens an anchored list is the pull request pill's recipe: key the chip, `anchored(card, KEY, Side::Top, Align::End, 6.0)`
  over a `scrim`, `focus_return(KEY)`, `modal(true, close)`. The list is the `Open::Popover("running")` state, so a work event that
  empties the list has to shut it from the state side (`shut_running_popover_if_idle`): an overlay can't close itself in a view.
- A fill that animates (hover to 24%) reads as an in-between colour in a test that clicks and looks at once: run ~30 frames first.
- A software renderer outruns the scripted model's 30 s `background-work` job: a start of the app costs seconds and the job is gone
  before the picture. Scenes fed with recorded items (`mock::running`, fixed clock) hold the state; Fleet's own picture is taken live
  in the same run that starts the jobs.
- No focus-ring API (Fleet's chip has `outline: 2px solid var(--running)` on `:focus-visible`): the chip shows none.
## Component 9, piece I (usage rules and state)

- Nothing in Rune got in the way: it is plain Rust. For the next one who tests a timer: the poll gets `now` as an argument, so a
  test calls the `keep_*` method itself with 0, 10, 69 and 70 s instead of running a minute of frames.
- `Link::start` also asks the server for `/api/machines`, the hub and plugins, so a fake server used as a link sees those too and
  answers `/api/machines` with an empty list unless told; its poll then replaces any machines a test put in by hand. Give the fake the
  machine list (and the other machine its sessions, projects and identity) rather than setting `machines.list`.
- **A poll that must stop when its page is covered keys off the route and the rail, not the route alone** (component 9, piece F): a rail pressed from Home leaves the route at Home and only swaps what is drawn, so the summary bar's reads stop on `route == Home && rail == Sessions`. `wake_after(1 s)` from `poll` is enough to run 30 s and 5 min timers; no thread sleeps.

## Component 9, piece J (usage chip and popover)

- A bar with a mark over it: `clip_rounded` on the track also clips a child that sticks out, so the pace tick (2 px, a 1 px ring as a
  `box_shadow` spread) sits in a plain frame beside the clipped bar, not inside it.
- A popover that must watch while open: don't call watch/unwatch from the handlers (a press outside, Esc and another menu all clear
  `ui.open` without them). Compare `ui.open` with a flag in the poll and count the change there.
- A grid with `minmax(70px, 1fr)` is a `grow(1).min_w(70)` cell next to fixed-width ones; let its text `wrap()` as the browser does.
- A button inside a stretched column fills the width: wrap it in a `row()` to keep its own width.

## Component 9, piece R (Report a problem: Review and Sent)

- `rich` text takes a colour and a background per span but no strikethrough: strike the same byte ranges with `El::strikes`.
- A dialog that changes height between steps moves while it settles (`token_motion`): after a press that makes it shorter, run
  about 30 frames before pressing something inside it, or the press lands on the scrim and closes it.
- A scrolling child with `grow(1.0)` in a column of unknown height collapses and its text draws over the footer; give it `min_h` and
  `max_h` and no `grow` (the same rule as piece Q's).

## Component 9, piece Q (Report a problem: Describe and the screenshot)

- **A running app can't take its own picture.** Rune renders a window's frame on the GPU and exposes no call that returns it to the app
  (`--snap` and `--script` do it from outside; `on_frame_drawn` gives the element tree, not pixels; `Offscreen::read` is for Rune's own
  renderer). Smallest case: in a handler, get the RGBA of the window as last drawn. So Report a problem ships "Couldn't take a screenshot"
  (`fleet::report_form::grab_frame` returns `None`) and the rest of the path works from a picture handed in.
- A flex column with only a `max_h` has no definite height, so a child with `grow(1.0)` gets 0: put the `max_h` on the scrolling child itself
  (the window's limit less the header and footer).
- A scrim's middle is under the dialog: a test that closes by the scrim must press its corner, not `rt.find("…-overlay")`.

## Component 9, piece P (Report a problem: data and requests)

- Nothing in Rune got in the way: it is plain Rust. A request that returns a file (`/api/reports/file`) needs the response's
  `content-disposition`, which the other helpers in `http.rs` drop: `report_call` returns status, header and bytes, with
  `http_status_as_error(false)` so a refusal's own words can be read. The file's name from a server is cut to its last path part
  before it is joined to the downloads folder.
- An answer that arrives after the report was closed must not bring it back: each request takes a generation from the state and the
  answer is dropped unless it still matches (`ReportState::finished`).
## Component 9, piece K (notices and the quiet moment)

- A key typed into a focused `text_input` is not a window `on_key` event unless the control passes it on, so "the user is typing" is read from `tx.focused()` inside `on_key` and from the draft text changing, not from every keydown.
- `on_focus` and `on_visibility` hand a `Tx`: a handler that must act at once (open a waiting card on return) can call the same keeper the poll calls, instead of waiting for the next `wake_after`.
- A scene's clock is the real one: `--snap --at 3.6` runs 3.6 s of frames, so a timed behaviour can be shown by two snaps either side of its time.

## Component 16, piece A (the Settings shell)

- Nothing in Rune got in the way. The menu reuses the session list's panel width, gutter and `sidebar_open`, so the one gutter and Ctrl/Cmd+B
  work as they do there. `cargo test | head` ends the run early (the pipe closes and cargo stops); send test output to a file.

## Component 16, piece B (the setting primitives and anchors)

- **No inline link in a wrapped line.** Fleet's More sits at the end of the summary's last line. A `rich` text can underline a run
  but not take a press, so More is its own element after the summary in a wrapping row: it follows the summary when that fits on one
  line and drops under it when it wraps.
- **`on_key` on a `focus_group` row sees the arrows first and can return `false`.** Segments pick the neighbour in `on_key` and let
  Rune's roving focus move; both wrap, so the picked and the focused segment stay the same one.
- **`ElementInfo` has no checked or switch state**, only `expanded`, `focused`, `opacity` and `role`. Tests read a switch's or a
  segment's state from `State`, and `expanded` from the More button.
- **Press goes on the element that has the key.** A wrapper `row` around a switch's track takes no press from a click on the track:
  `setting_switch` takes the handler and puts it on the track.
- **`Tx::scroll_into_view` reveals "just far enough"**, not to the top as `scrollIntoView({ block: "start" })` does; a group already
  in view doesn't move.

## Component 9, piece L (the notice card)

- A `row()` with `on_press` and a role is not focusable: add `.focusable(true)` or `tx.focus(key)` does nothing and a test finds no
  focused element. `tx.focus` takes effect after the next frame, so frame once before reading `tx.focused()` or pressing a key.
- Hold timers are plain arithmetic on the app clock (`keep_hold`), woken with `tx.wake_after`; hover comes from `on_hover_change`,
  and focus-inside from `tx.focused()` in the poll, so no focus handler is needed.
- A rich text's link takes a `LinkPaint` for hover and focus; a `Span` carries the label's smaller size, so a list item's muted label
  and its text are one wrapped paragraph.
- Tests that run many seconds of a card's hold use a coarser frame step (1/20 s); 60 fps made the notice tests take minutes.
## Component 16, piece D (General)

- **No native `<select>`.** Large pastes is a bordered row with a chevron that opens `ui::select::content` anchored to it
  (`Open::Select("large-pastes")`), so the open list is Rune's panel, not the system's: it has the check mark and the select look of the
  rest of the app, not Chrome's.
- **A test that clicks a list's choice** finds the element by its text and presses its centre; the choices carry no keys of their own.
- **`cargo test` with a fake server** needs the harness's wait helper (`test_wait::drive`) to run queued jobs: the answer of a request
  reaches the UI thread only through `Link`'s `Post`, which the tests collect and run.

## Component 16, piece E (Appearance data)

- **`Color` is linear light**, so a hex colour can't be got back from a `Color` by rounding its fields. Where Fleet's own rules need hex (the contrast lift), keep the hex strings and convert once with `rune::color::hex`; `Theme::code_grounds` holds the surfaces as the same literals `Theme::of` uses, and a test checks they agree.
- **A view can't be handed a computed theme by `Theme::of` alone** when part of it depends on `State`: `view::root` builds the theme and `State::with_code_theme` adds the chosen colours, caching the lift in a `RefCell` on `State` because views get `&State`.

## Component 9, piece M (chips, settling, the lift, Remembered)

- **A change made by `Runtime::update` or by `poll` skips the exit transition**; only an input handler's change plays it. Fleet's card
  settles from a timer, so a `.transition()` on it never showed. What works: keep the node in the tree after it "goes" and change its
  `.offset()`, `.scale()` and `.opacity()` with `*_motion(Spring)` (the same key keeps its springs). `fleet::notices` holds the card
  as `leaving` for 0.37 s and the view draws it at the chip's corner.
- A removed node with `.transition(..)` only exits if the node carrying the transition is the one removed (the frame around it isn't
  retained alone); and a test must settle by a press (`click`), never by `rt.update`.
- **`--video` and `--snap --at` draw at rest**: springs don't advance between scripted frames, so a card folding into its chip shows as
  a cut. Tests on virtual frames (`rt.frame(dt)` then `rt.presented(key)`) are the check for motion.
- `Transition` can't scale below 0.9 without `allow_small_scale` (floor 0.5): Fleet's fold shrinks to a chip's width (about 0.47).
- `on_layout` reports window points, so the card, composer and chip rects can be compared directly; it reports again only on change,
  so a composer that leaves the tree keeps its last rect (guard it by the route).
- A hidden (`opacity(0.0)`) element still takes layout and reports `on_layout`: how the card learns where its chip will be.

## Component 16, piece F (Appearance page)

- **`Cx::zoom(k)` is a run-time page zoom**, set on every run of the view (`view::root`). It is how font size scales the whole window; `ui::rem` stays a `const fn`. `Runtime::find` and pointer input are both in the zoomed view's points, and a click after a zoom change needs the layout to settle first (a test runs about 30 frames).
- **`Color::alpha(a)` sets the alpha, it doesn't multiply it.** Fleet's `border-border/60` is `t.border.alpha(t.border.a * 0.6)`; `.alpha(0.6)` on a translucent border draws a white line.
- **`ElementInfo` has no checked or disabled flags**, so a test reads a picked card by its fill and a disabled one by its 50% opacity.
- **A page taller than the headless window** needs `rt.set_viewport((w, h), 1.0)` before a test clicks its lower rows; the anchor scroll isn't applied by frames alone.

## Component 9, piece N (the Fleet restarted notice)

Nothing new about Rune. One trap in scripted runs: the shown-notice ids live in the settings file under `FLEET_NATIVE_DATA_DIR`, so a second run
against the same restart opens no card (`tap` then fails with "no element"). Empty that folder between runs.

## Component 16, piece C (Settings follows the machine)

- **A switch whose choice lives in `State`** is a `Switch { choice: fn(&mut State) -> &mut MachineChoice }`: a non-capturing closure coerces to the `fn`, so one widget serves every view's own choice.
- **`Runtime::find` returns an element's centre**, so "above" in a test compares centres, not edges.
- **No timed fade** was drawn for the pane swap; headless pictures draw at rest, so a fade could only be tested on virtual frames.

## Component 9, piece S (sign-in)

- **There is a password field after all.** At the pinned rev `TextEdit::new(String::new()).secure()` makes `text_input` draw one bullet per character, keep no undo history, hide the text from `Debug` (`[redacted]`) and wipe its buffer when replaced (`replace_text`) or dropped. The note under component 17, piece J ("no masked mode") is out of date: the New token field on Settings → Machines can use it too.
- **A secure field's text is not in a test's element tree**: `rt.elements()` shows the bullets, so "the token is never drawn" can be asserted on what the tree holds.

## Component 16, piece P (Memory page)

- **A page that reads a machine on a thread** is kept by a `poll` hook (`keep_memory`) that compares the machine key it asked with the one Settings shows, so opening the page, switching machine and coming back all use one path. A scene sets `loaded_for` to the current key, or the hook reads over the state the scene made.
- **A test of "the request went to the other machine"** needs home's fake `GET /api/machines` to list that machine with a token; otherwise home's own poll empties the list and the pick falls back to home.
- **Two equal columns** are `w(0.0).grow(1.0).min_w(0.0)` on each cell; there is no `basis`.

## Component 9, piece T (Improve a skill: data, diff, button)

- A conversation row is reused through `memo` until its key changes, so anything a row reads from outside its message (here the list of built-in
  skills, which can arrive after the row was drawn) has to be part of that key (`conversation::improvable`), or the row stays as first drawn.
- A button inside a pressable row takes the press alone: the card under the row did not open or close.
- The skill diff can't wrap a long line as Fleet's does: wrapped text has no fixed height, and the diff keeps its number column level with
  the code by giving both columns rows of one height.

## Component 16, piece T (Labs)

- `ElementInfo` has no `disabled` flag. A test reads a disabled switch from its `opacity` (the kit fades it to 0.6) or by pressing it and seeing nothing happen.
## Component 16, piece Q (Memory's notes)

- **Long unbroken text** wraps with `.wrap_anywhere()` (`overflow-wrap: anywhere`); plain `.wrap()` breaks at spaces only and a long token runs out of its card.
- **A text area's Enter and Esc** are `.on_submit` (Enter; Shift+Enter still adds a line) and `.on_input_key` (Esc). A text area does not take focus when it appears: a test clicks it first, as Fleet's user does.
- **`maxlength`** has no Rune equivalent. `State::limit_memory_fields` cuts the text each frame from `poll`, so a paste past 400 characters is cut one frame late.
- **Hover colour on a child** (Fleet's `.memory-link` text goes from muted to `--text` on hover) can't be set from the parent; the link keeps its muted colour and the focus outline is drawn.
- **A dashed border** (`EmptyState bordered`) is `.border(..).border_layout().border_dash(3.0, 3.0)`.
- **A popup select anchored to a keyed trigger** needs one more arm in `ui::mod`'s `Open::Select(name)` match; the choice closes it with `s.ui.open = None`.

## Component 9, piece U (Improve a skill: the dialog)

- A text field takes its keys before the frame does: Ctrl/Cmd+Enter in the note has to be asked through `on_input_key` on the field, not
  `on_key`, which the field never reaches for Enter.
- `text_input` neither clips nor scrolls its text, and a fixed `h` lets a long text draw out of its box. For a hand edit of a long file
  the field takes `min_h` from the line count (wrapped lines counted at about 76 characters) and sits in a scrolling box with `max_h`.
- An answer from a thread can come back after the dialog closed or opened for another skill, so each request carries a ticket and an
  older ticket's answer changes nothing.

## Component 16, piece G (Harnesses)

- **No `aria-selected`**: a tab is marked with `.checked(on)`; a test reads which tab is shown from `State`, since `ElementInfo` has no checked field.
- **No hover underline or hover text colour on its own**: a tab's text takes `hover_group()` on the row and `group_color` on the text; a link that only underlines on hover stays plain.
- **A link inside a sentence** is a wrapping row of the text, the link element and the full stop: it breaks between them, not inside the text.

## Component 16, piece H (Harness profiles)

- **No `p`, `shrink0` or `child_if` on `El`**: padding is `px`/`py`, a child that shouldn't shrink simply isn't given `shrink()`, and a conditional child is a plain `if` before `.child(..)`.
- **A test that clears the invented data first**: the mock `feeds()` seeds `profiles` for the home machine, so a headless test that wants the page to *ask* must clear `feeds.profiles`, else it reads "loaded".

## Component 16, piece U (Updates: data and requests)

Plain Rust, no view, so nothing new about Rune. For the next piece that draws these rows: `describe_version_line` returns the line's text and tone with `rule_menu`, so the page picks Rune's `menu` or `segments` for the per-machine rule (no native `<select>`), and a rollout narrative is one `Text` run per sentence.

## Component 16, piece AA (Folders: Locations)

- **A popover over a keyed button** is an `Open::Popover(name)` arm in `ui::overlays`, anchored with `overlay::anchored(.., key, Side::Bottom, Align::End, 4.0)` and a `scrim`; with no room below it flips above and covers the button's row.
- **Text that shows only while its row is hovered** (Fleet's `opacity-0 group-hover:opacity-100` Select) is a transparent colour with `.group_color(muted)` on the text and `.hover_group()` on the row.
- **Enter in a single-line field** is `.on_submit`, as for a text area; a test clicks the field, then sends `Key::Text`, then `Key::Enter`.
- **`El` has no `.p(..)`**: all-round padding is `.padding(..)`.

## Component 16, piece R (Dreaming's panel)

- **Strikethrough** is `El::strikes([(start, end, None)])` over byte ranges of a `text` (or `rich`); with `.opacity(0.7)` it draws Fleet's `line-through opacity-70` replaced note, wrapped lines included.
- **A busy poll on the app clock**: `poll` gets `now`; keep a `next_poll` in the state, start the read when `now >= next_poll`, and `tx.wake_after` the time left so the window wakes with nothing else moving. A test runs `rt.frame(3.5)` to pass the three seconds.
- **A test reading an element's text** joins the text of every element whose path starts with the keyed element's path; a key that is a prefix of another key (`dream-skipped` and `dream-skipped-list`) pulls the other's text in. Name the keys apart.
- **A progress line with a link inside it** (`Dreaming… reading 3 sessions` and a button) is a wrapping `row` of a text and a pressable text; Fleet's inline text and button wrap together, ours wrap as two items.
## Component 9, piece O (machine update notices)

- **A wrapping row doesn't move a whole button down.** The notice card's three buttons ("What's new", "Change the rule", "Dismiss")
  ran past the card's right edge in a `row().wrap()` with `w_full` and a left padding, even with `shrink().min_w(0.0)`. The card now packs the
  buttons into lines itself (`action_lines`, by estimated width: 6 px a letter plus 20 px of padding and a 6 px gap) and draws one row a line.
  The estimate agreed with Fleet's own wrap for these three labels; a long label could break a line early or late.

## Component 16, piece AB (Folders: Worktrees)

- **A test window that fits a long page**: `App::new(spec()).headless_runtime((1280.0, 1900.0), Env::new(true, 1.0))`, then `frame(0.0)` and `frame(FRAME)`. In the usual 760-high window a control below the fold is never hit by `click`.
- **Rich text with a monospace run** is `rich(body, vec![Span::new(a, b).mono().size(..)])`.
- **No hover border**: `.hover(c)` is a fill only, so Fleet's `hover:border-accent/45` on the token chips isn't drawn.

### Component 9, piece W (the sweep)

- **Nothing new about Rune.** The sweep found no row that Rune blocks beyond the ones already written down: no frame read-back for a window screenshot (9Q), no pressed tint or focus path for the rail's tooltip and the hints (A, B), no fade on an anchored overlay.
- **A background-work picture is a race**: the scripted model's `background-work` runs about 30 s, and a native start costs 5 s or more on a software renderer. Start the session, then the app, tap the counter in the script at 6 s, and take the picture at the mark after it; a script that taps at 3 s finds no counter yet.

## Component 16, piece AC (Folders: Storage)

- **The app's clock follows the frames**: `State::clock` is set from the runtime's time on every poll, so a test that adds to it is overwritten. To age a cache entry, run `rt.frame(seconds)`.
- **No "is it on screen" signal**: Rune can't tell a view that its element has scrolled into view (Fleet uses an `IntersectionObserver` so the Storage card measures folder sizes only once seen). Sizes start when Folders opens; the decision is in `docs/parity/16-settings.md`, Part 10 notes.
- **One-sided borders** are `border_sides([top, right, bottom, left], colour).border_layout()` (the rules between the preview's rows).
- **A snapshot doesn't scroll to an anchor** (`--snap` takes the first frames), so a scene of a card below the fold uses a tall `--size`.

## Component 16, piece I (the code box)

- **No code editor, so the box is a mono multi-line `text_input` in a scroll with a column of numbers beside it** (`ui::code_box`). The text wraps (`text_input` always wraps; there is no unwrapped multi-line field), so the numbers go by lines of the text and the ones under a wrapped line sit a row too high. No colours while typing, no bracket matching or closing.
- **`on_input_key` sees Ctrl/⌘ S and Tab before the field**: returning true keeps Tab from leaving the field, so Tab can insert two spaces with `TextEdit::insert`.
- **The caret's line is `TextEdit::focus` (a byte offset) counted in `\n`s**; the box needs no layout for its active line number.
- **A test must let the page settle before it presses a button in a row that just opened**: after the press that opens an editor, the layout moves for a few frames and `rt.find` gives a place the pointer doesn't hit yet (`element_at` named the page, not the button). Run ~30 frames, and give the headless window `set_viewport((1280, 2600), 1.0)` so the editor's buttons are in view.

## Component 16, piece S (Dreaming: the nightly row and notice)

- **No time field.** The time is a `text_input` that takes `HH:MM` (also `H:MM` and `HHMM`) and commits on Enter or when it loses focus (the poll sees `focused` go false); the browser's spinners and the OS's 12/24-hour display can't be matched.
- **A test that presses something far down a page needs a tall window**: elements below the 760-px window are not found by a click. `tests::app_tall()` is 1280 x 1900.
- **No time zone name without a crate**: Unix gives it in `TZ` or the `/etc/localtime` link; Windows has none, so none is sent.
- **Building the lib tests can pass 3000 MB** with incremental compilation: `CARGO_INCREMENTAL=0 cargo test -j1` stays under the cap.

## Component 16, piece M (What agents can do: levels, Edit files, Run commands)

- **A dashed pill border works** (`border(..).border_layout().border_dash(3.0, 3.0)` with `radius(999.0)`): the fixed chip and "+ exception" use it, so row 6.20's "dashed" needs no workaround.
- **No inline element in a wrapped line**: Fleet's "Needs you" is a glyph and a bold word in one coloured run. A `rich` span colours and weighs the word, and the glyph is the diamond character in that colour.
- **`Open::Select` takes a `&'static str`**, so a select that exists once per chip needs the chip kept in `State` (`settings.permissions.chip`) and one open name for all of them.
- **Linking the lib tests passed 3000 MB again**: `cargo test -j1 --config 'profile.dev.package.fleet-native.debug=0'` stays under the cap and only rebuilds this crate.

## Component 16, piece N (What agents can do: messages, machines, nobody watching)

- **An inline code span inside a wrapped paragraph works with `Span::mono().size(..).bg(colour, pad)`**: `<code>` needs no element.
- **A started `Link` reads home's machines at once**, so a test that seeds `machines` and then sets `s.link` is overwritten by the fake's answer. Wait for the first read to land (`machines.list` empty), then seed again.
- **A choice that is disabled while it saves is ignored when a test clicks it before the answer is handled**: wait for the saving flag to clear before the next click.

## Component 16, piece J (Harnesses: Install panel, Copilot path, update strip)

- **A wrapping row's text needs `.shrink().min_w(0.0)`**: a long message in a `flex_wrap` row with buttons beside it otherwise keeps its full width and runs under them; a mono command box in a card needs the same on the box or it overflows the card.
- **No CSS grid**: `fit-content(15em) 1fr` is a label column sized from the longest label's character count.
- **`cargo test` of this crate peaks near 3 GB in rustc**: under a 3000M cap it is OOM-killed (exit 143, nothing printed); `CARGO_BUILD_JOBS=1` and a 3500M cap built it.
- **A click can't reach below the first screen**: a test of a control further down the page calls the state method and checks the element exists.
## Component 16, piece K (Providers)

- **No margin-left:auto**: a right-pushed item in a wrapping row is a `el().grow(1.0)` sibling before it.
- **A first-run dialog makes the page inert in a test**: with no harness on, `harness-setup-overlay` covers the window and a click does nothing; close it (`harness_setup.close()`) before pressing in the page.
- **Text keys**: a test reads a text with `text_of(key)` only when the key is on the text element itself, not on its row.

## Component 16, piece O (What agents can do: the Browser group)

- **A page that is one column inside a `scroll()` box never scrolls.** `el().scroll(..).child(page_column)` squeezed the single child to the window's height (its overflow isn't scrollable), so no Settings page scrolled at all: `scroll_to`, the wheel and the anchors did nothing. Make the page's own column the scroll box (`page_column.padding(24.0).scroll(..).grow(1.0).min_h(0.0)`); its children then lay out at full height. Smallest case: a `col().h(200.0).child(el().scroll(..).child(col().child(10 rows of 40 px)))` has no scroll range; with the scroll on the inner `col` it has.
- **`Tx::scroll_into_view` only reveals an edge**: a group whose top is already on screen but whose body is below the fold doesn't move. `Tx::scroll_key_to(key, 0.0)` puts the top at the top of the box (instant, clamped to the end of the content), which is what `scrollIntoView({block: "start"})` does. It is instant, not smooth.
- **A radio has to be drawn**: a 13 px ring with a 7 px dot, `role("radio")`, `checked(..)`, and the group `role("radiogroup")`. Disabled ones take `opacity(0.6)` and no press.
- **`El` has no `shrink_0`**: a child doesn't shrink unless it asks with `.shrink()`.
- **No loop to wait on in a checker thread.** The updater's checker sleeps on an `mpsc` channel with `recv_timeout`, so a press of "Check for updates" wakes it at once instead of waiting out four hours; the sender lives in `State` and the thread keeps working if it is dropped.
- **A flexible bar needs weights, not a percent width.** Rune has no percent width; a track of `row()` with two children `w(0).grow(done)` and `w(0).grow(100 - done)` draws a bar that follows its container (`view/settings/updates.rs::bar`).

- **A form inside a `scroll()` list is clipped by it**: a button below the list's `max_h` is in the element tree but `rt.find` gives a point outside what is drawn, so a click lands on nothing. A test reaches it by `rt.wheel` over the list first (`scroll_to` in `src/tests_settings_harness_flow.rs`).
- **`on_edit` for boxes made at run time** (one per field a provider sends) takes a closure with a captured index into a `Vec` and a spare `TextEdit` for the frame after the vec went away: `State::flow_edit_mut`.
## Component 16, piece W (Updates & about: Machines)

- **A right-pushed item in a wrapping row, with a growing sibling:** `grow(1.0)` on the item itself and `justify_end()` (no spacer) right-aligns it whether it shares the line or wraps onto its own, as `margin-left: auto` does. With another item that also grows (a headline), give the right-hand item `grow(0.001)` so the headline keeps the space.
- **A paragraph beside a button in a wrapping row:** put the rich text in a `col().w(0.0).grow(1.0).min_w(256.0)` with the text `w_full()`; a text element left as a direct child measures at its one-line width and runs under its neighbours (the same as piece J's note), and `basis` doesn't exist, so `w(0).grow(1).min_w(n)` is Fleet's `flex: 1 1 n`.
- **A test that clicks after a rig with a fake Fleet:** the fake's empty harness list opens the first-run overlay (see piece K); close it and run a frame before each click, and use the tall window (`app_tall`) because a click can't reach below the first screen.

## Component 12, planning (Changes and review)

- **Picking diff lines by selecting them looks possible without a pin move** (read at `f1f54ce`, not built yet).
  Put the lines' `selectable` texts in an element with `selection_group(true)`, and add `on_selection_change` to it. Each
  selected text then comes with its path and range (`TextSelection::texts`), so the handler can map the texts to line
  numbers. Fleet's Split selects down one column only. Here that means one selection group per column, so Split is
  drawn as two columns of fixed-height rows side by side, not as rows that each hold both sides.

## Component 10, piece A (the tab model)

Nothing new about Rune: the piece is plain Rust. Two things to know for the pieces after it. Lucide 1.0 renamed `FileJson` to
`FileBraces`, so `tools/gen-icons.py` takes `file-braces` (Fleet's client still says `FileJson`; the picture is the same). And
`State::open_id` is `None` for the mock sessions; piece B's `State::canvas_session()` falls back to the shown session's id, so
`edit_canvases` works on them.
## Component 10, piece F (Progress data)

- **Nothing new about Rune.** The piece is plain Rust. One lesson in the app's own terms: keep a detail drawn while it is marked stale (`ProgressFeed`'s `fresh` flag) instead of deleting it, so a view never draws its empty state between a summary and the detail that follows it.

## Component 12, piece A (the changes list)

- **Nothing new about Rune.** The piece is plain Rust. For the pieces after it: a timer on the UI side is a thread that sleeps and posts back (`Link::off_thread` with a sleeping call), as the 250 ms edit window does; `Diffs` hands out a `Ticket` per request and drops an answer whose ticket is no longer current, so a test drives the rules with no thread at all and only a few cases need the fake server.

## Component 10, piece I (Turns, the logic)

- **Nothing new about Rune.** The piece is plain Rust. For piece J: derive the turns off the frame path and keep an `EditCache`
  on `State`, so a long session parses each call once. A global cache (as Fleet has) shares call ids between parallel tests.

## Component 10, piece B (the tab strip)

- **A tab strip that scrolls: `scroll_x()` on a keyed box with one `row()` inside it**, `.shrink().min_w(0.0)` so the buttons beside it keep
  their place, and `scroll_edge_fade(20.0)` for Fleet's fades. Children of the `scroll_x` box itself shrink to the box; put the tabs in
  the inner row and they keep their size and scroll. `Tx::scroll_into_view_in(strip, tab_key)` brings the active tab in, but a tab
  added this frame isn't laid out yet: `State::keep_strip` waits one frame.
- **A pressable `row()` isn't a keyboard target until it says `.focusable(true)`.** Without it a click leaves nothing focused, `on_key`
  on the strip is never called and `tx.focus(key, true)` finds nothing. Inactive tabs use `tab_index(-1)`.
- **A middle click in a test:** `rt.button_down(x, y, PointerButton::Middle)` then `rt.button_up(..)`; `pointer_down` is the primary button.
- **A control revealed by its parent's hover** is `hover_group()` on the parent and `group_opacity(rest, hovered)` on the child (the tab's
  close button: 1.0 at rest on the active tab, 0.0 on the others).
- **Mock sessions have no server id**, so `State::canvas_session()` is the open session's id on a server and the shown session's id
  otherwise; `edit_canvases` and `open_canvases` use it, so scenes and tests can change tabs without a route.

## Component 12, piece B (the review model)

Nothing about Rune: plain Rust. One thing for the pieces that draw it: word marks are byte offsets into a line's content, so a view slices with `marked_segments` and never counts characters.

## Component 12, piece C (one file's contents)

- **Nothing new about Rune.** Plain Rust. For piece D: call `State::load_diff_file(session, &file)` when a section opens (it ignores a version already there) and read `State.diff_contents.entry(path)`. The pure `Contents` hands out `Request`s; a request freed a place hands the next one back from `finish`, so the thread half is a loop of two functions.

## Component 10, piece K (Agents data)

- **Nothing new about Rune.** The piece is plain Rust. For piece L: a row's own session stream needs a side stream (watch the child, hand
  its snapshot over), because a watched topic's snapshot isn't given to `State`; `summarize_agent_activity` takes the reduced messages.

- **A "now" ring outside a clipped box** (Progress flow bar): the segment clips its fill with `clip_rounded`, so the ring is a 1.5-spread `Shadow` on an unclipped wrapper around it. Inline code in a wrapped title and the sub-count are spans of one `rich` text (`mono().size(11).bg(colour, 3)`); a `Span` can't add margin, so the count follows two spaces.

## Component 10, piece C (server canvases)

- **Nothing new about Rune.** For a test of a request made off the UI thread: record `(method, path)` in the fake server *before* it
  answers and wait for that with `test_wait::recv`; hold an answer back with a channel to test events that arrive during a load.
- **A scripted run against a live Fleet** closes a server tab with `canvas-tab-close-<tab id>`, which only exists while the tab is
  active; a DELETE really closes the canvas on the server, so re-focus it (`POST .../canvases/{id}/focus`) before each run.
## Component 12, piece D (the Changes body's states)

- **No whole-text select on press** (`user-select: all` on the fix command): the command is `selectable(true)`, so a drag or double-click selects it, but one click does not select it all. Copy command is the way to take it whole.
- **A margin on a full-width element overflows its parent**: Fleet's `m-3` on the error note is a wrapper with padding around a `w_full` note; `mx`/`my` on the note itself pushed it past the panel's right edge.

## Component 10, piece J (the Turns tab)

- **A container query** is `on_layout` writing the width into `State` (`turns_tab.width`) and the view reading it, so the 300 px rule lands one frame late, as elsewhere.
- **A row that wraps and pushes one child right** (`margin-left: auto` in a wrapping flex row) has no equivalent: a `grow(1.0)` spacer takes the free space on a line, but when the totals wrap to a second line they sit at the left, where Fleet's sit at the right of their line only while they fit beside the others.
- **A diff opening under a click**: the list lays out a frame after a diff opens, so a test that clicks the next row needs two frames first.

## Component 14, piece A (a web view over a page tab)

What the app's own host stands on, at `f1f54ce` (the research spike's findings held in the real app):

- **`App::on_window` is the only way in.** It runs on the UI thread with a `WindowHandle` that can be cloned and kept, so the
  web view is made later, when its tab is first shown (`with_raw_handle` → `WebViewBuilder::build_as_child`). It never runs in
  headless tooling, which is how `--snap`, `--script` and tests get the fallback panel without a flag.
- **Where to place a native view: `poll`.** `poll` runs before every frame, and an `on_layout` report is a state change that
  makes another frame, so comparing "what is wanted" with "what was last done" in `poll` places the view one frame after
  layout and hides it in the same frame a handler opened a menu. Nothing native is touched in a frame where nothing changed.
- **Taking the keyboard back without a Rune hook**: `Tx::focused()` read in `poll`. When the page reports focus (its own
  script over wry's IPC), the app calls `tx.blur()`; the next time `tx.focused()` is `Some`, a Rune element took focus, and
  the host hands the keyboard back (`focus_parent`, Windows and macOS). A press on something Rune doesn't focus (a button)
  is still missed: that needs the "a press landed anywhere" hook `docs/research/web-content.md` asks for.
- **Where the keyboard went, with real input on each system** (the CI walk): Linux (X11 with no window manager) moves it with
  the click by itself; Windows and macOS keep it in the web view until `focus_parent`, after which the page's field hears `blur`
  and Rune's composer gets the keys. macOS runners take real input from `cliclick` (`brew install cliclick`), which the research
  spike didn't have.
- **A window resized from outside before its first frame stopped drawing** under Xvfb with no window manager
  (`xdotool windowsize` while the first frame was still being made: the window stayed black, clicks did nothing). Resized after
  the first frame, it was fine. Not chased further: no real desktop does that.
- **Linux: WebKitGTK 2.52 needs EGL and GLES even to draw in software.** Without an EGL vendor its web process aborts ("Could not
  create default EGL display") and the view stays black; with Mesa's EGL but no `libGLESv2.so.2` the app itself aborts when the
  first view is made. A distribution's `libwebkit2gtk-4.1-0` brings both. Under Xvfb with no GPU Mesa falls back to llvmpipe
  and says so on stderr (`DRI3 error`, `/dev/dri` permission), which is harmless.

## Component 12, piece E (the review's header and file sections)

- **`sticky_top(0.0)` on a card's header** inside a `scroll()` column is Fleet's `position: sticky; top: -12px` inside the body's padding: it holds the header at the viewport's top while the card scrolls under it, and releases it at the card's end. Nothing more was needed.
- **A control that is `display: none` until hover takes no room in CSS; `group_opacity(0, 1)` keeps its room.** In a header that must give its path the width, pin the control (`pin(None, Some(top), Some(right), None)`) over the header's right end with the hover colour as its fill, so it takes no room while hidden. A row with a hidden-but-present child squeezed the path's `truncate()` to nothing and let the file name run into the counts.
- **A pressed state has no accessibility field of its own** (`aria-pressed`): `checked(on)` on a `role("button")` is the nearest; a test reads the state it came from (`State.review`), since `ElementInfo` has no `checked`.
- **`rt.elements()` still lists an element that is leaving** (faded out): a test that reads text after a state changes filters `!e.exiting`, or reads the old words.

## Component 10, piece L (the Agents tab)

- **A grid with a sub line across columns 2 and 3** (`8px 1fr auto`) is two rows here: the first line holds the mark, name and state word, the second is indented 18 px. The state word sits on the first line, as in Fleet.
- **A glyph wider than its box** (the 12 px check in the 8 px mark column) is a 12 px box with margin -2 on each side; negative margins work in a row.
- **A box that is `w_full` with a left margin** overflows its parent (as in 12D): put it in a `w_full` row with `pl` and let the box `grow(1.0)`.

## Component 14, piece B (the page bridge and the theme)

Nothing in Rune was needed. Learned about the web view:

- **`window.postMessage` can be replaced in WebKitGTK** from a document-start script, so Fleet's own script in a page
  (`parent.postMessage(...)`, with `parent === window` at top level) lands in the wrapper without any change to Fleet's file. The
  wrapper has to keep the original: a message *to* the page goes in through a function that calls it, or the wrapper swallows it.
- **`evaluate_script` and the init script run in the page's own world**, so a page's `message` listener hears what
  `__fleetDeliver` posts, and Fleet's script accepts it (`event.source === parent`).
- A wry IPC handler runs on the UI thread, so it can call `AppHandle::update` and, inside that, `evaluate_script` on the same view.
- With a real window, a theme key (Ctrl+Shift+L) needs the keyboard on Rune, not in the composer's text: click something first.

## Component 10, piece H (the subagent cards)

- **A 1 px border that lifts to another colour on hover** has no hover-border in Rune (`hover` is an overlay on the fill). Draw the border as a 1 px padded ring (`fill(border)`, `padding(1.0)`) around the card and give the ring `hover(wash)`; the overlay shows only at the ring. Radius of the inside is the outside's minus 1.
- **A line-height of 1.4 where CSS inherits 1.5** shortens a card by a few pixels: read `line-height` from the component's cascade (the body's), not from the nearest class.

## Component 14, piece C (the page tab's bar and choice bar)

Nothing in Rune was needed.

- **A sentence with a bold pick and a small time after it** is one `rich` text with spans (`weight(600)`, `size(11)`), not three elements:
  it wraps as Fleet's inline `<p>` does. The 8 px `margin-left` before the time is three spaces.
- **`border_sides([1,0,0,0], c)` with `border_layout()`** is `border-top` that takes room, as the bar's and the warnings' do.
- A keyed button's text is its child's, so a test reads `ElementInfo::label` (`.label(words)`) for it, not `text`.
- Under Xvfb with no window manager the window opens at 1360x880; `xdotool windowsize` to 1280x760 relays it out, so a grab can be the
  size of Fleet's own picture.

## Component 10, piece M (Agent's view)

- **A picture placed by arithmetic** (`scale = min(w/1280, h/800)`, pinned to the top, centred across) is a `pin(Some(left), Some(0.0), None, None)` element with `w`/`h` set from the stage's size and `pixels_fit(.., SurfaceFit::Fill)`. `pixels_fit` alone only centres. The stage gets its size from `on_layout`, one frame late, so the first frame draws no picture.
- **A pinned element needs no "relative" parent**: ring, caption and tag are all `pin(..)` children of the stage. Nothing without a handler takes the pointer, so Fleet's `pointer-events: none` ring needs nothing.
- **No transition on a position**: Fleet's 200 ms ease of the ring's left, top, width and height is a jump here.
- **A Browser tab is a server canvas, not a built-in**: `Canvases::open(.., CanvasKind::Browser)` does nothing. A scene stands one in with `canvases.replace` and sets `active_id`.

## Component 12, piece F (hunks, folds and lines)

- **A `scroll_x` viewport lays its own children side by side** (a filmstrip), so rows put straight on it ran off to the right at one height. Put one `col` of rows inside the viewport, as `rune-kit`'s markdown tables do.
- **A `scroll_x` viewport inside a clipped card shrinks to nothing** when the card's column is squeezed (it has no minimum height of its own): give it `h(n)` and `min_h(n)` when its rows have a known height.
- **`scroll_edge_fade(28.0)` on a horizontal viewport fades the left edge too once scrolled**, where Fleet's mask only fades the right; `sticky_left` children aren't faded on the edge they stick to, so the line numbers stay clear.
- **`TextHighlight::new(range, colour).pad(0.0)`** gives Fleet's word marks (`<mark>`), with byte offsets that match `conv::review`'s marks; the corner radius isn't settable.
- **`text().wrap_anywhere()` with `.shrink().min_w(0.0).grow(1.0)`** is `white-space: pre-wrap; overflow-wrap: anywhere` for a code line beside fixed-width number cells.
- **Left and Right arrows don't scroll a focused `scroll_x` viewport**, so a hunk's tab stop (Fleet's `tabindex=0` region) can be reached but not scrolled by keyboard.

## Component 10, piece E (the strip's motion and drag)

- **A slide-in or a fading background is drawn from the app's clock**: `opacity`, `offset` and `scale` take plain values each frame, so the view computes them from `State.clock` and `poll` asks for frames (`wake_after`) until the motion has run. A test sets `s.clock` and reads in the same `update`: the next frame's `poll` sets the clock back.
- **Dragging a scroll box that is full of pressable children**: put `on_drag_any` on the box *and* on each child (a press on a child never reaches the box's handler). Read the starting offset from `on_scroll_offset` (a thread-local `Cell`; that callback can't touch `State`), then `tx.scroll_to_x(key, start - d.dx)`. A click on a child still presses.
- **`rt.find(key)` is the element's centre**, and `rt.elements()[..].rect` is `[x, y, w, h]`; a tab scrolled out of a strip has a rect far outside the window, so a test aims a pointer at the strip, not the tab.
- **A plain vertical mouse wheel over a `scroll_x` box does nothing**; Fleet turns it into a sideways scroll (`useDragScroll`). Not worked around.

## Component 14, piece E (the Document tab)

Nothing in Rune was needed.

- **A button inside a column is stretched** to the column's width (`items_stretch`); to keep it as wide as its words, wrap it in a `row()`.
- **`WebViewBuilder::with_html`** takes the page's text; wry's `load_html` loads a revised one. A `with_navigation_handler` that refuses
  anything but `about:`/`data:`/`blob:` keeps a link in the document from replacing it.
- **A conversation's "mounted at"** (Fleet's `Date.now()` at mount) is read once per session by `keep_visuals`; a test that drives
  `watch_visuals` with its own clock must count from `Visuals::mounted_at`, since the frames before it already took the real time.
## Component 14, piece D (the web view host everywhere)

- **Rune has no app-wide press hook.** `on_pointer_move` on the root sees moves only, and a press goes to the nearest element with a handler. A native web view that has the keyboard (Windows, macOS) needs to know when the user pressed anywhere in Rune; only a press that focuses an element shows (`Tx::focused`). Smallest case: blur, then click a rail button; `focused()` stays `None` (`tests_web_host`). A `App::on_press(fn(&mut S, &mut Tx))` before handlers would do.
- **`on_layout` reports where an eased element comes to rest**, also for an ancestor's spring: a native view placed from it jumps to the new width at once while Widen eases. The host waits out the 200 ms instead (`Host::settle_until`).
- **`rune::memo::window_layout(key)` works inside the view build with no `Cx`**: `ui::overlays` reads the rect of each floating overlay by key while it builds and the view builds again when one moves.
- **A keyed element reports no rect until laid out**: read as "might be over the page" (hide) rather than "not".
- WebKitGTK: a view is black until it first paints; keep it out of the window until `PageLoadEvent::Finished` (6 s cap). `GDK_SCALE` is a whole number, so on X11 at 1.5× the view is placed at 2× positions.

## Component 14, piece G (app runs)

- Nothing new about Rune: plain Rust over `Link::off_thread`. The one thing worth knowing is that a request must record the store's event clock when it is *asked* (on the UI thread), not when its thread starts, or a late answer can undo an event that came in between.

## Component 12, piece G (Split)

- **`minmax(max-content, 1fr)` is `.w(0.0).grow(1.0).min_w(content)`**: flex basis 0 with a minimum, so a side with nothing long stays narrow until the region is wider than both. `.w(content).grow(1.0)` instead keeps each side's own width and shares only the extra, which isn't the grid.
- **Two exactly equal halves** (wrapped Split) need `.w_frac(0.5)`; basis 0 with `grow(1.0)` followed the wrapped text's width.
- **No pattern or hatch fill**, so a padded cell is a flat wash.
- **There is no way to measure a text's width while building a view**, so an unwrapped Split column's minimum comes from the character count times a mono advance of 0.6 em.

## Component 12, piece H (drawing what's near)

- There are no per-element rects in the app, only `on_scroll_offset` (`ScrollPos`: offset, max, viewport). To act on what is near the
  visible part, keep the place in an `Rc<Cell<..>>` from the handler (it may not change state), read it in `poll`, and work each
  section's top out from heights you know. Return true from `poll` only when the set that matters changed: returning true on every
  frame of a scroll prints Rune's "committed the view on N of N frames" warning.
- In headless tests `rt.wheel(x, y, dy)` with a **negative** `dy` scrolls further down; `ScrollPos` is `#[non_exhaustive]`, so a test
  can't write it as a literal, but it derives `Default` and its fields are public: `let mut at = ScrollPos::default(); at.offset = ..`
  builds one (`fleet::tests::history::reading`).

## Component 14, piece F (page cards and full size)

Nothing in Rune was needed.

- **`modal_tx(true, …)`** gives Escape and the scrim's press with a `Tx`, so a dialog can hand the keyboard back (`tx.focus(key)`) as it closes.
  `tx.focused()` gives the key to return to; read it before the dialog takes focus.
- **A mock scene that wants the fold plan** (something that depends on several messages) has to go through `State::stream`, not `show_conversation`,
  which resets the plan.
- **A web view in a dialog** is the one `Wanted` placement while the dialog is up; the other views move off screen by the normal rule.

## Component 12, piece I (Revert)

- **A confirm that has nothing to wait for is a `Call` the dialog closes on.** `ui::confirm`'s dialog holds itself open for a request (Delete session); Fleet's `confirmAction` with no `run` closes at once and the caller does the work after. `Call::Revert` takes that path in `take_confirm`, like the machine calls, so no new dialog was needed.
- **`rune::El` has no `items_baseline`.** The notice's words and Dismiss are centred instead of baseline-aligned; one line of text makes them the same.

## Component 12, piece J (find in changes, the logic)

- `Mods` off macOS sets `cmd` together with `ctrl`, so a chord that must not fire with Cmd (F3 plain) checks both. `Key::Command(c)` carries
  the character as given: `Key::Command('G')` does not match `is_command('g')`'s own `to_ascii_lowercase` on the argument side only, so a test
  sends `'g'` with `shift` held, as a real key arrives.
- A request that Fleet aborts (`AbortController`) has no cancel here: `Link::off_thread` runs to its end, and the answer is dropped by a run
  number when a newer query has replaced it. The old request still costs the server a git search; it isn't sent at all inside the debounce.
- The debounce is `tx.wake_after` from `poll`: nothing else wakes an idle app 120 ms after the last key.

## Component 13, piece A (the terminal)

- **`TextStyle::tracking`** pulls JetBrains Mono's 7.22 px advance to xterm's floored 7, so a `GridRow`'s columns land on 7 px cells (`term::screen::style`); pinned cells are measured with the same style.
- **`rt.key` is queued**: a key reaches `on_key` on the next `rt.frame`, so a test that types and then waits without framing sees only the earlier keys.
- **`El::on_key` on a `focusable(true)` box** receives every key before the window's `on_key` (Esc, Ctrl K), and a click focuses it with no `on_press`; adding an `on_press_tx` as well swallowed Enter.
- **A focused box's focus state isn't readable while building the view**: `Tx::focused()` is read in `poll` once a frame and kept in `State` for the cursor's block-or-outline.
- **`Term::read` closures**: `impl Trait` can't nest in a closure bound; `alacritty_terminal`'s listener type has to be public for a wrapper to name it.

## Component 13, piece B (the connection)

- **`Term::damage()` needs `&mut Term`**, so reading the screen takes the terminal's lock for writing (`Terminal::read_damage`); it must be read and `reset_damage()`d once per
  read by one reader. Damage says nothing when scrolled back or after a resize: both read every row.
- **A blocking `ws.read()` with a 20 ms timeout hides the UI's queue for that long**: input sent just before `ready` can be written directly after it
  instead of being kept, so a test can't observe the pending queue through a socket; it is tested on the queue itself.

## Component 12, piece K (the find bar, marks and keys)

- **`Tx::scroll_center` centres vertically only**, so a match far to the right of a long line isn't brought sideways (Fleet's `scrollRangeIntoView` does both).
- **A window key reaches `on_key` only if the field's `on_input_key` returns false**; Esc and Enter are taken in the field and Ctrl+F, F3 and Ctrl+G by the window's handler, which asks `tx.focused()` so the key stops in Changes. A tab stop that is not an input (`review-scroll`, `.focusable(true)`) is what a click in the diff focuses.
- **A test sends Ctrl+letter as `Key::Command('f')` with `cmd` and `ctrl` held** (not `Key::Text`).
- **`TextHighlight::key` on two copies of a line** would duplicate the key: only one copy (the old side of an unchanged Split row) is keyed for the scroll.

## Component 14, piece H (the browser tab)

- **`text_input` takes a `&TextEdit`**, not a `String`: the address field keeps a `TextEdit` in state (`BrowserUi::address`) and `on_edit` returns `&mut TextEdit`. A closure that captures the tab id works for `on_edit`; setting the text again (`TextEdit::new`) loses the caret, so it is only done while the field has no keyboard.
- **A field has no focus/blur callbacks**: whether it is being edited is read from `Tx::focused()` in `poll` and kept as the change (gained, lost), which is what Fleet's `@focus`/`@blur` give.
- **A dropped WebKitGTK view stays on screen until GTK's loop runs once more.** `pump` only turns the loop while a view exists, so dropping the last one (a tab showing an error instead of the page) left its pixels there. `platform::keep` now drains GTK's events when a view went and asks Rune for a draw.
- **WebKitGTK's page cache**: Back to a page it kept whole runs no script and fires `pageshow` with `persisted`, not `load`, so the injected preview script reports the location on that event too.

## Component 13, piece C (tabs per session)

Nothing new about Rune. One thing about the harness: a frame hook (`keep_terminal`) is where state that follows other state is kept in
step (the shown view follows the active tab), since a handler that changes the tabs doesn't know which views exist.

## Component 14, piece I (the app's panel, strip and output)

- **`poll` sets `State::clock` from the runtime's own time every frame**, so a test that adds to `clock` has it overwritten at the next frame: drive time with `rt.frame(seconds)`.
- **`Tx::wake_after` takes a `Duration` that must be finite**: a refusal that never expires (a scene's `f64::MAX`) panics `from_secs_f64`; the wake is clamped to the real 10 seconds.
- **`ScrollSpec { stick_bottom: true }` on a column with `max_h(220)`** is the output drawer: it follows the end of new lines and lets go when the user scrolls up, as Fleet's `scrollLogs` does.
- **No native `<select>`**: the port select is a trigger row plus `ui::select::content` opened through `Open::Select`, so its list is Rune's, not the system's.
- **A browser tab's width** (Fleet's container query for the strip) is read with `on_layout` on the tab's root and kept in `BrowserUi`.

## Component 11, the plan (files and the file viewer)

Read at the pin while planning; nothing built yet.

- **`Markdown::code_link(colour, f)`** turns inline code that names something into a link (Desktop's path look: coloured, no chip,
  underlined at 40 % under the pointer), with `on_link_tx` and `link_help`. Component 3 had left file links in messages with no owner
  because Markdown had no inline hook; this is that hook. Fleet's own look (a dotted underline at rest, `--accent-dim` on hover) isn't
  settable through it.
- **`rune_kit::line_selection`** is the line-number gesture (press, Shift, drag) for code and diff views; **`rune_kit::tree`** draws
  file trees with change counts, dots and badges.
- **`rune_kit::syntax` colours only the first 128 KiB** of a source; Fleet opens files up to 512 KB, so a larger file draws plain after
  that point.
- **`text_input(..).spans(..)`** paints syntax colours over an editable text and keeps them through undo and IME, and `App::on_ime` with
  `.ime_cursor()` serves an editor the app draws itself: the two ways slice 2's spike (component 11, piece P) compares. `ui::code_box`'s
  known gap (numbers under a wrapped line sit a row too high; no unwrapped multi-line field) is what makes the first one doubtful for files.
- **No syntax tree**: the highlighter gives tokens, so there is nothing to fold code by, and no outline or symbols.
## Component 12, piece L (selecting lines)

- **One `selection_group` per file plus `on_selection_change` reads a drag well**: `TextSelection::texts()` gives each selected text's key and byte range, so keys that carry the line's index are enough to get line numbers. A text the selection only reaches the edge of comes with an empty range, which is Fleet's "touches only" rule.
- **Split can't leave one column unselectable** without a group per column (the rows hold both cells), so the side is read from the anchor text (`backward()` says which end it is) and the other column's text is still highlighted.
- **`El::keeps_text_selection()` on a bar's buttons** keeps the selection and focus through a press, so the buttons act on what is selected and the bar doesn't vanish when the selection clears.
- **A test drags with `pointer_down`/`pointer_move`/`pointer_up` on rects from `rt.elements()`** in the tall window; the drag must end inside a text (the right padding of a line's text element is part of its rect, so stay 18 points in from its right edge).
- **A script's `drag` on a text's key** (`dx`, `dy`) selects from its centre, enough for a picture.

## Component 14, piece J (Open a browser tab, Agent's view)

- **`ui::dialog::Dialog` had a fixed 32 rem width**; a `sm:max-w-md` window (28 rem) needs the new `width` field.
- **A dialog's field is not focused by Rune on its own**: `tx.focus(KEY, false)` from the handler that opens it, as the fork dialog does. A test then sends keys with `rt.key`, not the composer helper, which clicks the composer first.
- **No `aria-pressed`**: the toggle's pressed button is shown by its fill and shadow only.
- **Shared dialog state that follows other state** (watching the agent's tab while Agent's view is chosen) is kept in step by `keep_browser` in `poll` and by the press handler, so the web view is hidden in the same frame.

## Component 11, piece B (reading a file)

- Nothing new about Rune: the piece is plain Rust with no view. One thing learned for the pieces after it: a file's text is kept as its lines (split on the file's own break), because Rune's text elements take lines and a buffer then needs no second copy; positions in `minimal_change` count chars, not CodeMirror's UTF-16 units.
- A test that drives a `State` over a fake server prints Rune's debug-build "memo(..) replaced its root's key" warning once; `RUNE_PERF_WARN=0` silences it.

## Component 13, piece D (the drawer)

- **A press changes state after `poll` ran in that frame**, so a view that follows state in `poll` (`keep_terminal`) shows the change one frame late. A handler that must show at once calls the follower itself (`show_terminal_tab` calls `sync_terminal`).
- **`truncate_head` cuts a text at its start** with "…" (Fleet's `direction: rtl` folder), and `scroll_x` on a strip with `shrink()` scrolls the tabs sideways with no scrollbar.
- **A child hidden by `group_opacity(0.0, 1.0)` still takes presses**: the tab's close button is under the pointer wherever it is drawn.
- Mock sessions have no id, so a scene that needs a session (the drawer is per session) gives one with `set_session_id`.
## Component 12, piece M (Rendered, the focus request)

- **`rune_kit` Markdown inside a scrolling list needs no wrapper**: `view::prose::prose` in a padded `col` sits in the review's scroll like any other element, unlike a web view, which is why HTML gets 14's card there.
- **`tx.scroll_into_view_in(scroll_key, card_key)`** needs the card drawn first, so a request that also opens and marks a file near does the scroll one poll later (`wake_after` 16 ms); it brings the card into view, it doesn't pin its top to the scroll box's.
- **A script's `tap` is the only pointer event** (no `move`): to show a header's hover actions, tap one of them that changes nothing.

## Component 11, piece C (keeping open files in step)

- **`App::on_focus` is the window-focus event**: it fires when the window gets or loses the keyboard, so Fleet's `window` `focus` listener is one line there (`if focused { s.file_window_focus() }`).
- **No "tab activated" hook in the model**, so a tab coming to the front is seen by comparing the active tab id in `poll` (`keep_file_front`), the way `keep_strip` does for the strip; it reads the id without copying the session's tabs.
- **A debounce is a generation number and a sleeping `Link::off_thread`**: each queue bumps the number, and the timer that wakes finds itself stale and reads nothing. A test of "once" checks the queue as a rule and a fake server for what is asked.
- **A test that waits for a second read must wait for the first one to be applied** (`!is_reading`), because a refresh asked while one is on its way is skipped on purpose.

## Component 11, piece D (the Files tab)

- **A press then a double-press** reach `on_press_tx` and then `on_double_press_tx` in that order on the second click, so a single click's wait is a deadline kept in `State` and checked in `poll` (`keep_files_tab`, with `tx.wake_after`), and the double-press just drops it. No thread is needed, and an invented session (no link) works the same.
- **No way to change a fill on focus-within**: `focus_within_ring` draws the ring but the field keeps its resting fill, where CSS `:focus-within` swaps it for transparent.
- **A vertical scroll box is `.scroll(ScrollSpec { stick_bottom: false })`**; there is no `scroll_y`.

## Component 12, piece N (the annotation popover)

- **A toolbar that waits for a drag's release** is `on_selection_change` on a `selection_group`: it is heard with `dragging()` true while the drag goes and once more with it false at the release. Keep a "was dragging" flag in the state, so a selection made by Cmd+A or by code (never dragging) opens nothing, as Fleet's `mouseup` does.
- **The release carries no point.** A `on_pointer_move_local` on the element that holds the text, writing `window_x/window_y` and calling `tx.live_only()`, keeps the last place; the popover opens there. `selection.text()` is the selected text as copying takes it (backticks of inline code are gone, as `textContent` is in Fleet).
- **`tx.clear_text_selection()` inside the selection handler** is fine (the handler is heard once more, with no selection).
- **Pressing a disabled button takes focus from a text field**: a test that clicks Send while it is off must click the field again before typing.
- **No block rectangles from `rune_kit::markdown`**, so Fleet's click-on-a-block anchor can't be ported without drawing the Markdown blocks ourselves.
## Component 13, piece E (height and keyboard)

- **A focused element's `on_key` that returns `true` for every key keeps Fleet's shortcuts out**; return `false` only for what must go on (Mod+J, Mod+Shift+B), and the global `on_key` then sees it.
- **Ctrl+Shift+V and Ctrl+Shift+C reach `on_key`** (they are chords, not edit commands); `tx.read_clipboard` then gives the text.
- **A test sends Ctrl+J as `Key::Command('j')` with `cmd` and `ctrl` both set**, as a real Linux or Windows key arrives; `Mods::default()` stops being Ctrl+J once something checks `cmd`.
- **`on_drag`'s `dy` is the pointer's travel since the press**: keep the start height in `d.data` and set `start - dy`; an element placed `absolute(0, -5)` outside its parent still takes the pointer.
- **`scroll_display` damages the whole screen**, so the row cache needs nothing extra.
- A test that waits for the app to react to a focus change must wait with the runtime driving frames (`Live::wait`), not a bare `until_true`.

## Component 13, piece F (selection, copy, paste, the bubble)

- **`rt.wheel` only drives scroll views; an `on_wheel` handler is reached with `rt.wheel_xy(x, y, dx, dy)`.** A notch arrives as `precise: false`.
- **`Drag` carries `clicks` (1, 2, 3) but no modifiers**, so Shift+click to extend a selection can't be told from a click.
- **There is no `translate(-50%, -100%)`**: a bubble centred on a point is placed from its measured size (`rune::text::advance` for the words).
- **`.focusable(false)` on a pressable keeps the keyboard where it was** when it is pressed (CSS `mousedown.prevent`).
- A replayed terminal that has a socket is resized to its box by `fit_terminal`; a test that wants a fixed size gives it no socket.

## Component 11, piece E (searching by name)

- A one-line bottom rule on a row is `.border_sides([0.0, 0.0, 1.0, 0.0], c).border_layout()` (top, right, bottom, left); without `border_layout` it takes no room.
- A debounce that waits for the clock needs a `tx.wake_after` for the remaining time, or a quiet window never draws the answer.

## Component 12, piece O (find in files, the logic)

- **Nothing new from Rune.** A debounce is the app's own clock plus `tx.wake_after`, as Find in Changes does; a test runs it by drawing frames until `State.clock` has passed the pause, and waits for the request with `test_wait::drive` before it counts requests.
- **Ctrl/Cmd+Shift+F is read in `on_key`** like the other window shortcuts; `is_command('f')` ignores the shift, so check `mods.shift` for it and leave Ctrl+F (`is_find_shortcut`) to the find bars.

## Component 11, piece F (file tabs in the strip)

- **`ElementInfo` has no font style**, so a test can't read "italic" off the drawn tree: test the function that chooses it (`name_italic`) and look at the picture.
- **A tab scrolled out of a `scroll_x` strip is still found by key, but a click on it lands on what is drawn there** (in a live script it hit the title's "..." menu). Widen the window or the panel before tapping a far tab; in a test, activate the tab through state, let the strip scroll to it, then click its pin.
- **A new tab slides in for 240 ms**: run about 30 frames before clicking it in a test.
- A child's `on_press_tx` (the pin, the close button) doesn't reach the tab's own press, so no `stopPropagation` is needed.

## Component 12, piece Q (the sweep)

- **`--script` has no `shortcut` for app-level keys** ("unknown shortcut registration"): reach a state by tapping its keyed element (`content-rail`, `canvas-tab-files`, `files-mode-contents`) instead of Ctrl+Shift+F.
- **`--script --server <url> --snap … --at mark:<name>` draws a live scratch Fleet headless**, so a live walk is one run per picture (about 25 s each); a tap on a key that isn't on screen yet stops the script, so leave 1.5 s after a click that opens something.
- **A tap lands where the element is drawn**: tapping a file header whose hover actions overlay it can hit "Rendered". Tap a line or a button, not the header, in a walk.

## Component 12, piece P (find in files, the view)

- **Marked words in one line of text are `Span`s with a `bg`**, set with `.spans(...)` on a single `text` (byte offsets into the string you built): no row of little elements, and `.truncate()` still ends the line with an ellipsis.
- **Arrow keys are `Key::Down` and `Key::Up`** (not `ArrowDown`); in a `text_input`'s `on_input_key` returning `true` stops the caret moving.
- **`tx.focus(key, false)` then `tx.select_all_text(key)` in one poll** focuses a field and selects its text.
- A press that opens a tab moves the keyboard off the field, so a test that wants more keys in the field presses them before it presses Enter.
## Component 11, piece G (the file tab's code view)

- **A virtual row's key is the list's, not the element's.** `virtual_rows(count, est, key, build)` keys each row `key(s, i)`
  whatever key the built element had, so `rt.find("code-line-3")` only works if the `key` closure returns that name. Name the rows
  in the key closure and key the pieces inside (the text) on the element.
- **Sideways scrolling and a held gutter on a virtual list** work together: `scroll_xy()` on the viewport, the growth column with
  `virtual_rows` inside, `sticky_left(0.0)` on each row's number cell. The scrolling width is the widest row built, so a very
  long line far from the window doesn't widen it until it is near.
- **Code colours stop at 128 KiB** (`rune_kit::syntax`, `MAX_SOURCE`, cut at a line end): in a file between 128 and 512 KB
  (what the viewer opens) the lines after that point draw plain, and no 1 MB-style fallback exists. The viewer joins the buffer's
  lines once per buffer revision and cuts the colours per line (`view::code_view::coloured`); the highlight runs on the UI thread
  the first time, about the cost of the 128 KiB it colours.
- **No properties or INI lexer**: `lang_for_path("x.env")`, `.ini` and `.properties` name no language, so those files draw plain
  where Fleet colours them.
- **`SyntaxTheme::spans` sets no italic or underline**, only colour and weight; the viewer builds its own `Span`s from the
  highlights to italicise comments and underline links as Fleet's theme does.

## Component 13, piece G (cells, the cursor, links)

- **`GridRow::shape` draws in the row's own points from its top-left corner**, one point a pixel at 1×: a 1 px line centred at `x + 3.5` in a 7 px cell is crisp, a 3 px heavy line centred there is too. Edges of block fills are rounded to whole points so neighbours meet.
- **`ElementInfo.rect` is `[x, y, w, h]`**, not corners.
- **`on_pointer_move_local` gives the pointer in the element's own points**, the way to find the cell under it; a small child with `.cursor(Cursor::Pointer)` and `on_press_tx` is the way to get a hand over just a link.
- **Rune's `Runtime::take_url_requests()` returns what `tx.open_url` asked for in a test.**
- A blink is the app's clock plus `tx.wake_after` for the time left in the half period; alacritty's default cursor style must be set to blinking in its `Config` (`default_cursor_style`) or a cursor the program never set is steady.

## Component 11, piece H (Wrap)

- **No negative text indent**: a hanging indent is a fixed-width empty column beside a `wrap_anywhere()` text in a row (`grow`, `shrink`, `min_w(0.0)`); the indent is then not part of a copy.
- **`virtual_rows` takes rows of different heights**: unvisited rows use the estimate and a measured row keeps its height by key. A row whose height depends on a setting (wrap) needs a different key when the setting changes (`code-line-3w`), or unmounted rows keep the old height.
- **An element's fill fades**: `ElementInfo.fill` reads a pressed toggle's colour only after about thirty frames in a test.

## Component 11, piece I (images and Rendered)

- **`ImageCache::load` decodes SVG too** (and AVIF), without a mime type, but draws an SVG at twice its own size so it stays sharp when scaled: `Image::size()` of an SVG is double its natural size. Halve it to say "96 × 96"; the app's own `sniff_image_mime` is for attachments and doesn't know SVG.
- **A checkerboard is a `Shape` of squares sized to the picture**, since a shape's points are in the element's own points and `view_box` scales them. The picture's size is worked out in the view from the area `on_layout` last reported (the lightbox does the same), so the first frame uses the panel's width.
- **A web view of a file is placed by `web::wanted`, not by the view**: the view only draws the frame (`web::html_body`) and reports its rectangle; the file tab's HTML is one more source for `Placement.html`, keyed by its text so a changed file loads again.
- **A scripted run (`--script`) is headless even with a display**: nothing shows on screen, so a live web view can't be grabbed from one. Open the window under Xvfb and click with `xdotool` (`scripts/web-walk/x-click.sh`). The window opens at 1360x880 whatever the screen.
## Component 13, piece H (mouse reporting, modes, input methods)

- **`on_drag` carries no modifiers and no pointer-move-with-button**: keep the held modifiers yourself with `App::on_modifiers` (it isn't called by `App::new(spec())` in a test, so apply the hooks through one function both `main` and the test use) and read the button with `tx.button()` after `El::pointer_buttons([Middle])`. The secondary button never arrives as a drag: `on_secondary_press_local` is a press and its release.
- **`on_pointer_move_local` fires with no button held, not during a drag**: motion for a mouse mode comes from both handlers.
- **`El::ime_cursor` on an element over the drawn cursor** turns the input method on for it; `App::on_ime` hears `Ime::Commit` (plain typing still arrives as keys). `Runtime::ime(Ime::Commit(..))` drives it in a test. The preedit has to be drawn by the app.
- **alacritty_terminal has no mode 9 (X10)**: its parser keeps 1000, 1002, 1003 (and 1005, 1006, 1004, 1007) only.
- A script's key step takes `"ctrl": true` with `"name": "c"`, not `"Ctrl+C"`.

## Component 11, piece L (Go to file)

- **`scroll_center_in_instant(scope, key)` mounts a virtual row and centres it**, so a line far down a long file needs no arithmetic; key it by the row's own key (`code-line-12`, `code-line-12w` when wrapped).
- **A scrolling `col` takes the keyboard with `.focusable(true)`** and shows no ring of its own, so the code view can take focus when a file opens.
- **A dialog that is a grid with `gap-4` is a `col().gap(16.0)`**: the 1 px line under its box then sits 16 above the list, as in Fleet.

## Component 11, piece K (picking lines, the lines bar)

- **`rune_kit::line_selection` is Pierre's gesture, not CodeMirror's `line-pick.ts`**: a press on the one selected line clears it on
  release, and Shift keeps the end of the selection the press is away from (Shift on 12 after 18–27 gives 12–27; Fleet's file tab gives
  12–18, from where the pick started). So the file tab's rule is ported in plain Rust (`fleet::file_lines::press`), and the number
  cell drives it with `on_drag`: `Phase::Down` reads Shift from `tx.mods()` and keeps the anchor in `Drag::data`, `Phase::Move` counts
  whole lines from the pressed cell by `Drag::y`.
- **No way to ask which row is under the pointer** from a handler (no element rects on `Tx`): a drag down the numbers of wrapped
  lines counts 20 px per line, so on wrapped rows taller than one line the pick runs ahead of the pointer. Unwrapped (every row 20
  px) it is exact.
- **A test holds Shift for a pointer press** with `rt.physical_input(PhysicalInput::Modifiers(Mods { shift: true, .. }))`; the
  press's handler then reads it from `tx.mods()`.
- **A row's fill over a sticky, opaque cell**: the number gutter is opaque (`--panel-bg`) so code scrolled sideways doesn't show
  through, so a picked line's number takes `--accent` 14 % mixed into `--panel-bg` (`Theme::mix`), not the 14 % alpha the line has.

## Component 13, piece I (the setup terminal)

- One terminal slot serves two hosts (a session's drawer and a setup row inside a window over the app) without a second set of handlers: the slot carries a `setup` marker and every "follow the open session" path steps aside while it is set. Cheaper than a second `Slot`, because the key, pointer, wheel and paste handlers all read `State::terminal`.
- A test rig whose fake speaks both HTTP and WebSocket must look at the request line without consuming it (`TcpStream::peek`) before choosing: `tungstenite::accept` wants to read the upgrade itself, `test_http::read` would eat it.
- A background thread's `Post` (here the socket's "changed") only queues a job; a headless test that waits on shared state, not on the job, must run a job or `update` before it looks at the tree.

## Component 11, piece J (Diff, the stripe, a deleted file)

- **`linear_gradient(PI, stops)` fades a bar's two ends** (CodeMirror's `.cm-collapsedLines`); a fill under it is not needed.
- **`text_highlights` takes byte ranges of the drawn string**, so marks made on the file's text must be moved through the tab/placeholder rewrite (`code_view::shown_range`).
- **A virtual row's version must include everything the row reads**: the stripe lines and the flash step went into `virtual_rows_version`, or rows built earlier keep the old tint.
- Fonts have no glyph for ⦚ (U+299A): the fold bar draws ¦.

## Component 11, piece N (find in a file)

- **A test click needs the pointer over the control for a few frames first.** `tests::click` moves and presses with no frame between, and a control drawn only a moment ago ignores it (the first three clicks on a new panel were lost); move, run 4 frames, then press.
- **`TextHighlight::ring(color, width)` is CodeMirror's `outline`**, and `.key(..)` on one gives `scroll_center` a target for sideways centring that `scroll_center_in_instant` on the row can't do.
- **A row cached by `virtual_rows_version` must carry everything its marks depend on** (the query, the boxes, the current match), or the marks stay as they were.
- **`regex` is the one new crate** (its `regex-automata`, `regex-syntax`, `aho-corasick` and `memchr` were already in `Cargo.lock` through alacritty_terminal); Rune has no regular expressions to use.

## Component 16, piece X (What's new)

- **`Span` carries `size`, `mono` and `color`**, so one wrapped `rich` text can hold a 12 px muted label, a 13.5 px title and an 11.5 px mono pull request number, as Fleet's CSS gives a list item; `Link::new(range, url)` makes the number pressable. A `rich` text under `.shrink()` (not `.grow()`) is only as wide as its words, so a test can press the end of the line to hit a trailing link.
- **`Markdown::on_link` takes `(&mut State, &str)`; the one with a `Tx` is `on_link_tx`.** The El-level `.on_link` is the one with a `Tx`.
- **A headless test on a tall window must wait for a wheel's glide to end** before pressing: `rt.find` reports the element where the last frame drew it, so press once the rect stops changing. A rig with a link to a fake also opens the first-run wizard over the page (`harness_setup.cloud = true` keeps it away).
- **`Tx::scroll_key_to` needs the element laid out**: the row a request names is only drawn once the notes loaded, so the scroll goes out on the poll after the request opens it (`WhatsNewState::scroll`).

## Component 11, piece M (file links in messages)

- **`Markdown::code_link(color, f)` works for resolved paths, and its look is Rune's**: the code draws in the colour, no chip, and an underline at 40 % under the pointer. Fleet draws a dotted `--accent` 70 % underline at rest on a chip. A space shows before the punctuation that follows such a link (", and" reads ", and" with a gap). Not worked around.
- **A link's destination is its key** (`rt.find("fleet-file:src/a.py#L24")` clicks it), so tests and scripts click a link by the destination the app chose.
- **`tx.mods()` inside `on_link_tx` is the click's modifiers**, which is how Ctrl/Cmd-click keeps a tab. Shift-click and Alt-click are the handler's to ignore.
- **No selection check from a handler.** `selected_text()` lives on the runtime, not `Tx`, so Fleet's "a click that ended a drag selecting text opens nothing" can't be asked. A drag that begins in other words and ends on the link opens nothing (the press isn't the link's); one that begins on the link and ends on it opens it.
- **Links are Tab stops before the composer**, so `Shift+Tab` from the composer reaches the last link, and Enter opens it. Focusing the text's own key doesn't put focus on a link.
- **`on_link_tx` makes every link in the text pressable**, so a handler that opens web addresses is needed with it (`tx.open_url`).

## Component 13, piece J (the sweep)

- A headless `--script` run drives the app's clock, not wall time, while a socket thread (and a server killed from outside) runs in real time: a script that waits "20 s" for a Reconnecting… notice can see the whole backoff pass in one frame run. To catch a live reconnect, put a small TCP proxy between the app and the server and stop and start the proxy; a full server restart (about 15 s) outlasts Fleet's five attempts, so it ends on the failed panel in both clients.
- `tap` with a `key` focuses an element (the grip) and `key` events then reach it by name (`Up`, not `ArrowUp`); a script that moves a keyboard-driven grip needs nothing else.

## Asking about text in a file tab's Rendered Markdown (11O)

Same recipe as Changes' Rendered view: a `selection_group(true)` column with `on_selection_change` calling `read_rendered_selection`, and an `on_pointer_move_local` on the scrolling parent that remembers the pointer (a selection change carries no position). The selection group must sit inside the scroll, not around it, or the scroll's own drags are taken for selections.

## Component 16, piece AG (Skills & tools)

- **Rune's `select::content` has no group labels and no rows you can't choose**, so Fleet's "Repository" label over the repositories and "Loading repositories…" in the "Install to" list can't be drawn without changing `ui/select.rs`; the list shows Global and the repositories only.
- **A button that appears (Cancel beside Confirm remove) slides in**, and `tests::click` presses where it is drawn that frame: run a few frames before clicking it, or the press lands off to the side.
- **A flex child that holds wrapping text needs `.w(0.0).grow(1.0).min_w(0.0)`** beside a fixed-width group of buttons; `.shrink()` alone let the buttons run past the card in a narrow window.

## Component 16, piece Y (the Weave group)

- **"Seen" has no equivalent**: Fleet asks when the group scrolls into view (`IntersectionObserver`). Here a per-frame `keep_` hook on the page asks once when the group is first built, and a state with no answer yet draws a spinner group under a different key (`settings-group-weave-loading`), so an anchor to `weave` keeps waiting for the real group.
- **A disabled button is `opacity < 1` in a test**: `ElementInfo` has no disabled flag; `button::disabled` fades it, and the fade takes a few frames to finish after a state change, so a test that checks it came back must run frames first.
- **`role("alert")` on a text element, not its row**: a test that reads the element by key gets the text's role, so put the role on the keyed element.

## Component 16, piece AH (Skills: Catalog and Custom)

- **`El::max_lines(n)` is Rune's `line-clamp`**: wrapped text that ends the last line with "…". Fleet's three-line card description uses it; it is not `.truncate()`, which is one line.
- **A grid row with cards of equal height**: make the row's children `col().w(0.0).grow(1.0)` with the card `.grow(1.0)` inside, and leave the row's `items_start` off (stretch is the default); a lone last card needs an empty `col().w(0.0).grow(1.0)` beside it to keep its half.
- **A `text_input` can be `.disabled(true)` but doesn't dim**; Fleet's disabled input doesn't either, so nothing else is drawn.

## Component 16, piece AD (Plugins)

- A fixed-size element needs no "don't shrink" call: Rune items only shrink where `.shrink()` is asked, so an icon tile or switch beside a `.grow(1.0).shrink()` text column keeps its size.
- A button laid over the end of another (Settings' menu fold button over the Plugins entry) is `absolute(x, 0)` from the parent's left; there is no `right:` offset, so x is worked out from the known width of the panel.
- There is no baseline alignment (`items-baseline`); `items_center` stands in where a heading and a small line share a row.

## Component 16, piece Z (the Weave config dialog)

- **A dialog taller than the window needs its own cap**: `ui::dialog::Dialog.max_height` (90% of the window, from `ui.sel.window`) caps the panel and scrolls it, as `max-h-[90vh] overflow-y-auto` does.
- **What appears inside an open dialog fades in**: a test that presses a button right after the state that shows it finds it at opacity < 1 and the press can land on the scrim, closing the dialog. Run 30 frames after each press before the next.
- **`on_input_key` sees Esc before the dialog does**: returning true from it in the prompt box cancels only the box.

## Component 11, piece P (the editor spike)

Two ways of editing a file in the code view were built and timed on a 20,000-line file (about 400 KB): **A**, the whole file in one
multi-line `text_input` with `.spans(..)` for the colours and a column of numbers beside it (`view::code_input_spike`, scenes and
`--bench-editor` only); **B**, an editor the app draws over the viewer's `virtual_rows` (`fleet::code_edit`, `fleet::code_editing`,
`view::code_edit`), (always on since 11R). B is the way for pieces R to U. What each costs is under "Timings" below.

- **Rune carries an editor the app draws.** `El::on_key` on the focusable scrolling column owns the keyboard while it has focus
  (it sees keys before focus navigation, the default button and the window's `on_key`, and "a click anywhere inside it focuses
  it"); `.ime_cursor()` on the drawn caret turns the input method on and places its window there; `App::on_ime` hears
  `Ime::Preedit` and `Ime::Commit` (used, through `Runtime::ime` in tests and `{"type": "ime", "preedit": …}` / `{"type": "ime",
  "commit": …}` in a script; not tried with a real input method: this machine's Xvfb has none).
- **Where a byte sits is Rune's own measure**: `rune::text::advance(&drawn[..byte], style)` gives the caret's x and, searched, the
  byte under a press, for any text (tabs drawn as spaces, CJK, the mono face at 600) without counting columns.
- **No layout read-back for a wrapped text.** Nothing gives a laid-out text's line starts or a byte's (x, y) (`Cx::caret` is the
  focused *field's* caret only; `line_starts` is crate-private and web-only). A wrapped line's caret row is found by measuring the
  text up to the end of the caret's word with `measure_with(.., Some(width), RichLayoutOptions::new().wrap_anywhere())`; a word
  longer than the row (which breaks inside itself) puts the caret on its last row. The width comes from `on_layout` of the caret's
  line, a frame late.
- **A paste goes to the root's `on_paste` only** (`Runtime::app_paste_handler` reads the root or the open modal): `on_paste` on a
  focused element is never called. The editor reads the clipboard itself on Ctrl/Cmd+V with `tx.read_clipboard`, as the terminal's
  keys do. (The element-level `on_paste` that `view::terminal_view` had was dead for the same reason and is gone; the terminal's paste key reads the clipboard itself and works, tested by `ctrl_shift_v_pastes_into_the_shell_bracketed_only_when_the_program_asked`. Fleet's terminal has no other paste route: no right-click or middle-click paste. Take care never to add a root `on_paste`: `accepts_paste()` then turns true, and Rune takes Ctrl+V for itself before the terminal's `on_key` sees it.)
- **A focusable column draws Rune's keyboard focus ring once keys are used**; the editor turns it off with `focus_ring(0, 0,
  TRANSPARENT)`, as CodeMirror's content element has none.
- **`text_input` is the whole text, every frame.** `text_input(&edit, ..)` clones the `TextEdit` (the text and its undo history)
  into the element on each view, and the field is one text element: no rows are skipped, it always wraps (so its numbers go out of
  step under a wrapped line, scene `editor-spike-input-wrap`), and colours over it are the highlighter's run over the whole new text
  after every key (`rune_kit::syntax::highlight` caches by text, so an edit misses).
- **What B has to build itself** (all plain Rust, none of it needs Rune changed): the selection model and its history (CodeMirror's
  500 ms typing groups), caret motion (words, line ends, the goal column), a blink, drawing a selection past a line's end (a
  `TextHighlight` paints only the text's own bytes, so an empty selected line shows nothing), the language's indent rules (Enter
  copies the line's indent; Fleet's Python adds a level after `:`), bracket matching and closing, multiple and rectangular
  selections, drag and drop of text.

**Timings** (`fleet-native --bench-editor`, release build, 19 November 2026, Linux on a 4-core laptop CPU with no GPU, load average
5 to 12 from other work: the CPU cost of a frame from the key to the end of its layout, before drawing; not real hardware for the
GPU side, which nobody measured). The file is 20,000 lines, 400 KB, Python; one key a frame.

| ms per frame | B p50 | B p95 | B p99 | B wrapped p50 | B wrapped p99 | A p50 | A p99 |
|---|--:|--:|--:|--:|--:|--:|--:|
| type 80 characters | 8.8 | 14.1 | 16.3 | 8.2 | 17.4 | **1,369** | **3,870** |
| Down 300 times | 6.6 | 8.4 | 9.7 | 7.1 | 10.4 | 11.9 | 15.7 |
| Page Down 40 times | 10.5 | 19.1 | 19.1 | 10.6 | 12.6 | 11.8 | 15.8 |
| undo and redo 10 times | 8.9 | 12.9 | 12.9 | 9.1 | 16.1 | **1,417** | **1,880** |
| scroll 10 s | 1.4 | 5.8 | 7.4 | 1.5 | 8.2 | 6.5 | 10.1 |
| open (the first frame) | 42 | | | 50 | | 3,484 | |

A is out: each key in a 400 KB `text_input` costs well over a second (the clone, the whole text laid out again, the whole text
coloured again). B types inside a frame at 60 Hz at the median and touches 16.7 ms at its slowest; a key's cost is the view built
again (the rows on screen are rebuilt on every caret move, since `virtual_rows_version` is one number for the list) plus, on an
edit, colouring the first 128 KiB again, joining the lines and diffing them against the saved text for the stripe. All three are
whole-file work that piece R can make incremental (colour from the edited line, diff only when the stripe shows).

## Component 16, piece AE (Mods runtime panel)

- **Text with a few words in the mono face** (the paths and commands in "Mods need Bun") is `rune::rich(all, vec![Span::new(a, b).mono().size(..).color(..)])`: the spans are byte ranges of the whole string, so build the string and the spans together from `(text, is_code)` parts. A paragraph with an inline *link* can't be one element: the link is its own pressable `row()` in a wrapping row beside the text before and after it, so a line break can fall between them but not inside the link.
- **A test that waits for "the server answered a button"** can't wait for the request alone: the answer is taken on the UI thread after the thread's job is handed back. Wait for the next request the handler makes after it has set the state (here the preferences read that follows every Mods button), or for the state itself.
- **`cargo test` of the lib with two rustc at once peaked at 3.9 GB** and the 4000M cap killed it (exit 143). `cargo test -j1` finished inside it in about 3.5 minutes.
## Component 16, piece AF (GitHub's connection page)

- **A test can't set the app's clock by hand**: `poll` writes `State.clock` from the runtime's time on every frame, so `s.clock = x` is overwritten. Advance time with `rt.frame(seconds)` (a long frame runs every timer that is due, once).
- **A plain `.row()` button in a column stretches to the full width**; `.self_start()` gives Fleet's inline-sized button.
- **`border_sides` is `[top, left, bottom, right]`** (a divider above a row is `[1.0, 0.0, 0.0, 0.0]`).
- **`TextEdit::secure()` fits a token box**: bullets, `[redacted]` in `Debug`; keep it out of a state's own `Debug` anyway.

## Component 16, piece AI (Tools: Installed and Catalog)

- **A cargo test build can pass 4000 M**: with the default job count the test build of this crate was OOM-killed twice in a 4000 M scope (exit 143, "Terminated", no error text; `journalctl --user` says `oom-kill`). `cargo test -j1` fits.
- **A page that mirrors another keeps its own state struct**: Tools has `ToolsPage` beside `SkillsPage`, because Fleet unmounts the Tools section (and its install target) when you leave the tab; the shared picker takes the page by `Kind`.
- **A test that presses two buttons in a row needs a frame between reading**: after `click` on a second card's Remove, `settle()` before reading the first card's label, or it still reads the old one.

## Component 16, piece AK (Built-in skills page)

- **`ElementInfo::text` is the text of a text element only.** A row that holds a text (a pill, a link) answers `""`; give the row `.label(words)` as well and tests read `label`. A text element with its own key can be read with `text`.
- **A tab-sized page with eight cards and a diff** adds nothing to the build cost, but `cargo test` of the lib still peaks near 4000M with the default jobs; `CARGO_BUILD_JOBS=1` fits (as in piece AE).
## Component 16, piece AJ (Tools: Custom)

- **A form's submit button far down the page is missed by a 760-high test window**: `click` hits nothing below the viewport. Open the rig with `app_tall()` (`rig_in`).
- **`El` text read by key still includes children that are fading out** after a switch (the old form's examples); ask `exists` of the key of what should be gone instead.
- **The test build was OOM-killed at 4000M even with `-j1` once the lib grew**; 5200M with `-j1` passed.

## Component 16, piece AM (Mods tab)

- **A flex row with a wrapping text and a button** (Fleet's `flex-wrap` callout) puts the button under the text if the row wraps, because a wrapping text asks for the full width. Drop `flex_wrap` on the row and let the text `.shrink()` so the button stays beside it.
- **A link in a column stretches to the column's width** (its press area too): `.self_start()`.
- **Skills & tools doesn't read the preferences itself** (only General, Labs and Plugins do), so a test of the Mods tab calls `refresh_preferences()` before opening it, as the app does at start.
- **`cargo test` (two rustc at once) was killed at the 4000M cap again**; `cargo test -j1` fits.

## Component 16, piece AL (skill Editor and History dialogs)

- **`TextEdit` is not `Eq`.** A state struct inside `SettingsNav` (which derives `Eq`) that holds one needs `impl Eq for X {}` by hand, as the other pages do.
- **A text field has no length limit** (`maxlength`). The note's 2,000 characters are cut when it is sent.
- **The dialog description didn't wrap**: `ui::dialog` drew it as one line, so a long one ran out of the panel. It now wraps to the panel's width.
- **Disabled controls fade on a spring**: a test that reads `opacity` must run a few dozen frames first (`tests_settings_skill_dialogs::disabled`).
- **A script's `type` event dropped the first five characters** of a text typed into the code box in a live run (the same text sent as one key event in a test arrives whole); I typed a throwaway word first for the picture. Not chased.

### The desktop shell (component 24's plan, 19 November 2026, pin `f1f54ce8`)

- **What the pin has for an app's own shell**, read in its source, not yet used: `rune::single_instance(id)` (Windows named pipe,
  Linux socket in `$XDG_RUNTIME_DIR`; a second launch hands its links to `on_open_url` or reopens the first);
  `App::keep_running`, `on_close`, `on_quit`, `on_reopen`; `rune::tray` (Windows and Linux StatusNotifierItem; not macOS);
  `App::menu_item` / `AppHandle::set_menu` / `standard_menus` (macOS only, `Unsupported` elsewhere); `rune-notify`, a separate
  crate in Rune's repository (macOS for a bundled app, Windows toasts; nothing on Linux).
- **No event says the window moved or was resized.** `WindowHandle::placement()` reads position, size, maximized and the display
  on the UI thread (`None` on Wayland), so saving the window's place means polling it and saving on close and quit.
- **`on_quit` can't cancel a quit.** A system quit (macOS Cmd-Q, logout) can't be held for a "sessions are still working" question;
  only quits the app starts itself can ask first.
- **`Tx::open_url` takes `http`, `https` and `mailto` only**, which is exactly Fleet's desktop link rule; opening a folder (Open logs)
  needs the system's opener on a thread.

## Settings → API keys (component 16, piece AO)

- **A border that changes on hover** can't be drawn with `.border()` plus `.hover()` (hover changes the fill only). Draw the border as an outer box filled with the border colour and 1px of padding, with `.hover(..)` on it, around an inner box with the real fill (`credentials_form::ringed`).
- **A secure `TextEdit` in a list of forms**: keep one per secret in the page's state, replace it with a fresh `TextEdit::new(..).secure()` to wipe it, and derive `PartialEq` for the page (`TextEdit` has it) so `SettingsNav`'s derive still holds.
- **A test build of the crate now peaks near 4.9 GB** (rustc, lib test), above the 4000M cap; run it under `-p MemoryMax=5500M` and give the unit `-p LimitNOFILE=65536`, or fake-server tests fail with "Too many open files".
- **`State` size matters on Windows**: adding the API keys page's five text boxes to `State` made the app overflow the main thread's 1 MB stack at start (CI's web walk, "has overflowed its stack", exit 127) though every test passed. The page is boxed in `SettingsNav`; box the next big page state too.

## Component 24, piece A (the server process)

- Nothing of Rune's is used here; what was learned is about the test setup. A test binary can't be its own fake server: libtest
  refuses an argument it doesn't know (`--urls`), so the process tests compile `examples/fake_fleet.rs` with `rustc` on their
  first start. `cargo test` builds no example for a library crate's unit tests, so there is no binary to find beside the test.
- The lib's test build gets killed ("Terminated", exit 143, no message) under `MemoryMax=4000M`; see the API keys section above for its peak.

## Component 11, piece R (editing in the tab)

- **A caret's blink is the app's.** There is no blink on an `ime_cursor` element: the app draws the caret with a transparent fill in the dark half
  (the element stays, so the input method still follows it), flips it from `poll` (`keep_code_edit_focus`, a `wake_after` of 250 ms while the code has
  the keyboard) and puts the phase in the rows' `virtual_rows_version`, so only the rows near the window are built again.
- **A selection past a line's end** is not a `TextHighlight`'s to paint (it covers the text's own bytes): the line gets one trailing space in
  the drawn text and the highlight runs over it.
- **No hover border** again: Save's border turning to `--accent` is a 1 px `--border` ring with the hover fill `--accent` round the button.

## Component 24, piece B (the start pages)

- `Receiver` isn't `Sync`, so a struct holding one can't go into a posted job inside an `Arc`; hand the UI a small `Clone` handle (`Stopper`) instead.
- A sliding bar inside a fixed box: `row().clip_rounded(r)` with a child `.offset(x, 0.0)` clips as CSS `overflow: hidden` does; drive `x` from `State::clock` and keep the poll asking for frames (`tx.wake_after`) only while it shows.
- Rune can't ask for the system's reduced-motion setting, so the splash always moves. **Out of date (22 Nov 2026, component 26 A):** at the pin `cx.system_reduced_motion()` holds the system's setting (refreshed on window focus) and Rune uses it by default; `cx.reduced_motion(v)` overrides (`crates/rune/README.md`, "Reduced motion"). The splash can hold still.
- The lib's test build peaked near 3.9 GB again and was OOM-killed at 4000M; 6000M worked.
## Component 15, piece A (the side conversation's state and routes)

- **Nothing new from Rune.** The state is plain Rust (`fleet/side.rs`) with the clock passed in, so an Undo window is tested by running the headless clock (`rt.frame(9.0)`), not by waiting.
- **Test fakes that keep state** (a side conversation that a `DELETE` moves to "discarded" and a `POST /restore` brings back) are small enough to write per file over `test_http`; the fake records `(method, route, body)` and a test waits for the call it means before asserting.
- **`cargo test` needs `-j1` at the 4000M cap** (the default two rustc were OOM-killed, exit 143, no message) and takes about 5 minutes for the lib's test build.

## Component 24, piece D (a Fleet that wants its key)

- **`autofocus()` is for modals** (the control a modal focuses when it opens); a page that isn't one asks with `tx.focus("<key>", false)` from `poll`, once, on the second poll after the page was posted (a counter in the state), because the key has to be in the drawn tree.
- **`max-width: 100%` on a fixed width**: `max_w` takes pixels only, so the card is 470 px at any window width. The app's window can't go narrower than that card plus its padding.
- A test that touches the process's one home token (`fleet::auth`) must hold `fleet::tests::auth::HOME_TOKEN`, now `pub(crate)`, even from `desktop`'s tests.
- **Return on a focused button** goes to the window's *default button* (`.default_button()`), not to the focused one; only Space presses the focused button. A page whose one button should answer Return (as an HTML button does) marks it the default.

## Component 11, piece S (conflicts and Compare)

- **`flex-wrap` rows work, but a wrapping text in one needs a width.** A `rich(..).wrap()` child of a `.flex_wrap()` row takes its max-content
  width and is cut at the row's edge instead of wrapping; giving it `.max_w(panel width - padding)` makes it take a line of its own and wrap, and
  the buttons after a `grow(1)` spacer then drop to the next line at the right, as CSS `margin-left: auto` does (`view::file_compare::notice`).
- **No enter animation for an element that is new.** Fleet's notice rises over 240 ms; here a notice is drawn at once (the lines bar's 160 ms
  rise is the same gap). `notice_card` does it with a state machine and springs, too much for a bar.
- **A floating button pocket in a line that scrolls sideways** sits at the end of the line, not of the view, when Wrap is off (Revert had the same
  gap); with Wrap on, the rows are as wide as the view and the pocket is at its right edge as in Fleet.
- **A test build peaks near 3.9 GB and is killed under `MemoryMax=4000M`** (exit 143, no message, journal says `oom-kill`): run it with 5500M, `-j 2`.
- **A scripted live run's clock is not the wall clock**: with frames costing 25 ms of CPU each, 11 s of script took 45 s. To change a file
  underneath a script, send the change from outside well after the script's typing is due (wall about 3 times the script's second) and put the
  step that needs the change late in the script.

## Component 24, piece E (closing, quitting and the tray)

- **`on_close` runs after the window is gone, and can't hold it.** Electron's `close` handler prevents the close, asks the Fleet how many
  sessions are working (up to 2 s) and then hides or quits. With `keep_running`, Rune has closed the window before `on_close`, so the app
  asks and then shows the tray (or quits); for up to two seconds the window is simply not there. Smallest case: `App::new(..).keep_running()
  .on_close(|s, tx| { /* start a thread */ })`: the window is already closed when the handler returns.
- **A `Tx` can't show the window.** `Tx` has `close_window`, `quit` and `toggle_full_screen`, but opening or focusing a closed window is
  `AppHandle::open` only, so the tray's "Show Fleet" (which arrives as a job on the UI thread) goes through a handle kept in a `Surface`
  (`desktop::tray::Real`), not through the job's `Tx`.
- **A system quit can't be held for a question** (written down in the plan, now with its case): `App::on_quit` runs for macOS Cmd-Q, the Dock's
  Quit and a Windows shutdown with no way to refuse. The dialog "A session is still working" covers the quits the app starts (the tray's "Quit
  Fleet", the menu's and the palette's); Cmd-Q stops the Fleet the app started without asking. Smallest case: `.on_quit(|_, tx| tx.focus("x", true))`
  has nothing to return; there is no `-> bool`.
- **`Tray::new` succeeds with no tray host on Linux** (the doc says so: the icon appears once a host starts), so the app can't tell that a
  window closed "to the tray" has nothing to bring it back. It only falls back (keeps the window) when `Tray::new` itself fails, which is no
  session bus. Making the tray also talks to D-Bus, so it runs on a thread of its own.
- **The headless runtime keeps what a handler asked of the shell** in `rt.shell_request` (`rune::Shell`): a test reads `Some(Shell::Quit)` to
  say "the app quit" without a window.
- **The tray icon decodes with the `png` crate Rune already builds** (0.18; a direct dependency adds no crate to `Cargo.lock`). `TrayIcon::rgba`
  wants straight RGBA rows, which `tray.png` (8-bit RGBA, 32 x 32) already is.

## Component 15, piece B (the panel, open)

- **A box that folds away with its content kept mounted:** a column of fixed height `h(card * share)` with `clip_rounded(0.0)`, the card inside it at its full size. Drive `share` from `State::clock` (`ease_out`) and ask for frames with `tx.wake_after` only while it runs; `presented(key)` in a test reads the box's height mid-fold.
- **One stream view, two conversations:** the rows read their session and fold plan through `conv(s, Which)` instead of `s.session()`; the element keys (`timeline`, `stream`, `stream-shell`) and the `memo` keys of the side one are prefixed (a `memo` key is also its root's element key, so a test finds a row by `side-<message id>`), or two rows with one key share a cache entry.
- **`tx.focus` runs from a handler, not from a view,** so "opening puts the caret back in the composer" is in the handler (`side_set_minimized`), and a test reads `tx.focused()` through `rt.update`.
- **The window's size** for a `max-width` media query or a `vh` is `s.ui.sel.window`, set by the frame's `on_layout`; a test makes a runtime of that size with `headless_runtime((w, h), …)`.
- **`State` size again (Windows web walk, 20 November 2026):** `SideView` (a `Session` and a fold plan) added to `State` overflowed the 1 MB main stack at start, as the API keys page did. It is `Box`ed in `State`; Linux and macOS never showed it. Box any field holding a `Session`.

## Component 24, piece G (the window's place, one instance)

- **No event for a move or resize.** An app that wants to remember its window reads `WindowHandle::placement()` itself. An idle Rune app isn't polled, so a window the person moves wakes nothing: the poll must ask `tx.wake_after` for its next look, or a move is only seen when something else draws. Here that is a wake every 250 ms while a window exists (headless runs have no handle and ask for nothing).
- **The displays come only with `run_windows`.** `App::run` gives no list of displays, so the window can't be checked against the screens before it opens. `run_windows_with_args(RunOptions::default(), args, |displays| [WindowSpec::from_app(key, app.maximized(m).window_options(opts))])` does, and `AppHandle`s taken from the `App` before still work. Pass the app's own arguments as the second argument (`App::args` isn't read there); keep `App::run` for headless runs.
- **Wayland: `placement()` is `None`, and nothing else reports the window's size** (`Cx` has no window size). A native app can't keep even the size there. A `WindowHandle::size()` or a size in `placement()` on Wayland would let row 24.58 ("restore the size only") be done.
- **On Linux `placement().restore` is `None` while maximised**, so the normal frame is whatever the app saw last before it grew.
- **`single_instance` works as documented:** the second process exited 0 in 60 to 430 ms with no window, over a runtime folder of its own (`XDG_RUNTIME_DIR`); `FLEET_NATIVE_DATA_DIR` runs skip it by the app's rule. On Xvfb with openbox the first window's raise couldn't be shown (openbox did not act on iconify or key requests from `xdotool` there).
- **Memory for the test build:** `cargo test` of this crate is killed at 4000M with `CARGO_BUILD_JOBS=3`; with 1 it finishes (4 min).

## Component 24, piece F (notifications)

- **`rune-notify` has no Linux side** ("off those, `available()` is false and every call does nothing"), so Linux speaks `org.freedesktop.Notifications`
  itself. Rune's `dbus` feature brings `zbus 6` and `tokio` in already; the app names them as Linux-only direct dependencies with the same features
  (`zbus`: `tokio`, `proxy`; `tokio`: `rt`, `sync`, `macros`), which adds no crate to `Cargo.lock`. A thread with a current-thread runtime owns the bus
  connection; a `ActionInvoked` signal with key `default` is a click on the notification itself.
- **A click carries only strings.** `rune-notify` hands back the notification's id and its `info` pairs; the app puts the tag in `info("tag", ...)` and routes the
  click into the app as a job, like the tray's.
- **No way to ask whether a Linux notification daemon listens** without calling it, so `can_post` is true there and a refused call is logged once.

## Component 11, piece T (closing and quitting with unsaved files)

- **A close can't be cancelled** (read at the pin, `app.rs`): `App::on_close` runs "whenever the window closes … before it goes",
  and `Tx::quit` is the only thing it can do about the app; it has no way to say "stay". Smallest case:
  `.on_close(|_, tx| tx.focus("x", true))` still closes. With `keep_running()` the close hides the window and the app lives on,
  so a guard has to ask *after* the close: `State::quit_app` (where 24 E's quit paths end) calls `ask_before_quitting`, and if it says yes the window is
  shown again (component 24's `Surface::show_window`) with the question over it. A quit that comes from the system (macOS Cmd-Q,
  logout) runs `on_quit` and can't be held at all (the same gap as 24.73), so those go unasked.
- **`ElementInfo` has no `disabled` field**: a test that a disabled button is off presses it and checks that nothing happened.
  (used)
- **A dialog with a state of its own** (`Dialog { dismiss: Some(fn) }`) takes Escape as that function; an alert has no close
  button and ignores a press on the scrim. (used)
- **The test build needs more than 4000M** (5200M here, 20 November 2026): `cargo test` of this crate was OOM-killed at 3.9 G while
  another session's clippy ran.

## Component 15, piece C (the side stream, live)

- **Nothing missing in Rune.** A second conversation is a second `StreamState` in `State` (`SideStreams`), built into a `Session` and a `FoldPlan` the same way `sync_open` builds the open one; the stream view only needed the keys prefixed (piece B).
- **The link listens to a topic on behalf of a component, with its own hand-over:** `Cmd::WatchSide` marks the topic (`Topics::mark_side`) so the read loop sends its snapshot and events to `State::on_side_snapshot` / `on_side_events` instead of dropping them (a watched topic's snapshot was only recorded) or dispatching them to subscribers. A reconnect subscribes every topic again, so the side's new snapshot replaces its stream.
- **The test build now needs more than 4000 M** to link and compile the library's tests (the kernel OOM killer took cargo at 3.9 G peak, 20 November 2026, with no message but exit 143): 5500 M passed on this machine.
- **A fake server that matches `/side` in the path** also catches `/api/sessions/side-1/permissions`; the fake answers `/permissions` first.

## Component 24, piece H (menus)

- **`MenuItem` can't be read back** (its fields are private), so a test can't ask "what does the menu hold". The app keeps its list as plain data and builds the `MenuItem`s from it; the test reads the data. `MenuItem`'s action is a `fn` pointer, so each item calls one shared `State` method instead of capturing.
- **Menus exist on macOS only.** `App::menu_item` on Windows or Linux isn't an error at the call, it leaves an `Unsupported` in `take_menu_errors()`; the app skips adding them there and puts the items in its palette.
- **`registerAccelerator: false`** (show the key, leave it to the page) is `MenuItem::keys_first()`: the focused element and `on_key` see the chord first; the item runs only if nothing took it.
- **Memory for the test build, again (20 November 2026):** `cargo test` was OOM-killed at `MemoryMax=4000M` (peak 3.9G, exit 143) with the default jobs; it finished under 6000M in about 3 minutes.

## Component 18, piece A (the workflows data and the runs store)

- **Nothing about Rune was missing:** the piece draws nothing. What it needed from the app's own plumbing: `State::subscribe` listeners are
  `FnMut(&Topic, &Event) + Send` with no `State`, so a store that must change state follows events through `State::dispatch`, as Memory and
  Mods do. A store per machine is a map by machine key, and every call goes through `link_for_machine(key)`; a machine's hub forwards `sessions`
  events for the live machine only (`link.rs`), so another machine's runs fill from the load and from each change's answer.
- **A test that counts requests counts only its own:** a `Link` made with `start_machine` has no machine or plugin poll, but its hub still
  talks to the fake, so the fake records every route and the test filters by `/api/workflows`.
- **`cargo test` of this crate is killed at 4000M with `CARGO_BUILD_JOBS=3`** (the build is "Terminated" with no error): use
  `CARGO_BUILD_JOBS=1`, and stop your own earlier cargo by its PID first, or the new one waits on the build lock.

## Component 15, piece D (the tab, the peek and Undo)

- **A press is lost when a sibling is inserted before the pressed element's unkeyed ancestor between the pointer's arrival and the press.** The peek came in as the pointer arrived and sat before the tab in the same column, so the tab's wrapper moved one place in the tree, was taken for a new element, and the press that began on it did nothing (live only: a test that moves and presses in one frame passed). The peek now lives in an always-present zero-high anchor, and the tab's wrapper has a key. Keep the position of anything that can be pressed steady when something beside it comes and goes; the unit test that finds it is `a_press_on_the_tab_after_the_peek_came_in_opens_it` (move, run frames, then press).
- **No per-corner paint radius.** `radius` is one value for all four corners. The tab (`10 10 0 0`) is drawn as a taller box with all four rounded and clipped to its top part (`clip_rounded(0.0)` on a box of the visible height).
- **`scale` is uniform.** Fleet's tab enters with `scaleY(.6)`; here it scales both axes from 60 %, from the bottom left (`origin`).
- **`paint_order` lets a tab overlap the element after it.** The tab sits 5 px over the composer's top border; the composer comes later in the column, so the section carries `paint_order(2)`. Negative margin (`mb(-5)`) works as in CSS.
- **`max_lines(n)` is `-webkit-line-clamp`:** the peek's six lines end in an ellipsis at the same word as Fleet's.
- **An animation whose last step is drawn after the poll stops asking for frames stays on its last frame.** The fold's end was one frame short, so a headless picture showed the card's top border at 1 px. Run the clock a little past the end (here 50 ms) so the settled frame is drawn.
- **`Tx::focused()` is enough for `:focus-within` on one element:** the tab's peek shows while the open button has the focus, read each frame (the × inside the tab is not counted).

## Component 24, piece I (the right-click menu on text)

- **Rune already draws a field's menu (Cut, Copy, Paste, Select All) and a selection's Copy, but only through `popup_menu`, which exists on macOS and Windows.** On Linux the request is answered `None` at once and nothing shows; no hook hands the app the request to draw itself (`popup_menu_supported()` is the only sign). So the app draws the menu where that is false.
- **A field with its own `on_secondary_press` loses Rune's menu, and an ancestor's handler never sees a press on a field** (the field claims it: `secondary_target`). The app's handler is therefore set on each field, by one wrapper (`ui::text_menu::text_input`) standing where `rune::text_input` was; the same for selectable text, whose nearest handler up the tree wins over Rune's Copy.
- **A handler's `Tx` is enough to build the menu's actions:** `tx.text_selection()` reads the selected text, `tx.element_at(x, y, ..)` finds the key of the field under the pointer (the handler can't know it), `tx.focus(key)` and `tx.return_focus(key)` aim the keys, and `tx.send_key(KeyEvent)` sends Cut, Copy, Paste and Select All to the focused field as typed. A secondary press does not focus a field.
- **A headless run answers a paste request with nothing** (no one reads the clipboard), so a test can check Cut and Copy (the memory clipboard) but only that a Paste pick closes the menu and leaves the field focused.
- **Links:** `Markdown::on_link_secondary` and `El::on_link_secondary` take the press from the elements around, and `Tx::open_url` refuses anything but `http`, `https` and `mailto` with an `Err`, so the link rule needs no code of ours; `Runtime::take_url_requests` is what a test reads.

## Component 18, piece B (the Workflows page, the off notice and the Library list)

- **No hover border.** Fleet's dashed New workflow and Model roles buttons mix the accent into the border on hover. `El::hover` only
  changes the fill, and `group_color` only the ink, so these buttons change their text colour on hover and keep the border. Smallest case: a
  row with `.border(1.0, c).border_dash(4.0, 3.0)` and nothing that recolours the border under the pointer.
- **A border takes no room unless asked:** a dashed button without `.border_layout()` came out 4 px shorter than Fleet's (the border was drawn
  over the padding). Every bordered row that must match a CSS box wants `.border_layout()`.
- **`ElementInfo` has no `disabled`:** a test reads a dimmed, disabled element through its `opacity` and `help` (the tooltip it was given).
- **A held route in a fake server is how a test sees a request in flight:** the fake in `tests_workflow_runs.rs` answers a route made with
  `hold` only once the test gives it an answer, so "still loading" can be asserted while the request is out.
- **Memory for the test build:** `cargo test` was OOM-killed at `MemoryMax=4000M` again (3.9G peak); 5500M built it in 1m42s.

## Component 11, piece U (completion, brackets, replace)

- **A popup under a drawn caret is `anchored_fit("code-caret", Side::Bottom, Align::Start, 4.0)`** on the element the editor already draws for the input method. It keeps the code's keyboard because nothing in it is focusable; the app takes the arrows, Enter, Tab and Esc itself in the code view's `on_key` before they reach anything. The caret element exists only while the code has focus, so a popup left open when focus goes is shut by the poll that sees the blur.
- **`tx.focus` is queued: `tx.focused()` still says the old element in the poll that asked.** A scene that sets `take_focus` and opens a popup must not let the blur check of that same poll shut it.
- **Debug test builds of this crate no longer fit in 4000M** (the kernel killed the compile at 3.9 GB on 20 November 2026): the test build needs `MemoryMax=5500M`.
- **Windows' main thread has 1 MB of stack and a debug build of the app comes close to it**: the web-view walk's start overflowed it ("thread 'main' has overflowed its stack", exit 127) on one pull request that only grew `State`, and on `main` once. `build.rs` links the binary with `/STACK:8388608` on MSVC targets (8 MB, as macOS and Linux have). Not checked on a Windows machine by hand; the CI run is the check.

## Component 24, piece J (the sweep)

- **A window can be closed without a window manager** by sending it what one sends: a `ClientMessage` `WM_PROTOCOLS` with `WM_DELETE_WINDOW` (a few lines of Python `ctypes` over `libX11`; `xdotool windowclose` destroys the window instead, and `windowquit` needs a manager). Rune treats it as the close button: `on_close` runs and `keep_running` works. That is how the hide-to-tray path is driven on the virtual display.
- **`single_instance` brings a hidden window back.** With the window closed to the tray (the app still running), a second launch exited 0 at once and the first window was on screen again at the place it had been (a window mapped again, not raised from behind).
- **Tray without a host:** `Tray::new` doesn't fail on a session bus with no `StatusNotifierWatcher`: it calls `RegisterStatusNotifierItem` and nothing answers, so there is no icon and the app can't know (24.84). The only way back is a second launch.
- **The Linux splash needs a slow server to be seen:** the released server answers `/readyz` in about 0.6 s, so the splash lasts about one frame. For a picture, wrap the server in a script that sleeps first (`FLEET_NATIVE_SERVER_BIN`).

## Component 18, piece C (a workflow's detail)

- **`flex-wrap` on a row is `.flex_wrap()`, not `.wrap()`:** `.wrap()` is the text's. It still compiles on a row and does nothing, so a strip of
  chips ran off the edge of its column (piece 18C) until each wrapping row said `.flex_wrap()`. A child that grows inside a wrapping row
  (`.min_w(180.0).grow(1.0)`) fills its line, which is Fleet's `grid-template-columns: repeat(auto-fit, minmax(180px, 1fr))`.
## Component 19, piece A (the schedule logic)

- **`regex` has no look-ahead, and `(?-u:\b)` is the ASCII word boundary a JavaScript pattern means.** Fleet's `(?![\w'])` is written `(?-u:\b)(?P<e>)`: the empty named group gives the position to check for a `'` by hand. The group is numbered too, so a port that reads groups by number must leave it out.
- **A debug test build wants `-j1` under `MemoryMax=4000M`:** with the default jobs the kernel killed it at 3.9 GB (the scope reports "Terminated", exit 143, and no compiler error); `cargo test -j1` built in 2m41s inside the cap.

## Component 19, piece B (the automations data and their words)

- **Nothing new about Rune:** the piece is plain Rust. What it learned is about the server: a `POST /api/automations/{id}/run` answers the run at once with `"state":"starting"`, and a run that can't make its worktree ends `failed` with `error` "Couldn't start: Couldn't create the worktree: there's no branch origin/main to start from" (a scratch project has no `origin`), which is a ready-made failed run to draw. `targetType` is `new_session` (not `session`) for a session target.

## Component 19, piece C (the store and the routes)

- **Nothing new about Rune:** the piece is plain Rust over `Link::off_thread`. Two Rust points: a `static` empty store (what `automations_of` hands out before anything asked) needs `Sync`, which a `Vec<Box<dyn FnOnce + Send>>` isn't, so the waiters of a load in flight sit in a `Mutex` (read with `get_mut`, no locking); and the test build needed `-j1` again under `MemoryMax=4000M` (two rustc at about 2 GB each were OOM-killed, the scope reporting "Terminated").

## Component 11, piece V (the sweep)

- **A live walk of editing needs a wait after every step that loads text** (11V). Keys sent to a code view before its file has been read go
  to an empty view or are lost (one run drew the typed line on the wrong row with a stray selection). Leave 2 s after the Go to file
  Enter before the first tap, and take one picture per run: a run that saves changes the scratch repository, so reset it between runs.

## Component 20, planning (Analytics)

- **A chart needs nothing Rune lacks at the pin:** `canvas(Shape)` takes a `Path` with `cubic_to` (Chart.js's tension
  curves), `fill_path` (the area under a line) and `stroke_path` (a dash is its own short stroke with `LineCap::Butt`);
  `on_pointer_move_local` gives the pointer inside an element for a hover that picks the nearest day, `on_hover_change` its
  leaving; `rune::text::measure` sizes the axis labels in the running app (not in a headless test: no fonts there).
- **A scripted tap leaves the rail's tooltip up** in a `--snap`: add `{"type": "move", "x": …, "y": …}` after it to
  move the pointer off before the picture.

## Windows test failure: the first ask for older history (history.rs)

- **The scroll reading `poll` sees after an update from another thread is of the old layout.** `AppHandle::update` (and a test's
  `rt.update`) runs outside any frame; the next frame runs `poll` first and lays out after, so that `poll` still reads what
  `on_scroll_offset` said about the box before the change. Whether that old reading was the empty box or the session left depended on
  whether a frame was drawn while the snapshot was on its way, and the first ask for older history went by it. The reading now carries
  the `State::scroll_layout` the view was built with, and `keep_scroll` skips readings from before the open conversation's first
  snapshot. That works because `on_scroll_offset` is called in every frame whose commit gives the box a new handler, so the first frame
  that draws the new view reports the box even when nothing moved. Smallest case: a scroll box with `on_scroll_offset` writing a
  `Cell`, an `rt.update` that replaces its content, then `rt.frame`: in that frame's `poll` the `Cell` still holds the old content's
  reading.
- **Text heights can differ between systems**, because Rune reads the system's fonts for code and for fallbacks. A test that needs to
  land within some number of pixels of a threshold should leave room: the hub test's conversation overflowed a 760 px window by 99 px
  here, against a 100 px threshold.

## Component 19, piece D (the page and the list)

- **A one-side border is `.border_sides([0.0, 0.0, 1.0, 0.0], c)` with `.border_layout()`:** that is Fleet's `border-bottom: 1px` under a `PageHeader` bar, taking its row in layout.
- **A centred empty state inside a scroll box needs its own `col().grow(1.0).center()`:** the scroll's child is only as tall as its content, so a bare `EmptyState` sat at the top of the list where Fleet's `flex-1` centres it.
- **Nothing else new about Rune.** A debug test build of the whole crate still needs more than `MemoryMax=4000M` once the test modules grow (the kernel killed it at 3.9 GB, exit 143, no compiler message); 5200M let `cargo test` through.

## Component 18, piece D (the Run box)

- **A picker can serve two pages by asking which page is open**, not by being copied: the New session page's folder, base and harness
  menus read "the draft" in a dozen places. `State::run_box_active()` (the Workflows rail is showing) makes `target_key`, `set_folder`,
  `repository_path`, the chosen base and the harness choice answer for the Run box instead, and no menu was written twice. The
  draft's own code (what `settle_choices` rewrites) keeps reading the draft, so a late answer for the New session page can't write
  a workflow's harness into it.
- **A field has no read-only:** Fleet's Run box makes its textarea `readonly` while the run starts; the Rune field is `.disabled(true)`,
  which also stops the caret and dims nothing (the frame is the same).
- **`auto_grow(field, 2, 7)` is two lines of the field's line height, without its padding:** a CSS `rows="2"` box with 14/6 px padding is
  62 px tall, Chrome draws it 66 tall, so the wrapper wants `.min_h(66.0)` to match.
- **A chip that flips something has no chevron, a static one has no hover:** `ui::chip::Chip` got `.flat()` and `.still()` for the
  optional-step and Check with me chips and for "New worktree".
- **A test build that is killed with "Terminated" (exit 143) and no message was OOM-killed:** `journalctl` shows
  `Memory cgroup out of memory: Killed process (rustc)`. `MemoryMax=4000M` and 5000M were both too little for the test build on
  20 November 2026 with two other sessions running; 6000M with `CARGO_BUILD_JOBS=2` built it.

## Component 20, piece A (sections, filters and the data)

- **Nothing new about Rune:** the piece is plain Rust (data, queries, dates and comparisons), tested headless.

## Component 20, piece B (the store and the reads)

- **A numbered read has to settle its batch even when dropped:** `refreshing_all` is `Promise.allSettled`, so an answer that a newer request
  overtook still counts. The test holds one read of a filter change, lets a poll overtake it, then releases it: `refreshing_all` going false
  is the only sign the old answer arrived, so the test asserts the newer numbers survive after waiting for that, with no sleep.
- **A test that counts a fake's requests must wait for them:** `asked` (counted when the UI thread sends) is exact at once; what the fake
  heard is not. Wait on the fake's own counter before asserting on its paths.
- **The test build was OOM-killed again at `MemoryMax=4000M`** (exit 143, `Failed with result 'oom-kill'` in the user journal, 3.9G peak);
  5500M built it in under three minutes.

## Component 21, piece A (the plugin seam)

- **A plugin's views are plain `fn(&State, &Theme) -> El` pointers**, so a registry of them is a `Vec` of `Copy` fn pointers and
  a plugin's own state is a `Box<dyn Any + Send>`: nothing in Rune is needed, and `State` stays `Send`. A handler reaches its
  state with `places.state_mut::<T>(id)`; borrow `s.places` (a field), not `s`, so other fields stay usable beside it.
- **`Icon` has no `Debug`**, so a struct holding `&'static Icon` that sits in a `#[derive(Debug)]` needs a hand-written `Debug`.

## Component 18, piece E (Model roles)

- **One selector menu can serve any trigger by name.** The composer's selector is `Open::Menu(name)` with the rows, the selected id and the
  pick all looked up by `name`; the Model roles dialog and the Run box's Models popover add `wf-model` and `wf-effort`, which list a
  role's models or efforts from `State::role_menu_data`, and the three places that read "the session's" harness and lists
  (`harness_key`, `selector_lists`, `selector_data`) answer for the role while `workflows_nav.roles.menu` is set. Nothing of the rows,
  the filter, the stars or the "N more" cue was written twice.
- **A menu over a dialog must be drawn after the dialog.** `overlays()` draws what `Ui::open` names first and the flag-driven dialogs after
  it, so a menu from `Open::Menu` would sit under the dialog it was opened from. The dialog's own overlay function returns the dialog and then
  its menu, and `Open::Menu("wf-…")` draws nothing by itself while the dialog is the place it opened from.
- **`.offset(x, 0.0)` on an anchored panel moves it after the anchor placed it:** Fleet's `shiftLeft` (the menu slides left to stay 8 px
  inside the dialog) is `offset(-shift_left(trigger_x, clip_right), 0.0)`, with the trigger's x written by its `on_layout` and the
  dialog's right edge known from its width, because it is centred.
- **Escape needs nothing special:** a dialog's panel and a menu's panel are both `.modal(...)`, and Escape goes to the one on top, so the
  three `WorkflowModelRolesEscape` cases pass as written.
- **A test build can be OOM-killed (exit 143, no message) at 4000M:** 5200M built `cargo test --lib --no-run` here on 20 November 2026.

## Component 19, piece E (an automation's header and actions)

- **A pane that swaps content eases, and `elements()` shows both.** After a row press changes which automation is open, the
  old header's note was still in `rt.elements()` for the next frames (the state had already dropped it). A test that asserts
  something is *gone* after a swap draws about 40 frames first; a test that asserts something *is* there needs only the two
  that `Rig::frames` draws. Asserting on the state (`automations_nav.notice`) and on the picture are different checks.
- **A switch with its own off track.** `ui::controls::switch` takes the off colour from the theme's `input`; Automations needs the
  text colour at 22 % (`.automation-page__switch[data-state="unchecked"]`), so `switch_with(t, on, off)` takes it as an argument.
- **A key on a row is not a key on its words.** `rt.elements()` gives a text only for the element that holds it, so a test that
  reads a note's words keys the text (`automation-action-message-text`), not the bordered row around it.
- A `Confirm` whose action needs no dialog-held request (Fleet's `confirmAction` with no `run`) closes on the button and lets the
  page do the rest; `Call::DeleteAutomation` follows `Revert` and `ForgetMachine` in `take_confirm` for that.

## Component 18, piece F (the stepper)

- **A header strip is one child in the sheet's column.** `view::sheet` puts `workflows::stepper::stepper` between the header and the
  banners as `.children(Option<El>)`; with no run it adds nothing, so the conversation keeps its height and nothing moves.
- **A wrapping row with a joint in each item:** Fleet's `<li>` holds the 14 px rule and the pill together so a wrap never strands a
  rule at a line's end; here each item is a small `row()` with the rule first, inside `flex_wrap().gaps(4.0, 0.0)` (row gap first,
  column gap second). A rule can start a wrapped line, as it does in Fleet.
- **A disabled pill is an element with no press handler plus `.disabled(true)`;** `.disabled` is on every `El`, not only on fields.
- **A pill's switch track is drawn, not a `Switch`:** Fleet's `.wf-check` is a 26 × 15 track with an 11 px thumb, unlike `Switch.vue`'s
  28 × 16, so it is a `row().size(26.0, 15.0)` with a thumb margin of 2 (off) or 13 (on).
- **A mark above a bubble needs state, a bubble does not have it:** `message::marks` takes the label from the caller
  (`State::workflow_mark`), computed only while a run holds the open session, so the stream does no run lookups for sessions that
  are not steps.
- **A scripted live picture leaves the server changed.** The snap ends the process at its mark, so a run's switch flipped by the
  script stays flipped for the next picture: set it back through the API (`PUT …/check-with-me`) before each one.
- **A test build can be OOM-killed (exit 143) at 4000M** again here: 5600M built `cargo test --no-run` in 1 m 40 s.

## Component 21, piece C (GitHub's stores)

- **A `Send + Sync` object can't reach `State`.** Core's `RepositorySource` is shared (`Arc<dyn …>`) and has `&self` methods, so it
  can't start a request through `State::feed_call`. The repository cache is an `Arc<Mutex<…>>` that the plugin's state and the source
  both hold; the source only sets a `wanted` flag and the plugin's `keep` (run from `poll`) asks on the next frame. Opening the Clone
  view therefore costs one frame before the request leaves.
- **A test for an answer dropped after `clear` takes the job off the queue itself.** `test_wait::drive` stops as soon as its condition
  holds, so a late answer that must change nothing is never seen being dropped; the test receives the job from the `Link` queue
  (`test_wait::recv`), applies it, and then looks at the state.
- **`rt.frame(dt)` moves the app's clock by `dt`**, which is how a test passes 60 s or 15 minutes without waiting; the stores' TTLs read
  `State::clock`, not the wall clock (the wall clock is kept beside it only to say "updated 3m ago").


## Component 18, piece G (the run card)

- **`flex: 1 1 240px` has no spelling.** `.grow(g)` sets the flex basis to 0, so `.w(240.0).grow(1.0)` in a wrapping row packs every
  item onto one line and squeezes them. `.min_w(240.0).grow(1.0)` does what the CSS does: the floor is what the line break counts,
  then the item takes the free space of its line.
- **`margin-left: auto` has no spelling either.** An empty `spacer()` at the end of a wrapping row joins the line before it and takes
  space from its neighbour; the button ends up alone on the next line, on the left. A growing box with `.min_w(<width of the
  button>).justify_end()` around the button wraps as the button would and puts it at the end of its own line. The width is an
  estimate when the label comes from the server (a step's name).
- **A field's `:focus` outline shows for any focus; `.focus_ring` only for the keyboard's.** For a text field in a frame, put
  `.focus_within_ring(2.0, -1.0, accent)` on the frame (the Run box does the same) and keep the field itself bare.
- **A field can only take the keyboard once its key is in the drawn tree.** A press that opens it (a "Send back" choice) sets a counter;
  `poll` moves it on once per call, and the second call asks for `tx.focus`. The same shape as the start page's `autofocus`.
- **State that belongs to "this wait" is dropped from `poll`, not from the view:** the card keeps `owner` (session, step id) and
  `keep_workflow_card` starts it again when the wait it is drawn for changes, as the component's `watch` does. Answers that come back
  for an owner that has gone are ignored, so a late reply never fills the next wait's card.
- **A scripted live picture leaves the server as it was after the click.** A script that presses a choice moves the run on, so a
  second theme has to open the session without pressing again.
- **The scripted model writes `$HOME` into its answers** (a path in a "show-page" reply, the file written by `write-file`). It also
  answers `show-page` to any prompt that contains that word, such as a step's file name; give the run a request that avoids it, and
  start the model with a scratch `HOME`.
- **A scratch Fleet's commit of a step's files names the user.** With no git identity in the scratch home the server's error is
  "unable to auto-detect email address (got '<user>@<host>.(none)')", which the card then shows. Put a `.gitconfig` with an invented
  name in the scratch home before running a workflow.

## Component 19, piece F (the runs list)

- **Sibling keys must be unique even for text.** A run's notes were one `text().key("automation-run-note")` each under the same
  column; Rune renamed the later ones (`#dupN`) and `rt.elements()` then left them out of a test's read. Each note sits in its own
  keyed row (`automation-run-<id>-note-<n>`) with the text keeping the shared key, which `texts_of` reads.
- **A poll from the app's tick needs a clock the test can move.** The runs poll reads `view::message::now_ms` (`State::now_ms` when a
  test fixes it), so a test sets `now_ms` to just before and just at the due time and counts the requests the fake heard. To know an
  answer has landed (and the next read is scheduled from the clock as it stood), wait on the state the answer sets (`next_poll`), not on
  the request having been heard: the request is heard before the answer is applied, and moving the clock in between changes the schedule.
- **Dashed card = `border(1.0, c).border_dash(4.0, 3.0).border_layout()`** (the Workflows list's dashed buttons do the same); Fleet's
  `EmptyState bordered` is that around the title and one line.
- **The theme toggle flips on every run of a script.** With `FLEET_NATIVE_DATA_DIR` set, `cmd+shift+l` in the script saves the new
  theme, so a second run with the same folder and the same toggle draws dark again. After the first light run, leave the toggle out.

## Component 21, piece D (the GitHub rail panel)

- **Nothing in Rune stopped this piece.** A row with a button revealed on its hover is the session list's pattern: `hover_group()` on a shell,
  `group_opacity(0.0, 1.0)` on the child, and `group_color(TRANSPARENT)` on the text that gives way to it (the counts).
- **A bottom-only border does not exist**: `border` draws all four sides, so Fleet's `border-bottom` on the account row is a 1 px `el()` under it.
- **A `hover` colour is drawn over the row's `fill`, not instead of it**: a lit row (9 % wash) with the same colour as its hover came out twice as dark under the pointer, so the lit row's hover is transparent.
- **The cargo test build outgrew 5500M** (an OOM kill at 5.3 G peak, 20 Nov 2026); `MemoryMax=7000M` with `CARGO_BUILD_JOBS=1` passed.

## Component 19, piece G (the built-in Dreaming automation)

- **A disabled button shows only in `opacity`.** `ui::button::disabled` sets `opacity(0.5)` and `disabled(true)`, and `ElementInfo` has no
  `disabled` field, so a test reads `opacity < 1.0` on the keyed element.
- **No time field** (as Settings' nightly time): the built-in page's Change time is a text box taking `HH:MM`, parsed by
  `dream_nightly::parse_time`; a time that isn't one is the error note "Use the 24-hour form, like 03:30." and nothing is sent. Fleet's
  `<input type=time>` is about 110 px wide with a clock glyph; ours is 63 px.


## Component 21, piece E (Follow a repository)

- **A plugin's dialog needs its own place in core**, because Rune's modal layer is built where core draws its overlays (`ui::overlays`). `Entry.overlay` (`src/fleet/places.rs`) is that place: a function from the state to the dialog's elements, empty while the plugin has none open. The dialog keeps its own `open` flag in the plugin's state; it is not `ui.open`, which is core's one-menu-at-a-time enum.
- **Fleet's `Command` filters by `contains` and its empty words need a typed search** (`Command.vue`, `CommandEmpty.vue`): no fuzzy match, no ranking, and a loading or empty list is blank until you type. Ported as is.
- **cmdk lights no row on opening, and the first on typing** (read from Fleet's client): so the list's lit row is an `Option`. The command palette's list lights the first from the start; it was ported that way before this was looked at.
- **A fresh `cargo test` build at `CARGO_BUILD_JOBS=2` was killed at 5500M** (5.3 G peak, 21 Nov 2026); `CARGO_BUILD_JOBS=1` fits in 5500M and took 3 min 30 s.

## Component 20, piece C (the page and the menu)

- **Light scenes can draw dark headless.** A `-light` scene snapped in a fresh data folder came out dark here; the light twin that
  works is the dark scene with the theme key (`{"type":"key","name":"l","cmd":true,"shift":true}`) in `--script`, `--at 0.6`, and a
  fresh `FLEET_NATIVE_DATA_DIR` for each run (the toggle saves the theme there).
- **No `aria-current` found:** I found no way in Rune at the pin to mark the open item of a menu to a screen reader; the native menu keys each item
  (`analytics-section-<id>`) and a test reads the open one from `State`.
- **A test build was OOM-killed again (exit 143) at 5500M with `CARGO_BUILD_JOBS=2`:** 7000M with one job built and ran it.

## Component 18, piece H (the finish bar)

- **A text field's `.disabled(true)` dims it but leaves it `focusable`.** The test reads `opacity < 1.0` for the field, and `opacity < 1.0
  && !focusable` for a button. Put `.disabled` on the field itself (the element that holds the key), not on its frame.
- **A button with something after its label** (`Move on to Plan →`): `button_lead` only puts an element in front. Chain `.child(icon)`
  onto the returned row and set `.px(..)` to the padding `has-[>svg]` gives.
- **An underlined word is a `Span` with `underline` set**, drawn through `rich(..)`; a plain `text` has no underline.
- **A test build of this crate needs about 5.3 GB of memory in its peak** (20 and 21 November 2026): it was killed at 4000M and at 5500M
  and passed at 7500M with `CARGO_BUILD_JOBS=1`.

## Component 18, piece I (runs in Sessions and on the dashboard)

- **Nothing new about Rune.** A group is plain columns of the existing session row: the row took two more `Slots` fields (`label`, `step`)
  instead of a second row view. A scene's mock sessions need an id (`mock::machines::listed`), or the runs store can't find them.
- **The scratch Fleet's `Workflows` switch is the preference `Workflows`**, not `features.workflows.enabled`; with the wrong key the
  routes answer 404. v0.72.0 has no workflow routes at all: use the v0.74.0 package.

## Component 21, piece F (an item's row and its session chip)

- **A row whose right side swaps on hover is two elements stacked, not a grid cell**: the signals stay in the row (they give it its width, with a `min_w` for the actions) and the actions are pinned over them (`pin(None, Some(0), Some(10), Some(0))` plus `center_y`), each with `group_opacity` against the shell's `hover_group`. The shell, not the row, holds the 5 % wash, so it stays while the pointer is over the actions (a pinned sibling is not inside the row). `hover_group` follows keyboard focus too, so Tab reaches the buttons as `:focus-within` does.
- **A link is a button**: a Rune element has no anchor, so "Open on GitHub" calls `tx.open_url`. Fleet's `@media (hover: none)` rule (touch has no hover, so the actions are hidden) has no counterpart on desktop.
- **A press on a child inside a pressable row goes to the child only** (the label chip and the session chip are inside the row that opens the item): the row's own handler is not called, which is Fleet's `@click.stop`.
- **No `clip()`**: a row that must cut its content short uses `clip_rounded(0.0)`.
- **A cargo test build at `CARGO_BUILD_JOBS=1` and `MemoryMax=7000M`** took over ten minutes of wall time on this machine; `2` and 5500M is killed at the test link.

## Component 18, piece J (slice 1 sweep)

- **A row with text that can't shrink runs over its neighbours.** A `text` in a flex row has no `min-width: 0`: give it
  `.truncate().shrink().min_w(0.0)` or it overflows (the stepper's branch ran over the title and the switch).
- **A script scrolls with `move` then `wheel` and a negative `dy`** (`dy: -500` scrolls down); a `wheel` with no `move` before it did
  nothing, and a tap on a key the scroll has put below the window fails with "no element with key".
- **One `--video` run gives every state of a long live walk**: a `--snap` runs the whole script again (and starts another run on
  the server), so the walk is recorded once and `ffmpeg -ss <mark time> -frames:v 1` takes each state.

## Component 20, piece D (the filters)

- **No date field in Rune**, so the two dates are plain text fields (`MM/DD/YYYY`). Rune's `TextEdit` has no blur callback; `focused` is a
  field the runtime sets, so `Fields::step` (run by the page's poll) reads it: a typed whole date or an emptied field is sent, a half-typed field
  goes back when it loses focus.
- **`Open::Select(name)` falls to the gallery's list for a name `ui::overlays` doesn't match** (a stray model list drew in the corner); a new
  select needs its own arm there, not only a list of its own.
- **`border_sides` takes no layout space** unless `border_layout()` follows it, so a rule above a block with padding needs both.

## Component 19, piece K (where it runs)

- **`margin-left: auto` in a wrapping row is not `spacer()`.** With a `spacer()` before the last chip of a `flex_wrap()` row, the
  chip went onto a line of its own, at that line's start, though it fit on the first line. A `row().grow(1.0).justify_end()`
  around the chip fits it on the line when there is room and puts it at the right end of the next line when there isn't, as
  Fleet's `.automation-composer__strip-end` does.
- **`ElementInfo` has no `checked` or `placeholder`** at this pin: a test reads a checkbox's or a placeholder's value from the
  state, not from `elements()`.

## Component 19, piece J (the When menu)

- **A popover that scrolls is `pop(..).scroll(ScrollSpec { stick_bottom: false }).max_h(h)` on the panel itself**, padding and all; with
  `anchored_fit` it is also never taller than the room above the chip. The window's height for a `70vh` cap is `s.ui.sel.window.1`
  (fall back to the fixed cap while it is 0).
- **A field's blur, again** (see piece D of component 20): the cron box applies on leaving it, so the page's poll keeps "has had the
  keys since the menu opened" and acts when `TextEdit::focused` goes false while the menu is still open. A press on a row runs
  before the poll, so a row chosen while the cron box had the keys wins and the cron is dropped, as the menu has closed.
- **A test that closes a menu and opens it again at once finds the old panel's children by key** while it fades out (the note
  at component 16, piece AJ): an error from before is still "there". Let the close finish (about 30 frames) first.

## Component 19, piece I (the automation composer)

- **A `Link` in `rich` text is its own element in `elements()`**: role `link`, keyed by the link's id, its text the linked words, a
  Tab stop. So a test presses it with `click(rt, id)`, and the id should be the test key the view promises (`automation-plan-ask`,
  Fleet's testid), not an action name. The text's own `on_link` gets the id.
- **A `row().role("button").on_press(..)` is not a Tab stop** (`ElementInfo.focusable` false); `.focusable(true)` makes it one,
  as Fleet's `<button>` is. Leave it off a disabled one.
- **`tx.focus(key, false)` from the press that makes the field appear works**: New automation switches the page to create mode and
  focuses the box in the same handler; the box has the focus on the next frame.
- **A field that is also state elsewhere**: the composer's text lives in `ComposerState::text` (H's rules read it) and the box needs a
  `TextEdit`. Keep both and copy in `on_input_change`; anything that sets the text from outside (a scene, a test, a read) goes
  through one helper that sets both (`replace_text` keeps the focus).

## Component 20, piece E (the calendar and Export CSV)

- A popover with a scrim: `.modal(true, close)` on the panel makes Escape close it, and a keyed full-window scrim under it closes on a press outside; `ui::calendar::popover` does both and anchors under a glyph with `anchored`. A test closes it by pressing the scrim's key.
- A save dialog answers after the thread's fetch: `tx.save_file` is called from the job the thread posts, and the test drives it with `rt.file_dialog()` then `rt.resolve_file_dialog(Some(path))`; wait for the dialog (jobs applied and a frame between looks) before asking for it.
- The whole test binary needs more than 5 GB of memory to compile on this machine now (5.3 GB peak, once past 4000M's cap at 5500M): a cap of 7500M with `CARGO_BUILD_JOBS=1` built it.


## Component 21, piece G (Your work)

- **A page that scrolls inside a bordered sheet** is the sheet (`view::workflows::sheet`) holding a column with the padding and `.scroll(ScrollSpec { stick_bottom: false })`, the content column inside it capped with `max_w(1080.0).self_center()`. A `vh` margin is the window's height (`s.ui.sel.window.1`, kept by the root's `on_layout`) times the share.
- **No underline on a text link's hover:** `group_color` changes the colour, there is no hover underline on a plain `text`, so `.gh-home__link` (muted, text colour and an underline on hover) gets the colour only.
- **A Rune line of text has no `line-height: 1.5` by default**: the row's lines were set at 1.2 and 1.3 and came out 5 px short of Fleet's 55.5 px row; set `sans(size, weight, 1.5)` where the CSS inherits the page's line height.


## Component 21, piece H (search, items, metadata)

- **A debounce is a due time on the app's clock, sent by `keep`**, not a timer: the store sets `due = clock + 0.25`, and the plugin's poll sends it when `now` has passed. A test steps the pause with `rt.frame(secs)` and needs no sleep. Showing "loading" during the pause is a flag set when the query is typed.
- **Dropping an answer** is a generation per request (Fleet aborts the fetch): the thread's answer is applied only when its generation is still the store's. A test that proves a drop gates the older request on the fake, lets the newer one land, then releases the gate and looks again after a few frames.

## Component 20, piece F (Overview: the cards, the panels and the cost bars)

- **`auto-fit` grids are `flex_wrap` rows of `min_w(n).grow(1.0)` children**, as in the workflow facts: four cards across at 1280 and two panels come out the same as Fleet's. A last short row stretches instead of keeping the columns.
- **A bar fill is a `grow` share**: a row with a clipped round track holding `el().grow(w).w(0.0)` and `el().grow(100 - w).w(0.0)` draws `width: w%` with no pixel width needed (the track's `clip_rounded` makes the pill). It doesn't ease; a 300 ms transition would need the canvas stroke with `live_trim`.
- **No baseline alignment**: a 24 px value with a 12 px "Est." beside it is `items_end` with the small text raised 3 px (`mb(3.0)`); wrapped under the value it sits 3 px low. `gaps(row, column)` gives CSS `gap: 4px 8px`.
- **A `scroll()` box's own bottom padding isn't part of its range**: a page column with `padding(24.0)` that is the scroll box scrolled to its end with the last panel flush to the bottom edge. Padding left and top on it and a trailing child for the bottom (`el().h(4.0)` after a 20 px gap) gave Fleet's 24 px below the last panel.
- **A scene that wants a state the first look would overwrite** (loading, failed, empty) can't set the reads itself: the page mounts on its first poll and asks again, and with no server the invented answers fill in. `Analytics::mock_answers` (`Data`, `Silent`, `Failed`, `Empty`) says what those invented answers are.
- **Fleet's states by request routing**: Playwright's `page.route('**/api/analytics/**', …)` holds the answers (loading), refuses them with 503 (error) or sends empty bodies (empty), so Fleet's own client draws states the scratch Fleet can't make.


## Component 21, piece I (a repository's page)

- **A page that watches a query asks from `keep`, not from the view:** a view can't change the state, so the plugin's poll computes the query the tab, filter and typed text make and hands it to the search each tick; the same query again does nothing. A test types into the state, steps `DEBOUNCE` with `rt.frame` and waits for the request.
- **A tab strip with a line under the selected tab** is a row with a 1 px bottom border whose tabs have a transparent 2 px bottom border (accent when selected) and `mb(-1.0)`, as `ui::controls`' underline tabs do. A pressed state is `.checked(on)`; Rune has no `.selected`.
- **A wrapping row of chips** is `flex_wrap()` (there is no `wrap()` on a row); the box that takes the rest of the line is `grow(1.0).min_w(220.0)`.

## Component 23, piece A (the Labs frame)

- A page that is several sibling panes (`[panel][gutter][page]`, or the notice alone) is a `Vec<El>` from the view; `view.rs` puts `ml(8.0)` on the first and appends the rest, as Workflows and Analytics do. A fn pointer can't capture, so a shared frame that takes a Turn on handler per feature matches on the feature to give each its own non-capturing closure.
- A test fake sees requests the app makes for its own sake (the events hub negotiate, the update check). Assert on the prefixes you mean (`/api/jobs`, `/api/boards`, `/api/sessions`), never on "nothing but the preferences".
- The test binary was OOM-killed at 4000M and again at 5500M here; 7000M with `CARGO_BUILD_JOBS=2` built and ran it.

## Component 22, piece B: a mod's tree (Box, Text, Pill, Icon)

- **`grow` shares the room, not the extras.** CSS's `flex-grow: 1` on Boxes of different content widths (`flex: 1 1 auto`) gives each its
  content plus an equal share of what is left; `.grow(1.0)` sets the basis to 0, so each gets an equal share of the whole. Four
  `flexGrow` Boxes side by side come out equal in width here and proportional to their words in Fleet. No fix: Rune has no `basis`.
- **No baseline alignment.** `alignItems: "baseline"` draws as `start` (a row of different sizes lines up at its top).
- **A nested `Text` is spans of one text element**, not nested elements: `Span` takes weight, mono, size, colour and italic, and
  strike-through is `El::strike(start, end, None)` (byte offsets). The words of a Text and its nested Texts flow and wrap as one paragraph,
  as inline spans do.
- **`inline-flex` is `.self_start()`**, and a lone Pill or Icon at a block root needs `items_start` on its parent; a stretched `col()` makes
  it as wide as the room, where a browser keeps an inline-flex item at its content.
- **An icon in a row that stretches its items** (CSS: `inline-flex; align-items: center`, so the 14 px glyph sits in the middle of a
  taller line) is the glyph inside a `row().center()` wrapper; a bare `icon()` would stay at the top.
- **A pill that can't squeeze:** `row().min_w(0.0).shrink()` with a `.truncate()` label cuts the label to the room and keeps the pill 22 px
  high; a wrapping row (`flex_wrap`) moves a whole pill to the next line first, as `flex-wrap: wrap` does.
- **`border_sides([0, 0, 0, 3], c)` with `border_layout()`** is a 3 px left-only quote bar that takes its 3 px of room; the order is
  top, right, bottom, left.
- **The loader spins in Fleet** and fades here (no looping rotation, see above); the `prefers-reduced-motion` 3 s turn has no twin. **Out of date (22 Nov 2026):** `.looping(LoopSpec::rotation(period))` turns an element forever and holds `still` under reduced motion (`crates/rune/README.md`, "Periodic motion"); the spinners can turn.
- **`Runtime::accessibility_json()`** is how a test reads a label or role off an element: the roles and labels of an `El` aren't readable from
  outside Rune.

## Component 23, piece B (mission control's data)

- Plain-Rust data pieces need no Rune at all: build sessions in tests from list-row JSON through `parse_sessions_for(.., Retention::All)` (it keeps archived rows, drops hidden ones) instead of constructing `mock::Session`, so a test names fields as Fleet's own tests do.
- A request that has to go to any machine is kept as data (`Request { method, path, body }`) and sent with `Request::send(base)` on a thread; the address is chosen by the caller from `Machines::key_of_machine` and the link, so the data file never touches `State`.


## Component 21, piece J (the issue filter bar)

- **There is no blur event.** A field that commits when it loses focus (`FilterExpressionField`) is watched from the plugin's `keep`: `tx.focused()` there says whether the field still has focus, so focus starts an edit and its loss commits it. Enter and Escape are `on_input_key` handlers that call `tx.blur()`. A control pressed meanwhile has to commit the edit first (the press lands before the next poll).
- **A field that shows state until it is edited** keeps its `TextEdit` in the state and the poll writes the serialised value into it while no edit is under way; a view can't set a `TextEdit`.
- **A popover with a search box** is a plain panel on `anchored` + `scrim` + `.modal(true, close)` with the plugin's own close; `ui::menu::dropdown` and `ui::dialog::popover` close through `ui.open`, which a plugin's menu mustn't use (core would draw an unknown name).
- **`on_hover_change` on a list reports leaving it**, so "the pointer left, nothing is highlighted" works for a list the pointer has been over; it isn't called for a pointer that was never inside.
- **Fleet's client can draw something other than its CSS says**: `.filter-popover-content` asks for 256 px and no padding, the client draws 252 and 14 px of padding (`w-72`, `p-4` win), the `Button`'s icons draw 14 px whatever `:size` says. Measure the live client (`getBoundingClientRect`) before trusting a component's own style block.


## Component 20, piece G (the chart)

- **A chart is a canvas and absolutely placed text.** `canvas(Shape)` takes `fill` (the area under the line, a hovered point as a 24-gon, the tooltip's rounded `Path` with its caret in the outline), `line` and `stroke_with(.., LineCap::Butt, ..)`. There is no dash: a dashed line is its own short strokes, cut by arc length from a polyline that the code flattens itself (`fleet::analytics::chart::dashes`). `fill` wants a simple polygon, so the area under a curve is the curve's points, then the two bottom corners.
- **`on_pointer_move_local` reaches every element under the pointer that has it, not only the top one**, so one on the chart's root hears moves over the canvas, the labels and the legend alike; the decoration layers are `inert()` so they never take a press. A canvas as big as the root that draws the tooltip sits above the legend's press targets only if it is `inert`.
- **`on_layout` gives the width the chart is laid out to**; the first frame has none (0), so the root draws empty until the second. The state holds it, and a scene's picture taken at `--at 0.3` has it.
- **`rune::text::measure` for layout, a fixed-width stand-in for tests.** Chart.js's layout depends on label widths, so the geometry takes a `Measure` closure; the headless test passes 0.55 of the size per character and the numbers in `analytics_chart_cases.json` were made by Chart.js 4.5 with `measureText` replaced the same way, so the test holds the geometry against Chart.js's exactly, with no font involved.
- **`Family::Named("Helvetica Neue, Helvetica, Arial")` falls back to the sans slot** where none of them is installed (this Linux machine): the text is Inter, wider than Chart.js's Helvetica, and the area comes out 2 px narrower on each side than Fleet's. Chart.js text isn't Fleet's font, which is why it is named.
- **Playwright's `page.clock.setFixedTime` freezes Chart.js's entry animation at its first frame** (the lines sit flat at zero): use `page.clock.install({ time })` then `page.clock.resume()` to have a fixed day and a clock that runs.

## Component 23, piece C (mission control kept live)

- **A `--script` run's clock isn't the wall clock.** Frames under `--snap`/`--script` draw slowly on a software renderer, and the app's clock follows frames, so a 300 ms fold showed as 2.5 s between a log's event and read lines. Check timing in a real window under Xvfb (`xvfb-run`), where the same fold measured 301-314 ms.
- A timer-driven store keeps deadlines in app seconds and calls `tx.wake_after(next deadline)` from its `keep_*`; jobs from threads that change what is due call `tx.wake_after(20 ms)` so the next frame notices.
- Tests for such a store move the clock with `rt.frame(secs)` and drain jobs with `frame(0.0)` between looks (as the Analytics tests do); the hub of a fake that isn't a SignalR server fails, which is how a test gets "polling".

## Component 20, piece H (Daily and Projects)

- **A table is its columns side by side**: five `col().grow(1.0)`s in a `row`, so a column is as wide as its widest cell and the rows line up. Putting that row inside a `scroll_x()` box drew the columns at the width of their first cell, overlapped (the box measures its child unconstrained); the sideways scroll is left out until a smaller case is written.
- **A card pinned to its row's bottom** (`margin-top: auto`) is a `grow(1.0).min_h(gap)` spacer between the parts, with no `gap` on the card, or the gap is paid twice.

## Component 22, piece C: a mod's controls, Markdown and Code

- **No percentage maximum**: `w_frac` sets a share, but nothing says "at most half of the parent" (`max-width: 50%`). An Input's submit button
  shrinks with its field instead, so a very long `submitLabel` takes more than half the row. A control that needs `min(10em, 100%)` has only the 10 em.
- **A `Markdown` can't drop images or dead links**: with no `images` handler an image draws Chromium's broken-image glyph and its alt text; Rune has
  no hook that turns a link into plain text. A mod's Markdown is therefore rewritten as source first (pulldown-cmark's offset events: replace each
  image by its alt text, strip the marks of each link that isn't `http(s):` or `mailto:`, escape raw HTML) and drawn as the conversation is.
- **`MarkdownStyle` has no heading colour or rule**: Fleet's `--md-heading` (62% accent) and the 1 px rule under h1 and h2 can't be drawn, in a message or
  in a mod's Markdown. Only the fonts are set (`heading_font`), so headings are in the text colour with no rule.
- **A column doesn't shrink its children's widths**: a fixed-width item in a column overflows a narrow parent; put it in a row of its own (main axis) to let
  `shrink` work. The Select's field is built so.
- **`text_input(...).on_edit` takes a capturing closure**, so one `TextEdit` per element key can live in a map; return a spare edit when the key is gone, as
  a handler must always hand back a `&mut TextEdit`.
- **`ElementInfo` is enough to test controls**: `fill`, `opacity`, `text_color`, `label`, `role` and the rect read size, tone and state of a laid-out control.


## Component 20, piece I (Models)

- **`scroll_x()` does work around a table once the inner row has `min_w(720.0).grow(1.0)`** (not `w_full()`): at 1280 the columns fill the panel, at 900 the box clips at its edge and scrolls. With `w_full()` the box measured the row at its content width and the columns stopped short of the panel's right edge. (This is the smaller case piece H's note asked for.)
- **A row's hover tint across separate column boxes**: give every cell of a row `hover_group_shared("<table>-row-<n>")` and a child `el().pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).fill(tint).group_opacity(0.0, 1.0)`; the pointer over any cell tints the whole row.
- **A wrapped cell breaks a column-built table**: each column is its own box, so a cell that wraps makes its row taller in that column only. The model and provider cells stay on one line (Fleet wraps them anywhere when the table is narrower than its content).
- **`cargo test`'s binary isn't `cargo build`'s**: a scene snapshot after only `cargo test` draws the old binary ("no scene ..."); run `cargo build` too.

## Component 21, piece K (an item's page)

- **`border_sides` is top, right, bottom, left**, as CSS writes it: `[1, 0, 0, 0]` is a rule above, not at the left. A timeline's left rule is `[0, 0, 0, 1]`; the first try drew a line under the description instead.
- **`Open::Menu(name)` draws core's menu by name** (`ui::menus::dropdown`), and any name it doesn't know draws the gallery's menu in the corner. A plugin's own menu keeps its open state in its own state and draws `menu::panel` on `anchored` + `scrim` + `.modal(true, close)`, as the issue bar's sort menu does.
- **An absolute child at a negative left** (`el().absolute(-24.0, 14.0)`) draws outside its parent and keeps the parent's size, which is how the conversation's dots (`::before` at `left: -24px`) sit on the rule beside it.
- **`rich(text, spans)` is the by-line**: "{author} wants to merge" with the author in 500 is one wrapping text with a span; the branch chips beside it are rows with their own padding, so the sentence is a wrapping row of the text and two chips.
- **A scroll column's `grow` child** (the body row under the header) takes the rest of the column when the content is short, so the rail's wash reaches the page's bottom as `flex: 1` does.

## Component 23, piece D (the fleet-menu and Needs you)

- **One border colour per element, and `border_sides` replaces `border`.** Fleet's card (`border` on three sides and a 4 px `border-l-4 border-l-accent`) is a row with `border_sides([1, 1, 1, 0], border)`, `clip_rounded`, and a 4 px accent element as its first child. The order of `border_sides` is CSS's: top, right, bottom, left.
- **`text(..).truncate()` is `text-overflow: ellipsis`** (there is no `.ellipsis()`); give the text `.grow(1.0).shrink().min_w(0.0)` so the count beside it keeps its place.
- A test can tell a Fleet menu item's open state only by its `fill` (`ElementInfo.fill.a`): Rune has no `aria-current`.
- **A test binary of this crate needs about 5 GB to link** (`MemoryMax=7000M`, `-j2`); 4000M is OOM-killed with exit 143 and no message when other sessions are using the machine.

## Component 20, piece J (Sessions)

- **A table whose cells wrap is built as rows, not columns**: each row is a `row()` of cells with `.w(basis).grow(basis).shrink().min_w(0.0)`, so every row shares its columns and the tallest cell sets the row's height (the column-built table of piece I can't do that). The 3 % hover tint is then just `row.hover(color)`.
- **`text(..).group_color(c)` with `hover_group()` on the button** turns a head's label and icon `--text` together under the pointer.
- **A scene can't set state the page's mount resets** (the sort, the filters): the first look remounts. `show_analytics_sorted` marks the page mounted first.

## Component 21, piece L (an item's side rail)

- **A container query is the page's own `on_layout` width.** The column that holds header and body writes its width into `Page.width`; the body reads it one frame later and switches between `row()` (rail at the right) and `col()` (rail first). A scene has no window to narrow, so it pins the width (`pinned_width`) instead.
- **`border_sides([0, 0, 1, 0], …)` is the rule under the rail** in one column; the same rail has `[0, 0, 0, 1]` beside the conversation. `border_layout()` goes with both.
- **Capitals are `to_uppercase()`** on the string (there is no text-transform); `.tracking(0.66)` is `letter-spacing: .06em` at 11 px.
- **`cargo test`'s build needs `MemoryMax=7000M`** here: at 4000M it is killed with a silent exit 143.

## Component 23, piece E (a machine's view and the peek)

- **A tile's border that changes on hover is a ring**: an outer column with `padding(1.0)` and the border colour as its fill, `hover(...)` on it, and the tile's own fill inside (`ring.hover` shows at the edge only, because the face covers the rest). `.hover()` on the outer element still fires with the pointer over its child.
- **Measure Fleet's spacing in its DOM, not from the class names**: Tailwind's steps are 3.5 px at the 14 px root (`gap-3` is 10.5, `p-3` 10.5, `mb-5` 17.5, `px-1.5` 5.25). A first try with 4 px steps left every tile 3 px taller than Fleet's; Playwright's `getBoundingClientRect` on one tile and its children settled it.
- **A test clock that is moved in one frame folds a noted event too early**: `mission_sessions_event` is noted on a frame and its 300 ms fold runs from that frame, so `later(FOLD + 0.1)` in one step never fires it; move the clock in two (`later(0.1)`, then `later(FOLD + 0.1)`).
- **`text_input(...).on_edit(|s| &mut s.x)` with `.on_submit` is a complete nudge field**; `press_key(Key::Text(..))` after a `click` on its key types into it, `Key::Enter` submits.

## Component 21, piece M (the `#` picker)

- **A key reaches the field after the frame's `poll`.** A test that types `#` and then reads what the page's `keep` made of it must draw one more frame first, or it sees the state from before the key (it read "not loading" and went on).
- **A popup above a frame is `anchored_fit(frame_key, Side::Top, Align::Start, gap)`**, as wide as the frame by setting its own width from the frame's `on_layout`; the host's row keys (`github-item-picker-row-{n}`) and `scroll_into_view_in` keep the arrow keys' row on screen.
- **`on_hover(|s| ...)` is `mouseenter`**: the picker's row sets the selected index with it, and `on_press_tx` picks. A row with `.focusable(false)` takes the press without taking the box's focus (Fleet's `@mousedown.prevent`).
- **A `position: sticky` head and foot are fixed rows outside the scroller** when the popup has a `max_h`: the rows scroll between them, which looks the same.
- **A test fake that answers by exact path** needs the query string encoded as the request encodes it; build the expected path with the same `encode_component` the request uses.


## Component 18, piece K (the draft model)

Nothing new about Rune: the draft model is plain Rust with no view. One note for the next piece: Fleet's `routes` is an object
whose order the file keeps, so it is a `Vec<(String, String)>` here, and a rename that lands on an existing key keeps one entry
(the last value, in the first one's place), as an object would.

## Component 20, piece K (the sweep)

- A scripted run answers a save dialog with `{"type":"file-dialog","path":…}` (`file-dialog-cancel` cancels), so Export CSV can be walked live and the saved file compared with the server's.
- **Walking a live Fleet headlessly:** `--script` taps by key (`rail-github`, `github-bookmark-<owner>/<repo>`, `github-row-<owner>/<repo>#<n>`) and a final `{"type":"move","key":…}` onto a neutral element keeps the rail tooltip and a row's hover tooltip out of the picture; a menu left open (the Label popover) is shot by not moving the pointer away. Run it with `LD_LIBRARY_PATH` from `env.sh` or the binary does not start.

## Component 23, piece F (jobs' data and store)

- **A store test can call a store's own methods on `rt.state`** (`rt.state.jobs.watch()`): `rt.update` is for what must run on the UI thread with a `Tx`, and a plain method needs neither. The next frame's `keep_*` then does the reading.
- **Count only the requests you mean.** A connected app asks its fake home for sessions, preferences and more, so a "nothing is asked" test counts the paths under `/api/jobs`, not every request.

## Component 23, piece G (Jobs' list, page and a job)

- **Tailwind's spacing is rem, so at Fleet's 14 px root `px-3` is 10.5 px, `p-5` 17.5, `max-w-2xl` 588 and `text-sm` 12.25.** A checklist row that says "padding 20" came from reading the class as 4 px steps; take sizes from the class with `rem()`, and check against a picture of Fleet's client.
- **`--border` is translucent, so `bg-border/60` is `t.border.alpha(t.border.a * 0.6)`**, not `alpha(0.6)` (which is nearly opaque grey on the dark theme).
- **Rune has no grid:** a two-column `dl` whose first column is `auto` takes the width of its widest name; when that name is always there ("Repository"), a fixed width equals it.
- **A text element's key belongs on the text to read it in a test** (`e.text`); a keyed row around it has no text of its own.

## Component 23, piece H (the Agents tab, New job, the seats in Settings)

- **An element has no change or blur event.** A browser input's `@change` (Fleet's seat and label fields save on it) is Enter here, plus the focus moving away: `keep_job_seats` runs in `poll`, remembers which field `tx.focused()` last named and saves it when the focus is somewhere else. A poll-driven refill of the field from the server must skip the field being typed in.
- **Rune gives a view no window width**, so a Tailwind `lg:grid-cols-2` can't be switched on; two columns are rows of two `grow(1).w(0.0).min_w(0.0)` cards (without `min_w(0.0)` a card with a long wrapped line won't shrink to its half). `items_stretch` on the row makes the cards of a row equal in height, as a grid does.
- **A select in a dialog**: `ui::select::trigger` + `content_in` with a `close` that only clears the form's own `picking` keeps the dialog open when a list is picked from or dismissed (Escape closes the list first, then the dialog).
- **`ErrorNote` needs a text key on its words to read in a test** (`<key>-text`, from `error_note::message`).

## Component 25, piece A (the server inside)

- Nothing in Rune was needed. Two things about this machine's builds: `cargo test` of the whole crate is OOM-killed (silent exit 143) at `MemoryMax=4000M`; it finished at 7000M. A non-test `cargo build` of the app and an example fits in 4000M.
- A real window on a virtual display starts the bundled server in under 3 s when warm, so a screen grab of the start page needs to be taken at about 1 s, before Rune has drawn anything but black: not worth chasing.

## Component 22, piece D (tool rows asking the server)

- **No view, so nothing new about Rune.** What this piece needed from the app's side: a request in flight has to be counted by the app, not by the test. A
  store that starts work on threads keeps its own `idle()` (renders running or queued, reads of the sites or the switch on their way) so a test drives frames until
  it holds, then a frame or two more and checks again: the first answer's job can start the next request.
- A store with a bounded number of requests at once is tested with a fake server that holds each one on a `Condvar` permit and counts `active`/`peak`; the
  test hands out permits one at a time and waits for the next request to arrive before the next permit.

## Component 18, piece L (the editor's state)

- **A wait after the last key needs the clock, and a handler has none.** An edit arms a timer; `keep_*` (called from `poll` with `now`) turns "armed" into a deadline and asks `tx.wake_after` for the frame. The wait therefore starts at the next poll, so a test draws a frame, then the delay.
- **`Runtime::update` returns nothing**: read a value out of the state with `rt.state.…` (a `&mut self` method such as `take_saved()` works there too).
- **A test fake that must answer from the request** (a check echoing its text) is easier as a small handler table than as fixed routes; one that must be slow waits in its handler on an `AtomicBool` with no lock held.

## Component 22, piece E (tool rows: drawing the trees)

- **`max-width: 60%` and `min-width: min(30%, 12ch)` are a measured width.** Rune has no percentage size, so the header row measures itself (`on_layout`) into one
  number in `State` and the slot's `max_w` and the title's `min_w` are shares of it (less the header's padding). It arrives a frame late and is the width of the
  last row that measured, so a row built before any has measured is uncapped for a frame. Rows in a column are all as wide as each other, which is why one number does.
- **A press on an element with its own `on_press` stops there**, even where the press lands on text under it that has no handler: that is `@click.stop` for a mod's
  slot (an empty `on_press(|_| {})` on the slot), and a control inside it, being the innermost, takes its own press first.
- **A reused row (`memo`) has to be told what a view changed.** A row's memo key is a tuple of things the row reads; a store that changes how a row draws without
  changing its message (a view arriving from a thread) adds a number to that key. A pointer is not one (a freed view's address comes back): `Views::stamp` is the
  contribution's own counter. A message none of whose rows has a view hashes to 0, so another row's view never moves it.
- **One `ModControls` can't hold one sink per site.** A tool row's controls all go through one `send`; the sink is a closure with no `State`, so a row's action is queued
  (`pending`) and the frame finds the row by the handle in the views' trees before it posts (the session's machine matters). A pane (piece F) routes its own the same way.

## Component 25, piece B (Linux packages)

- **Rune has `rune::set_app_id(id)`** (call before `run`): it sets X11's `WM_CLASS` (class and instance) and Wayland's `app_id`, which is what a desktop entry's `StartupWMClass` has to match. Without it the window's class is the title. Checked on X11 only (`xdotool search --class`).
- A Rune binary links libxkbcommon, Wayland and WebKitGTK directly, so a `.deb`'s `Depends` must list them: the app does not start without them.

## Component 18, piece M (the editor's frame)

- **A keyed row has no text of its own in `Runtime::elements()`**: the text is on the child. A test that wants "what this banner says" joins the text of the element and of every element whose `path` starts with its path plus `/`.
- **`ElementInfo` has no `disabled`**; a disabled control is `focusable == false`.
- **`border_sides` is CSS order** (top, right, bottom, left) and takes no layout room unless `.border_layout()` follows.
- **A toast over a column**: a row with `pin(Some(l), None, Some(r), Some(bottom))` and `justify_center` centres a pill without a wrapper (seen in the scene).
- **An alert dialog Escape can't close** (Fleet's `:open`-only dialogs): give `Dialog::dismiss` a function that does nothing.

## Component 18, piece N (the File view)

- **Component 11's editor works on a buffer that is no session's file by giving it a session of its own.** `file_buffers` keys a buffer by (machine, session, path), so the workflow file lives under the session name `FIELD_SESSION`, which no
  session has: the code view, its keys, the caret, the history and the colours then work unchanged (`FileBuffers::load_text` makes it, `take_text` reads it back and marks it clean, so it is never "unsaved" to the app's own guards).
  What it took: `code_edit_target` and `edit_surface` answer for the field first (`workflow_field_target`), Ctrl/Cmd+S in `code_edit_key` goes to the workflow's Save, and `Marks::error_line` tints the parser's lines. The field has no file tab, so
  nothing else of component 11 (Find, the changes lists, the pick bar) sees it.
- **Typing goes to the workflow editor's text by a once-a-frame diff** (`keep_workflow_field` in `poll`): the buffer's text against the last text both sides agreed on. A change on the buffer's side is `edit_text`; a change of the editor's text under it (a reload, a save) replaces the document, as `watch(content)` does in Fleet, and the caret and history go with it.
- **A virtual row list's `version` has to carry whatever a row reads besides its text**: the error lines are in it, or a check answering would not redraw the tinted rows.
- **A buffer made again must not restart its revision at 1**: the code view caches colours and the stripe by (machine, session, path, revision), so a field opened a second time (or a second scene) with another text under the same number drew with the first one's colours, and indexed past its lines. `FileBuffers::load_text` gives each load a range of its own.
- **Known gap (component 11's colours):** Rune's YAML grammar tags plain scalars as strings (green), where Fleet's legacy YAML mode leaves them in the text colour; the keys, comments and brackets match.

## Component 22, piece F (panes: the tab, the events, the `+` menu)

- **No scroll padding.** `scroll-padding-inline: 24px` has no Rune equivalent (`scroll_into_view_in` is CSS's `nearest`). While a Draft mark is on a tab, each tab sits in a
  keyed box 24 px wider on each side, pulled back with a negative margin so layout doesn't change, and the strip reveals that box instead of the tab. With no Draft tab
  the strip is as before.
- **No `margin-left: auto`.** The hint at a menu row's end is a `spacer()` with `ml(-8.0)` (it takes back one of the row's two gaps), so it costs no width. A plain
  `spacer()` between the title and the hint cost 8 px and made a 252 px row truncate its title.
- **A new tab slides in for 240 ms**, so a test that measures where it lies has to run the clock first (a rect read straight after was 3 px off).
- **The test build needs about 4 GB** and a build next to another session's was OOM-killed (exit 143 from the scope, `oom-kill` in the journal). Wait for the other
  `rustc --crate-name fleet_native` to end, or give the scope more (7000M finished).


## Component 22, piece G (the pane's body)

- **`rt.elements()` still lists an element that is leaving** (`exiting: true`) for a few frames after a tab switch: a test that reads the words of the front pane has to skip those, or it
  sees both panes' text.
- **A button that works on its own** takes `.disabled(true)` and an `opacity`; there is no `disabled` look in `ui::button`, so the pane's busy Turn on dims to 0.5 like Fleet's.
- **Text with a different line height needs the line in the font spec** (`sans(13.0, 400.0, 1.5)`): 1.4 made the off state's button 3 px higher than Fleet's, after two wrapped lines.

## Component 18, piece R (New workflow and Duplicate)

- **A dialog's `<code>` run is a `rich` text with a mono `Span`**, found by `str::find` on the sentence (`ui::dialog::Dialog::description_el`); nothing else is needed to wrap it with the rest.
- **The scripted theme toggle flips the saved theme on every start**: with `FLEET_NATIVE_DATA_DIR` kept between runs, only the first run of a theme may carry the Cmd+Shift+L key, or the next run comes out the other theme.
- **A script's `type` is slow (`every`, 0.03 s a letter by default)**: a later `tap` placed too soon cuts the typing short, so a picture of a submitted name needs the tap after the last letter.
- **Rune has no select-all on focus** for a `TextEdit` (a browser input selects its text when autofocused, which Fleet's Duplicate name does); the caret sits at the end.
## Component 18, piece O (the canvas and its loop arcs)

- **Loop arcs are a flattened path with the dashes made by hand.** `Path::quad_to` gives the corners; the code samples each curve at 8 points, walks the polyline by length (4 on, 3 off) and strokes every dash as its own `LineCap::Butt` stroke, so a dash goes round a corner unbroken. The arrow head (SVG `marker-end`) is a filled triangle placed from the end point.
- **Arcs are drawn from the nodes' measured places**: every node and the flow report `on_layout`, and the view subtracts the flow's origin. The rects are in window points, so a scroll moves all of them together and the difference holds; the first frame has no places and draws no arc.
- **A label turned -90 degrees about a point**: an absolute box of a fixed size centred on the point, `rotate(-FRAC_PI_2)` about its centre, with the pill centred inside. Rune has no `translate(-50%, -50%)` for a box of unknown size.
- **A pattern background (CSS `radial-gradient` tiled) has no equivalent**: the dots are one `Shape` of small squares on the viewport element, which stays still while its scroll child moves. About 2,700 fills at 1280x760.
- **A box-shadow ring is `outline(3.0, 0.0, colour)`** (Fleet's `0 0 0 3px`).
- **A menu that must sit over the next sibling is an overlay**: a child of the connector would be drawn under the node after it. `anchored(..)` with `.modal(true, close)` gives Escape and click-outside.
- **`on_layout` fires only when the element moves**, so measured places kept in `State` must not be cleared while the same elements stay on screen (opening another file with the same layout): the arcs would vanish for good. `Canvas::reopen` clears the picks and keeps the places.
## Component 23, piece I (slice 1 sweep)

- **`border_sides` paints but takes no room.** A box with `.border_sides([1.0, 1.0, 1.0, 0.0], c)` lays its content out as if it had no border, so a
  card Fleet draws with a 1 px border all round (`border border-l-4`) came out 2 px shorter than Fleet's. Put the 1 px in the padding of the box inside
  (`py(pad + 1.0)`); `.border_layout()` is the other way when all four sides are alike.
- **Tailwind at a 14 px root**: `mt-1` is 3.5, `mt-2` 7, `gap-1.5` 5.25, `gap-3` 10.5, `px-2` 7. Round numbers (4, 8, 6, 12) look right in a picture and drift by 1 to 2 px a card.
  Measure Fleet's DOM (`getBoundingClientRect`) once and compare the native card's rect in a test.


## Component 22, piece H (draft cards)

- **A virtual row's own margin is dropped; its padding is kept.** The cards' wrapper had `mt(8)` and drew flush against the reply above; `pt` on the same column
  works. CSS margin collapsing (Fleet's cards are in a block parent, so their `margin: 8px 0` meet as 8, not 16) has no Rune equivalent: use `gap` between the cards.
- **`rt.find(key)` doesn't find every keyed element.** A keyed column that only wraps others (`mod-draft-cards`) was never found, while the keyed row
  of the Draft mark was; tests of what a row draws read the words (`rt.elements()` text) or a leaf's key.
- **`ui::button` pressed in a test runs its handler even with `.disabled(true)`**: the test click doesn't check it, so the handler also refuses (Keep… on a draft still being written).
- **A POST with no body sends `{}`** (`http::text_with`), so Fleet's "no body for an empty note" is `{}` here; the server reads a missing note as none either way.
## Component 18, piece P (the inspector)

- **A form of many text fields is one poll, not many handlers.** `on_edit` hands back a `TextEdit` and nothing is told what it now says, so `Editor::sync_inspector` keeps a mirror of what each field last agreed with: a field that differs from its mirror is the user's edit (it goes into the draft); a draft that differs from the mirror under an untouched field goes into the field. That also covers a reload, a rename and a step that opens.
- **"On change" for a field that commits when it is left** (Fleet's `@change`) is `TextEdit::focused` seen going from true to false in that poll, plus Enter in `on_input_key`. A click on another node moves the focus before it selects, so the id is committed for the step it was typed in.
- **A browser `<select>` is the kit's list**: `select::content_in` anchored to a box keyed per row, opened by a state field (not `ui::Open`) so it can sit in the designer's own overlay beside the + menu.


## Component 22, piece I (the Keep dialog)

- **`ui::dialog` scrolls the whole panel when it has a `max_height`**, so its footer scrolls away. A dialog that keeps its footer in view puts a scrolling column
  (`.max_h(window * 0.88 - chrome).scroll(..)`) in the body and leaves `max_height` off; the chrome (frame, title, footer) is a constant that has to be guessed.
- **A disabled `ui::button` is only half opacity** in `rt.elements()` (there is no `disabled` field), so a test reads `opacity < 0.9`.
- **`TextEdit` has no character limit**; a field's limit is applied where its text is used.

## Component 18, piece Q (the inspector: an agent step)

- **A chip that inserts at the caret is `TextEdit::insert` plus `tx.focus(key, false)`.** Pressing a button doesn't clear the field's selection, so `insert` takes the caret or the selected text; `tx.focus` gives the box the keys back without selecting all.
- **Several boxes that show one value** (a max on each loop) need one `TextEdit` each: the poll takes the first box that differs from the mirror as the user's edit and writes it into the others.
- **A `<datalist>` has no counterpart.** Agent, Skill and Effort are plain boxes; the kit's list can't take typed text.
- **The test build now needs more than 5000M** (`cargo test` was OOM-killed at 5000M, finished at 7000M, 22 November 2026).
- **A field loaded in `poll` draws only if `poll` says it changed.** `sync_inspector` loaded a newly picked step's fields and returned false, so a live click showed empty boxes until the next input (tests and scenes run frames, so they never saw it). Return true whenever a field was set under the view.

## Component 22, piece K (safe mode, the band and the chip)

- **A row that wraps needs `gaps(row, column)`**, not `gap`: Fleet's banner text uses `gap-x-2 gap-y-0.5`, and one `gap` would space the wrapped line too far apart.
- **A bar's end holds its children in one row, so a chip is inserted into `view::status::end`**, not laid beside it; a chip that clips takes `max_w` and `clip_rounded(0.0)`.
- **A test reads "is this button dimmed" from `opacity < 0.9`** (`ui::button::disabled` sets 0.5); there is no disabled flag in `ElementInfo`.
- **The test build was OOM-killed at 4.8 GB** under a 5000M scope while another session built (exit 143, `oom-kill` in the journal); 7000M with `CARGO_BUILD_JOBS=2` finished.
- **Opening a link from a button** (updater, piece 25F): a handler that needs the `Tx` (`tx.open_url`) goes on `on_press_tx(|s, tx| …)`; `on_press` closures take only the state and a two-argument closure fails to compile with "expected closure that takes 1 argument".

## Component 23, piece L (the Board's page)

- **A button's words are its `label`, not its `text`**: `ui::button`'s keyed row has the label; the words are a child text with no key, so a test reads `ElementInfo::label`.
- **Equal columns need `w(0.0).grow(1.0).min_w(0.0)`** (Rune has no flex-basis): with `grow(1.0)` alone the share is what is left after each content's width.
- **A window `on_key` can't tell a text field from a button**: `tx.focused()` is only the key. The Board's fields are keyed `board-…-input` and `board::page::is_text_field` reads that.
- **`wrap_anywhere` is `overflow-wrap: anywhere`** (a long unbroken board name wraps); a bare `wrap` overflows.
- **Opacity eases**: a button that was just enabled still reads below 1 for a few frames, so a test of "not disabled" runs about 30 frames first.

## Component 18, piece S (drafting)

- **A sentence with parts that are their own elements** (Fleet's drafted banner: bold lead, a muted cost `<span data-testid>`, an errors span) is one `rich` text with coloured `Span`s here; a span can't be keyed, so a test reads the whole paragraph's text.
- **A request can't be cancelled from the UI thread's side**: `Link::off_thread` has no abort, so Cancel and "a newer ask" bump a run number and the answer that comes late is dropped (`Nav::draft_answers` counts answers so a test can wait for the dropped one).
- **Fleet's `<select>` is the browser's own**; the kit's `select::trigger` and `content_in` stand in for it (the list is the kit's, not the OS's).

## Component 23, piece M (the Board's lanes and cards)

- **A card with `border-left: 3px <colour>` and a 1 px border in another is a row**: a 3 px stripe `el()` with `self_stretch()`, then the body, on a box with `border_sides([1.0, 1.0, 1.0, 0.0], c)`, `border_layout()` and `clip_rounded` (Rune has one border colour per box).
- **No hover border or hover fill on a keyed row that isn't a button**: Fleet's GitHub link on a card changes border and fill on hover; only `El::hover(fill)` exists, so it doesn't.
- **A hover lift is `offset(0.0, -2.0)` plus a `Spring` `offset_motion`, switched by `on_hover_change`** writing the hovered card's id into the state; `.shadow(y, blur, colour)` adds Fleet's drop shadow with it.
- **`border_dash(3.0, 3.0)` on a `ui::button`** gives the dashed "+ Add a card" (add `border(1.0, c).border_layout()` first).
- **A label that must wrap in a narrow grid column needs `.wrap()`**; a bare `text` runs past its column ("GITHUB UPDATED").
- **Fleet's body line height is 1.5**, not the 1.6 an `h-` guess suggests: measure `getBoundingClientRect` in Playwright (the card was 160.7 px tall with `dt` 15, `dd` 16.5, pill 23).
- **`TextEdit::insert` returns the old value**: a test's `rt.update(|s, _| map.insert(..))` needs `drop(..)` to return `()`.

## Component 26, piece E (Machines: the data)

- Nothing new about Rune: this piece draws nothing. Two build notes: a test build of this crate peaked above 4.8 GB with `CARGO_BUILD_JOBS=3` and was OOM-killed under a 5000M cap (exit 143 on the wrapper); `CARGO_BUILD_JOBS=1` finished in 6 minutes.

- **A scripted tap on a key that a server answer creates exits 2** (key not found) if it comes too soon after the state change: the `+` menu's mod rows exist only once the session's sites have been read. Give such a tap 1.5 s or more after the one before it (22L, live walk).
- **An IME-confirming Enter never reaches a key handler**: Rune gives Return to a composing field first (`rt.ime(Ime::Preedit(..))` then Return in a headless test), so Fleet's app-wide `ime-enter-guard` has nothing to do here (tested on the composer, piece 26C).

## Component 18, piece T (slice 2 sweep)

- **Walking a scrolled canvas in a script**: `{"type":"wheel","key":"workflow-canvas","dy":-400}` scrolls the canvas down (a negative `dy` moves the content up); a `tap` on a node that is clipped out of the scroll area does nothing, so scroll first. A positive `dy` did nothing at the top.
- **A script's clock is virtual**: a headless `--script` run takes about 6 times its script time in wall seconds, so an outside change timed by wall clock (a file edited on disk to make a conflict) lands unpredictably. Have a watcher poll for the file the walk creates, then change it a moment later.

## Component 26, piece L (the palette's last five)

- `cx.zoom` is one factor for the whole window, so Appearance's font size and the palette's zoom multiply into one call
  (`view.rs`). Fleet's zoom changes only what is in `rem`, so its sidebar width and fixed-px parts stay put; here the
  whole window scales, as the font size already did.
- A `text(..)` with `.shrink()` and no `.truncate()` does not wrap on its own in a row; add `.wrap()` (the palette's
  descriptions).

## Component 26, piece F (Machines: the list)

- Nothing new about Rune: the reach line is a `text` under the state line, coloured by its tone. Build note: even `CARGO_BUILD_JOBS=1` can pass 5000 MB while compiling the test build of this crate (peak 4.8 GB, killed at 5000M twice, exit 143 or 137); 5600M finished.

## Component 23, piece N (the Board's manage mode)

- **An inset box-shadow draws a header's inner line**: `.box_shadow(Shadow { x: 0.0, y: -1.0, blur: 0.0, spread: 0.0, color, inset: true })` is Fleet's `inset 0 -1px 0`; `border_sides([0.0, 0.0, 1.0, 0.0], c)` after `bar_lead` recolours its rule, since a later `border_sides` replaces the first.
- **A button that is off screen can't be clicked in a test**: `rt.find(key)` finds it but the press lands on whatever is there, so a test clicks the lanes it can see (the test window shows three of four lanes) and reaches the rest through the state.
- **A test build of this crate peaks near 4.8 GB with debug info**: on a shared 14 GB machine it is OOM-killed (exit 143 under `systemd-run`); `--config profile.dev.package.fleet-native.debug=0` with `CARGO_BUILD_JOBS=1` fits (the dependencies stay built).

## Component 23, piece O (dragging a card)

- **A pointer drag with targets inside a scroll box works for cards as for rows** (`Tx::element_at`, the keys on `Picked::path()`). Lane ids hold hyphens (`lane-doing`), so a path segment can't be split into lane and index: try each known lane's id as the prefix (`board-slot-<lane>-<i>`).
- **A slot's height eases on the pick's own commit**: set `tx.motion_curve(Curve::tween(0.2, ..))` inside the `element_at` callback, or a drag's commit lands the 8 to 18 px change at once.
- **A dragged element must not follow the pointer with `offset`**: hit tests follow the shift, so the pick would find the dragged card itself and never the target. The card stays where it is, faded and tilted (`opacity(.45).rotate(..)`); no ghost is drawn.
- **A test drag** presses on the card's title (the card's own centre can be a button), moves 12 pt, runs two frames, then moves to the target's `rt.find` point and runs two more.

## Component 26, piece M (sub-agents and running work)

- Nothing Rune couldn't do. Two build notes: a test build of this crate was OOM-killed (silent exit 143, 4.7-5.1 GB peak)
  under a 5000M cap on a loaded machine; `cargo test --config 'profile.dev.package.fleet-native.debug=0'` (debuginfo off
  for this crate only, Rune isn't rebuilt) with `CARGO_BUILD_JOBS=1` and a 5600M cap finished in 4.5 minutes.
- The note that stands for the composer (`view::subagent_prompt_line`) is a wrapping row of three texts, not one inline
  paragraph: Rune has no inline link in running text (see component 3's chips), so a title too long for the line moves to
  the next line as a whole rather than breaking in the middle.
- **A focused text field takes Esc before the window's `on_key`** (`edit/input.rs`: `Key::Escape => out.blur = true`), so a window-level Esc handler never sees it while a field has the focus. A field that must clear itself on Esc, as a `type=search` box does, uses `.on_input_key` on the `text_input` and returns true (the list's filter, piece 26P).
- **A pinned row that moves is a new element in the Pinned group**, so the focus it had is lost (it falls to the group's `pinned` element). After a pin move, ask for the focus again by the row's key once the list is drawn (`tx.focus("session-<p>-<i>", true)` from the poll, piece 26P).
- **No right-aligned absolute position**: `absolute(left, top)` is all Rune has; a button laid over the right end of a field is a row with the button given `.ml(-width)` over the field, which grows (the filter's clear button).
