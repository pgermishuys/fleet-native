# fleet-native: agent instructions

## What this is

[Weave Fleet](https://github.com/pgermishuys/weave-fleet)'s desktop app, rebuilt as a native app in
[Rune](https://github.com/anomalyco/rune) (Rust, drawn on the GPU, no browser engine).

It is an experiment. Fleet's desktop app today is Electron around a Vue client, and that stays the
app people install until this one is shown to be better. The aim here is a native client of **the
same Fleet server**: same HTTP API, same live events, same look, same keys. Desktop only (macOS,
Windows, Linux). The phone app and the browser UI are not part of this.

What exists: the shell (rail, session list, conversation, composer, content panel, status bar),
drawn from invented data. `README.md` lists what works.

What to build, in what order, and what has been decided: **`docs/PORT.md`. Read it before you
start.** A session takes one piece of one component from it, and nothing else.

| Read | For |
|---|---|
| `docs/PORT.md` | The plan: the components in order, how each is done, the decisions |
| `docs/research/` | Why the terminal and web content are built the way they are |
| `docs/FLEET.md` | Fleet's server contract and where each part lives in weave-fleet |
| `docs/RUNE.md` | What Rune has, what it lacks, and what we learned using it |

## weave-fleet is the source of truth

You are porting, not redesigning.

- Layout, sizes, colours, wording, keys and behaviour come from weave-fleet's `client/`. Where this
  app differs and Rune isn't the reason, that's a bug here.
- Get the source: use a `weave-fleet` checkout beside this repo if there is one, else
  `git clone --depth 1 https://github.com/pgermishuys/weave-fleet ../weave-fleet`.
- Treat it as read-only. Never change weave-fleet from a session on this repo. If you find a bug
  in Fleet itself, say so in your report; don't fix it from here.
- Each view file names the Vue components it ports in its first comment. Keep that true.
- Sizes are Fleet's CSS pixels at its 14px root font size (`1rem` = 14). Take them from the
  component and its Tailwind classes, not from a screenshot.
- Colours and type sizes come from `src/theme.rs`, which is copied from
  `client/src/assets/main.css`. A view never holds a colour literal. A token that's missing gets
  added to `theme.rs` with the CSS variable's name beside it.

## Layout vocabulary

The same words as weave-fleet's `AGENTS.md`:

```
[rail][context]
```

- **Rail**: the icon navigation on the left edge, always visible.
- **Context**: everything to its right. Whatever the user picks in the rail owns all of it. No view
  shows another view's panel (the session list belongs to Sessions only).

| Route | Context layout |
|---|---|
| Sessions | `[session-list][conversation][content]` |
| Settings | `[settings-menu][settings-detail]` |
| Automations | `[automations-list][automations-detail]` |
| Analytics | `[analytics-menu][analytics-detail]` |
| Workflows | `[workflows-list][workflows-detail]` |
| Jobs | `[jobs-list][jobs-detail]` |
| Fleet | `[fleet-menu][fleet-detail]` |

Fleet has two resize gutters (`AppShell.vue`): at the right edge of the left panel, and between
`[conversation]` and `[content]`. Nowhere else.

## Toolchain

- Rust, pinned in `rust-toolchain.toml`; rustup installs it on first use. Cargo only: this repo has
  no Node, no bun.
- Linux needs `libxkbcommon-dev libwayland-dev` to build, `libxkbcommon-x11-0` to open a window on
  X11, and for pictures on a machine with no GPU `mesa-vulkan-drivers`.
- The first build compiles Rune and takes minutes (about 16 for a release build on a four-core
  laptop, about 8 on a GitHub Windows runner). After that a change rebuilds in seconds. Never
  `cargo clean`. Use debug builds for work and tests; build `--release` only to feel or measure.

```sh
cargo fmt                                   # rustfmt.toml: 140 columns
cargo clippy --all-targets -- -D warnings   # CI fails on any warning
cargo test                                  # headless: no window, no GPU
cargo run                                   # the window
bash scripts/snap-scenes.sh                 # every scene to shots/out/*.png (debug build)
```

## Check your work: look, don't guess

Rune apps can be driven and drawn with no window. Use that, in this order.

1. **`cargo test`.** `src/tests.rs` builds a headless runtime, clicks elements by key, presses
   keys, runs the clock and reads the state. New behaviour gets a test in that style. Give every
   element a test needs a `.key("...")`.
2. **Scenes.** Every screen and every state worth looking at is a scene in `spec()` (`src/lib.rs`).
   A new panel or state adds one. `scripts/snap-scenes.sh` draws them all.
3. **Look at the picture.** Open the PNG and compare it with the real app showing the same thing
   at the same size. A picture you didn't look at is not a check.
4. **Scripted runs.** `--script file.json` clicks, types and drags on a clock; `--snap out.png --at
   mark:<name>` takes a picture at a mark and `--video out.mp4` records the run (needs `ffmpeg`).
   `scripts/tour.json` is the example.
5. **A live Fleet**, for anything that talks to the server: a scratch one, never a real one.
   `docs/FLEET.md` says how.
6. **Three systems.** CI builds, lints and tests on Linux, Windows and macOS for every pull
   request, but draws the scenes on Linux only; Windows and macOS pictures come from the runs on
   `main` (every push). Reason: Rune draws one scene per process start (`--script` can't switch scenes,
   so there is no one-renderer way to draw them all), and a start costs about 3 s on Windows' software
   renderer, which made the step 500 s there. To see Windows or macOS for a change that could differ,
   run the scenes there yourself, or look at `main`'s run after the merge.

Say in the pull request what you checked on which system, and what you couldn't. "Not checked on
macOS" is a fine sentence; a claim you didn't test is not.

Take pictures with `--at 0.3` (the script does). The sheet measures itself one frame late, so a
picture of the very first frame can be wrong.

On Linux with no GPU, set `VK_ICD_FILENAMES` to the lavapipe file
(`/usr/share/vulkan/icd.d/lvp_icd*.json`). Numbers from a software renderer say nothing about
speed; only measure on real hardware.

## Code conventions

- **One file per panel** under `src/view/`. A view is a plain function, `(&State, &Theme) -> El`. A plugin's views (GitHub's) live in
  the plugin's own `src/plugins/<name>/view/`, one file per panel there too: core never imports a plugin, so they are not under `src/view/`.
- **Plugins are add-ons.** Nothing outside `src/plugins/` names one: a plugin fills the places core offers (`src/fleet/places.rs`)
  and `src/plugins/mod.rs` `register` is the one composition root. `plugins::tests_seam` fails a core file that imports GitHub's.
  Keep a file under about 500 lines; split by panel, not by kind.
- **One `State`.** Handlers change it; the view reads it. No hidden state in views.
- **Nothing slow on the UI thread.** No network, file or process work in a view, a handler or
  `poll`. Do it on another thread and hand the result back (`docs/RUNE.md`, "Work off the UI
  thread").
- **Mock data stays.** With no server given, the app draws `src/mock.rs`. Tests, scenes and
  pictures use it, so they come out the same every time. Don't make a test or a scene need a
  server.
- **Fake servers bind and read requests through `crate::test_http`.** A test fake binds with `test_http::bind()` (never
  `TcpListener::bind`): its accept loop ends when the test that bound it is over. One that never ended kept a thread
  for the rest of the run, and about 2,000 of them hit macOS's threads-per-process cap. It reads with
  `test_http::read(&mut stream)` (the whole request: head, then exactly Content-Length bytes or the chunks, with a
  bound) and may answer with `test_http::respond_json`. Never one `stream.read()` or a hand-rolled Content-Length
  parser: on a slow Windows or macOS runner they see half a request (#203, #214, #250, #265).
  `test_http::tests::fakes_read_requests_through_test_http` fails a file that does either.
- **Invented data only**, in mock data, tests, fixtures, pictures and examples: never a real
  session title, repository, person, path or company.
- **Icons** are generated (`tools/gen-icons.py`, see `README.md`); don't edit `src/icons.rs`.
- **A new crate** costs build time on three systems, every time. Use what Rune already brings in
  (`Cargo.lock`) when you can, and say in the pull request why a new one is needed.
- **Borrowed code or assets**: copy the licence text, unchanged, beside what you borrowed and add a
  row to the README's Licences list.
- Comments say why, in plain words. No comment that repeats the code.

## This repository is public

Everything pushed stays in its history. Keep out: secrets, tokens, home-folder paths, machine names,
private hostnames, and anything from a real Fleet (titles, prompts, repository names).

## Rune

- `Cargo.toml` pins Rune to one commit (the `rev` appears five times). Rune is pre-release and
  changes many times a day, so its API moves.
- Read Rune's docs **at that commit**: `crates/rune/README.md` and `crates/rune-kit/README.md` in a
  clone checked out at the rev, or in Cargo's own copy under `~/.cargo/git/checkouts/`. The
  examples there are the best guide (`crates/rune-kit/examples/shell.rs` is a chat app's shell).
- Moving the pin is its own pull request with nothing else in it: all five revs, `Cargo.lock`,
  whatever broke, and every scene looked at again.
- Rune doesn't take pull requests. When Rune can't do something or does it wrong, write the
  smallest case that shows it into `docs/RUNE.md` and work around it here. Don't open issues on
  Rune's repository unless the user asks.

## Pictures, always

**No pull request is opened or merged without pictures of the app in its description.** That
includes docs-only, CI-only and refactoring pull requests: a refactor shows the app still draws the
same, a CI change shows a picture from the run it changed.

- The pictures are in the description **from the moment the pull request is opened**. Never a
  placeholder filled in later: take them first, then open it.
- Commit them under `shots/<topic>/` (a component's go under `shots/parity/<nn>-<component>/`) and
  link them by commit (`https://github.com/pgermishuys/fleet-native/raw/<sha>/shots/...`), so the
  link keeps showing what was true for that pull request.
- Show what the pull request changes. For a component, that is the native app beside Fleet's own
  client, same state, same size, both themes.
- Look at every picture before you put it in. A picture you didn't look at is not a check.
- CI enforces it: the **Picture in the description** check fails while the description has no
  image, and `main` only takes a pull request whose checks (that one and all three systems) are
  green on top of current `main`.

## Working in a session

- A session does one thing from `docs/PORT.md`, "How a component gets done": either a component's
  planning (its checklist and piece list), or one piece from a checklist in `docs/parity/`. Stay
  inside that piece's rows and files; when it's merged, report and stop.
- Work on a branch. Rebase on `main`; don't merge `main` in.
- A pull request has: its pictures (above); which checklist rows it covers and what is still open;
  what was checked on which system.
- In the same pull request, update the component's row in `docs/PORT.md`, its checklist in
  `docs/parity/`, and add what you learned about Rune to `docs/RUNE.md`.
- Merge when every check is green (all three systems and the picture check) on top of current
  `main`; rebase and wait for the new run if `main` moved. Squash.
- Stop and ask when: a gap needs a product decision that `docs/PORT.md`, "Decisions", doesn't
  cover; the component needs Rune's pin moved; or Rune can't do something the component needs
  (write the smallest case into `docs/RUNE.md` first).
