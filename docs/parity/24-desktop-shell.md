# Component 24: Desktop shell (checklist and pieces)

Planned on 19 November 2026, from weave-fleet `main` at commit `107c794` (read in a shallow clone). `desktop/` was last
changed before that commit; its files are the same in the v0.72.0 release the checks run against.

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
| 24.3 | Packaged: `<resources>/fleet/WeaveFleet.Api`, content root that folder; `.exe` on Windows | PA; DT "uses the bundled server when packaged", "names the Windows binary with .exe" | Path done (`<exe dir>/fleet/`, mac `Contents/Resources/fleet`); no bundle until 25 | 25 (the bundle) |
| 24.4 | Development: the repository's Release build `src/WeaveFleet.Api/bin/Release/net10.0/<rid>/` | PA; DT "uses the repo's Release build in development" | n/a: the native app isn't built inside weave-fleet; the override covers it | — |
| 24.5 | The other installs (decision 1): CLI install, Fleet's desktop app's copy, `fleet` on PATH | native only | Done (`desktop::paths::candidates`: beside the app, CLI install, Fleet's own app, `fleet` on PATH) | — |
| 24.6 | No binary anywhere: "Fleet isn't where the app expects it: <path>" (native: the places it looked) | SRV `launch` | Words done (`paths::not_found_message`, one place: Fleet's sentence; several: the list); page in B | 24 B (page) |

### 2. Starting, watching and stopping it (SRV, MAIN `connect`)

| # | Row | Source | Native | Owner |
|---|---|---|---|---|
| 24.10 | Look first: a Fleet already running on this data is used, never a second (`findRunningFleet`) | MAIN `connect` | Done (discovery, more places than Fleet) | — |
| 24.11 | Environment: the app's own minus `ELECTRON_*`, `URLS`, `ASPNETCORE_URLS`, `DOTNET_URLS`, `ASPNETCORE_ENVIRONMENT` (case-insensitive), plus `ASPNETCORE_ENVIRONMENT=Production`, `Fleet__Host=127.0.0.1` (load-bearing: loopback sign-in), `Fleet__Port`, `Fleet__DatabasePath`, `Fleet__AnalyticsDatabasePath` (`fleet-analytics.db`), `Fleet__DataProtection__KeyPath` (`fleet-keys`, created first), `Fleet__Desktop__Enabled=true`. Native also drops `FLEET_NATIVE_TOKEN` | SRV `buildServerEnv`; DT "runs Fleet in desktop mode…", "drops Electron's variables…" | Done (`desktop::server::server_env`) | — |
| 24.12 | Arguments `--urls http://127.0.0.1:<port> --contentRoot <root>`, working folder the data folder, stdio all piped, no console window on Windows (`windowsHide`: `CREATE_NO_WINDOW`) | SRV `launch` | Done (`desktop::server`: `--urls`, `--contentRoot`, cwd the data folder, piped stdio, `CREATE_NO_WINDOW`) | — |
| 24.13 | Port: the one used last time if free (keeps the client's local storage; for native, its settings) else 6262 else any free one. "Free" = nothing accepts a loopback connect within 1 s **and** a 127.0.0.1 bind succeeds (Windows binds beside a 0.0.0.0 holder) | SRV `pickPort`, `canListen`; MAIN `DEFAULT_PORT`; DT "ports" ×3 | Done (`pick_port`, `can_listen`; preferred ports are a list, the caller passes the remembered one then 6262) | — |
| 24.14 | Ready: `/readyz` every 250 ms (1 s timeout each) for up to 60 s; stops waiting if the process exits | SRV `waitUntilReady`, INS `isReady` | Done (the supervisor looks every 250 ms, 1 s each, 60 s; stops at the exit) | — |
| 24.15 | Ready → remember the port, connect the app to it, marked *owned* | MAIN `onServerEvent` "ready" | Done (`desktop::start::watch`: remembers the port in `settings.json` as `desktopPort`, connects, marks `owned`) | — |
| 24.16 | Exit code 75 (another Fleet took the data first) → look for that Fleet every 500 ms for 30 s and use it; else "Another Fleet is using your data, but it isn't answering." | SRV exit; MAIN "taken" | Event done (`Event::Taken`, no restart after it); wiring and words in B | 24 B (wiring, words) |
| 24.17 | Crash → restart: at once the first time in a minute, then 1 s, 2 s, 4 s; the fifth crash in a minute gives up: "Fleet keeps stopping unexpectedly." | SRV `restartDelay`; DT "restarts" ×3 | Done (`restart_delay`; `Event::Restarting`, then `Failed`) | — |
| 24.18 | Never answered within 60 s while running → "Fleet started but never answered." | SRV | Done (`Event::Failed`) | — |
| 24.19 | Spawn failure → "Couldn't start Fleet: <error>" and a log line `── could not start Fleet: …` | SRV | Done (`Event::Failed`, log line) | — |
| 24.20 | Stop: close stdin (the server's `StandardInputWatcher` shuts down cleanly with its agents), wait 20 s, then SIGTERM, 5 s, SIGKILL (Windows: terminate) | SRV `stop` | Done (`FleetServer::stop`: stdin, grace, SIGTERM, 5 s, kill; Windows: stdin, then terminate) | — |
| 24.21 | The app dying any way at all closes the pipe, so the server stops by itself and frees the lock (`scripts/smoke.mjs` kills the app and checks) | server `StandardInputWatcher`; smoke | Done in the server (its stdin closes with the app); the live check against the released server is still open (see piece B) | 24 B (live check open) |
| 24.22 | Stopped only if the app started it; a Fleet it found keeps running after quit | MAIN `before-quit`, `fleet.owned` | Done (`StartState::owned` is only set for a server this app started; quit stops that one, never a found one) | — |
| 24.23 | Log file (decision 6): header line per start, stdout and stderr appended, exit line, rotate to `.1` past 10 MB | SRV `openLog` | Done (`desktop::log`) | — |
| 24.24 | A restart after a crash fires "ready" again with the same URL; the app doesn't reload a window already on it | MAIN "ready" | Done (`watch` skips a second `Ready` for the address it is on) | — |
| 24.25 | Restarting: log `Fleet stopped (exit code N); restarting in M ms.` | MAIN "restarting" | Done as the event (`Event::Restarting`); the log line is the app's to write in B | — |
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
| 24.34 | "Fleet already runs on this computer" card: 470 wide, padding 22×24, radius 12, border; 40 px icon; h1; "<host> runs Fleet in the background (started <d MMM>). The desktop app will use it: the same sessions, harnesses and machines, and it keeps running when you close the window."; detail box "<host> · Fleet <version>" (12.5 px, radius 10, `--main-bg`); **Use it** (accent, autofocus, disabled once pressed) | running-fleet.html | None | 24 D |
| 24.35 | Splash and error replace the shell; the rail and status bar aren't there in Fleet's (it's a different page) | MAIN | Done (the pages replace the whole window, as in Fleet) | — |

### 4. A Fleet that wants its key (a `fleet node`) (MAIN `useRunning`, `openRunning`, INS)

| # | Row | Source | Native | Owner |
|---|---|---|---|---|
| 24.40 | Instance file's `requiresToken` (true only when exactly `true`; missing = false) | INS `parseInstance`; DT "knows a Fleet that wants its key…" | None (`instance.rs` lacks it) | 24 D |
| 24.41 | Instance file rules: pid a positive integer, URL loopback `http` only, version default "unknown", trailing `/` trimmed | INS; DT "reads what the server writes", "rejects %s" ×6, "only trusts loopback URLs" | Done (`instance.rs`, with tests) | — |
| 24.42 | First time, the "Fleet already runs" page; **Use it** saves `useRunningFleet: true` and goes on; later starts skip the page | MAIN `useRunning`, `use-running-fleet` | None: the key is adopted silently | 24 D |
| 24.43 | The key: `<db name>.machine.json` beside the database, `accessToken`, at least 32 characters | INS `machineFileFor`, `parseAccessToken`; DT "is in the machine file…", "is read from what the server saves", "is not taken from %s" ×5, "is read from disk…" | Done with 16 as the minimum (`local_token.rs`); move to 32 | 24 D |
| 24.44 | Can't read it: "Fleet runs on this computer, but the app can't read its key. The key is in fleet.machine.json beside its database." | MAIN `openRunning` | None (logs only) | 24 D |
| 24.45 | Refused: "Fleet runs on this computer, but it didn't accept the key the app read. If you set WEAVE_FLEET_AUTH_TOKEN for it, restart it without that variable." | MAIN; DT "reports a key Fleet turned away…" | None | 24 D |
| 24.46 | Sign-in is `POST /auth/token-login {token}` for Electron's cookie jar | INS `signIn`; DT "signs the window in…" | n/a: native sends the key on every request (`fleet::auth`); the check that it's accepted is a call with it | — |

### 5. The window (MAIN `createWindow`, SET)

| # | Row | Source | Native | Owner |
|---|---|---|---|---|
| 24.50 | 1360×880 by default, minimum 720×480, title "Fleet", the app icon | MAIN | Size and minimum done; title is "Weave Fleet"; icon comes with 25's packages | 24 G (title) |
| 24.51 | Save bounds and maximized 400 ms after a move or resize, not while minimized or full screen; normal bounds when maximized | MAIN `saveBounds` | None (no move event in Rune: poll `placement()`, decision in G) | 24 G |
| 24.52 | Restore saved bounds only if at least 100×100 of them is on a screen now and they're at least 400×300; else default size, centred | SET `restorableBounds`; DT "window bounds" ×3 | None | 24 G |
| 24.53 | Open maximized when it was | MAIN | None | 24 G |
| 24.54 | Background before the first paint (`#0d0d10` / `#F3F2EF`) | MAIN | n/a: Rune paints the first frame itself | — |
| 24.55 | `autoHideMenuBar` on Windows and Linux | MAIN | n/a (no menu bar there) | — |
| 24.56 | Spell check in fields | MAIN `spellcheck: true` | Out of reach: Rune's text fields have no spell checker | — |
| 24.57 | Toggle full screen (View menu; the palette entry waits on 24) | MENU `togglefullscreen`; `ui/commands.rs` | Palette entry drawn, `Run::Waiting(24)` | 24 H |
| 24.58 | Wayland hides window positions: restore the size only | Rune | — | 24 G |

### 6. One instance (MAIN)

| # | Row | Source | Native | Owner |
|---|---|---|---|---|
| 24.60 | A second launch quits and brings the first one's window forward (restored, shown, focused) | MAIN `requestSingleInstanceLock`, `second-instance` → `showWindow` | None | 24 G |
| 24.61 | A launch while the window is closed to the tray makes the window again and loads the Fleet (or connects) | MAIN `showWindow` | None | 24 E |
| 24.62 | macOS: Dock click (`activate`) shows the window | MAIN `activate` | None | 24 E (`on_reopen`) |

### 7. Closing and quitting (MAIN `onWindowClose`, `before-quit`)

| # | Row | Source | Native | Owner |
|---|---|---|---|---|
| 24.70 | macOS: closing the window hides it; the app stays in the Dock | MAIN | None (closing quits) | 24 E |
| 24.71 | Windows/Linux, a Fleet the app didn't start: closing quits | MAIN | Done (closing quits) | — |
| 24.72 | Windows/Linux, its own Fleet: ask `GET /api/desktop/status` (2 s timeout; 0 on any failure); sessions working → hide to the tray, else quit | MAIN `onWindowClose`, `workingSessions`; `DesktopEndpoints.cs` (busy or retry, the user's own) | None | 24 E |
| 24.73 | Quitting with its own Fleet and sessions working asks: "A session is still working" / "<n> sessions are still working", detail "Quitting Fleet stops them.", buttons **Quit Fleet** / **Cancel** (Cancel the default) | MAIN `confirmQuit`, `confirmStoppingSessions` | None. Rune's `on_quit` can't cancel a system quit (macOS Cmd-Q, logout): the dialog covers quits the app starts (tray, menu, palette); write the smallest case in `docs/RUNE.md` | 24 E |
| 24.74 | Confirmed: stop its Fleet (24.20), then quit | MAIN | None | 24 E |
| 24.75 | The updater's restart stops its own Fleet first and waits (else the new process finds a Fleet that is stopping) | MAIN `installUpdate` | None (`update::install` restarts at once) | 24 E |
| 24.76 | Updating with sessions working asks: "Updating restarts Fleet, which stops them.", **Update Now** / **Cancel** | MAIN `installUpdate` | Native's updater dialog has no such warning | 9 V (the card), 24 E (the count) |

### 8. Tray (MAIN `hideToTray`)

| # | Row | Source | Native | Owner |
|---|---|---|---|---|
| 24.80 | Icon `tray.png`; menu **Show Fleet**, separator, **Quit Fleet**; a click shows the window | MAIN | None | 24 E |
| 24.81 | Tooltip "Fleet: a session is working" / "Fleet: <n> sessions are working" | MAIN | None | 24 E |
| 24.82 | Showing the window removes the tray icon | MAIN `showWindow` | None | 24 E |
| 24.83 | macOS: no tray (the Dock) | MAIN (darwin returns early) | Rune has no macOS tray: not needed | — |
| 24.84 | Linux: shown only where a tray host runs (KDE, most bars; GNOME needs an extension) | Rune `tray.rs` | — | 24 E (note it; with no host the window can't come back but by a second launch, 24.60) |

### 9. Notifications

| # | Row | Source | Native | Owner |
|---|---|---|---|---|
| 24.90 | On hiding to the tray: "Fleet is still running", "A session is still working. Quit from the tray icon when you're done." / "<n> sessions are still working. …", silent, if the system can | MAIN `hideToTray` | None | 24 F |
| 24.91 | Session notifications: when `DesktopNotifications` is "true" and a `session_notification` event comes for a session not in front (open, window visible and focused): title `payload.title` or "Weave Fleet", body `payload.body`, tagged with the session id (one per session) | `use-session-notifications.ts` | None (the switch is saved, 16 piece D) | 24 F |
| 24.92 | Click: window forward, that session opens | same | None | 24 F |
| 24.93 | Opening the session, or the window coming forward on it, takes its notification down | same `clearFor` | None | 24 F |
| 24.94 | Fleet asks the browser for permission; native has none to ask (macOS asks on the first post itself) | `requestNotificationPermission`; 16 rows 2.12, 2.13 | Open in 16 | 24 F (the macOS case), 16 |
| 24.95 | Focus is reported to the server only while recap or notifications are on | 02 row 6.6, `presence.rs` `REPORT_FOCUS` | Always reported | 24 F |
| 24.96 | Linux: none from Rune; freedesktop notifications over D-Bus (decision 8) | — | — | 24 F |
| 24.97 | macOS: only a bundled app can post (`rune-notify::available()`); an unbundled build posts nothing | Rune | — | 24 F, 25 |

### 10. Menus (MENU)

| # | Row | Source | Native | Owner |
|---|---|---|---|---|
| 24.100 | macOS "Fleet": About, **Check for Updates…**, Services, Hide, Hide Others, Show All, Quit | MENU | None | 24 H |
| 24.101 | Windows/Linux "File": **Quit Fleet** | MENU | None (decision 9: palette) | 24 H |
| 24.102 | Edit: Undo, Redo, Cut, Copy, Paste, Select All, then **Find in Conversation** shown with ⌘F/Ctrl+F but the key left to the app (`registerAccelerator: false`; Windows shows "Find in Conversation\tCtrl+F" as text) | MENU; MT ×2 | Key done (03 P); `State::find_request` | 24 H (macOS item; `standard_menus` brings Edit) |
| 24.103 | View: Reload, Force Reload, Toggle DevTools | MENU | n/a: no page to reload | — |
| 24.104 | View: Actual Size, Zoom In, Zoom Out | MENU | The palette's zoom entries (component 9) and Appearance's font size (16) | 9, 16 |
| 24.105 | View: Toggle Full Screen | MENU | see 24.57 | 24 H |
| 24.106 | Window (role): Minimize, Close (and Zoom on macOS) | MENU | None | 24 H (`standard_menus`) |
| 24.107 | Help: **Report a Problem…** (shows the window, opens the report), **Check for Updates…** (not macOS), **Open Logs Folder**, **Open Fleet Data Folder** | MENU, PRE `onReportProblem` | Report opens from the rail's Help (9) | 24 H |
| 24.108 | Check for Updates… says in a dialog: "This build of Fleet doesn't update itself." / "You're on the latest version (<v>)." / "Fleet is already checking for updates." / "Fleet <v> is downloading. Fleet lets you know when it's ready to install." / "Couldn't check for updates: <error>"; a found one opens the update card | UPD `updateCheckMessage`; UT "Check for Updates… from the menu" ×3 | Native updater has its own words (builds, not versions) | 25 / 9 V (words); 24 H (the menu entry wakes the checker) |
| 24.109 | Right-click: up to 5 spelling suggestions and "Add to Dictionary" | MENU `showContextMenu` | Out of reach (no spell checker) | — |
| 24.110 | Right-click on a link (http, https, mailto): **Open Link in Browser**, **Copy Link** | MENU | None | 24 I |
| 24.111 | Right-click in a field: Cut, Copy, Paste (each enabled by what it can do), separator, Select All | MENU | None | 24 I |
| 24.112 | Right-click on selected text: Copy | MENU | None | 24 I |
| 24.113 | No trailing separator; nothing to show → no menu | MENU | — | 24 I |

### 11. Links (LNK)

| # | Row | Source | Native | Owner |
|---|---|---|---|---|
| 24.120 | Fleet's own pages stay in the window | LNK; DT "keeps Fleet's own pages in the window" | n/a (native draws them) | — |
| 24.121 | http, https, mailto open in the system browser | LNK; DT "opens web and mail links in the browser" | Done (Rune `open_url`) | — |
| 24.122 | Everything else is blocked (`javascript:`, `file:`, `vscode:`, not a URL) | LNK; DT "blocks everything else" | Done (Rune refuses other schemes); a test here names it | 24 I (the test) |
| 24.123 | Web views' own navigation | MAIN `will-navigate` | 14 owns the web views | 14 |

### 12. The login shell's environment (ENV)

| # | Row | Source | Native | Owner |
|---|---|---|---|---|
| 24.130 | Before starting Fleet (not on Windows): run `$SHELL` (else `/bin/zsh` on macOS, `/bin/bash` on Linux) `-ilc` with a command that prints each of `PATH`, `SSH_AUTH_SOCK`, `LANG`, `LC_ALL`, `LC_CTYPE`, `HOMEBREW_PREFIX`, `HOMEBREW_CELLAR`, `HOMEBREW_REPOSITORY` between markers; `TERM=dumb`; 5 s timeout; 1 MB cap | ENV; DT "asks the shell with printenv for each name" | None | 24 C |
| 24.131 | Read each between its markers, ignoring shell noise; empty values dropped | ENV `parseCaptured`; DT "reads each variable between its markers…" | None | 24 C |
| 24.132 | PATH: the shell's first, then entries only the app had, no repeats | ENV `mergePath`; DT "puts the shell's PATH first…" | None | 24 C |
| 24.133 | The others only when the app has none | ENV | None | 24 C |
| 24.134 | Off the UI thread, before the server starts (it can take seconds) | MAIN `start` | — | 24 C |

### 13. The bridge (PRE, `client/src/lib/desktop.ts`), as native calls

| # | Row | Source | Native | Owner |
|---|---|---|---|---|
| 24.140 | `version`, `platform` | PRE | `version_line()`; report's app line (9) | — |
| 24.141 | `openLogs`, `retry`, `useRunningFleet` | PRE | — | 24 B, D |
| 24.142 | `getUpdateState`, `onUpdateState`, `checkForUpdates`, `installUpdate`, `onShowUpdate` | PRE, UPD | Native updater (25) | 25, 9 V |
| 24.143 | `captureWindow` | PRE | Out of reach (09 row 9.12) | — |
| 24.144 | `onReportProblem`, `onFindInConversation` | PRE | Menu items call `report_open` and `find_request` | 24 H |

### 14. Updates and packages (UPD, `electron-builder.config.js`)

| # | Row | Source | Native | Owner |
|---|---|---|---|---|
| 24.150 | Update mode: install on Windows and AppImage, notify on macOS and the .deb, off unpackaged or `FLEET_DESKTOP_UPDATES=off`; check after 10 s and every 4 h; `nextState` rules | UPD; UT "update mode" ×3, "update state" ×7 | Native updater: every 4 h, its own rules | 25 |
| 24.151 | Installers with the server inside (`resources/fleet`), app id `com.pgermishuys.fleet`, Linux `StartupWMClass: Fleet`, category Development | builder config | None | 25 |
| 24.152 | Smoke test of a packaged app: start on scratch data, Fleet answers, kill the app hard, Fleet stops and frees the lock | `scripts/smoke.mjs` | None | 24 B (as a live check), 25 (in CI on packages) |

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

**Built:** `src/desktop/start.rs` (`Config`, `begin`, `StartState`, `Phase`), `src/view/start_page.rs`, `Sidebar::desktop_port`, `FleetServer::stopper` (a shareable handle: the server itself holds a `Receiver`, which isn't `Sync`), scenes `start-splash`/`start-error` (+ `-light`), `src/desktop/start_tests.rs` (11 tests). **Differences:** the owned server is a `Stopper` in `StartState`, not the whole `FleetServer`; quitting closes its input and waits on a thread, with `rune::exit_deadline(25 s)`; with no server binary found the old 3 s search stays (decision 4) and the notice names the places, so row 24.6's page isn't shown. **Open:** the live check (Xvfb, the released server, SIGKILL of the app) and Windows/macOS runs; row 24.21's live part and 24.152 stay open. Fleet's own pictures are the reduced-motion ones in this folder.

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

### Piece D: a Fleet that wants its key

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

### Piece E: closing, quitting and the tray

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

### Piece F: notifications

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

### Piece J: sweep

- **Rows:** every row left open above gets its reason or owner; 24.152's smoke check written as `scripts/smoke-desktop.sh`
  for 25's packages to run; README's "what works"; `docs/PORT.md` row 24 set to done.
- **Depends on:** A–I.
- **Pictures:** every start page beside Electron's once more, both themes.
