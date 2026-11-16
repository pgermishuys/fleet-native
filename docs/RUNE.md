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
- **`ElementInfo` has `opacity` and `text` but no tooltip text**: test a `.help` string through the function that builds it.
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
- **A spinner doesn't turn** (no looping rotation): it fades, as in the older-messages row.

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

- No turning animation: `Cycle` only moves opacity, so a spinning icon (`CheckIcon` pending) fades in and out like the other spinners here.
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

