# Component 25: Packages and updates (checklist and pieces)

Planned on 22 November 2026, from weave-fleet `main` at commit `c1a48bd` (read in a shallow clone; the latest release,
v0.74.0, ships the same `desktop/`). Self-update was built early (`src/update/`); this component is what is left:
installers with Fleet's server inside, signing, the bundle and icon, the update path for those installers, and the
Windows smoke run that component 24 left over.

Sources (weave-fleet): `desktop/electron-builder.config.js` (EB), `desktop/package.json`, `desktop/src/updates.ts` (UPD),
`desktop/src/paths.ts` (PA), `desktop/scripts/smoke.mjs` (SMK), `check-bundled-server.mjs` (CBS), `merge-update-manifests.mjs`
(MUM), `desktop/build/icon.png`, `desktop/static/` (icons), `.github/workflows/desktop-packages.yml` (DP),
`.github/actions/package-desktop/action.yml` (PD), `.github/workflows/release.yml` (REL, the matrix and the publish step),
`RELEASE.md`; tests `desktop/test/updates.test.ts` (UT), `check-bundled-server.test.mjs` (CT), `merge-update-manifests.test.mjs`
(MT). On the client side, `components/settings/DesktopAppUpdates.vue` (the app row; built by component 16, see below).

## What Fleet ships today

- **Four installers per release**, built by electron-builder around the Electron app, each with the AOT server in
  `resources/fleet` (outside the asar so it stays executable): Linux `AppImage` and `.deb` (x64), macOS `.dmg` and `.zip`
  (arm64), Windows NSIS `-setup.exe` (x64 and arm64). Names `Fleet-<version>-<os>-<arch>.<ext>`, plus unversioned copies
  (`Fleet-mac-arm64.dmg`, `Fleet-win-x64-setup.exe`, …), a `.sha256` for each, `.blockmap` files and the update feeds
  `latest.yml` (the two Windows feeds merged by MUM), `latest-linux.yml`, `latest-mac.yml`. A release is 120 to 200 MB per file
  (v0.74.0: AppImage 165 MB, dmg 196 MB, Windows x64 131 MB, deb 130 MB), most of it Electron.
- **App id** `com.pgermishuys.fleet`, product name "Fleet", Linux executable `fleet-desktop`, `StartupWMClass: Fleet`, category
  Development, synopsis "Run and watch many AI coding agents at once"; macOS category `public.app-category.developer-tools`.
- **Signing: none that costs a secret.** macOS is *ad-hoc* signed (`identity: "-"`) so Apple silicon will run it ("unidentified
  developer, Open Anyway"); hardened runtime and notarisation are switched on only when the secret `CSC_LINK` exists (the
  certificate; electron-builder also reads `CSC_KEY_PASSWORD`, and notarisation would need Apple's `APPLE_ID`,
  `APPLE_APP_SPECIFIC_PASSWORD`, `APPLE_TEAM_ID`). None of those is set in weave-fleet's workflows; Windows is not signed (no
  `WIN_CSC_LINK`, no signing service): SmartScreen warns. The update mode follows from that (UPD): Windows and the AppImage
  install by themselves; macOS (unsigned: can't install into it) and the `.deb` (needs a password prompt) only link to the release.
- **Checks in CI** (PD): the server has its mod host; `check-bundled-server.mjs` proves the app carries the server byte for
  byte (every Mach-O compared up to its signature); the packaged app is smoke-tested (`smoke.mjs`: starts on scratch data,
  Fleet answers, kill the app hard, Fleet stops and frees the lock); the installers get a sha256 file each.
- **The packages are built from the server the release's own build job compiled and smoke-tested**; the desktop workflow
  builds the same on pull requests that touch `desktop/`, with no release.

## What the native app has today

| Thing | Where | State |
|---|---|---|
| Self-update: the status-bar item "Update available · build N", one dialog, download, SHA-256 and size check, replace in place, restart; checks on start and every 4 h (20 min after a failure) | `src/update/`, `view::update` | Done. Windows: the exe swapped with `self-replace`; macOS: the `.app` replaced from the `.dmg` (or the disk image opened for a drag when the folder can't be written); Linux: the executable replaced from the tar |
| Fleet app row on Updates & about, and the notice card's update states | `src/view/settings/updates_app.rs`, `src/fleet/settings/updates_page.rs`, `tests_settings_updates.rs` | Done by component 16 (13.21 to 13.23: "Fleet app", build, status line, Check for updates, Update and restart) |
| Rolling `latest` release: three assets and a `latest.json` (`commit`, `built_at`, `build`, per-platform `name`, `size`, `sha256`) | `.github/workflows/ci.yml` (`package`, `publish`), `scripts/publish-release.sh`, `verify-release.yml` | Done: `fleet-native-windows-x64.zip` (the exe and README), `fleet-native-macos-arm64.dmg` (`Fleet Native.app`, ad-hoc signed, Applications link), `fleet-native-linux-x64.tar.gz` (exe, `.desktop`, icon, README) |
| Bundle and icon | `assets/icon/` (`.icns`, `.ico`, `icon.png`), `build.rs` (Windows icon and version information via `winresource`), `scripts/make-mac-app.sh` (Info.plist, icon, ad-hoc signature), `packaging/fleet-native.{desktop,png}` | Done for the app alone: name "Fleet Native", id `io.tryweave.fleet-native`, version `0.0.<build>`; **not** Fleet's name, id or `StartupWMClass` (rows 25.4, 25.5) |
| Where the app looks for a server | `src/desktop/paths.rs` | Done: `<exe dir>/fleet/` (macOS `Contents/Resources/fleet`), then the CLI install, then the places Fleet's own installers use (`/opt/Fleet/resources/fleet`, `%LOCALAPPDATA%\Programs\Fleet\resources\fleet`, `/Applications/Fleet.app/Contents/Resources/fleet`). Nothing ships a server there yet |
| A smoke run of a built app | `scripts/smoke-desktop.sh` (Linux and macOS, SIGKILL) | Done; a Windows twin is open (25.40) |
| CI: build, lint and test on three systems; release build of each system's app on `main`; verification of the published assets by hand | `ci.yml`, `verify-release.yml` | Done |

## Decisions this component needs (for the owner, none is blocking the first pieces)

1. **Which package tool.** Fleet uses electron-builder (Node). This repository has no Node ("Cargo only"). Recommendation: no
   packager crate and no Node; plain scripts per system, as `make-mac-app.sh` already is: macOS `.app` and `.dmg` by hand,
   Windows NSIS script run with `makensis` (preinstalled on GitHub's Windows runners), Linux AppImage with `appimagetool`
   downloaded by hash and the `.deb` with `dpkg-deb`. Each script is under 150 lines and tested in CI. The alternative
   (`cargo-packager`, one config, builds all of them) costs a tool install and its own opinions; piece B names the fallback.
2. **Which server goes in.** Recommendation: the released CLI archive of weave-fleet (`fleet-v<version>-<rid>.tar.gz` or
   `.zip`, whose `app/` folder is the AOT publish output the desktop app wants), pinned by version and SHA-256 in
   `packaging/server-version`, so no .NET build runs here. Bumping the pin is a one-line pull request; the app is shown a
   new server only with a new app build (see 25.12).
3. **What an installer's update does.** Recommendation: keep one `latest.json` and one replace-in-place updater, but make the
   asset for each system the *installer's own package*, so what is downloaded is what a fresh install would put there
   (the server included), and say in the dialog when the download is large (about 100 MB more than today).
4. **Windows on ARM and macOS on Intel.** Fleet ships Windows arm64; the native updater has keys for three platforms only
   (`manifest::key_for`). Recommendation: out of scope until a runner for them is in the plan (25.30); Intel Macs are not shipped
   by Fleet either.
5. **Signing.** No secret exists for it in either repository. Recommendation: plan unsigned (macOS ad-hoc, Windows none)
   packages, and the signing step behind a "secret missing, skipped" check, exactly as Fleet does; the owner adds the
   secrets (listed in rows 25.20 to 25.27) when a certificate exists.

## Checklist

Columns: row, what, source, native state, owner (a piece letter, or "—" when it needs nothing). "Done" rows are checked
against the code; nothing below was built by this planning session.

### 1. What is in a package

| Row | What | Source | Native state | Piece |
|---|---|---|---|---|
| 25.1 | Fleet's server inside each package, from a pinned release, with its UI, `appsettings` and mod host (`mods-host/host.js`); under `resources/fleet` in Fleet's | EB `extraResources` (`!**/*.pdb`, `!**/*.dbg`), PD "server has the mod host" | None | A |
| 25.2 | The packaged server is the one the pin names: SHA-256 of the downloaded archive checked; the staged folder holds exactly the archive's `app/` files, byte for byte (macOS: Mach-O compared up to the signature) | CBS, CT ×8 ("sees a re-signed file as the same", "allows zero padding before a signature codesign placed further on", "catches one changed byte of code", "catches bytes hidden where only padding may be", "compares each architecture of a fat file", "passes an app that ships the server as built, re-signed Mach-O files included", "fails on a changed, missing or extra file", "fails when nothing was packaged") | None. Port as a Rust test binary or a script beside the packages, tests under CT's names | A |
| 25.3 | The server is found at the path the app expects: Linux `<install>/fleet/` and `/opt/Fleet/resources/fleet`; Windows beside the exe and `%LOCALAPPDATA%\Programs\Fleet\resources\fleet`; macOS `Contents/Resources/fleet` | `paths.ts`, `paths.rs` (24.3) | Done in `src/desktop/paths.rs`; piece A adds a test of each package's real folder layout | A |
| 25.4 | Names: product "Fleet", app id `com.pgermishuys.fleet`, executable `fleet-desktop` (Linux), `StartupWMClass: Fleet`, category Development, synopsis "Run and watch many AI coding agents at once" | EB | None: the native app is "Fleet Native", id `io.tryweave.fleet-native`, executable `fleet-native`. **Decision to take in piece B**: it is an experiment that must not replace Fleet's own install, so keep the distinct name and id and give both a distinct data and install folder; the window's title stays "Fleet" (24.50) | B, C, D |
| 25.5 | The window's class on Linux (taskbar grouping, icon): `StartupWMClass` matches the window's app id | EB `linux.desktop` | `packaging/fleet-native.desktop` has none; the window sets no app id | B |
| 25.6 | Package version: `0.0.<build>` on every system (the build number only goes up); `--version` prints version, build and commit; the Windows exe's file and product version carry it | EB `extraMetadata.version`, REL "Resolve the desktop version" | Done: `FLEET_NATIVE_BUILD`, `build.rs`, `make-mac-app.sh`; the Linux and Windows installers need it too | B, C |
| 25.7 | Size: a native package is expected near 40 MB plus the server (about 100 to 130 MB), against Fleet's 130 to 200 MB; the numbers go in the pull requests | measured v0.74.0 assets | — | A, G |

### 2. The three installers

| Row | What | Source | Native state | Piece |
|---|---|---|---|---|
| 25.10 | **Linux AppImage**: runs with no install, carries the server, icon and desktop entry; `APPIMAGE` is set when it runs (the updater needs it) | EB `linux.target` | Tar only (exe, `.desktop`, icon); no AppImage | B |
| 25.11 | **Linux `.deb`**: installs to `/opt/Fleet` (`resources/fleet` inside), a `/usr/bin` link, the desktop entry and icons (hicolor sizes), removes cleanly; `Depends` lists the libraries the app opens (libxkbcommon-x11, Vulkan loader, and WebKitGTK for the web views) | EB `deb` defaults | None | B |
| 25.12 | The Linux tar stays for the updater until B moves it to the AppImage; the release asset names and `latest.json` keys stay one per system | `publish-release.sh`, `manifest.rs` | Done | B, F |
| 25.13 | **Windows installer** (NSIS `-setup.exe`, per user, no admin): copies the exe and `fleet\` to `%LOCALAPPDATA%\Programs\Fleet-Native`, a Start-menu shortcut with the icon, an uninstaller entry in Settings → Apps with the version, uninstall removes the app and leaves the data folder; running it twice upgrades in place; closes a running copy first or says so | EB `win.target`, `nsis` | None: a zip of the bare exe | C |
| 25.14 | Windows first screen of the installer: Fleet's NSIS default (one-click, progress bar, then launches the app) picture | `nsis` default (one-click) | None | C |
| 25.15 | **macOS `.dmg`**: "Fleet Native.app" with the server in `Contents/Resources/fleet`, an Applications link, the icon; opens to the drag-to-Applications view (Fleet's volume title "Fleet <version>") | EB `mac`, `dmg.title` | Done without the server | D |
| 25.16 | Mac `.app` Info.plist: id, name, version, category, `LSMinimumSystemVersion` from the binary, `NSHighResolutionCapable`; no `LSUIElement` (the app has a dock icon); usage strings the app needs, if any | `make-mac-app.sh` | Done; usage strings only if a feature asks for one | D |
| 25.17 | macOS `.zip` of the app, which Fleet's own updater uses (electron-updater) | EB `mac.target` | Not needed: the native updater reads the `.dmg` | — |
| 25.18 | Icons at every size each system wants: `.ico` with 16 to 256, `.icns` with retina sizes, PNGs 16 to 512 for the deb, the tray image; same artwork as Fleet's `desktop/build/icon.png` | `desktop/build`, `static/icon.png`, `tray.png` | `.ico`, `.icns`, `icon.png`, `tray.png` exist (24 owed the icon: **done**, the picture is the check); hicolor set for the deb is B's | B |

### 3. Signing and notarising (none is run without its secrets)

Every row is one step in the packaging workflow with `if: secrets are set`, printing "not signed: <name> is not set" when
it skips, so a pull request and a fork build the same unsigned packages the release does. **No secret is added, printed
or guessed by this component's pieces**; the owner creates them in the repository's settings when a certificate exists.

| Row | What | Secrets it needs | Native state | Piece |
|---|---|---|---|---|
| 25.20 | macOS Developer ID signing: the app, the server and every `.dylib` and helper inside, deepest first, with the hardened runtime and a timestamp; entitlements for what the server and the web views need (allow JIT, unsigned executable memory, library validation off, network client and server) | `APPLE_CERTIFICATE` (base64 `.p12`), `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY` | Ad-hoc only (`codesign -s -`) | E |
| 25.21 | macOS notarisation (`xcrun notarytool submit --wait`, `stapler staple` on the app and the `.dmg`) and a check with `spctl --assess` | `APPLE_API_KEY` (the `.p8`, base64), `APPLE_API_KEY_ID`, `APPLE_API_ISSUER` (or `APPLE_ID`, `APPLE_APP_SPECIFIC_PASSWORD`, `APPLE_TEAM_ID`) | None | E |
| 25.22 | Without those: ad-hoc, as today; the package says so in `latest.json`'s notes and the README ("unidentified developer, Open Anyway") | none | Done for the app alone | E |
| 25.23 | Windows Authenticode signing of the exe, the server's exe and dlls, and the installer, with a timestamp | One of: `WINDOWS_CERTIFICATE` (base64 `.pfx`) and `WINDOWS_CERTIFICATE_PASSWORD`; or Azure Trusted Signing (`AZURE_TENANT_ID`, `AZURE_CLIENT_ID`, `AZURE_CLIENT_SECRET`, endpoint, account, profile). Piece E writes the step for the first and says how to switch | None | E |
| 25.24 | Without it: unsigned; SmartScreen warns on first run; the README says how to continue ("More info", "Run anyway") | none | None | E |
| 25.25 | Linux: a `.sha256` per file; optional detached GPG signature of the AppImage | `GPG_PRIVATE_KEY`, `GPG_PASSPHRASE` (optional) | Hashes are in `latest.json`; no `.sha256` files | B, E |
| 25.26 | The update is only trusted by hash: `latest.json` is read over https from the repository, files are checked for size and SHA-256 before anything is touched. With signing on, the downloaded package's signature is checked too (macOS `codesign --verify`, Windows `Get-AuthenticodeSignature`) before the swap, and a failed check shows "can't verify" and leaves the old app | UPD (electron-updater checks the Windows signature itself) | Hash and size done; the signature check follows E | F |
| 25.27 | A signed build keeps working with `scripts/verify-release.yml` (it checks `codesign --verify --deep --strict` today; add `spctl`, `stapler validate` and the Windows signature when secrets exist) | `verify-release.yml` | Done for ad-hoc | E, H |

### 4. Update path for the installers

| Row | What | Source | Native state | Piece |
|---|---|---|---|---|
| 25.30 | Update modes: Windows installs by itself; Linux AppImage installs; macOS installs when the folder can be written (Fleet only links, because its macOS build is unsigned and electron-updater can't install into it); the `.deb` only links to the release; development builds and `FLEET_NATIVE_UPDATES=off` never check | UPD `updateMode`; UT "installs on Windows and from an AppImage", "only tells you about a new version on macOS (unsigned) and from the .deb", "is off in development and when turned off" | Partly: all installs in place; no `.deb` mode; no off switch (`running_build` is `None` for a dev build, which never checks) | F |
| 25.31 | An update replaces the server too: the new package's `fleet/` folder replaces the old one together with the exe, in one step, and a Fleet the app started stops first (it does today: "the updater stops its own Fleet first") | `updates.ts`, 24.E | Exe only; with the server bundled the whole app folder is replaced | F |
| 25.32 | Windows: run the installer silently over the old one (or replace the folder), keep the data, start the new exe; a failure leaves the old app untouched | `electron-updater` NSIS | Exe swap only | F |
| 25.33 | Linux: AppImage replaces `$APPIMAGE`; a `.deb` install says "Download the new .deb" with the release link, as Fleet's notify mode does | UPD | Tar, replaces the exe | F |
| 25.34 | The dialog says the size ("about 130 MB") before it downloads; progress, cancel and the error words already exist | `src/update/` | Done except the size | F |
| 25.35 | `latest.json` lists, per system, the package the updater downloads (and the installer for fresh installs), with `size` and `sha256`; the old keys keep working for builds already installed (a build installed from a zip updates once from the zip) | `publish-release.sh`, `manifest.rs` | Three assets | F |
| 25.36 | Update states and words of Fleet's updater: idle "Up to date", checking, downloading with percent, ready "v<v> is ready" (Restart to update), available (Download), error "Update check failed", off "This build doesn't update itself"; Help → Check for Updates… dialogs ("You're on the latest version (<v>).", "Fleet is already checking for updates.", "Fleet <v> is downloading…", failure) | UPD `nextState`, `updateCheckMessage`; UT ×13 ("downloads a new version where it can install it", "links to the release where it can't install", "keeps a downloaded update through later checks and errors", "stays ready when a check finds the version it already downloaded", "reports a failed check", "goes back to idle when there's nothing new", "ignores progress when nothing is downloading", the three "Check for Updates… from the menu" cases) | Owned by 9 V and 24.108 for the words and the menu item; this component accounts for each UT case as "native: same rule under tests in `src/update/tests.rs`" or "not applicable: builds, not versions" | F |
| 25.37 | The Windows update feed merge (`latest.yml` from two feeds) | MUM, MT ×4 | Not applicable: one `latest.json` serves every system | — |
| 25.38 | The mac "ready, restart" when the app can't write its own folder: the disk image is opened for a drag | `install.rs` `MacResult` | Done | — |

### 5. Smoke runs and package checks

| Row | What | Source | Native state | Piece |
|---|---|---|---|---|
| 25.40 | **Windows smoke run** (what 24 left over): start the app on scratch data, wait for its Fleet, list a repository, kill the app hard (`Stop-Process -Force`), check the server stops and the lock is free, start again on the same data. PowerShell `scripts/smoke-desktop.ps1`, run on a Windows runner against the installed app | SMK (Windows branches: git's stdin, the instance file) | None. Linux and macOS are `smoke-desktop.sh` | G |
| 25.41 | Smoke each *package*, not the build: Linux unpack the AppImage and `dpkg -x` the `.deb`; Windows run the installer silently into a scratch folder, then the uninstaller; macOS mount the `.dmg` and run from it (best effort as today, the runner's disk service is flaky) | PD "Smoke test packaged app" | Builds only | G |
| 25.42 | The package carries the server as pinned (25.2), has its icon and name (Windows: file and product version, icon extracted as a picture; macOS: `plutil -lint`, `codesign --verify`; Linux: `.desktop` passes `desktop-file-validate`) | CBS, `verify-release.yml` | Partly | G |
| 25.43 | The Windows smoke starts with no Visual C++ redistributable (the exe links the C runtime in) and with the server's own dependencies | `ci.yml` `+crt-static` | Done for the exe | G |
| 25.44 | Packages are built on every pull request that touches packaging, uploaded as artifacts (not released); the full set builds on `main` | DP | Packages build on `main` only | G |

### 6. Elsewhere or out of reach

| Row | What | Why |
|---|---|---|
| 25.50 | Electron's `electron-updater` rules (differential downloads with `.blockmap`, the `app-update.yml`, staged rollouts) | Not Rune's; the native updater downloads a whole file and checks its hash |
| 25.51 | Windows arm64, macOS Intel, Linux arm64 installers | No runner in the plan, no key in `manifest::key_for` (decision 4). Linux arm64 and Windows arm64 are the first two to add once there is one |
| 25.52 | The unversioned `Fleet-…` copies and `install.sh` / `install.ps1` | Fleet's own release channel; the native app has one rolling release |
| 25.53 | A Microsoft Store, Mac App Store, Homebrew cask, winget, Flatpak or Snap listing | Not Fleet's either |
| 25.54 | Notarising and signing without a certificate | Needs the owner's secrets (rows 25.20 to 25.24); planned behind a check, built only so far as it can be tested without them (a dry-run flag that runs every step but the signing call) |
| 25.55 | A picture of the macOS and Windows installers' first screen from this machine | Linux only here: those two are pictured from CI runs (a runner screenshot of the NSIS window; the mounted `.dmg`'s Finder view is not capturable, so its content listing stands in) |

## Fleet's tests, accounted for

| Fleet test | Cases | Native |
|---|---|---|
| `updates.test.ts` | 13 | Row 25.36: each as a native test in `src/update/tests.rs` (rules) or "not applicable" with the reason; piece F |
| `check-bundled-server.test.mjs` | 8 | Row 25.2, same names, in piece A |
| `merge-update-manifests.test.mjs` | 4 | Not applicable (25.37) |
| `desktop.test.ts`, `menu.test.ts` | — | Component 24 (done) |

## Pieces

Each is one pull request from a fresh session working from this file alone. Build order; pictures in the description from
the moment it is opened (AGENTS.md "Pictures, always"). A piece that only changes packaging shows the packaged app
running (a picture of it with the bundled server, "About"/Updates page showing the build, the first screen of the
installer where the system allows) and the files a package holds. All use invented data, scratch folders and no paths.

| Piece | Name | Rows | Touches | Depends on | Tests to port | Pictures |
|---|---|---|---|---|---|---|
| **A** | The server inside | 25.1, 25.2, 25.3, 25.7 | `packaging/server-version` (tag and the four SHA-256s), `scripts/fetch-server.sh` (download, hash, unpack the CLI archive's `app/` to `fleet/`; a `.ps1` twin or Git-bash on Windows), `scripts/check-bundled-server.sh` or a small `tools/` Rust test target (about 300 lines, same machine-code comparison as CBS), `ci.yml` `package` stages `fleet/` beside the exe and the tar/zip/dmg carry it; `src/desktop/paths.rs` test per layout | none | CT's 8 under their names | The packaged app (from the tar, on this machine) starting its own Fleet with no `FLEET_NATIVE_SERVER_BIN`: the splash, then the sessions page with the build and the server's version, both themes; the package's file listing with sizes |
| **B** | Linux packages | 25.4 (Linux), 25.5, 25.6 (Linux), 25.10, 25.11, 25.12, 25.18 (hicolor), 25.25 (hashes) | `scripts/make-appimage.sh`, `scripts/make-deb.sh`, `packaging/fleet-native.desktop` (`StartupWMClass`, `Categories=Development`, `Comment`), `packaging/icons/<size>/`, the window's app id in `src/lib.rs` (`spec()`) if Rune has the call (else a note for `docs/RUNE.md`), `ci.yml` | A | Packages: `desktop-file-validate`, `dpkg-deb --info`, the AppImage's `--appimage-extract` listing, in CI | The AppImage running on this machine under Xvfb (window class and icon), the `.deb` installed into a scratch root (`dpkg -x`) with the app run from it; the `.desktop` file |
| **C** | Windows installer | 25.4 (Windows), 25.6, 25.13, 25.14 | `packaging/windows/installer.nsi`, `scripts/make-installer.ps1`, `ci.yml` (`makensis`), the CI check that installs silently into a scratch folder and uninstalls | A | The install and uninstall, silent, in CI; the file list the installer wrote | A picture from the Windows runner of the installer's window and of the installed app (Start-menu entry, Apps list); the installer's script listing. Not run on macOS or this machine |
| **D** | macOS bundle | 25.15, 25.16 (+ server in `Contents/Resources/fleet`) | `scripts/make-mac-app.sh` (copy `fleet/`, sign inside out, hdiutil volume title), `ci.yml`, a check that every Mach-O inside passes `codesign --verify` | A | CT's Mach-O cases run against the signed app on the macOS runner (the byte-for-byte rule up to the signature) | A picture of the app launched on the macOS runner (its window, its About line) or, where the runner can't show one, the mounted image's listing, `plutil -p` and `codesign -dv` output. Not run on a real Mac |
| **E** | Signing and notarising, behind secrets | 25.20 to 25.25, 25.27 | `scripts/sign-mac.sh`, `scripts/sign-windows.ps1`, `packaging/mac/entitlements.plist`, `ci.yml` steps with `if: env.APPLE_CERTIFICATE != ''` style checks, README "Installing" section | C, D | The skipped-path itself: a workflow test that runs the steps with no secrets and expects "not signed: … is not set" and a green job; the signing calls only dry-run (arguments printed) | The CI log of the skip step; the README's section; no signed package can be shown until the owner adds secrets (said in the pull request) |
| **F** | The updater for installers | 25.26, 25.30 to 25.36 | `src/update/` (`install.rs`: replace the app folder with the server; `mod.rs`: modes, size in the dialog; `manifest.rs`: asset keys per package kind), `scripts/publish-release.sh` and `latest.json`, `FLEET_NATIVE_UPDATES=off`, tests | A, B, C, D | UT's 13 under native names; the existing `scripts/update-check.sh` proof extended to a bundle with a server; tests for the `.deb` notify mode | The update dialog with the size line and the ready state, native beside Fleet's Updates & about "Fleet app" row (same state, both themes), and a recorded run of an update on a scratch release from a loopback server |
| **G** | Package smoke and Windows | 25.40 to 25.44 | `scripts/smoke-desktop.ps1`, `ci.yml` smoke steps per package kind, `verify-release.yml` extended to the new assets | A to D | none of Fleet's; the smoke itself is the test | The Windows runner's log and picture of the smoke run, the Linux and macOS ones; a table of what each ran on which system |
| **H** | Sweep | all | `docs/parity/25-packages-and-updates.md`, `docs/PORT.md` row 25, `README.md` Installing and Licences (AppImage tool, NSIS, if bundled) | A to G | Every UT, CT, MT case accounted for | A walk: download the `latest` assets, install each on the systems available, run the app with its server, take an update; the pictures next to Fleet's own installers where Fleet's can be run (Linux AppImage on this machine) |

## Notes for the pieces

- Never print, add or guess a secret; never trigger a real release, a real update or a tag. Build and test packages in a
  scratch folder, an update against a loopback server (`FLEET_NATIVE_UPDATE_URL`).
- The server archives are about 100 to 130 MB each: download them into a scratch folder, never into the repository, and
  delete them. CI caches the downloaded archive by its pinned hash.
- On this machine only the Linux packages can be built and run; Windows and macOS are CI only. Say so in each pull
  request.
- The release build takes minutes and a lot of memory; build the app in the shared build folder as `docs`/`AGENTS.md` say,
  and package from that.
