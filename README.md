# fleet-native

[Weave Fleet](https://github.com/pgermishuys/weave-fleet)'s desktop app, rebuilt as a native app
with [Rune](https://github.com/anomalyco/rune), a GPU-rendered UI framework written in Rust. No
browser engine under it.

**This is an experiment.** Fleet's desktop app is the Electron one in weave-fleet; that is the one
to install. This repository exists to find out whether a native Fleet is enough better to be worth
building. So far it draws Fleet's shell, from invented data or, with `--server`, from a Fleet
server: its sessions, the open conversation (text and tool rows) and what you send to the agent.

| Read | For |
|---|---|
| [`docs/PORT.md`](docs/PORT.md) | The plan: what gets built in what order, and what is undecided |
| [`docs/FLEET.md`](docs/FLEET.md) | Fleet's server contract, and where each part lives in weave-fleet |
| [`docs/RUNE.md`](docs/RUNE.md) | What Rune has, what it lacks, what we learned |
| [`AGENTS.md`](AGENTS.md) | How to work in this repository (written for coding agents; people too) |

![The shell, dark theme](shots/shell/dark.png)

![A reply streaming in from a scratch Fleet](shots/server/streaming.png)

## Run it

You need [rustup](https://rustup.rs). The first build downloads and compiles Rune, which takes
minutes; after that a change rebuilds in seconds.

```sh
cargo run             # a debug build: quick to build, fine for work
cargo run --release   # to feel how fast it is
```

- **macOS and Windows** need nothing else (on Windows, the Visual Studio C++ build tools that
  rustup asks for). Pages agents show are drawn by the system's web view: WKWebView on macOS, WebView2 on
  Windows (part of Windows 11 and current Windows 10; elsewhere Microsoft's WebView2 runtime).
- **Linux** needs three development packages to build, and two libraries to open a window:
  `sudo apt-get install libxkbcommon-dev libwayland-dev libwebkit2gtk-4.1-dev libxkbcommon-x11-0`
  (Fedora: `dnf install libxkbcommon-devel wayland-devel webkit2gtk4.1-devel libxkbcommon-x11`).
  WebKitGTK shows pages in an X11 window (or XWayland); on Wayland alone a page says it can't be shown
  and offers to open it in the browser.
- `cargo build --no-default-features` leaves the web view out (no WebKitGTK needed): every page then
  offers to open in the browser instead.

### Try it

No Rust needed. Every merge to `main` replaces one public pre-release,
[`latest`](https://github.com/pgermishuys/fleet-native/releases/tag/latest), with fresh builds. The links never change:

- Windows: [`fleet-native-windows-x64.zip`](https://github.com/pgermishuys/fleet-native/releases/download/latest/fleet-native-windows-x64.zip)
- macOS (Apple silicon): [`fleet-native-macos-arm64.dmg`](https://github.com/pgermishuys/fleet-native/releases/download/latest/fleet-native-macos-arm64.dmg)
- Linux: [`fleet-native-linux-x64.tar.gz`](https://github.com/pgermishuys/fleet-native/releases/download/latest/fleet-native-linux-x64.tar.gz)

Each holds the app (macOS: `Fleet Native.app` to drag to Applications; Windows: the exe, with its icon) and, except the
`.dmg`, this README. `latest.json` beside them gives the commit, the build number and each file's
SHA-256; `fleet-native --version` prints the build you have.

**What works** is the table below, and what is still missing is under "What isn't built" and in
[`docs/PORT.md`](docs/PORT.md): it is a partly built experiment, not a replacement for Fleet's desktop app.
It finds the Fleet running on the same machine by itself (see "Try it"); for another one, run `fleet-native --server <url>` or set `FLEET_NATIVE_SERVER`.
Sending a message makes that Fleet's agent act.

**The app updates itself.** A build you downloaded (not one from `cargo run`) looks at `latest.json` when it starts and every
few hours. When a newer build is out, "Update available · build N" appears in the status bar; click it, then **Update and
restart**. It downloads the file for your system, checks its SHA-256 against `latest.json`, replaces itself and starts again;
your drafts are kept. If anything fails, the app you have keeps running, unchanged, and says why. On macOS, if the folder the app
is in can't be written, the `.dmg` is opened for the usual drag instead. `fleet-native --update-now` does the same with no window.

These builds are **unsigned**:

- **Windows:** unzip and run `fleet-native.exe`. SmartScreen says "Windows protected your PC": click **More info**,
  then **Run anyway**.
- **macOS:** open the `.dmg` and drag **Fleet Native** onto Applications. The first time, right-click it in
  Applications and choose **Open** (then **Open** again), or run
  `xattr -dr com.apple.quarantine "/Applications/Fleet Native.app"` once. The app is ad hoc signed so Apple
  silicon runs it, but not notarised.
- **Linux:** unpack and run `./fleet-native`; it needs `libxkbcommon-x11-0` on X11 and WebKitGTK
  (`libwebkit2gtk-4.1-0`). For a launcher entry with an
  icon, copy the two files beside the program:

  ```sh
  install -Dm755 fleet-native ~/.local/bin/fleet-native
  install -Dm644 fleet-native.png ~/.local/share/icons/hicolor/512x512/apps/fleet-native.png
  install -Dm644 fleet-native.desktop ~/.local/share/applications/fleet-native.desktop
  ```

### Against a Fleet server

```sh
cargo run -- --server http://127.0.0.1:6411
```

Loopback and no token only so far: the server must be on this machine, and `http://`. Never point
it at a Fleet you use: sending a message makes its agent act. `scripts/scratch-fleet.sh` starts a
throwaway Fleet in an empty home with a scripted model instead of a real one (it needs a released
server, a weave-fleet checkout for the model's script, and `opencode`; its header says how).
A Fleet on another machine asks for its access token: `--token <token>` or `FLEET_NATIVE_TOKEN` (Fleet shows it in Settings > Machines > This machine;
the environment variable keeps it out of the process list). Only `http://` addresses work so far, so use the machine's own address (for example its
`--host 0.0.0.0` or tailnet address), not one behind `tailscale serve`. Without the token the window says the Fleet needs one and doesn't keep retrying.

`FLEET_NATIVE_SERVER=<url>` does the same where arguments are awkward (macOS from Finder: `open -n -a "Fleet Native" --args --server <url>`
also works; `open` ignores `--args` while the app is already running, so quit it first or add `-n`; or set it with `launchctl setenv FLEET_NATIVE_SERVER <url>` before opening the app); `--server` wins when both are given.

With neither, the app looks for the Fleet running on this machine, in this order, and uses the first that answers `/readyz`:

1. an instance file (`fleet.instance.json`, written by a running Fleet next to its database) at `Fleet__DatabasePath` (extension swapped),
   `WEAVE_FLEET_DATA_DIR` (what the `fleet` command uses; default `~/.weave`), `FLEET_DESKTOP_DATA_DIR` or `~/.weave` (Fleet's desktop app);
2. the server program's own default, `LocalApplicationData/WeaveFleet`: `~/Library/Application Support/WeaveFleet` (macOS),
   `%LOCALAPPDATA%\WeaveFleet` (Windows), `~/.local/share/WeaveFleet` (Linux);
3. the port Fleet's desktop app remembers in `desktop-settings.json` (`~/Library/Application Support/Fleet`, `%APPDATA%\Fleet`,
   `~/.config/Fleet`), since it takes any free port when 6262 is busy;
4. port 6262.

When none answers, the status bar says where it looked. With no Fleet found the window is empty and says so; it never shows invented data in a normal run. The invented sessions are what the tests and scenes use, and `fleet-native --demo` shows them by hand. The app keeps looking every few seconds, so it connects when Fleet's desktop app finishes starting its server.

The window opens at the desktop app's size (1360 x 880, no smaller than 720 x 480) and follows the
system's light or dark theme.

## What works

| Part | What it does |
|---|---|
| Rail | Sessions is the shell. Workflows, Automations, Analytics, GitHub and Settings each take the whole context area with a notice. Help does nothing. |
| Session list | Click a session to open it. Click a project to fold it. Type in the filter. Drag the gutter to resize the list (200 to 500). |
| Header | The open session's title, project, harness, model, branch and path. The buttons show their tooltips and do nothing, except Abort. |
| Conversation | Scrolls, and stays at the bottom as messages arrive. Text can be selected. |
| Composer | Type and press Enter (or click Send). The message joins the conversation and a canned reply follows 2.5 seconds later. With `--server` the message goes to the agent and the reply streams in as it is written. Shift+Enter adds a line; the field grows to seven lines. |
| Content panel | Click the collapsed rail on the right to open the session's plan; the chevron closes it. In a sheet too narrow for a third column the panel opens over the conversation, as in Fleet. |
| Status bar | The keyboard hints, the harness and the open session's token count. |

Keys (Cmd on macOS, Ctrl elsewhere):

| Key | Does |
|---|---|
| Cmd/Ctrl+B | Show or hide the session list |
| Cmd/Ctrl+[ and ] | Previous and next session |
| Esc | Interrupt the working session |
| Cmd/Ctrl+Shift+L | Switch between light and dark (the prototype's own key, not one of Fleet's) |

## What isn't built

Everything behind the shell: older history and the rest of the reducer's events, New session, the command
palette, the terminal, Settings and the other rail views, menus and dialogs, markdown and code
blocks in messages, tool call details and the content panel's tabs. [`docs/PORT.md`](docs/PORT.md)
has the order they get built in.

## Tests and pictures without a window

```sh
cargo test                    # drives the app with no window and no GPU
bash scripts/snap-scenes.sh   # draws every scene to shots/out/<scene>.png
```

A scene is a named state of the app (`light`, `dark`, `sidebar-closed`, `content-open`,
`needs-input`, `automations`). On Linux without a GPU, install `mesa-vulkan-drivers`; if a picture
still fails, set `VK_ICD_FILENAMES` to `/usr/share/vulkan/icd.d/lvp_icd*.json`.

One picture, or a scripted run, by hand:

```sh
cargo build
target/debug/fleet-native --list-scenes
target/debug/fleet-native --snap plan.png --scene content-open --size 1280x760 --scale 2 --at 0.3
```

`scripts/tour.json` clicks and types through the shell (opens sessions, sends a message, opens the
plan, drags the gutter, hides the list, switches theme). Take a picture at one of its marks
(`sent`, `replied`, `plan`, `dragged`, `end`), or record the whole run (needs `ffmpeg`):

```sh
target/debug/fleet-native --script scripts/tour.json --snap replied.png --at mark:replied
target/debug/fleet-native --script scripts/tour.json --video tour.mp4
```

## How it's put together

| File | Holds |
|---|---|
| `src/lib.rs` | The state, the keys, Send and Interrupt, and the app's `AppSpec` |
| `src/theme.rs` | Fleet's colours and type sizes, copied from weave-fleet's `client/src/assets/main.css` |
| `src/mock.rs` | The invented projects, sessions and messages |
| `src/mock/long.rs` | The 2,000-message "Long conversation" and the reply that streams into it |
| `src/script.rs` | The scripted reply (no server): grows the last message on the app's clock |
| `src/view.rs` | `[rail][context]` over the status bar, and the resize gutter |
| `src/view/rail.rs` | The rail |
| `src/view/sessions.rs` | The session list |
| `src/view/conversation.rs`, `prose.rs` | The message type, the virtual list of messages, and a message's markdown |
| `src/view/sheet.rs` | The workspace sheet: header, conversation, composer, content panel |
| `src/view/status.rs` | The status bar |
| `src/view/parts.rs` | The working glyph, the running dot and the toolbar button |
| `src/icons.rs` | Lucide icons, generated |
| `src/tests.rs` | Tests that click, type and read the state with no window |

Sizes are Fleet's CSS pixels at its 14px root font size, taken from the Vue components (in
weave-fleet's `client/`) named at the top of each file.

`Cargo.toml` pins Rune to one commit. Rune is pre-release and its API changes, so move the pin on
purpose (the `rev` appears five times).

### Icons

`src/icons.rs` is generated from the same lucide package Fleet's client uses. To add an icon, add
its name to this command and run it again. It reads the package from a weave-fleet checkout
(after `bun install` in its `client/`):

```sh
python3 tools/gen-icons.py ../weave-fleet/client/node_modules/lucide-vue-next \
  message-square workflow zap chart-column github settings circle-question-mark plus folder-plus \
  search chevron-down chevron-right archive square-terminal octagon-x git-fork ellipsis \
  chart-no-axes-column git-branch terminal file-text pencil paperclip circle-x arrow-up list-todo \
  circle circle-check check x chevron-left chevron-up folder folder-git-2 refresh-cw loader-circle bug \
  copy panel-left-close panel-right-close layout-grid monitor moon-star scroll-text file-search \
  text-search maximize-2 zoom-in zoom-out puzzle power-off download eraser focus git-branch-plus \
  square-dashed-bottom menu undo-2 arrow-up-to-line arrow-down-to-line sun file-down trash-2 \
  app-window book-open camera code globe layers lightbulb lightbulb-off list-checks \
  message-circle-question-mark messages-square monitor-up package-check panels-top-left send server wrench brain history corner-down-right bot arrow-up-right circle-alert star star:filled triangle-alert rotate-cw play arrow-down activity ban circle-stop list-end git-pull-request git-pull-request-draft git-pull-request-closed git-merge shield-alert shield-check pin pencil-line compass eye-off folder-open eye map-pin arrow-left corner-left-up info archive-restore cloud pin-off circle-dashed minus circle-dot repeat external-link sparkles hexagon infinity cable key-round settings-2 users radar gauge sliders-horizontal palette flask-conical image circle-arrow-up coins hourglass moon git-compare folder-tree network minimize-2 file-code file-braces file text-wrap chevrons-down-up chevrons-up-down
```

## Licences

This repository has no licence yet (`docs/PORT.md`, "Open decisions"). What it borrows:

- Rune: MIT.
- Inter (`assets/fonts/InterVariable.ttf`): SIL Open Font License, `assets/fonts/Inter-LICENSE.txt`.
- JetBrains Mono (`assets/fonts/JetBrainsMono-Variable.ttf`): SIL Open Font License,
  `assets/fonts/JetBrainsMono-OFL.txt`.
- The app icon (`assets/icon/`) is made from `assets/weave_logo.png` by `tools/gen-app-icon.py`.
- `winresource` (MIT or Apache-2.0, build time only on Windows, nothing of it ships in the exe) writes the exe's icon
  and version information. It is the small, maintained fork of `winres`, and needs no tool beyond the Windows SDK
  the runner has.
- `alacritty_terminal` (Apache-2.0, a crate dependency, nothing copied): the terminal drawer's parser, grid and scrollback (`src/term/`, `docs/research/terminal.md`). Its pty module comes along unused.
- Lucide icons (`src/icons.rs`): ISC, `tools/lucide-LICENSE.txt`.
