# Component 24: Desktop shell (checklist and pieces)

Planned on 19 November 2026, from weave-fleet `main` at commit `107c794` (read in a shallow clone). `desktop/` was last
changed before that commit. Its four latest changes (#1076-#1078 and the reach dialog's desktop side) first shipped in v0.74.0, not v0.72.0; the checks below ran against v0.72.0 unless a row says otherwise.

Sources (weave-fleet): `desktop/src/main.ts` (MAIN), `server.ts` (SRV), `instance.ts` (INS), `paths.ts` (PA),
`settings.ts` (SET), `shell-env.ts` (ENV), `menu.ts` (MENU), `links.ts` (LNK), `preload.ts` (PRE), `updates.ts` (UPD),
`static/splash.html`, `error.html`, `running-fleet.html`, `electron-builder.config.js`, `scripts/smoke.mjs`; tests
`desktop/test/desktop.test.ts` (DT), `menu.test.ts` (MT), `updates.test.ts` (UT). Behind them: the client's
`lib/desktop.ts` (the bridge as the UI sees it), `composables/use-session-notifications.ts` (desktop notifications, which
Electron shows as the system's), the server's `Endpoints/DesktopEndpoints.cs`, `Desktop/StandardInputWatcher.cs`,
`Program.cs` (`Fleet:Desktop:Enabled`), and the CLI's `scripts/install.sh` / `launcher.sh` (where an installed Fleet lives).

## What Fleet's desktop app is

An Electron window around Fleet's own web client. It owns no UI of its own beyond three static pages (starting, failed,
"Fleet already runs here"), a tray icon, menus and one system notification. Everything else it does is process work:
find the Fleet already running on this computer's data, or start the bundled server in *desktop mode* and stop it on
quit; keep one instance; remember the window; merge the login shell's PATH; update itself.

The native app is itself the client, so the "load the UI in the window" half is already the whole of components 1–23.
This component ports the rest.

## Checked live (19 November 2026)

Against the v0.72.0 server package (it reports 0.73.0 after its own update) with a scratch HOME and data folder, its
own port, started exactly as `server.ts` starts it (`--urls`, `--contentRoot`, the environment of `buildServerEnv`, a pipe
on stdin):

- `/readyz` answered after 0.56 s on this laptop.
- `fleet.instance.json` appeared beside the database with `desktop: true` and the URL; no `requiresToken` key for a
  loopback Fleet (so "missing" must read as false, as INS does).
- `GET /api/desktop/status` → `{"workingSessions":0}`.
- A second server on the same data folder (another port) exited with code **75** (`IN_USE_EXIT_CODE`).
- Closing the first one's stdin stopped it with code 0 in 0.03 s, and it removed its instance file.

So the contract the pieces rely on holds with the released server; no server change is needed.

## What the native app has today

- **Discovery** (`src/fleet/discover.rs`, `src/fleet/instance.rs`): reads `fleet.instance.json` in more places than
  Fleet does (`Fleet__DatabasePath`, `WEAVE_FLEET_DATA_DIR`, `FLEET_DESKTOP_DATA_DIR` or `~/.weave`, the server's own
  `LocalApplicationData/WeaveFleet`, Electron's `desktop-settings.json` port), then port 6262; believes a file only when
  its pid is alive and `/readyz` answers. With nothing found it keeps looking every 3 s and the status bar says "No
  Fleet found (looked in …)". **It never starts a server.** `instance.rs` has no `requiresToken`.
- **This machine's key** (`src/fleet/local_token.rs`): when the Fleet found wants a token, the app reads
  `<db>.machine.json` and adopts it silently (no "Use it" page). Its shortest accepted key is 16; Fleet's desktop app
  takes 32 (`MINIMUM_TOKEN_LENGTH`).
- **`--server` / `FLEET_NATIVE_SERVER`, `--token` / `FLEET_NATIVE_TOKEN`**: connect to a named Fleet.
- **Self-update** (`src/update/`, component 25): its own status-bar item and dialog; replaces itself and restarts
  (`Command::spawn` + `exit(0)` after saving drafts and settings).
- **The window**: `AppSpec::new("Weave Fleet", (1360.0, 880.0), …)`, `min_size` 720×480 (Fleet's). No saved position,
  no single instance, no `on_close` / `on_quit`, no tray, no menus, no notifications. The palette's "Toggle full screen"
  waits on this component (`ui/commands.rs`, `Run::Waiting(24)`).
- **Its own data folder** (`composer::drafts::app_dir`): `FLEET_NATIVE_DATA_DIR`, else `%APPDATA%\fleet-native`,
  `~/Library/Application Support/fleet-native`, `$XDG_DATA_HOME/fleet-native` (`~/.local/share/fleet-native`), with
  `settings.json` (`fleet::settings`) beside the drafts.
- **Links**: Rune's `Tx::open_url` already opens only `http`, `https` and `mailto` and refuses the rest, which is LNK's
  "browser" and "block". The app has no "window" case (it shows Fleet's pages itself).
- **Help menu** (component 9): "Report a problem…", "What's new", Keyboard shortcuts…; the report has no screenshot (09
  row 9.12: Rune can't read its own frame back).
- **Find in conversation**: the key and `State::find_request`, which is what a menu item would call (03 row 6.6).

The pictures in `shots/parity/24-desktop-shell/` show the gap: Electron's three start pages, and what the native app
shows with no Fleet running (the invented data and a red line in the status bar).

## What Rune has for this (pin `f1f54ce8`)

| Need | Rune | Systems |
|---|---|---|
| One instance, second launch brings the window forward | `rune::single_instance(id)`; Rune's default reopen | Windows (named pipe), Linux (socket in `$XDG_RUNTIME_DIR`); macOS does it itself |
| Close the window but keep running | `App::keep_running`, `on_close`, `AppHandle::show/hide/focus/open/quit` | all |
| Save before a quit | `App::on_quit` (Cmd-Q, Dock, logout, Windows shutdown); it can't cancel the quit | all |
| Tray icon with a menu | `rune::tray` (`TraySpec`, `TrayItem`, `TrayEvent::Activate/Menu`) | Windows, Linux (StatusNotifierItem over D-Bus; GNOME needs an extension); **not macOS** |
| Notifications | `rune-notify` (a separate crate in Rune's repository) | macOS (bundled app only), Windows (registers an AppUserModelID under HKCU); **not Linux** |
| Menu bar | `App::menu_item`, `AppHandle::set_menu`, `App::standard_menus` | **macOS only** |
| Window place | `WindowHandle::placement()` (position, size, maximized, display; `None` on Wayland); `WindowOptions { position, physical_size }`, `App::maximized`, `App::full_screen` | all (no position on Wayland) |
| Open a link | `Tx::open_url` (http, https, mailto) | all; no `file:` (opening a folder needs the system's opener) |

No event tells the app the window moved or was resized; a piece polls `placement()` (see piece G).

## Decisions (the owner can overturn)

1. **Which server the app starts.** Fleet's app starts the server it bundles. The native app bundles none yet (component
   25), so it looks, in order: `FLEET_NATIVE_SERVER_BIN` (and `FLEET_DESKTOP_SERVER_BIN`, Fleet's own name, with
   `…_CONTENT_ROOT`); the server bundled beside the app (`<exe dir>/fleet/`, where component 25 will put it; on macOS
   `<bundle>/Contents/Resources/fleet/`); the CLI install (`WEAVE_FLEET_INSTALL_DIR`, else `~/.weave/fleet`, then
   `app/WeaveFleet.Api[.exe]` with `app/` as content root, `install.sh`); Fleet's own desktop app's copy (Linux deb
   `/opt/Fleet/resources/fleet/`, Windows `%LOCALAPPDATA%\Programs\Fleet\resources\fleet\`, macOS
   `/Applications/Fleet.app/Contents/Resources/fleet/` and `~/Applications/…`); last, `fleet` on PATH (after the login
   shell's PATH is merged), as `<its folder>/../app/WeaveFleet.Api`. The first that exists wins.
2. **It runs `WeaveFleet.Api` itself, in desktop mode, never the `fleet` launcher**, as Fleet's app does: the pipe on
   stdin is what stops the server when the app goes, however it goes. Cost: a CLI install's staged update
   (`~/.weave/fleet/update/`) is applied by the launcher only, so it waits until the person next runs `fleet`. Say so in
   the log when an `update/` folder is there.
3. **Same data as everyone**: `FLEET_DESKTOP_DATA_DIR`, else `~/.weave` (PA), which `instance::data_dir` already reads.
   One computer, one Fleet: the app never starts a second server on data a running one holds (exit 75 → find that one).
4. **When it starts one**: only on a normal start with no `--server`/`FLEET_NATIVE_SERVER`, not headless, not `--demo`,
   after one look in every place discovery knows finds nothing answering. `FLEET_NATIVE_START_SERVER=0` turns it off
   (new; for a person who wants to start Fleet themselves). If no server binary is found, today's "No Fleet found"
   notice and the 3 s search stay, with the places it looked for a server added to the words.
5. **The app's own settings** (the port its Fleet used, `useRunningFleet`, the window) go in its own `settings.json`
   (`fleet::settings`), not Electron's `desktop-settings.json`, which belongs to Fleet's app. Discovery keeps reading
   Electron's file for its port as it does now.
6. **Logs**: `logs/fleet-server.log` in the app's own data folder (Electron: `app.getPath("logs")`), the same lines as
   SRV (`── <ISO time> starting <binary> on <url>`, the server's stdout and stderr, `── Fleet exited (code …, signal …)`),
   rolled to `.1` past 10 MB.
7. **The start pages are drawn natively** (splash, error, "Fleet already runs here"), in the window's whole context
   area, at the static pages' sizes, with the theme's tokens: they are HTML with their own colours in Fleet, and the
   native app has no web view for them. Their colours (`#F3F2EF`, `#0d0d10`, `#5B6EC7`, `#6366f1`) are `--main-bg` and
   `--accent` of `theme.rs` already.
8. **Desktop notifications are built here** (component 16's open decision 3, "delivery as its own later piece"; 02 row
   for `session_notification` names this component): `rune-notify` on macOS and Windows (a new dependency from Rune's
   own repository at the same rev: Rune's rev then appears six times, and a pin move moves all six), and on Linux the
   freedesktop `org.freedesktop.Notifications` call over `zbus`, which is already in `Cargo.lock` through Rune's tray (a
   direct dependency, no new crate built). Piece F. If the owner says no, F is skipped and E's "Fleet is still running"
   notice becomes a tray tooltip only.
9. **Menus**: macOS gets Fleet's menu bar (Rune can). Windows and Linux have no menu bar in Rune; Electron hides its
   bar there anyway (`autoHideMenuBar`, Alt shows it). The items found nowhere else (Open Logs Folder, Open Fleet Data
   Folder, Check for Updates…, Quit Fleet) become command-palette entries there. Not Fleet's place for them; flagged.
10. **Single instance is skipped** for headless runs, `--demo`, `--server` and when `FLEET_NATIVE_DATA_DIR` is set
    (scratch runs and CI must run side by side). Its id is `com.pgermishuys.fleet-native`, not Electron's
    `com.pgermishuys.fleet`, so the two apps can run together while Electron is still the installed one.

## Rows

"Native" is what exists on 19 November 2026. Owner: `24 <piece>`, or another component.

### 1. Finding the server to start (PA, decision 1)

| # | Row | Source | Native | Owner |
|---|---|---|---|---|
| 24.1 | Data folder: `FLEET_DESKTOP_DATA_DIR` or `~/.weave`; database `fleet.db`, instance file `fleet.instance.json` beside it | PA `resolvePaths`; DT "shares ~/.weave with CLI installs", "takes overrides for scratch runs" | Done (`instance::data_dir`) | — |
| 24.2 | Server binary: an override (`FLEET_DESKTOP_SERVER_BIN`, content root `FLEET_DESKTOP_SERVER_CONTENT_ROOT` or the binary's folder) | PA; DT "takes overrides for scratch runs" | Done (`desktop::paths`: `FLEET_NATIVE_SERVER_BIN`, then Fleet's `FLEET_DESKTOP_SERVER_BIN`; an override stands alone) | — |
| 24.3 | Packaged: `<resources>/fleet/WeaveFleet.Api`, content root that folder; `.exe` on Windows | PA; DT "uses the bundled server when packaged", "names the Windows binary with .exe" | Done (`<exe dir>/fleet/`, mac `Contents/Resources/fleet`); component 25's pieces A-D bundle the server (closed by component 26 A, 22 November 2026) | 25 (done) |
| 24.4 | Development: the repository's Release build `src/WeaveFleet.Api/bin/Release/net10.0/<rid>/` | PA; DT "uses the repo's Release build in development" | n/a: the native app isn't built inside weave-fleet; the override covers it | — |
| 24.5 | The other installs (decision 1): CLI install, Fleet's desktop app's copy, `fleet` on PATH | native only | Done (`desktop::paths::candidates`: beside the app, CLI install, Fleet's own app, `fleet` on PATH) | — |
| 24.6 | No binary anywhere: "Fleet isn't where the app expects it: <path>" (native: the places it looked) | SRV `launch` | Words done (`paths::not_found_message`). **Open by design** (decision 4): with no server found the old notice and 3 s search stay, so the page isn't shown; the notice names the places | — |

### 2. Starting, watching and stopping it (SRV, MAIN `connect`)

| # | Row | Source | Native | Owner |
|---|---|---|---|---|
| 24.10 | Look first: a Fleet already running on this data is used, never a second (`findRunningFleet`) | MAIN `connect` | Done (discovery, more places than Fleet) | — |
| 24.11 | Environment: the app's own minus `ELECTRON_*`, `URLS`, `ASPNETCORE_URLS`, `DOTNET_URLS`, `ASPNETCORE_ENVIRONMENT` (case-insensitive), plus `ASPNETCORE_ENVIRONMENT=Production`, `Fleet__Host=127.0.0.1` (load-bearing: loopback sign-in), `Fleet__Port`, `Fleet__DatabasePath`, `Fleet__AnalyticsDatabasePath` (`fleet-analytics.db`), `Fleet__DataProtection__KeyPath` (`fleet-keys`, created first), `Fleet__Desktop__Enabled=true`. Native also drops `FLEET_NATIVE_TOKEN` | SRV `buildServerEnv`; DT "runs Fleet in desktop mode…", "drops Electron's variables…" | Done (`desktop::server::server_env`) | — |
| 24.12 | Arguments `--urls http://127.0.0.1:<port> --contentRoot <root>`, working folder the data folder, stdio all piped, no console window on Windows (`windowsHide`: `CREATE_NO_WINDOW`) | SRV `launch` | Done (`desktop::server`: `--urls`, `--contentRoot`, cwd the data folder, piped stdio, `CREATE_NO_WINDOW`) | — |
| 24.13 | Port: the one used last time if free (keeps the client's local storage; for native, its settings) else 6262 else any free one. "Free" = nothing accepts a loopback connect within 1 s **and** a 127.0.0.1 bind succeeds (Windows binds beside a 0.0.0.0 holder) | SRV `pickPort`, `canListen`; MAIN `DEFAULT_PORT`; DT "ports" ×3 | Done (`pick_port`, `can_listen`; preferred ports are a list, the caller passes the remembered one then 6262) | — |
| 24.14 | Ready: `/readyz` every 250 ms (1 s timeout each) for up to 60 s; stops waiting if the process exits | SRV `waitUntilReady`, INS `isReady` | Done (the supervisor looks every 250 ms, 1 s each, 60 s; stops at the exit) | — |
| 24.15 | Ready → remember the port, connect the app to it, marked *owned* | MAIN `onServerEvent` "ready" | Done (`desktop::start::watch`: remembers the port in `settings.json` as `desktopPort`, connects, marks `owned`) | — |
| 24.16 | Exit code 75 (another Fleet took the data first) → look for that Fleet every 500 ms for 30 s and use it; else "Another Fleet is using your data, but it isn't answering." | SRV exit; MAIN "taken" | **Done** (B: `Event::Taken`, then every 500 ms for 30 s the Fleet that took the data is looked for and used; else Fleet's "isn't answering" page; `start_tests` "another fleet that took the data is used once it answers") | — |
| 24.17 | Crash → restart: at once the first time in a minute, then 1 s, 2 s, 4 s; the fifth crash in a minute gives up: "Fleet keeps stopping unexpectedly." | SRV `restartDelay`; DT "restarts" ×3 | Done (`restart_delay`; `Event::Restarting`, then `Failed`) | — |
| 24.18 | Never answered within 60 s while running → "Fleet started but never answered." | SRV | Done (`Event::Failed`) | — |
| 24.19 | Spawn failure → "Couldn't start Fleet: <error>" and a log line `── could not start Fleet: …` | SRV | Done (`Event::Failed`, log line) | — |
| 24.20 | Stop: close stdin (the server's `StandardInputWatcher` shuts down cleanly with its agents), wait 20 s, then SIGTERM, 5 s, SIGKILL (Windows: terminate) | SRV `stop` | Done (`FleetServer::stop`: stdin, grace, SIGTERM, 5 s, kill; Windows: stdin, then terminate) | — |
| 24.21 | The app dying any way at all closes the pipe, so the server stops by itself and frees the lock (`scripts/smoke.mjs` kills the app and checks) | server `StandardInputWatcher`; smoke | **Done** (J, 20 November 2026): the server stops by itself when its input closes; `scripts/smoke-desktop.sh` kills the app with SIGKILL against the released server on Linux, sees the Fleet stop, and starts the app again on the same data. Windows and macOS: the script is for 25's packages to run (Windows needs a PowerShell twin) | 25 (other systems) |
| 24.22 | Stopped only if the app started it; a Fleet it found keeps running after quit | MAIN `before-quit`, `fleet.owned` | Done (`StartState::owned` is only set for a server this app started; quit stops that one, never a found one) | — |
| 24.23 | Log file (decision 6): header line per start, stdout and stderr appended, exit line, rotate to `.1` past 10 MB | SRV `openLog` | Done (`desktop::log`). Difference: each pipe is copied a whole line at a time (Fleet pipes chunks), so a line that reaches its pipe in pieces never has the other pipe's line inside it | — |
| 24.24 | A restart after a crash fires "ready" again with the same URL; the app doesn't reload a window already on it | MAIN "ready" | Done (`watch` skips a second `Ready` for the address it is on) | — |
| 24.25 | Restarting: log `Fleet stopped (exit code N); restarting in M ms.` | MAIN "restarting" | **Done** (B: `start.rs` writes the line to the log) | — |
| 24.26 | Off the UI thread: spawning, waiting, reading output, stopping | AGENTS.md | Done (one supervisor thread; the app reads a channel) | — |

### 3. The start pages (`static/`, MAIN `showPage`, `showError`)

Each page fills the window, centred, on `--main-bg`, 14 px system font (native: the app's font), `prefers-color-scheme`
for the theme (native: the app's theme). Icon `icon.png`.

| # | Row | Source | Native | Owner |
|---|---|---|---|---|
| 24.30 | Splash: 72 px icon, 18 px gap, a 160×3 bar (track `--border`-like, radius 3) with a 40 % accent slider moving 1.2 s ease-in-out (full bar at 50 % with reduced motion), status line `--muted` "Starting Fleet…" | splash.html | Done (`view::start_page::splash`; the sweep is drawn from `State::clock`, eased as CSS `ease-in-out`; reduced motion isn't read: Rune has no such query) | — |
| 24.31 | Error: max 460 wide, gap 14; 40 px icon; h1 18/600 "Fleet didn't start"; the message (`--muted`); "Its log is here:"; the log path in a mono 12 px box (card, border, radius 6, padding 8×10, wraps anywhere); buttons **Try again** (accent) and **Open logs** (13/500, radius 8, padding 7×14) | error.html | Done (`start_page::error`; the icon is Fleet's mark from `weave_logo.png` at 83 % of the square, not `icon.png`) | — |
| 24.32 | Try again: `connect()` again from the top (look, then start) | PRE `retry`, MAIN | Done (`StartState::retry`: splash at once, the whole flow again) | — |
| 24.33 | Open logs: opens the logs folder in the system's file manager | PRE `openLogs` | Done (`start::open_folder`: `xdg-open`, `open` or `explorer` on a thread, the log's folder) | — |
| 24.34 | "Fleet already runs on this computer" card: 470 wide, padding 22×24, radius 12, border; 40 px icon; h1; "<host> runs Fleet in the background (started <d MMM>). The desktop app will use it: the same sessions, harnesses and machines, and it keeps running when you close the window."; detail box "<host> · Fleet <version>" (12.5 px, radius 10, `--main-bg`); **Use it** (accent, autofocus, disabled once pressed) | running-fleet.html | Done (`view::start_page::running_fleet`, scenes `start-running-fleet` and `-light`; the host is the system's (`desktop::start::host_name`), the date is in this computer's zone; **Use it** is focused when the card shows, disabled once pressed). The card is 470 px wide whatever the window, where Fleet's is `max-width: 100%` (Rune has no such width; the window's minimum is wider) | — |
| 24.35 | Splash and error replace the shell; the rail and status bar aren't there in Fleet's (it's a different page) | MAIN | Done (the pages replace the whole window, as in Fleet) | — |

### 4. A Fleet that wants its key (a `fleet node`) (MAIN `useRunning`, `openRunning`, INS)

| # | Row | Source | Native | Owner |
|---|---|---|---|---|
| 24.40 | Instance file's `requiresToken` (true only when exactly `true`; missing = false) | INS `parseInstance`; DT "knows a Fleet that wants its key…" | Done (`Instance::requires_token`, true only for exactly `true`) | — |
| 24.41 | Instance file rules: pid a positive integer, URL loopback `http` only, version default "unknown", trailing `/` trimmed | INS; DT "reads what the server writes", "rejects %s" ×6, "only trusts loopback URLs" | Done (`instance.rs`, with tests) | — |
| 24.42 | First time, the "Fleet already runs" page; **Use it** saves `useRunningFleet: true` and goes on; later starts skip the page | MAIN `useRunning`, `use-running-fleet` | Done (`desktop::start::use_found`: a Fleet whose instance file says `requiresToken` shows the card the first time; **Use it** keeps `useRunningFleet` in `settings.json` and goes on; later starts read the key at once). A Fleet found by port or saved port has no instance file to say so, so it is used as before (this machine's key adopted silently if it asks) | — |
| 24.43 | The key: `<db name>.machine.json` beside the database, `accessToken`, at least 32 characters | INS `machineFileFor`, `parseAccessToken`; DT "is in the machine file…", "is read from what the server saves", "is not taken from %s" ×5, "is read from disk…" | Done (`local_token::machine_file_for`, `read_access_token`, minimum `MINIMUM_TOKEN_LENGTH` = 32) | — |
| 24.44 | Can't read it: "Fleet runs on this computer, but the app can't read its key. The key is in fleet.machine.json beside its database." | MAIN `openRunning` | Done (`local_token::CANT_READ`, shown on the error page; the log path under it is the server log's, as in Fleet) | — |
| 24.45 | Refused: "Fleet runs on this computer, but it didn't accept the key the app read. If you set WEAVE_FLEET_AUTH_TOKEN for it, restart it without that variable." | MAIN; DT "reports a key Fleet turned away…" | Done (`local_token::TURNED_AWAY`: `Outcome::Refused`, for a key Fleet answered with a refusal or didn't answer to) | — |
| 24.46 | Sign-in is `POST /auth/token-login {token}` for Electron's cookie jar | INS `signIn`; DT "signs the window in…" | n/a: native sends the key on every request (`fleet::auth`); the check that it's accepted is a call with it | — |

### 5. The window (MAIN `createWindow`, SET)

| # | Row | Source | Native | Owner |
|---|---|---|---|---|
| 24.50 | 1360×880 by default, minimum 720×480, title "Fleet", the app icon | MAIN | Done (size, minimum, title "Fleet" in `spec()`); the icon comes with 25's packages (done, pieces B-D; closed by component 26 A, 22 November 2026) | 25 (done) |
| 24.51 | Save bounds and maximized 400 ms after a move or resize, not while minimized or full screen; normal bounds when maximized | MAIN `saveBounds` | Done (`desktop::window::Keeper`: read from `placement()` by the poll, which asks Rune for a wake every 250 ms while a window exists; written 400 ms after the last change, and on close and quit; skipped while minimised or full screen; the normal frame is kept with the maximised flag, from `placement().restore` on Windows, else the last normal frame seen) | — |
| 24.52 | Restore saved bounds only if at least 100×100 of them is on a screen now and they're at least 400×300; else default size, centred | SET `restorableBounds`; DT "window bounds" ×3 | Done (`desktop::window::restorable`, DT "window bounds" ×3 plus the 100 px edge, a second screen on the left, and 400 x 300 read in points) | — |
| 24.53 | Open maximized when it was | MAIN | Done (`App::maximized`; the saved frame stays what restoring returns to). Checked by tests only: the virtual display's window manager here has no maximise | — |
| 24.54 | Background before the first paint (`#0d0d10` / `#F3F2EF`) | MAIN | n/a: Rune paints the first frame itself | — |
| 24.55 | `autoHideMenuBar` on Windows and Linux | MAIN | n/a (no menu bar there) | — |
| 24.56 | Spell check in fields | MAIN `spellcheck: true` | Out of reach: Rune's text fields have no spell checker | — |
| 24.57 | Toggle full screen (View menu; the palette entry waits on 24) | MENU `togglefullscreen`; `ui/commands.rs` | Done (palette entry live: `Tx::toggle_full_screen`; macOS View menu item) | 24 H |
| 24.58 | Wayland hides window positions: restore the size only | Rune | **Open**: `placement()` is `None` on Wayland and nothing else in Rune reports the window's size, so nothing is saved there and the window opens at its default (written in `docs/RUNE.md`) | Rune |

### 6. One instance (MAIN)

| # | Row | Source | Native | Owner |
|---|---|---|---|---|
| 24.60 | A second launch quits and brings the first one's window forward (restored, shown, focused) | MAIN `requestSingleInstanceLock`, `second-instance` → `showWindow` | Done (`rune::single_instance("com.pgermishuys.fleet-native")` first thing in a normal start; the second process exits 0 within about 0.1 s and opens no window). The first one's coming forward is Rune's `reopen`; not seen on the virtual display (see below) | — |
| 24.61 | A launch while the window is closed to the tray makes the window again and loads the Fleet (or connects) | MAIN `showWindow` | **Done** (E): the tray's Show Fleet and a click call `AppHandle::open`; the state lives on, so the Fleet is already loaded. A second *launch* showing it is G | 24 E |
| 24.62 | macOS: Dock click (`activate`) shows the window | MAIN `activate` | **Done** (E): `keep_running` and Rune's default reopen show the hidden window; no `on_reopen` hook is needed (CI only: no Mac here) | 24 E (`on_reopen`) |

### 7. Closing and quitting (MAIN `onWindowClose`, `before-quit`)

| # | Row | Source | Native | Owner |
|---|---|---|---|---|
| 24.70 | macOS: closing the window hides it; the app stays in the Dock | MAIN | **Done** (E): `close::on_close(Mac, _)` is `Hide`; the app stays (rules tested for every case; CI only on macOS) | 24 E |
| 24.71 | Windows/Linux, a Fleet the app didn't start: closing quits | MAIN | Done (closing quits) | — |
| 24.72 | Windows/Linux, its own Fleet: ask `GET /api/desktop/status` (2 s timeout; 0 on any failure); sessions working → hide to the tray, else quit | MAIN `onWindowClose`, `workingSessions`; `DesktopEndpoints.cs` (busy or retry, the user's own) | **Done** (E): `fleet::http::desktop_working` (2 s, 0 on any failure) on a thread; `close::after_status`. Difference: Rune has closed the window before `on_close` runs, so for up to 2 s it is simply gone (`docs/RUNE.md`) | 24 E |
| 24.73 | Quitting with its own Fleet and sessions working asks: "A session is still working" / "<n> sessions are still working", detail "Quitting Fleet stops them.", buttons **Quit Fleet** / **Cancel** (Cancel the default) | MAIN `confirmQuit`, `confirmStoppingSessions` | **Done** for quits the app starts (the tray's Quit Fleet; `State::quit_requested` is what the menu and the palette of H will call): `view::quit_dialog`, Cancel the default (Enter and Escape cancel). **Open by design:** a system quit (macOS Cmd-Q, logout) can't be held, so it stops the Fleet unasked; case in `docs/RUNE.md` | 24 E |
| 24.74 | Confirmed: stop its Fleet (24.20), then quit | MAIN | **Done** (E): Quit Fleet quits; `App::on_quit` stops the owned Fleet on a thread (B) | 24 E |
| 24.75 | The updater's restart stops its own Fleet first and waits (else the new process finds a Fleet that is stopping) | MAIN `installUpdate` | **Done** (E): `update::stop_owned_fleet` takes the owned Fleet and waits until it is gone, then the restart | 24 E |
| 24.76 | Updating with sessions working asks: "Updating restarts Fleet, which stops them.", **Update Now** / **Cancel** | MAIN `installUpdate` | Native's updater dialog has no such warning | 9 V (the card), 24 E (the count) |

### 8. Tray (MAIN `hideToTray`)

| # | Row | Source | Native | Owner |
|---|---|---|---|---|
| 24.80 | Icon `tray.png`; menu **Show Fleet**, separator, **Quit Fleet**; a click shows the window | MAIN | **Done** (E): `desktop::tray` (`assets/tray.png`, menu Show Fleet / line / Quit Fleet, click shows the window); Windows: CI only | 24 E |
| 24.81 | Tooltip "Fleet: a session is working" / "Fleet: <n> sessions are working" | MAIN | **Done** (E): `close::tooltip`. The "Fleet is still running" note (`close::still_running`) has its words; its delivery is F | 24 E |
| 24.82 | Showing the window removes the tray icon | MAIN `showWindow` | **Done** (E): `State::window_visible` (from `on_visibility`) takes the icon away however the window came back | 24 E |
| 24.83 | macOS: no tray (the Dock) | MAIN (darwin returns early) | Rune has no macOS tray: not needed | — |
| 24.84 | Linux: shown only where a tray host runs (KDE, most bars; GNOME needs an extension) | Rune `tray.rs` | **Done as a note** (E): with no host the icon never shows and the window can't come back but by a second launch (G); the app only falls back (keeps the window) when `Tray::new` fails, i.e. no session bus. Rune can't tell whether a host runs | 24 E (note it; with no host the window can't come back but by a second launch, 24.60) |

### 9. Notifications

| # | Row | Source | Native | Owner |
|---|---|---|---|---|
| 24.90 | On hiding to the tray: "Fleet is still running", "A session is still working. Quit from the tray icon when you're done." / "<n> sessions are still working. …", silent, if the system can | MAIN `hideToTray` | **Done** (F): `State::post_still_running` as `hide_to_tray` runs, silent; a click on it only shows the window | 24 F |
| 24.91 | Session notifications: when `DesktopNotifications` is "true" and a `session_notification` event comes for a session not in front (open, window visible and focused): title `payload.title` or "Weave Fleet", body `payload.body`, tagged with the session id (one per session) | `use-session-notifications.ts` | **Done** (F): `fleet::notifications` (`on_notification_event`): the preference, not looking (open, visible, focused), title or "Weave Fleet", body, tag = session id; the system replaces a notification of the same tag | 24 F |
| 24.92 | Click: window forward, that session opens | same | **Done** (F): `notification_clicked`: window forward (`Surface::show_window`), session opens | 24 F |
| 24.93 | Opening the session, or the window coming forward on it, takes its notification down | same `clearFor` | **Done** (F): `clear_notification_for_open` on every route change and when the window gains the keyboard or comes on screen | 24 F |
| 24.94 | Fleet asks the browser for permission; native has none to ask (macOS asks on the first post itself) | `requestNotificationPermission`; 16 rows 2.12, 2.13 | **Done** (F) as: native asks for nothing; where the system won't take a post (macOS, not a bundled app) the General row says so (see 24.97). Windows registers the AppUserModelID in `Real::new`; macOS asks on the first post itself | 24 F (the macOS case), 16 |
| 24.95 | Focus is reported to the server only while recap or notifications are on | 02 row 6.6, `presence.rs` `REPORT_FOCUS` | **Done** earlier (16 AQ, `report_focus_wanted`: recap or notifications on, or preferences not read yet); F's test checks it with the switch | 24 F |
| 24.96 | Linux: none from Rune; freedesktop notifications over D-Bus (decision 8) | — | **Done** (F): `desktop::notify` `linux::Bus`, `Notify` / `CloseNotification` / `ActionInvoked` / `NotificationClosed` over zbus on a thread; checked on a session bus (see the pull request) | 24 F |
| 24.97 | macOS: only a bundled app can post (`rune-notify::available()`); an unbundled build posts nothing | Rune | **Done** (F): `Notifier::can_post` is `rune_notify::available()`; unbundled, nothing posts and the General row says "This app can't show them from here." | 24 F, 25 |

### 10. Menus (MENU)

| # | Row | Source | Native | Owner |
|---|---|---|---|---|
| 24.100 | macOS "Fleet": About, **Check for Updates…**, Services, Hide, Hide Others, Show All, Quit | MENU | Check for Updates… done (app menu, after About); About, Services, Hide, Hide Others, Show All, Quit are Rune's own default items (macOS, CI only) | 24 H |
| 24.101 | Windows/Linux "File": **Quit Fleet** | MENU | Done as a palette entry, "Quit Fleet" (decision 9), through E's quit rules | 24 H |
| 24.102 | Edit: Undo, Redo, Cut, Copy, Paste, Select All, then **Find in Conversation** shown with ⌘F/Ctrl+F but the key left to the app (`registerAccelerator: false`; Windows shows "Find in Conversation\tCtrl+F" as text) | MENU; MT ×2 | Done (macOS: Edit → Find in Conversation with ⌘F, key left to the app by `keys_first`; `standard_menus` brings the Edit items) | 24 H (macOS item; `standard_menus` brings Edit) |
| 24.103 | View: Reload, Force Reload, Toggle DevTools | MENU | n/a: no page to reload | — |
| 24.104 | View: Actual Size, Zoom In, Zoom Out | MENU | The palette's zoom entries (component 9) and Appearance's font size (16) | 9, 16 |
| 24.105 | View: Toggle Full Screen | MENU | Done (macOS View menu with ⌃⌘F; palette entry everywhere) | 24 H |
| 24.106 | Window (role): Minimize, Close (and Zoom on macOS) | MENU | Done on macOS (`standard_menus`: Minimize, Close; Zoom is the system's); none elsewhere (no bar) | 24 H (`standard_menus`) |
| 24.107 | Help: **Report a Problem…** (shows the window, opens the report), **Check for Updates…** (not macOS), **Open Logs Folder**, **Open Fleet Data Folder** | MENU, PRE `onReportProblem` | macOS Help: Report a Problem…, Open Logs Folder, Open Fleet Data Folder; elsewhere palette: Report a problem (9), Check for updates, Open logs folder, Open Fleet data folder | 24 H |
| 24.108 | Check for Updates… says in a dialog: "This build of Fleet doesn't update itself." / "You're on the latest version (<v>)." / "Fleet is already checking for updates." / "Fleet <v> is downloading. Fleet lets you know when it's ready to install." / "Couldn't check for updates: <error>"; a found one opens the update card | UPD `updateCheckMessage`; UT "Check for Updates… from the menu" ×3 | Entry done (wakes the checker; opens the card when a build is found; notes "Checking for updates…", "Fleet is already checking for updates.", "This build of Fleet doesn't update itself."); the "latest version" and "Couldn't check" words stay with 25 / 9 V | 25 / 9 V (words); 24 H (the menu entry wakes the checker) |
| 24.109 | Right-click: up to 5 spelling suggestions and "Add to Dictionary" | MENU `showContextMenu` | Out of reach (no spell checker) | — |
| 24.110 | Right-click on a link (http, https, mailto): **Open Link in Browser**, **Copy Link** | MENU | **Done** (I): `ui::text_menu::on_link` on Markdown and rich links (messages, notes, question, failure); drawn on every system, Rune has none for links. Not on terminal links (13) or file links | 24 I |
| 24.111 | Right-click in a field: Cut, Copy, Paste (each enabled by what it can do), separator, Select All | MENU | **Done** (I): macOS and Windows keep Rune's native menu; Linux draws `ui::text_menu` from every field (`text_input` wrapper). Cut and Copy are on with a selection (off for a secret field); Paste is always on (the clipboard can't be read in a handler); the caret does not move to the click | 24 I |
| 24.112 | Right-click on selected text: Copy | MENU | **Done** (I): native on macOS and Windows; Linux draws it for messages (`prose`), the question and failure text. Not yet on tool output, shell commands, the start page's log or the code view (open) | 24 I |
| 24.113 | No trailing separator; nothing to show → no menu | MENU | **Done** (I): `rows` pops trailing separators; no rows, no menu | 24 I |

### 11. Links (LNK)

| # | Row | Source | Native | Owner |
|---|---|---|---|---|
| 24.120 | Fleet's own pages stay in the window | LNK; DT "keeps Fleet's own pages in the window" | n/a (native draws them) | — |
| 24.121 | http, https, mailto open in the system browser | LNK; DT "opens web and mail links in the browser" | Done (Rune `open_url`) | — |
| 24.122 | Everything else is blocked (`javascript:`, `file:`, `vscode:`, not a URL) | LNK; DT "blocks everything else" | **Done** (I): Rune refuses other schemes; `tests_text_menu` ports the three cases ("opens web and mail links in the browser", "blocks everything else"; "keeps Fleet's own pages in the window" is n/a) | 24 I |
| 24.123 | Web views' own navigation | MAIN `will-navigate` | 14 owns the web views | 14 |

### 12. The login shell's environment (ENV)

| # | Row | Source | Native | Owner |
|---|---|---|---|---|
| 24.130 | Before starting Fleet (not on Windows): run `$SHELL` (else `/bin/zsh` on macOS, `/bin/bash` on Linux) `-ilc` with a command that prints each of `PATH`, `SSH_AUTH_SOCK`, `LANG`, `LC_ALL`, `LC_CTYPE`, `HOMEBREW_PREFIX`, `HOMEBREW_CELLAR`, `HOMEBREW_REPOSITORY` between markers; `TERM=dumb`; 5 s timeout; 1 MB cap | ENV; DT "asks the shell with printenv for each name" | Done (`desktop::shell_env::read_login_shell_env`; `$SHELL`, else zsh on macOS and bash elsewhere; Windows asks nothing) | — |
| 24.131 | Read each between its markers, ignoring shell noise; empty values dropped | ENV `parseCaptured`; DT "reads each variable between its markers…" | Done (`parse_captured`) | — |
| 24.132 | PATH: the shell's first, then entries only the app had, no repeats | ENV `mergePath`; DT "puts the shell's PATH first…" | Done (`merge_path`) | — |
| 24.133 | The others only when the app has none | ENV | Done (`apply`) | — |
| 24.134 | Off the UI thread, before the server starts (it can take seconds) | MAIN `start` | Done (`shell_env::spawn` runs it on its own thread and hands the environment back once; `start::connect` calls it through `Config::login_env` before `FleetServer::start`) | — |

### 13. The bridge (PRE, `client/src/lib/desktop.ts`), as native calls

| # | Row | Source | Native | Owner |
|---|---|---|---|---|
| 24.140 | `version`, `platform` | PRE | `version_line()`; report's app line (9) | — |
| 24.141 | `openLogs`, `retry`, `useRunningFleet` | PRE | Done (`Actions::{open_logs, retry, use_running}`) | — |
| 24.142 | `getUpdateState`, `onUpdateState`, `checkForUpdates`, `installUpdate`, `onShowUpdate` | PRE, UPD | Native updater (25) | 25, 9 V |
| 24.143 | `captureWindow` | PRE | Out of reach (09 row 9.12) | — |
| 24.144 | `onReportProblem`, `onFindInConversation` | PRE | Done (`State::menu_run`: report_open, find_request) | 24 H |

### 14. Updates and packages (UPD, `electron-builder.config.js`)

| # | Row | Source | Native | Owner |
|---|---|---|---|---|
| 24.150 | Update mode: install on Windows and AppImage, notify on macOS and the .deb, off unpackaged or `FLEET_DESKTOP_UPDATES=off`; check after 10 s and every 4 h; `nextState` rules | UPD; UT "update mode" ×3, "update state" ×7 | Native updater: every 4 h, its own rules | 25 |
| 24.151 | Installers with the server inside (`resources/fleet`), app id `com.pgermishuys.fleet`, Linux `StartupWMClass: Fleet`, category Development | builder config | None | 25 |
| 24.152 | Smoke test of a packaged app: start on scratch data, Fleet answers, kill the app hard, Fleet stops and frees the lock | `scripts/smoke.mjs` | **Done** (J): `scripts/smoke-desktop.sh <app>` is `smoke.mjs` in bash; passes on Linux against the debug build and the released v0.72.0 server | 25 (run it on each package; a Windows twin) |

### 15. Deep links

Fleet's desktop app registers no URL scheme and handles none (`setAsDefaultProtocolClient`, `open-url` appear nowhere in
weave-fleet), so there is nothing to port. Rune has `App::on_open_url` should Fleet add one. The only place a scheme
would come in is a clicked Windows toast after the app quit (`rune-notify`'s activation scheme): piece F leaves that out
(a click while the app runs reaches it without a scheme).

## Out of reach, and why

- Spell check and its right-click suggestions (24.56, 24.109): Rune's text fields have no spell checker.
- `captureWindow` (24.143): Rune can't hand a running app its frame (09 row 9.12).
- Reload, Force Reload, DevTools (24.103): there is no page.
- macOS tray (24.83): not needed (the Dock); Rune has none anyway.
- Cancelling a system quit to ask first (part of 24.73): Rune's `on_quit` can't cancel.

## OS-specific parts and how each is checked

| Part | Windows | macOS | Linux |
|---|---|---|---|
| Server binary name and places (A) | `.exe`, `%LOCALAPPDATA%\Programs\Fleet` | `.app` bundle paths | `/opt/Fleet`, `~/.weave/fleet` |
| No console window for the server (A) | `CREATE_NO_WINDOW` | — | — |
| Stopping (A) | no SIGTERM: stdin, then `TerminateProcess` | SIGTERM, SIGKILL | SIGTERM, SIGKILL |
| Port probe (A) | the 0.0.0.0 case matters here | — | — |
| Login shell (C) | skipped | `/bin/zsh` default | `/bin/bash` default |
| Close rules (E) | tray | hide, Dock | tray (needs a tray host) |
| Tray (E) | `Shell_NotifyIcon` | none | StatusNotifierItem over D-Bus |
| Notifications (F) | toasts, AppUserModelID registered under HKCU | bundled app only | D-Bus freedesktop |
| Menus (H) | palette entries | menu bar | palette entries |
| Single instance (G) | named pipe | the system | socket |
| Window position (G) | yes | yes | X11 yes, Wayland size only |

This machine runs Linux (X11 under Xvfb, no tray host, no notification daemon by default). **Windows and macOS are
checked by CI only** (build, clippy, the headless tests, which use a fake server process on all three) unless a piece runs
on taron (Windows) or another Mac by hand; each piece's pull request says which. The tray and notifications can't be
checked headless anywhere: Linux's can be checked with a session D-Bus and a notification daemon on Xvfb (`dbus-run-session`,
a test `org.freedesktop.Notifications` listener), Windows' only on a real desktop.

## Tests to port

| Fleet test file | Cases | Where |
|---|---|---|
| `desktop/test/desktop.test.ts` "paths" | 5 | A (4; "uses the repo's Release build in development" n/a) plus native's places |
| "instance file" | 3 (+6 rejects) | Done in `instance.rs` except "knows a Fleet that wants its key…" (D) |
| "the key of the Fleet running here" | 5 (+5) | D ("signs the window in…" n/a; "reports a key Fleet turned away…" as a call that's refused) |
| "links" | 3 | I |
| "server environment" | 2 | A |
| "restarts" | 3 | A |
| "ports" | 3 | A |
| "window bounds" | 3 | G |
| "login shell environment" | 3 | C |
| `menu.test.ts` | 2 | H (macOS item with the key left to the app; the palette entries on the others) |
| `updates.test.ts` | 13 | 25 / 9 V (native's updater is not Electron's) |
| `client/.../use-session-notifications.test.ts` | 8 | F (adapted: "the browser has not granted permission" becomes "the system can't post") |

Process tests (A, B) need a server that starts, answers `/readyz`, exits 75, crashes or stops when stdin closes, on all
three systems with no Fleet installed: **the test binary runs itself** as the fake (`std::env::current_exe()` with a
`#[test]` that acts as the server when `FLEET_NATIVE_FAKE_SERVER=<mode>` is set, binding the `--urls` port and reading
stdin to its end), so no script, Python or .NET is needed on CI. Fake HTTP reads go through `crate::test_http`.

## The pieces

In build order, one pull request each; each starts from this file alone. Pictures: the native app beside Electron's
own pages (`shots/parity/24-desktop-shell/electron-*.png` were taken from `desktop/static/` with Playwright at 1280×760,
both colour schemes, reduced motion) or, for behaviour with no Fleet equivalent picture, the native app showing it in a
real window under Xvfb. Live checks use a scratch data folder (`FLEET_DESKTOP_DATA_DIR` and `FLEET_NATIVE_DATA_DIR` under
the scratch folder, HOME there too), the released server binary, and their own ports; never the owner's Fleet, never
6262, and stop only processes the check started.

### Piece A: start its own Fleet: the server process (first: the owner's daily use) — **done, 20 November 2026**

**Built:** `src/desktop/{mod,paths,server,log,tests}.rs` and `examples/fake_fleet.rs`; `lib.rs` has `pub mod desktop` only (public so nothing is dead code until B wires it); `test_http.rs`'s fake-server guard lists `desktop/server.rs` (it binds only to find a free port). **Differences from the plan:** the fake server is a separate std-only program compiled by the tests with `rustc` on their first start, not the test binary itself: libtest refuses the `--urls` argument, and `cargo test` builds no example (see `docs/RUNE.md`). The live check is an `#[ignore]`d test (`live_server_starts_answers_and_stops_with_the_input`): against the released v0.72.0 server on a scratch HOME and data folder it reached `/readyz`, wrote an instance file with `desktop: true`, answered `/api/desktop/status`, and stopped in 0.03 s when its input closed, with the instance file removed. **Not here:** the remembered port (B keeps it in `settings.json` and passes it in `Options::preferred_ports`), the login shell's PATH (C), the Windows and macOS runs (CI only; `CREATE_NO_WINDOW`, terminate and the `.app` places are written but not run by hand).

- **Rows:** 24.2, 24.3 (the path), 24.5, 24.6 (words), 24.11–24.14, 24.16–24.20, 24.23, 24.25, 24.26.
- **Files:** new `src/desktop/mod.rs`, `src/desktop/paths.rs` (finding the binary, decision 1; pure, takes `get` and an
  `exists` check), `src/desktop/server.rs` (`FleetServer`: start, events `Ready{url,port}`, `Taken`, `Restarting{delay,
  exit}`, `Failed(message)` over a channel; stop; `pick_port`, `can_listen`, `restart_delay`, `server_env`), `src/desktop/log.rs`
  (open with rotation, the header and exit lines), `src/desktop/tests.rs`; `lib.rs` gets `mod desktop` only.
- **Depends on:** nothing. No Rune in it.
- **Tests:** DT "paths" (4), "server environment" (2), "restarts" (3), "ports" (3, the last on every system), plus:
  native places in order; spawn → ready; exit 75 → `Taken`; a crash → restart, five → `Failed`; never answers →
  `Failed` (short timeout); stdin closed → the fake exits and `stop` returns quickly; the log has its header, the fake's
  output and the exit line; rotation past the limit (small limit in the test). Self-running fake, as above.
- **OS:** `CREATE_NO_WINDOW` and terminate on Windows, signals elsewhere (`libc` is already a dependency): CI only for
  Windows and macOS.
- **Pictures:** non-visual. The pull request shows the app still draws the same (the `no-fleet-found` scene, both
  themes) and the log of a live run against the released server on a scratch folder (paths replaced).
- **About:** 600 lines with tests.

### Piece B: start its own Fleet: in the app, the splash and the error page — **built, 20 November 2026**

**Built:** `src/desktop/start.rs` (`Config`, `begin`, `StartState`, `Phase`), `src/view/start_page.rs`, `Sidebar::desktop_port`, `FleetServer::stopper` (a shareable handle: the server itself holds a `Receiver`, which isn't `Sync`), scenes `start-splash`/`start-error` (+ `-light`), `src/desktop/start_tests.rs` (11 tests). **Differences:** the owned server is a `Stopper` in `StartState`, not the whole `FleetServer`; quitting closes its input and waits on a thread, with `rune::exit_deadline(25 s)`; with no server binary found the old 3 s search stays (decision 4) and the notice names the places, so row 24.6's page isn't shown. **Open then, done in J:** the live check (Xvfb, the released server, SIGKILL of the app) is `scripts/smoke-desktop.sh` and the live run below; Windows and macOS stay CI only. Fleet's own pictures are the reduced-motion ones in this folder.

- **Rows:** 24.6 (page), 24.15, 24.16 (wiring), 24.21, 24.22, 24.24, 24.30–24.33, 24.35, 24.141 (`openLogs`, `retry`),
  24.152 (as a live check).
- **Files:** `lib.rs` `start_desktop` (one look; none → start, decision 4; `--server`, headless, `--demo`,
  `FLEET_NATIVE_START_SERVER=0` never start; `Taken` → look every 500 ms for 30 s), `State` gets `start: StartState
  { Idle, Starting(status), Failed{message, log}, Owned(FleetServer handle) }`, the port remembered in `fleet::settings`;
  new `src/view/start_page.rs` (splash and error, drawn over the whole window as Fleet does); stop on quit with
  `App::on_quit` (owned only: close stdin, wait up to 20 s off the UI thread, then `rune::exit_deadline`); Open logs
  through the system's opener on a thread (`xdg-open`, `open`, `explorer`); scenes `start-splash`, `start-error` and
  their `-light` twins.
- **Depends on:** A.
- **Tests:** a headless start with a fake server binary (the self-run fake via `FLEET_NATIVE_SERVER_BIN`) reaches the
  shell and marks the Fleet owned; a failing fake shows the error page with Fleet's words; Try again starts again;
  `--server` never starts one; quit stops an owned one and leaves a found one; the splash's slider follows the app's clock.
- **Live check (Linux, Xvfb):** a scratch data folder with nothing running: the window shows the splash, then the
  scratch server's (empty) list; `fleet.instance.json` says `desktop: true`; quit → the server is gone and the instance
  file removed; start again, kill the app with SIGKILL → the server stops by itself within a second and a new start
  gets the lock (smoke.mjs's check).
- **Pictures:** native splash and error beside `electron-splash-*` and `electron-error-*`, both themes, 1280×760; the
  window under Xvfb showing the scratch Fleet it started.
- **About:** 500 lines.

### Piece C: the login shell's environment

- **Rows:** 24.130–24.134.
- **Files:** new `src/desktop/shell_env.rs`; `server.rs` takes the merged environment; B's start waits for it.
- **Depends on:** A (B if it lands first: then the call goes where B starts the server).
- **Tests:** DT "login shell environment" (3); a fake shell (a script on Unix) printing noise and markers; Windows skips.
- **OS:** macOS (zsh default) checked by CI only.
- **Pictures:** non-visual: the app still draws the same; the started server's PATH from a live run (`/proc/<pid>/environ`,
  invented folders) shown as text in the description.
- **About:** 200 lines.

**Built (20 November 2026):** `src/desktop/shell_env.rs` (`capture_command`, `parse_captured`, `merge_path`, `shell_from`, `read_login_shell_env`, `apply`, `merged`, `merged_with`, `spawn`) and `tests_shell_env.rs`: DT's three "login shell environment" tests under their names, the rest of the rows as tests, and the shell tests against a fake shell script (Unix; Windows runs the parsing tests and asks no shell). **Differences:** a shell that runs past 5 s or prints past 1 MB is stopped and what it had printed so far is parsed (Node does the same); `merged` is called with the app's environment as a list, and `FleetServer`'s `Options::env` takes its result, so `server.rs` is unchanged. **Checked live:** `live_server_gets_the_login_shells_path` (ignored; run by hand) merged a fake shell's invented PATH and LANG, started the released v0.72.0 server with them and read its `/proc/<pid>/environ`: the shell's folder came first and LANG was set. `start.rs` (B) merges it on its own start thread (`Config::login_env`, default `shell_env::merged`; its tests give a stand-in so no real shell is asked), and the stand-in server prints its `PATH` so a test sees it arrive. `spawn` is not used by it (the start thread already is off the UI thread). **Open:** macOS (zsh) and Windows by CI only.

### Piece D: a Fleet that wants its key — **built, 21 November 2026**

**Built:** `Instance::requires_token`; `local_token` (minimum 32, `machine_file_for`, `read_access_token`, `adopt_required`, `Outcome::Refused`, Fleet's two messages); `Sidebar::use_running_fleet`; `Phase::RunningFleet` and `use_found`/`open_running` in `desktop::start`; the card in `view::start_page`; scenes `start-running-fleet` (+ `-light`). **Differences:** the card shows only for a Fleet found through its instance file (the only place `requiresToken` is written); the native app sends the key on every request, so there is no cookie sign-in (row 24.46) and "signed in" means Fleet answered a call carrying the key. **Tests:** the cases of "instance file" and "the key of the Fleet running here" under their names, and the flow against `test_http` fakes (card, Use it, next start, can't read, turned away). **Live check (Linux, Xvfb, 20 November 2026):** the released v0.72.0 server on a scratch folder with `Fleet__Auth__RequireToken=true` (it answers 401 to a call with no key). That release writes no `requiresToken` into its instance file (the source on `main` does), so the key was added to the scratch file by hand, as a newer server writes it. The card showed; **Use it** (a click) signed in with the machine file's key and the Fleet's shell loaded; the choice was in `settings.json`; the next start showed no card; with the machine file unreadable (`chmod 000`) the error page showed Fleet's "can't read its key" words. **Open:** Windows and macOS pictures (CI on `main`); the "turned away" page was checked by test only.


- **Rows:** 24.34, 24.40, 24.42–24.45, 24.141 (`useRunningFleet`).
- **Files:** `src/fleet/instance.rs` (`requires_token`), `src/fleet/local_token.rs` (minimum 32; the outcomes carry
  Fleet's two messages), `src/view/start_page.rs` (the "already runs" card; host name from the system), `lib.rs` (ask once,
  `useRunningFleet` in `fleet::settings`); scenes `start-running-fleet` and `-light`.
- **Depends on:** B.
- **Tests:** DT "knows a Fleet that wants its key…", "is in the machine file…", "is read from what the server saves",
  "is not taken from %s" (5, with the half-written key at 31 characters), "is read from disk…", "reports a key Fleet turned
  away…" (against a `test_http` fake answering 401); Use it saves the choice and the next start skips the page.
- **Live check:** a scratch server started with `--require-token` (`Fleet__Auth__RequireToken=true`): the card, Use it, the
  list; then the machine file made unreadable → the error page's words.
- **Pictures:** the card beside `electron-running-fleet-*`, both themes.
- **About:** 400 lines.

### Piece E: closing, quitting and the tray — **built, 20 November 2026**

**Built:** `src/desktop/close.rs` (the rules, `CloseState`, the flow on `State`), `src/desktop/tray.rs` (`Real`: the window and the icon through Rune's handle and tray, made on a thread), `src/view/quit_dialog.rs`, `fleet::http::desktop_working`, `update::stop_owned_fleet`, scenes `quit-dialog` (+ `-light`, `-one`), `src/desktop/close_tests.rs`. **Open:** the tray menu and the "Fleet is still running" note can't be pictured here (no tray host, no notification daemon; the note is F); 24.76's count for the updater's warning (9 V's card); Windows and macOS by CI only. Fleet's own message box is the system's, so its picture is its words.

- **Rows:** 24.61, 24.62, 24.70, 24.72–24.75, 24.80–24.82, 24.84.
- **Files:** `lib.rs` (`keep_running`, `on_close`, `on_reopen`, `on_quit`), new `src/desktop/close.rs` (the rules as a
  pure function of system, owned, working count), `src/desktop/tray.rs` (Rune `tray`; `tray.png` copied from weave-fleet
  with its licence row, decoded with the `png` crate Rune already brings), the quit dialog with the kit's dialog
  (`src/view/quit_dialog.rs`), `GET /api/desktop/status` in `fleet/http.rs` on a thread, `src/update/install.rs` stops an
  owned Fleet before the restart.
- **Depends on:** B.
- **Tests:** the close rules for each system × owned × working; the status call with a `test_http` fake (a number, a
  missing field → 0, an error → 0); the dialog's words for 1 and 3 sessions, Cancel the default; tray menu ids → actions;
  the updater waits for the stop.
- **OS:** Windows tray and macOS hide-on-close: CI only (headless tests of the rules) unless run on taron / a Mac; Linux
  tray checked under Xvfb with a session D-Bus and a StatusNotifierWatcher if one can be had, else said so.
- **Pictures:** the quit dialog beside Electron's message box (a picture of it from Fleet's app on taron if possible,
  else its words), both themes; the tray menu if a host can draw it.
- **Rune:** write the "can't cancel a system quit" case into `docs/RUNE.md`.
- **About:** 600 lines.

### Piece F: notifications — **built, 21 November 2026**

**Built:** `src/desktop/notify.rs` (`Notifier`, `Real`: `rune-notify` on macOS and Windows, the freedesktop protocol over `zbus` on a thread of its own on Linux; `NotifyState`; a test `Recorder`), `src/fleet/notifications.rs` (`looking_at`, `on_notification_event`, `clear_notification_for_open`, `notification_clicked`, `post_still_running`), `src/tests_notifications.rs` (the eight `use-session-notifications.test.ts` cases under their names, plus the title default, the click, the tray note, the Settings line). The General row's line "This app doesn't show them yet." is gone; it now shows only where the system won't take a post. **Open:** the toast's icon on Windows (`rune-notify` wants a file path; the app has the PNG only embedded) and a real toast or banner on Windows and macOS: CI builds only. **New dependencies:** `rune-notify` (Rune's own crate, MIT, a sixth place for Rune's rev), and `zbus`, `tokio`, `futures-util` named directly on Linux at versions Cargo.lock already holds, so no crate was added.

- **Rows:** 24.90–24.97.
- **Files:** `Cargo.toml` (`rune-notify` at Rune's rev, a sixth rev line; `zbus` as a direct dependency at the version in
  `Cargo.lock`), new `src/desktop/notify.rs` (post, take down, clicks back as `AppHandle::update`; macOS/Windows through
  `rune-notify`, Linux through `org.freedesktop.Notifications` `Notify` / `CloseNotification` / `ActionInvoked`),
  `src/fleet/notifications.rs` (Fleet's rules: the preference, in front or not, tag per session), `fleet/presence.rs`
  (`REPORT_FOCUS` from the two preferences), E's "Fleet is still running".
- **Depends on:** E, decision 8, 16 piece D (the switch).
- **Tests:** the eight `use-session-notifications.test.ts` cases adapted, against a recording notifier; 02 row 6.6's focus
  rule.
- **OS:** Windows' AppUserModelID registration (HKCU) and macOS's bundle-only posting: CI builds only; a real post on
  Linux checked with a D-Bus listener under `dbus-run-session`.
- **Pictures:** none of the system's notification can be taken headless: the description shows the D-Bus call it made
  (as text) and the app's state after a click (the session open), both themes.
- **About:** 600 lines.

### Piece G: one instance and the window's place

- **Rows:** 24.50 (title "Fleet"), 24.51–24.53, 24.58, 24.60.
- **Files:** `main.rs`/`lib.rs` (`rune::single_instance` before the window, decision 10), new `src/desktop/window.rs`
  (`restorable_bounds`, the saved frame in `fleet::settings` under `window`, polled from `placement()` at most every 400 ms
  while it changes, and saved on close and quit), `spec()` takes the saved size and position.
- **Depends on:** nothing of this component (can run beside A–F if a track is free).
- **Tests:** DT "window bounds" (3); the id rule (headless, `--demo`, `--server`, `FLEET_NATIVE_DATA_DIR` skip it); a
  saved maximized window opens maximized.
- **OS:** Wayland size-only; Windows named pipe and macOS: CI only.
- **Pictures:** two launches under Xvfb: the second exits and the first comes forward (a short video or two pictures);
  the window reopened where it was left.
- **About:** 350 lines.

**Built (20 November 2026):** `src/desktop/window.rs` (`Frame`, `restorable`, `opening`, `Keeper`, `single_instance_wanted`, `INSTANCE_ID`; no Rune in it), `src/window_place.rs` (the thread-local window handle, `State::keep_window`, `flush_window`), `Sidebar::window`/`save_window` (`window` in `settings.json`), `tests_window.rs`. Not a headless or `--demo` or `--server` run, and not one with `FLEET_NATIVE_DATA_DIR` set (decision 10). The saved frame is read once at start (a few hundred bytes, before the event loop) and the factory of `rune::run_windows_with_args` checks it against the displays; headless runs keep `App::run`. **Differences:** Electron saves on move and resize events; here the poll reads the window (a wake every 250 ms while it exists, a look at most every 200 ms) and saves 400 ms after the last change. Sizes are physical pixels (Rune's own space), "at least 400 x 300" is read in points with the saved scale. Screens are Rune's `Display` (the whole display, Electron uses the work area). **Checked by hand (Linux, Xvfb + openbox, scratch HOME and runtime folder):** the window moved to 300,180 at 1000x640 was saved in `settings.json` and a new start opened there (`shots/parity/24-desktop-shell/g-*.png`); a second launch exited 0 in 60 to 430 ms, printed nothing and no second window appeared. **Not seen:** the first window coming forward from behind or minimised: the display's window manager here did not act on iconify or key requests, and an unmapped window is not remapped by Rune's reopen. **Open:** Wayland (above); maximised, Windows' named pipe and macOS by CI and tests only.

### Piece H: menus

- **Rows:** 24.57, 24.100–24.102, 24.105–24.108 (the entry), 24.144.
- **Files:** `lib.rs` (`standard_menus`, `menu_item`s on macOS), `src/ui/commands.rs` (the palette entries of decision 9;
  "Toggle full screen" made live with `App::full_screen` / a `Tx` call), open the logs and data folders with B's opener.
- **Depends on:** B (the logs folder), E (Quit Fleet goes through the quit rules).
- **Tests:** MT's two cases adapted (the macOS item shows ⌘F and leaves the key to the app; on the others the palette
  has the entries); each item's action (report opens, find opens, folders asked to open, checker woken).
- **OS:** the menu bar is macOS only: CI builds it; no picture from this machine.
- **Pictures:** the palette with the new entries beside Electron's Help menu (Fleet's words), both themes.
- **About:** 300 lines.

**Built (20 November 2026):** `src/desktop/menu.rs` (the macOS list as data, the palette's four entries, Fleet's words), `src/menu_bar.rs` (`State::menu_run`, `MenuState` for the folders and the opener, `install` which adds `standard_menus` and the items on macOS only), `src/desktop/menu_tests.rs`. **Differences:** Electron's Reload, Force Reload and DevTools have no page here; zoom stays the palette's; "Open Fleet Data Folder" opens Fleet's data (`~/.weave`, as PA), "Open Logs Folder" the app's own `logs/`. "Find in Conversation" uses `keys_first` (the window sees ⌘F before the menu), the nearest Rune has to `registerAccelerator: false`. **Checked by hand (Linux):** the palette entries drawn (`shots/parity/24h-menus/`) beside Fleet's own client's palette for the same searches. **Not seen anywhere:** the macOS menu bar (Rune builds it on macOS only; CI builds it and the headless tests read the list), and Electron's native Help menu (Playwright can't draw a system menu, so Fleet's words are in the pull request as text).

### Piece I: the right-click menu on text, and the link rule's test

- **Rows:** 24.110–24.113, 24.122 (test).
- **Files:** a kit menu on text fields, the composer and selectable text (`src/ui/text_menu.rs`), wired where the app
  draws fields and links.
- **Depends on:** nothing (kit, component 1).
- **Tests:** each case's entries (field with and without a selection, read-only selection, a link, nothing → no menu);
  LNK's three cases against what the app hands `open_url`.
- **Pictures:** the menu over a field and over a link beside Fleet's in Electron (if taron can take one, else its words),
  both themes.
- **About:** 350 lines.

**Built (21 November 2026):** `src/ui/text_menu.rs` (`rows` is Fleet's `showContextMenu` as data; `text_input` wraps Rune's and adds the handler on Linux; `selectable`, `on_link`), `Open::TextMenu`, `Ui::text_menu`, four scenes (`text-menu-field`, `text-menu-link`, each with its light twin), `tests_text_menu.rs` (13 cases: each menu's rows, the drawn menu, Cut/Copy through the composer, and LNK's cases against `take_url_requests`). **Differences:** a native menu is not Electron's (Rune's own on macOS and Windows); the Linux one is Fleet's menu look. Paste is always enabled. The caret doesn't move to the click, and the menu has no spelling rows (24.109). **Checked by hand (Linux, lavapipe):** the live menu over the composer with a selection (`shots/parity/24i-text-menu/live-field.png`) and the four scenes, both themes. **Not seen:** Electron's own menus (an OS menu, Playwright can't draw it; Fleet's words are in the pull request as its source), macOS and Windows (CI only; they keep Rune's native menus). **Open:** Copy on tool output, shell commands, the start page and the code view; terminal links.

### Piece J: the sweep — **done, 20 November 2026**

**Component 24 is done.** What this piece did: read every row against the code on `main`; accounted for every desktop test of Fleet (below);
wrote `scripts/smoke-desktop.sh` (24.152); walked the app live on Linux, on a scratch data folder, the released v0.72.0 server and its own
ports, and put the start pages beside Electron's once more. Pictures: `shots/parity/24j-sweep/` (`start-pages-dark.png`,
`start-pages-light.png` are the native scenes beside Fleet's own pages, reduced motion as before; `1-` to `6-` are the live run).

#### Every row that isn't plainly done

Rows not listed here are **Done** in the tables above, with the piece named there.

| Row | State | Why, and who |
|---|---|---|
| 24.3 | Done | The path is done and component 25's piece A puts the server inside the packages (closed by component 26 A, 22 November 2026). |
| 24.4 | n/a | The app isn't built inside weave-fleet; `FLEET_NATIVE_SERVER_BIN` covers it. |
| 24.6 | Open by design | With no server found anywhere the old notice and 3 s search stay (decision 4); Fleet's "isn't where the app expects it" page isn't shown. |
| 24.30 | Done bar one thing | Reduced motion isn't read: the bar always slides. (Rune has the query now, `cx.system_reduced_motion()`; wiring it is open.) |
| 24.34 | Done bar one thing | The card is 470 wide whatever the window (Fleet: `max-width: 100%`); the minimum window is wider. Native's **Use it** shows no focus ring; Fleet's autofocus does (look at `start-pages-*.png`). |
| 24.46 | n/a | Native sends the key on every request; there's no cookie to sign in. |
| 24.50 | Done | The app icon comes with 25's packages (pieces B-D: `assets/icon/`, the Linux, Windows and macOS bundles; closed by component 26 A, 22 November 2026). |
| 24.53 | Tests only | Maximised: the virtual display's window manager here has no maximise. |
| 24.54, 24.55 | n/a | Rune paints the first frame itself; there's no menu bar on Windows and Linux. |
| 24.56, 24.109 | Out of reach | Rune's text fields have no spell checker. **Owner: Rune.** |
| 24.58 | Open | Wayland: `placement()` is `None` and nothing else reports the size, so nothing is kept there (`docs/RUNE.md`, piece G). **Owner: Rune.** |
| 24.60 | Done bar one thing | The first window coming forward from behind or minimised was not seen under the bare virtual display (no window manager acts on it); seen from the tray (`4-live-second-launch-forward.png`). |
| 24.76 | Open | The updater's "Updating restarts Fleet, which stops them." warning needs the working count in the update card. **Owner: 9 V.** |
| 24.84 | A note | With no tray host the icon never shows; the window then comes back only by a second launch. Rune can't tell whether a host runs. |
| 24.100, 24.102, 24.105, 24.106 | CI only | The macOS menu bar is built by Rune on macOS only; the headless tests read the list. |
| 24.103 | n/a | There is no page to reload. |
| 24.104 | Elsewhere | Zoom is the palette's (9) and Appearance's font size (16). |
| 24.108 | Open (words) | "You're on the latest version" and "Couldn't check" stay with the updater. **Owner: 25 / 9 V.** |
| 24.112 | Open (small) | Copy on selected text is drawn for messages, the question and the failure text; not yet on tool output, shell commands, the start page's log or the code view; terminal links are 13's. No piece owns it: a follow-up with `ui::text_menu::selectable`. |
| 24.120 | n/a | Native draws Fleet's own pages. |
| 24.123 | Elsewhere | Web views' own navigation is 14's. |
| 24.142, 24.150, 24.151 | Elsewhere | The updater's states and modes, the installers with the server inside, the app id, `StartupWMClass`. **Owner: 25** (native's updater is not Electron's). |
| 24.143 | Out of reach | Rune can't read its own frame back (09 row 9.12). |
| 24.152 | **Done** | `scripts/smoke-desktop.sh`; Windows needs a PowerShell twin (25). |

Out of reach, as before: cancelling a system quit to ask first (macOS Cmd-Q, logout: Rune's `on_quit` can't cancel), and a Mac or Windows
look at the tray, the toasts and the menu bar: **Windows and macOS are checked by CI only** (build, clippy, the headless tests with their fake
server process on all three).

#### Fleet's desktop tests, all accounted for (45 cases)

Fleet's names are in `desktop/test/*.ts`; native's are the same sentence in snake case.

| Fleet case | Native test (file) |
|---|---|
| paths: shares ~/.weave with CLI installs; uses the bundled server when packaged; takes overrides for scratch runs; names the Windows binary with .exe | `shares_the_weave_folder_with_cli_installs`, `uses_the_server_bundled_beside_the_app`, `takes_overrides_for_scratch_runs`, `names_the_windows_binary_with_exe` (`desktop/tests.rs`) |
| paths: uses the repo's Release build in development | n/a (24.4) |
| instance file: reads what the server writes; knows a Fleet that wants its key; only trusts loopback URLs | `parses_a_valid_instance_file`, `knows_a_fleet_that_wants_its_key_but_only_when_it_says_so_exactly`, `rejects_a_remote_url` with `accepts_localhost_and_ipv6_loopback` (`fleet/tests/instance.rs`) |
| the key: is in the machine file; is read from what the server saves (and "is not taken from" ×5); is read from disk, null when missing; reports a key Fleet turned away | `the_key_is_in_the_machine_file_beside_the_database`, `the_key_is_read_from_what_the_server_saves`, `the_key_is_not_taken_from_what_is_not_a_key`, `the_key_is_read_from_disk_and_nothing_when_the_file_is_missing`, `a_key_fleet_turned_away_is_reported_with_its_own_words` (`fleet/tests/local_token.rs`) |
| the key: signs the window in by posting it to the Fleet's login | n/a (24.46) |
| links: opens web and mail links in the browser; blocks everything else | `opens_web_and_mail_links_in_the_browser`, `blocks_everything_else` (`tests_text_menu.rs`) |
| links: keeps Fleet's own pages in the window | n/a (24.120) |
| server environment ×2 | `runs_fleet_in_desktop_mode_on_the_shared_data_directory`, `drops_electrons_variables_and_inherited_listen_urls` |
| restarts ×3 | `restarts_straight_away_after_the_first_crash_then_backs_off`, `gives_up_after_five_crashes_in_a_minute`, `forgets_crashes_older_than_a_minute` |
| ports ×3 | `keeps_the_preferred_port_when_its_free`, `picks_another_port_when_the_preferred_one_is_taken`, `picks_another_port_when_the_preferred_one_is_held_on_all_addresses` |
| window bounds ×3 | `restores_bounds_that_are_still_on_a_screen`, `drops_bounds_on_a_monitor_thats_gone`, `drops_bounds_too_small_to_use` (`desktop/window.rs`) |
| login shell environment ×3 | `reads_each_variable_between_its_markers_ignoring_shell_noise`, `asks_the_shell_with_printenv_for_each_name`, `puts_the_shells_path_first_and_keeps_the_apps_extra_entries` (`desktop/tests_shell_env.rs`) |
| menu.test.ts ×2 | `has_edit_find_in_conversation_shown_with_the_command_key_but_with_the_key_left_to_the_app` and, for Windows' "shown as text", `on_windows_and_linux_the_items_found_nowhere_else_are_in_the_palette` (`desktop/menu_tests.rs`) |
| updates.test.ts ×13 (update mode, update state, Check for Updates…) | Not ported: Electron's updater is not native's. Native's own updater (component 25) has its tests in `update/tests.rs` and `fleet/updates/`; the menu entry's words are in `menu_tests.rs` (24.108). **Owner: 25.** |
| `use-session-notifications.test.ts` ×8 (a client test) | `tests_notifications.rs`, piece F, under their names |

#### The smoke script (24.152)

`bash scripts/smoke-desktop.sh <app> [args]` is `smoke.mjs`: a scratch folder (HOME, the app's data and the Fleet's data inside it, a free
port written into the app's settings so it never takes 6262), the app started on it, the Fleet's instance file, `/readyz`, the UI, `/api/desktop/status`,
a repository listed (git runs from Fleet), `kill -9` of the app and the Fleet gone within 30 s, then the app started again on the same data
(the lock was freed) and killed again. With no display on Linux it runs the app under `xvfb-run`. Run on this machine against the debug build and
the released server, twice, it printed `Smoke test passed.` (Fleet 0.74.0 started in about 1 s; listed the repository in under a second).
Windows needs a PowerShell twin (no SIGKILL in bash): **25 runs it on each package**.

#### The live run (Linux, 20 November 2026)

The debug build under Xvfb with a session D-Bus (`dbus-run-session`, `dbus-monitor` on the notification and tray interfaces), a scratch HOME, runtime
and data folders, the released v0.72.0 server wrapped so it waits 5 s before it starts (so the splash can be seen), `Fleet__Auth__RequireToken=true`,
a scripted model, no window manager and no notification daemon or tray host. Nothing but the scratch folders and its own ports was touched.

| Step | What happened |
|---|---|
| Nothing running; app started | The splash (`1-live-splash.png`), then the Fleet's instance file with `desktop: true` and `requiresToken: true`, then the shell. |
| The key | The log said "using this machine's access token from Fleet's data folder"; the shell loaded with no card (`2-live-shell-with-key.png`). |
| The login shell | The app's own PATH was `/usr/bin:/bin`; the server's PATH had the folder a scratch `~/.bash_profile` added (`opencode` ran, so sessions could start). |
| Quit with a session working | The palette's "Quit Fleet" asked "A session is still working. Quitting Fleet stops them." Cancel kept it (`3-live-quit-question.png`). |
| Window place | Moved to 300,180 at 1000x640, then closed. `settings.json` held `"window": {x: 300, y: 180, width: 1000, height: 640}`; the next start opened there (`5-live-reopened-where-left.png`). |
| Close with a session working | The window went (no visible window) and the app kept running. D-Bus saw `Notify("Fleet", …, "Fleet is still running", "A session is still working. Quit from the tray icon when you're done.", actions [default, Open], hints desktop-entry, suppress-sound)` and a `RegisterStatusNotifierItem` call to a watcher that wasn't there (no tray host, as the checklist's 24.84 says). |
| Second launch while hidden | Exit code 0 at once, no second window, the first window back (`4-live-second-launch-forward.png`). |
| A session asks a question | With `DesktopNotifications` on, D-Bus saw `Notify("Fleet", …, "Needs an answer", "Asked you a question.", …)` (`6-live-session-asks-notification-posted.png`). The preference was set through the API after the app connected and was seen only after a restart of the app; nothing here says Fleet's own client does otherwise. The daemon was missing, so the app logged one "notification wasn't posted (…ServiceUnknown)" line, as designed. |
| Quit with nothing working | The app and its Fleet were gone and `fleet.instance.json` removed; the Fleet is only stopped because the app started it. |

**Not seen:** the tray icon and its menu (no host here), a real toast or banner (no daemon), the macOS menu bar and Dock, the Windows tray and
toasts; Electron's own native menus and message boxes (an operating system's, Playwright can't draw them).
