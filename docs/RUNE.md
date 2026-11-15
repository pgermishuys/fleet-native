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
